//! What the window is told: `Handoff`s the audio thread writes once a block
//! and the window reads when it draws (`docs/specifications/architecture.md`).
//!
//! The one thing going the other way is [`Analysis::clears`]: the window asks
//! for the maxima to be forgotten by counting up, and the audio thread does it
//! (`peek-plan.md`, decision 6). The window never touches the meters.

use nxe_dsp::Handoff;
use std::sync::atomic::AtomicU32;

/// Left and right. A mono input arrives in both.
pub const CHANNELS: usize = 2;

#[derive(Default)]
pub struct Analysis {
    /// The moving readings per channel, as amplitudes.
    pub peaks: Handoff<CHANNELS>,
    pub holds: Handoff<CHANNELS>,
    pub rms: Handoff<CHANNELS>,
    /// Since the last clear, across both channels: sample peak, true peak,
    /// RMS.
    pub maxima: Handoff<3>,
    /// How many times the window has asked for the maxima to be cleared.
    pub clears: AtomicU32,
}

#[cfg(test)]
mod tests {
    /// **Every handoff is written by the audio thread** — a handoff nobody
    /// writes is a meter that never moves, and nothing else notices
    /// (`air/src/analysis.rs` has the history).
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
        assert_eq!(fields.len(), 4, "the handoff list moved: {fields:?}");
        let plugin: String = PLUGIN.chars().filter(|c| !c.is_whitespace()).collect();
        for field in fields {
            assert!(
                plugin.contains(&format!("analysis.{field}.write")),
                "{field} is never published"
            );
        }
    }
}
