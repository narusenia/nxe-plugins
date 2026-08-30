//! The band across the top of a plugin window.
//!
//! **Three plugins had three headers that were the same thing written three
//! times** — a wordmark in a row, and nothing else. Identical by accident is
//! not the same as identical on purpose: the moment one of them wanted a rule
//! under it, the three would have drifted. It lives here so that a change to
//! what a window's top looks like is one edit.
//!
//! ```text
//! Sparkleur  [SOFT|HARD]      five-band dynamics + sparkle   ⬔
//! ─────────────────────────────────────────────────
//! ```
//!
//! The rule is the design's structural device (`crates/nxe-ui/README.md`), and
//! this is the one it starts from. **A hairline, like every other rule.** It
//! was a 2 px accent bar, which announced the window before the window had said
//! anything; the accent now shows up where the window is actually doing
//! something instead.
//!
//! **The wordmark is the product's name without the vendor on it.** It used to
//! read `NXE SPARKLEUR`, which is the string a host's plugin list shows — and
//! that is exactly why it does not belong here: by the time the window is open,
//! the list has already been read.
//!
//! **The vendor closes the band rather than opening it** ([`crate::logo`]). It
//! sat to the wordmark's left for one build and read as a second wordmark —
//! two marks competing in the corner a window is read from.
//!
//! **The right of the band says what the window is for.** The line about
//! whatever the pointer is on lives at the bottom instead
//! ([`crate::status`]) — it was here first, and at the eyebrow size on the
//! black ground it was hard to read.

use crate::font;
use crate::logo;
use crate::theme;
use vizia::prelude::*;

/// How tall the vendor's mark is drawn.
///
/// **10 was mush.** The `E` is three bars with two gaps, and at that height the
/// gaps closed — the mark stopped being letters and became a smear. It needs
/// about 13 before the bars separate.
pub const VENDOR_HEIGHT: f32 = 13.0;

/// How tall [`header`] is. Part of a window's height, so it is arithmetic
/// rather than something to measure on screen (`theme::LINE_TITLE`).
pub const HEIGHT: f32 = theme::LINE_TITLE + theme::SPACE_2 + theme::RULE;

/// The vendor mark, the wordmark, the mode slot, the role, and the rule under
/// all of it.
///
/// `name` is the **product's** name — `Sparkleur`, not `NXE Sparkleur` and not
/// the crate name. `role` is what the window is for, in the fewest words that
/// distinguish it from its siblings.
///
/// `mode` builds whatever belongs beside the wordmark — the `Soft` / `Hard`
/// switch on the two plugins that have one. **A window without one passes an
/// empty closure rather than getting a different function**: five headers whose
/// left edges line up matter more than one saved argument.
pub fn header(
    cx: &mut Context,
    name: &'static str,
    role: &'static str,
    mode: impl Fn(&mut Context) + 'static,
) {
    VStack::new(cx, move |cx| {
        HStack::new(cx, move |cx| {
            font::title(cx, name).height(Pixels(theme::LINE_TITLE));

            // **Centred, not top-aligned.** A control in this band is not text
            // on the wordmark's baseline; sitting it at the top of the row put
            // it visibly above the middle of everything beside it.
            HStack::new(cx, move |cx| mode(cx))
                .width(Auto)
                .height(Stretch(1.0))
                .child_top(Stretch(1.0))
                .child_bottom(Stretch(1.0));

            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));

            // Bottom-aligned against the wordmark rather than centred: the two
            // sit on the same baseline that way, which is the point of putting
            // them on one line.
            Label::new(cx, role)
                .class("eyebrow")
                .width(Auto)
                .height(Pixels(theme::LINE_EYEBROW))
                .top(Stretch(1.0))
                .bottom(Pixels(theme::SPACE_1));

            // **The vendor, at the far end of the band.** It was beside the
            // wordmark first and read as a second one — two marks competing in
            // the corner a window is read from. Here it closes the line instead
            // of opening it.
            logo::Mark::new(cx)
                .width(Pixels(logo::width_at(VENDOR_HEIGHT)))
                .height(Pixels(VENDOR_HEIGHT))
                .top(Stretch(1.0))
                .bottom(Stretch(1.0));
        })
        .height(Pixels(theme::LINE_TITLE))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_2));

        Element::new(cx).class("rule");
    })
    .height(Auto)
    .row_between(Pixels(theme::SPACE_2));
}
