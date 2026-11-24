//! Parameter declarations, and the mapping to `air_core`'s plain values.
//!
//! The adapter the architecture calls for: `air-core` knows nothing about
//! truce, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Ids are truce's hash of the field name**, so renaming a field is renaming a
//! parameter for every saved session. The display `name` is free to change.
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

use crate::analysis::Analysis;
use air_core::Shape;
use air_core::follow::{BRIGHTNESS, DETECTORS, ENVELOPE, TRANSIENT};
use nxe_audio::oversample::Factor;
use nxe_egui::text;
use truce::prelude::*;

/// How hard the harmonic half runs internally.
///
/// A separate type from `nxe_audio::Factor` on purpose: deriving truce's
/// `ParamEnum` on the shared type would make it depend on truce.
#[derive(ParamEnum, Debug)]
pub enum FactorParam {
    #[name = "2x"]
    Two,
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
    /// What the editor is shown about the sound. Not a parameter: the audio
    /// thread writes it and the editor reads it, and the params `Arc` is the
    /// one thing both are handed (`analysis.rs`).
    #[skip]
    pub analysis: Analysis,

    /// How much surface to make. **Zero is exactly nothing**, and one of the
    /// two ways out of the plugin (`REQ-AIR-001`).
    ///
    /// **Not named `AIR`.** Velour and Sparkleur both have a knob by that name
    /// already, and a plugin called Air with a knob called `AIR` would be the
    /// third (`REQ-AIR-010`).
    #[param(
        name = "Surface",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub surface: FloatParam,

    /// Harmonic ↔ noise. A power-preserving crossfade, so the middle is not a
    /// dip (`REQ-AIR-005`).
    #[param(
        name = "Blend",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub blend: FloatParam,

    /// The quality axis. **Its meaning moves with `BLEND`** — a knee on the
    /// harmonic side, grain on the noise side — which is why there is one of it
    /// rather than one per half (`REQ-AIR-005`).
    #[param(
        name = "Character",
        range = "linear(0, 1)",
        default = 0.72,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub character: FloatParam,

    /// Where the layer sits. Moves both halves together (`REQ-AIR-006`).
    #[param(name = "Focus", range = "linear(-1, 1)", default = -0.5, format = "octaves", parse = "number", smooth = "linear(50)")]
    pub focus: FloatParam,

    /// How far the layer spreads. **The noise half only** — the harmonic half
    /// is made from the input, and widening correlated material takes an
    /// all-pass, which is a comb in mono (`REQ-AIR-008`).
    #[param(
        name = "Width",
        range = "linear(0, 1)",
        default = 0.6,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub width: FloatParam,

    /// How much of the layer to add. **It does not turn the original down**:
    /// the topology is additive, so `MIX` scales what was added
    /// (`REQ-AIR-012`).
    /// How much of the input's movement the layer answers to. **This is what
    /// Air is** (`REQ-AIR-002`): at zero the layer is static, which is Velour;
    /// with only the transient deviation up it is Sparkleur's Sparkle.
    #[param(
        name = "Follow",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub follow: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    #[param(
        name = "Output",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub output: FloatParam,

    /// Advanced: the curve's own numbers, which `CHARACTER` does not reach
    /// (`REQ-AIR-010`).
    ///
    /// The defaults are `air_core::harmonic::DRIVE` and `BIAS`, copied because a
    /// truce default has to be a literal; `the_curve_defaults_are_the_core_ones`
    /// keeps the copies honest. The ranges are `nxe_audio::shaper`'s.
    #[param(
        name = "Drive",
        group = "Curve",
        range = "linear(0.05, 8)",
        default = 4.0,
        format = "plain",
        parse = "number",
        smooth = "linear(30)"
    )]
    pub drive: FloatParam,
    #[param(
        name = "Bias",
        group = "Curve",
        range = "linear(0, 0.8)",
        default = 0.3,
        format = "plain",
        parse = "number",
        smooth = "linear(30)"
    )]
    pub bias: FloatParam,

    /// Advanced: each detector's depth **as a deviation from `FOLLOW`**
    /// (`REQ-AIR-010`). Zero is "as the macro says".
    #[param(
        name = "Follow Envelope",
        group = "Follow",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub follow_envelope: FloatParam,
    #[param(
        name = "Follow Brightness",
        group = "Follow",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub follow_brightness: FloatParam,
    #[param(
        name = "Follow Transient",
        group = "Follow",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub follow_transient: FloatParam,

    /// Advanced: the protection's bipolar deviation (`REQ-AIR-009`). Zero is
    /// the shipped threshold; **the bottom of the range is exactly off** and
    /// the top is stricter.
    #[param(
        name = "Guard",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub guard: FloatParam,

    /// 4x by default: 2x is a cost saving, not an equal — it leaves aliasing
    /// about 14 dB higher (`nxe_audio::oversample`).
    #[param(name = "Oversample", default = 1, parse = "factor")]
    pub oversample: EnumParam<FactorParam>,
}

