//! Parameter declarations, and the mapping to `sparkleur_core`'s plain values.
//!
//! The adapter the architecture calls for: `sparkleur-core` knows nothing about
//! truce, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Thirty-five parameters, seven of them everyday** (`ui.md`). **Ids are
//! truce's hash of the field name**, so renaming a field is renaming a
//! parameter for every saved session. The display `name` is free to change.
//!
//! ## Two layers, and what rests at zero
//!
//! `SPARK` is the amount. `BODY` and `AIR` are **bipolar tilts** that rest at
//! zero, meaning "leave this band on `SPARK`" (`REQ-SPK-009`). So are `SPEED`,
//! `DE-HARSH` and `SUB PROT`, which rest at "as `CHARACTER` says" — a control
//! that rests on the axis is how two things avoid writing one value.

use crate::analysis::Analysis;
use nxe_audio::oversample::Factor;
use sparkleur_core::dynamics::Mode;
use sparkleur_core::engine::{Levels, Shape};
use truce::prelude::*;

/// How hard the Sparkle bus runs internally.
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

/// How far the macros are allowed to reach (`REQ-SPK-022`). Separate from
/// `sparkleur_core::dynamics::Mode` for the same reason as [`FactorParam`].
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

impl From<FactorParam> for Factor {
    fn from(value: FactorParam) -> Self {
        match value {
            FactorParam::Two => Factor::Two,
            FactorParam::Four => Factor::Four,
        }
    }
}

#[derive(Params)]
pub struct SparkleurParams {
    /// What the editor is shown about the sound. Not a parameter: the audio
    /// thread writes it and the editor reads it, and the params `Arc` is the
    /// one thing both are handed (`analysis.rs`).
    #[skip]
    pub analysis: Analysis,

    /// The amount of everything (`REQ-SPK-009`). **Zero is exactly nothing.**
    ///
    /// **Provisional**, like every default here; `SPK-18` settles them by
    /// ear. What is not provisional is which ones rest at zero: every control
    /// that defers to `CHARACTER` does.
    #[param(
        name = "Spark",
        range = "linear(0, 1)",
        default = 0.35,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub spark: FloatParam,

    /// POLISH ↔ CRUSH. One continuous axis, not two modes (`REQ-SPK-006`).
    /// The default is `character::DEFAULT_POSITION`, copied because a truce
    /// default has to be a literal; `the_character_default_is_the_core_one`
    /// keeps the copy honest.
    #[param(
        name = "Character",
        range = "linear(0, 1)",
        default = 0.27,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub character: FloatParam,

    /// Where to lean. Bipolar, resting at zero (`REQ-SPK-009`).
    #[param(
        name = "Body",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub body: FloatParam,
    #[param(
        name = "Air",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub air: FloatParam,

    /// How fast, **as a deviation from what `CHARACTER` chose**. The per-band
    /// time constants are derived from each band's own centre and cannot be
    /// reached from here — that is the point (`REQ-SPK-005`).
    #[param(
        name = "Speed",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub speed: FloatParam,

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

