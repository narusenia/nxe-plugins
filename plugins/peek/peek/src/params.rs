//! What the picture is drawn with: the VU's reference, the loudness target,
//! the spectrum's tilt and the scope's window. None of it touches the sound.

use peek_core::meters::{
    LevelReadings, LoudnessReadings, SPECTROGRAM_BANDS, SPECTRUM_BANDS, ScopeReadings,
    SpectrumReadings, StereoReadings, VuReadings,
};
use truce::prelude::*;

/// Every measurement's hand-off: the same structs the single meters hold.
#[derive(Default)]
pub struct Readings {
    pub level: LevelReadings,
    pub vu: VuReadings,
    pub stereo: StereoReadings,
    pub spectrum: SpectrumReadings<SPECTRUM_BANDS>,
    pub spectrogram: SpectrumReadings<SPECTROGRAM_BANDS>,
    pub loudness: LoudnessReadings,
    pub scope: ScopeReadings,
}

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
pub struct PeekParams {
    #[skip]
    pub readings: Readings,

    #[param(name = "Reference", range = "linear(-24, -8)", default = -18.0, unit = "dB")]
    pub reference: FloatParam,

    #[param(name = "Target", default = 0)]
    pub target: EnumParam<Target>,

    #[param(name = "Tilt", default = 2)]
    pub tilt: EnumParam<Tilt>,

    #[param(name = "Window", range = "log(5, 500)", default = 50.0, unit = "ms")]
    pub window: FloatParam,

    /// The louder channel's peak, for a host's meter.
    #[meter]
    pub peak: MeterSlot,
}
