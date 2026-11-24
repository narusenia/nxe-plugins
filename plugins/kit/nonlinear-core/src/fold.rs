//! Fold: a sine wavefolder (`nonlinear-kit-plan.md`, decision 7).
//!
//! ```text
//! shape(x) = (sin(π/2·(d·x + s)) − sin(π/2·s)) · 2/π
//! ```
//!
//! `d` drives the signal past the sine's peak, where it folds back; `s` slides
//! the fold off centre and brings even harmonics. The `2/π` makes the small
//! signal gain `d` at `s = 0`, so `d = 1` leaves a quiet signal at its level.

use crate::{Channel, mix};
use std::f32::consts::{FRAC_2_PI, FRAC_PI_2};

pub const FOLD_MIN: f32 = 1.0;
pub const FOLD_MAX: f32 = 8.0;

/// The curve, as the window draws it and the engine runs it.
pub fn shape(x: f32, fold: f32, symmetry: f32) -> f32 {
    ((FRAC_PI_2 * (fold * x + symmetry)).sin() - (FRAC_PI_2 * symmetry).sin()) * FRAC_2_PI
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `FOLD_MIN..=FOLD_MAX`.
    pub fold: f32,
    /// `-1..=1`.
    pub symmetry: f32,
    /// `0..=1`.
    pub mix: f32,
}

pub struct Fold {
    channels: [Channel; 2],
}

impl Fold {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            channels: [Channel::new(sample_rate), Channel::new(sample_rate)],
        }
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let Settings {
            fold,
            symmetry,
            mix: amount,
        } = *settings;
        let mut output = [0.0; 2];
        for (index, channel) in self.channels.iter_mut().enumerate() {
            output[index] = channel.process(input[index], |value| {
                mix(value, shape(value, fold, symmetry), amount)
            });
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::{rms, sine};

    #[test]
    fn silence_stays_silent_whatever_the_symmetry() {
        for symmetry in [-1.0, -0.3, 0.0, 0.7, 1.0] {
            assert!(shape(0.0, 5.0, symmetry).abs() < 1e-6);
        }
    }

    #[test]
    fn a_quiet_signal_keeps_its_level_at_one() {
        let x = 1e-3;
        assert!((shape(x, 1.0, 0.0) / x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn more_fold_folds_a_loud_peak_back() {
        // At d = 1 the peak of a full-scale input is the top of the sine; at
        // d = 2 it has gone all the way round to zero.
        assert!(shape(1.0, 1.0, 0.0) > 0.6);
        assert!(shape(1.0, 2.0, 0.0).abs() < 1e-5);
    }

    #[test]
    fn the_engine_stays_bounded_and_centred() {
        let rate = 48_000.0;
        let mut fold = Fold::new(rate);
        let settings = Settings {
            fold: FOLD_MAX,
            symmetry: 0.8,
            mix: 1.0,
        };
        let out: Vec<f32> = (0..48_000)
            .map(|index| {
                let x = sine(index, 220.0, 0.9, rate);
                fold.process([x, x], &settings)[0]
            })
            .collect();
        assert!(out.iter().all(|x| x.is_finite() && x.abs() < 3.0));
        let tail = &out[24_000..];
        let mean = tail.iter().sum::<f32>() / tail.len() as f32;
        assert!(mean.abs() < 1e-2, "DC {mean}");
        assert!(rms(tail) > 0.1);
    }

    #[test]
    fn a_closed_mix_keeps_the_level() {
        let rate = 48_000.0;
        let mut fold = Fold::new(rate);
        let settings = Settings {
            fold: FOLD_MAX,
            symmetry: 0.5,
            mix: 0.0,
        };
        let (dry, wet): (Vec<f32>, Vec<f32>) = (0..24_000)
            .map(|index| {
                let x = sine(index, 440.0, 0.5, rate);
                (x, fold.process([x, x], &settings)[0])
            })
            .unzip();
        let ratio = rms(&wet[4_800..]) / rms(&dry[4_800..]);
        assert!((ratio - 1.0).abs() < 0.01, "{ratio}");
    }
}
