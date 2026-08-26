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
use nxe_audio::oversample::Factor;
use velour_core::engine::{Levels, Shape};

/// How hard the generator bus runs internally.
///
/// A separate type from `nxe_audio::Factor` on purpose: deriving nih-plug's
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

    /// How hard the generator bus's input is compressed
    /// (`velour_core::density`).
    ///
    /// **A main knob, not an Advanced one.** It is safe to push — the dry path
    /// is not in the compressor — so it is the control that decides how much
    /// the texture ignores the performance, and that is a mixing decision
    /// (`REQ-VEL-007`).
    #[id = "density"]
    pub density: FloatParam,

    #[id = "focus"]
    pub focus: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "output"]
    pub output: FloatParam,
    #[id = "os"]
    pub oversample: EnumParam<FactorParam>,

    /// Per-band deviation from `TEXTURE`, and the drive-versus-level split
    /// (`REQ-VEL-010`). Bipolar, resting at zero.
    ///
    /// **Three flat fields per control rather than a nested array.** A nested
    /// array would name them "Band 1 Bias" in a host's automation list; the
    /// bands have names, and the list is where a user looks for them.
    #[id = "tex_body"]
    pub texture_body: FloatParam,
    #[id = "tex_pres"]
    pub texture_presence: FloatParam,
    #[id = "tex_air"]
    pub texture_air: FloatParam,

    #[id = "bias_body"]
    pub bias_body: FloatParam,
    #[id = "bias_pres"]
    pub bias_presence: FloatParam,
    #[id = "bias_air"]
    pub bias_air: FloatParam,

    /// How far each guard may pull its generator back (`REQ-VEL-006`).
    ///
    /// **Exposed rather than hidden.** Fully automatic protection means a user
    /// who turns AIR up and hears nothing has no way to find out why. Zero is
    /// exactly off.
    #[id = "guard_harsh"]
    pub guard_harsh: FloatParam,
    #[id = "guard_sib"]
    pub guard_sib: FloatParam,

    /// How much the input's envelope moves the curves
    /// (`velour_core::emotion`).
    ///
    /// **One knob, and it has to be reachable.** This is the plugin's
    /// differentiation, and nobody can decide whether it is an improvement
    /// without being able to switch it off and A/B it — so it sits in Advanced
    /// rather than being a hidden behaviour, and zero is exactly static
    /// (`REQ-VEL-008`).
    #[id = "emotion"]
    pub emotion: FloatParam,

    /// Listen to one generator alone, with the dry muted.
    ///
    /// **A parameter, reluctantly.** It changes the sound, so it is not the kind
    /// of interface state the Doubler keeps in a `#[persist]` flag — and it is
    /// the only way to reach it before the interface exists (`VEL-14`). Marked
    /// non-automatable, because automating a listen button is nothing anyone
    /// means to do.
    ///
    /// It is still *saved*, which nih-plug gives no way out of. A project
    /// reopening with a solo latched sounds broken, so `VEL-14` has to make the
    /// state obvious on screen.
    #[id = "solo_body"]
    pub solo_body: BoolParam,
    #[id = "solo_pres"]
    pub solo_presence: BoolParam,
    #[id = "solo_air"]
    pub solo_air: BoolParam,
}

