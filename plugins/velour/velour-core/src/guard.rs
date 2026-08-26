//! Pulling a generator back when the band it feeds is already too loud.
//!
//! **Move candidate**: Sparkleur's De-Harsh is the same block, so nothing here
//! knows about Velour (`REQ-VEL-015`).
//!
//! This is the half of the product's promise that is normally invisible: "in
//! front without being painful" is kept by something that only acts
//! occasionally, and in most plugins nothing on screen says when. What it does
//! here is reported (`Guards::reduction`) so the interface can show it
//! (`REQ-VEL-013`).
//!
//! ## Relative, not absolute
//!
//! A guard measures its band's energy **against a wider reference band**, not
//! against a threshold in dBFS. An absolute detector fires on every loud
//! moment — which is a loud note, not a harsh one — and suppressing those would
//! fight `EMOTION`, whose whole job is to make loud moments sound different
//! (`REQ-VEL-006`, `REQ-VEL-008`).
//!
//! The useful side effect is that **it does not depend on input gain**: the
//! numerator and the denominator move together, so the ratio does not.
//!
//! ## What it moves
//!
//! The generator's **output gain**, and nothing else. Moving the curve's drive
//! or bias instead would make the band change character as it faded, and then
//! nobody could tell what had happened. This is also what separates it from a
//! de-esser: **the level is not reduced, the distortion is.**

use crate::biquad::BandPass;

/// How fast a follower rises and falls. Fast enough to catch one consonant,
/// slow enough not to chatter — a guard that opens and closes inside a syllable
/// would be heard as the thing it is protecting against.
const ATTACK_SECONDS: f32 = 0.002;
const RELEASE_SECONDS: f32 = 0.060;

/// The widest the guard may pull, in dB. Past this the band is gone rather than
/// protected.
pub const MAX_REDUCTION_DB: f32 = 18.0;

/// How many dB of reduction per dB of excess.
pub const SLOPE: f32 = 1.0;

/// The reference band every guard measures against. Wide enough to stand for
/// "the sound", narrow enough that rumble and air do not swamp it.
const REFERENCE_LOW_HZ: f32 = 300.0;
const REFERENCE_HIGH_HZ: f32 = 8_000.0;

/// Guards the presence band. Centred where a vocal turns painful.
const HARSH_LOW_HZ: f32 = 2_000.0;
const HARSH_HIGH_HZ: f32 = 5_000.0;
/// **Ear-tuned** (`dsp.md`): the band-to-reference ratio a normal vocal sits at.
const HARSH_THRESHOLD_DB: f32 = -8.0;

/// Guards the air band. Where an `s` lives.
const SIB_LOW_HZ: f32 = 5_000.0;
const SIB_HIGH_HZ: f32 = 10_000.0;
const SIB_THRESHOLD_DB: f32 = -14.0;

/// Below this the reference is treated as silence and no guard fires. Without
/// it, the ratio of two numbers made of noise floor decides whether the plugin
/// is allowed to work.
const FLOOR: f32 = 1e-10;

/// `10·log10(x)` is `10/log2(10)` times `log2(x)`.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;
/// And a gain in dB is `2^(dB / (20·log10(2)))`.
const DECIBELS_PER_OCTAVE_AMPLITUDE: f32 = 6.020_6;

/// A band-passed energy follower.
struct Follower {
    band: BandPass,
    energy: f32,
    attack: f32,
    release: f32,
}

impl Follower {
    fn new(low_hz: f32, high_hz: f32, sample_rate: f32) -> Self {
        Self {
            band: BandPass::new(low_hz, high_hz, sample_rate),
            energy: 0.0,
            // A one-pole reaches `1 - 1/e` of a step in one time constant, which
            // is the same definition the rest of the crate uses.
            attack: 1.0 - (-1.0 / (ATTACK_SECONDS * sample_rate)).exp(),
            release: 1.0 - (-1.0 / (RELEASE_SECONDS * sample_rate)).exp(),
        }
    }

    /// **Energy, not amplitude.** The instantaneous ratio of two amplitudes
    /// swings from end to end on almost any material; power holds still enough
    /// to threshold (`nxe_dsp::PanScope` reached the same conclusion).
    fn push(&mut self, input: f32) -> f32 {
        let filtered = self.band.process(input);
        let squared = filtered * filtered;
        let coefficient = if squared > self.energy {
            self.attack
        } else {
            self.release
        };
        self.energy += (squared - self.energy) * coefficient;
        self.energy
    }

    fn reset(&mut self) {
        self.band.reset();
        self.energy = 0.0;
    }
}

/// How much a guard is holding its generator back, and which generator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Guarded {
    Presence,
    Air,
}

pub const GUARDS: [Guarded; 2] = [Guarded::Presence, Guarded::Air];

/// Both guards and the reference they share.
pub struct Guards {
    reference: Follower,
    harsh: Follower,
    sib: Follower,
    /// Linear gains, in the order of [`GUARDS`]. `1.0` is "not holding back".
    gains: [f32; 2],
    thresholds: [f32; 2],
}

