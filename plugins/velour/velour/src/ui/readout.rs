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
//!
//! **The figures are copied into the model, not read inside a lens** — see
//! `nxe_ui::readout` for why that is not the cheap way round it looks.

use super::{METER_FLOOR_DB, Ui};

/// Where each figure sits in `Ui::readouts`. **The order is the strip's.**
const IN: usize = 0;
const OUT: usize = 1;
const HARSH: usize = 2;
const SIB: usize = 3;

/// How many figures the strip prints.
pub(crate) const FIGURES: usize = 4;
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;

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

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn poll(analysis: &Analysis, figures: &mut [String]) {
    let peaks = analysis.peaks.read();
    figures[IN] = level(peaks[0].max(peaks[1]));
    figures[OUT] = level(peaks[2].max(peaks[3]));

    let guards = analysis.guards.read();
    figures[HARSH] = pull(guards[0]);
    figures[SIB] = pull(guards[1]);
}

pub fn view(cx: &mut Context) {
    nxe_ui::readout::strip(cx, |cx| {
        nxe_ui::readout::cell(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::readout::cell(cx, "OUT", Ui::readouts.index(OUT), "dB");
        nxe_ui::readout::cell(cx, "HARSH", Ui::readouts.index(HARSH), "dB");
        nxe_ui::readout::cell(cx, "SIB", Ui::readouts.index(SIB), "dB");
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
