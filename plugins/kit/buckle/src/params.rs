//! Threshold, Snap and Mix. **The field names are the parameter ids**
//! (`nonlinear-kit-plan.md`, the table): never rename them.

use nxe_egui::text;
use truce::prelude::*;

#[derive(Params)]
pub struct BuckleParams {
    /// Where the waveform turns over, in dBFS.
    #[param(
        name = "Threshold",
        range = "linear(-36, 0)",
        default = -12.0,
        format = "decibels",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub threshold: FloatParam,

    /// How far it turns: all the way over at 100 %.
    #[param(
        name = "Snap",
        range = "linear(0, 1)",
        default = 0.75,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub snap: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    /// The input's peak, for the figure's shaded reach.
    #[meter]
    pub input: MeterSlot,

    /// How buckled the louder channel is, `0..=1`.
    #[meter]
    pub buckled: MeterSlot,
}

impl BuckleParams {
    fn decibels(&self, value: f64) -> String {
        format!("{value:+.1} dB")
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