impl Guards {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            reference: Follower::new(REFERENCE_LOW_HZ, REFERENCE_HIGH_HZ, sample_rate),
            harsh: Follower::new(HARSH_LOW_HZ, HARSH_HIGH_HZ, sample_rate),
            sib: Follower::new(SIB_LOW_HZ, SIB_HIGH_HZ, sample_rate),
            gains: [1.0; 2],
            // Stored as linear power ratios so the common case — nothing to do —
            // is one multiplication and a comparison.
            thresholds: [
                power_ratio(HARSH_THRESHOLD_DB),
                power_ratio(SIB_THRESHOLD_DB),
            ],
        }
    }

    /// Feeds one sample and updates the gains. **Host rate**, on the mono sum.
    ///
    /// The sum rather than each channel separately: a guard that fired on one
    /// side only would throw the voice sideways, which is the one thing a vocal
    /// processor cannot do (`REQ-VEL-011`). It also halves the filter count.
    /// The cost is that anti-phase content between the channels reads quieter
    /// than it is — acceptable for a source that lives in the middle.
    pub fn push(&mut self, mono: f32, amounts: [f32; 2]) {
        let reference = self.reference.push(mono);
        let bands = [self.harsh.push(mono), self.sib.push(mono)];

        for index in 0..2 {
            self.gains[index] = gain_of(
                bands[index],
                reference,
                self.thresholds[index],
                amounts[index],
            );
        }
    }

    /// The gain the named generator should be multiplied by.
    pub fn gain(&self, guarded: Guarded) -> f32 {
        match guarded {
            Guarded::Presence => self.gains[0],
            Guarded::Air => self.gains[1],
        }
    }

    /// How far each guard is pulling, in dB — for the display
    /// (`REQ-VEL-018`). Zero when it is doing nothing.
    pub fn reduction_db(&self, guarded: Guarded) -> f32 {
        let gain = self.gain(guarded);
        if gain >= 1.0 {
            0.0
        } else {
            20.0 * gain.log10()
        }
    }

    pub fn reset(&mut self) {
        self.reference.reset();
        self.harsh.reset();
        self.sib.reset();
        self.gains = [1.0; 2];
    }
}

/// A power ratio in dB as a plain ratio.
fn power_ratio(decibels: f32) -> f32 {
    10.0f32.powf(decibels / 10.0)
}

