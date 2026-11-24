//! Shear, Length and Mix. **The field names are the parameter ids**
//! (`spectral-kit-plan.md`, the table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use spectral_core::BANDS;
use truce::prelude::*;

#[derive(Params)]
pub struct ShearParams {
    /// What comes out, in bands, for the spectrogram. Not a parameter.
    #[skip]
    pub levels: Handoff<BANDS>,

    /// Which end is late, and how much of `Length` the lean uses.
    #[param(
        name = "Shear",
        range = "linear(-1, 1)",
        default = 0.5,
        format = "signed_percent",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub shear: FloatParam,

    /// The longest delay, in ms.
    #[param(
        name = "Length",
        range = "log(20, 1000)",
        default = 250.0,
        format = "milliseconds",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub length: FloatParam,

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

impl ShearParams {
    /// **Signed, always**, so the row does not twitch at centre.
    fn signed_percent(&self, value: f64) -> String {
        format!("{:+.0} %", value * 100.0)
    }

    fn milliseconds(&self, value: f64) -> String {
        format!("{value:.0} ms")
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
