//! Everything Pumice does to a signal, in one object the wrapper drives.
//!
//! **The ear-tuned constants live here** ([`Settings`]), in one block rather
//! than scattered through the modules that use them. `PUM-11` settles all of
//! them; the specification's "耳で詰める定数" table and this struct must agree
//! (`../docs/specifications/dsp.md`).
//!
//! **`STATIC` only, at this unit** (`PUM-3`). The long-term map that decides
//! *where* resonance lives is `PUM-4`; what runs here is the short-term
//! follower alone, which is what soothe does and what `MODE` will offer as the
//! way out when the adaptive path is wrong for a piece of material.

use crate::gain::{Computer, Follower, range_into, smooth_into};
use crate::quality::Quality;
use crate::reference::{excess_db_into, power_into};
use crate::smoothing::Prefix;
use crate::stft::{MAX_BINS, MAX_CHANNELS, Stft};

/// The product's tuning: everything an ear decides rather than a requirement.
///
/// A `const` rather than a set of arguments, so what Pumice is reads as one
/// block (`nxe_audio::guard::Settings` is the same shape for the same reason).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// A **band-to-reference ratio**, not a level. Zero says "this bin carries
    /// as much power as its neighbourhood", which spectrally ordinary material
    /// sits at — so zero takes nothing out of it (`SPK-18`).
    pub threshold_db: f32,
    /// How many dB of reduction per dB of excess. One would flatten the bin
    /// onto its reference exactly.
    pub slope: f32,
    /// The widest any bin may be pulled. Past this a vocal is not protected,
    /// it is missing (`REQ-PUM-023`).
    pub ceiling_db: f32,
    /// What `SHARPNESS` interpolates between, in octaves. Wide catches broad
    /// humps, narrow only spikes.
    pub reference_wide_octaves: f32,
    pub reference_narrow_octaves: f32,
    /// What `SPEED` interpolates between. The fast end is bounded by the hop
    /// whatever is asked for (`REQ-PUM-020`).
    pub attack_slow_seconds: f32,
    pub attack_fast_seconds: f32,
    pub release_slow_seconds: f32,
    pub release_fast_seconds: f32,
    /// **Not reachable from any control** (`REQ-PUM-005`). The floor that
    /// keeps the reconstruction from warbling.
    pub gain_smoothing_octaves: f32,
    pub low_hz: f32,
    pub high_hz: f32,
    /// How wide each edge of the operating range fades, in octaves, centred on
    /// the edge.
    pub edge_octaves: f32,
}

impl Settings {
    /// `dsp.md`'s "耳で詰める定数". **None of these has been measured or
    /// listened to** — they are the design-time values, and `PUM-11` replaces
    /// them.
    pub const DEFAULT: Settings = Settings {
        threshold_db: 0.0,
        slope: 0.7,
        ceiling_db: 18.0,
        reference_wide_octaves: 1.0,
        reference_narrow_octaves: 0.17,
        attack_slow_seconds: 0.040,
        attack_fast_seconds: 0.005,
        release_slow_seconds: 0.400,
        release_fast_seconds: 0.040,
        gain_smoothing_octaves: 1.0 / 12.0,
        low_hz: 100.0,
        high_hz: 18_000.0,
        edge_octaves: 0.5,
    };
}

/// What the host's controls are worth this block.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// `0..=1`. Zero is exactly nothing (`REQ-PUM-002`).
    pub depth: f32,
    /// `0..=1`. Zero is the wide reference, one the narrow.
    pub sharpness: f32,
    /// `0..=1`. Zero is the slow end.
    pub speed: f32,
    pub quality: Quality,
}

