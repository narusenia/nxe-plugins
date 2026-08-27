//! The figure: what came in as a line, what was added as grains.
//!
//! **Only an additive plugin can draw this** (`ui.md`). The layer is a signal
//! of its own, so it can be shown beside the source rather than inferred from
//! what the source became — and it comes from `Engine::layer` rather than from
//! `out − dry`, which throws away most of its precision when the source is
//! louder (`docs/HANDOVER.md`).

use super::Ui;
use crate::analysis::{HIGH_HZ, LOW_HZ};
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::dots::DotField;
use nxe_ui::theme;

/// **The same height as every other plugin's figure.** Two figures tuned
/// separately are each right on their own and wrong beside each other
/// (`.agents/rules/ui.md`).
pub const HEIGHT: f32 = 200.0;

/// The axis labels' own line, under the plot.
pub const MARKS_HEIGHT: f32 = theme::LINE_VALUE;

const MARKS: [(f32, &str); 6] = [
    (20.0, "20"),
    (200.0, "200"),
    (1_000.0, "1k"),
    (4_000.0, "4k"),
    (10_000.0, "10k"),
    (20_000.0, "20k"),
];

/// Hz onto `0..=1` across the view, logarithmically.
pub fn axis_x(hz: f32) -> f32 {
    (hz.max(LOW_HZ) / LOW_HZ).log10() / (HIGH_HZ / LOW_HZ).log10()
}

pub fn view(cx: &mut Context) {
    VStack::new(cx, |cx| {
        // **`alignment` is `BLEND`, inverted.** At the harmonic end the layer
        // is made from the source and lands on its harmonics, so the grains
        // stand in columns; at the noise end there is nothing to line up with
        // and they scatter (`ui.md`).
        DotField::new(
            cx,
            Ui::dry,
            Ui::layer,
            Ui::params.map(|params| 1.0 - params.blend.value()),
        )
        .height(Pixels(HEIGHT - MARKS_HEIGHT - theme::SPACE_1))
        .width(Stretch(1.0));

        // The axis labels are the caller's, placed with the caller's own
        // mapping — **a custom view cannot draw text at arbitrary positions**
        // (`.agents/rules/vizia.md`).
        HStack::new(cx, |cx| {
            for (hz, text) in MARKS {
                let label = Label::new(cx, text)
                    .class("subtle")
                    .class("decoration")
                    .position_type(PositionType::SelfDirected)
                    .height(Pixels(MARKS_HEIGHT));
                // **The last one is hung off the right.** `left: 100%` is where
                // a label *starts*, so "20k" renders as "20" with the rest
                // outside the window (`.agents/rules/vizia.md`).
                if hz >= HIGH_HZ {
                    label.left(Stretch(1.0)).right(Pixels(0.0));
                } else {
                    label.left(Percentage(axis_x(hz) * 100.0));
                }
            }
        })
        .height(Pixels(MARKS_HEIGHT))
        .width(Stretch(1.0));
    })
    // **Not `.class("row")`**, which centres its children vertically and hands
    // the height two more stretches to divide (`.agents/rules/vizia.md`).
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_spans_the_view() {
        assert_eq!(axis_x(LOW_HZ), 0.0);
        assert!((axis_x(HIGH_HZ) - 1.0).abs() < 1e-5);
        // Logarithmic: a decade is a third of the three-decade span.
        assert!((axis_x(200.0) - 1.0 / 3.0).abs() < 1e-5);
    }

    /// Every mark is inside the plot, and they are in order.
    #[test]
    fn the_marks_are_in_order_and_inside() {
        let positions: Vec<f32> = MARKS.iter().map(|(hz, _)| axis_x(*hz)).collect();
        for pair in positions.windows(2) {
            assert!(pair[1] > pair[0], "{positions:?}");
        }
        for position in &positions {
            assert!((0.0..=1.0).contains(position), "{position}");
        }
    }
}
