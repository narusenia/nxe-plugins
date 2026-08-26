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
//! **Two spectra, because the topology allows it** (`REQ-VEL-001`): the input,
//! and the generator bus alone. That pair is the whole reason the figure can
//! show what is being *added* rather than only what came out (`ui.md`).

use nxe_dsp::Handoff;

/// Bands across the audible range.
///
/// **48 rather than the Doubler's 32.** The upper curve here is a harmonic
/// series, and at 32 bands — a little over three per octave — two neighbouring
/// harmonics land in one band above a few kHz and the curve stops showing the
/// thing it exists for. **The cost is measured in `VEL-16`**; if two spectra at
/// this resolution do not fit the budget, this number is the first thing to
/// give.
pub const BANDS: usize = 48;

/// The range the spectra cover, matching the Band Field's own axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// IN left, IN right, OUT left, OUT right — in that order, which the meters
/// depend on.
pub const METERS: usize = 4;

#[derive(Default)]
pub struct Analysis {
    /// What came in, per band.
    pub dry: Handoff<BANDS>,
    /// The generator bus alone, per band — the harmonics being added.
    pub wet: Handoff<BANDS>,
    /// Peak per meter, and the held peak beside it. Two frames rather than one
    /// of eight, so neither has to be unpacked by index arithmetic.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// How far each guard is pulling, in dB, in the order of
    /// `velour_core::guard::GUARDS`.
    pub guards: Handoff<2>,
}