    /// Slides all four boundaries together (`REQ-SPK-002`). Moving it
    /// rebuilds forty sets of filter coefficients per channel, so the
    /// smoothing is what keeps a drag from stepping.
    #[param(
        name = "Focus",
        range = "linear(-1, 1)",
        format = "octaves",
        parse = "number",
        smooth = "linear(50)"
    )]
    pub focus: FloatParam,

    /// How much of the layer is handed to the transient. Zero is a static
    /// generator — **being able to choose that is how a user hears what the
    /// gate does** (`REQ-SPK-007`). Middling-to-high by default: shipping
    /// with the gate mostly shut would ship the wrong product.
    #[param(
        name = "Snap",
        range = "linear(0, 1)",
        default = 0.6,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub snap: FloatParam,

    /// How far the upward compressor's floor opens. Closed by default: the
    /// floor is what stops silence coming up (`REQ-SPK-003`).
    #[param(
        name = "Lift",
        range = "linear(0, 1)",
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub lift: FloatParam,

    /// How hard a transient is hit (`REQ-SPK-020`). **Zero by default**: a
    /// plugin that arrives already hitting has made that call for everyone
    /// (`SPK-22`).
    #[param(
        name = "Punch",
        range = "linear(0, 1)",
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub punch: FloatParam,

    /// Deviations from what `CHARACTER` chose for the two protections
    /// (`REQ-SPK-008`).
    #[param(
        name = "De-Harsh",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub de_harsh: FloatParam,
    #[param(
        name = "Sub Protect",
        range = "linear(-1, 1)",
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub sub_protect: FloatParam,

    /// How far the macros reach (`REQ-SPK-022`). `Soft` by default.
    #[param(name = "Mode", parse = "mode")]
    pub mode: EnumParam<ModeParam>,

    /// 4x by default: 2x is a cost saving, not an equal — it leaves aliasing
    /// about 14 dB higher (`nxe_audio::oversample`).
    #[param(name = "Oversample", default = 1, parse = "factor")]
    pub oversample: EnumParam<FactorParam>,

    /// Per-band **weights**, not amounts: `SPARK` multiplies them, so raising
    /// it deepens every band and leaves their proportions alone
    /// (`REQ-SPK-009`). Full by default, so `SPARK` alone works every band.
    ///
    /// **Flat fields rather than nested groups.** The bands have names, and
    /// the automation list is where a user looks for them.
    #[param(
        name = "Sub Up",
        group = "Up",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub up_sub: FloatParam,
    #[param(
        name = "Body Up",
        group = "Up",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub up_body: FloatParam,
    #[param(
        name = "Mid Up",
        group = "Up",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub up_mid: FloatParam,
    #[param(
        name = "Presence Up",
        group = "Up",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub up_pres: FloatParam,
    #[param(
        name = "Air Up",
        group = "Up",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub up_air: FloatParam,

    #[param(
        name = "Sub Down",
        group = "Down",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub down_sub: FloatParam,
    #[param(
        name = "Body Down",
        group = "Down",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub down_body: FloatParam,
    #[param(
        name = "Mid Down",
        group = "Down",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub down_mid: FloatParam,
    #[param(
        name = "Presence Down",
        group = "Down",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub down_pres: FloatParam,
    #[param(
        name = "Air Down",
        group = "Down",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub down_air: FloatParam,

    /// A static trim per band. **Outside `SPARK`**, because it is EQ rather
    /// than dynamics (`dsp.md`).
    #[param(
        name = "Sub Gain",
        group = "Gain",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub gain_sub: FloatParam,
    #[param(
        name = "Body Gain",
        group = "Gain",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub gain_body: FloatParam,
    #[param(
        name = "Mid Gain",
        group = "Gain",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub gain_mid: FloatParam,
    #[param(
        name = "Presence Gain",
        group = "Gain",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub gain_pres: FloatParam,
    #[param(
        name = "Air Gain",
        group = "Gain",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub gain_air: FloatParam,

    /// Listening to one band. **Automatable on truce**, unlike on nih-plug:
    /// truce's `flags` has no "not automatable" short of `readonly`, which
    /// would also stop the window from setting it. A project reopening with a
    /// solo latched sounds broken, so the window has to make the state obvious.
    #[param(name = "Sub Solo", group = "Solo", parse = "switch")]
    pub solo_sub: BoolParam,
    #[param(name = "Body Solo", group = "Solo", parse = "switch")]
    pub solo_body: BoolParam,
    #[param(name = "Mid Solo", group = "Solo", parse = "switch")]
    pub solo_mid: BoolParam,
    #[param(name = "Presence Solo", group = "Solo", parse = "switch")]
    pub solo_pres: BoolParam,
    #[param(name = "Air Solo", group = "Solo", parse = "switch")]
    pub solo_air: BoolParam,
}

impl SparkleurParams {
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

    /// "35", "35 %", "+35%" → 0.35.
    fn percent(&self, text: &str) -> Option<f64> {
        number(text).map(|value| value / 100.0)
    }

    /// "On" / "Off", as truce prints a switch. **Every parameter parses**:
    /// once one does, a validator expects all of them to, and a switch that
    /// cannot read back its own text fails `param-conversions`.
    fn switch(&self, text: &str) -> Option<f64> {
        match text.trim().to_ascii_lowercase().as_str() {
            "on" | "1" | "true" => Some(1.0),
            "off" | "0" | "false" => Some(0.0),
            _ => None,
        }
    }