impl Default for VelourParams {
    fn default() -> Self {
        Self {
            // **Settled by ear on real material** (`VEL-17`). The plugin has no
            // presets (`REQ-VEL-020`), so these *are* the product's face: what
            // it sounds like the moment it is dropped on a vocal.
            //
            // The set leans further in than the placeholders did — drive 0.40 →
            // 0.80, mix 0.50 → 0.80 — because a saturator that does almost
            // nothing out of the box reads as broken rather than as tasteful,
            // and everything here is safe to push: the dry path is untouched
            // (`REQ-VEL-001`) and the guards are on (`REQ-VEL-006`).
            drive: percentage("Drive", 0.80),
            body: percentage("Body", 0.40),
            presence: percentage("Presence", 0.60),
            air: percentage("Air", 0.40),

            // Clear, the middle of the axis.
            texture: percentage("Texture", 0.50),

            // Half way, not off. **This is the ear reversing an argument made
            // from the structure**: off was chosen because "a compressor nobody
            // asked for" is what this plugin is trying not to be, and that
            // reasoning was sound and wrong — `DENSITY` is not on the voice, it
            // is on the texture being added, so a resting amount of it makes the
            // texture even rather than making the performance flat
            // (`REQ-VEL-007`).
            density: percentage("Density", 0.50),

            focus: FloatParam::new(
                "Focus",
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" oct")
            // Moving `FOCUS` rebuilds twelve sets of filter coefficients, so
            // the smoothing is what keeps a drag from stepping. The edges
            // themselves are moved once per block (`velour_core::Engine`).
            .with_smoother(SmoothingStyle::Linear(50.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            mix: percentage("Mix", 0.80),

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
            // aliasing about 14 dB higher (`nxe_audio::oversample`).
            oversample: EnumParam::new("Oversample", FactorParam::Four),

            texture_body: bipolar("Body Texture"),
            texture_presence: bipolar("Presence Texture"),
            texture_air: bipolar("Air Texture"),

            bias_body: bipolar("Body Bias"),
            bias_presence: bipolar("Presence Bias"),
            bias_air: bipolar("Air Bias"),

            // On, and most of the way up: the plugin promises not to get
            // painful, and a protection nobody switched on does not keep it.
            guard_harsh: percentage("Harsh Guard", 0.75),
            guard_sib: percentage("Sib Guard", 0.75),

            // Not zero, for the same reason the guards are not: a default of
            // off means the plugin ships sounding like every other saturator
            // (`REQ-VEL-008`).
            emotion: percentage("Emotion", 0.50),

            solo_body: listen("Body Solo"),
            solo_presence: listen("Presence Solo"),
            solo_air: listen("Air Solo"),
        }
    }
}

/// A `-1..=1` control resting at zero. Six of these, so the shape lives in one
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

/// A listen switch: on/off, and not something to automate.
fn listen(name: &'static str) -> BoolParam {
    BoolParam::new(name, false).non_automatable()
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
            // **Order matters**: these arrays are filled by position, and
            // `the_band_order_matches_the_engine` is what holds it in place.
            texture_offsets: [
                self.texture_body.smoothed.next_step(samples),
                self.texture_presence.smoothed.next_step(samples),
                self.texture_air.smoothed.next_step(samples),
            ],
            bias: [
                self.bias_body.smoothed.next_step(samples),
                self.bias_presence.smoothed.next_step(samples),
                self.bias_air.smoothed.next_step(samples),
            ],
            solo: [
                self.solo_body.value(),
                self.solo_presence.value(),
                self.solo_air.value(),
            ],
            guards: [
                self.guard_harsh.smoothed.next_step(samples),
                self.guard_sib.smoothed.next_step(samples),
            ],
            emotion: self.emotion.smoothed.next_step(samples),
            density: self.density.smoothed.next_step(samples),
            focus: self.focus.smoothed.next_step(samples),
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
            ("harsh guard", params.guard_harsh.default_plain_value()),
            ("sib guard", params.guard_sib.default_plain_value()),
        ] {
            assert!((0.0..=1.0).contains(&value), "{name} defaults to {value}");
        }
        assert_eq!(params.focus.default_plain_value(), 0.0);
        assert_eq!(params.output.default_plain_value(), 0.0);

        // The bipolar controls rest at the middle, and nothing is soloed.
        for (name, value) in [
            ("body texture", params.texture_body.default_plain_value()),
            (
                "presence texture",
                params.texture_presence.default_plain_value(),
            ),
            ("air texture", params.texture_air.default_plain_value()),
            ("body bias", params.bias_body.default_plain_value()),
            ("presence bias", params.bias_presence.default_plain_value()),
            ("air bias", params.bias_air.default_plain_value()),
        ] {
            assert_eq!(value, 0.0, "{name} defaults to {value}");
        }
        assert!(!params.solo_body.default_plain_value());
        assert!(!params.solo_presence.default_plain_value());
        assert!(!params.solo_air.default_plain_value());
    }
}
