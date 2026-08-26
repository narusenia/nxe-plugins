//! The one envelope detector `EMOTION` and `DENSITY` share.
//!
//! **Move candidate**: nothing here knows about Velour (`REQ-VEL-015`).
//!
//! ## Why one
//!
//! `EMOTION` moves the *quality* of the harmonics with how hard the singer is
//! working; `DENSITY` levels their *quantity*. Both are answers to the same
//! question — how loud is this phrase — and two detectors with two sets of time
//! constants would answer it differently, which is how the two controls would
//! start fighting (`REQ-VEL-008`).
//!
//! ## Why the input, before the compressor
//!
//! `DENSITY`'s compressor sits on the generator bus, so if the detector read
//! the compressor's output it would read a signal `DENSITY` had already
//! flattened — and `EMOTION` would stop reacting as `DENSITY` came up. Reading
//! the input keeps the two orthogonal, and it is **the only point where that
//! collision is resolved** (`REQ-VEL-008`).
//!
//! ## Peak, not RMS
//!
//! The guards threshold one band against another and want power
//! (`crate::guard`). This one is asked "how hard is this being sung", and a
//! peak follower with a slow release answers that with one number that does
//! not dip between syllables.

/// Fast enough to be inside the first note of a phrase, and **slow enough on
/// the way down not to fall back between syllables**: a single consonant must
/// not be able to move the character, or `EMOTION` reads as chatter rather than
/// as expression (`dsp.md`).
const ATTACK_SECONDS: f32 = 0.005;
const RELEASE_SECONDS: f32 = 0.150;

/// The quietest level worth reporting. `20·log10` of zero is `-inf`, and an
/// infinity multiplied by a knob is a NaN in the coefficients.
const FLOOR: f32 = 1e-6;

/// A peak follower, in decibels.
pub struct Envelope {
    level: f32,
    attack: f32,
    release: f32,
}

impl Envelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            level: 0.0,
            // A one-pole reaches `1 - 1/e` of a step in one time constant, the
            // definition the rest of the crate uses (`crate::guard`).
            attack: 1.0 - (-1.0 / (ATTACK_SECONDS * sample_rate)).exp(),
            release: 1.0 - (-1.0 / (RELEASE_SECONDS * sample_rate)).exp(),
        }
    }

    /// **Audio rate.** Feed it the mono sum, so that the character does not
    /// move sideways when one channel is louder (`REQ-VEL-011`).
    ///
    /// A sample that is not a number is read as silence rather than let
    /// through: this is state, so one NaN would latch the detector for the rest
    /// of the session.
    pub fn push(&mut self, input: f32) {
        let magnitude = if input.is_finite() { input.abs() } else { 0.0 };
        let coefficient = if magnitude > self.level {
            self.attack
        } else {
            self.release
        };
        self.level += (magnitude - self.level) * coefficient;
    }

    pub fn level(&self) -> f32 {
        self.level
    }

    pub fn decibels(&self) -> f32 {
        20.0 * self.level.max(FLOOR).log10()
    }

    pub fn reset(&mut self) {
        self.level = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::tone;

    const RATE: f32 = 48_000.0;

    /// A steady tone has to read as its own amplitude, or every threshold built
    /// on top of this is calibrated against nothing.
    #[test]
    fn a_steady_tone_reads_its_amplitude() {
        let mut envelope = Envelope::new(RATE);
        for sample in tone(0.5, 220.0, RATE, 48_000) {
            envelope.push(sample);
        }
        // Not exactly the amplitude: a peak follower with a 150 ms release
        // still sags a little between the peaks of a 220 Hz cycle.
        let level = envelope.decibels();
        assert!((level - -6.0).abs() < 1.0, "{level:.2} dB");
    }

    /// The asymmetry is the whole point: up fast, down slow.
    #[test]
    fn it_rises_faster_than_it_falls() {
        let mut envelope = Envelope::new(RATE);
        // 5 ms of full scale — one attack time constant, so about 63%.
        for _ in 0..(0.005 * RATE) as usize {
            envelope.push(1.0);
        }
        let risen = envelope.level();
        assert!((risen - 0.63).abs() < 0.05, "{risen}");

        // The same span of silence must take far less off than it put on.
        for _ in 0..(0.005 * RATE) as usize {
            envelope.push(0.0);
        }
        let fallen = envelope.level();
        assert!(fallen > risen * 0.9, "fell from {risen} to {fallen}");
    }

    /// The release has to hold across a gap between syllables — the reason
    /// `EMOTION` reads phrases rather than consonants.
    #[test]
    fn it_holds_through_a_gap() {
        let mut envelope = Envelope::new(RATE);
        for _ in 0..(0.2 * RATE) as usize {
            envelope.push(1.0);
        }
        // 50 ms of nothing, which is a long consonant.
        for _ in 0..(0.05 * RATE) as usize {
            envelope.push(0.0);
        }
        let held = envelope.decibels();
        assert!(held > -3.0, "a 50 ms gap dropped it to {held:.2} dB");
    }

    /// Silence reads as the floor and not as an infinity.
    #[test]
    fn silence_reads_as_the_floor() {
        let envelope = Envelope::new(RATE);
        assert_eq!(envelope.decibels(), 20.0 * FLOOR.log10());
    }

    /// The trap this crate has already hit twice: one hostile sample must not
    /// poison the state (`REQ-VEL-016`).
    #[test]
    fn hostile_samples_do_not_latch_it() {
        let mut envelope = Envelope::new(RATE);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            envelope.push(value);
            assert!(envelope.level().is_finite(), "{value} latched it");
        }
        for _ in 0..1_000 {
            envelope.push(0.5);
        }
        assert!(envelope.decibels() > -12.0);
    }

    /// The time constants are seconds, so the same signal has to read the same
    /// at any sample rate.
    #[test]
    fn the_rate_does_not_change_it() {
        let mut readings = Vec::new();
        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut envelope = Envelope::new(rate);
            for sample in tone(0.4, 220.0, rate, (0.5 * rate) as usize) {
                envelope.push(sample);
            }
            readings.push(envelope.decibels());
        }
        let first = readings[0];
        for reading in &readings {
            assert!((reading - first).abs() < 0.5, "{readings:?}");
        }
    }
}
