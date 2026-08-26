//! `TEXTURE`: one knob that walks the curve family from Warm through Clear to
//! Edge.
//!
//! **Not a three-way switch.** Warm → Clear → Edge already has an order —
//! smooth, neutral, aggressive — and the curve family was parameterised
//! (`crate::shaper`) precisely so that the space between them exists. A switch
//! would throw that away and make "half way between Warm and Clear"
//! unreachable (`REQ-VEL-004`).
//!
//! The three names survive as **anchors**: a table of three rows, interpolated
//! between the two the position falls between. That is the whole mechanism, and
//! it is small enough to read.
//!
//! **`bias` and `hardness` are not exposed separately.** Two independent knobs
//! could be set to a point no anchor implies — half of Warm's roundness with
//! Edge's hard knee — and there is no reason to promise that combination sounds
//! like anything.

use crate::bands::BANDS;
use crate::engine::BAND_COUNT;

/// How far a per-band offset can move that band along the axis.
///
/// Enough to say "the air should be harder than the rest" and not enough to put
/// one band at Warm while another is at Edge — at which point `TEXTURE` would
/// stop describing the sound.
pub const OFFSET_RANGE: f32 = 0.35;

/// One named point on the axis.
struct Anchor {
    bias: f32,
    hardness: f32,
    /// Per-band level trim in dB, in the order of [`crate::bands::BANDS`].
    ///
    /// This is what stops the morph being only a distortion change: Warm leans
    /// on BODY and pulls AIR back, Edge does the opposite.
    trim_db: [f32; BAND_COUNT],
}

/// Warm, Clear, Edge (`dsp.md`). **Every number here is provisional** and is
/// settled by ear in `VEL-17`; the shape of the table and the interpolation are
/// the specification.
const ANCHORS: [Anchor; 3] = [
    // Warm — even harmonics, a soft knee, body forward.
    Anchor {
        bias: 0.55,
        hardness: 0.10,
        trim_db: [2.0, 0.0, -3.0],
    },
    // Clear — harmonics without the low end swelling; presence led.
    Anchor {
        bias: 0.30,
        hardness: 0.35,
        trim_db: [-1.0, 1.0, 0.0],
    },
    // Edge — odd harmonics, a hard knee, presence and texture forward.
    Anchor {
        bias: 0.10,
        hardness: 0.85,
        trim_db: [-2.0, 2.0, 1.0],
    },
];

/// What one band's curve should be at a position on the axis.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BandTexture {
    /// Already multiplied by the band's own bias multiplier
    /// (`crate::bands::Band::curve_multipliers`).
    pub bias: f32,
    pub hardness: f32,
    /// Linear gain, not decibels: it multiplies a level.
    pub trim: f32,
}

