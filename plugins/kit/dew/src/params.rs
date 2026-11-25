//! Size, Splash, Mix and the Tuning switch. **The field names are the
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

    fn tuning(&self, text: &str) -> Option<f64> {
        text::choice(&Tuning::NAMES, text)
    }
}