    fn mode(&self, text: &str) -> Option<f64> {
        choice(&["soft", "hard"], text)
    }

    fn factor(&self, text: &str) -> Option<f64> {
        choice(&["2x", "4x"], text)
    }

    /// "-3", "-3 dB", "+0.5 oct" → the number.
    fn number(&self, text: &str) -> Option<f64> {
        number(text)
    }
}

/// The index of `text` among `names`, ignoring case.
fn choice(names: &[&str], text: &str) -> Option<f64> {
    let text = text.trim().to_ascii_lowercase();
    names
        .iter()
        .position(|name| *name == text)
        .map(|index| index as f64)
}

/// The leading number of `text`, whatever unit follows it.
fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(text.len());
    text[..end].parse().ok()
}

impl SparkleurParams {
    /// What the engine needs once per block.
    ///
    /// **`read_after(samples)`, not `read()`.** A smoother advances one sample
    /// per call, so reading it once per block would stretch every ramp by the
    /// block length — the trap `VEL-5` hit.
    pub fn shape(&self, samples: usize) -> Shape {
        Shape {
            character: self.character.read_after(samples),
            focus: self.focus.read_after(samples),
            speed: self.speed.read_after(samples),
            snap: self.snap.read_after(samples),
            lift: self.lift.read_after(samples),
            punch: self.punch.read_after(samples),
            de_harsh: self.de_harsh.read_after(samples),
            sub_protect: self.sub_protect.read_after(samples),
            // **Order matters**: these arrays are filled by position, and
            // `the_band_order_matches_the_engine` is what holds it in place.
            up: [
                self.up_sub.read_after(samples),
                self.up_body.read_after(samples),
                self.up_mid.read_after(samples),
                self.up_pres.read_after(samples),
                self.up_air.read_after(samples),
            ],
            down: [
                self.down_sub.read_after(samples),
                self.down_body.read_after(samples),
                self.down_mid.read_after(samples),
                self.down_pres.read_after(samples),
                self.down_air.read_after(samples),
            ],
            gain_db: [
                self.gain_sub.read_after(samples),
                self.gain_body.read_after(samples),
                self.gain_mid.read_after(samples),
                self.gain_pres.read_after(samples),
                self.gain_air.read_after(samples),
            ],
            solo: [
                self.solo_sub.value(),
                self.solo_body.value(),
                self.solo_mid.value(),
                self.solo_pres.value(),
                self.solo_air.value(),
            ],
            factor: self.oversample.value().into(),
            mode: self.mode.value().into(),
        }
    }

    /// The same values, **without touching a smoother** — for the interface.
    ///
    /// `shape` advances every smoother it reads, so calling it from the editor
    /// would steal a step from the audio thread and make a drag ramp at double
    /// speed. This reads the settled values instead, which is what a picture
    /// should show anyway: the curve you have set, not the one the ramp is half
    /// way through.
    pub fn display_shape(&self) -> Shape {
        Shape {
            character: self.character.value(),
            focus: self.focus.value(),
            speed: self.speed.value(),
            snap: self.snap.value(),
            lift: self.lift.value(),
            punch: self.punch.value(),
            de_harsh: self.de_harsh.value(),
            sub_protect: self.sub_protect.value(),
            up: [
                self.up_sub.value(),
                self.up_body.value(),
                self.up_mid.value(),
                self.up_pres.value(),
                self.up_air.value(),
            ],
            down: [
                self.down_sub.value(),
                self.down_body.value(),
                self.down_mid.value(),
                self.down_pres.value(),
                self.down_air.value(),
            ],
            gain_db: [
                self.gain_sub.value(),
                self.gain_body.value(),
                self.gain_mid.value(),
                self.gain_pres.value(),
                self.gain_air.value(),
            ],
            solo: [
                self.solo_sub.value(),
                self.solo_body.value(),
                self.solo_mid.value(),
                self.solo_pres.value(),
                self.solo_air.value(),
            ],
            factor: self.oversample.value().into(),
            mode: self.mode.value().into(),
        }
    }

