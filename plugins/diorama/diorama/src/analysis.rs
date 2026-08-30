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
//! **No spectrum.** Distance is not a spectral quantity: what says "far away"
//! is the direct-to-reflected ratio and the spread of arrival times, and
//! neither is visible in a spectrum (`ui.md`). What is published instead is the
//! arrival pattern, which is the figure's whole subject.

use diorama_core::reflections::TAPS;
use nxe_dsp::Handoff;

/// IN left, IN right, OUT left, OUT right — in that order, which the meters
/// depend on.
pub const METERS: usize = 4;

/// The readout cells that come out of `Level`: direct and reflected.
pub const BUSES: usize = 2;

#[derive(Default)]
pub struct Analysis {
    /// Peak per meter, and the held peak beside it.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// The two buses' levels, in dB. **Side by side rather than as a ratio**:
    /// the ratio *is* the distance (`REQ-DIO-018`), and two numbers say which
    /// of them moved where one cannot.
    pub buses: Handoff<BUSES>,
    /// Where each reflection arrives and how loud it is.
    ///
    /// **Taken from the tap weights, not from `DEPTH`** — a figure computed
    /// from the parameter would agree with the sound only until somebody
    /// changed the window (`diorama_core::Reflections::pattern`).
    pub arrivals: Handoff<TAPS>,
    pub arrival_levels: Handoff<TAPS>,
    /// How far `CLARITY` is putting the presence band back, in dB.
    ///
    /// **A protection that works invisibly is a control that does nothing**
    /// (`REQ-DIO-006`): if the plugin is quietly undoing part of the distance
    /// the user asked for, the window says so and says how far.
    pub clarity: Handoff<1>,
    /// How alike the reflections' two channels are.
    ///
    /// **The width promise, measured** (`REQ-DIO-007`): the two channels share
    /// no tap time, so this is where that can be checked rather than taken on
    /// trust.
    pub correlation: Handoff<1>,
    /// The direct sound's damping corner, in Hz. **Zero means there is none** —
    /// the audio path is passing everything through, and writing 20 000 would
    /// be a lie (`diorama_core::Damping::direct_corner_hz`).
    pub damping: Handoff<1>,
}

#[cfg(test)]
mod tests {
    /// **Every handoff is written by the audio thread.**
    ///
    /// A handoff nobody writes is a display that never moves, and **nothing
    /// else notices**: it compiles, the editor binds to it, the heartbeat reads
    /// it thirty times a second, and every figure sits at zero while the plugin
    /// makes sound. Air shipped one build exactly that way when the whole
    /// publishing block was lost in an edit (`docs/HANDOVER.md`).
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
        assert_eq!(fields.len(), 8, "the handoff list moved: {fields:?}");

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
