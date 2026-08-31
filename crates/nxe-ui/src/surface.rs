//! The one panel in a window whose ground is the accent.
//!
//! **A window has at most one** (`.agents/rules/ui.md`). It is where the figure
//! the window exists to show goes — the thing a glance should land on first.
//! The rows of controls and the tables of figures stay on the black ground,
//! because a coloured field under a column of numbers costs legibility and buys
//! nothing.
//!
//! **Inversion is a palette, not a second concept.** The surface builds a
//! [`theme::Palette::inverted`] as a nested model, and everything drawn under
//! it asks the tree for its colours the way it always did
//! (`crates/nxe-ui/README.md`). No widget knows this module exists.
//!
//! **Text is the exception, and it has to be marked.** A stylesheet cannot say
//! "labels inside this panel" — the generated CSS is flat — so a label on the
//! accent ground carries `.ink` (or `.ink-muted`). Forgetting it leaves near
//! white on the accent, which is the failure the base `label` rule exists to
//! prevent everywhere else. Put few enough words here that it stays easy to
//! see.

use crate::theme;
use vizia::prelude::*;

/// A panel whose ground is the accent, with the palette inside it flipped.
///
/// ```no_run
/// # use nxe_ui::surface;
/// # use vizia::prelude::*;
/// # fn build(cx: &mut Context) {
/// surface::inverted(cx, |cx| {
///     Label::new(cx, "FIELD").class("eyebrow").class("ink-muted");
///     // the figure goes here
/// });
/// # }
/// ```
pub fn inverted<'a>(
    cx: &'a mut Context,
    content: impl Fn(&mut Context) + 'static,
) -> Handle<'a, VStack> {
    // Read before the builder borrows `cx` mutably — a palette lookup inside a
    // modifier chain does not compile (`UI-15`).
    let flipped = theme::palette(cx).inverted();
    VStack::new(cx, move |cx| {
        flipped.build(cx);
        content(cx);
    })
    .class("inverted")
}
