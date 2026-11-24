//! Overlap-add: the buffering an FFT runs inside.
//!
//! **Written here rather than borrowed from `nih_plug::util::stft`**
//! (`REQ-PUM-015`). nih-plug has this, and it works; depending on it would move
//! `pumice-core` to the GPL side and close the door on an AU wrapper built
//! against another framework. That is the whole reason `<plugin>-core` exists.
//!
//! ## Hann in, Hann out, hop = N/4
//!
//! Both windows, because the gain applied between them moves: an analysis-only
//! window leaves the discontinuity a changed gain creates at the frame edge,
//! and the overlap sums it into the output. `Σ hann²` over four offsets is
//! **exactly 1.5** — the cosine terms cancel across four phases 90° apart — so
//! the synthesis scale is `2/3`, not a number found by measuring.
//!
//! ## The latency is one whole block
//!
//! An output sample is final only once every frame overlapping it has been
//! added, and the last such frame *ends* one block later. **Overlap does not
//! buy that back**; it adds more contributions to the same sample without
//! making the last one arrive sooner. This module was specified with
//! `block − hop` and that was wrong (`quality::Quality::latency`).
//!
//! ## Why every step is planned up front
//!
//! `QUALITY` is changeable while the transport runs (`PUM-1` confirmed two
//! hosts recover from it), and planning an FFT allocates. So all three plans,
//! all three windows and one scratch big enough for any of them are built in
//! [`Stft::new`]. Switching steps then changes an index.
//!
//! **`realfft`'s `process` allocates** — it calls `make_scratch_vec` on every
//! call. Only `process_with_scratch` is safe here, and that is not obvious from
//! the name.

use realfft::num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

use crate::quality::{MAX_BLOCK, OVERLAP, Quality};

/// Pumice is 2→2 and 1→1 (`REQ-PUM-011`), so two is the most there can be.
pub const MAX_CHANNELS: usize = 2;

/// Bins in the largest transform: a real FFT of `n` gives `n/2 + 1`.
pub const MAX_BINS: usize = MAX_BLOCK / 2 + 1;

/// One prepared transform size.
struct Plan {
    block: usize,
    hop: usize,
    window: Vec<f32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
    /// `2/3` for the window overlap, `1/block` because `realfft`'s inverse is
    /// unnormalised.
    scale: f32,
}

impl Plan {
    fn new(planner: &mut RealFftPlanner<f32>, quality: Quality, sample_rate: f32) -> Self {
        let block = quality.block(sample_rate);

        // Periodic, not symmetric: `cos(2πn/N)` rather than `cos(2πn/(N−1))`.
        // The symmetric one does not sum to a constant under overlap, which is
        // the one property this window is chosen for.
        let window = (0..block)
            .map(|n| {
                let phase = std::f32::consts::TAU * n as f32 / block as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();

        Self {
            block,
            hop: block / OVERLAP,
            window,
            forward: planner.plan_fft_forward(block),
            inverse: planner.plan_fft_inverse(block),
            scale: 2.0 / (3.0 * block as f32),
        }
    }
}

/// One transform's worth of bins, for every channel, handed to the caller
/// between the forward and the inverse.
///
/// **The gain a caller applies has to be real.** `ComplexToReal` requires the
/// imaginary parts of the first and last bins to be zero, and this type does
/// not stop a caller writing something else there — [`Stft`] zeroes them again
/// before the inverse rather than failing on the audio thread.
pub struct Frame<'a> {
    spectra: &'a mut [Vec<Complex<f32>>; MAX_CHANNELS],
    channels: usize,
    bins: usize,
}

impl Frame<'_> {
    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn bins(&self) -> usize {
        self.bins
    }

    /// One channel's bins. Out of range gives an empty slice rather than a
    /// panic: this runs on the audio thread (`.agents/rules/rust.md`).
    pub fn channel(&mut self, index: usize) -> &mut [Complex<f32>] {
        if index >= self.channels {
            return &mut [];
        }
        &mut self.spectra[index][..self.bins]
    }

