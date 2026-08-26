//! Parameter declarations and the mapping to `velour_core`'s plain values.
//!
//! The adapter the architecture calls for: `velour-core` knows nothing about
//! nih-plug, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **This is `VEL-5`'s subset**, not the whole control surface. `TEXTURE`
//! arrives with `VEL-4`, and the Advanced layer with `VEL-9` and `VEL-14`
//! (`plugins/velour/docs/implementation/velour-plan.md`). Adding a parameter
//! later is safe — nih-plug keys them by id, not by position — but **changing or
//! removing an id is not**, so the ids below are as final as `CLAP_ID`.

use nih_plug::prelude::*;
use velour_core::engine::{Levels, Shape};
use velour_core::oversample::Factor;

/// How hard the generator bus runs internally.
///
/// A separate type from `velour_core::Factor` on purpose: deriving nih-plug's
/// `Enum` on the core type would make the core depend on nih-plug.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum FactorParam {
    #[id = "2x"]
    #[name = "2x"]
    Two,
    #[id = "4x"]
    #[name = "4x"]
    Four,
}

impl From<FactorParam> for Factor {
    fn from(value: FactorParam) -> Self {
        match value {
            FactorParam::Two => Factor::Two,
            FactorParam::Four => Factor::Four,
        }
    }
}

#[derive(Params)]
pub struct VelourParams {
    #[id = "drive"]
    pub drive: FloatParam,

    /// The three band faders. **Linear amplitude, and zero really is zero** —
    /// which is what makes "every band down" bit-transparent
    /// (`REQ-VEL-001`). A decibel range would have to stop somewhere above
    /// silence.
    #[id = "body"]
    pub body: FloatParam,
    #[id = "presence"]
    pub presence: FloatParam,
    #[id = "air"]
    pub air: FloatParam,

    /// Walks Warm → Clear → Edge (`velour_core::texture`). **One knob, not a
    /// switch plus a knob**: the three names are anchors on a continuous axis,
    /// and the interface marks them on the track (`REQ-VEL-004`).
    #[id = "texture"]
    pub texture: FloatParam,

    #[id = "focus"]
    pub focus: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "output"]
    pub output: FloatParam,
    #[id = "os"]
    pub oversample: EnumParam<FactorParam>,
}

impl Default for VelourParams {
    fn default() -> Self {
        Self {
            // **Every default here is provisional.** The plugin has no presets
            // (`REQ-VEL-020`), so these *are* the product's face, and they are
            // settled by ear in `VEL-17` — not now.
            drive: percentage("Drive", 0.40),
            body: percentage("Body", 0.50),
            presence: percentage("Presence", 0.60),
            air: percentage("Air", 0.40),

            // Clear, the middle. Which of the three is the right default is a
            // `VEL-17` question.
            texture: percentage("Texture", 0.50),

            focus: FloatParam::new("Focus", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_unit(" oct")
                // Moving `FOCUS` rebuilds twelve sets of filter coefficients, so
                // the smoothing is what keeps a drag from stepping. The edges
                // themselves are moved once per block (`velour_core::Engine`).
                .with_smoother(SmoothingStyle::Linear(50.0))
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

            mix: percentage("Mix", 0.50),

            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            // 4x by default: 2x is a cost saving, not an equal — it leaves
            // aliasing about 14 dB higher (`velour_core::oversample`).
            oversample: EnumParam::new("Oversample", FactorParam::Four),
        }
    }
}

/// A `0..=100%` control. One helper because five of them are the same shape, and
/// a difference between two of them should be visible rather than buried in a
/// wall of builders.
fn percentage(name: &'static str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

impl VelourParams {
    /// What the engine needs once per block.
    ///
    /// **`next_step(samples)`, not `next()`.** A smoother advances one sample per
    /// `next()`, so reading it once per block would stretch a 20 ms ramp out to
    /// twenty milliseconds' worth of *blocks* — slower by the block size and
    /// different on every host.
    ///
    /// These two are smoothed even though the engine uses them per block: the
    /// smoothed value is where a drag lives, and the raw one would step.
    pub fn shape(&self, samples: u32) -> Shape {
        Shape {
            drive: self.drive.smoothed.next_step(samples),
            texture: self.texture.smoothed.next_step(samples),
            // The per-band offsets arrive with the Advanced layer (`VEL-14`).
            // They exist in the engine already, and there is nowhere to put
            // three more knobs until there is somewhere to put them.
            texture_offsets: [0.0; velour_core::BAND_COUNT],
            focus: self.focus.smoothed.next_step(samples),
            factor: self.oversample.value().into(),
        }
    }

    /// What the engine needs per sample.
    pub fn levels(&self) -> Levels {
        Levels {
            bands: [
                self.body.smoothed.next(),
                self.presence.smoothed.next(),
                self.air.smoothed.next(),
            ],
            mix: self.mix.smoothed.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use velour_core::BANDS;
    use velour_core::bands::Band;

    /// The hinge of the bit-transparency claim (`REQ-VEL-012`). If a unity trim
    /// were 0.9999997 instead of 1.0, "bypassed is bit identical" would quietly
    /// stop being true and nothing would sound wrong.
    #[test]
    fn a_zero_decibel_trim_is_exactly_unity() {
        assert_eq!(util::db_to_gain(0.0), 1.0);
    }

    /// [`VelourParams::levels`] fills an array by position, so the engine's band
    /// order is part of this file's contract. Swapping two of them would move
    /// BODY's fader onto AIR and still compile.
    #[test]
    fn the_band_order_matches_the_engine() {
        assert_eq!(BANDS, [Band::Body, Band::Presence, Band::Air]);
    }

    #[test]
    fn every_default_sits_inside_its_range() {
        let params = VelourParams::default();
        for (name, value) in [
            ("drive", params.drive.default_plain_value()),
            ("body", params.body.default_plain_value()),
            ("presence", params.presence.default_plain_value()),
            ("air", params.air.default_plain_value()),
            ("texture", params.texture.default_plain_value()),
            ("mix", params.mix.default_plain_value()),
        ] {
            assert!((0.0..=1.0).contains(&value), "{name} defaults to {value}");
        }
        assert_eq!(params.focus.default_plain_value(), 0.0);
        assert_eq!(params.output.default_plain_value(), 0.0);
    }
}
