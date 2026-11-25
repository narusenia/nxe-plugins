//! Density, Size, Scatter, Wet (the field is `mix`). **The field names are
//! the parameter ids** (`particle-kit-plan.md`, the table): never rename them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use particle_core::pollen::{GRAINS, READING};
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

#[derive(Params)]
pub struct PollenParams {
    #[param(
        name = "Density",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub density: FloatParam,

    #[param(
        name = "Size",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub size: FloatParam,

    #[param(
        name = "Scatter",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub scatter: FloatParam,

    /// Shown as Wet: the dry signal is always all there, this is how much of
    /// the cloud is added to it.
    #[param(
        name = "Wet",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    /// The grains sounding, for the figure (`Pollen::grains`).
    #[skip]
    pub grains: Handoff<{ GRAINS * READING }>,
    /// Blocks processed, so the window knows when the host stopped calling.
    #[skip]
    pub count: AtomicU32,
}

impl PollenParams {
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
