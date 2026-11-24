//! One channel of Peek Level: the moving reading, and the largest values
//! since the user last cleared them.
//!
//! The moving part is `nxe_dsp::Level`, the same meter every Basic window
//! shows, so a Peek and a Basic meter side by side move alike.

use crate::TruePeak;
use nxe_dsp::Level;

pub struct ChannelLevel {
    level: Level,
    true_peak: TruePeak,
    max_peak: f32,
    max_true_peak: f32,
    max_rms: f32,
}

impl ChannelLevel {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            level: Level::new(sample_rate),
            true_peak: TruePeak::new(),
            max_peak: 0.0,
            max_true_peak: 0.0,
            max_rms: 0.0,
        }
    }

    pub fn push(&mut self, sample: f32) {
        self.level.push(sample);
        self.max_true_peak = self.max_true_peak.max(self.true_peak.push(sample));
        self.max_peak = self.max_peak.max(sample.abs());
        self.max_rms = self.max_rms.max(self.level.rms());
    }

    pub fn level(&self) -> &Level {
        &self.level
    }

    /// Since the last [`ChannelLevel::clear`], as amplitudes.
    pub fn max_peak(&self) -> f32 {
        self.max_peak
    }

    pub fn max_true_peak(&self) -> f32 {
        self.max_true_peak
    }

    pub fn max_rms(&self) -> f32 {
        self.max_rms
    }

    /// Forgets the maxima only. The moving reading keeps going, so clearing
    /// does not make a playing meter drop to the floor.
    pub fn clear(&mut self) {
        self.max_peak = 0.0;
        self.max_true_peak = 0.0;
        self.max_rms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn sine(channel: &mut ChannelLevel, amplitude: f32, seconds: f32) {
        for index in 0..(seconds * SR) as usize {
            channel.push(amplitude * (TAU * 1_000.0 * index as f32 / SR).sin());
        }
    }

    /// The two numbers with a right answer: a full-scale sine peaks at 0 dB
    /// and its RMS is −3.01 dB.
    #[test]
    fn a_full_scale_sine_reads_zero_and_minus_three() {
        let mut channel = ChannelLevel::new(SR);
        sine(&mut channel, 1.0, 1.0);
        let db = |amplitude: f32| 20.0 * amplitude.log10();
        assert!(db(channel.max_peak()).abs() < 0.01);
        assert!(
            (db(channel.max_rms()) + 3.01).abs() < 0.05,
            "{}",
            db(channel.max_rms())
        );
        assert!(channel.max_true_peak() >= channel.max_peak());
    }

    #[test]
    fn clearing_forgets_the_maxima_and_keeps_the_meter() {
        let mut channel = ChannelLevel::new(SR);
        sine(&mut channel, 1.0, 0.2);
        channel.clear();
        assert_eq!(channel.max_peak(), 0.0);
        assert!(channel.level().peak() > 0.5);
        sine(&mut channel, 0.5, 0.2);
        assert!((channel.max_peak() - 0.5).abs() < 0.01);
    }
}