    /// One channel's bins, read-only, while another is being written.
    pub fn read(&self, index: usize) -> &[Complex<f32>] {
        if index >= self.channels {
            return &[];
        }
        &self.spectra[index][..self.bins]
    }
}

/// The overlap-add buffering, for up to [`MAX_CHANNELS`] channels.
pub struct Stft {
    plans: [Plan; 3],
    /// The last `block` input samples, oldest at `position`.
    input: [Vec<f32>; MAX_CHANNELS],
    /// Overlapping frames summed in place. A slot is emitted and zeroed in the
    /// same step, which is also what keeps it from drifting.
    accumulator: [Vec<f32>; MAX_CHANNELS],
    spectra: [Vec<Complex<f32>>; MAX_CHANNELS],
    /// One frame, windowed, between the ring and the transform.
    frame: Vec<f32>,
    fft_scratch: Vec<Complex<f32>>,
    quality: Quality,
    position: usize,
    since_frame: usize,
}

impl Stft {
    /// **The only place that allocates.** Every step is planned, so changing
    /// `QUALITY` later is an index (`REQ-PUM-008`).
    pub fn new(sample_rate: f32, quality: Quality) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let plans = [
            Plan::new(&mut planner, Quality::Low, sample_rate),
            Plan::new(&mut planner, Quality::Normal, sample_rate),
            Plan::new(&mut planner, Quality::High, sample_rate),
        ];

        let scratch = plans
            .iter()
            .map(|plan| {
                plan.forward
                    .get_scratch_len()
                    .max(plan.inverse.get_scratch_len())
            })
            .max()
            .unwrap_or(0);
        let widest = plans.iter().map(|plan| plan.block).max().unwrap_or(0);

