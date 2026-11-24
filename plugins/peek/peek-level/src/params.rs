//! **No parameters.** A meter has nothing to automate; the one action it
//! offers, clearing the maxima, is a request to the audio thread rather than
//! a value a host should store (`peek-plan.md`, PEEK-1).
//!
//! What it does declare is the two levels as **meter slots**, which a host
//! may show beside the track. truce requires a params struct to declare
//! something, and a meter reporting its level is the one thing it honestly
//! has.

use crate::analysis::Analysis;
use truce::prelude::*;

#[derive(Params)]
pub struct PeekLevelParams {
    #[skip]
    pub analysis: Analysis,

    /// The left and right peaks, on the window's scale (`0..=1`).
    #[meter]
    pub left: MeterSlot,
    #[meter]
    pub right: MeterSlot,
}
