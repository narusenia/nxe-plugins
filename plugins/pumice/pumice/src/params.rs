//! What the host can change.
//!
//! **Frequencies and widths are declared in their own units** on truce, which
//! has a logarithmic range; on nih-plug they were `0..=1` positions mapped by
//! hand. Pumice never shipped, so nothing saved depends on the old shape
//! (`v2-plan.md`, decision 16).
//!
//! **Every parameter reads its own text back** (`nxe_egui::text`): a
//! parameter without `parse` fails clap-validator's conversions test.

use crate::analysis::Analysis;
use nxe_egui::text;
use pumice_core::{NODES, Node, Quality, Range};
use truce::prelude::*;

/// The narrowest and widest a node can be, in octaves. The `range` of every
/// node's width says the same; `the_width_range_is_these` keeps them equal.
pub const NARROWEST_OCTAVES: f32 = 0.1;
pub const WIDEST_OCTAVES: f32 = 4.0;

/// How big the transform is (`REQ-PUM-008`). A separate type from
/// `pumice_core::Quality` so the core does not depend on truce.
#[derive(ParamEnum, Debug)]
pub enum QualityParam {
    #[name = "Low"]
    Low,
    #[name = "Normal"]
    Normal,
    #[name = "High"]
    High,
}

impl From<QualityParam> for Quality {
    fn from(value: QualityParam) -> Self {
        match value {
            QualityParam::Low => Quality::Low,
            QualityParam::Normal => Quality::Normal,
            QualityParam::High => Quality::High,
        }
    }
}

/// "1.2 kHz" or "350 Hz" (or a bare number) back to Hz.
pub fn hz(text: &str) -> Option<f64> {
    let value = text::number(text)?;
    Some(if text.to_ascii_lowercase().contains('k') {
        value * 1_000.0
    } else {
        value
    })
}

/// One node's four parameters.
///
/// **Six types, not one.** truce fixes a default on the type, and each node
/// starts at its own frequency (Doubler's `voice!` does the same).
#[rustfmt::skip]
macro_rules! node {
    ($ty:ident, $group:tt, $on:tt, $freq_name:tt, $width_name:tt, $depth_name:tt,
     freq = $freq:tt) => {
        #[derive(Params)]
        pub struct $ty {
            #[param(name = $on, group = $group, parse = "switch")]
            pub enabled: BoolParam,
            #[param(
                name = $freq_name,
                group = $group,
                range = "log(20, 20000)",
                default = $freq,
                unit = "Hz",
                parse = "hz"
            )]
            pub freq: FloatParam,
            #[param(
                name = $width_name,
                group = $group,
                range = "log(0.1, 4)",
                default = 0.5,
                format = "octaves",
                parse = "number"
            )]
            pub width: FloatParam,
            #[param(
                name = $depth_name,
                group = $group,
                range = "linear(-1, 1)",
                default = 0.5,
                format = "signed_percent",
                parse = "percent"
            )]
            pub depth: FloatParam,
        }

        impl $ty {
            fn switch(&self, text: &str) -> Option<f64> {
                text::switch(text)
            }

            fn hz(&self, text: &str) -> Option<f64> {
                hz(text)
            }

            fn number(&self, text: &str) -> Option<f64> {
                text::number(text)
            }

            fn percent(&self, text: &str) -> Option<f64> {
                text::percent(text)
            }

            fn octaves(&self, value: f64) -> String {
                format!("{value:.2} oct")
            }

            fn signed_percent(&self, value: f64) -> String {
                format!("{:+.0} %", value * 100.0)
            }

            fn resolve(&self) -> Node {
                Node {
                    enabled: self.enabled.value(),
                    freq_hz: self.freq.value(),
                    width_octaves: self.width.value(),
                    depth: self.depth.value(),
                }
            }

            fn ids(&self) -> [u32; 4] {
                [
                    self.enabled.id(),
                    self.freq.id(),
                    self.width.id(),
                    self.depth.id(),
                ]
            }
        }
    };
}

node!(
    Node1,
    "Node 1",
    "Node 1",
    "Node 1 Frequency",
    "Node 1 Width",
    "Node 1 Depth",
    freq = 200.0
);
node!(
    Node2,
    "Node 2",
    "Node 2",
    "Node 2 Frequency",
    "Node 2 Width",
    "Node 2 Depth",
    freq = 400.0
);
node!(
    Node3,
    "Node 3",
    "Node 3",
    "Node 3 Frequency",
    "Node 3 Width",
    "Node 3 Depth",
    freq = 1000.0
);
node!(
    Node4,
    "Node 4",
    "Node 4",
    "Node 4 Frequency",
    "Node 4 Width",
    "Node 4 Depth",
    freq = 2500.0
);
node!(
    Node5,
    "Node 5",
    "Node 5",
    "Node 5 Frequency",
    "Node 5 Width",
    "Node 5 Depth",
    freq = 5000.0
);
node!(
    Node6,
    "Node 6",
    "Node 6",
    "Node 6 Frequency",
    "Node 6 Width",
    "Node 6 Depth",
    freq = 10000.0
);

#[derive(Params)]
pub struct PumiceParams {
    #[skip]
    pub analysis: Analysis,

