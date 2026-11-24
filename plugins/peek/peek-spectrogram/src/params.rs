//! Where the picture's strength starts and where it is full. Neither touches
//! the sound.

use peek_core::meters::{SPECTROGRAM_BANDS, SpectrumReadings};
use truce::prelude::*;

#[derive(ParamEnum, Debug)]
pub enum Scale {
    #[name = "Log"]
    Log,
    #[name = "Linear"]
    Linear,
}

#[derive(Params)]
pub struct PeekSpectrogramParams {
    #[skip]
    pub readings: SpectrumReadings<SPECTROGRAM_BANDS>,

    #[param(name = "Floor", range = "linear(-120, -48)", default = -72.0, unit = "dB")]
    pub floor: FloatParam,

    #[param(name = "Ceiling", range = "linear(-48, 0)", default = -18.0, unit = "dB")]
    pub ceiling: FloatParam,

    /// How frequency is laid out up the picture.
    #[param(name = "Scale", default = 0)]
    pub scale: EnumParam<Scale>,
}
