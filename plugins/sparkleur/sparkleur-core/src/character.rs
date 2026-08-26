//! One knob that walks the whole product from POLISH through GLOSS to CRUSH.
//!
//! **Not a two-mode switch** (`REQ-SPK-006`). Soft/Hard makes "eight tenths of
//! Soft" unreachable, and eight tenths of Soft is where most material wants to
//! sit. The named points survive as **anchors**: a table of three rows,
//! interpolated between the two the position falls between. Velour reached the
//! same conclusion for `TEXTURE` (`velour_core::texture`), and this is
//! deliberately not the same code — the rows carry different things.
//!
//! ## What is on the axis, and what is not
//!
//! The axis carries **shape**: the two ratios, the ceiling, the knee, the
//! curve's odd/even balance, how hard the protections work, and where `SPEED`
//! sits. It does not carry the **thresholds** — those are one pair of numbers
//! for the product (`dynamics::DOWN_THRESHOLD_DB`), because a threshold that
//! moved with the character would make the axis change how much is happening as
//! well as what it sounds like, and `REQ-SPK-010` wants those separable.
//!
//! ## The odd/even balance is the point
//!
//! POLISH leans on even harmonics (sheen, thickness), CRUSH on odd (edge).
//! `nxe_audio::shaper` produces both continuously from `(β, h)`, so **the axis
//! needs no DSP of its own** — it is a table and a lerp.
//!
//! ## The level trim is here from the start
//!
//! Changing ratios and a knee moves the average level, so the axis will need
//! level correction. Velour discovered that after the fact and had to fit nine
//! trims into a finished `TEXTURE` (`VEL-17`). The column exists here before
//! anything needs it; `SPK-18` settles the numbers by ear.

use crate::dynamics::{Curve, DOWN_THRESHOLD_DB, UP_THRESHOLD_DB};

/// Where the axis sits by default.
///
/// **Toward POLISH** (`REQ-SPK-006`): "easy and pretty" is the main use, and
/// CRUSH is something to reach for rather than to start from.
pub const DEFAULT_POSITION: f32 = 0.27;

/// One named point on the axis.
///
/// **Every number is provisional** and settled by ear in `SPK-18`; the shape of
/// the table and the interpolation are the specification (`dsp.md`).
struct Anchor {
    down_ratio: f32,
    up_ratio: f32,
    ceiling_db: f32,
    knee_db: f32,
    /// The curve's asymmetry — high is even-harmonic, low is odd.
    bias: f32,
    hardness: f32,
    de_harsh: f32,
    sub_protect: f32,
    speed_centre: f32,
    trim_db: f32,
}

const ANCHORS: [Anchor; 3] = [
    // POLISH — gentle ratios, a wide knee, even harmonics, protections on.
    Anchor {
        down_ratio: 1.5,
        up_ratio: 1.2,
        ceiling_db: 6.0,
        knee_db: 12.0,
        bias: 0.50,
        hardness: 0.15,
        de_harsh: 1.0,
        sub_protect: 0.0,
        speed_centre: 0.35,
        trim_db: 0.0,
    },
    // GLOSS — the middle of everything.
    Anchor {
        down_ratio: 2.5,
        up_ratio: 1.5,
        ceiling_db: 9.0,
        knee_db: 6.0,
        bias: 0.30,
        hardness: 0.35,
        de_harsh: 0.6,
        sub_protect: 0.4,
        speed_centre: 0.5,
        trim_db: 0.0,
    },
    // CRUSH — hard ratios, a corner for a knee, odd harmonics, protections off
    // except the one that stops the bottom exploding.
    Anchor {
        down_ratio: 6.0,
        up_ratio: 3.0,
        ceiling_db: 15.0,
        knee_db: 1.0,
        bias: 0.10,
        hardness: 0.80,
        de_harsh: 0.2,
        sub_protect: 1.0,
        speed_centre: 0.75,
        trim_db: 0.0,
    },
];

/// Everything one position on the axis decides.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Character {
    /// The gain computer's shape (`SPK-4`).
    pub curve: Curve,
    /// What `Sparkle` shapes with (`SPK-6`).
    pub bias: f32,
    pub hardness: f32,
    /// How hard De-Harsh pulls, `0..=1` (`SPK-7`).
    pub de_harsh: f32,
    /// How far the bottom band's ceiling is closed, `0..=1`. **Sub Protect is
    /// not a process** — it is this number times that band's ceiling
    /// (`REQ-SPK-008`).
    pub sub_protect: f32,
    /// Where `SPEED` sits when its own control is centred (`SPK-8` combines
    /// them).
    pub speed_centre: f32,
    /// The level correction the axis needs, in dB. **Provisional** — the frame
    /// exists so that `SPK-18` has somewhere to put the answer.
    pub trim_db: f32,
}

