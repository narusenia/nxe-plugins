//! Shear: each frequency delayed by its own amount, so the spectrum leans in
//! time (`spectral-kit-plan.md`, decision 5).
//!
//! The delay runs along the log frequency axis from nothing at one end to
//! `length` at the other: highs late when `shear` is positive, lows late when
//! it is negative, and `|shear|` scales the whole lean. A bin delayed by whole
//! frames is exact — overlap-add rebuilds the band as it was, later — so this
//! is a ring of past spectra and nothing else.

use crate::{Shell, mix, position};
use nxe_spectral::Complex;
use nxe_spectral::stft::MAX_CHANNELS;

pub const LENGTH_MAX: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `-1..=1`.
    pub shear: f32,
    /// The longest delay, in seconds, up to [`LENGTH_MAX`].
    pub length: f32,
    /// `0..=1`.
    pub mix: f32,
}

/// The delay at `hz`, in seconds. The figure draws this.
pub fn delay(hz: f32, shear: f32, length: f32) -> f32 {
    let at = position(hz);
    let lean = if shear >= 0.0 { at } else { 1.0 - at };
    lean * shear.abs() * length.min(LENGTH_MAX)
}

pub struct Shear {
    shell: Shell,
    /// `frames` past spectra per channel, one after another.
    history: [Vec<Complex<f32>>; MAX_CHANNELS],
    frames: usize,
    write: usize,
    /// Each bin's place on the log axis, worked out once.
    positions: Vec<f32>,
}

impl Shear {
    pub fn new(sample_rate: f32) -> Self {
        let shell = Shell::new(sample_rate);
        let frames = (LENGTH_MAX / shell.hop_seconds).ceil() as usize + 1;
        let bins = shell.bins;
        Self {
            history: std::array::from_fn(|_| vec![Complex::new(0.0, 0.0); frames * bins]),
            frames,
            write: 0,
            positions: (0..bins).map(|bin| bin as f32 * shell.bin_hz).collect(),
            shell,
        }
    }

    /// In samples: one block.
    pub fn latency(&self) -> usize {
        self.shell.latency()
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let Self {
            shell,
            history,
            frames,
            write,
            positions,
        } = self;
        let (frames, hop) = (*frames, shell.hop_seconds);
        let bins = shell.bins;
        let settings = *settings;
        shell.process(input, |frame| {
            for (index, history) in history.iter_mut().enumerate().take(frame.channels()) {
                let spectrum = frame.channel(index);
                history[*write * bins..(*write + 1) * bins].copy_from_slice(spectrum);
                for (bin, value) in spectrum.iter_mut().enumerate() {
                    let seconds = delay(positions[bin], settings.shear, settings.length);
                    let back = ((seconds / hop).round() as usize).min(frames - 1);
                    let read = (*write + frames - back) % frames;
                    *value = mix(*value, history[read * bins + bin], settings.mix);
                }
            }
            *write = (*write + 1) % frames;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::*;

    /// The first sample index where a tone that starts at `start` shows up.
    fn arrival(hz: f32, settings: &Settings) -> usize {
        let mut shear = Shear::new(RATE);
        let start = RATE as usize / 10;
        (0..RATE as usize)
            .position(|index| {
                let x = if index >= start { sine(index, hz) } else { 0.0 };
                shear.process([x, x], settings)[0].abs() > 0.25
            })
            .unwrap_or(usize::MAX)
    }

    #[test]
    fn a_positive_shear_makes_the_highs_late() {
        let settings = Settings {
            shear: 1.0,
            length: 0.5,
            mix: 1.0,
        };
        let low = arrival(100.0, &settings);
        let high = arrival(10_000.0, &settings);
        // 10 kHz sits at 0.9 of the axis and 100 Hz at 0.23: about 0.33 s apart.
        assert!(
            high > low + (0.25 * RATE) as usize,
            "low {low}, high {high}"
        );
    }

    #[test]
    fn no_shear_is_no_lean() {
        let settings = Settings {
            shear: 0.0,
            length: 1.0,
            mix: 1.0,
        };
        let low = arrival(100.0, &settings);
        let high = arrival(10_000.0, &settings);
        assert!(low.abs_diff(high) < 2_048, "low {low}, high {high}");
    }

    #[test]
    fn the_delay_runs_the_right_way() {
        assert_eq!(delay(LOW_HZ_FOR_TEST, 1.0, 1.0), 0.0);
        assert!((delay(20_000.0, -1.0, 1.0)).abs() < 1e-6);
        assert!((delay(20_000.0, 1.0, 0.4) - 0.4).abs() < 1e-6);
    }

    const LOW_HZ_FOR_TEST: f32 = crate::LOW_HZ;
}
