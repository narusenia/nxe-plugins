//! The pitch of a monophonic input, by YIN (de Cheveigné and Kawahara, 2002).
//!
//! Written in `parasite-core` not knowing what Parasite is, and moved here
//! unchanged when Dew became its second caller (`resonance-kit-plan.md`).
//!
//! The input is low-passed and decimated to about 12 kHz first, which is
//! plenty for a fundamental under 1 kHz and cuts the cost by the square of the
//! factor. Every [`HOP`] decimated samples (about 5 ms) the last window is
//! analysed: the cumulative mean normalised difference, the first dip under
//! [`THRESHOLD`], and a parabola through it for a pitch between lags.
//!
//! Allocation-free once built.

use crate::biquad::{Biquad, Coefficients};

/// The lowest and highest fundamental it looks for.
pub const LOW_HZ: f32 = 50.0;
pub const HIGH_HZ: f32 = 1_000.0;
/// Where the analysis runs, roughly.
const ANALYSIS_HZ: f32 = 12_000.0;
/// Decimated samples between analyses.
pub const HOP: usize = 64;
/// YIN's absolute threshold: the first lag whose normalised difference dips
/// under it is taken, which keeps an octave below from winning.
const THRESHOLD: f32 = 0.15;
/// Below this mean square the window is silence, and has no pitch.
const SILENCE: f32 = 1e-8;

pub struct Pitch {
    factor: usize,
    rate: f32,
    lowpass: [Biquad; 2],
    /// Input samples until the next decimated one.
    countdown: usize,
    /// The decimated history, oldest overwritten first.
    ring: Vec<f32>,
    write: usize,
    /// Decimated samples since the last analysis.
    since: usize,
    /// The integration window, and the largest lag: both `rate / LOW_HZ`.
    window: usize,
    min_lag: usize,
    frame: Vec<f32>,
    difference: Vec<f32>,
    hz: f32,
    clarity: f32,
}

impl Pitch {
    pub fn new(sample_rate: f32) -> Self {
        let factor = (sample_rate / ANALYSIS_HZ).round().max(1.0) as usize;
        let rate = sample_rate / factor as f32;
        let window = (rate / LOW_HZ).ceil() as usize;
        let min_lag = ((rate / HIGH_HZ).floor() as usize).max(2);
        let cutoff = (rate * 0.4).min(sample_rate * 0.45);
        let lowpass = || Biquad::new(Coefficients::lowpass(cutoff, 0.707, sample_rate));
        Self {
            factor,
            rate,
            lowpass: [lowpass(), lowpass()],
            countdown: factor,
            ring: vec![0.0; window * 2 + 1],
            write: 0,
            since: 0,
            window,
            min_lag,
            frame: vec![0.0; window * 2 + 1],
            difference: vec![0.0; window + 2],
            hz: 0.0,
            clarity: 0.0,
        }
    }

    /// One input sample. True when a new estimate is ready.
    pub fn push(&mut self, input: f32) -> bool {
        let input = if input.is_finite() { input } else { 0.0 };
        let once = self.lowpass[0].process(input);
        let filtered = self.lowpass[1].process(once);
        self.countdown -= 1;
        if self.countdown > 0 {
            return false;
        }
        self.countdown = self.factor;
        self.ring[self.write] = filtered;
        self.write = (self.write + 1) % self.ring.len();
        self.since += 1;
        if self.since < HOP {
            return false;
        }
        self.since = 0;
        self.analyse();
        true
    }

    /// The last estimate in Hz. Kept when the input has no pitch: read
    /// [`clarity`](Self::clarity) to know whether to believe it.
    pub fn hz(&self) -> f32 {
        self.hz
    }

    /// How periodic the last window was, `0..=1`: one minus the normalised
    /// difference at the chosen lag. A sung vowel reads above 0.9, a breath
    /// or a snare under 0.5, silence 0.
    pub fn clarity(&self) -> f32 {
        self.clarity
    }

    fn analyse(&mut self) {
        let length = self.ring.len();
        for (index, sample) in self.frame.iter_mut().enumerate() {
            *sample = self.ring[(self.write + index) % length];
        }
        let window = self.window;
        let energy = self.frame[..window].iter().map(|x| x * x).sum::<f32>() / window as f32;
        if energy < SILENCE {
            self.clarity = 0.0;
            return;
        }

        // The cumulative mean normalised difference, d'(0) = 1.
        let mut running = 0.0;
        self.difference[0] = 1.0;
        for lag in 1..=window {
            let sum: f32 = self.frame[..window]
                .iter()
                .zip(&self.frame[lag..lag + window])
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            running += sum;
            self.difference[lag] = if running > 0.0 {
                sum * lag as f32 / running
            } else {
                1.0
            };
        }

        let range = self.min_lag..window;
        let mut chosen = None;
        let mut lag = range.start;
        while lag < range.end {
            if self.difference[lag] < THRESHOLD {
                // Down to the bottom of this dip.
                while lag + 1 < range.end && self.difference[lag + 1] < self.difference[lag] {
                    lag += 1;
                }
                chosen = Some(lag);
                break;
            }
            lag += 1;
        }
        let lag = chosen.unwrap_or_else(|| {
            range
                .clone()
                .min_by(|a, b| self.difference[*a].total_cmp(&self.difference[*b]))
                .unwrap_or(range.start)
        });

        // A parabola through the dip and its neighbours.
        let (before, at, after) = (
            self.difference[lag - 1],
            self.difference[lag],
            self.difference[lag + 1],
        );
        let bend = before - 2.0 * at + after;
        let offset = if bend > 0.0 {
            (0.5 * (before - after) / bend).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.hz = self.rate / (lag as f32 + offset);
        self.clarity = (1.0 - at).clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const RATE: f32 = 48_000.0;

    fn track(mut signal: impl FnMut(usize) -> f32) -> (f32, f32) {
        let mut pitch = Pitch::new(RATE);
        for index in 0..RATE as usize / 2 {
            pitch.push(signal(index));
        }
        (pitch.hz(), pitch.clarity())
    }

    #[test]
    fn a_sine_reads_its_frequency() {
        for hz in [82.0, 220.0, 440.0, 880.0] {
            let (found, clarity) = track(|i| (TAU * hz * i as f32 / RATE).sin() * 0.5);
            assert!((found / hz - 1.0).abs() < 0.01, "{hz} read as {found}");
            assert!(clarity > 0.9, "{hz}: clarity {clarity}");
        }
    }

    #[test]
    fn a_sawtooth_reads_its_fundamental_not_a_harmonic() {
        let hz = 110.0;
        let (found, _) = track(|i| ((hz * i as f32 / RATE).fract() * 2.0 - 1.0) * 0.5);
        assert!((found / hz - 1.0).abs() < 0.01, "read as {found}");
    }

    #[test]
    fn noise_is_not_clear() {
        let mut state = 0x1234_5678_u32;
        let (_, clarity) = track(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as f32 / u32::MAX as f32 - 0.5) * 0.5
        });
        assert!(clarity < 0.6, "clarity {clarity}");
    }

    #[test]
    fn silence_has_no_clarity() {
        assert_eq!(track(|_| 0.0).1, 0.0);
    }

    #[test]
    fn other_rates_read_the_same() {
        for rate in [44_100.0, 96_000.0] {
            let mut pitch = Pitch::new(rate);
            for i in 0..rate as usize / 2 {
                pitch.push((TAU * 220.0 * i as f32 / rate).sin() * 0.5);
            }
            assert!(
                (pitch.hz() / 220.0 - 1.0).abs() < 0.01,
                "{rate}: {}",
                pitch.hz()
            );
        }
    }
}
