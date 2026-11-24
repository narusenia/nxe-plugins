//! Persistence, Damping and Mix. **The field names are the parameter ids**
//! (`spectral-kit-plan.md`, the table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use spectral_core::BANDS;
use truce::prelude::*;

#[derive(Params)]
pub struct AfterimageParams {
    /// The input and the trace over it, in bands, for the figure. Not
    /// parameters.
    #[skip]
    pub present: Handoff<BANDS>,
    #[skip]
    pub trace: Handoff<BANDS>,

    /// How long a trace lasts at the bottom of the spectrum, in seconds.
    #[param(
        name = "Persistence",
        range = "log(0.2, 20)",
        default = 3.0,
        format = "seconds",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub persistence: FloatParam,

    /// How much sooner the top fades.
    #[param(
        name = "Damping",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub damping: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,
}

impl AfterimageParams {
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
