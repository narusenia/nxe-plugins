//! The band across the top of a plugin window.
//!
//! **Three plugins had three headers that were the same thing written three
//! times** — a wordmark in a row, and nothing else. Identical by accident is
//! not the same as identical on purpose: the moment one of them wanted a rule
//! under it, the three would have drifted. It lives here so that a change to
//! what a window's top looks like is one edit.
//!
//! ```text
//! NXE  Sparkleur   [SOFT|HARD]        how much of the effect is applied
//! ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━──────────────  ← fades out to the right
//! ```
//!
//! The rule is the design's structural device (`crates/nxe-ui/README.md`), and
//! this is the one it starts from.
//!
//! **The wordmark is the product's name without the vendor on it.** It used to
//! read `NXE SPARKLEUR`, which is the string a host's plugin list shows — and
//! that is exactly why it does not belong here: by the time the window is open,
//! the list has already been read. `NXE` stays as a small mark to its left,
//! where a vendor belongs.
//!
//! **The right of the band is one line about whatever the pointer is on**
//! ([`crate::hint`]), falling back to what the window is for when the pointer
//! is on nothing. A plugin window is a wall of abbreviations and this is the
//! cheapest way out of it that does not put a second layer over the plane.

use crate::font;
use crate::hint::Hint;
use crate::theme;
use vizia::prelude::*;

/// The vendor. Small, quiet, and to the left of every wordmark in the line.
pub const VENDOR: &str = "NXE";

/// How tall [`header`] is. Part of a window's height, so it is arithmetic
/// rather than something to measure on screen (`theme::LINE_TITLE`).
pub const HEIGHT: f32 = theme::LINE_TITLE + theme::SPACE_2 + theme::RULE_ACCENT;

/// The vendor mark, the wordmark, the mode slot, the hint, and the rule under
/// all of it.
///
/// `name` is the **product's** name — `Sparkleur`, not `NXE Sparkleur` and not
/// the crate name. `role` is what the window is for, in the fewest words that
/// distinguish it from its siblings; it is what the right of the band says when
/// the pointer is describing nothing.
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
            // The vendor, small and quiet, sitting on the wordmark's baseline.
            Label::new(cx, VENDOR)
                .class("eyebrow")
                .width(Auto)
                .height(Pixels(theme::LINE_EYEBROW))
                .top(Stretch(1.0))
                .bottom(Pixels(theme::SPACE_1));

            font::title(cx, name).height(Pixels(theme::LINE_TITLE));

            mode(cx);

            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));

            // Bottom-aligned against the wordmark rather than centred: the two
            // sit on the same baseline that way, which is the point of putting
            // them on one line.
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
            .class("eyebrow")
            .width(Auto)
            .height(Pixels(theme::LINE_EYEBROW))
            .top(Stretch(1.0))
            .bottom(Pixels(theme::SPACE_1));
        })
        .height(Pixels(theme::LINE_TITLE))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_2));

        Element::new(cx).class("rule-accent");
    })
    .height(Auto)
    .row_between(Pixels(theme::SPACE_2));
}
