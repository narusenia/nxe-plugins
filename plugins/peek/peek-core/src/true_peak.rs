//! The peak between the samples, from a 4x upsampled copy.
//!
//! **Not the BS.1770 reference filter** (a 48-tap FIR). Two of `nxe-audio`'s
//! allpass halfbands, which are flat in the passband and already tested; the
//! difference from the reference is measured by the tests below rather than
//! claimed away, and the window does not call the reading "BS.1770".

use nxe_audio::oversample::{STAGE_ONE, STAGE_TWO, Upsampler2x};

pub struct TruePeak {
    up_one: Upsampler2x<{ STAGE_ONE.len() }>,
    up_two: Upsampler2x<{ STAGE_TWO.len() }>,
}

impl Default for TruePeak {
    fn default() -> Self {
        Self::new()
    }
}

impl TruePeak {
    pub fn new() -> Self {
        Self {
            up_one: Upsampler2x::new(STAGE_ONE),
            up_two: Upsampler2x::new(STAGE_TWO),
        }
    }

    /// Feeds one sample and returns the largest magnitude among it and the
    /// four upsampled points it produced. **Never below the sample itself**, so
    /// a true peak can never read lower than the sample peak beside it.
    pub fn push(&mut self, sample: f32) -> f32 {
        if !sample.is_finite() {
            return 0.0;
        }
        let [first, second] = self.up_one.process(sample);
        let [a, b] = self.up_two.process(first);
        let [c, d] = self.up_two.process(second);
        [a, b, c, d]
            .into_iter()
            .fold(sample.abs(), |peak, value| peak.max(value.abs()))
    }

    pub fn reset(&mut self) {
        self.up_one.reset();
        self.up_two.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_4, TAU};

    const SR: f32 = 48_000.0;

    /// The largest reading over a sine, after the filters have settled.
    fn peak_of(frequency: f32, phase: f32) -> (f32, f32) {
        let mut meter = TruePeak::new();
        let (mut sample_peak, mut true_peak) = (0.0f32, 0.0f32);
        for index in 0..4_800 {
            let sample = (TAU * frequency * index as f32 / SR + phase).sin();
            let reading = meter.push(sample);
            if index > 480 {
                sample_peak = sample_peak.max(sample.abs());
                true_peak = true_peak.max(reading);
            }
        }
        (sample_peak, true_peak)
    }

    /// **The case the meter exists for**: a sine at a quarter of the rate,
    /// sampled 45° off its crest, never puts a sample above −3 dB.
    #[test]
    fn it_finds_the_peak_between_the_samples() {
        let (sample_peak, true_peak) = peak_of(SR / 4.0, FRAC_PI_4);
        assert!(
            (sample_peak - FRAC_PI_4.sin()).abs() < 1e-3,
            "{sample_peak}"
        );
        let error_db = 20.0 * true_peak.log10();
        assert!(error_db.abs() < 0.5, "true peak read {error_db:+.2} dB");
    }

    #[test]
    fn a_low_sine_reads_its_amplitude() {
        let (_, true_peak) = peak_of(997.0, 0.3);
        assert!((true_peak - 1.0).abs() < 0.01, "{true_peak}");
    }

    #[test]
    fn it_never_reads_below_the_sample() {
        let mut meter = TruePeak::new();
        assert!(meter.push(0.9) >= 0.9);
        assert_eq!(meter.push(f32::NAN), 0.0);
    }
}
