//! Invert: the spectrum turned upside down between two frequencies
//! (`spectral-kit-plan.md`, decision 5).
//!
//! A component at `f` inside `low..=high` comes out at `low + high − f`:
//! the band's bottom becomes its top. Outside the band nothing moves. The new
//! frequencies are exact, not the nearest bin's: a phase vocoder reads each
//! bin's true frequency, mirrors it, and keeps a running phase per output bin.

use crate::{Analysis, Shell, expected_advance, mix};
use nxe_spectral::Complex;
use nxe_spectral::stft::MAX_CHANNELS;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// The band's edges in Hz. Either order.
    pub low: f32,
    pub high: f32,
    /// `0..=1`.
    pub mix: f32,
}

/// Where `hz` lands for a band `low..=high`. The figure draws this.
pub fn mirror(hz: f32, low: f32, high: f32) -> f32 {
    let (low, high) = (low.min(high), low.max(high));
    if (low..=high).contains(&hz) {
        low + high - hz
    } else {
        hz
    }
}

struct Channel {
    analysis: Analysis,
    synthesis: Vec<f32>,
    magnitude: Vec<f32>,
    frequency: Vec<f32>,
}

pub struct Invert {
    shell: Shell,
    channels: [Channel; MAX_CHANNELS],
}

impl Invert {
    pub fn new(sample_rate: f32) -> Self {
        let shell = Shell::new(sample_rate);
        let bins = shell.bins;
        Self {
            channels: std::array::from_fn(|_| Channel {
                analysis: Analysis::new(bins),
                synthesis: vec![0.0; bins],
                magnitude: vec![0.0; bins],
                frequency: vec![0.0; bins],
            }),
            shell,
        }
    }

    /// In samples: one block.
    pub fn latency(&self) -> usize {
        self.shell.latency()
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let bin_hz = self.shell.bin_hz;
        let channels = &mut self.channels;
        let (low, high) = (
            settings.low.min(settings.high) / bin_hz,
            settings.low.max(settings.high) / bin_hz,
        );
        let amount = settings.mix;
        self.shell.process(input, |frame| {
            for (index, channel) in channels.iter_mut().enumerate().take(frame.channels()) {
                let bins = frame.channel(index);
                // The band as bins that exist: at a low sample rate `high`
                // can sit past Nyquist.
                let (first, top) = (low.ceil() as usize, (high as usize).min(bins.len() - 1));
                channel.magnitude.fill(0.0);
                for (bin, value) in bins.iter().enumerate() {
                    let (magnitude, frequency) = channel.analysis.read(bin, *value);
                    if !(first..=top).contains(&bin) {
                        continue;
                    }
                    let target = low + high - frequency;
                    let to = (target.round().max(0.0) as usize).clamp(first, top);
                    // The loudest source decides the frequency a bin carries.
                    if magnitude > channel.magnitude[to] {
                        channel.frequency[to] = target;
                    }
                    channel.magnitude[to] += magnitude;
                }
                let band = first..=top;
                for (bin, value) in bins
                    .iter_mut()
                    .enumerate()
                    .filter(|(bin, _)| band.contains(bin))
                {
                    channel.synthesis[bin] = crate::wrap(
                        channel.synthesis[bin] + expected_advance(channel.frequency[bin]),
                    );
                    let wet = Complex::from_polar(channel.magnitude[bin], channel.synthesis[bin]);
                    *value = mix(*value, wet, amount);
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::*;

    fn run(hz: f32, settings: &Settings) -> Vec<f32> {
        let mut invert = Invert::new(RATE);
        (0..RATE as usize)
            .map(|index| {
                let x = sine(index, hz);
                invert.process([x, x], settings)[0]
            })
            .collect()
    }

    const BAND: Settings = Settings {
        low: 200.0,
        high: 5_000.0,
        mix: 1.0,
    };

    #[test]
    fn a_tone_in_the_band_comes_out_mirrored() {
        let out = run(1_000.0, &BAND);
        let tail = &out[RATE as usize / 2..];
        let moved = level_at(tail, 4_200.0);
        let stayed = level_at(tail, 1_000.0);
        assert!(moved > stayed * 10.0, "moved {moved}, stayed {stayed}");
    }

    #[test]
    fn a_tone_outside_the_band_stays() {
        let out = run(8_000.0, &BAND);
        let tail = &out[RATE as usize / 2..];
        assert!(level_at(tail, 8_000.0) > 0.2, "{}", level_at(tail, 8_000.0));
    }

    #[test]
    fn a_band_past_nyquist_does_not_panic() {
        let mut invert = Invert::new(8_000.0);
        for index in 0..8_000 {
            let x = sine(index, 1_000.0);
            assert!(invert.process([x, x], &BAND)[0].is_finite());
        }
    }

    #[test]
    fn mirror_swaps_the_ends() {
        assert_eq!(mirror(200.0, 200.0, 5_000.0), 5_000.0);
        assert_eq!(mirror(5_000.0, 5_000.0, 200.0), 200.0);
        assert_eq!(mirror(10.0, 200.0, 5_000.0), 10.0);
    }
}
