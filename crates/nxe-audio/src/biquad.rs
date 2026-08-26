//! Second-order sections, Butterworth-aligned.
//!
//! **Move candidate**: nothing here knows about Velour (`REQ-VEL-015`).
//!
//! Direct form 1 with the RBJ cookbook coefficients, the same shape
//! `nxe_dsp::Spectrum` uses. Retuning writes new coefficients and **leaves the
//! state alone**, which is what lets `FOCUS` slide the band edges without a step
//! in the signal.

use std::f32::consts::TAU;

/// Butterworth: the flattest passband a single section has.
pub const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// How close to Nyquist a corner may sit before the coefficients stop meaning
/// anything. Past this a low-pass becomes a pass-through and a high-pass
/// becomes silence, which is the limit each is heading toward anyway.
const NYQUIST_MARGIN: f32 = 0.49;

#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

/// The five coefficients, without the state — what a retune replaces.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Coefficients {
    /// Passes everything through unchanged.
    pub const PASS: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Passes nothing.
    pub const SILENT: Self = Self {
        b0: 0.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    pub fn lowpass(hz: f32, q: f32, sample_rate: f32) -> Self {
        // A corner above Nyquist is a low-pass that low-passes nothing.
        if !(hz.is_finite() && hz > 0.0) || hz >= sample_rate * NYQUIST_MARGIN {
            return Self::PASS;
        }

        let (sin, cos, alpha, a0) = shared(hz, q, sample_rate);
        let b = (1.0 - cos) * 0.5;
        let _ = sin;
        Self {
            b0: b / a0,
            b1: (1.0 - cos) / a0,
            b2: b / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    pub fn highpass(hz: f32, q: f32, sample_rate: f32) -> Self {
        // A corner above Nyquist is a high-pass with nothing left above it.
        if hz >= sample_rate * NYQUIST_MARGIN {
            return Self::SILENT;
        }
        if !(hz.is_finite() && hz > 0.0) {
            return Self::PASS;
        }

        let (sin, cos, alpha, a0) = shared(hz, q, sample_rate);
        let b = (1.0 + cos) * 0.5;
        let _ = sin;
        Self {
            b0: b / a0,
            b1: -(1.0 + cos) / a0,
            b2: b / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }
}

fn shared(hz: f32, q: f32, sample_rate: f32) -> (f32, f32, f32, f32) {
    let w0 = TAU * hz / sample_rate;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * q.max(1e-3));
    (sin, cos, alpha, 1.0 + alpha)
}

impl Biquad {
    pub fn new(coefficients: Coefficients) -> Self {
        let mut biquad = Self::default();
        biquad.set(coefficients);
        biquad
    }

    /// Replaces the coefficients. **The state stays**, so a corner can be moved
    /// while signal is running through it.
    pub fn set(&mut self, coefficients: Coefficients) {
        self.b0 = coefficients.b0;
        self.b1 = coefficients.b1;
        self.b2 = coefficients.b2;
        self.a1 = coefficients.a1;
        self.a2 = coefficients.a2;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// A band-pass built from two second-order sections.
///
/// Shared because both the generators and the guards want the same thing: keep a
/// range, and be able to move its edges without a step
/// (`velour_core::bands`, `crate::guard`).
#[derive(Clone, Copy)]
pub struct BandPass {
    high: Biquad,
    low: Biquad,
}

impl BandPass {
    pub fn new(low_hz: f32, high_hz: f32, sample_rate: f32) -> Self {
        Self {
            high: Biquad::new(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate)),
            low: Biquad::new(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate)),
        }
    }

    /// Replaces both sets of coefficients and **keeps the state**, so an edge
    /// can be moved while signal is running through it.
    pub fn retune(&mut self, low_hz: f32, high_hz: f32, sample_rate: f32) {
        self.high
            .set(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate));
        self.low
            .set(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate));
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.low.process(self.high.process(input))
    }

    pub fn reset(&mut self) {
        self.high.reset();
        self.low.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{amplitude, db_ratio, sine};

    const RATE: f32 = 48_000.0;
    /// A tenth of a second, so a bin is 10 Hz.
    const LENGTH: usize = 4_800;

    fn run(coefficients: Coefficients, hz: usize) -> f32 {
        let mut biquad = Biquad::new(coefficients);
        // Twice the length, so the settled half holds `hz / 10` whole cycles and
        // the bin it is measured in lands exactly on the tone.
        let input = sine(1.0, hz / 5, LENGTH * 2);
        let output: Vec<f32> = input.iter().map(|s| biquad.process(*s)).collect();
        db_ratio(amplitude(&output[LENGTH..], hz / 10), 1.0)
    }

    #[test]
    fn a_lowpass_is_flat_below_and_falls_above() {
        let coefficients = Coefficients::lowpass(1_000.0, BUTTERWORTH_Q, RATE);
        assert!(run(coefficients, 100).abs() < 0.2);
        // Butterworth is 3 dB down at the corner and 12 dB per octave after.
        assert!((run(coefficients, 1_000) + 3.0).abs() < 0.5);
        assert!(run(coefficients, 4_000) < -20.0);
    }

    #[test]
    fn a_highpass_is_flat_above_and_falls_below() {
        let coefficients = Coefficients::highpass(1_000.0, BUTTERWORTH_Q, RATE);
        assert!(run(coefficients, 10_000).abs() < 0.2);
        assert!((run(coefficients, 1_000) + 3.0).abs() < 0.5);
        assert!(run(coefficients, 250) < -20.0);
    }

    /// `FOCUS` can push a corner past Nyquist, and coefficients computed up there
    /// are not "steeper" — they are meaningless, and an unstable section is a
    /// burst of noise rather than a filter.
    #[test]
    fn a_corner_above_nyquist_becomes_the_limit_it_is_heading_for() {
        assert_eq!(
            Coefficients::lowpass(40_000.0, BUTTERWORTH_Q, RATE),
            Coefficients::PASS
        );
        assert_eq!(
            Coefficients::highpass(40_000.0, BUTTERWORTH_Q, RATE),
            Coefficients::SILENT
        );
    }

    #[test]
    fn hostile_corners_do_not_produce_nonsense() {
        for hz in [f32::NAN, f32::INFINITY, -1.0, 0.0, 1e9] {
            for build in [
                Coefficients::lowpass as fn(f32, f32, f32) -> Coefficients,
                Coefficients::highpass,
            ] {
                let mut biquad = Biquad::new(build(hz, BUTTERWORTH_Q, RATE));
                for _ in 0..64 {
                    assert!(biquad.process(0.5).is_finite(), "{hz} broke it");
                }
            }
        }
    }

    /// What makes `FOCUS` movable: writing new coefficients must not clear the
    /// state, or every turn of the knob would put a step in the signal.
    #[test]
    fn retuning_keeps_the_state() {
        let mut biquad = Biquad::new(Coefficients::lowpass(1_000.0, BUTTERWORTH_Q, RATE));
        for _ in 0..256 {
            biquad.process(1.0);
        }
        let before = biquad.process(1.0);
        biquad.set(Coefficients::lowpass(1_100.0, BUTTERWORTH_Q, RATE));
        let after = biquad.process(1.0);
        assert!((after - before).abs() < 0.05, "{before} jumped to {after}");
    }
}
