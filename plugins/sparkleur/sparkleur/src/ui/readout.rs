//! The band of figures under the header: what is going in, what is coming out,
//! and how much is being taken off.
//!
//! **The three numbers a compressor is read by**, and none of them were on
//! screen. The meters said "about here" with a bar; the figure said "this band
//! sank" with a region. Neither answers "how much", which is the question asked
//! while deciding whether a setting is too much — and the one that gets asked
//! against a number rather than against a picture.
//!
//! **Nothing new is measured.** `IN` and `OUT` are the meter frames the strip
//! beside the window already draws, and `REDUCTION` is the deepest of the
//! per-band gains the figure already moves the regions by (`analysis.rs`).
//!
//! `SPARKLE` and `DE-HARSH` were **published every block and read by nothing**
//! (`SPK-19`). The gate is what separates this from a static exciter
//! (`REQ-SPK-007`) and there was no way to see it working at all; the guard's
//! doc says a protection that works invisibly leaves a user with an `AIR` knob
//! that does nothing and no way to find out why (`REQ-SPK-008`) — the figure
//! sinks the region it holds back, but never said how far.
//!
//! **The figures are copied into the model, not read inside a lens** — see
//! `nxe_ui::readout` for why that is not the cheap way round it looks.

use super::{METER_FLOOR_DB, Ui};

/// Where each figure sits in `Ui::readouts`. **The order is the strip's.**
const IN: usize = 0;
const OUT: usize = 1;
const REDUCTION: usize = 2;
const DE_HARSH: usize = 3;

/// How many figures the strip prints, and how many bars it draws.
pub(crate) const FIGURES: usize = 4;
pub(crate) const GAUGES: usize = 1;

/// The gate's bar, the one cell that is not a figure.
const SPARKLE: usize = 0;
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;
use sparkleur_core::crossover::BAND_COUNT;

/// Below this the number is not a level, it is a floor. Saying `-142.0 dB` for
/// silence is worse than saying nothing: it is six characters of noise where a
/// glance expects a level.
const SILENT_DB: f32 = METER_FLOOR_DB;

/// A level in dBFS, or a dash when there is nothing there.
///
/// **Always signed.** `-18.2` is five characters and `18.2` is four, so a
/// number that crosses zero shortens — and in a row that is laid out around it,
/// everything after it steps sideways. The sign is always printed so the width
/// never changes; [`cell`] fixes the box as well, for the dash.
fn level(amplitude: f32) -> String {
    let db = 20.0 * amplitude.max(1e-9).log10();
    if db <= SILENT_DB {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// The deepest cut any band is taking, in dB.
///
/// **The deepest, not the sum or the mean.** A five-band compressor's answer to
/// "how hard is it working" is the band working hardest; averaging it with four
/// bands doing nothing reports a number that is true of no band.
///
/// **Always negative, including at rest.** Reduction only ever goes one way, so
/// the sign carries no information — but a sign that appears and disappears as
/// the first band starts working makes the figure twitch, which reads as the
/// plugin being unsure. `-0.0` is the resting state, and nothing moves when it
/// stops being the resting state.
fn reduction(gains: &[f32; BAND_COUNT]) -> String {
    let deepest = gains.iter().copied().fold(0.0f32, f32::min);
    format!("-{:.1}", deepest.abs())
}

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn poll(analysis: &Analysis, figures: &mut [String], gauges: &mut [f32]) {
    let peaks = analysis.peaks.read();
    figures[IN] = level(peaks[0].max(peaks[1]));
    figures[OUT] = level(peaks[2].max(peaks[3]));
    figures[REDUCTION] = reduction(&analysis.gains.read());
    // Always signed, for the same reason `reduction` is — and more so: this one
    // sits at zero on ordinary material by design (`SPK-18`), so the sign would
    // otherwise appear only on the rare occasion it has something to say.
    figures[DE_HARSH] = format!("-{:.1}", analysis.de_harsh.read()[0].abs());

    gauges[SPARKLE] = analysis.sparkle.read()[0];
}

/// The figures, on the right of the status bar.
///
/// **They were a strip of their own under the header** at the headline size,
/// and that was a lot of window for five short numbers — it pushed the figure
/// the plugin exists to show down past it. On one line at the bottom they take
/// no height that was not already spent, and the right of the bar was empty
/// (`SPK-23`, looked at in a host).
pub fn status(cx: &mut Context) {
    nxe_ui::status::bar(cx, |cx| {
        nxe_ui::status::figure(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::status::figure(cx, "OUT", Ui::readouts.index(OUT), "dB");
        // **`GR`, not `REDUCTION`.** The strip shares its line with the
        // sentence about whatever the pointer is on, and a long name pushed
        // that sentence into the figures. `GR` is what a compressor's reduction
        // meter is called everywhere else (`SPK-23`).
        nxe_ui::status::figure(cx, "GR", Ui::readouts.index(REDUCTION), "dB");

        // **The gate, as a bar rather than a number.** What is asked of it is
        // "is it lighting up on this material", which a moving bar answers at a
        // glance and a figure flickering between 0 and 100 does not.
        nxe_ui::status::gauge(cx, "SPARK", Ui::gauges.index(SPARKLE));

        nxe_ui::status::figure(cx, "HARSH", Ui::readouts.index(DE_HARSH), "dB");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Silence reads as a dash rather than as a very large negative number.
    #[test]
    fn a_silent_input_has_no_level_to_report() {
        assert_eq!(level(0.0), "—");
        assert_eq!(level(1e-9), "—");
        // And a real level is a real number.
        assert_eq!(level(1.0), "+0.0");
        assert_eq!(level(0.5), "-6.0");
    }

    /// The deepest band, not an average that is true of none of them.
    #[test]
    fn the_reduction_is_the_band_working_hardest() {
        assert_eq!(reduction(&[0.0, -6.2, 0.0, -1.1, 0.0]), "-6.2");
        // Upward compression is not reduction: a band being lifted must not
        // report as a cut.
        assert_eq!(reduction(&[2.0, 3.0, 1.0, 0.5, 4.0]), "-0.0");

        // **The sign never comes and goes**, which is the twitch this exists to
        // stop: resting and working differ in the digits, not in whether there
        // is a minus in front of them.
        for gains in [
            [0.0f32; BAND_COUNT],
            [-0.01, 0.0, 0.0, 0.0, 0.0],
            [0.0, -6.2, 0.0, -1.1, 0.0],
            [-12.5, 0.0, 0.0, 0.0, 0.0],
        ] {
            let reading = reduction(&gains);
            assert!(
                reading.starts_with('-'),
                "{gains:?} read as {reading}, with no sign"
            );
        }

        // **The width still changes crossing ten decibels**, and no format
        // string fixes that. The box is a fixed width and the reading is set
        // right-aligned inside it (see `cell`), so the extra digit grows into
        // the padding instead of pushing the unit along.
        assert_eq!(reduction(&[-6.2, 0.0, 0.0, 0.0, 0.0]).len(), 4);
        assert_eq!(reduction(&[-12.5, 0.0, 0.0, 0.0, 0.0]).len(), 5);
    }
}
