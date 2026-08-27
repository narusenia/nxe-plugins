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
//! **Two spectra, which only an additive plugin can publish** (`REQ-AIR-018`).
//! Sparkleur has one because a split topology has no separable added layer
//! (`REQ-SPK-018`); Air's layer is a signal of its own, taken straight from
//! `Engine::layer` rather than by subtracting the dry back out — a subtraction
//! throws away most of the layer's precision when the source is louder
//! (`docs/HANDOVER.md`).

use air_core::follow::DETECTORS;
use nxe_dsp::Handoff;

/// Bands across the audible range, and the number of columns the grain field
/// draws.
///
/// **32, the same as Sparkleur.** Three bands per octave is enough to read as
/// "where the layer sits", and there are two of these rather than one — the
/// cheapest thing to halve if `AIR-12` finds the budget tight.
pub const BANDS: usize = 32;

/// The range the spectra cover, matching the figure's own axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// IN left, IN right, OUT left, OUT right — in that order, which the meters
/// depend on.
pub const METERS: usize = 4;

#[derive(Default)]
pub struct Analysis {
    /// What came in. The ground the layer is read against.
    pub dry: Handoff<BANDS>,
    /// **The subject**: what was added, on its own.
    pub layer: Handoff<BANDS>,
    /// Peak per meter, and the held peak beside it.
    pub peaks: Handoff<METERS>,
    pub holds: Handoff<METERS>,
    /// The three following coefficients, separately.
    ///
    /// **They multiply**, so one shut is the whole layer gone — and a single
    /// combined figure cannot say whether a dark pad closed `BRIGHTNESS`, the
    /// amount is simply low, or the protection is pulling (`REQ-AIR-018`).
    pub follow: Handoff<DETECTORS>,
    /// How far the protection is pulling, in dB. Zero when it is doing nothing.
    ///
    /// **A protection that works invisibly is a control that does nothing**
    /// (`REQ-AIR-009`): Air's can only reach its own layer, so pulled hard the
    /// layer disappears, and a layer that disappeared without saying why reads
    /// as a broken plugin.
    pub guard: Handoff<1>,
    /// How alike the layer's two channels are.
    ///
    /// **This is the promise, measured** (`REQ-AIR-008`): nothing in the path
    /// rotates phase, so the fold cannot comb — and the reading is where that
    /// can be checked rather than taken on trust.
    pub correlation: Handoff<1>,
}

#[cfg(test)]
mod tests {
    /// **Every handoff is written by the audio thread.**
    ///
    /// A handoff nobody writes is a display that never moves, and **nothing
    /// else notices**: it compiles, the editor binds to it, the heartbeat reads
    /// it thirty times a second, and every figure sits at zero while the plugin
    /// makes sound. That shipped once — the whole publishing block was lost in
    /// an edit and the window looked exactly like a plugin with no signal.
    ///
    /// The mirror of Sparkleur's finding, which was two handoffs **written and
    /// read by nothing** (`SPK-19`). Both directions are silent, so both are
    /// worth a test.
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
        assert_eq!(fields.len(), 7, "the handoff list moved: {fields:?}");

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