/// The gain one guard should apply.
///
/// Pure, so the one thing that matters — that it is flat until the ratio crosses
/// the threshold, and that it never inverts — is testable without running a
/// signal.
pub fn gain_of(band_energy: f32, reference_energy: f32, threshold: f32, amount: f32) -> f32 {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };

    // `amount == 0` has to be *exactly* off, not nearly: a guard nobody asked
    // for must not colour the sound (`REQ-VEL-006`).
    if amount == 0.0 || reference_energy < FLOOR || !band_energy.is_finite() {
        return 1.0;
    }

    // The common case is nothing to do, and it costs one multiplication.
    let allowed = reference_energy * threshold;
    if band_energy <= allowed {
        return 1.0;
    }

    // `log2`/`exp2` rather than `log10`/`powf`: same answer, cheaper, and only
    // reached while a guard is actually firing.
    let excess_db = DECIBELS_PER_OCTAVE_POWER * (band_energy / allowed).log2();
    let reduction_db = (amount * SLOPE * excess_db).min(MAX_REDUCTION_DB);
    (-reduction_db / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{sine, tone};

    const RATE: f32 = 48_000.0;

    /// A tone in the harsh band, at `amplitude`, plus a quiet tone in the
    /// reference band to stand for "the rest of the voice".
    fn signal_at(harsh_amplitude: f32, rate: f32, length: usize) -> Vec<f32> {
        let body = tone(0.3, 400.0, rate, length);
        let harsh = tone(harsh_amplitude, 2_800.0, rate, length);
        body.iter().zip(&harsh).map(|(a, b)| a + b).collect()
    }

    fn signal(harsh_amplitude: f32, length: usize) -> Vec<f32> {
        signal_at(harsh_amplitude, RATE, length)
    }

    fn settle(guards: &mut Guards, input: &[f32], amounts: [f32; 2]) -> f32 {
        for sample in input {
            guards.push(*sample, amounts);
        }
        guards.reduction_db(Guarded::Presence)
    }

    /// **The property the relative detector exists for** (`REQ-VEL-006`).
    #[test]
    fn the_reduction_does_not_depend_on_input_gain() {
        let base = signal(0.6, 12_000);
        let mut readings = Vec::new();

        // Four times either side of unity is ±12 dB.
        for gain in [0.25f32, 0.5, 1.0, 2.0, 4.0] {
            let scaled: Vec<f32> = base.iter().map(|sample| sample * gain).collect();
            let mut guards = Guards::new(RATE);
            readings.push(settle(&mut guards, &scaled, [1.0, 1.0]));
        }

        let first = readings[0];
        assert!(first < -1.0, "the guard never fired: {first:.1} dB");
        for reading in &readings {
            assert!(
                (reading - first).abs() < 0.2,
                "gain changed the reduction: {readings:?}"
            );
        }
    }

    #[test]
    fn a_harsh_band_gets_pulled_and_a_balanced_one_does_not() {
        let mut quiet = Guards::new(RATE);
        let balanced = settle(&mut quiet, &signal(0.02, 12_000), [1.0, 1.0]);
        assert_eq!(balanced, 0.0, "it fired on a balanced signal");

        let mut loud = Guards::new(RATE);
        let harsh = settle(&mut loud, &signal(0.8, 12_000), [1.0, 1.0]);
        assert!(harsh < -3.0, "it barely reacted: {harsh:.1} dB");
    }

    #[test]
    fn the_amount_scales_it_and_zero_is_exactly_off() {
        let input = signal(0.8, 12_000);

        let mut off = Guards::new(RATE);
        assert_eq!(settle(&mut off, &input, [0.0, 0.0]), 0.0);
        assert_eq!(off.gain(Guarded::Presence), 1.0);
        assert_eq!(off.gain(Guarded::Air), 1.0);

        let mut half = Guards::new(RATE);
        let partial = settle(&mut half, &input, [0.5, 0.5]);
        let mut full = Guards::new(RATE);
        let complete = settle(&mut full, &input, [1.0, 1.0]);

        assert!(partial < 0.0 && complete < partial, "{partial} / {complete}");
        // Slope 1, so half the amount is half the reduction.
        assert!((partial * 2.0 - complete).abs() < 0.5, "{partial} / {complete}");
    }

    #[test]
    fn a_guard_never_pulls_further_than_its_limit() {
        let mut guards = Guards::new(RATE);
        // Nothing but harsh: as lopsided as a signal gets.
        let input = tone(1.0, 2_800.0, RATE, 12_000);
        settle(&mut guards, &input, [1.0, 1.0]);

        let reduction = guards.reduction_db(Guarded::Presence);
        assert!(
            reduction >= -MAX_REDUCTION_DB - 0.1,
            "it pulled {reduction:.1} dB"
        );
    }

    #[test]
    fn silence_fires_nothing() {
        let mut guards = Guards::new(RATE);
        for _ in 0..12_000 {
            guards.push(0.0, [1.0, 1.0]);
        }
        assert_eq!(guards.gain(Guarded::Presence), 1.0);
        assert_eq!(guards.gain(Guarded::Air), 1.0);
    }

    #[test]
    fn the_gain_is_flat_below_the_threshold_and_monotonic_above_it() {
        let threshold = power_ratio(-8.0);
        let reference = 1.0;

        // Below and at the threshold: untouched.
        for band in [0.0f32, threshold * 0.1, threshold * 0.99, threshold] {
            assert_eq!(gain_of(band, reference, threshold, 1.0), 1.0, "{band}");
        }

        // Above: falling, never inverting, and stopping at the limit rather
        // than running away.
        let floor = (-MAX_REDUCTION_DB / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2();
        let mut previous = 1.0;
        for multiple in [1.1f32, 2.0, 10.0, 100.0, 1e6] {
            let gain = gain_of(threshold * multiple, reference, threshold, 1.0);
            assert!(gain <= previous, "{multiple} gave {gain}, was {previous}");
            assert!(gain >= floor - 1e-6, "{multiple} went past the limit: {gain}");
            previous = gain;
        }
        // And it does reach the limit, or the bound above is vacuous.
        assert!((previous - floor).abs() < 1e-6, "it stopped at {previous}");
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 0.0, 1e9];
        for band in wild {
            for reference in wild {
                for amount in wild {
                    let gain = gain_of(band, reference, power_ratio(-8.0), amount);
                    assert!(gain.is_finite() && gain > 0.0, "{band}/{reference}/{amount}");
                }
            }
        }
    }

    /// The time constants are in seconds, so they have to mean the same thing
    /// whatever the host is running at.
    #[test]
    fn the_ballistics_are_the_same_at_every_sample_rate() {
        let mut readings = Vec::new();
        for rate in [44_100.0f32, 48_000.0, 96_000.0] {
            // A quarter second at every rate, so a **fixed** cycle count is a
            // fixed frequency: `cycles * rate / length` with `length = rate/4`
            // is just `cycles * 4`. Scaling the cycles with the buffer — the
            // obvious thing — doubles the frequency instead.
            let length = (rate * 0.25) as usize;
            let body = sine(0.3, 100, length);
            let harsh = sine(0.8, 700, length);
            let input: Vec<f32> = body.iter().zip(&harsh).map(|(a, b)| a + b).collect();

            let mut guards = Guards::new(rate);
            readings.push(settle(&mut guards, &input, [1.0, 1.0]));
        }

        let first = readings[0];
        for reading in &readings {
            assert!((reading - first).abs() < 1.0, "{readings:?} disagree");
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut guards = Guards::new(RATE);
        settle(&mut guards, &signal(0.8, 12_000), [1.0, 1.0]);
        assert!(guards.gain(Guarded::Presence) < 1.0);
        guards.reset();
        assert_eq!(guards.gain(Guarded::Presence), 1.0);
    }
}
