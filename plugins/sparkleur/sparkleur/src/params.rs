//! Parameter declarations, and the mapping to `sparkleur_core`'s plain values.
//!
//! The adapter the architecture calls for: `sparkleur-core` knows nothing about
//! nih-plug, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Thirty-three parameters, seven of them everyday** (`ui.md`). Adding one
//! later is safe — nih-plug keys them by id, not by position — but **changing
//! or removing an id is not**, so the ids below are as final as `CLAP_ID`.
//!
//! ## Two layers, and what rests at zero
//!
//! `SPARK` is the amount. `BODY` and `AIR` are **bipolar tilts** that rest at
//! zero, meaning "leave this band on `SPARK`" (`REQ-SPK-009`). So are `SPEED`,
//! `DE-HARSH` and `SUB PROT`, which rest at "as `CHARACTER` says" — a control
//! that rests on the axis is how two things avoid writing one value
//! (`.agents/rules/vizia.md`).

use nih_plug::prelude::*;
use nxe_audio::oversample::Factor;
use sparkleur_core::character;
use sparkleur_core::dynamics::Mode;
use sparkleur_core::engine::{Levels, Shape};

/// How hard the Sparkle bus runs internally.
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

/// How far the macros are allowed to reach (`REQ-SPK-022`).
///
/// A separate type from `sparkleur_core::dynamics::Mode` for the same reason
/// [`FactorParam`] is: deriving nih-plug's `Enum` on the shared type would make
/// the core depend on nih-plug.
///
/// **The variants carry `#[id]`, and that is what makes this safe to extend.**
/// nih-plug writes an enum with ids into saved state as its id string rather
/// than as a number, so a third step can be added later without moving what an
/// existing session means.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ModeParam {
    #[id = "soft"]
    #[name = "Soft"]
    #[default]
    Soft,
    #[id = "hard"]
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
    /// The amount of everything (`REQ-SPK-009`). **Zero is exactly nothing.**
    #[id = "spark"]
    pub spark: FloatParam,

    /// POLISH ↔ CRUSH. One continuous axis, not two modes — "eight tenths of
    /// POLISH" is where most material sits (`REQ-SPK-006`).
    #[id = "character"]
    pub character: FloatParam,

    /// Where to lean. Bipolar, resting at zero (`REQ-SPK-009`).
    #[id = "body"]
    pub body: FloatParam,
    #[id = "air"]
    pub air: FloatParam,

    /// How fast, **as a deviation from what `CHARACTER` chose**. The per-band
    /// time constants are derived from each band's own centre and cannot be
    /// reached from here — that is the point (`REQ-SPK-005`).
    #[id = "speed"]
    pub speed: FloatParam,

    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "output"]
    pub output: FloatParam,

    /// Slides all four boundaries together (`REQ-SPK-002`).
    #[id = "focus"]
    pub focus: FloatParam,

    /// How much of the layer is handed to the transient. Zero is a static
    /// generator, which is what Velour is — **being able to choose that is how
    /// a user hears what the gate does** (`REQ-SPK-007`).
    #[id = "snap"]
    pub snap: FloatParam,

    /// How far the upward compressor's floor opens. Right is where OTT's
    /// breathing lives (`REQ-SPK-003`).
    #[id = "lift"]
    pub lift: FloatParam,

    /// Deviations from what `CHARACTER` chose for the two protections
    /// (`REQ-SPK-008`).
    #[id = "deharsh"]
    pub de_harsh: FloatParam,
    #[id = "subprot"]
    pub sub_protect: FloatParam,

    /// How far the macros reach (`REQ-SPK-022`).
    ///
    /// **The default is `Soft`, and it has to be.** A session saved before this
    /// parameter existed has no value for it, so it loads with the default —
    /// anything but `Soft` changes the sound of work that is already finished.
    #[id = "mode"]
    pub mode: EnumParam<ModeParam>,

    #[id = "os"]
    pub oversample: EnumParam<FactorParam>,

    /// Per-band **weights**, not amounts: `SPARK` multiplies them, so raising
    /// it deepens every band and leaves their proportions alone
    /// (`REQ-SPK-009`).
    ///
    /// **Twenty flat fields rather than four nested arrays.** A nested array
    /// would name them "Band 1 Up" in a host's automation list; the bands have
    /// names, and the list is where a user looks for them.
    #[id = "up_sub"]
    pub up_sub: FloatParam,
    #[id = "up_body"]
    pub up_body: FloatParam,
    #[id = "up_mid"]
    pub up_mid: FloatParam,
    #[id = "up_pres"]
    pub up_pres: FloatParam,
    #[id = "up_air"]
    pub up_air: FloatParam,

    #[id = "dn_sub"]
    pub down_sub: FloatParam,
    #[id = "dn_body"]
    pub down_body: FloatParam,
    #[id = "dn_mid"]
    pub down_mid: FloatParam,
    #[id = "dn_pres"]
    pub down_pres: FloatParam,
    #[id = "dn_air"]
    pub down_air: FloatParam,

    /// A static trim per band. **Outside `SPARK`**, because it is EQ rather
    /// than dynamics (`dsp.md`).
    #[id = "gain_sub"]
    pub gain_sub: FloatParam,
    #[id = "gain_body"]
    pub gain_body: FloatParam,
    #[id = "gain_mid"]
    pub gain_mid: FloatParam,
    #[id = "gain_pres"]
    pub gain_pres: FloatParam,
    #[id = "gain_air"]
    pub gain_air: FloatParam,

    /// Listening to one band. **Not automatable**, and nih-plug still saves it —
    /// a project reopening with a solo latched sounds broken, so `SPK-15` has to
    /// make the state obvious on screen (Velour hit the same wall).
    #[id = "solo_sub"]
    pub solo_sub: BoolParam,
    #[id = "solo_body"]
    pub solo_body: BoolParam,
    #[id = "solo_mid"]
    pub solo_mid: BoolParam,
    #[id = "solo_pres"]
    pub solo_pres: BoolParam,
    #[id = "solo_air"]
    pub solo_air: BoolParam,
}

