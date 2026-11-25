//! Low, High and Mix. **The field names are the parameter ids**
//! (`spectral-kit-plan.md`, the table): never rename them.

use nxe_egui::text;
use truce::prelude::*;

#[derive(Params)]
pub struct InvertParams {
    /// The band's bottom, which comes out at its top.
    #[param(
        name = "Low",
        range = "log(20, 20000)",
        default = 200.0,
        format = "hertz",
        parse = "hertz_in",
        smooth = "linear(20)"
    )]
    pub low: FloatParam,

    /// The band's top, which comes out at its bottom.
    #[param(
        name = "High",
        range = "log(20, 20000)",
        default = 5000.0,
        format = "hertz",
        parse = "hertz_in",
        smooth = "linear(20)"
    )]
    pub high: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,
}

impl InvertParams {
    fn hertz(&self, value: f64) -> String {
        if value >= 1_000.0 {
            format!("{:.2} kHz", value / 1_000.0)
        } else {
            format!("{value:.0} Hz")
        }
    }

    /// "5.00 kHz" and "200 Hz" both read back.
    fn hertz_in(&self, text: &str) -> Option<f64> {
        text::hz(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
