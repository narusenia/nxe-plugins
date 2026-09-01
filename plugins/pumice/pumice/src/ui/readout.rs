//! The band of figures under the header, and the strip along the bottom.
//!
//! **Nothing new is measured.** Every figure here is something the engine
//! already publishes (`analysis.rs`) — anything the audio thread writes and
//! nobody reads is a cost already paid for no return (`.agents/rules/ui.md`).
//!
//! **There is no band of figures under the header.** There was, and it printed
//! `REDUCTION` / `IN` / `OUT` at the headline size — the same three the strip
//! along the bottom already carries. One window does not say the same thing
//! twice, and the row it took back is 40 px the figure now has.
//!
//! **The status bar prints the node under the pointer.** The figure cannot label
//! its own points — vizia's `draw_text` only renders a view's own text — so the
//! bar is where a node's frequency, width and depth are read (`nxe_ui::node`).
//! This is the first window in the line where the strip carries something the
//! figure could not say itself.

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use crate::params;
use nih_plug_vizia::vizia::prelude::*;

/// Where each figure sits in `Ui::readouts`. **The order is the strip's.**
const IN: usize = 0;
const OUT: usize = 1;
const REDUCTION: usize = 2;

/// How many figures the strip prints.
pub(crate) const FIGURES: usize = 3;

/// A level in dBFS, or a dash when there is nothing there.
///
/// **Always signed.** `-18.2` is five characters and `18.2` is four, so a
/// number that crosses zero shortens — and in a row laid out around it,
/// everything after it steps sideways.
fn level(amplitude: f32) -> String {
    let db = 20.0 * amplitude.max(1e-9).log10();
    if !db.is_finite() || db <= METER_FLOOR_DB {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// How much is being taken out. Already negative, and a dash when it is none.
fn reduction(db: f32) -> String {
    if !db.is_finite() || db > -0.05 {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn figures(analysis: &Analysis, peaks: &[f32]) -> Vec<String> {
    let mut out = vec![String::new(); FIGURES];
    out[IN] = level(
        peaks
            .first()
            .copied()
            .unwrap_or(0.0)
            .max(peaks.get(1).copied().unwrap_or(0.0)),
    );
    out[OUT] = level(
        peaks
            .get(2)
            .copied()
            .unwrap_or(0.0)
            .max(peaks.get(3).copied().unwrap_or(0.0)),
    );
    out[REDUCTION] = reduction(analysis.readouts.read()[0]);
    out
}

/// The strip at the foot of the window.
///
/// **The hovered node goes on the left, where the hover's description goes**
/// (`nxe_ui::status`). A node has no name to describe, so what it gets is its
/// own three numbers — which is the one thing the figure cannot draw.
pub fn status(cx: &mut Context) {
    nxe_ui::status::bar(cx, |cx| {
        nxe_ui::status::figure(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::status::figure(cx, "OUT", Ui::readouts.index(OUT), "dB");
        nxe_ui::status::figure(cx, "GR", Ui::readouts.index(REDUCTION), "dB");
    });
}

/// What the pointer is on, as a sentence for the strip.
///
/// **Built from the parameters, not from the figure's copy.** The figure holds
/// positions; these are the values, printed the way the parameters print them.
pub(crate) fn hovered_text(ui: &Ui) -> String {
    let Some(index) = ui.hovered else {
        return String::new();
    };
    // The figure only draws the nodes that are on, so its indices are into that
    // list rather than into all six.
    let Some(node) = ui
        .params
        .nodes
        .iter()
        .filter(|node| node.enabled.value())
        .nth(index)
    else {
        return String::new();
    };

    let hz = params::position_to_hz(node.freq.value());
    let octaves = params::position_to_octaves(node.width.value());
    let depth = node.depth.value();
    let frequency = if hz >= 1_000.0 {
        format!("{:.2} kHz", hz / 1_000.0)
    } else {
        format!("{hz:.0} Hz")
    };
    format!("{frequency} · {octaves:.2} oct · {:+.0} %", depth * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Silence is a dash, not a number (`.agents/rules/ui.md`).
    #[test]
    fn silence_reads_as_a_dash() {
        assert_eq!(level(0.0), "—");
        assert_eq!(reduction(0.0), "—");
        assert_eq!(reduction(f32::NEG_INFINITY), "—");
    }

    /// Every figure keeps its width, which is what stops the row after it from
    /// stepping sideways (`.agents/rules/ui.md`).
    #[test]
    fn the_figures_keep_their_width() {
        assert_eq!(level(0.5).len(), reduction(-6.0).len());
        assert_eq!(reduction(-3.25), "-3.2");
    }

    #[test]
    fn nothing_prints_a_nan() {
        for text in [level(f32::NAN), reduction(f32::NAN)] {
            assert!(!text.contains("NaN"), "{text}");
        }
    }
}
