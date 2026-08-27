//! Two followers with independent attack and release: one over amplitude, one
//! over power.
//!
//! ## Peak, not RMS
//!
//! Asked "how hard is this being played", a peak follower with a slow release
//! answers with one number that does not dip between events — that is
//! [`Envelope`]. Where the question is instead "is this band loud against that
//! one", power is the right measure, and that is [`Power`].
//!
//! ## The time constants belong to the caller
//!
//! They are what decides whether a follower reads phrases or single events, and
//! the right pair depends on the material: Velour's are tuned for a sung line
//! (`velour_core::envelope`) and Sparkleur derives its own from band centres
//! (`REQ-SPK-005`). Passing them in is what keeps a shared follower from
//! carrying one product's taste into another.

/// The quietest level worth reporting. `20·log10` of zero is `-inf`, and an
/// infinity multiplied by a knob is a NaN in the coefficients.
const FLOOR: f32 = 1e-6;

/// A one-pole's coefficient for a time constant in **seconds**.
///
/// One definition for the whole workspace: a one-pole reaches `1 - 1/e` of a
/// step in one time constant. Three modules had their own copy of this line
/// before it was shared.
pub fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

/// A follower over **power**, with independent attack and release.
///
/// ## Why this is not [`Envelope`]
///
/// [`Envelope`] answers "how hard is this being played" and follows the peak of
/// an amplitude. This answers "how much energy is in this", which is the right
/// measure whenever two readings are about to be compared as a ratio: the
/// instantaneous ratio of two amplitudes swings from end to end on almost any
/// material, and power holds still enough to threshold ([`crate::guard`],
/// `nxe_dsp::PanScope`).
///
/// ## Why it lives here
///
/// It was written twice — once inside [`crate::guard`]'s band follower and once
/// inside Sparkleur's transient gate — and Air asked for a third. The
/// architecture's rule is that the third caller is what lifts a block, not the
/// second (`docs/specifications/architecture.md`).
///
/// **The time constants stay with the caller**, the same split `SPK-1` made for
/// [`Envelope`]: this is the mechanism, and which pair of numbers reads a
/// syllable rather than a phrase is the product's decision.
///
/// **It does not sanitise its input.** A NaN pushed in latches it, exactly as
/// the two hand-written copies did — the callers already clean what they read,
/// and cleaning twice would hide where the responsibility sits.
#[derive(Clone, Copy, Default)]
pub struct Power {
    energy: f32,
    attack: f32,
    release: f32,
}

impl Power {
    pub fn new(attack_seconds: f32, release_seconds: f32, sample_rate: f32) -> Self {
        Self {
            energy: 0.0,
            attack: coefficient(attack_seconds, sample_rate),
            release: coefficient(release_seconds, sample_rate),
        }
    }

    /// Feeds one **already squared** sample and returns the new energy.
    ///
    /// Squared rather than raw, because every caller either squares a filtered
    /// sample or already holds the square.
    pub fn push(&mut self, squared: f32) -> f32 {
        let step = if squared > self.energy {
            self.attack
        } else {
            self.release
        };
        self.energy += (squared - self.energy) * step;
        self.energy
    }

    pub fn energy(&self) -> f32 {
        self.energy
    }

    pub fn reset(&mut self) {
        self.energy = 0.0;
    }
}

/// A peak follower, in decibels.
pub struct Envelope {
    level: f32,
    attack: f32,
    release: f32,
}

impl Envelope {
    /// Attack and release in **seconds**, so the same pair means the same thing
    /// whatever rate the host is running at.
    pub fn new(attack_seconds: f32, release_seconds: f32, sample_rate: f32) -> Self {
        Self {
            level: 0.0,
            attack: coefficient(attack_seconds, sample_rate),
            release: coefficient(release_seconds, sample_rate),
        }
    }

    /// **Audio rate.** Feed it the mono sum, so that whatever reads it does not
    /// move sideways when one channel is louder.
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

    /// Velour's pair, because it is the one that has been listened to
    /// (`velour_core::envelope`). The properties below are about the mechanism,
    /// not about these two numbers.
    const ATTACK_SECONDS: f32 = 0.005;
    const RELEASE_SECONDS: f32 = 0.150;

    fn envelope(sample_rate: f32) -> Envelope {
        Envelope::new(ATTACK_SECONDS, RELEASE_SECONDS, sample_rate)
    }

    /// A steady tone has to read as its own amplitude, or every threshold built
    /// on top of this is calibrated against nothing.
    #[test]
    fn a_steady_tone_reads_its_amplitude() {
        let mut envelope = envelope(RATE);
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
        let mut envelope = envelope(RATE);
        // One attack time constant of full scale, so about 63%.
        for _ in 0..(ATTACK_SECONDS * RATE) as usize {
            envelope.push(1.0);
        }
        let risen = envelope.level();
        assert!((risen - 0.63).abs() < 0.05, "{risen}");

        // The same span of silence must take far less off than it put on.
        for _ in 0..(ATTACK_SECONDS * RATE) as usize {
            envelope.push(0.0);
        }
        let fallen = envelope.level();
        assert!(fallen > risen * 0.9, "fell from {risen} to {fallen}");
    }

    /// Silence reads as the floor and not as an infinity.
    #[test]
    fn silence_reads_as_the_floor() {
        let envelope = envelope(RATE);
        assert_eq!(envelope.decibels(), 20.0 * FLOOR.log10());
    }

    /// The trap this crate has already hit twice: one hostile sample must not
    /// poison the state (`REQ-VEL-016`).
    #[test]
    fn hostile_samples_do_not_latch_it() {
        let mut envelope = envelope(RATE);
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
            let mut envelope = envelope(rate);
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