/// The character at `position`, `0..=1` from POLISH to CRUSH.
///
/// A position that is not a number falls back to the middle of the axis rather
/// than to an end: these arrive from a host, and the middle is the one setting
/// that is certainly meant (`REQ-SPK-017`).
pub fn at(position: f32) -> Character {
    let position = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.5
    };

    // Three anchors make two segments, so the position scaled by two is the
    // segment plus the fraction along it.
    let scaled = position * (ANCHORS.len() - 1) as f32;
    let first = (scaled.floor() as usize).min(ANCHORS.len() - 2);
    let blend = scaled - first as f32;

    let low = &ANCHORS[first];
    let high = &ANCHORS[first + 1];
    let between = |from: f32, to: f32| from + (to - from) * blend;

    Character {
        curve: Curve {
            down_threshold_db: DOWN_THRESHOLD_DB,
            down_ratio: between(low.down_ratio, high.down_ratio),
            up_threshold_db: UP_THRESHOLD_DB,
            up_ratio: between(low.up_ratio, high.up_ratio),
            knee_db: between(low.knee_db, high.knee_db),
            ceiling_db: between(low.ceiling_db, high.ceiling_db),
        },
        bias: between(low.bias, high.bias),
        hardness: between(low.hardness, high.hardness),
        de_harsh: between(low.de_harsh, high.de_harsh),
        sub_protect: between(low.sub_protect, high.sub_protect),
        speed_centre: between(low.speed_centre, high.speed_centre),
        trim_db: between(low.trim_db, high.trim_db),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field of one character, so a scan can look at all of them without
    /// naming each one twice.
    fn fields(character: &Character) -> [f32; 10] {
        [
            character.curve.down_ratio,
            character.curve.up_ratio,
            character.curve.knee_db,
            character.curve.ceiling_db,
            character.bias,
            character.hardness,
            character.de_harsh,
            character.sub_protect,
            character.speed_centre,
            character.trim_db,
        ]
    }

    /// The named points are the table, exactly — an axis whose ends are not
    /// where the table says is an axis nobody can reason about.
    #[test]
    fn the_three_anchors_are_exactly_the_table() {
        for (position, anchor) in [
            (0.0f32, &ANCHORS[0]),
            (0.5, &ANCHORS[1]),
            (1.0, &ANCHORS[2]),
        ] {
            let character = at(position);
            let wanted = [
                anchor.down_ratio,
                anchor.up_ratio,
                anchor.knee_db,
                anchor.ceiling_db,
                anchor.bias,
                anchor.hardness,
                anchor.de_harsh,
                anchor.sub_protect,
                anchor.speed_centre,
                anchor.trim_db,
            ];
            for (index, (got, want)) in fields(&character).iter().zip(wanted).enumerate() {
                assert!(
                    (got - want).abs() < 1e-5,
                    "at {position}, field {index} is {got}, not {want}"
                );
            }
        }
    }

    /// The thresholds are **not** on the axis (`REQ-SPK-010`).
    #[test]
    fn the_thresholds_do_not_move_with_the_axis() {
        for step in 0..=100 {
            let curve = at(step as f32 / 100.0).curve;
            assert_eq!(curve.down_threshold_db, DOWN_THRESHOLD_DB);
            assert_eq!(curve.up_threshold_db, UP_THRESHOLD_DB);
        }
    }

    /// **No step anywhere along it** (`REQ-SPK-006`), including at the middle
    /// anchor, which is where a table of three is easiest to get wrong.
    #[test]
    fn the_axis_has_no_discontinuity() {
        const STEP: f32 = 0.001;

        let mut previous = fields(&at(0.0));
        let mut worst = [0.0f32; 10];
        for index in 1..=1_000 {
            let current = fields(&at(index as f32 * STEP));
            for (slot, (now, before)) in worst.iter_mut().zip(current.iter().zip(previous)) {
                *slot = slot.max((now - before).abs());
            }
            previous = current;
        }

        // The steepest field spans 15 dB (the ceiling) over half the axis, so
        // one thousandth of the axis can move it by 0.03. Anything an order
        // larger is a jump.
        for (index, step) in worst.iter().enumerate() {
            assert!(*step < 0.1, "field {index} jumped {step} in one step");
        }
    }

    /// Moving along it has to actually change the shape, or the axis is
    /// decoration.
    #[test]
    fn the_ends_are_far_apart() {
        let polish = at(0.0);
        let crush = at(1.0);

        assert!(crush.curve.down_ratio > polish.curve.down_ratio * 3.0);
        assert!(crush.curve.up_ratio > polish.curve.up_ratio * 2.0);
        assert!(crush.curve.knee_db < polish.curve.knee_db * 0.2);
        assert!(crush.hardness > polish.hardness * 4.0);
        // The odd/even balance runs the other way, which is the point
        // (`REQ-SPK-006`).
        assert!(crush.bias < polish.bias * 0.3);
    }

    /// The default sits toward POLISH, not in the middle (`REQ-SPK-006`).
    #[test]
    fn the_default_leans_toward_polish() {
        let default = at(DEFAULT_POSITION);
        let gloss = at(0.5);
        assert!(default.curve.down_ratio < gloss.curve.down_ratio);
        assert!(default.bias > gloss.bias);
    }

    #[test]
    fn a_hostile_position_falls_back_to_the_middle() {
        let middle = at(0.5);
        for position in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(at(position), middle, "{position} did not fall back");
        }
        // And the ends clamp rather than extrapolate.
        assert_eq!(at(-1e9), at(0.0));
        assert_eq!(at(1e9), at(1.0));
    }
}