/// The detection and the gain, with no transform of its own.
///
/// Separate from [`Engine`] so that the per-frame work can borrow it while
/// [`Stft`] is borrowed too — one `&mut self` closure over the whole engine
/// would not compile.
struct Detector {
    prefix: Prefix,
    power: Vec<f32>,
    reference: Vec<f32>,
    excess_db: Vec<f32>,
    drive_db: Vec<f32>,
    reduction_db: Vec<f32>,
    smoothed_db: Vec<f32>,
    gain: Vec<f32>,
    /// The operating range as a per-bin weight. Rebuilt when the bin spacing
    /// changes, never per frame.
    weight: Vec<f32>,
    follower: Follower,
    computer: Computer,
    settings: Settings,
    depth: f32,
    reference_octaves: f32,
    bins: usize,
    /// Scratch for handing the channels' spectra to [`power_into`] without
    /// borrowing them mutably.
    channels: usize,
}

impl Detector {
    fn new(settings: Settings) -> Self {
        Self {
            prefix: Prefix::new(MAX_BINS),
            power: vec![0.0; MAX_BINS],
            reference: vec![0.0; MAX_BINS],
            excess_db: vec![0.0; MAX_BINS],
            drive_db: vec![0.0; MAX_BINS],
            reduction_db: vec![0.0; MAX_BINS],
            smoothed_db: vec![0.0; MAX_BINS],
            gain: vec![1.0; MAX_BINS],
            weight: vec![0.0; MAX_BINS],
            follower: Follower::new(MAX_BINS),
            computer: Computer {
                threshold_db: settings.threshold_db,
                slope: settings.slope,
                ceiling_db: settings.ceiling_db,
            },
            settings,
            depth: 0.0,
            reference_octaves: settings.reference_wide_octaves,
            bins: 0,
            channels: 0,
        }
    }

    fn run(&mut self, frame: &mut crate::stft::Frame<'_>) {
        let bins = frame.bins();
        let channels = frame.channels();
        self.bins = bins;
        self.channels = channels;

        {
            // `Frame::read` hands out shared slices, which is what lets both
            // channels be measured before either is written.
            let left = frame.read(0);
            let right = frame.read(1);
            let spectra: [&[realfft::num_complex::Complex<f32>]; MAX_CHANNELS] = [left, right];
            power_into(&spectra[..channels], &mut self.power[..bins]);
        }

        excess_db_into(
            &self.power[..bins],
            &mut self.prefix,
            &mut self.reference[..bins],
            self.reference_octaves,
            &mut self.excess_db[..bins],
        );

        // **`STATIC`**: the short-term follower is the whole drive. `PUM-4`
        // puts `min(WHEN, WHERE)` here.
        self.follower
            .follow(&self.excess_db[..bins], &mut self.drive_db[..bins]);

        self.computer.reduction_db_into(
            &self.drive_db[..bins],
            &self.weight[..bins],
            self.depth,
            &mut self.reduction_db[..bins],
        );

        smooth_into(
            &self.reduction_db[..bins],
            &mut self.prefix,
            &mut self.smoothed_db[..bins],
            self.settings.gain_smoothing_octaves,
            &mut self.gain[..bins],
        );

        for channel in 0..channels {
            for (bin, value) in frame.channel(channel).iter_mut().enumerate() {
                *value *= self.gain[bin];
            }
        }
    }
}

/// The whole of Pumice, driven a buffer at a time.
pub struct Engine {
    stft: Stft,
    detector: Detector,
    sample_rate: f32,
    /// What [`Detector::weight`] was last built for, so a rebuild only happens
    /// when the spacing actually moved.
    weight_bins: usize,
}

