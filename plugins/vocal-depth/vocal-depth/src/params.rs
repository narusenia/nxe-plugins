//! Parameter declarations, and the mapping to `vocal_depth_core`'s plain
//! values.
//!
//! The adapter the architecture calls for: `vocal-depth-core` knows nothing
//! about nih-plug, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! **Adding a parameter later is safe** — nih-plug keys them by id, not by
//! position — but **changing or removing an id is not**, so the ids below are
//! as final as `CLAP_ID`.
//!
//! ## Only what is wired
//!
//! `REQ-VDP-009`'s seven main controls are all here, plus the output trim.
//! Nothing is declared that does not work: **a plugin must not describe a
//! planned feature as a working one** (`.agents/rules/documentation.md` says the
//! same about documents), and adding an id later is safe because ids are keys.

use nih_plug::prelude::*;
use vocal_depth_core::Macros;

/// The output trim's range. Zero is the default and **exactly unity**, which is
/// what the `MIX` = 0 bit-identity rides on (`REQ-VDP-001`).
const OUTPUT_MIN_DB: f32 = -24.0;
const OUTPUT_MAX_DB: f32 = 12.0;

/// How long a macro takes to travel when a host jumps it. The core smooths
/// again on its own — this one is what keeps automation from arriving in
/// block-sized steps.
const SMOOTHING_MS: f32 = 20.0;

#[derive(Params)]
pub struct VocalDepthParams {
    /// Close ↔ far. **One control, seven consequences** (`REQ-VDP-002`), and
    /// the reason the product is not a reverb with a wet knob.
    #[id = "depth"]
    pub depth: FloatParam,

    /// How near the direct sound is, on top of `DEPTH`. Rests at the middle,
    /// where it contributes nothing — that is how two controls avoid writing
    /// one value (`REQ-VDP-009`).
    #[id = "direct"]
    pub direct: FloatParam,

    /// How much early reflection there is. **Exactly none at zero**
    /// (`REQ-VDP-003`), independent of `DEPTH`: a small room can be far away
    /// and a large one can be close.
    #[id = "room"]
    pub room: FloatParam,

    /// How much high-frequency loss the distance brings. **Exactly none at
    /// zero** (`REQ-VDP-005`), and it lands on the direct sound and the
    /// reflections in **different amounts** — the same filter on both is
    /// muffled rather than distant.
    #[id = "damping"]
    pub damping: FloatParam,

    /// How far the space spreads. **The reflections only** — the voice stays
    /// centred, and a mono sum leaves it untouched (`REQ-VDP-007`).
    #[id = "width"]
    pub width: FloatParam,

    /// How much intelligibility to put back when the voice is far away.
    /// **Exactly nothing at zero, and nothing at all when the voice is close**
    /// (`REQ-VDP-006`). How far it is working is on screen — a protection that
    /// works invisibly is a control that does nothing.
    ///
    /// **Rests at zero.** What it does is measurable and bounded, but it is a
    /// restoration, so the plugin does not perform one uninvited.
    #[id = "clarity"]
    pub clarity: FloatParam,

    /// Dry ↔ wet. **A crossfade, not an addition** — getting further away is
    /// taking presence off the voice, and leaving the untouched original in
    /// would put it straight back (`vocal_depth_core::engine`).
    #[id = "mix"]
    pub mix: FloatParam,

    /// The final trim. **Not part of the distance** — `DEPTH` is normalised on
    /// its own (`REQ-VDP-008`), so this is only here for the gain staging a mix
    /// needs.
    #[id = "output"]
    pub output: FloatParam,
}

impl Default for VocalDepthParams {
    fn default() -> Self {
        Self {
            depth: unit("Depth", 0.5),
            direct: unit("Direct", 0.5),
            room: unit("Room", 0.5),
            damping: unit("Damping", 0.5),
            width: unit("Width", 0.6),
            clarity: unit("Clarity", 0.0),
            mix: unit("Mix", 1.0),
            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: OUTPUT_MIN_DB,
                    max: OUTPUT_MAX_DB,
                },
            )
            .with_unit(" dB")
            .with_step_size(0.1)
            .with_smoother(SmoothingStyle::Linear(SMOOTHING_MS)),
        }
    }
}

impl VocalDepthParams {
    /// What the core is asked for, advanced by one block.
    ///
    /// **`next_step(samples)` rather than one `next()` per sample**, which is
    /// the shape Air settled on: the engine resolves its filters and its
    /// normalisation once per block, so the parameters have to arrive at block
    /// rate too — and stepping by the block length keeps the travel per second
    /// the same whatever the host's buffer is (`.agents/rules/rust.md` on block
    /// size).
    pub fn macros(&self, samples: u32) -> Macros {
        Macros {
            depth: self.depth.smoothed.next_step(samples),
            direct: self.direct.smoothed.next_step(samples),
            room: self.room.smoothed.next_step(samples),
            damping: self.damping.smoothed.next_step(samples),
            width: self.width.smoothed.next_step(samples),
            clarity: self.clarity.smoothed.next_step(samples),
            mix: self.mix.smoothed.next_step(samples),
            // **0 dB has to come out exactly 1.0** or the bit-identity at
            // `MIX` = 0 is off by a rounding error (`REQ-VDP-001`).
            output: util::db_to_gain(self.output.smoothed.next_step(samples)),
        }
    }
}

/// A plain `0..=1` macro with a percentage readout.
fn unit(name: &str, default: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit(" %")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
        .with_smoother(SmoothingStyle::Linear(SMOOTHING_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A parameter id is as final as `CLAP_ID`: a host stores it in the project
    /// file. This is here so that renaming one is a failing test rather than a
    /// silent loss of every saved setting.
    #[test]
    fn the_parameter_ids_are_what_was_shipped() {
        let params = VocalDepthParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(
            ids,
            [
                "depth", "direct", "room", "damping", "width", "clarity", "mix", "output"
            ]
        );
    }

    /// **0 dB is exactly unity.** The transparency promise is a bit comparison,
    /// so a trim that resolved to 0.999999 would break it (`REQ-VDP-001`).
    #[test]
    fn the_default_output_trim_is_exactly_unity() {
        assert_eq!(util::db_to_gain(0.0), 1.0);
        let params = VocalDepthParams::default();
        assert_eq!(params.output.default_plain_value(), 0.0);
    }
}
