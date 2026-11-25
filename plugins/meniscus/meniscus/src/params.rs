//! Sensitivity, Tension, Viscosity, Splash, Resonance, Drops, Wet (the field
//! is `mix`), Output, and the Tuning switch and its Pitch. **The field names
//! are the parameter ids** (`meniscus-plan.md`, the table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use resonance_core::dew::{DROPS, READING};
use resonance_core::meniscus::HISTORY;
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

/// The surface's history, the drops, then the pitch in Hz.
pub const SHOWN: usize = HISTORY + DROPS * READING + 1;

#[derive(ParamEnum, Debug)]
pub enum Tuning {
    Free,
    Follow,
}

impl Tuning {
    pub const NAMES: [&'static str; 2] = ["Free", "Follow"];
}

#[derive(Params)]
pub struct MeniscusParams {
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
        name = "Tension",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub tension: FloatParam,

    #[param(
        name = "Viscosity",
        range = "linear(0, 1)",
        default = 0.35,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub viscosity: FloatParam,

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
        name = "Resonance",
        range = "linear(0, 1)",
        default = 0.7,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub resonance: FloatParam,

    #[param(
        name = "Drops",
        range = "linear(0, 1)",
        default = 0.0,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub drops: FloatParam,

    /// Shown as Wet: the dry signal is always all there, this is how much of
    /// the surface is added to it.
    #[param(
        name = "Wet",
        range = "linear(0, 1)",
        default = 0.5,
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

    #[param(name = "Tuning", default = 0, parse = "tuning")]
    pub tuning: EnumParam<Tuning>,

    /// The surface's pitch in Free.
    #[param(
        name = "Pitch",
        range = "log(30, 1000)",
        default = 110.0,
        format = "hz",
        parse = "hz_text",
        smooth = "linear(20)"
    )]
    pub pitch: FloatParam,

    /// The surface, the drops and the pitch, for the figure.
    #[skip]
    pub shown: Handoff<SHOWN>,
    /// Blocks processed, so the window knows when the host stopped calling.
    #[skip]
    pub count: AtomicU32,
}

impl MeniscusParams {
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn hz(&self, value: f64) -> String {
        format!("{value:.0} Hz")
    }

    fn hz_text(&self, text: &str) -> Option<f64> {
        text::hz(text)
    }

    fn tuning(&self, text: &str) -> Option<f64> {
        text::choice(&Tuning::NAMES, text)
    }
}