impl Engine {
    /// **The only place that allocates** (`REQ-PUM-016`).
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        let mut engine = Self {
            stft: Stft::new(sample_rate, Quality::default()),
            detector: Detector::new(settings),
            sample_rate,
            weight_bins: 0,
        };
        engine.rebuild_weight();
        engine
    }

    pub fn latency(&self) -> usize {
        self.stft.latency()
    }

    pub fn reset(&mut self) {
        self.stft.reset();
        self.detector.follower.reset();
    }

    /// Once per block. Everything derived from a control is resolved here so
    /// that the per-frame path is arithmetic only.
    pub fn set(&mut self, controls: Controls) {
        let settings = self.detector.settings;

        self.stft.set_quality(controls.quality);
        if self.stft.block() / 2 + 1 != self.weight_bins {
            self.rebuild_weight();
        }

        self.detector.depth = controls.depth.clamp(0.0, 1.0);

        // Interpolated **in the log domain**: the octave width is a ratio, so
        // half a turn should land on the geometric middle.
        let sharpness = controls.sharpness.clamp(0.0, 1.0);
        self.detector.reference_octaves = settings.reference_wide_octaves
            * (settings.reference_narrow_octaves / settings.reference_wide_octaves).powf(sharpness);

        let speed = controls.speed.clamp(0.0, 1.0);
        let attack = settings.attack_slow_seconds
            * (settings.attack_fast_seconds / settings.attack_slow_seconds).powf(speed);
        let release = settings.release_slow_seconds
            * (settings.release_fast_seconds / settings.release_slow_seconds).powf(speed);

        // **The hop is the clock** — a bin is looked at once per frame.
        let frame_rate = self.sample_rate / (self.stft.block() / crate::quality::OVERLAP) as f32;
        self.detector.follower.set(attack, release, frame_rate);
    }

    pub fn process(&mut self, channels: &mut [&mut [f32]]) {
        let Self { stft, detector, .. } = self;
        stft.process(channels, |frame| detector.run(frame));
    }

    /// The largest reduction any bin is taking, in dB, for the readout
    /// (`REQ-PUM-018`).
    pub fn reduction_db(&self) -> f32 {
        self.detector.smoothed_db[..self.detector.bins]
            .iter()
            .fold(0.0_f32, |worst, value| worst.min(*value))
    }

    fn rebuild_weight(&mut self) {
        let block = self.stft.block();
        let bins = block / 2 + 1;
        let settings = self.detector.settings;
        range_into(
            bins,
            self.sample_rate / block as f32,
            settings.low_hz,
            settings.high_hz,
            settings.edge_octaves,
            &mut self.detector.weight[..bins],
        );
        self.weight_bins = bins;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn tone(length: usize, hz: f32, rate: f32, amplitude: f32) -> Vec<f32> {
        (0..length)
            .map(|n| amplitude * (std::f32::consts::TAU * hz * n as f32 / rate).sin())
            .collect()
    }

    fn run(engine: &mut Engine, input: &[f32]) -> Vec<f32> {
        let mut output = input.to_vec();
        for piece in output.chunks_mut(512) {
            let mut channels = [piece];
            engine.process(&mut channels);
        }
        output
    }

    fn rms(samples: &[f32]) -> f32 {
        let total: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
        (total / samples.len() as f64).sqrt() as f32
    }

    fn controls(depth: f32) -> Controls {
        Controls {
            depth,
            sharpness: 0.5,
            speed: 0.5,
            quality: Quality::Normal,
        }
    }

    /// `REQ-PUM-002` and `REQ-PUM-001`: at zero the engine is the transform and
    /// nothing else, so the output is the input one block late, to −120 dB.
    #[test]
    fn depth_zero_reconstructs_the_input() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(0.0));

        let input = noise(2048 * 6);
        let output = run(&mut engine, &input);

        let latency = engine.latency();
        let mut worst: f32 = 0.0;
        for index in (2048 * 2)..input.len() {
            worst = worst.max((output[index] - input[index - latency]).abs());
        }
        let error = 20.0 * worst.max(f32::MIN_POSITIVE).log10();
        assert!(error <= -120.0, "worst sample is {error:.1} dB");
    }

    /// **`REQ-PUM-003`'s acceptance condition, and the `SPK-18` regression.**
    /// Pink-ish broadband material must come out untouched at the shipped
    /// threshold — Sparkleur shipped pulling 1.3 dB out of exactly this.
    #[test]
    fn ordinary_material_is_left_alone() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));

        let input = noise(2048 * 10);
        let output = run(&mut engine, &input);

        let settled = 2048 * 4;
        let before = rms(&input[settled..input.len() - engine.latency()]);
        let after = rms(&output[settled + engine.latency()..]);
        let change_db = 20.0 * (after / before).log10();

        assert!(
            change_db.abs() < 1.0,
            "flat noise lost {change_db:.2} dB — the threshold is pulling on ordinary material"
        );
    }

    /// The product working: a tone standing well above its neighbourhood comes
    /// down, and it comes down further as `DEPTH` rises.
    #[test]
    fn a_resonance_comes_down_and_depth_moves_it() {
        let rate = 48_000.0;
        let length = 2048 * 12;
        let mut input = noise(length);
        for (sample, extra) in input.iter_mut().zip(tone(length, 2_500.0, rate, 0.9)) {
            *sample = *sample * 0.05 + extra;
        }

        let mut previous = f32::INFINITY;
        for depth in [0.0_f32, 0.25, 0.5, 1.0] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(depth));
            let output = run(&mut engine, &input);
            let level = rms(&output[2048 * 6..]);

            assert!(
                level < previous,
                "depth {depth} did not reduce further ({level} vs {previous})"
            );
            previous = level;
        }
    }

    /// `REQ-PUM-003`: the reduction must not move with input gain.
    #[test]
    fn input_gain_does_not_change_the_reduction() {
        let rate = 48_000.0;
        let length = 2048 * 12;
        let mut base = noise(length);
        for (sample, extra) in base.iter_mut().zip(tone(length, 2_500.0, rate, 0.9)) {
            *sample = *sample * 0.05 + extra;
        }

        let mut reductions = Vec::new();
        for gain_db in [-12.0_f32, 0.0, 12.0] {
            let gain = 10.0_f32.powf(gain_db / 20.0);
            let scaled: Vec<f32> = base.iter().map(|sample| sample * gain).collect();

            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(1.0));
            let output = run(&mut engine, &scaled);

            let settled = 2048 * 6;
            let before = rms(&scaled[settled..scaled.len() - engine.latency()]);
            let after = rms(&output[settled + engine.latency()..]);
            reductions.push(20.0 * (after / before).log10());
        }

        let span = reductions.iter().fold(f32::MIN, |a, b| a.max(*b))
            - reductions.iter().fold(f32::MAX, |a, b| a.min(*b));
        assert!(
            span < 0.2,
            "reduction moved {span:.3} dB across ±12 dB: {reductions:?}"
        );
    }

    /// `REQ-PUM-017`: the host's block size must not reach the output.
    #[test]
    fn the_host_block_size_does_not_change_the_output() {
        let rate = 48_000.0;
        let input = noise(2048 * 5);

        let mut reference = Vec::new();
        for chunk in [512, 1, 64, 4096] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(1.0));
            let mut output = input.clone();
            for piece in output.chunks_mut(chunk) {
                let mut channels = [piece];
                engine.process(&mut channels);
            }
            if reference.is_empty() {
                reference = output;
            } else {
                assert_eq!(output, reference, "block size {chunk}");
            }
        }
    }

    /// Extreme input must not produce a non-finite sample or a panic
    /// (`REQ-PUM-016`).
    #[test]
    fn extreme_input_stays_finite() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));

        let mut input = vec![0.0; 2048 * 4];
        input[100] = 1e6;
        input[200] = -1e6;
        let output = run(&mut engine, &input);
        assert!(output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn every_quality_runs() {
        let rate = 48_000.0;
        for quality in Quality::ALL {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(Controls {
                depth: 1.0,
                sharpness: 0.5,
                speed: 0.5,
                quality,
            });
            assert_eq!(engine.latency(), quality.block(rate));
            let output = run(&mut engine, &noise(quality.block(rate) * 5));
            assert!(output.iter().all(|sample| sample.is_finite()));
        }
    }
}
