//! How much time the picture covers. It does not touch the sound.

use peek_core::meters::ScopeReadings;
use truce::prelude::*;

#[derive(Params)]
pub struct PeekScopeParams {
    #[skip]
    pub readings: ScopeReadings,

    #[param(name = "Window", range = "log(5, 500)", default = 50.0, unit = "ms")]
    pub window: FloatParam,
}
