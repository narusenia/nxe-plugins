//! The two things that stop "brighter" turning into "painful", neither of which
//! is a process of its own.
//!
//! **De-Harsh** is one instance of the relative guard Velour already shipped
//! (`nxe_audio::guard`), pointed at the presence band. **Sub Protect** is the
//! bottom band's upward ceiling closed down — a number, not a block
//! (`REQ-SPK-008`). Writing either as new machinery would put a second thing to
//! keep in step next to a first that already works.
//!
//! ## Why the axis owns how hard they work
//!
//! "Brighter without becoming painful" is the promise, so the protection has to
//! arrive with the brightness rather than be found later in a panel. That is
//! what `CHARACTER` carrying `de_harsh` and `sub_protect` means, and it is how
//! the concept document's "more Spark, more harshness suppression" is honoured
//! without a hidden link between two controls (`REQ-SPK-008`).
//!
//! Advanced offers a **bipolar deviation** on top: zero follows the axis, ±
//! leans. An absolute control there would make `CHARACTER` and the panel write
//! the same value, which the UI rules forbid (`.agents/rules/vizia.md`).
//!
//! ## Why De-Harsh has its own band-pass
//!
//! It listens through a filter of its own rather than reading the split band 4.
//! `FOCUS` moves where band 4 is; **it does not move where a vocal turns
//! painful** (`dsp.md`).

use nxe_audio::guard::{Band, GuardedBand, RelativeGuard, Settings};

use crate::crossover::BAND_COUNT;

/// The band De-Harsh pulls: PRESENCE, the fourth of five.
pub const GUARDED_BAND: usize = 3;
/// The band Sub Protect closes: SUB, the first.
pub const PROTECTED_BAND: usize = 0;

/// **Ear-tuned** (`SPK-18`): the band-to-reference power ratio ordinary
/// material sits at, so ordinary material does not move it at all.
const THRESHOLD_DB: f32 = -8.0;

/// De-Harsh's shape (`dsp.md`).
const DE_HARSH: Settings<1> = Settings {
    // Wide enough to stand for "the sound", narrow enough that rumble and air
    // do not swamp it — the same reference Velour's guards share.
    reference: Band {
        low_hz: 300.0,
        high_hz: 8_000.0,
    },
    bands: [GuardedBand {
        // Where it hurts. Fixed, because `FOCUS` does not move the ear.
        band: Band {
            low_hz: 1_500.0,
            high_hz: 5_000.0,
        },
        threshold_db: THRESHOLD_DB,
    }],
    attack_seconds: 0.002,
    release_seconds: 0.060,
    slope: 1.0,
    max_reduction_db: 12.0,
};

/// One relative guard, pointed at the presence band.
pub struct DeHarsh {
    guard: RelativeGuard<1>,
}

impl DeHarsh {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            guard: RelativeGuard::new(DE_HARSH, sample_rate),
        }
    }

    /// Feeds one sample. **Host rate, on the mono sum** — a guard that fired on
    /// one side only would throw the image sideways (`REQ-SPK-011`).
    pub fn push(&mut self, mono: f32, amount: f32) {
        self.guard.push(mono, [amount]);
    }

    /// The gain band [`GUARDED_BAND`] should be multiplied by. `1.0` is "not
    /// holding back".
    pub fn gain(&self) -> f32 {
        self.guard.gain(0)
    }

    /// How far it is pulling, in dB. Zero when it is doing nothing.
    ///
    /// **The picture draws this** (`SPK-13`, `REQ-SPK-008`): a protection that
    /// works invisibly leaves the user with an `AIR` knob that does nothing and
    /// no way to find out why.
    pub fn reduction_db(&self) -> f32 {
        self.guard.reduction_db(0)
    }

    pub fn reset(&mut self) {
        self.guard.reset();
    }
}

/// How hard a protection works: what the axis says, moved by the panel.
///
/// **Additive and clamped**, so zero is exactly "follow `CHARACTER`" and the
/// bottom of the deviation is exactly off whatever the axis said
/// (`REQ-SPK-008`).
pub fn amount_of(from_character: f32, deviation: f32) -> f32 {
    let base = finite(from_character).clamp(0.0, 1.0);
    let deviation = finite(deviation).clamp(-1.0, 1.0);
    (base + deviation).clamp(0.0, 1.0)
}

