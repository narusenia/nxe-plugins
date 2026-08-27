//! What the editor is told about the sound going through.
//!
//! `Handoff`s shared between the audio thread and the editor: the audio
//! thread publishes a frame per block, the editor reads whatever is there when
//! it redraws. Held in an `Arc` so closing the window disturbs nothing
//! (`docs/specifications/architecture.md`).
//!
//! **The analysers themselves live on the plugin**, not here — they are audio
//! thread state and nothing else may touch them.

use nxe_dsp::Handoff;

/// IN left, IN right, OUT left, OUT right — in that order, which the readout
/// depends on.
pub const METERS: usize = 4;

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
    /// Peak per meter, and the held peak beside it.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// **How alike the two output channels are**, `-1..=1`
    /// (`nxe_dsp::Correlation`).
    ///
    /// The one number a widener owes its user. Every way this plugin makes an
    /// image — detuning a copy, delaying it, panning the pair apart — works by
    /// making the channels differ, and the far end of that is a pair that
    /// cancels when summed to mono. The Voice Field says the image is wide; it
    /// cannot say whether the width survives a fold.
    pub correlation: Handoff<1>,
}

#[cfg(test)]
mod tests {
    /// **Every handoff is written by the audio thread.**
    ///
    /// A handoff nobody writes is a display that never moves, and **nothing
    /// else notices**: it compiles, the editor binds to it, the heartbeat reads
    /// it thirty times a second, and every figure sits at zero while the plugin
    /// makes sound. Air shipped one build exactly that way, and it looked like
    /// a track with no signal on it (`.agents/rules/ui.md`).
    ///
    /// The mirror of `SPK-19`'s finding, which was two handoffs **written and
    /// read by nothing**. Both directions are silent, so both are worth a test.
    #[test]
    fn every_handoff_is_published() {
        const ANALYSIS: &str = include_str!("analysis.rs");
        const PLUGIN: &str = include_str!("lib.rs");

        let fields: Vec<&str> = ANALYSIS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub "))
            .filter(|rest| rest.contains(": Handoff<"))
            .filter_map(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
            .collect();
        assert_eq!(fields.len(), 5, "the handoff list moved: {fields:?}");

        // **Whitespace removed first.** rustfmt breaks a long call across
        // lines, so a scan for the literal text finds nothing the moment the
        // line grows — which would make this test pass by being blind.
        let plugin: String = PLUGIN.chars().filter(|c| !c.is_whitespace()).collect();
        for field in fields {
            let write = format!("analysis.{field}.write");
            assert!(
                plugin.contains(&write),
                "{field} is never published, so whatever reads it never moves"
            );
        }
    }
}
