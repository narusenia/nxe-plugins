//! The band of figures under the header: what went in and out, what is holding
//! the layer back, and which of the three detectors is open.
//!
//! **Nothing new is measured.** Every figure here is something `Engine`
//! already publishes (`analysis.rs`) — anything the audio thread writes and
//! nobody reads is a cost already paid for no return (`.agents/rules/ui.md`).
//!
//! **The three detectors are separate cells, not one.** They multiply, so one
//! shut is the whole layer gone; a combined figure cannot say whether a dark
//! pad closed `BRIGHTNESS`, the amount is simply low, or the protection is
//! pulling (`REQ-AIR-018`).

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use air_core::follow::{BRIGHTNESS, ENVELOPE, TRANSIENT};
use nih_plug_vizia::vizia::prelude::*;
use std::sync::Arc;

/// Below this the number is not a level, it is a floor. `-142.0 dB` is six
/// characters of noise where a glance expects a level.
const SILENT_DB: f32 = METER_FLOOR_DB;

/// A level in dBFS, or a dash when there is nothing there.
///
/// **Always signed.** `-18.2` is five characters and `18.2` is four, so a
/// number that crosses zero shortens — and in a row laid out around it,
/// everything after it steps sideways.
fn level(amplitude: f32) -> String {
    let db = 20.0 * amplitude.max(1e-9).log10();
    if db <= SILENT_DB {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// How far the protection is pulling.
///
/// **Always negative, including at rest.** Reduction only goes one way, so the
/// sign carries no information — but a sign that appears the moment it starts
/// working makes the figure twitch, which reads as the plugin being unsure
/// (`.agents/rules/ui.md`).
fn pull(decibels: f32) -> String {
    if !decibels.is_finite() {
        return "-0.0".to_owned();
    }
    format!("-{:.1}", decibels.abs())
}

/// How alike the layer's two channels are.
///
/// **This is the promise, measured** (`REQ-AIR-008`). `+1.00` is one stream in
/// both channels and `0.00` is two independent ones; nothing in the path can
/// produce a negative reading, and if one ever appears something has started
/// rotating phase.
fn correlation(value: f32) -> String {
    if !value.is_finite() {
        return "+0.00".to_owned();
    }
    format!("{value:+.2}")
}

pub fn view(cx: &mut Context, analysis: Arc<Analysis>) {
    nxe_ui::readout::strip(cx, move |cx| {
        // **Read inside the lens, not copied into the model.** The handoff's
        // identity never changes, so mapping the `Arc` would tell the binding
        // system nothing; any change to the model re-evaluates this, and the
        // heartbeat is a change to the model thirty times a second.
        let input = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "IN",
            Ui::params.map(move |_| {
                let frame = input.peaks.read();
                level(frame[0].max(frame[1]))
            }),
            "dB",
        );

        let output = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "OUT",
            Ui::params.map(move |_| {
                let frame = output.peaks.read();
                level(frame[2].max(frame[3]))
            }),
            "dB",
        );

        let guard = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "GUARD",
            Ui::params.map(move |_| pull(guard.guard.read()[0])),
            "dB",
        );

        let width = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "WIDTH",
            Ui::params.map(move |_| correlation(width.correlation.read()[0])),
            "",
        );

        for (name, index) in [("ENV", ENVELOPE), ("BRT", BRIGHTNESS), ("TRN", TRANSIENT)] {
            let follow = analysis.clone();
            nxe_ui::readout::meter_cell(
                cx,
                name,
                Ui::params.map(move |_| follow.follow.read()[index].clamp(0.0, 1.0)),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_a_dash() {
        assert_eq!(level(0.0), "—");
        assert_eq!(level(1e-9), "—");
    }

    /// **Always signed**, so nothing laid out after it moves as the number
    /// crosses zero.
    ///
    /// The other two ways a reading changes width — a digit appearing at ten,
    /// and the dash — a format string cannot fix. `nxe_ui::readout::cell`'s
    /// fixed box and right alignment do.
    #[test]
    fn a_level_is_always_signed() {
        for amplitude in [1.0f32, 0.5, 0.1, 0.01] {
            let text = level(amplitude);
            assert!(text.starts_with('+') || text.starts_with('-'), "{text}");
        }
        assert_eq!(level(1.0), "+0.0");
    }

    /// The resting state is `-0.0`, and nothing about the figure changes when
    /// it stops being the resting state.
    #[test]
    fn the_pull_is_always_negative() {
        assert_eq!(pull(0.0), "-0.0");
        assert_eq!(pull(-3.25), "-3.2");
        assert_eq!(pull(f32::NAN), "-0.0");
    }

    #[test]
    fn the_correlation_is_always_signed() {
        assert_eq!(correlation(1.0), "+1.00");
        assert_eq!(correlation(0.0), "+0.00");
        assert_eq!(correlation(-0.5), "-0.50");
        assert_eq!(correlation(f32::INFINITY), "+0.00");
    }
}
