//! Fever, Recovery and Mix. **The field names are the parameter ids**
//! (`nonlinear-kit-plan.md`, the table): never rename them.

use nxe_egui::text;
use truce::prelude::*;

#[derive(Params)]
pub struct FeverParams {
    /// How readily it heats.
    #[param(
        name = "Fever",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub fever: FloatParam,

    /// How long it takes to cool.
    #[param(
        name = "Recovery",
        range = "log(0.2, 10)",
        default = 2.0,
        format = "seconds",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub recovery: FloatParam,

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

    /// How hot it is, `0..=1`.
    #[meter]
    pub heat: MeterSlot,
}

impl FeverParams {
    fn seconds(&self, value: f64) -> String {
        format!("{value:.1} s")
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
