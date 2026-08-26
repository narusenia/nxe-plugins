//! Pulling a generator back when the band it feeds is already too loud.
//!
//! The mechanism is [`nxe_audio::guard`]; what is Velour about it is the two
//! bands, the two thresholds and how hard it may pull. Sparkleur's De-Harsh is
//! the same block with one band (`SPK-1`).
//!
//! This is the half of the product's promise that is normally invisible: "in
//! front without being painful" is kept by something that only acts
//! occasionally, and in most plugins nothing on screen says when. What it does
//! here is reported ([`Guards::reduction_db`]) so the interface can show it
//! (`REQ-VEL-013`).
//!
//! ## Why relative
//!
//! A guard measures its band's energy **against a wider reference band**, not
//! against a threshold in dBFS. An absolute detector fires on every loud
//! moment — which is a loud note, not a harsh one — and suppressing those would
//! fight `EMOTION`, whose whole job is to make loud moments sound different
//! (`REQ-VEL-006`, `REQ-VEL-008`).
//!
//! ## What it moves
//!
//! The generator's **output gain**, and nothing else. Moving the curve's drive
//! or bias instead would make the band change character as it faded, and then
//! nobody could tell what had happened. This is also what separates it from a
//! de-esser: **the level is not reduced, the distortion is.**

use nxe_audio::guard::{Band, GuardedBand, RelativeGuard, Settings};

/// The widest the guard may pull, in dB. Past this the band is gone rather than
/// protected. Read by the interface as well (`REQ-VEL-018`).
pub const MAX_REDUCTION_DB: f32 = 18.0;

/// Velour's guards: where they listen, how far they let a band run, and how
/// fast they move.
///
/// **Ear-tuned** (`dsp.md`): the thresholds are the band-to-reference ratio a
/// normal vocal sits at, so a normal vocal does not move them at all.
const SETTINGS: Settings<2> = Settings {
    // Wide enough to stand for "the voice", narrow enough that rumble and air
    // do not swamp it.
    reference: Band {
        low_hz: 300.0,
        high_hz: 8_000.0,
    },
    bands: [
        // Presence. Centred where a vocal turns painful.
        GuardedBand {
            band: Band {
                low_hz: 2_000.0,
                high_hz: 5_000.0,
            },
            threshold_db: -8.0,
        },
        // Air. Where an `s` lives.
        GuardedBand {
            band: Band {
                low_hz: 5_000.0,
                high_hz: 10_000.0,
            },
            threshold_db: -14.0,
        },
    ],
    // Fast enough to catch one consonant, slow enough not to chatter — a guard
    // that opens and closes inside a syllable would be heard as the thing it is
    // protecting against.
    attack_seconds: 0.002,
    release_seconds: 0.060,
    slope: 1.0,
    max_reduction_db: MAX_REDUCTION_DB,
};

/// How much a guard is holding its generator back, and which generator.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Guarded {
    Presence,
    Air,
}

pub const GUARDS: [Guarded; 2] = [Guarded::Presence, Guarded::Air];

impl Guarded {
    /// The position in [`GUARDS`], which is also the position in
    /// [`Settings::bands`]. The two orders are the same on purpose: one list to
    /// keep in step instead of a mapping to get wrong.
    fn index(self) -> usize {
        match self {
            Guarded::Presence => 0,
            Guarded::Air => 1,
        }
    }
}

/// Both guards and the reference they share.
pub struct Guards {
    guard: RelativeGuard<2>,
}

impl Guards {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            guard: RelativeGuard::new(SETTINGS, sample_rate),
        }
    }

    /// Feeds one sample and updates the gains. **Host rate**, on the mono sum.
    ///
    /// The sum rather than each channel separately: a guard that fired on one
    /// side only would throw the voice sideways, which is the one thing a vocal
    /// processor cannot do (`REQ-VEL-011`).
    pub fn push(&mut self, mono: f32, amounts: [f32; 2]) {
        self.guard.push(mono, amounts);
    }

    /// The gain the named generator should be multiplied by.
    pub fn gain(&self, guarded: Guarded) -> f32 {
        self.guard.gain(guarded.index())
    }

    /// How far each guard is pulling, in dB — for the display
    /// (`REQ-VEL-018`). Zero when it is doing nothing.
    pub fn reduction_db(&self, guarded: Guarded) -> f32 {
        self.guard.reduction_db(guarded.index())
    }

    pub fn reset(&mut self) {
        self.guard.reset();
    }
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

        assert!(
            partial < 0.0 && complete < partial,
            "{partial} / {complete}"
        );
        // Slope 1, so half the amount is half the reduction.
        assert!(
            (partial * 2.0 - complete).abs() < 0.5,
            "{partial} / {complete}"
        );
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
