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
use crate::theme;
use vizia::prelude::*;

/// How tall the strip is. Part of a window's height, so it is arithmetic
/// rather than something to measure on screen.
pub const HEIGHT: f32 = theme::LINE_LABEL + theme::SPACE_2 * 2.0;

/// The strip, with `role` as what it says when the pointer is on nothing that
/// describes itself.
pub fn bar(cx: &mut Context, role: &'static str) {
    // Read before the builder borrows `cx` mutably (`UI-15`).
    let flipped = theme::palette(cx).inverted();
    HStack::new(cx, move |cx| {
        flipped.build(cx);
        Label::new(
            cx,
            Hint::text.map(move |text| {
                if text.is_empty() {
                    role.to_owned()
                } else {
                    text.clone()
                }
            }),
        )
        .class("label")
        .class("ink")
        .width(Stretch(1.0))
        .height(Pixels(theme::LINE_LABEL));
    })
    .class("inverted")
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .child_left(Pixels(theme::SPACE_3))
    .child_right(Pixels(theme::SPACE_3))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}