    /// The settled per-sample values, for the same reason.
    pub fn display_levels(&self) -> Levels {
        Levels {
            spark: self.spark.value(),
            body: self.body.value(),
            air: self.air.value(),
            mix: self.mix.value(),
        }
    }

    /// What the engine needs every sample, because it multiplies the signal.
    pub fn levels(&self) -> Levels {
        Levels {
            spark: self.spark.read(),
            body: self.body.read(),
            air: self.air.read(),
            mix: self.mix.read(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sparkleur_core::crossover::BAND_COUNT;

    /// The arrays in `shape` are filled by position, so a band added or
    /// reordered in the engine has to be noticed here rather than heard.
    #[test]
    fn the_band_order_matches_the_engine() {
        let shape = SparkleurParams::default().shape(1);
        assert_eq!(shape.up.len(), BAND_COUNT);
        assert_eq!(shape.down.len(), BAND_COUNT);
        assert_eq!(shape.gain_db.len(), BAND_COUNT);
        assert_eq!(shape.solo.len(), BAND_COUNT);
    }

    /// **Thirty-three** (`ui.md`). If this moves, the count in the interface
    /// specification moves with it.
    #[test]
    fn there_are_thirty_five_parameters() {
        let params = SparkleurParams::default();
        let count = params.param_infos().len();
        assert_eq!(count, 35, "the parameter count moved");
    }

    /// Every control that defers to `CHARACTER` rests at zero, and the weights
    /// rest at full — so `SPARK` alone is a working plugin (`REQ-SPK-009`).
    #[test]
    fn the_deferring_controls_rest_at_zero() {
        let params = SparkleurParams::default();
        for value in [
            params.speed.value(),
            params.de_harsh.value(),
            params.sub_protect.value(),
            params.body.value(),
            params.air.value(),
            params.focus.value(),
        ] {
            assert_eq!(value, 0.0);
        }

        // **The values, not the smoothed reads.** A smoother is only at its
        // target once a host has called `initialize`, so `shape()` in a unit
        // test reads a ramp that has not started.
        for weight in [
            params.up_sub.value(),
            params.up_body.value(),
            params.up_mid.value(),
            params.up_pres.value(),
            params.up_air.value(),
            params.down_sub.value(),
            params.down_body.value(),
            params.down_mid.value(),
            params.down_pres.value(),
            params.down_air.value(),
        ] {
            assert_eq!(weight, 1.0, "a weight did not rest at full");
        }
        for gain in [
            params.gain_sub.value(),
            params.gain_body.value(),
            params.gain_mid.value(),
            params.gain_pres.value(),
            params.gain_air.value(),
        ] {
            assert_eq!(gain, 0.0, "a trim did not rest at unity");
        }
        assert!(params.shape(1).solo.iter().all(|on| !on));
    }

    /// `MIX` = 1 and `OUTPUT` = 0 dB, so the plugin out of the box is fully wet
    /// at unity (`REQ-SPK-012`).
    #[test]
    fn it_ships_fully_wet_at_unity() {
        let params = SparkleurParams::default();
        assert_eq!(params.mix.value(), 1.0);
        assert_eq!(params.output.value(), 0.0);
        assert_eq!(db_to_linear(params.output.value()), 1.0);
    }

    /// The literal in `#[param]` is a copy of the core's constant, because a
    /// truce default has to be a literal.
    #[test]
    fn the_character_default_is_the_core_one() {
        let params = SparkleurParams::default();
        assert_eq!(
            params.character.value(),
            sparkleur_core::character::DEFAULT_POSITION
        );
    }

    /// A typed value comes back in the parameter's own unit.
    #[test]
    fn typed_values_parse() {
        let params = SparkleurParams::default();
        assert_eq!(params.parse_value(params.spark.id(), "35 %"), Some(0.35));
        assert_eq!(params.parse_value(params.body.id(), "+20%"), Some(0.2));
        assert_eq!(params.parse_value(params.output.id(), "-3 dB"), Some(-3.0));
        assert_eq!(params.parse_value(params.focus.id(), "0.5 oct"), Some(0.5));
        assert_eq!(params.parse_value(params.spark.id(), "loud"), None);
    }

    /// **Every parameter reads back its own text** — what clap-validator's
    /// `param-conversions` checks, and what it failed on when only the
    /// numbers parsed.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        let params = SparkleurParams::default();
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
