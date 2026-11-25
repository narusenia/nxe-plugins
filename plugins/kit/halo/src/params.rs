//! Decay, Shimmer, Tone, Mix, and the chord, the tuning and the root.
//! **The field names are the parameter ids** (`resonance-kit-plan.md`, the
//! table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use resonance_core::halo::{READING, VOICES};
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

/// The rings, then the root in Hz.
pub const SHOWN: usize = VOICES * READING + 1;

#[derive(ParamEnum, Debug)]
pub enum Tuning {
    Free,
    Follow,
}

impl Tuning {
    pub const NAMES: [&'static str; 2] = ["Free", "Follow"];
}

#[derive(ParamEnum, Debug)]
pub enum Chord {
    Fifth,
    #[name = "Maj"]
    Major,
    #[name = "Min"]
    Minor,
}

impl Chord {
    pub const NAMES: [&'static str; 3] = ["Fifth", "Maj", "Min"];
}

#[derive(Params)]
pub struct HaloParams {
    #[param(
        name = "Decay",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub decay: FloatParam,

    #[param(
        name = "Shimmer",
        range = "linear(0, 1)",
        default = 0.3,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub shimmer: FloatParam,

    #[param(
        name = "Tone",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub tone: FloatParam,

    #[param(
        name = "Mix",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    #[param(name = "Chord", default = 0, parse = "chord")]
    pub chord: EnumParam<Chord>,

    #[param(name = "Tuning", default = 0, parse = "tuning")]
    pub tuning: EnumParam<Tuning>,

    /// The root in Free.
    #[param(
        name = "Root",
        range = "log(40, 1000)",
        default = 220.0,
        format = "hz",
        parse = "hz_text",
        smooth = "linear(20)"
    )]
    pub root: FloatParam,

    /// The rings and the root in use, for the figure (`Halo::rings`).
    #[skip]
    pub shown: Handoff<SHOWN>,
    /// Blocks processed, so the window knows when the host stopped calling.
    #[skip]
    pub count: AtomicU32,
}

impl HaloParams {
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn hz(&self, value: f64) -> String {
        format!("{value:.0} Hz")
    }

    fn hz_text(&self, text: &str) -> Option<f64> {
        text::hz(text)
    }

    fn chord(&self, text: &str) -> Option<f64> {
        text::choice(&Chord::NAMES, text)
    }

    fn tuning(&self, text: &str) -> Option<f64> {
        text::choice(&Tuning::NAMES, text)
    }
}