impl AirParams {
    /// **Signed, always.** A minus that appears the moment the knob crosses
    /// centre makes the row twitch (`.agents/rules/ui.md`).
    fn signed_percent(&self, value: f64) -> String {
        format!("{:+.0} %", value * 100.0)
    }

    fn decibels(&self, value: f64) -> String {
        format!("{value:+.1} dB")
    }

    fn octaves(&self, value: f64) -> String {
        format!("{value:+.2} oct")
    }

    fn plain(&self, value: f64) -> String {
        format!("{value:.2}")
    }

    // **Every parameter reads its own text back** (`nxe_egui::text`).
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn factor(&self, text: &str) -> Option<f64> {
        text::choice(&["2x", "4x"], text)
    }
}

impl AirParams {
    /// What the engine needs once per block.
    ///
    /// **`next_step(samples)`, not `next()`.** A smoother advances one sample
    /// per call, so reading it once per block would stretch every ramp by the
    /// block length — the trap `VEL-5` hit.
    pub fn shape(&self, samples: usize) -> Shape {
        Shape {
            focus: self.focus.read_after(samples),
            character: self.character.read_after(samples),
            blend: self.blend.read_after(samples),
            width: self.width.read_after(samples),
            drive: self.drive.read_after(samples),
            bias: self.bias.read_after(samples),
            depths: self.depths(samples),
            guard: self.guard.read_after(samples),
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
    /// Each detector's depth, **without touching a smoother** — for the
    /// window, which must not steal a step from the audio thread.
    pub fn display_depths(&self) -> [f32; DETECTORS] {
        let follow = self.follow.value();
        let mut depths = [0.0; DETECTORS];
        depths[ENVELOPE] = (follow + self.follow_envelope.value()).clamp(0.0, 1.0);
        depths[BRIGHTNESS] = (follow + self.follow_brightness.value()).clamp(0.0, 1.0);
        depths[TRANSIENT] = (follow + self.follow_transient.value()).clamp(0.0, 1.0);
        depths
    }

    fn depths(&self, samples: usize) -> [f32; DETECTORS] {
        let follow = self.follow.read_after(samples);
        let mut depths = [0.0; DETECTORS];
        depths[ENVELOPE] = (follow + self.follow_envelope.read_after(samples)).clamp(0.0, 1.0);
        depths[BRIGHTNESS] = (follow + self.follow_brightness.read_after(samples)).clamp(0.0, 1.0);
        depths[TRANSIENT] = (follow + self.follow_transient.read_after(samples)).clamp(0.0, 1.0);
        depths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_defaults_are_the_core_ones() {
        let params = AirParams::default();
        assert_eq!(params.drive.value(), air_core::harmonic::DRIVE);
        assert_eq!(params.bias.value(), air_core::harmonic::BIAS);
        let infos = params.param_infos();
        let drive = infos
            .iter()
            .find(|info| info.id == params.drive.id())
            .unwrap();
        let bias = infos
            .iter()
            .find(|info| info.id == params.bias.id())
            .unwrap();
        assert_eq!(drive.range.min() as f32, nxe_audio::shaper::DRIVE_MIN);
        assert_eq!(drive.range.max() as f32, nxe_audio::shaper::DRIVE_MAX);
        assert_eq!(bias.range.max() as f32, nxe_audio::shaper::BIAS_MAX);
    }

    /// **Every parameter reads back its own text** — a typed value in the
    /// window, and clap-validator's `param-conversions`.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        let params = AirParams::default();
        for info in params.param_infos() {
            for plain in [info.range.min(), info.range.max()] {
                let text = params.format_value(info.id, plain).unwrap();
                let back = params.parse_value(info.id, &text);
                assert!(
                    back.is_some_and(|back| (back - plain).abs() < 0.01),
                    "{} printed {plain} as {text:?} and read back {back:?}",
                    info.name
                );
            }
        }
    }
}
