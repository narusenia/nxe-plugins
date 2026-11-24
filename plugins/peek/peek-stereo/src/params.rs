//! **No parameters**; truce needs a params struct to declare something, and
//! the correlation as a host meter slot is the honest thing to declare.

use peek_core::meters::StereoReadings;
use truce::prelude::*;

#[derive(Params)]
pub struct PeekStereoParams {
    #[skip]
    pub readings: StereoReadings,

    /// The correlation, `0..=1` for −1…+1.
    #[meter]
    pub correlation: MeterSlot,
}
