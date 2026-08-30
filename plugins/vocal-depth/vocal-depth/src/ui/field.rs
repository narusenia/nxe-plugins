//! The figure: where every arrival lands, against time.
//!
//! **One axis, and it is both time and distance** (`ui.md`). The direct sound
//! stands at the left edge; each reflection stands where it arrives. Moving the
//! voice away moves the weight of the picture to the right.
//!
//! **Drawn from the tap weights, not from `DEPTH`.** A figure computed from the
//! parameter would agree with the sound only until somebody changed the window
//! (`vocal_depth_core::Reflections::pattern`).

use super::Ui;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::taps::TapField;
use nxe_ui::theme;

/// **The same height as every other plugin's figure.** Two figures tuned
/// separately are each right on their own and wrong beside each other
/// (`.agents/rules/ui.md`).
pub const HEIGHT: f32 = 200.0;

/// The axis labels' own line, under the plot.
pub const MARKS_HEIGHT: f32 = theme::LINE_VALUE;

/// The span the figure covers, which is the span the reflections live in
/// (`REQ-VDP-003`).
pub const SPAN_MS: f32 = 120.0;

const MARKS: [(f32, &str); 7] = [
    (0.0, "0"),
    (20.0, "20"),
    (40.0, "40"),
    (60.0, "60"),
    (80.0, "80"),
    (100.0, "100"),
    (120.0, "120 ms"),
];

/// Milliseconds onto `0..=1` across the view. **Linear**, because time is —
/// a log axis here would compress exactly the part the ear reads as distance.
pub fn axis_x(milliseconds: f32) -> f32 {
    (milliseconds / SPAN_MS).clamp(0.0, 1.0)
}

pub fn view(cx: &mut Context) {
    VStack::new(cx, |cx| {
        TapField::new(cx, Ui::arrivals, Ui::direct)
            .height(Pixels(HEIGHT - MARKS_HEIGHT - theme::SPACE_1))
            .width(Stretch(1.0));

        // The axis labels are the caller's, placed with the caller's own
        // mapping — **a custom view cannot draw text at arbitrary positions**
        // (`.agents/rules/vizia.md`).
        HStack::new(cx, |cx| {
            for (ms, text) in MARKS {
                let label = Label::new(cx, text)
                    .class("subtle")
                    .class("decoration")
                    .position_type(PositionType::SelfDirected)
                    .height(Pixels(MARKS_HEIGHT));
                // **The last one is hung off the right.** `left: 100%` is where
                // a label *starts*, so "120 ms" renders as "1" with the rest
                // outside the window (`.agents/rules/vizia.md`).
                if ms >= SPAN_MS {
                    label.left(Stretch(1.0)).right(Pixels(0.0));
                } else {
                    label.left(Percentage(axis_x(ms) * 100.0));
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
        assert_eq!(axis_x(0.0), 0.0);
        assert_eq!(axis_x(SPAN_MS), 1.0);
        // Linear: half the span is half the width.
        assert!((axis_x(SPAN_MS / 2.0) - 0.5).abs() < 1e-6);
    }

    /// Every mark is inside the plot, and they are in order.
    #[test]
    fn the_marks_are_in_order_and_inside() {
        let positions: Vec<f32> = MARKS.iter().map(|(ms, _)| axis_x(*ms)).collect();
        for pair in positions.windows(2) {
            assert!(pair[1] > pair[0], "{positions:?}");
        }
        for position in &positions {
            assert!((0.0..=1.0).contains(position), "{position}");
        }
    }

    /// **The figure covers the range the reflections are specified to live in**
    /// (`REQ-VDP-003`). A figure narrower than the sound would hide arrivals.
    #[test]
    fn the_span_covers_every_arrival() {
        let mut reflections = vocal_depth_core::Reflections::new(48_000.0);
        reflections.set(vocal_depth_core::reflections::Settings {
            distance: 1.0,
            amount: 1.0,
        });
        for (position, _) in reflections.pattern() {
            assert!(
                (0.0..=1.0).contains(&position),
                "an arrival at {position} of the span is off the figure"
            );
        }
    }
}