impl Default for SparkleurParams {
    fn default() -> Self {
        Self {
            // **Provisional.** The plugin has no presets (`REQ-SPK-021`), so the
            // defaults are the product's face and `SPK-18` settles them by ear.
            // What is not provisional is which ones rest at zero: every control
            // that defers to `CHARACTER` does.
            spark: percentage("Spark", 0.35),
            character: percentage("Character", character::DEFAULT_POSITION),
            body: bipolar("Body"),
            air: bipolar("Air"),
            speed: bipolar("Speed"),

            mix: percentage("Mix", 1.0),
            output: decibels("Output"),

            focus: FloatParam::new(
                "Focus",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" oct")
            // Moving `FOCUS` rebuilds forty sets of filter coefficients per
            // channel, so the smoothing is what keeps a drag from stepping. The
            // edges themselves move once per block
            // (`sparkleur_core::Crossover`).
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            // Middling-to-high (`REQ-SPK-007`): the dynamic layer is what
            // separates this from a static exciter, so shipping with the gate
            // mostly shut would ship the wrong product.
            snap: percentage("Snap", 0.60),
            // Closed. The floor is the thing that stops silence coming up, and
            // opening it is a deliberate move toward OTT (`REQ-SPK-003`).
            lift: percentage("Lift", 0.0),

            de_harsh: bipolar("De-Harsh"),
            sub_protect: bipolar("Sub Protect"),

            mode: EnumParam::new("Mode", ModeParam::Soft),

            // 4x by default: 2x is a cost saving, not an equal — it leaves
            // aliasing about 14 dB higher (`nxe_audio::oversample`).
            oversample: EnumParam::new("Oversample", FactorParam::Four),

            up_sub: weight("Sub Up"),
            up_body: weight("Body Up"),
            up_mid: weight("Mid Up"),
            up_pres: weight("Presence Up"),
            up_air: weight("Air Up"),

            down_sub: weight("Sub Down"),
            down_body: weight("Body Down"),
            down_mid: weight("Mid Down"),
            down_pres: weight("Presence Down"),
            down_air: weight("Air Down"),

            gain_sub: decibels("Sub Gain"),
            gain_body: decibels("Body Gain"),
            gain_mid: decibels("Mid Gain"),
            gain_pres: decibels("Presence Gain"),
            gain_air: decibels("Air Gain"),

            solo_sub: listen("Sub Solo"),
            solo_body: listen("Body Solo"),
            solo_mid: listen("Mid Solo"),
            solo_pres: listen("Presence Solo"),
            solo_air: listen("Air Solo"),
        }
    }
}

/// A `-1..=1` control resting at zero. Five of these, so the shape lives in one
/// place and a deviation from it would be visible.
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

/// A per-band weight: full by default, so `SPARK` on its own works every band.
fn weight(name: &'static str) -> FloatParam {
    percentage(name, 1.0)
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

/// A listen switch: on/off, and not something to automate.
fn listen(name: &'static str) -> BoolParam {
    BoolParam::new(name, false).non_automatable()
}

impl SparkleurParams {
    /// What the engine needs once per block.
    ///
    /// **`next_step(samples)`, not `next()`.** A smoother advances one sample
    /// per call, so reading it once per block would stretch every ramp by the
    /// block length — the trap `VEL-5` hit.
    pub fn shape(&self, samples: u32) -> Shape {
        Shape {
            character: self.character.smoothed.next_step(samples),
            focus: self.focus.smoothed.next_step(samples),
            speed: self.speed.smoothed.next_step(samples),
            snap: self.snap.smoothed.next_step(samples),
            lift: self.lift.smoothed.next_step(samples),
            de_harsh: self.de_harsh.smoothed.next_step(samples),
            sub_protect: self.sub_protect.smoothed.next_step(samples),
            // **Order matters**: these arrays are filled by position, and
            // `the_band_order_matches_the_engine` is what holds it in place.
            up: [
                self.up_sub.smoothed.next_step(samples),
                self.up_body.smoothed.next_step(samples),
                self.up_mid.smoothed.next_step(samples),
                self.up_pres.smoothed.next_step(samples),
                self.up_air.smoothed.next_step(samples),
            ],
            down: [
                self.down_sub.smoothed.next_step(samples),
                self.down_body.smoothed.next_step(samples),
                self.down_mid.smoothed.next_step(samples),
                self.down_pres.smoothed.next_step(samples),
                self.down_air.smoothed.next_step(samples),
            ],
            gain_db: [
                self.gain_sub.smoothed.next_step(samples),
                self.gain_body.smoothed.next_step(samples),
                self.gain_mid.smoothed.next_step(samples),
                self.gain_pres.smoothed.next_step(samples),
                self.gain_air.smoothed.next_step(samples),
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
            spark: self.spark.smoothed.next(),
            body: self.body.smoothed.next(),
            air: self.air.smoothed.next(),
            mix: self.mix.smoothed.next(),
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
    fn there_are_thirty_four_parameters() {
        let params = SparkleurParams::default();
        let count = params.param_map().len();
        assert_eq!(count, 34, "the parameter count moved");
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
        assert_eq!(util::db_to_gain(params.output.value()), 1.0);
    }
}
