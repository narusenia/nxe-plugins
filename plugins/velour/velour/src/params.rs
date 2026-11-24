//! Parameter declarations and the mapping to `velour_core`'s plain values.
//!
//! The adapter the architecture calls for: `velour-core` knows nothing about
//! truce, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **This is `VEL-5`'s subset**, not the whole control surface. `TEXTURE`
//! arrives with `VEL-4`, and the Advanced layer with `VEL-9` and `VEL-14`
//! (`plugins/velour/docs/implementation/velour-plan.md`). Adding a parameter
//! later is safe — truce keys them by id, not by position — but **changing or
//! removing an id is not**, so the ids below are as final as `CLAP_ID`.

use crate::analysis::Analysis;
use nxe_audio::oversample::Factor;
use nxe_egui::text;
use truce::prelude::*;
use velour_core::Mode;
use velour_core::engine::{Levels, Shape};

/// How hard the generator bus runs internally.
///
/// A separate type from `nxe_audio::Factor` on purpose: deriving truce's
/// `ParamEnum` on the core type would make the core depend on truce.
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

/// How far the macros are allowed to reach (`REQ-VEL-021`). Separate from
/// `velour_core::Mode` for the same reason as [`FactorParam`].
///
/// **Saved by index**, so a third step goes on the end or not at all.
#[derive(ParamEnum, Debug)]
pub enum ModeParam {
    #[name = "Soft"]
    Soft,
    #[name = "Hard"]
    Hard,
}

impl From<ModeParam> for Mode {
    fn from(value: ModeParam) -> Self {
        match value {
            ModeParam::Soft => Mode::Soft,
            ModeParam::Hard => Mode::Hard,
        }
    }
}

/// **Ids are truce's hash of the field name**, so renaming a field is renaming
/// a parameter for every saved session. The display `name` is free to change.
///
/// The defaults were **settled by ear on real material** (`VEL-17`), and they
/// lean in: a saturator that does almost nothing out of the box reads as
/// broken rather than as tasteful, and everything here is safe to push — the
/// dry path is untouched (`REQ-VEL-001`) and the guards are on
/// (`REQ-VEL-006`).
#[derive(Params)]
pub struct VelourParams {
    /// What the editor is shown about the sound. Not a parameter: the audio
    /// thread writes it and the editor reads it, and the params `Arc` is the
    /// one thing both are handed (`analysis.rs`).
    #[skip]
    pub analysis: Analysis,

    #[param(
        name = "Drive",
        range = "linear(0, 1)",
        default = 0.8,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub drive: FloatParam,

    /// The three band faders. **Linear amplitude, and zero really is zero** —
    /// which is what makes "every band down" bit-transparent
    /// (`REQ-VEL-001`). A decibel range would have to stop somewhere above
    /// silence.
    #[param(
        name = "Body",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub body: FloatParam,
    #[param(
        name = "Presence",
        range = "linear(0, 1)",
        default = 0.6,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub presence: FloatParam,
    #[param(
        name = "Air",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub air: FloatParam,

    /// Walks Warm → Clear → Edge (`velour_core::texture`). **One knob, not a
    /// switch plus a knob**: the three names are anchors on a continuous axis
    /// (`REQ-VEL-004`). Clear, the middle, by default.
    #[param(
        name = "Texture",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub texture: FloatParam,

    /// How hard the generator bus's input is compressed
    /// (`velour_core::density`). **Half way, not off**: it is on the texture
    /// being added, not on the voice, so a resting amount makes the texture
    /// even rather than making the performance flat (`REQ-VEL-007`).
    #[param(
        name = "Density",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub density: FloatParam,

