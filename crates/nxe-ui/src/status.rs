//! The strip along the bottom of a window.
//!
//! **Where the sentence about what the pointer is on goes** ([`crate::hint`]),
//! and the one place the accent is on screen before anything is touched.
//!
//! It started in the header, at the eyebrow size, on the black ground — and it
//! was hard to read there: small, pale, and at the far end of the row from
//! whatever the pointer was on. **Given its own strip it is a line of text on a
//! coloured ground**, which is the easiest thing to read in the window.
//!
//! **This is the window's one inverted surface** (`.agents/rules/ui.md`). It
//! was the figure first, and that was wrong for a reason worth writing down:
//! the figure's parts are drawn by hand *and* styled by CSS, and **only the
//! drawing follows a nested palette**. The transfer curve's ground came from
//! `.panel` and stayed black while its traces inverted to black with it, so the
//! curve vanished. A strip that is only a ground and a word has no such halves.

use crate::hint::Hint;
use crate::meter::Meter;
use crate::{font, theme};
use vizia::prelude::*;

/// How tall the strip is. Part of a window's height, so it is arithmetic
/// rather than something to measure on screen.
pub const HEIGHT: f32 = theme::LINE_LABEL + theme::SPACE_2 * 2.0;

/// The box a figure in the strip gets. Fixed for the reason every readout's is:
/// a number that changes width moves everything laid out after it.
const VALUE_WIDTH: f32 = 44.0;

/// How wide a gauge in the strip is.
const GAUGE_WIDTH: f32 = 64.0;

/// A figure in the strip: its name, the number, and the unit.
///
/// **Set at the label and value sizes, not the readout's.** A strip is one line
/// high; the headline size belongs to a region that has a subject, and this row
/// has five things in it (`.agents/rules/ui.md`).
pub fn figure(
    cx: &mut Context,
    name: &'static str,
    value: impl Res<String> + Clone,
    unit: &'static str,
) {
    HStack::new(cx, move |cx| {
        Label::new(cx, name)
            .class("label")
            .class("ink-muted")
            .height(Pixels(theme::LINE_LABEL));
        font::value(cx, value.clone())
            .class("ink")
            .width(Pixels(VALUE_WIDTH))
            .height(Pixels(theme::LINE_LABEL))
            .text_align(TextAlign::Right);
        Label::new(cx, unit)
            .class("label")
            .class("ink-muted")
            .height(Pixels(theme::LINE_LABEL));
    })
    .height(Pixels(theme::LINE_LABEL))
    .width(Auto)
    .col_between(Pixels(theme::SPACE_1));
}

/// A gauge in the strip: a name and a bar, for something asked
/// "is it doing anything right now".
pub fn gauge(cx: &mut Context, name: &'static str, level: impl Res<f32> + Clone + 'static) {
    HStack::new(cx, move |cx| {
        Label::new(cx, name)
            .class("label")
            .class("ink-muted")
            .height(Pixels(theme::LINE_LABEL));
        Meter::horizontal(cx, level.clone(), 0.0, Vec::new())
            .width(Pixels(GAUGE_WIDTH))
            .height(Pixels(theme::RULE_GAUGE));
    })
    .height(Pixels(theme::LINE_LABEL))
    .width(Auto)
    .col_between(Pixels(theme::SPACE_1))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_4));
}

/// The strip.
///
/// **Empty when the pointer is on nothing that describes itself.** It said what
/// the window was for at rest, which put the same sentence in two places —
/// the header's role already says that, and a strip that is always talking
/// stops being read.
pub fn bar(cx: &mut Context, figures: impl Fn(&mut Context) + 'static) {
    // Read before the builder borrows `cx` mutably (`UI-15`).
    let flipped = theme::palette(cx).inverted();
    HStack::new(cx, move |cx| {
        flipped.build(cx);
        Label::new(cx, Hint::text)
            .class("label")
            .class("ink")
            .width(Stretch(1.0))
            .height(Pixels(theme::LINE_LABEL));

        // **What the plugin is doing, on the right of the same line.** It was a
        // strip of its own under the header, at the headline size — which was a
        // lot of window for five short numbers, and it pushed the figure the
        // plugin exists to show down past it (`SPK-23`, looked at in a host).
        figures(cx);
    })
    .class("inverted")
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .child_left(Pixels(theme::SPACE_3))
    .child_right(Pixels(theme::SPACE_3))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}
