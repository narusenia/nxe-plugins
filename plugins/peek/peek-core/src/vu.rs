//! A VU reading: the rectified signal through the needle's ballistics.
//!
//! **The ballistics are two equal one-poles**, critically damped, timed so a
//! step reaches 99 % in 300 ms — the VU's integration time. A real meter
//! overshoots by about 1 %; this does not, which reads the same to an eye.
//!
//! Scaled so **a sine reads its RMS**, as a VU is calibrated: the average of a
//! rectified sine is `2/π` of its peak, its RMS `1/√2`.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_2_PI};

/// The integration time of a VU, step to 99 %.
const RISE_SECONDS: f32 = 0.300;
/// Where a critically damped pair of one-poles reaches 99 %, in time
/// constants: `1 − e^{−x}(1 + x) = 0.99`.
const RISE_CONSTANTS: f32 = 6.638;
const SINE_RMS_PER_MEAN: f32 = FRAC_1_SQRT_2 / FRAC_2_PI;

pub struct Vu {
    first: f32,
    second: f32,
    coefficient: f32,
}

impl Vu {
    pub fn new(sample_rate: f32) -> Self {
        let tau = RISE_SECONDS / RISE_CONSTANTS;
        Self {
            first: 0.0,
            second: 0.0,
            coefficient: 1.0 - (-1.0 / (tau * sample_rate)).exp(),
        }
    }

    pub fn push(&mut self, sample: f32) {
        let rectified = if sample.is_finite() {
            sample.abs()
        } else {
            0.0
        } * SINE_RMS_PER_MEAN;
        self.first += (rectified - self.first) * self.coefficient;
        self.second += (self.first - self.second) * self.coefficient;
    }

    /// As an amplitude: a sine reads its RMS.
    pub fn value(&self) -> f32 {
        self.second
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn sine(vu: &mut Vu, seconds: f32) {
        for index in 0..(seconds * SR) as usize {
            vu.push((TAU * 1_000.0 * index as f32 / SR).sin());
        }
    }

    #[test]
    fn a_sine_reads_its_rms() {
        let mut vu = Vu::new(SR);
        sine(&mut vu, 1.0);
        assert!((vu.value() - FRAC_1_SQRT_2).abs() < 0.005, "{}", vu.value());
    }

    #[test]
    fn it_takes_three_hundred_milliseconds_to_arrive() {
        let mut vu = Vu::new(SR);
        sine(&mut vu, 0.25);
        let early = vu.value() / FRAC_1_SQRT_2;
        sine(&mut vu, 0.1);
        let arrived = vu.value() / FRAC_1_SQRT_2;
        assert!(early < 0.99, "already there at 250 ms: {early}");
        assert!(arrived > 0.98, "not there at 350 ms: {arrived}");
    }
}
