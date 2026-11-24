//! The target the history is drawn against. It does not touch the sound.

use peek_core::meters::LoudnessReadings;
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Target {
    #[name = "-14 LUFS"]
    Fourteen,
    #[name = "-16 LUFS"]
    Sixteen,
    #[name = "-18 LUFS"]
    Eighteen,
    #[name = "-23 LUFS"]
    TwentyThree,
}

impl Target {
    pub const LUFS: [f32; 4] = [-14.0, -16.0, -18.0, -23.0];
    pub const NAMES: [&str; 4] = ["-14", "-16", "-18", "-23"];
}

#[derive(Params)]
pub struct PeekLoudnessParams {
    #[skip]
    pub readings: LoudnessReadings,

    #[param(name = "Target", default = 0)]
    pub target: EnumParam<Target>,
}