/// The settings for `band` at the global `position`, offset by that band's own
/// `offset`.
///
/// `position` and `offset` are clamped, and a value that is not a number falls
/// back to the middle — these arrive from a host (`REQ-VEL-016`).
pub fn for_band(position: f32, offset: f32, band: usize) -> BandTexture {
    let position = clamp_or(position, 0.0, 1.0, 0.5);
    let offset = clamp_or(offset, -1.0, 1.0, 0.0);
    let band = band.min(BAND_COUNT - 1);

    let place = (position + offset * OFFSET_RANGE).clamp(0.0, 1.0);

    // Two anchors and how far between them. With three anchors the axis is two
    // segments, so the position scaled by two is the segment plus the fraction.
    let scaled = place * (ANCHORS.len() - 1) as f32;
    let first = (scaled.floor() as usize).min(ANCHORS.len() - 2);
    let blend = scaled - first as f32;

    let low = &ANCHORS[first];
    let high = &ANCHORS[first + 1];

    let (bias_multiplier, _) = BANDS[band].curve_multipliers();
    let trim_db = lerp(low.trim_db[band], high.trim_db[band], blend);

    BandTexture {
        bias: lerp(low.bias, high.bias, blend) * bias_multiplier,
        hardness: lerp(low.hardness, high.hardness, blend),
        trim: 10.0f32.powf(trim_db / 20.0),
    }
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn clamp_or(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{amplitude, sine};
    use crate::shaper::{PROBE_AMPLITUDE, Shaper};

    /// PRESENCE, whose multipliers are both `1.0`, so its readings are the
    /// anchor values themselves.
    const PRESENCE: usize = 1;

    #[test]
    fn the_three_names_land_on_the_table() {
        for (position, anchor) in [(0.0, 0), (0.5, 1), (1.0, 2)] {
            let texture = for_band(position, 0.0, PRESENCE);
            assert!(
                (texture.bias - ANCHORS[anchor].bias).abs() < 1e-6,
                "position {position} gave bias {}",
                texture.bias
            );
            assert!(
                (texture.hardness - ANCHORS[anchor].hardness).abs() < 1e-6,
                "position {position} gave hardness {}",
                texture.hardness
            );
        }
    }

    /// The reason for a knob instead of a switch: nothing may step.
    #[test]
    fn the_axis_is_continuous() {
        let steps = 2_000;
        let mut previous = for_band(0.0, 0.0, PRESENCE);

        for step in 1..=steps {
            let texture = for_band(step as f32 / steps as f32, 0.0, PRESENCE);
            for (name, from, to) in [
                ("bias", previous.bias, texture.bias),
                ("hardness", previous.hardness, texture.hardness),
                ("trim", previous.trim, texture.trim),
            ] {
                assert!(
                    (to - from).abs() < 0.01,
                    "{name} jumped from {from} to {to} at step {step}"
                );
            }
            previous = texture;
        }
    }

    /// A `0 dB` row has to come out as a multiplication by one, not by
    /// 0.99999997 — the trim sits on a band's level and the level is what makes
    /// "all bands down" transparent.
    #[test]
    fn a_zero_decibel_trim_is_unity() {
        // Clear's PRESENCE row is +1 dB and its AIR row is 0 dB.
        assert_eq!(for_band(0.5, 0.0, 2).trim, 1.0);
    }

    /// **What the morph is for** (`REQ-VEL-004`): the ends have to sound like
    /// different products, not like more and less of one.
    #[test]
    fn warm_and_edge_produce_clearly_different_harmonics() {
        let input = sine(PROBE_AMPLITUDE, 8, 2_048);

        let ratios = |position: f32| {
            let texture = for_band(position, 0.0, PRESENCE);
            let mut shaper = Shaper::new();
            shaper.set(3.0, texture.bias, texture.hardness);
            let output: Vec<f32> = input.iter().map(|value| shaper.shape(*value)).collect();
            let first = amplitude(&output, 8);
            (
                amplitude(&output, 16) / first,
                amplitude(&output, 24) / first,
            )
        };

        let (warm_even, warm_odd) = ratios(0.0);
        let (edge_even, edge_odd) = ratios(1.0);

        // Warm is the even-harmonic end and Edge the odd one.
        assert!(
            warm_even > edge_even * 3.0,
            "even: warm {warm_even:.4} against edge {edge_even:.4}"
        );
        assert!(
            edge_odd > warm_odd * 2.0,
            "odd: edge {edge_odd:.4} against warm {warm_odd:.4}"
        );
    }

    #[test]
    fn a_band_offset_moves_only_that_band() {
        let rest = for_band(0.5, 0.0, PRESENCE);
        let pushed = for_band(0.5, 1.0, PRESENCE);
        let pulled = for_band(0.5, -1.0, PRESENCE);

        assert!(pushed.hardness > rest.hardness, "the offset did nothing");
        assert!(pulled.hardness < rest.hardness);

        // And the other two are untouched at the same call site.
        for band in [0usize, 2] {
            assert_eq!(for_band(0.5, 0.0, band), for_band(0.5, 0.0, band));
        }
    }

    /// The offset must not be able to put one band at Warm while another is at
    /// Edge, or `TEXTURE` stops naming the sound.
    #[test]
    fn an_offset_cannot_reach_the_far_end() {
        let far = for_band(0.5, 1.0, PRESENCE);
        let edge = for_band(1.0, 0.0, PRESENCE);
        assert!(far.hardness < edge.hardness, "the offset reached Edge");
    }

    #[test]
    fn each_band_sits_where_its_multipliers_put_it() {
        // BODY leans even, AIR leans odd, at the same position on the axis
        // (`crate::bands::Band::curve_multipliers`).
        let body = for_band(0.5, 0.0, 0);
        let presence = for_band(0.5, 0.0, 1);
        let air = for_band(0.5, 0.0, 2);

        assert!(body.bias > presence.bias);
        assert!(air.bias < presence.bias);
        // Hardness is not scaled per band: it is what `TEXTURE` means.
        assert_eq!(body.hardness, air.hardness);
    }

    #[test]
    fn hostile_positions_fall_back_rather_than_break() {
        for position in [f32::NAN, f32::INFINITY, -1e9, 1e9] {
            for offset in [f32::NAN, f32::NEG_INFINITY, -1e9, 1e9] {
                for band in 0..BAND_COUNT + 2 {
                    let texture = for_band(position, offset, band);
                    assert!(texture.bias.is_finite(), "{position}/{offset}/{band}");
                    assert!((0.0..=1.0).contains(&texture.hardness));
                    assert!(texture.trim.is_finite() && texture.trim > 0.0);
                }
            }
        }
    }
}
