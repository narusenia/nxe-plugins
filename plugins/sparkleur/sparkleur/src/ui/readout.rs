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

use super::{METER_FLOOR_DB, Ui};
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::{font, theme};
use sparkleur_core::crossover::BAND_COUNT;
use std::sync::Arc;

/// Below this the number is not a level, it is a floor. Saying `-142.0 dB` for
/// silence is worse than saying nothing: it is six characters of noise where a
/// glance expects a level.
const SILENT_DB: f32 = METER_FLOOR_DB;

/// Room for the widest reading — a sign, two digits, a point and a decimal — in
/// the mono face at [`theme::FONT_READOUT`].
const VALUE_WIDTH: f32 = 44.0;

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

pub fn view(cx: &mut Context, analysis: Arc<Analysis>) {
    let peaks = analysis.clone();
    let gains = analysis;

    HStack::new(cx, |cx| {
        // **Read inside the lens, not copied into the model.** The handoff's
        // identity never changes, so mapping the `Arc` would tell the binding
        // system nothing; any change to the model re-evaluates this, and the
        // heartbeat is a change to the model thirty times a second
        // (`ui/field.rs` does the same).
        let input = peaks.clone();
        cell(
            cx,
            "IN",
            Ui::params.map(move |_| {
                let frame = input.peaks.read();
                level(frame[0].max(frame[1]))
            }),
        );

        let output = peaks;
        cell(
            cx,
            "OUT",
            Ui::params.map(move |_| {
                let frame = output.peaks.read();
                level(frame[2].max(frame[3]))
            }),
        );

        cell(
            cx,
            "REDUCTION",
            Ui::params.map(move |_| reduction(&gains.gains.read())),
        );
    })
    .height(Auto)
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_4));
}

/// One figure under its own eyebrow and rule — the same shape every named
/// region in this design takes (`crates/nxe-ui/README.md`).
fn cell(cx: &mut Context, name: &'static str, value: impl Res<String> + Clone) {
    VStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            Label::new(cx, name).class("eyebrow");
        })
        .class("heading");

        HStack::new(cx, |cx| {
            // **A fixed box, not an auto one.** The dash for silence is one
            // character and a level is five; without a width the unit beside it
            // walks left and right as the signal comes and goes.
            font::value(cx, value)
                .class("readout")
                .width(Pixels(VALUE_WIDTH))
                .height(Auto)
                // **Right, not left.** The box is fixed so the unit beside it
                // cannot move, but the reading still gains a digit crossing ten
                // decibels — right-aligned, that digit grows into the padding
                // and the decimal point stays where the eye left it.
                .text_align(TextAlign::Right);
            Label::new(cx, "dB").class("subtle").top(Stretch(1.0));
        })
        .height(Auto)
        .width(Auto)
        .col_between(Pixels(theme::SPACE_1));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1));
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
