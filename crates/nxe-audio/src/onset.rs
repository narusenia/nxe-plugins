//! An attack: the power over the last couple of milliseconds jumping well
//! above the power over the last sixty. **Sensitivity** sets how far it must
//! jump, from 12 dB at none to 1.5 dB at full. After one, the next waits at
//! least [`HOLDOFF_MS`], so a single consonant is one attack.
//!
//! Written for Dew (`resonance-core`) and moved here when Glint
//! (`particle-core`) asked for the same thing.
//!
//! Allocation-free.

const FAST_MS: f32 = 2.0;
const SLOW_MS: f32 = 60.0;
/// How far the short average must rise over the long one, in dB, from no
/// Sensitivity to full.
const ATTACK_DB: (f32, f32) = (12.0, 1.5);
/// Below this power (-60 dBFS) nothing is an attack.
const FLOOR: f32 = 1e-6;
/// After an attack, the next one waits at least this long.
pub const HOLDOFF_MS: f32 = 50.0;

#[derive(Clone, Debug)]
pub struct Onset {
    fast: f32,
    slow: f32,
    fast_coefficient: f32,
    slow_coefficient: f32,
    holdoff: u32,
    holdoff_samples: u32,
}

impl Onset {
    pub fn new(sample_rate: f32) -> Self {
        let coefficient = |ms: f32| 1.0 - (-1.0 / (ms * 0.001 * sample_rate)).exp();
        Self {
            fast: 0.0,
            slow: 0.0,
            fast_coefficient: coefficient(FAST_MS),
            slow_coefficient: coefficient(SLOW_MS),
            holdoff: 0,
            holdoff_samples: (HOLDOFF_MS * 0.001 * sample_rate) as u32,
        }
    }

    /// One sample. True on the sample an attack is heard; `sensitivity` is
    /// `0..=1`.
    pub fn push(&mut self, sample: f32, sensitivity: f32) -> bool {
        let power = sample * sample;
        self.fast += self.fast_coefficient * (power - self.fast);
        self.slow += self.slow_coefficient * (power - self.slow);
        if self.holdoff > 0 {
            self.holdoff -= 1;
            return false;
        }
        let attack = self.fast > FLOOR && self.fast > ratio(sensitivity) * self.slow;
        if attack {
            self.holdoff = self.holdoff_samples;
        }
        attack
    }
}

/// The power ratio that counts as an attack at `sensitivity`.
fn ratio(sensitivity: f32) -> f32 {
    let t = sensitivity.clamp(0.0, 1.0);
    let db = ATTACK_DB.0 + (ATTACK_DB.1 - ATTACK_DB.0) * t;
    10f32.powf(db / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn attacks(sensitivity: f32, signal: impl Fn(usize) -> f32) -> usize {
        let mut onset = Onset::new(RATE);
        (0..RATE as usize)
            .filter(|&i| onset.push(signal(i), sensitivity))
            .count()
    }

    #[test]
    fn silence_and_a_held_tone_are_no_attack() {
        assert_eq!(attacks(1.0, |_| 0.0), 0);
        // A tone already there: one attack as it starts, then none.
        assert_eq!(attacks(0.5, |i| (i as f32 * 0.05).sin() * 0.3), 1);
    }

    #[test]
    fn clicks_are_attacks_and_holdoff_merges_close_ones() {
        let clicks = |i: usize| if i % 12_000 < 24 { 0.5 } else { 0.0 };
        assert_eq!(attacks(0.5, clicks), 4);
        let doubled = |i: usize| {
            if i % 12_000 < 24 || (i % 12_000).abs_diff(480) < 24 {
                0.5
            } else {
                0.0
            }
        };
        assert_eq!(attacks(0.5, doubled), 4);
    }

    #[test]
    fn more_sensitivity_hears_a_softer_rise() {
        // A steady level stepping from -20 dB to -16 dB: a 4 dB rise. Steady,
        // so the short average does not ripple with a waveform.
        let step = |i: usize| if i < 24_000 { 0.1 } else { 0.16 };
        // Only the start at none; the step too at full.
        assert_eq!(attacks(0.0, step), 1);
        assert!(attacks(1.0, step) > 1);
    }
}