    /// Moving it rebuilds twelve sets of filter coefficients, so the smoothing
    /// is what keeps a drag from stepping.
    #[param(
        name = "Focus",
        range = "linear(-1, 1)",
        format = "octaves",
        parse = "number",
        smooth = "linear(50)"
    )]
    pub focus: FloatParam,
    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 0.8,
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

    /// How far the macros reach (`REQ-VEL-021`). `Soft` by default.
    #[param(name = "Mode", parse = "mode")]
    pub mode: EnumParam<ModeParam>,

    /// 4x by default: 2x is a cost saving, not an equal — it leaves aliasing
    /// about 14 dB higher (`nxe_audio::oversample`).
    #[param(name = "Oversample", default = 1, parse = "factor")]
    pub oversample: EnumParam<FactorParam>,

    /// Per-band deviation from `TEXTURE`, and the drive-versus-level split
    /// (`REQ-VEL-010`). Bipolar, resting at zero.
    ///
    /// **Flat fields rather than nested groups.** The bands have names, and
    /// the automation list is where a user looks for them.
    #[param(
        name = "Body Texture",
        group = "Texture",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub texture_body: FloatParam,
    #[param(
        name = "Presence Texture",
        group = "Texture",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub texture_presence: FloatParam,
    #[param(
        name = "Air Texture",
        group = "Texture",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub texture_air: FloatParam,

    #[param(
        name = "Body Bias",
        group = "Bias",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub bias_body: FloatParam,
    #[param(
        name = "Presence Bias",
        group = "Bias",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub bias_presence: FloatParam,
    #[param(
        name = "Air Bias",
        group = "Bias",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub bias_air: FloatParam,

    /// How far each guard may pull its generator back (`REQ-VEL-006`).
    /// **Exposed rather than hidden**, and on most of the way by default: a
    /// protection nobody switched on does not keep the plugin's promise.
    #[param(
        name = "Harsh Guard",
        range = "linear(0, 1)",
        default = 0.75,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub guard_harsh: FloatParam,
    #[param(
        name = "Sib Guard",
        range = "linear(0, 1)",
        default = 0.75,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub guard_sib: FloatParam,

    /// How much the input's envelope moves the curves
    /// (`velour_core::emotion`). The plugin's differentiation, so zero is
    /// exactly static and it has to be reachable to A/B (`REQ-VEL-008`). Not
    /// zero by default: off ships sounding like every other saturator.
    #[param(
        name = "Emotion",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub emotion: FloatParam,

    /// Listen to one generator alone, with the dry muted. **Automatable on
    /// truce**, unlike on nih-plug: truce's `flags` has no "not automatable"
    /// short of `readonly`, which would also stop the window from setting it
    /// (`v2-plan.md`, V2-9). A project reopening with a solo latched sounds
    /// broken, so the window makes the state obvious.
    #[param(name = "Body Solo", group = "Solo", parse = "switch")]
    pub solo_body: BoolParam,
    #[param(name = "Presence Solo", group = "Solo", parse = "switch")]
    pub solo_presence: BoolParam,
    #[param(name = "Air Solo", group = "Solo", parse = "switch")]
    pub solo_air: BoolParam,
}

impl VelourParams {
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

    // **Every parameter reads its own text back** (`nxe_egui::text`).
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn switch(&self, text: &str) -> Option<f64> {
        text::switch(text)
    }

    fn mode(&self, text: &str) -> Option<f64> {
        text::choice(&["Soft", "Hard"], text)
    }

    fn factor(&self, text: &str) -> Option<f64> {
        text::choice(&["2x", "4x"], text)
    }
}

impl VelourParams {
    /// What the engine needs once per block.
    ///
    /// **`read_after(samples)`, not `read()`.** A smoother advances one sample per
    /// `read()`, so reading it once per block would stretch a 20 ms ramp out to
    /// twenty milliseconds' worth of *blocks* — slower by the block size and
    /// different on every host.
    ///
    /// These two are smoothed even though the engine uses them per block: the
    /// smoothed value is where a drag lives, and the raw one would step.
    pub fn shape(&self, samples: usize) -> Shape {
        Shape {
            drive: self.drive.read_after(samples),
            texture: self.texture.read_after(samples),
            // **Order matters**: these arrays are filled by position, and
            // `the_band_order_matches_the_engine` is what holds it in place.
            texture_offsets: [
                self.texture_body.read_after(samples),
                self.texture_presence.read_after(samples),
                self.texture_air.read_after(samples),
            ],
            bias: [
                self.bias_body.read_after(samples),
                self.bias_presence.read_after(samples),
                self.bias_air.read_after(samples),
            ],
            solo: [
                self.solo_body.value(),
                self.solo_presence.value(),
                self.solo_air.value(),
            ],
            guards: [
                self.guard_harsh.read_after(samples),
                self.guard_sib.read_after(samples),
            ],
            emotion: self.emotion.read_after(samples),
            density: self.density.read_after(samples),
            focus: self.focus.read_after(samples),
            mode: self.mode.value().into(),
            factor: self.oversample.value().into(),
        }
    }

