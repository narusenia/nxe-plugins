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
//!
//! **The figures are copied into the model, not read inside a lens** — see
//! `nxe_ui::readout` for why that is not the cheap way round it looks.

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use air_core::follow::{BRIGHTNESS, ENVELOPE, TRANSIENT};
use nih_plug_vizia::vizia::prelude::*;

/// Where each figure sits in `Ui::readouts`. **The order is the strip's**, so
/// the constants and the cells below cannot drift apart without the window
/// showing it.
const IN: usize = 0;
const OUT: usize = 1;
const GUARD: usize = 2;
const WIDTH: usize = 3;

/// How many figures the strip prints, and how many bars it draws.
pub(crate) const FIGURES: usize = 4;
pub(crate) const GAUGES: usize = 3;

/// The three detectors, in the order they are drawn.
const DETECTORS: [(&str, usize); GAUGES] =
    [("ENV", ENVELOPE), ("BRT", BRIGHTNESS), ("TRN", TRANSIENT)];

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

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn poll(analysis: &Analysis, figures: &mut [String], gauges: &mut [f32]) {
    let peaks = analysis.peaks.read();
    figures[IN] = level(peaks[0].max(peaks[1]));
    figures[OUT] = level(peaks[2].max(peaks[3]));
    figures[GUARD] = pull(analysis.guard.read()[0]);
    figures[WIDTH] = correlation(analysis.correlation.read()[0]);

    let follow = analysis.follow.read();
    for (gauge, (_, index)) in gauges.iter_mut().zip(DETECTORS) {
        *gauge = follow[index].clamp(0.0, 1.0);
    }
}

pub fn view(cx: &mut Context) {
    nxe_ui::readout::strip(cx, |cx| {
        nxe_ui::readout::cell(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::readout::cell(cx, "OUT", Ui::readouts.index(OUT), "dB");
        nxe_ui::readout::cell(cx, "GUARD", Ui::readouts.index(GUARD), "dB");
        nxe_ui::readout::cell(cx, "WIDTH", Ui::readouts.index(WIDTH), "");

        for (position, (name, _)) in DETECTORS.iter().enumerate() {
            nxe_ui::readout::meter_cell(cx, name, Ui::gauges.index(position));
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
