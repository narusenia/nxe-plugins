//! The tilt the picture is drawn with. It does not touch the sound.

use peek_core::meters::{SPECTRUM_BANDS, SpectrumReadings};
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Tilt {
    #[name = "0 dB/oct"]
    Flat,
    #[name = "3 dB/oct"]
    Three,
    #[name = "4.5 dB/oct"]
    FourHalf,
}

impl Tilt {
    pub const SLOPES: [f32; 3] = [0.0, 3.0, 4.5];
}

#[derive(Params)]
pub struct PeekSpectrumParams {
    #[skip]
    pub readings: SpectrumReadings<SPECTRUM_BANDS>,

    #[param(name = "Tilt", default = 2)]
    pub tilt: EnumParam<Tilt>,
}
