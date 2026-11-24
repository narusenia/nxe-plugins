//! Fold, Symmetry and Mix. **The field names are the parameter ids**
//! (`nonlinear-kit-plan.md`, the table): never rename them.

use nxe_egui::text;
use truce::prelude::*;

#[derive(Params)]
pub struct FoldParams {
    #[param(
        name = "Fold",
        range = "log(1, 8)",
        default = 2.5,
        format = "times",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub fold: FloatParam,

    #[param(
        name = "Symmetry",
        range = "linear(-1, 1)",
        default = 0.0,
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub symmetry: FloatParam,

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
}

impl FoldParams {
    fn times(&self, value: f64) -> String {
        format!("{value:.1}×")
    }

    /// **Signed, always**, so the row does not twitch at centre.
    fn signed_percent(&self, value: f64) -> String {
        format!("{:+.0} %", value * 100.0)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
