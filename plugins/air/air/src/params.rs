//! Parameter declarations, and the mapping to `air_core`'s plain values.
//!
//! The adapter the architecture calls for: `air-core` knows nothing about
//! nih-plug, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Adding a parameter later is safe** — nih-plug keys them by id, not by
//! position — but **changing or removing an id is not**, so the ids below are
//! as final as `CLAP_ID`. The guard's deviation arrives in `AIR-6`; it is not
//! declared here because a control that does nothing is worse than one that
//! does not exist yet.
//!
//! ## Two layers
//!
//! Seven everyday controls plus an output trim (`REQ-AIR-010`). `DRIVE` and
//! `BIAS` are Advanced — the curve's own numbers, which `CHARACTER`
//! deliberately does not reach — and so are the three following deviations.
//!
//! **A deviation is bipolar and rests at zero**, meaning "as `FOLLOW` says".
//! A control that rests on the macro is how two parameters avoid writing one
//! value (`REQ-AIR-010`).

use air_core::Shape;
use air_core::follow::{BRIGHTNESS, DETECTORS, ENVELOPE, TRANSIENT};
use nih_plug::prelude::*;
use nxe_audio::oversample::Factor;
use nxe_audio::shaper::{BIAS_MAX, DRIVE_MAX, DRIVE_MIN};

/// How hard the harmonic half runs internally.
///
/// A separate type from `nxe_audio::Factor` on purpose: deriving nih-plug's
/// `Enum` on the shared type would make it depend on nih-plug.
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
pub struct AirParams {
    /// How much surface to make. **Zero is exactly nothing**, and one of the
    /// two ways out of the plugin (`REQ-AIR-001`).
    ///
    /// **Not named `AIR`.** Velour and Sparkleur both have a knob by that name
    /// already, and a plugin called Air with a knob called `AIR` would be the
    /// third (`REQ-AIR-010`).
    #[id = "surface"]
    pub surface: FloatParam,

    /// Harmonic ↔ noise. A power-preserving crossfade, so the middle is not a
    /// dip (`REQ-AIR-005`).
    #[id = "blend"]
    pub blend: FloatParam,

    /// The quality axis. **Its meaning moves with `BLEND`** — a knee on the
    /// harmonic side, grain on the noise side — which is why there is one of it
    /// rather than one per half (`REQ-AIR-005`).
    #[id = "character"]
    pub character: FloatParam,

    /// Where the layer sits. Moves both halves together (`REQ-AIR-006`).
    #[id = "focus"]
    pub focus: FloatParam,

    /// How far the layer spreads. **The noise half only** — the harmonic half
    /// is made from the input, and widening correlated material takes an
    /// all-pass, which is a comb in mono (`REQ-AIR-008`).
    #[id = "width"]
    pub width: FloatParam,

    /// How much of the layer to add. **It does not turn the original down**:
    /// the topology is additive, so `MIX` scales what was added
    /// (`REQ-AIR-012`).
    /// How much of the input's movement the layer answers to. **This is what
    /// Air is** (`REQ-AIR-002`): at zero the layer is static, which is Velour;
    /// with only the transient deviation up it is Sparkleur's Sparkle.
    #[id = "follow"]
    pub follow: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,

    #[id = "output"]
    pub output: FloatParam,

    /// Advanced: the curve's own numbers, which `CHARACTER` does not reach
    /// (`REQ-AIR-010`).
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "bias"]
    pub bias: FloatParam,

    /// Advanced: each detector's depth **as a deviation from `FOLLOW`**
    /// (`REQ-AIR-010`). Zero is "as the macro says".
    #[id = "fol_env"]
    pub follow_envelope: FloatParam,
    #[id = "fol_brt"]
    pub follow_brightness: FloatParam,
    #[id = "fol_trn"]
    pub follow_transient: FloatParam,

    #[id = "os"]
    pub oversample: EnumParam<FactorParam>,
}

