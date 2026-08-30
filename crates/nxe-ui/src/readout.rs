//! A row of figures that say what is happening right now.
//!
//! **The shape, not the numbers.** What a plugin reports is its own business —
//! this crate knows nothing about decibels or hertz — but every one of them
//! reports it the same way: a region name as an eyebrow over a rule, and one
//! figure under it in the mono face.
//!
//! ## Why the value box is a fixed width
//!
//! A reading that changes length moves everything laid out after it. Three ways
//! it changes: the sign appears when a value crosses zero, a digit appears when
//! it crosses ten, and a dash replaces the whole thing when there is no signal.
//! A caller can fix the first by always printing the sign; it cannot fix the
//! others with a format string. **A fixed box and right alignment fix all
//! three** — the unit beside it never moves, and a new digit grows leftward into
//! the padding rather than pushing anything along.
//!
//! Sparkleur's reduction readout twitched on both counts before this existed
//! (`plugins/sparkleur/docs/implementation/sparkleur-plan.md`, `SPK-19`).
//!
//! ## Give it a value from the model, never a lens that reads the audio thread
//!
//! Every plugin built its cells from a lens that read the handoff directly —
//! `Ui::params.map(|_| level(peaks.read()[0]))` — on the reasoning that the
//! heartbeat re-evaluates it thirty times a second. **That reasoning was
//! wrong.** `binding_system` re-evaluates every store **once per frame**,
//! whatever the heartbeat does, so such a lens formats a fresh `String` 66.7
//! times a second and redraws the window every time the number moves.
//!
//! Measured in Studio One with audio running, one open window took **62 % of
//! the host's UI thread**, against 1 % for the same window in silence
//! (`docs/investigations/ui-frame-cost.md`). The window is drawn on the host's
//! run loop, so that is the host's own interface stalling.
//!
//! With the figure copied into the model on the heartbeat, `BasicStore`
//! compares the `String` against the last one and the window redraws when the
//! **printed** figure changes — half as often, and not at all while a reading
//! is steady.

use crate::font;
use crate::meter::Meter;
use crate::theme;
use vizia::prelude::*;

/// Room for a sign, two digits, a point and a decimal in the mono face at
/// [`theme::FONT_READOUT`].
pub const VALUE_WIDTH: f32 = 44.0;

/// A bar in a cell. Short, so it reads as a readout rather than as a meter of
/// its own.
pub const GATE_HEIGHT: f32 = 8.0;

/// How tall a cell's content is, whatever the content is.
///
/// **A figure and a bar do not measure the same**, so left to themselves the two
/// kinds of cell come out different heights — and a strip with one of each is
/// then a different height from a strip with none, which moves everything below
/// it. Two windows meant to sit on the same grid stopped lining up over exactly
/// this (`SPK-19`). One number, both kinds.
const CONTENT_HEIGHT: f32 = 20.0;

/// A region's name over its rule, as a height.
pub const HEADING_HEIGHT: f32 = theme::LINE_EYEBROW + theme::SPACE_1 + theme::RULE;

/// How tall [`strip`] is. Part of a window's height, so it is arithmetic.
pub const HEIGHT: f32 = HEADING_HEIGHT + theme::SPACE_1 + CONTENT_HEIGHT;

/// The row. Give it the cells.
pub fn strip(cx: &mut Context, content: impl Fn(&mut Context)) {
    HStack::new(cx, |cx| content(cx))
        .height(Pixels(HEIGHT))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_4));
}

/// A region's name over its rule, and one figure under it.
///
/// `unit` is set beside the figure rather than inside it, so it stays put while
/// the number moves.
pub fn cell(
    cx: &mut Context,
    name: &'static str,
    value: impl Res<String> + Clone,
    unit: &'static str,
) {
    body(cx, name, move |cx| {
        HStack::new(cx, |cx| {
            font::value(cx, value.clone())
                .class("readout")
                .width(Pixels(VALUE_WIDTH))
                .height(Auto)
                .text_align(TextAlign::Right);
            Label::new(cx, unit).class("subtle").top(Stretch(1.0));
        })
        .height(Pixels(CONTENT_HEIGHT))
        .width(Auto)
        .col_between(Pixels(theme::SPACE_1))
        .child_top(Stretch(1.0))
        .child_bottom(Pixels(0.0));
    });
}

/// A cell whose content is a bar rather than a figure.
///
/// **No hold and no marks.** A held peak answers "how loud did it get"; a gate
/// or an activity light is asked "is it doing something right now", and a mark
/// left behind would answer the wrong question after it stopped.
pub fn meter_cell(cx: &mut Context, name: &'static str, level: impl Res<f32> + Clone + 'static) {
    body(cx, name, move |cx| {
        // Wrapped in a box of the same height as a figure's, so a strip with a
        // bar in it is the same height as one without.
        HStack::new(cx, |cx| {
            Meter::horizontal(cx, level.clone(), 0.0, Vec::new())
                .width(Stretch(1.0))
                .height(Pixels(GATE_HEIGHT));
        })
        .height(Pixels(CONTENT_HEIGHT))
        .width(Stretch(1.0))
        .child_top(Stretch(1.0))
        .child_bottom(Pixels(theme::SPACE_1));
    });
}

fn body(cx: &mut Context, name: &'static str, content: impl Fn(&mut Context)) {
    VStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            Label::new(cx, name)
                .class("eyebrow")
                .height(Pixels(theme::LINE_EYEBROW));
        })
        .class("heading")
        .height(Pixels(HEADING_HEIGHT));
        content(cx);
    })
    .width(Stretch(1.0))
    .height(Pixels(HEIGHT))
    .row_between(Pixels(theme::SPACE_1));
}
