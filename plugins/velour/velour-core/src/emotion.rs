//! `EMOTION`: the envelope moves the curve, not the level.
//!
//! This is the difference between a saturator that sounds the same at every
//! volume and one that reacts to how the line was sung (`REQ-VEL-008`). Quietly
//! sung comes out warm and dense; pushed comes out edged and excited.
//!
//! **It moves `(β, h, k)` and never a gain.** A level-following gain is a
//! compressor, and there is one of those already (`DENSITY`). What changes here
//! is which harmonics get made — the shaper's normalisation
//! (`crate::shaper`) is what makes that possible, because it holds the
//! generator's output level still while the curve underneath it changes.
//!
//! **Block rate.** The envelope moves over hundreds of milliseconds, so
//! resolving the curve once per block is inaudible — and doing it per sample
//! would mean recomputing the normalisation for every sample
//! (`crate::shaper::Shaper::set`).

/// Where the axis is centred: the level a vocal sits at when it is not being
/// pushed. **Ear-tuned** (`dsp.md`, `VEL-17`).
pub const REF_DB: f32 = -18.0;

/// And how far from there counts as all the way. 12 dB is about the span
/// between a held note and a belted one.
pub const RANGE_DB: f32 = 12.0;

/// How far each of the three follows the deflection, at `amount = 1`.
///
/// The signs are the character claim: **louder is less even, harder, and
/// more** (`dsp.md`). All three provisional.
const BIAS_FOLLOW: f32 = 0.50;
const HARDNESS_FOLLOW: f32 = 0.40;
const DRIVE_FOLLOW: f32 = 0.30;

/// Where the envelope sits on the axis, `-1..=1`. Zero is a vocal at
/// [`REF_DB`].
pub fn deflection(env_db: f32) -> f32 {
    if !env_db.is_finite() {
        return 0.0;
    }
    ((env_db - REF_DB) / RANGE_DB).clamp(-1.0, 1.0)
}

/// Moves one band's curve by `motion` — the deflection already multiplied by
/// the amount, so **zero is exactly the curve that went in** (`REQ-VEL-008`).
///
/// Takes and returns `(bias, hardness, drive)` in the order
/// [`crate::shaper::Shaper::set`] wants them, and clamps nothing: `set` already
/// does, and doing it twice would hide which range actually applies.
pub fn modulate(bias: f32, hardness: f32, drive: f32, motion: f32) -> (f32, f32, f32) {
    if !motion.is_finite() || motion == 0.0 {
        return (bias, hardness, drive);
    }
    (
        bias * (1.0 - BIAS_FOLLOW * motion),
        hardness + HARDNESS_FOLLOW * motion,
        drive * (1.0 + DRIVE_FOLLOW * motion),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_is_centred_on_a_resting_vocal() {
        assert_eq!(deflection(REF_DB), 0.0);
        assert_eq!(deflection(REF_DB + RANGE_DB), 1.0);
        assert_eq!(deflection(REF_DB - RANGE_DB), -1.0);
        // And it saturates rather than running away.
        assert_eq!(deflection(0.0), 1.0);
        assert_eq!(deflection(-120.0), -1.0);
        assert_eq!(deflection(f32::NAN), 0.0);
    }

    /// **The acceptance condition, at this level** (`REQ-VEL-008`): amount zero
    /// is not "nearly the same curve", it is the same curve.
    #[test]
    fn no_motion_is_bit_identical() {
        for deflection in [-1.0f32, -0.3, 0.0, 0.7, 1.0] {
            let motion = 0.0 * deflection;
            assert_eq!(modulate(0.3, 0.4, 2.0, motion), (0.3, 0.4, 2.0));
        }
    }

    /// The direction the whole feature claims.
    #[test]
    fn louder_is_less_even_harder_and_more() {
        let (bias, hardness, drive) = modulate(0.3, 0.4, 2.0, 1.0);
        assert!(bias < 0.3, "even harmonics did not drop");
        assert!(hardness > 0.4, "the knee did not harden");
        assert!(drive > 2.0, "the harmonics did not grow");
    }

    /// And quieter is the mirror image, or the control only works one way.
    #[test]
    fn quieter_is_the_other_direction() {
        let (bias, hardness, drive) = modulate(0.3, 0.4, 2.0, -1.0);
        assert!(bias > 0.3);
        assert!(hardness < 0.4);
        assert!(drive < 2.0);
    }

    /// Hostile input reaches this from a host through the amount knob.
    #[test]
    fn a_hostile_motion_leaves_the_curve_alone() {
        for motion in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(modulate(0.3, 0.4, 2.0, motion), (0.3, 0.4, 2.0));
        }
    }
}
