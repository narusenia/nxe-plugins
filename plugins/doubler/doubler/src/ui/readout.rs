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

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;
use std::sync::Arc;

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

pub fn view(cx: &mut Context, analysis: Arc<Analysis>) {
    nxe_ui::readout::strip(cx, move |cx| {
        // Read inside the lens rather than copied into the model: the handoff's
        // identity never changes, and the heartbeat is a change to the model
        // thirty times a second.
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

        let width = analysis.clone();
        nxe_ui::readout::cell(
            cx,
            "CORRELATION",
            Ui::params.map(move |_| correlation(width.correlation.read()[0])),
            "",
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
