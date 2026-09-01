//! What the editor is told about the sound going through.
//!
//! `Handoff`s shared between the audio thread and the editor: the audio thread
//! publishes a frame per block, the editor reads whatever is there when it
//! redraws. Held in an `Arc`, so closing the window disturbs nothing
//! (`docs/specifications/architecture.md`).
//!
//! **The analysers themselves live on the plugin**, not here — they are audio
//! thread state and nothing else may touch them.
//!
//! ## The spectrum is not measured separately, and that is the point
//!
//! Every other plugin in the line runs `nxe_dsp::Spectrum` — a bank of
//! band-passes — beside its engine, and pays for it (Velour: 45 µs for two).
//! Pumice already has the spectrum: it is what the engine is working on. So
//! **the picture and the processing cannot disagree**, which none of the other
//! five can claim, and the display costs nothing (`REQ-PUM-018`).

use nxe_dsp::Handoff;
use pumice_core::CURVE_POINTS;

/// IN left, IN right, OUT left, OUT right — in that order, which the meters
/// depend on.
pub const METERS: usize = 4;

/// The one figure in the readout row: how much is being taken out at the
/// deepest point.
pub const READOUTS: usize = 1;

#[derive(Default)]
pub struct Analysis {
    /// Peak per meter, and the held peak beside it.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// Input power in dB, on the figure's logarithmic axis.
    pub spectrum: Handoff<CURVE_POINTS>,
    /// **The figure's subject** (`REQ-PUM-013`): what is being taken out, per
    /// frequency, in dB.
    ///
    /// A process that works automatically and cannot be seen reads as a process
    /// that is not working — which is the mistake `REQ-SPK-008` records about
    /// Sparkleur's guard.
    pub reduction: Handoff<CURVE_POINTS>,
    /// The nodes and the operating range, `0..=2`. What the user set, as
    /// against what the plugin is doing with it.
    pub weight: Handoff<CURVE_POINTS>,
    /// The deepest reduction, in dB.
    pub readouts: Handoff<READOUTS>,
}