    /// How much is taken out where something sticks out.
    #[param(
        name = "Depth",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub depth: FloatParam,
    /// How narrow a thing has to be to count.
    #[param(
        name = "Sharpness",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub sharpness: FloatParam,
    #[param(
        name = "Speed",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub speed: FloatParam,
    /// The default is `pumice_core::Settings::DEFAULT.threshold_db`, as a
    /// literal because truce wants one; `threshold_is_the_cores` keeps them
    /// equal.
    #[param(
        name = "Threshold",
        range = "linear(0, 12)",
        default = 4.5,
        format = "decibels",
        parse = "number",
        smooth = "linear(30)"
    )]
    pub threshold: FloatParam,
    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(30)"
    )]
    pub mix: FloatParam,
    #[param(
        name = "Output",
        range = "linear(-12, 12)",
        format = "decibels",
        parse = "number",
        smooth = "linear(30)"
    )]
    pub output: FloatParam,
    /// Listen to what is being taken out.
    #[param(name = "Delta", parse = "switch")]
    pub delta: BoolParam,
    /// Where the work may happen.
    #[param(
        name = "Low",
        group = "Range",
        range = "log(20, 20000)",
        default = 100.0,
        unit = "Hz",
        parse = "hz"
    )]
    pub low: FloatParam,
    #[param(
        name = "High",
        group = "Range",
        range = "log(20, 20000)",
        default = 18000.0,
        unit = "Hz",
        parse = "hz"
    )]
    pub high: FloatParam,
    /// **Automatable on truce**: `flags` has no "not automatable" short of
    /// `readonly`, which would also lock the window (`v2-plan.md`, V2-9). An
    /// automated change moves the latency, which truce polls after the block
    /// (`PluginLogic::latency`).
    #[param(name = "Quality", default = 1, parse = "quality")]
    pub quality: EnumParam<QualityParam>,

    #[nested]
    pub node1: Node1,
    #[nested]
    pub node2: Node2,
    #[nested]
    pub node3: Node3,
    #[nested]
    pub node4: Node4,
    #[nested]
    pub node5: Node5,
    #[nested]
    pub node6: Node6,
}

impl PumiceParams {
    fn decibels(&self, value: f64) -> String {
        format!("{value:+.1} dB")
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn switch(&self, text: &str) -> Option<f64> {
        text::switch(text)
    }

    fn hz(&self, text: &str) -> Option<f64> {
        hz(text)
    }

    fn quality(&self, text: &str) -> Option<f64> {
        text::choice(&["Low", "Normal", "High"], text)
    }

    /// The controls for a block of `samples`.
    ///
    /// **`read_after(samples)`**: a smoother advances one sample per read, and
    /// this is read once per block.
    pub fn controls(&self, samples: usize) -> pumice_core::Controls {
        pumice_core::Controls {
            depth: self.depth.read_after(samples),
            sharpness: self.sharpness.read_after(samples),
            speed: self.speed.read_after(samples),
            threshold_db: self.threshold.read_after(samples),
            mix: self.mix.read_after(samples),
            output: db_to_linear(self.output.read_after(samples)),
            delta: self.delta.value(),
            quality: self.quality.value().into(),
            nodes: self.nodes(),
            range: self.range(),
        }
    }

    pub fn nodes(&self) -> [Node; NODES] {
        [
            self.node1.resolve(),
            self.node2.resolve(),
            self.node3.resolve(),
            self.node4.resolve(),
            self.node5.resolve(),
            self.node6.resolve(),
        ]
    }

    pub fn range(&self) -> Range {
        Range {
            low_hz: self.low.value(),
            high_hz: self.high.value(),
        }
    }

    /// Node `index`'s ids, as `[enabled, freq, width, depth]`.
    pub fn node_ids(&self, index: usize) -> [u32; 4] {
        match index {
            0 => self.node1.ids(),
            1 => self.node2.ids(),
            2 => self.node3.ids(),
            3 => self.node4.ids(),
            4 => self.node5.ids(),
            _ => self.node6.ids(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use truce_params::Params;

    #[test]
    fn threshold_is_the_cores() {
        let params = PumiceParams::default();
        assert_eq!(
            params.threshold.value(),
            pumice_core::Settings::DEFAULT.threshold_db
        );
    }

    #[test]
    fn the_width_range_is_these() {
        let params = PumiceParams::default();
        let id = params.node1.width.id();
        let info = params
            .param_infos()
            .into_iter()
            .find(|info| info.id == id)
            .unwrap();
        assert!((info.range.min() - f64::from(NARROWEST_OCTAVES)).abs() < 1e-6);
        assert!((info.range.max() - f64::from(WIDEST_OCTAVES)).abs() < 1e-6);
    }

    #[test]
    fn hz_reads_what_truce_prints() {
        assert_eq!(hz("1.2 kHz"), Some(1_200.0));
        assert_eq!(hz("350 Hz"), Some(350.0));
        assert_eq!(hz("x"), None);
    }

    /// Every parameter's printed text parses back to a value that prints the
    /// same — what clap-validator's conversions test checks.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        let params = PumiceParams::default();
        for info in params.param_infos() {
            let (min, max) = (info.range.min(), info.range.max());
            let plain = min + (max - min) * 0.37;
            let text = params.format_value(info.id, plain).unwrap();
            let back = params
                .parse_value(info.id, &text)
                .unwrap_or_else(|| panic!("{} could not read {text:?}", info.name));
            let again = params.format_value(info.id, back).unwrap();
            assert_eq!(text, again, "{} drifted", info.name);
        }
    }

    #[test]
    fn the_nodes_start_off_and_spread() {
        let params = PumiceParams::default();
        let nodes = params.nodes();
        assert!(nodes.iter().all(|node| !node.enabled));
        assert!(
            nodes
                .windows(2)
                .all(|pair| pair[0].freq_hz < pair[1].freq_hz)
        );
    }
}