impl Default for AirParams {
    fn default() -> Self {
        Self {
            // **Provisional.** The plugin has no presets (`REQ-AIR-022`), so the
            // defaults are the product's face and `AIR-13` settles them by ear.
            surface: percentage("Surface", 0.35),
            // The middle, because the two halves are the product's two answers
            // and shipping against one stop would hide the other.
            blend: percentage("Blend", 0.5),
            character: percentage("Character", 0.35),

            focus: FloatParam::new(
                "Focus",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" oct")
            // Moving `FOCUS` rebuilds ten sets of filter coefficients per
            // channel, so the smoothing is what keeps a drag from stepping.
            // The corners themselves move once per block (`air_core::layer`).
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // Wide, because the layer sitting around the source rather than on
            // it is what the product is for (`REQ-AIR-008`).
            width: percentage("Width", 0.60),
            // Halfway: the layer answers to the music without disappearing
            // between phrases. **Provisional** like every other default here.
            follow: percentage("Follow", 0.50),
            mix: percentage("Mix", 1.0),
            output: decibels("Output"),

            drive: FloatParam::new(
                "Drive",
                air_core::harmonic::DRIVE,
                FloatRange::Linear {
                    min: DRIVE_MIN,
                    max: DRIVE_MAX,
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            bias: FloatParam::new(
                "Bias",
                air_core::harmonic::BIAS,
                FloatRange::Linear {
                    min: 0.0,
                    max: BIAS_MAX,
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            follow_envelope: bipolar("Follow Envelope"),
            follow_brightness: bipolar("Follow Brightness"),
            follow_transient: bipolar("Follow Transient"),

            // 4x by default: 2x is a cost saving, not an equal — it leaves
            // aliasing about 14 dB higher (`nxe_audio::oversample`).
            oversample: EnumParam::new("Oversample", FactorParam::Four),
        }
    }
}

/// A `-1..=1` control resting at zero, for a deviation from a macro.
fn bipolar(name: &'static str) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: -1.0,
            max: 1.0,
        },
    )
    .with_smoother(SmoothingStyle::Linear(30.0))
    .with_value_to_string(formatters::v2s_f32_rounded(2))
}

/// A `0..=100%` control.
fn percentage(name: &'static str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}

/// A `±12 dB` trim resting at unity.
fn decibels(name: &'static str) -> FloatParam {
    FloatParam::new(
        name,
        0.0,
        FloatRange::Linear {
            min: -12.0,
            max: 12.0,
        },
    )
    .with_unit(" dB")
    .with_smoother(SmoothingStyle::Linear(20.0))
    .with_value_to_string(formatters::v2s_f32_rounded(1))
}

impl AirParams {
    /// What the engine needs once per block.
    ///
    /// **`next_step(samples)`, not `next()`.** A smoother advances one sample
    /// per call, so reading it once per block would stretch every ramp by the
    /// block length — the trap `VEL-5` hit.
    pub fn shape(&self, samples: u32) -> Shape {
        Shape {
            focus: self.focus.smoothed.next_step(samples),
            character: self.character.smoothed.next_step(samples),
            blend: self.blend.smoothed.next_step(samples),
            width: self.width.smoothed.next_step(samples),
            drive: self.drive.smoothed.next_step(samples),
            bias: self.bias.smoothed.next_step(samples),
            depths: self.depths(samples),
            factor: self.oversample.value().into(),
        }
    }

    /// `FOLLOW` plus each detector's deviation, clamped.
    ///
    /// **The macro and the deviation never write the same value**
    /// (`REQ-AIR-010`): the deviation is relative, so moving `FOLLOW` leaves
    /// every deviation where the user put it.
    ///
    /// **Order matters** — these go into an array the engine reads by position,
    /// and `the_detector_order_matches_the_engine` is what holds it in place.
    fn depths(&self, samples: u32) -> [f32; DETECTORS] {
        let follow = self.follow.smoothed.next_step(samples);
        let mut depths = [0.0; DETECTORS];
        depths[ENVELOPE] =
            (follow + self.follow_envelope.smoothed.next_step(samples)).clamp(0.0, 1.0);
        depths[BRIGHTNESS] =
            (follow + self.follow_brightness.smoothed.next_step(samples)).clamp(0.0, 1.0);
        depths[TRANSIENT] =
            (follow + self.follow_transient.smoothed.next_step(samples)).clamp(0.0, 1.0);
        depths
    }
}