/// How much of the curve's ceiling each band may use.
///
/// **Sub Protect in full is this and nothing else** (`REQ-SPK-008`): the bottom
/// band's share closed, every other band untouched. At `1.0` the bottom band's
/// upward compression is exactly nothing.
pub fn ceiling_scales(sub_protect: f32) -> [f32; BAND_COUNT] {
    let mut scales = [1.0f32; BAND_COUNT];
    scales[PROTECTED_BAND] = 1.0 - finite(sub_protect).clamp(0.0, 1.0);
    scales
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character;
    use crate::dynamics::{Curve, Weights, band_gain_db};
    use nxe_audio::harmonics::tone;

    const RATE: f32 = 48_000.0;

    /// A quiet tone standing for "the rest of the sound", plus one in the
    /// painful band at `harsh`.
    fn signal(harsh: f32, length: usize) -> Vec<f32> {
        let body = tone(0.3, 400.0, RATE, length);
        let edge = tone(harsh, 2_800.0, RATE, length);
        body.iter().zip(&edge).map(|(a, b)| a + b).collect()
    }

    fn settle(input: &[f32], amount: f32) -> f32 {
        let mut de_harsh = DeHarsh::new(RATE);
        for sample in input {
            de_harsh.push(*sample, amount);
        }
        de_harsh.reduction_db()
    }

    /// **The property the relative detector exists for** (`REQ-SPK-008`). It is
    /// the same measurement Velour's guard passes, on the same mechanism — which
    /// is the point of `SPK-1` having moved it.
    #[test]
    fn the_reduction_does_not_depend_on_input_gain() {
        let base = signal(0.6, 12_000);
        let mut readings = Vec::new();

        // Four times either side of unity is ±12 dB.
        for gain in [0.25f32, 0.5, 1.0, 2.0, 4.0] {
            let scaled: Vec<f32> = base.iter().map(|sample| sample * gain).collect();
            readings.push(settle(&scaled, 1.0));
        }

        let first = readings[0];
        assert!(first < -1.0, "it never fired: {first:.1} dB");
        for reading in &readings {
            assert!(
                (reading - first).abs() < 0.2,
                "gain changed the reduction: {readings:?}"
            );
        }
    }

    #[test]
    fn a_balanced_signal_does_not_fire_it_and_a_harsh_one_does() {
        assert_eq!(
            settle(&signal(0.02, 12_000), 1.0),
            0.0,
            "it fired on balance"
        );
        let harsh = settle(&signal(0.8, 12_000), 1.0);
        assert!(harsh < -3.0, "it barely reacted: {harsh:.1} dB");
    }

    /// Zero deviation follows the axis, and the bottom of the deviation is
    /// **exactly** off (`REQ-SPK-008`).
    #[test]
    fn the_axis_sets_the_amount_and_the_deviation_leans_on_it() {
        for position in [0.0f32, 0.27, 0.5, 1.0] {
            let from_axis = character::at(position).de_harsh;
            assert_eq!(amount_of(from_axis, 0.0), from_axis, "at {position}");
            assert_eq!(amount_of(from_axis, -1.0), 0.0, "at {position}");
            assert_eq!(amount_of(from_axis, 1.0), 1.0, "at {position}");
        }

        // POLISH protects hardest, CRUSH least (`dsp.md`).
        assert!(character::at(0.0).de_harsh > character::at(1.0).de_harsh);
    }

    #[test]
    fn the_minimum_deviation_turns_it_exactly_off() {
        let input = signal(0.8, 12_000);
        let off = amount_of(character::at(0.0).de_harsh, -1.0);
        assert_eq!(settle(&input, off), 0.0);

        let mut de_harsh = DeHarsh::new(RATE);
        for sample in &input {
            de_harsh.push(*sample, off);
        }
        assert_eq!(de_harsh.gain(), 1.0, "an amount of zero still coloured it");

        // And the measurement can fail: the axis's own amount does pull.
        assert!(settle(&input, character::at(0.0).de_harsh) < -3.0);
    }

    #[test]
    fn reset_clears_it() {
        let mut de_harsh = DeHarsh::new(RATE);
        for sample in signal(0.8, 12_000) {
            de_harsh.push(sample, 1.0);
        }
        assert!(de_harsh.gain() < 1.0);
        de_harsh.reset();
        assert_eq!(de_harsh.gain(), 1.0);
    }

    /// **Sub Protect touches one band** (`REQ-SPK-008`).
    #[test]
    fn sub_protect_closes_only_the_bottom_bands_ceiling() {
        for amount in [0.0f32, 0.4, 1.0] {
            let scales = ceiling_scales(amount);
            assert!((scales[PROTECTED_BAND] - (1.0 - amount)).abs() < 1e-6);
            for (band, scale) in scales.iter().enumerate().skip(1) {
                assert_eq!(*scale, 1.0, "band {band} was touched at {amount}");
            }
        }
    }

    /// And what it means for the gain: the bottom band stops lifting while the
    /// others carry on.
    #[test]
    fn sub_protect_at_full_stops_the_bottom_band_lifting() {
        // Deep enough to be asking for the whole ceiling, and clear of the
        // floor's fade.
        const LEVEL_DB: f32 = -58.0;
        let curve = Curve {
            up_ratio: 3.0,
            ceiling_db: 15.0,
            ..Curve::GLOSS
        };
        let lift = |scale| {
            band_gain_db(
                LEVEL_DB,
                &curve,
                &Weights {
                    ceiling_scale: scale,
                    ..Weights::NEUTRAL
                },
                1.0,
                crate::dynamics::FLOOR_DB,
            )
        };

        // Open, the lift is what the ratio asks for and the ceiling is not
        // even reached.
        let open = lift(1.0);
        assert!(
            open > 5.0,
            "the band was not lifting to begin with: {open:.2}"
        );
        assert!(open < curve.ceiling_db, "the ceiling was already binding");

        // Closed to 0.6, the ceiling **is** what binds, and it binds at exactly
        // that share of it.
        let closed = lift(0.6);
        let expected = curve.ceiling_db * 0.6;
        assert!(
            (closed - expected).abs() < 1e-3,
            "a 0.6 ceiling gave {closed:.3} dB, not {expected:.3}"
        );

        assert_eq!(lift(0.0), 0.0, "a fully closed ceiling still lifted");
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            assert!((0.0..=1.0).contains(&amount_of(value, value)));
            for scale in ceiling_scales(value) {
                assert!((0.0..=1.0).contains(&scale), "{value} gave {scale}");
            }

            let mut de_harsh = DeHarsh::new(RATE);
            for sample in signal(0.8, 4_800) {
                de_harsh.push(sample, value);
            }
            assert!(de_harsh.gain().is_finite() && de_harsh.gain() > 0.0);
            assert!(de_harsh.reduction_db().is_finite());
        }
    }
}
