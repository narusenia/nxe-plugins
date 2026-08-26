//! The band of figures under the header: what is going in, what is coming out,
//! and what the two guards are holding back.
//!
//! **Nothing new is measured** (`SPK-19` did the same for Sparkleur). `IN` and
//! `OUT` are the meter frames the strip beside the window already draws, and
//! `HARSH` and `SIB` are the guard readings the figure already sinks the
//! regions by (`analysis.rs`).
//!
//! The guards are the reason this is worth a number here. `REQ-VEL-006` puts
//! them in the picture so that "the `AIR` knob does nothing" has a visible
//! cause — but a region sinking says *that* it is being held back, never *how
//! far*, and how far is what decides whether the setting is wrong or the
//! material is.

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;
use std::sync::Arc;

/// Below this the number is not a level, it is a floor.
const SILENT_DB: f32 = METER_FLOOR_DB;

/// A level in dBFS, or a dash when there is nothing there.
///
/// **Always signed**: `-18.2` is five characters and `18.2` is four, so a
/// reading that crosses zero would shorten. The box is fixed as well, for the
/// dash and for the extra digit past ten (`nxe_ui::readout`).
fn level(amplitude: f32) -> String {
    let db = 20.0 * amplitude.max(1e-9).log10();
    if db <= SILENT_DB {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// How far a guard is pulling.
///
/// **Always negative, including at rest.** A guard only ever goes one way, so
/// the sign carries nothing — but one that appears the moment the guard starts
/// working makes the row twitch, and these two sit at zero on ordinary material
/// by design (`docs/HANDOVER.md`).
fn pull(decibels: f32) -> String {
    if !decibels.is_finite() {
        return "-0.0".to_owned();
    }
    format!("-{:.1}", decibels.abs())
}

pub fn view(cx: &mut Context, analysis: Arc<Analysis>) {
    nxe_ui::readout::strip(cx, move |cx| {
        // Read inside the lens rather than copied into the model: the handoff's
        // identity never changes, and the heartbeat is a change to the model
        // thirty times a second (`ui/field.rs` does the same).
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

        let harsh = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "HARSH",
            Ui::params.map(move |_| pull(harsh.guards.read()[0])),
            "dB",
        );

        let sib = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "SIB",
            Ui::params.map(move |_| pull(sib.guards.read()[1])),
            "dB",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_silent_input_has_no_level_to_report() {
        assert_eq!(level(0.0), "—");
        assert_eq!(level(1.0), "+0.0");
        assert_eq!(level(0.5), "-6.0");
    }

    /// **The sign never comes and goes**, which is the twitch this exists to
    /// stop: a guard resting and a guard working differ in the digits, not in
    /// whether there is a minus in front of them.
    #[test]
    fn a_guard_reads_the_same_shape_resting_and_working() {
        for decibels in [0.0f32, -0.01, -3.4, -18.0, f32::NAN] {
            let reading = pull(decibels);
            assert!(
                reading.starts_with('-'),
                "{decibels} read as {reading}, with no sign"
            );
        }
        assert_eq!(pull(0.0), "-0.0");
        assert_eq!(pull(-3.4), "-3.4");
    }
}
