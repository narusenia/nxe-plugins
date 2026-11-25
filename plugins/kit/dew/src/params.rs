//! Sensitivity, Size, Splash, Mix, the Tuning switch and its Pitch. **The field names are the
//! parameter ids** (`resonance-kit-plan.md`, the table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use resonance_core::dew::{DROPS, READING};
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Tuning {
    Free,
    Follow,
}

impl Tuning {
    pub const NAMES: [&'static str; 2] = ["Free", "Follow"];
}

#[derive(Params)]
pub struct DewParams {
    #[param(
        name = "Sensitivity",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub sensitivity: FloatParam,

    #[param(
        name = "Size",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub size: FloatParam,

    #[param(
        name = "Splash",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub splash: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    #[param(name = "Tuning", default = 0, parse = "tuning")]
    pub tuning: EnumParam<Tuning>,

    /// Where a drop settles; in Follow, the harmonic of the input nearest to it.
    #[param(
        name = "Pitch",
        range = "log(200, 8000)",
        default = 2000.0,
        format = "hz",
        parse = "hz_text",
        smooth = "linear(20)"
    )]
    pub pitch: FloatParam,

    /// The drops sounding, for the figure (`Dew::drops`).
    #[skip]
    pub drops: Handoff<{ DROPS * READING }>,
    /// Blocks processed, so the window knows when the host stopped calling.
    #[skip]
    pub count: AtomicU32,
}

impl DewParams {
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn hz(&self, value: f64) -> String {
        if value >= 1_000.0 {
            format!("{:.2} kHz", value / 1_000.0)
        } else {
            format!("{value:.0} Hz")
        }
    }

    fn hz_text(&self, text: &str) -> Option<f64> {
        text::hz(text)
    }

    fn tuning(&self, text: &str) -> Option<f64> {
        text::choice(&Tuning::NAMES, text)
    }
}
