//! What the host can change.
//!
//! **The main five are `REQ-PUM-009`'s, and three of them are here** — `MIX`
//! and `OUTPUT` arrive with the dry path they mix against (`PUM-6`). The six
//! nodes are `PUM-5`. Declaring either now would put parameters in a host's
//! project file that do nothing, and a parameter id is as final as `CLAP_ID`.

use nih_plug::prelude::*;

/// How long a control takes to travel to a new value.
///
/// **Longer than a knob feels, on purpose.** These are resolved once per block
/// and reach the audio through a gain curve that is already smoothed in
/// frequency and followed in time, so the only thing this protects against is
/// a host jumping a parameter (`REQ-PUM-002` — no discontinuity when `DEPTH`
/// is swept).
const SMOOTHING_MS: f32 = 30.0;

/// How big the transform is (`REQ-PUM-008`).
///
/// A separate type from `pumice_core::Quality` on purpose: deriving nih-plug's
/// `Enum` on the shared type would make the core depend on nih-plug
/// (Sparkleur's `params.rs` says the same about `Factor`).
///
/// **The variants carry `#[id]`.** nih-plug writes an enum with ids into saved
/// state as its id string rather than as a number, so a fourth step could be
/// added later without moving what an existing session means.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum QualityParam {
    #[id = "low"]
    #[name = "Low"]
    Low,
    #[id = "normal"]
    #[name = "Normal"]
    #[default]
    Normal,
    #[id = "high"]
    #[name = "High"]
    High,
}

/// What decides that a bin is resonance rather than a note (`REQ-PUM-003`).
///
/// A separate type from `pumice_core::Mode` for the same reason
/// [`QualityParam`] is separate from `Quality`.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum ModeParam {
    #[id = "adaptive"]
    #[name = "Adaptive"]
    #[default]
    Adaptive,
    #[id = "static"]
    #[name = "Static"]
    Static,
}

impl From<ModeParam> for pumice_core::Mode {
    fn from(value: ModeParam) -> Self {
        match value {
            ModeParam::Adaptive => pumice_core::Mode::Adaptive,
            ModeParam::Static => pumice_core::Mode::Static,
        }
    }
}

impl From<QualityParam> for pumice_core::Quality {
    fn from(value: QualityParam) -> Self {
        match value {
            QualityParam::Low => pumice_core::Quality::Low,
            QualityParam::Normal => pumice_core::Quality::Normal,
            QualityParam::High => pumice_core::Quality::High,
        }
    }
}

/// One node's four controls (`REQ-PUM-004`).
///
/// **`#[nested(array)]` gives these a `_1`..`_6` suffix**, so `on_3` and `hz_3`
/// belong to node three and a host groups them together. Six copies of one
/// struct rather than twenty-four fields, and the ids are still unique and
/// still final.
#[derive(Params)]
pub struct NodeParams {
    #[id = "on"]
    pub enabled: BoolParam,
    #[id = "hz"]
    pub freq: FloatParam,
    #[id = "w"]
    pub width: FloatParam,
    /// **Bipolar. Negative protects** (`REQ-PUM-004`).
    #[id = "d"]
    pub depth: FloatParam,
}

impl NodeParams {
    /// **No smoothers, on purpose.** These are read once per block and compared
    /// against what the weight curve was last built from; a smoother would make
    /// every block a change and rebuild the curve for nothing. A jump in the
    /// curve is crossfaded by the transform's own overlap anyway.
    fn new(index: usize, freq_hz: f32) -> Self {
        let ordinal = index + 1;
        Self {
            enabled: BoolParam::new(format!("Node {ordinal}"), false),
            freq: FloatParam::new(
                format!("Node {ordinal} Frequency"),
                freq_hz,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 20_000.0,
                    factor: FloatRange::skew_factor(-2.0),
                },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
            .with_string_to_value(formatters::s2v_f32_hz_then_khz()),
            width: FloatParam::new(
                format!("Node {ordinal} Width"),
                0.5,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 4.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" oct")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),
            depth: FloatParam::new(
                format!("Node {ordinal} Depth"),
                0.5,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }

    fn resolve(&self) -> pumice_core::Node {
        pumice_core::Node {
            enabled: self.enabled.value(),
            freq_hz: self.freq.value(),
            width_octaves: self.width.value(),
            depth: self.depth.value(),
        }
    }
}

#[derive(Params)]
pub struct PumiceParams {
    /// The amount of everything. **Zero is exactly nothing**
    /// (`REQ-PUM-002`).
    #[id = "depth"]
    pub depth: FloatParam,

    /// How narrow a peak has to be to count as a resonance — the width of the
    /// reference each bin is judged against (`REQ-PUM-005`).
    #[id = "sharpness"]
    pub sharpness: FloatParam,

    /// How fast the reduction follows. The fast end is bounded by the hop
    /// whatever is asked for (`REQ-PUM-020`).
    #[id = "speed"]
    pub speed: FloatParam,

