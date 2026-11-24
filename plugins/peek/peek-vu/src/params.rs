//! The reference level and the look. Neither touches the sound.

use peek_core::meters::VuReadings;
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Look {
    #[name = "Arc"]
    Arc,
    #[name = "Line"]
    Line,
}

#[derive(Params)]
pub struct PeekVuParams {
    #[skip]
    pub readings: VuReadings,

    /// Which dBFS level reads 0 VU.
    #[param(name = "Reference", range = "linear(-24, -8)", default = -18.0, unit = "dB")]
    pub reference: FloatParam,

    #[param(name = "Look", default = 0)]
    pub look: EnumParam<Look>,
}
