//! IN and OUT, side by side, always visible.
//!
//! **The one judgement a saturator is used for** is "is this better, or just
//! louder" (`ui.md`). Two pairs of bars on the same scale is what makes that
//! answerable without reaching for the host's meters.
//!
//! **No reduction meter.** What the guards are doing is already on screen — the
//! regions sink by the amount they are pulling (`REQ-VEL-006`) — and the same
//! number in two places is one of them being ignored.

use super::{Ui, meter_position};
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::meter::Meter;
use nxe_ui::theme;

/// The width of the whole strip, and of one bar inside it.
pub const WIDTH: f32 = 56.0;
const BAR_WIDTH: f32 = 10.0;

/// Where the scale is marked. Only the numbers a mix is read against: nothing
/// below −24 matters for "is it louder", and a ladder of ten ticks would be a
/// texture rather than a scale.
const MARKS_DB: [f32; 3] = [-6.0, -12.0, -24.0];

pub fn view(cx: &mut Context) {
    VStack::new(cx, |cx| {
        pair(cx, "IN", 0);
        pair(cx, "OUT", 2);
    })
    .class("panel")
    .width(Pixels(WIDTH))
    .height(Stretch(1.0))
    .child_space(Pixels(theme::SPACE_2))
    .row_between(Pixels(theme::SPACE_2));
}

/// One label over two bars: left and right of the same point in the signal.
///
/// A mono source puts them exactly on top of each other, which is itself
/// information — Velour does not move the image (`REQ-VEL-011`), so two bars
/// that differ mean the source did.
fn pair(cx: &mut Context, label: &'static str, first: usize) {
    VStack::new(cx, |cx| {
        Label::new(cx, label).class("subtle");

        HStack::new(cx, |cx| {
            for offset in 0..2 {
                let index = first + offset;
                Meter::new(
                    cx,
                    Ui::peaks.map(move |peaks| peaks.get(index).copied().unwrap_or(0.0)),
                    Ui::holds.map(move |holds| holds.get(index).copied().unwrap_or(0.0)),
                    MARKS_DB.iter().copied().map(decibel_mark).collect(),
                )
                .width(Pixels(BAR_WIDTH))
                .height(Stretch(1.0));
            }
        })
        .width(Auto)
        .height(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_1))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));
    })
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

/// A mark in dB, placed with the same mapping the bars are filled by — so the
/// tick and the level cannot disagree.
fn decibel_mark(decibels: f32) -> f32 {
    meter_position(10.0f32.powf(decibels / 20.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_marks_land_where_the_scale_puts_them() {
        // Full scale is the top, and the marks descend in order.
        assert_eq!(decibel_mark(0.0), 1.0);
        let marks: Vec<f32> = MARKS_DB.iter().copied().map(decibel_mark).collect();
        for pair in marks.windows(2) {
            assert!(pair[0] > pair[1], "{marks:?}");
        }
        for mark in &marks {
            assert!((0.0..1.0).contains(mark), "{mark}");
        }
    }
}
