//! Parameter declarations, and the mapping to `diorama_core`'s plain
//! values.
//!
//! The adapter the architecture calls for: `diorama-core` knows nothing
//! about truce, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Ids are truce's hash of the field name**, so renaming a field is renaming a
//! parameter for every saved session. The display `name` is free to change.
//!
//! ## Only what is wired
//!
//! `REQ-DIO-009`'s seven main controls are all here, plus the output trim.
//! Nothing is declared that does not work: **a plugin must not describe a
//! planned feature as a working one** (`.agents/rules/documentation.md` says the
//! same about documents), and adding an id later is safe because ids are keys.

use crate::analysis::Analysis;
use diorama_core::Macros;
use nxe_egui::text;
use truce::prelude::*;

#[derive(Params)]
pub struct DioramaParams {
    /// What the editor is shown about the sound. Not a parameter: the audio
    /// thread writes it and the editor reads it, and the params `Arc` is the
    /// one thing both are handed (`analysis.rs`).
    #[skip]
    pub analysis: Analysis,

    /// Close ↔ far. **One control, seven consequences** (`REQ-DIO-002`), and
    /// the reason the product is not a reverb with a wet knob.
    #[param(
        name = "Depth",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub depth: FloatParam,

    /// How near the direct sound is, on top of `DEPTH`. Rests at the middle,
    /// where it contributes nothing — that is how two controls avoid writing
    /// one value (`REQ-DIO-009`).
    #[param(
        name = "Direct",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub direct: FloatParam,

    /// How much early reflection there is. **Exactly none at zero**
    /// (`REQ-DIO-003`), independent of `DEPTH`: a small room can be far away
    /// and a large one can be close.
    #[param(
        name = "Room",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub room: FloatParam,

    /// How much high-frequency loss the distance brings. **Exactly none at
    /// zero** (`REQ-DIO-005`), and it lands on the direct sound and the
    /// reflections in **different amounts** — the same filter on both is
    /// muffled rather than distant.
    #[param(
        name = "Damping",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub damping: FloatParam,

    /// How far the space spreads. **The reflections only** — the voice stays
    /// centred, and a mono sum leaves it untouched (`REQ-DIO-007`).
    #[param(
        name = "Width",
        range = "linear(0, 1)",
        default = 0.6,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub width: FloatParam,

    /// How much intelligibility to put back when the voice is far away.
    /// **Exactly nothing at zero, and nothing at all when the voice is close**
    /// (`REQ-DIO-006`). How far it is working is on screen — a protection that
    /// works invisibly is a control that does nothing.
    ///
    /// **Rests at zero.** What it does is measurable and bounded, but it is a
    /// restoration, so the plugin does not perform one uninvited.
    #[param(
        name = "Clarity",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub clarity: FloatParam,

    /// Dry ↔ wet. **A crossfade, not an addition** — getting further away is
    /// taking presence off the voice, and leaving the untouched original in
    /// would put it straight back (`diorama_core::engine`).
    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    /// The final trim. **Not part of the distance** — `DEPTH` is normalised on
    /// its own (`REQ-DIO-008`), so this is only here for the gain staging a mix
    /// needs.
    ///
    /// **0 dB has to come out exactly 1.0** or the bit-identity at `MIX` = 0 is
    /// off by a rounding error (`REQ-DIO-001`).
    #[param(
        name = "Output",
        range = "linear(-24, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub output: FloatParam,
}

impl DioramaParams {
    fn decibels(&self, value: f64) -> String {
        format!("{value:+.1} dB")
    }

    // **Every parameter reads its own text back** (`nxe_egui::text`).
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    /// What the engine needs once per block. **`read_after(samples)`, not
    /// `read()`**: reading a smoother once per block would stretch every ramp
    /// by the block length.
    pub fn macros(&self, samples: usize) -> Macros {
        Macros {
            depth: self.depth.read_after(samples),
            direct: self.direct.read_after(samples),
            room: self.room.read_after(samples),
            damping: self.damping.read_after(samples),
            width: self.width.read_after(samples),
            clarity: self.clarity.read_after(samples),
            mix: self.mix.read_after(samples),
            output: db_to_linear(self.output.read_after(samples)),
        }
    }

    /// The same, **without touching a smoother** — for the window.
    pub fn display_macros(&self) -> Macros {
        Macros {
            depth: self.depth.value(),
            direct: self.direct.value(),
            room: self.room.value(),
            damping: self.damping.value(),
            width: self.width.value(),
            clarity: self.clarity.value(),
            mix: self.mix.value(),
            output: db_to_linear(self.output.value()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_output_trim_is_exactly_unity() {
        assert_eq!(db_to_linear(0.0f32), 1.0);
        let params = DioramaParams::default();
        assert_eq!(params.output.value(), 0.0);
    }

    /// **Every parameter reads back its own text** — a typed value in the
    /// window, and clap-validator's `param-conversions`.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        let params = DioramaParams::default();
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
