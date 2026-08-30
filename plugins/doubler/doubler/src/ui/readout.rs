//! The band of figures under the header: what is going in, what is coming out,
//! and whether the width survives a mono fold.
//!
//! **The Doubler had no metering at all.** Its two siblings both carry an in/out
//! strip; this one shipped with a picture of where the sound is and no way to
//! see how loud it got — and a doubler always changes the level, because it adds
//! copies of the signal to itself.
//!
//! `CORRELATION` is the number this plugin in particular owes its user
//! (`nxe_dsp::Correlation`). Every way it makes an image — detuning a copy,
//! delaying it, panning the pair apart — works by making the channels differ,
//! and the far end of that is a pair that cancels when summed to mono. The
//! Voice Field says the image is wide; it cannot say whether the width
//! survives.
//!
//! **The figures are copied into the model, not read inside a lens** — see
//! `nxe_ui::readout` for why that is not the cheap way round it looks.

use super::{METER_FLOOR_DB, Ui};

/// Where each figure sits in `Ui::readouts`. **The order is the strip's.**
const IN: usize = 0;
const OUT: usize = 1;
const CORRELATION: usize = 2;

/// How many figures the strip prints.
pub(crate) const FIGURES: usize = 3;
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;

/// Below this the number is not a level, it is a floor.
const SILENT_DB: f32 = METER_FLOOR_DB;

/// A level in dBFS, or a dash when there is nothing there.
///
/// **Always signed**, so the reading does not shorten as it crosses zero; the
/// box is fixed as well, for the dash and for the extra digit past ten
/// (`nxe_ui::readout`).
fn level(amplitude: f32) -> String {
    let db = 20.0 * amplitude.max(1e-9).log10();
    if db <= SILENT_DB {
        "—".to_owned()
    } else {
        format!("{db:+.1}")
    }
}

/// The correlation, `-1..=1`.
///
/// **Always signed, and the sign is the whole message**: below zero the two
/// channels are pulling against each other and a mono fold will take level off
/// the result. Two decimals, because the interesting range is narrow — the
/// difference between `+0.90` and `+0.70` is the difference between a widened
/// source and a barely widened one.
fn correlation(value: f32) -> String {
    if !value.is_finite() {
        return "+0.00".to_owned();
    }
    format!("{:+.2}", value.clamp(-1.0, 1.0))
}

/// Re-reads the handoff and rewrites the strip's figures. Called on the
/// heartbeat, which is the only thing that should move them.
pub(crate) fn poll(analysis: &Analysis, figures: &mut [String]) {
    let peaks = analysis.peaks.read();
    figures[IN] = level(peaks[0].max(peaks[1]));
    figures[OUT] = level(peaks[2].max(peaks[3]));
    figures[CORRELATION] = correlation(analysis.correlation.read()[0]);
}

/// The strip at the foot of the window: the hover's one-line description on
/// the left, these on the right (`nxe_ui::status`).
///
/// **`CORRELATION` is `CORR` here.** Every character is width the sentence
/// beside it does not get (`SPK-23`).
pub fn status(cx: &mut Context) {
    nxe_ui::status::bar(cx, |cx| {
        nxe_ui::status::figure(cx, "IN", Ui::readouts.index(IN), "dB");
        nxe_ui::status::figure(cx, "OUT", Ui::readouts.index(OUT), "dB");
        nxe_ui::status::figure(cx, "CORR", Ui::readouts.index(CORRELATION), "");
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

    /// **The sign never comes and goes**, and it is the part that matters: a
    /// negative reading is the warning this figure exists to give.
    #[test]
    fn the_correlation_always_carries_its_sign_and_stays_in_range() {
        assert_eq!(correlation(1.0), "+1.00");
        assert_eq!(correlation(0.0), "+0.00");
        assert_eq!(correlation(-1.0), "-1.00");
        // Out of range and hostile values do not escape the scale.
        assert_eq!(correlation(2.5), "+1.00");
        assert_eq!(correlation(-2.5), "-1.00");
        assert_eq!(correlation(f32::NAN), "+0.00");

        // Every reading is the same width, so nothing beside it moves.
        let widths: Vec<usize> = [-1.0f32, -0.5, 0.0, 0.5, 1.0]
            .iter()
            .map(|value| correlation(*value).len())
            .collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] == pair[1]),
            "the reading changed width: {widths:?}"
        );
    }
}
