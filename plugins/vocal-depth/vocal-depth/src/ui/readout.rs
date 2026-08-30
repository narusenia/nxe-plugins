//! The band of figures under the header: what went in and out, where the voice
//! sits, and what the plugin is putting back.
//!
//! **Nothing new is measured.** Every figure here is something the engine
//! already publishes (`analysis.rs`) — anything the audio thread writes and
//! nobody reads is a cost already paid for no return (`.agents/rules/ui.md`).
//!
//! **`DIRECT` and `ROOM` are two cells, not a ratio.** The ratio *is* the
//! distance (`REQ-VDP-018`), and two numbers say which of them moved where one
//! cannot.
//!
//! **The figures are copied into the model, not read inside a lens** — see
//! `nxe_ui::readout` for why that is not the cheap way round it looks.

use super::{METER_FLOOR_DB, Ui};

/// Where each figure sits in `Ui::readouts`. **The order is the strip's.**
const IN: usize = 0;
const OUT: usize = 1;
const DIRECT: usize = 2;
const ROOM: usize = 3;
const CLARITY: usize = 4;
const CORR: usize = 5;
const HF: usize = 6;

/// How many figures the strip prints.
pub(crate) const FIGURES: usize = 7;
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;

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

/// A level already in dB, or a dash when there is nothing there.
fn decibels(value: f32) -> String {
    if !value.is_finite() || value <= SILENT_DB {
        "—".to_owned()
    } else {
        format!("{value:+.1}")
    }
}

/// How far `CLARITY` is putting the presence band back.
///
/// **Always signed, including at rest.** It only goes one way, so the sign
/// carries no information — but a sign that appears the moment it starts
/// working makes the figure twitch, which reads as the plugin being unsure
/// (`.agents/rules/ui.md`).
fn lift(value: f32) -> String {
    if !value.is_finite() {
        return "+0.0".to_owned();
    }
    format!("+{:.1}", value.abs())
}

/// How alike the reflections' two channels are.
///
/// **The width promise, measured** (`REQ-VDP-007`): the two tap sets share no
/// arrival time, so a reading near zero is the mechanism working. `+1.00` would
/// mean the reflections had collapsed to one stream.
fn correlation(value: f32) -> String {
    if !value.is_finite() {
        return "+0.00".to_owned();
    }
    format!("{value:+.2}")
}

/// The damping corner, or a dash when there is none.
///
/// **A dash rather than `20000`.** At `DAMPING` = 0 the audio path passes
/// everything through (`Coefficients::PASS`), so printing a corner would be a
/// number for something that is not happening.
fn corner(hz: f32) -> String {
    if !hz.is_finite() || hz <= 0.0 {
        "—".to_owned()
    } else if hz >= 1_000.0 {
        format!("{:.1}k", hz / 1_000.0)
    } else {
        format!("{hz:.0}")
    }
}

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn poll(analysis: &Analysis, figures: &mut [String]) {
    let peaks = analysis.peaks.read();
    figures[IN] = level(peaks[0].max(peaks[1]));
    figures[OUT] = level(peaks[2].max(peaks[3]));

    let buses = analysis.buses.read();
    figures[DIRECT] = decibels(buses[0]);
    figures[ROOM] = decibels(buses[1]);
    figures[CLARITY] = lift(analysis.clarity.read()[0]);
    figures[CORR] = correlation(analysis.correlation.read()[0]);
    figures[HF] = corner(analysis.damping.read()[0]);
}

pub fn view(cx: &mut Context) {
    nxe_ui::readout::strip(cx, |cx| {
        nxe_ui::readout::cell(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::readout::cell(cx, "OUT", Ui::readouts.index(OUT), "dB");
        nxe_ui::readout::cell(cx, "DIRECT", Ui::readouts.index(DIRECT), "dB");
        nxe_ui::readout::cell(cx, "ROOM", Ui::readouts.index(ROOM), "dB");
        nxe_ui::readout::cell(cx, "CLARITY", Ui::readouts.index(CLARITY), "dB");
        nxe_ui::readout::cell(cx, "CORR", Ui::readouts.index(CORR), "");
        nxe_ui::readout::cell(cx, "HF", Ui::readouts.index(HF), "Hz");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Silence is a dash, not a number (`.agents/rules/ui.md`).
    #[test]
    fn silence_reads_as_a_dash() {
        assert_eq!(level(0.0), "—");
        assert_eq!(decibels(f32::NEG_INFINITY), "—");
        assert_eq!(corner(0.0), "—");
    }

    /// Every figure keeps its width, which is what stops the row after it from
    /// stepping sideways (`.agents/rules/ui.md`).
    #[test]
    fn the_figures_keep_their_width() {
        assert_eq!(level(0.5).len(), decibels(-6.0).len());
        assert_eq!(lift(0.0), "+0.0");
        assert_eq!(lift(3.25), "+3.2");
        assert_eq!(correlation(0.0), "+0.00");
        assert_eq!(correlation(-0.5), "-0.50");
    }

    /// A non-finite reading is a bug somewhere else, and the window's job is to
    /// stay legible rather than to print `NaN`.
    #[test]
    fn nothing_prints_a_nan() {
        for text in [
            level(f32::NAN),
            decibels(f32::NAN),
            lift(f32::NAN),
            correlation(f32::NAN),
            corner(f32::NAN),
        ] {
            assert!(!text.contains("NaN"), "{text}");
        }
    }

    /// The corner reads in kilohertz where a corner actually sits, so the cell
    /// does not have to hold five digits.
    #[test]
    fn a_corner_reads_in_kilohertz() {
        assert_eq!(corner(8_700.0), "8.7k");
        assert_eq!(corner(400.0), "400");
    }
}