    /// The same values, **without touching a smoother** — for the interface.
    ///
    /// `shape` advances every smoother it reads, so calling it from the editor
    /// would steal a step from the audio thread and make a drag ramp at double
    /// speed. This reads the settled values instead, which is what a picture
    /// should show anyway: the curve you have set, not the one the ramp is
    /// half way through.
    ///
    /// `EMOTION` is left at zero here. The window exists to show what `TEXTURE`
    /// and the biases did to the curve (`ui.md`), and a curve breathing with the
    /// envelope thirty times a second is harder to read, not easier.
    pub fn display_shape(&self) -> Shape {
        Shape {
            drive: self.drive.value(),
            texture: self.texture.value(),
            texture_offsets: [
                self.texture_body.value(),
                self.texture_presence.value(),
                self.texture_air.value(),
            ],
            bias: [
                self.bias_body.value(),
                self.bias_presence.value(),
                self.bias_air.value(),
            ],
            solo: [
                self.solo_body.value(),
                self.solo_presence.value(),
                self.solo_air.value(),
            ],
            guards: [self.guard_harsh.value(), self.guard_sib.value()],
            emotion: 0.0,
            density: self.density.value(),
            focus: self.focus.value(),
            mode: self.mode.value().into(),
            factor: self.oversample.value().into(),
        }
    }

    /// Band `index`'s parameter ids, as `[level, texture, bias, solo]`. For
    /// the editor, which addresses the three bands by number.
    pub fn band_ids(&self, index: usize) -> [u32; 4] {
        match index {
            0 => [
                self.body.id(),
                self.texture_body.id(),
                self.bias_body.id(),
                self.solo_body.id(),
            ],
            1 => [
                self.presence.id(),
                self.texture_presence.id(),
                self.bias_presence.id(),
                self.solo_presence.id(),
            ],
            _ => [
                self.air.id(),
                self.texture_air.id(),
                self.bias_air.id(),
                self.solo_air.id(),
            ],
        }
    }

    /// What the engine needs per sample.
    pub fn levels(&self) -> Levels {
        Levels {
            bands: [self.body.read(), self.presence.read(), self.air.read()],
            mix: self.mix.read(),
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
        assert_eq!(db_to_linear(0.0f32), 1.0);
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
            ("drive", params.drive.value()),
            ("body", params.body.value()),
            ("presence", params.presence.value()),
            ("air", params.air.value()),
            ("texture", params.texture.value()),
            ("mix", params.mix.value()),
            ("harsh guard", params.guard_harsh.value()),
            ("sib guard", params.guard_sib.value()),
        ] {
            assert!((0.0..=1.0).contains(&value), "{name} defaults to {value}");
        }
        assert_eq!(params.focus.value(), 0.0);
        assert_eq!(params.output.value(), 0.0);

        // The bipolar controls rest at the middle, and nothing is soloed.
        for (name, value) in [
            ("body texture", params.texture_body.value()),
            ("presence texture", params.texture_presence.value()),
            ("air texture", params.texture_air.value()),
            ("body bias", params.bias_body.value()),
            ("presence bias", params.bias_presence.value()),
            ("air bias", params.bias_air.value()),
        ] {
            assert_eq!(value, 0.0, "{name} defaults to {value}");
        }
        assert!(!params.solo_body.value());
        assert!(!params.solo_presence.value());
        assert!(!params.solo_air.value());
    }

    /// **Every parameter reads back its own text** — a typed value in the
    /// window, and clap-validator's `param-conversions`.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        use truce::params::Params;
        let params = VelourParams::default();
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