    /// What counts as resonance (`REQ-PUM-003`).
    ///
    /// **`Adaptive` is the default and this is the first release**, so the
    /// discipline that pinned Sparkleur's `MODE` to `Soft` — a session saved
    /// before the parameter existed loads with the default — does not bind
    /// here. It binds from now on.
    #[id = "mode"]
    pub mode: EnumParam<ModeParam>,

    /// The band the plugin works in at all (`REQ-PUM-004`).
    ///
    /// **Not on the main row.** The edges and the nodes are the same kind of
    /// statement about frequency, and the figure is where both are read.
    #[id = "low"]
    pub low: FloatParam,
    #[id = "high"]
    pub high: FloatParam,

    /// Six nodes. **Six because a host's parameters are static** — automation
    /// is stored against ids, so a seventh cannot appear at run time
    /// (`REQ-PUM-004`).
    #[nested(array, group = "Nodes")]
    pub nodes: [NodeParams; pumice_core::NODES],

    /// **`.non_automatable()`, and that is the point** (`REQ-PUM-008`).
    ///
    /// Each step reports a different latency. A host asked to redo delay
    /// compensation at every automation point would glitch at every automation
    /// point, so the parameter is savable and settable but never a lane.
    /// A run-time change is confirmed to recover in two hosts (`PUM-1`).
    ///
    /// **The default can never move.** `NORMAL` is what a session saved before
    /// any future step existed will load as, and `QUALITY` changes the sound as
    /// well as the latency (`REQ-SPK-022` is the same discipline).
    #[id = "quality"]
    pub quality: EnumParam<QualityParam>,
}

impl Default for PumiceParams {
    fn default() -> Self {
        Self {
            // **Provisional, all three** (`PUM-11`). `REQ-PUM-024` says the
            // defaults are the product's face, and no ear has been near them.
            depth: unit("Depth", 0.5),
            sharpness: unit("Sharpness", 0.5),
            speed: unit("Speed", 0.5),
            mode: EnumParam::new("Mode", ModeParam::Adaptive),
            low: edge("Low", 100.0),
            high: edge("High", 18_000.0),
            // **Spread across the band**, so a node switched on from a host's
            // generic UI lands somewhere useful. The window places them where
            // the pointer is instead (`PUM-10`).
            nodes: std::array::from_fn(|index| {
                const SPREAD: [f32; pumice_core::NODES] =
                    [200.0, 400.0, 1_000.0, 2_500.0, 5_000.0, 10_000.0];
                NodeParams::new(index, SPREAD[index])
            }),
            quality: EnumParam::new("Quality", QualityParam::Normal).non_automatable(),
        }
    }
}

impl PumiceParams {
    /// What the core is asked for, advanced by one block.
    ///
    /// **`next_step(samples)` rather than one `next()` per sample**: the engine
    /// resolves its reference width and its coefficients once per block, so the
    /// parameters have to arrive at block rate too — and stepping by the block
    /// length keeps the travel per second the same whatever the host's buffer
    /// is.
    pub fn controls(&self, samples: u32) -> pumice_core::Controls {
        pumice_core::Controls {
            depth: self.depth.smoothed.next_step(samples),
            sharpness: self.sharpness.smoothed.next_step(samples),
            speed: self.speed.smoothed.next_step(samples),
            mode: self.mode.value().into(),
            quality: self.quality.value().into(),
            nodes: std::array::from_fn(|index| self.nodes[index].resolve()),
            range: pumice_core::Range {
                low_hz: self.low.value(),
                high_hz: self.high.value(),
            },
        }
    }
}

/// One end of the operating range.
fn edge(name: &str, default_hz: f32) -> FloatParam {
    FloatParam::new(
        name,
        default_hz,
        FloatRange::Skewed {
            min: 20.0,
            max: 20_000.0,
            factor: FloatRange::skew_factor(-2.0),
        },
    )
    .with_unit(" Hz")
    .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
    .with_string_to_value(formatters::s2v_f32_hz_then_khz())
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
        let params = PumiceParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        let mut expected = vec![
            "depth".to_string(),
            "sharpness".to_string(),
            "speed".to_string(),
            "mode".to_string(),
            "low".to_string(),
            "high".to_string(),
        ];
        for ordinal in 1..=pumice_core::NODES {
            for field in ["on", "hz", "w", "d"] {
                expected.push(format!("{field}_{ordinal}"));
            }
        }
        expected.push("quality".to_string());
        assert_eq!(ids, expected);
    }

    /// `REQ-PUM-008`: a latency-changing parameter must never be a lane.
    #[test]
    fn quality_is_not_automatable() {
        let params = PumiceParams::default();
        for (id, pointer, _) in params.param_map() {
            let flags = unsafe { pointer.flags() };
            let automatable = !flags.contains(ParamFlags::NON_AUTOMATABLE);
            assert_eq!(
                automatable,
                id != "quality",
                "{id} has the wrong automation flag"
            );
        }
    }
}