        Self {
            plans,
            input: std::array::from_fn(|_| vec![0.0; widest]),
            accumulator: std::array::from_fn(|_| vec![0.0; widest]),
            spectra: std::array::from_fn(|_| vec![Complex::new(0.0, 0.0); widest / 2 + 1]),
            frame: vec![0.0; widest],
            fft_scratch: vec![Complex::new(0.0, 0.0); scratch],
            quality,
            position: 0,
            since_frame: 0,
        }
    }

    pub fn quality(&self) -> Quality {
        self.quality
    }

    pub fn block(&self) -> usize {
        self.plans[self.quality.index()].block
    }

    /// What the plugin reports to the host: one whole block.
    pub fn latency(&self) -> usize {
        self.block()
    }

    /// **Clears rather than rebuilds.** A step change is a discontinuity in the
    /// output either way — the ring holds samples measured against a different
    /// window length — and the host has just been told the delay moved.
    pub fn set_quality(&mut self, quality: Quality) {
        if quality == self.quality {
            return;
        }
        self.quality = quality;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.position = 0;
        self.since_frame = 0;
        for channel in 0..MAX_CHANNELS {
            self.input[channel].fill(0.0);
            self.accumulator[channel].fill(0.0);
        }
    }

    /// Replaces each channel's samples with the reconstruction, calling
    /// `on_frame` once per hop with the bins of every channel at once.
    ///
    /// **Every channel is transformed before `on_frame` runs**, because the
    /// detector reads both and applies one gain curve to both
    /// (`REQ-PUM-011`).
    pub fn process<F>(&mut self, channels: &mut [&mut [f32]], mut on_frame: F)
    where
        F: FnMut(&mut Frame<'_>),
    {
        let used = channels.len().min(MAX_CHANNELS);
        let Some(samples) = channels[..used].iter().map(|c| c.len()).min() else {
            return;
        };

        let index = self.quality.index();
        let (block, hop) = (self.plans[index].block, self.plans[index].hop);

        for sample in 0..samples {
            let position = self.position;

            for (channel, buffer) in channels[..used].iter_mut().enumerate() {
                let input = buffer[sample];
                buffer[sample] = self.accumulator[channel][position];
                self.accumulator[channel][position] = 0.0;
                self.input[channel][position] = input;
            }

            self.position = position + 1;
            if self.position == block {
                self.position = 0;
            }

            self.since_frame += 1;
            if self.since_frame == hop {
                self.since_frame = 0;
                self.run_frame(used, &mut on_frame);
            }
        }
    }

    fn run_frame<F>(&mut self, channels: usize, on_frame: &mut F)
    where
        F: FnMut(&mut Frame<'_>),
    {
        let index = self.quality.index();
        let block = self.plans[index].block;
        let bins = block / 2 + 1;
        // The oldest sample sits where the next one will be written.
        let start = self.position;

        for channel in 0..channels {
            for offset in 0..block {
                let mut read = start + offset;
                if read >= block {
                    read -= block;
                }
                self.frame[offset] = self.input[channel][read] * self.plans[index].window[offset];
            }

            if self.plans[index]
                .forward
                .process_with_scratch(
                    &mut self.frame[..block],
                    &mut self.spectra[channel][..bins],
                    &mut self.fft_scratch,
                )
                .is_err()
            {
                debug_assert!(false, "the forward plan and the frame disagree on length");
                return;
            }
        }

        on_frame(&mut Frame {
            spectra: &mut self.spectra,
            channels,
            bins,
        });

        for channel in 0..channels {
            // `ComplexToReal` refuses a spectrum whose DC and Nyquist bins
            // carry an imaginary part. A real gain preserves that; a caller
            // that writes something else would otherwise take the frame out on
            // the audio thread, so it is fixed here rather than reported.
            self.spectra[channel][0].im = 0.0;
            self.spectra[channel][bins - 1].im = 0.0;

            if self.plans[index]
                .inverse
                .process_with_scratch(
                    &mut self.spectra[channel][..bins],
                    &mut self.frame[..block],
                    &mut self.fft_scratch,
                )
                .is_err()
            {
                debug_assert!(false, "the inverse plan and the frame disagree on length");
                return;
            }

            let scale = self.plans[index].scale;
            for offset in 0..block {
                let mut write = start + offset;
                if write >= block {
                    write -= block;
                }
                self.accumulator[channel][write] +=
                    self.frame[offset] * self.plans[index].window[offset] * scale;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic broadband signal. Not `rand`: a test that draws a new
    /// signal every run cannot be told apart from a flaky one.
    fn noise(length: usize) -> Vec<f32> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 40) as f32 / 8_388_608.0 - 1.0
            })
            .collect()
    }

    fn run(quality: Quality, rate: f32, chunk: usize, input: &[f32]) -> Vec<f32> {
        let mut stft = Stft::new(rate, quality);
        let mut output = input.to_vec();
        for piece in output.chunks_mut(chunk) {
            let mut channels = [piece];
            stft.process(&mut channels, |_| {});
        }
        output
    }

    fn error_db(output: &[f32], input: &[f32], latency: usize, skip: usize) -> f32 {
        let mut worst: f32 = 0.0;
        for index in skip..input.len() {
            worst = worst.max((output[index] - input[index - latency]).abs());
        }
        20.0 * worst.max(f32::MIN_POSITIVE).log10()
    }

    /// `REQ-PUM-007`: with nothing touching the bins, what comes out is what
    /// went in, one block late.
    #[test]
    fn an_untouched_frame_reconstructs() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for quality in Quality::ALL {
                let block = quality.block(rate);
                let input = noise(block * 6);
                let output = run(quality, rate, 512, &input);

                // The first block has fewer than `OVERLAP` frames summed into
                // it, which is the ramp-in every overlap-add has rather than an
                // error.
                let error = error_db(&output, &input, quality.latency(rate), block * 2);
                assert!(
                    error <= -120.0,
                    "{quality:?} at {rate} Hz: worst sample is {error:.1} dB"
                );
            }
        }
    }

    /// `REQ-PUM-017`: the host's block size must not reach the output.
    #[test]
    fn the_host_block_size_does_not_change_the_output() {
        let quality = Quality::Normal;
        let rate = 48_000.0;
        let input = noise(quality.block(rate) * 5);
        let reference = run(quality, rate, 512, &input);

        for chunk in [1, 64, 4096] {
            let output = run(quality, rate, chunk, &input);
            assert_eq!(output, reference, "block size {chunk} disagrees with 512");
        }
    }

    /// The delay the host is told about is the delay the audio takes.
    #[test]
    fn the_reconstruction_arrives_at_the_reported_latency() {
        let (quality, rate) = (Quality::Normal, 48_000.0);
        let block = quality.block(rate);
        let mut input = vec![0.0; block * 6];
        input[block * 3] = 1.0;

        let output = run(quality, rate, 512, &input);
        let peak = output
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(index, _)| index);

        assert_eq!(peak, Some(block * 3 + quality.latency(rate)));
    }

    /// A caller that writes the bins is the whole point; check the path works
    /// and that the promise "`DEPTH` = 0 is exactly nothing" is reachable.
    #[test]
    fn a_flat_gain_scales_the_reconstruction() {
        let (quality, rate) = (Quality::Normal, 48_000.0);
        let block = quality.block(rate);
        let input = noise(block * 5);

        let mut stft = Stft::new(rate, quality);
        let mut output = input.clone();
        for piece in output.chunks_mut(512) {
            let mut channels = [piece];
            stft.process(&mut channels, |frame| {
                for bin in frame.channel(0) {
                    *bin *= 0.5;
                }
            });
        }

        let halved: Vec<f32> = input.iter().map(|sample| sample * 0.5).collect();
        let error = error_db(&output, &halved, quality.latency(rate), block * 2);
        assert!(error <= -120.0, "worst sample is {error:.1} dB");
    }

    /// Both channels reach the callback before either is written back, which is
    /// what lets one gain curve be derived from both (`REQ-PUM-011`).
    #[test]
    fn both_channels_are_present_in_one_frame() {
        let (quality, rate) = (Quality::Normal, 48_000.0);
        let block = quality.block(rate);
        let mut left = noise(block * 3);
        let mut right = noise(block * 3);
        right.reverse();

        let mut stft = Stft::new(rate, quality);
        let mut seen = 0;
        for (a, b) in left.chunks_mut(512).zip(right.chunks_mut(512)) {
            let mut channels = [a, b];
            stft.process(&mut channels, |frame| {
                seen += 1;
                assert_eq!(frame.channels(), 2);
                assert_eq!(frame.bins(), block / 2 + 1);
                assert_ne!(frame.read(0), frame.read(1));
            });
        }
        assert!(seen > 0, "no frame ran");
    }

    /// Switching steps must not leave the ring half-full of samples measured
    /// against the other window, and must not allocate.
    #[test]
    fn a_quality_change_clears_the_state() {
        let rate = 48_000.0;
        let mut stft = Stft::new(rate, Quality::Normal);
        let mut samples = noise(4096);
        {
            let mut channels = [&mut samples[..]];
            stft.process(&mut channels, |_| {});
        }

        stft.set_quality(Quality::High);
        assert_eq!(stft.block(), Quality::High.block(rate));
        assert_eq!(stft.latency(), Quality::High.block(rate));
        assert!(stft.accumulator[0].iter().all(|sample| *sample == 0.0));
    }

    /// The mono layout hands over one channel (`REQ-PUM-011`).
    #[test]
    fn one_channel_works() {
        let (quality, rate) = (Quality::Low, 48_000.0);
        let block = quality.block(rate);
        let input = noise(block * 6);
        let output = run(quality, rate, 64, &input);
        let error = error_db(&output, &input, quality.latency(rate), block * 2);
        assert!(error <= -120.0, "worst sample is {error:.1} dB");
    }

    #[test]
    fn no_channels_is_not_a_panic() {
        let mut stft = Stft::new(48_000.0, Quality::Normal);
        stft.process(&mut [], |_| unreachable!());
    }
}
