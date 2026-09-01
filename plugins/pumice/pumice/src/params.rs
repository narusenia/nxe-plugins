//! What the host can change.
//!
//! **The main five are `REQ-PUM-009`'s, and three of them are here** — `MIX`
//! and `OUTPUT` arrive with the dry path they mix against (`PUM-6`). The six
//! nodes are `PUM-5`. Declaring either now would put parameters in a host's
//! project file that do nothing, and a parameter id is as final as `CLAP_ID`.

use nih_plug::prelude::*;
use std::sync::Arc;

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

/// The bottom and the top of the figure's frequency axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// The narrowest and widest a node may be, in octaves.
pub const NARROWEST_OCTAVES: f32 = 0.1;
pub const WIDEST_OCTAVES: f32 = 4.0;

/// `freq` and `width` are **positions**, not quantities.
///
/// **Every node control is a plain `0..=1`**, and what it means is applied
/// here. The alternative was a skewed range, and it does not survive contact
/// with the figure: a node is dragged to a *place on the axis*, so the window
/// would have to invert the skew to write the value it just read a position
/// from — and a skew that is only approximately logarithmic makes that
/// inversion approximate too. A normalized parameter **is** the figure's own
/// coordinate, so there is nothing to invert (`ui.md`).
///
/// A host's generic UI still reads in Hz and octaves, because
/// `with_value_to_string` says so.
pub fn position_to_hz(position: f32) -> f32 {
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(position.clamp(0.0, 1.0))
}

pub fn hz_to_position(hz: f32) -> f32 {
    ((hz / LOW_HZ).log2() / (HIGH_HZ / LOW_HZ).log2()).clamp(0.0, 1.0)
}

pub fn position_to_octaves(position: f32) -> f32 {
    NARROWEST_OCTAVES * (WIDEST_OCTAVES / NARROWEST_OCTAVES).powf(position.clamp(0.0, 1.0))
}

pub fn octaves_to_position(octaves: f32) -> f32 {
    ((octaves / NARROWEST_OCTAVES).log2() / (WIDEST_OCTAVES / NARROWEST_OCTAVES).log2())
        .clamp(0.0, 1.0)
}

/// A position printed the way a person reads a frequency.
fn hz_text(position: f32) -> String {
    let hz = position_to_hz(position);
    if hz >= 1_000.0 {
        format!("{:.2} kHz", hz / 1_000.0)
    } else {
        format!("{hz:.0} Hz")
    }
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
                hz_to_position(freq_hz),
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(hz_text)),
            width: FloatParam::new(
                format!("Node {ordinal} Width"),
                octaves_to_position(0.5),
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_value_to_string(Arc::new(|position| {
                format!("{:.2} oct", position_to_octaves(position))
            })),
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

    pub fn resolve(&self) -> pumice_core::Node {
        pumice_core::Node {
            enabled: self.enabled.value(),
            freq_hz: position_to_hz(self.freq.value()),
            width_octaves: position_to_octaves(self.width.value()),
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

    /// **How easily it reacts.** How far above its own neighbourhood a bin has
    /// to sit before anything is taken out of it.
    ///
    /// **It was an internal constant, and that was the gap** (`PUM-10c`). The
    /// default is measured — 4.5 dB is where white noise and a sung line's
    /// partials both read nothing (`pumice_core::Settings::threshold_db`) — but
    /// a measurement on synthetic material is a starting point, not an answer
    /// for every voice. Turning it **down** makes the plugin react to less.
    #[id = "thresh"]
    pub threshold: FloatParam,

    /// Dry against wet (`REQ-PUM-012`).
    #[id = "mix"]
    pub mix: FloatParam,

    /// The last gain in the chain.
    #[id = "output"]
    pub output: FloatParam,

    /// Listen to what is being taken out, and nothing else (`REQ-PUM-019`).
    ///
    /// **A parameter rather than a view-only toggle**, the way Sparkleur's
    /// per-band solos are: being able to automate it makes a before-and-after
    /// comparison something a host can do rather than something a hand has to.
    #[id = "delta"]
    pub delta: BoolParam,

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
            threshold: FloatParam::new(
                "Threshold",
                pumice_core::Settings::DEFAULT.threshold_db,
                FloatRange::Linear {
                    min: 0.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_smoother(SmoothingStyle::Linear(SMOOTHING_MS)),
            mix: unit("Mix", 1.0),
            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(1))
            .with_smoother(SmoothingStyle::Linear(SMOOTHING_MS)),
            delta: BoolParam::new("Delta", false),
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
            threshold_db: self.threshold.smoothed.next_step(samples),
            mix: self.mix.smoothed.next_step(samples),
            // **0 dB has to come out exactly 1.0**, or `OUTPUT` at rest is a
            // rounding error away from unity.
            output: util::db_to_gain(self.output.smoothed.next_step(samples)),
            delta: self.delta.value(),
            quality: self.quality.value().into(),
            nodes: std::array::from_fn(|index| self.nodes[index].resolve()),
            range: pumice_core::Range {
                low_hz: position_to_hz(self.low.value()),
                high_hz: position_to_hz(self.high.value()),
            },
        }
    }
}

/// `OUTPUT` at rest must be exactly unity, which is why the parameter is in dB
/// and the conversion happens once per block rather than being stored linear.
///
/// One end of the operating range.
fn edge(name: &str, default_hz: f32) -> FloatParam {
    FloatParam::new(
        name,
        hz_to_position(default_hz),
        FloatRange::Linear { min: 0.0, max: 1.0 },
    )
    .with_value_to_string(Arc::new(hz_text))
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

    /// A position and a frequency have to be the same thing read two ways, or
    /// the figure writes a node somewhere other than where it was dropped.
    #[test]
    fn positions_and_frequencies_round_trip() {
        for hz in [20.0_f32, 100.0, 440.0, 2_500.0, 12_000.0, 20_000.0] {
            let back = position_to_hz(hz_to_position(hz));
            assert!(
                (back / hz).log2().abs() < 1e-4,
                "{hz} Hz came back as {back}"
            );
        }
        for octaves in [0.1_f32, 0.5, 1.0, 4.0] {
            let back = position_to_octaves(octaves_to_position(octaves));
            assert!(
                (back / octaves).log2().abs() < 1e-4,
                "{octaves} oct came back as {back}"
            );
        }
    }

    /// The ends of the axis are the ends of the parameter.
    #[test]
    fn the_axis_ends_line_up() {
        assert_eq!(hz_to_position(LOW_HZ), 0.0);
        assert!((hz_to_position(HIGH_HZ) - 1.0).abs() < 1e-6);
        assert!((position_to_hz(0.0) - LOW_HZ).abs() < 0.01);
        assert!((position_to_hz(1.0) - HIGH_HZ).abs() < 1.0);
    }

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
            "thresh".to_string(),
            "mix".to_string(),
            "output".to_string(),
            "delta".to_string(),
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
