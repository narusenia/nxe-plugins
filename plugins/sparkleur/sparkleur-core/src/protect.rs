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

/// The band-to-reference power ratio above which the presence band is being
/// held back.
///
/// **Measured, not guessed** (`SPK-18`). Zero says "the 1.5–5 kHz band carries
/// as much power as the 200–1200 Hz band under it", which spectrally ordinary
/// material sits 1.7 dB below:
///
/// | | ratio |
/// |---|---|
/// | six steady partials | −6.8 dB |
/// | **pink noise** | **−1.7 dB** |
/// | pink, presence +3 dB | +0.2 dB |
/// | hi-hats | +3.8 dB |
/// | pink, presence +10 dB | +5.4 dB |
/// | white noise | +5.6 dB |
///
/// It shipped at −8 dB, which is below every one of those — so it pulled 1.3 dB
/// out of plain pink noise at the defaults, exactly what its own comment said
/// it must not do. The tests it had used two tones with almost nothing in the
/// guarded band, so nothing caught it until the whole engine was measured on
/// broadband material.
const THRESHOLD_DB: f32 = 0.0;

/// De-Harsh's shape (`dsp.md`).
const DE_HARSH: Settings<1> = Settings {
    // **Below the band it judges, not around it** (`SPK-18`). Velour's guards
    // share a 300 Hz–8 kHz reference, and copying it here put the guarded band
    // inside its own reference: lifting 1.5–5 kHz by 10 dB lifted the reference
    // by 6 of them, so ten decibels of harshness read as two and the threshold
    // had almost nothing to sit between. Taking the guarded band out of the
    // reference widens that span from **2.0 dB to 7.1 dB**.
    //
    // Velour is not changed to match. It is released, its `CLAP_ID` is in
    // people's projects, and this is a different judgement about a different
    // band — a shared mechanism carrying one product's tuning is what `SPK-1`
    // took apart.
    reference: Band {
        low_hz: 200.0,
        high_hz: 1_200.0,
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
    use nxe_audio::harmonics::{pink, tone};

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

    /// Pink noise with the painful band lifted: a signal that is harsh rather
    /// than merely loud.
    fn tilted(boost_db: f32, length: usize) -> Vec<f32> {
        let mut band = nxe_audio::biquad::BandPass::new(1_500.0, 5_000.0, RATE);
        let gain = 10.0f32.powf(boost_db / 20.0) - 1.0;
        pink(0.15, length)
            .iter()
            .map(|sample| sample + band.process(*sample) * gain)
            .collect()
    }

    /// How long the followers are given before anything is believed.
    ///
    /// **The reference follower starts at zero**, so for the first instants the
    /// band is compared against silence and every signal reads as infinitely
    /// harsh. Measured from sample zero, plain pink noise pulls the full 12 dB.
    const SETTLE: usize = 4_800;

    /// The furthest it pulled at any point after it settled.
    ///
    /// **Not the reading left at the end**, which on noise is one arbitrary
    /// instant — a guard that fires in bursts would measure as doing nothing.
    fn worst(input: &[f32], amount: f32) -> f32 {
        let mut de_harsh = DeHarsh::new(RATE);
        let mut worst = 0.0f32;
        for (index, sample) in input.iter().enumerate() {
            de_harsh.push(*sample, amount);
            if index > SETTLE {
                worst = worst.min(de_harsh.reduction_db());
            }
        }
        worst
    }

    /// **Ordinary material does not move it, and harsh material does**
    /// (`SPK-18`, `REQ-SPK-008`).
    ///
    /// This is the test the shipping threshold did not have. The one next to it
    /// balances two tones, and a two-tone signal with almost nothing in the
    /// guarded band passes at any threshold at all — which is how −8 dB survived
    /// to the point where the whole engine was measured and found to be pulling
    /// 1.3 dB out of plain pink noise.
    ///
    /// The ratio each material actually sits at, over four settled seconds:
    ///
    /// | | p50 | p95 | max |
    /// |---|---|---|---|
    /// | six steady partials | −7.04 | −6.80 | −6.69 |
    /// | **pink noise** | **−1.67** | **−0.40** | **+0.29** |
    /// | pink, presence +6 dB | +2.69 | +3.77 | +4.86 |
    /// | pink, presence +10 dB | +5.57 | +6.45 | +7.58 |
    ///
    /// A threshold of zero therefore sits above everything ordinary material
    /// reaches — pink crosses it in the top one per cent of instants, by three
    /// tenths of a decibel — while leaving five decibels of pull available on
    /// material that is genuinely presence-heavy.
    #[test]
    fn ordinary_material_does_not_move_it_and_harsh_material_does() {
        let length = 48_000;

        // Spectrally neutral, and something with even less up there.
        assert_eq!(
            worst(&pink(0.15, length), 1.0),
            0.0,
            "it fired on pink noise"
        );
        let mut mixed = vec![0.0f32; length];
        for hz in [110.0f32, 220.0, 330.0, 550.0, 1_100.0, 3_300.0] {
            for (sample, value) in mixed.iter_mut().zip(tone(0.05, hz, RATE, length)) {
                *sample += value;
            }
        }
        assert_eq!(worst(&mixed, 1.0), 0.0, "it fired on steady partials");

        // And it grows with how harsh the material actually is.
        let mut previous = 0.0f32;
        for boost_db in [3.0f32, 6.0, 10.0] {
            let pull = worst(&tilted(boost_db, length), 1.0);
            assert!(
                pull < previous - 0.5,
                "+{boost_db:.0} dB pulled {pull:.2}, no more than +{:.0} dB did ({previous:.2})",
                boost_db - 3.0
            );
            previous = pull;
        }
        assert!(
            previous < -3.0,
            "ten decibels of harshness only pulled {previous:.2} dB"
        );
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
