//! Sensitivity, Height, Decay, Wet (the field is `mix`). **The field names
//! are the parameter ids** (`particle-kit-plan.md`, the table): never rename
//! them.

use nxe_dsp::Handoff;
use nxe_egui::text;
use particle_core::glint::{READING, SPARKS};
use std::sync::atomic::AtomicU32;
use truce::prelude::*;

#[derive(Params)]
pub struct GlintParams {
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
        name = "Height",
        range = "linear(0, 1)",
        default = 0.5,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub height: FloatParam,

    #[param(
        name = "Decay",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub decay: FloatParam,

    /// Shown as Wet: the dry signal is always all there, this is how much of
    /// the sparks is added to it.
    #[param(
        name = "Wet",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        parse = "percent",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,

    /// The sparks sounding, for the figure (`Glint::sparks`).
    #[skip]
    pub sparks: Handoff<{ SPARKS * READING }>,
    /// Blocks processed, so the window knows when the host stopped calling.
    #[skip]
    pub count: AtomicU32,
}

impl GlintParams {
    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }
}
