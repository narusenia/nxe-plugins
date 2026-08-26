//! What the editor is told about the sound going through.
//!
//! Two `Handoff`s shared between the audio thread and the editor: the audio
//! thread publishes a frame per block, the editor reads whatever is there when
//! it redraws. Held in an `Arc` so closing the window disturbs nothing
//! (`docs/specifications/architecture.md`).
//!
//! **The analysers themselves live on the plugin**, not here — they are audio
//! thread state and nothing else may touch them.

use nxe_dsp::Handoff;

/// Directions across the arc. Enough for the shape of a stereo image to read at
/// the width the figure is drawn; more would be a finer picture of noise.
pub const PAN_BINS: usize = 24;

/// Bands across the audible range — a little over three per octave, which is
/// the resolution the Filter View can show at its width.
pub const BANDS: usize = 32;

/// The range the spectrum covers, matching the Filter View's own axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

#[derive(Default)]
pub struct Analysis {
    /// Energy per direction, from the plugin's output — where the sound
    /// actually is, under the dots that say where it was asked to be.
    pub pan: Handoff<PAN_BINS>,
    /// Level per band, from the **wet bus**: `Tone` is what the Filter View
    /// draws, and the wet bus is what `Tone` acts on.
    pub spectrum: Handoff<BANDS>,
}
