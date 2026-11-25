//! **The field names are the parameter ids** (`parasite-plan.md`, the table):
//! never rename them. The order is the window's: host, parasite, output.

use nxe_dsp::Handoff;
use nxe_egui::text;
use parasite_core::SCOPE_POINTS;
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Species {
    #[name = "FM"]
    Fm,
    Ring,
    Sync,
    Voice,
}

impl Species {
    pub const NAMES: [&'static str; 4] = ["FM", "Ring", "Sync", "Voice"];
}

#[derive(Params)]
pub struct ParasiteParams {
    /// The host and the output over two host periods, for the figure. Not
    /// parameters.
    #[skip]
    pub host: Handoff<SCOPE_POINTS>,
    #[skip]
    pub output_wave: Handoff<SCOPE_POINTS>,
    /// Pitch (Hz), presence, infection and hunger as they act now.
    #[skip]
    pub readings: Handoff<4>,
    /// Pictures made so far, so the window can tell when they stop (bypass).
    #[skip]
    pub count: AtomicU32,

    // The host.
    /// How readily a pitch is believed.
    #[param(
        name = "Sensitivity",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub sensitivity: FloatParam,

    /// How much an attack shakes the parasite off.
    #[param(
        name = "Resistance",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub resistance: FloatParam,

    // The parasite.
    #[param(name = "Species", default = 0, parse = "species")]
    pub species: EnumParam<Species>,

    #[param(
        name = "Infection",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub infection: FloatParam,

    #[param(
        name = "Spread",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub spread: FloatParam,

    #[param(
        name = "Hunger",
        range = "linear(0, 1)",
        default = 0.3,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub hunger: FloatParam,

    #[param(
        name = "Mutation",
        range = "linear(0, 1)",
        default = 0.1,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mutation: FloatParam,

    // The output.
    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 1.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    #[param(
        name = "Output",
        range = "linear(-12, 12)",
        default = 0.0,
        unit = "dB",
        parse = "number",
        smooth = "linear(20)"
    )]
    pub output: FloatParam,
}

impl ParasiteParams {
    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn species(&self, text: &str) -> Option<f64> {
        text::choice(&Species::NAMES, text)
    }
}
