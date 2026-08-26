//! The band across the top of a plugin window.
//!
//! **Three plugins had three headers that were the same thing written three
//! times** — a wordmark in a row, and nothing else. Identical by accident is
//! not the same as identical on purpose: the moment one of them wanted a rule
//! under it, the three would have drifted. It lives here so that a change to
//! what a window's top looks like is one edit.
//!
//! ```text
//! NXE SPARKLEUR                          five-band dynamics
//! ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━──────────────  ← fades out to the right
//! ```
//!
//! The rule is the design's structural device (`crates/nxe-ui/README.md`), and
//! this is the one it starts from. The role on the right is what the window is
//! for, in the fewest words that distinguish it from its siblings — a plugin
//! list gives you a name and nothing else, and by the time the window is open
//! the name is the one thing you already know.

use crate::font;
use crate::theme;
use vizia::prelude::*;

/// The wordmark, the role, and the rule under both.
///
/// `name` is the **shipped** name — the same string as the plugin's `NAME`, its
/// bundle, and its entry in a host's plugin list. Anything else makes the window
/// and the list disagree about what is open.
pub fn header(cx: &mut Context, name: &'static str, role: &'static str) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            font::title(cx, name);
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
            // Bottom-aligned against the wordmark rather than centred: the two
            // sit on the same baseline that way, which is the point of putting
            // them on one line.
            Label::new(cx, role)
                .class("eyebrow")
                .width(Auto)
                .height(Auto)
                .top(Stretch(1.0))
                .bottom(Pixels(theme::SPACE_1));
        })
        .height(Auto)
        .width(Stretch(1.0));

        Element::new(cx).class("rule-accent");
    })
    .height(Auto)
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_2));
}
