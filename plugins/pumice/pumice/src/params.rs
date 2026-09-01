//! What the host can change. **One parameter, and it is here to be tested
//! rather than to be used** (`PUM-1`).
//!
//! The rest of `REQ-PUM-009` — `DEPTH`, `SHARPNESS`, `SPEED`, `MIX`,
//! `OUTPUT`, `MODE`, `DELTA` and the six nodes — arrives with the engine that
//! gives them something to do (`PUM-3` onward). Declaring them now would mean
//! shipping a gate build whose parameters do nothing, and a host would save
//! them into a project either way.

use nih_plug::prelude::*;

/// How big the transform is (`REQ-PUM-008`).
///
/// A separate type from `pumice_core::Quality` on purpose: deriving nih-plug's
/// `Enum` on the shared type would make the core depend on nih-plug
/// (`params.rs` in Sparkleur says the same about `Factor`).
///
/// **The variants carry `#[id]`.** nih-plug writes an enum with ids into saved
/// state as its id string rather than as a number, so a fourth step could be
/// added later without moving what an existing session means.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum QualityParam {
    #[id = "low"]
    #[name = "Low"]
    Low,
    #[id = "normal"]
    #[name = "Normal"]
    #[default]
    Normal,
    #[id = "high"]
    #[name = "High"]
    High,
}

impl From<QualityParam> for pumice_core::Quality {
    fn from(value: QualityParam) -> Self {
        match value {
            QualityParam::Low => pumice_core::Quality::Low,
            QualityParam::Normal => pumice_core::Quality::Normal,
            QualityParam::High => pumice_core::Quality::High,
        }
    }
}

#[derive(Params)]
pub struct PumiceParams {
    /// **`.non_automatable()`, and that is the whole point of declaring it in
    /// the gate build** (`REQ-PUM-008`).
    ///
    /// Each step reports a different latency. A host asked to redo delay
    /// compensation at every automation point would glitch at every automation
    /// point, so the parameter is savable and settable but never a lane.
    ///
    /// **The default can never move.** `NORMAL` is what a session saved before
    /// any future step existed will load as, and `QUALITY` changes the sound as
    /// well as the latency (`REQ-SPK-022` is the same discipline).
    #[id = "quality"]
    pub quality: EnumParam<QualityParam>,
}

impl Default for PumiceParams {
    fn default() -> Self {
        Self {
            quality: EnumParam::new("Quality", QualityParam::Normal).non_automatable(),
        }
    }
}
