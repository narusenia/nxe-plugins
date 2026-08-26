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
//! **One spectrum, not Velour's two** (`REQ-SPK-018`). Velour could separate
//! "the layer being added" because its topology adds one; a split topology has
//! no such thing — every band is the signal. **The per-band gains take that
//! role**, and they are the subject of the picture rather than a backdrop to
//! it (`ui.md`).

use nxe_dsp::Handoff;
use sparkleur_core::crossover::BAND_COUNT;

/// Bands across the audible range.
///
/// **32, not Velour's 48.** Velour's upper curve is a harmonic series, and
/// resolving neighbouring harmonics is what 48 bought; here the spectrum is the
/// backdrop behind the regions, and three bands per octave is enough to read as
/// "where the music is". **`SPK-17` decides whether it stays** — if the budget
/// is tight this is a cheap thing to halve, and if it is not, raising it costs
/// only clarity nobody asked for.
pub const BANDS: usize = 32;

/// The range the spectrum covers, matching the Band Field's own axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// IN left, IN right, OUT left, OUT right — in that order, which the meters
/// depend on.
pub const METERS: usize = 4;

#[derive(Default)]
pub struct Analysis {
    /// What came in, per band. The backdrop.
    pub dry: Handoff<BANDS>,
    /// Peak per meter, and the held peak beside it. Two frames rather than one
    /// of eight, so neither has to be unpacked by index arithmetic.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// **The subject**: what is actually being applied to each band, in dB, in
    /// band order. Positive is upward compression, and the figure draws it
    /// above the unity line (`nxe_ui::band::Band::delta`, `SPK-10`).
    pub gains: Handoff<BAND_COUNT>,
    /// What each band's detector is reading, in dB — **the input side of the
    /// transfer curve** (`SPK-19`). Paired with `gains` it says where on its own
    /// curve a band is sitting right now, which is what the plot is read for.
    pub levels: Handoff<BAND_COUNT>,
    /// How far De-Harsh is pulling, in dB. Zero when it is doing nothing —
    /// **a protection that works invisibly leaves a user with an `AIR` knob
    /// that does nothing and no way to find out why** (`REQ-SPK-008`).
    pub de_harsh: Handoff<1>,
    /// How far the Sparkle gate stands open, `0..=1`. The moment it lights.
    pub sparkle: Handoff<1>,
}
