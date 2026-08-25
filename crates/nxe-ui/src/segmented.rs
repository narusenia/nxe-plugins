//! A row of mutually exclusive choices.
//!
//! Unlike [`Knob`](crate::knob::Knob) and [`Bar`](crate::bar::Bar), this takes a
//! **lens** rather than an `impl Res`. Each segment needs its own reactive
//! "am I the selected one" state, and that means mapping the selection per
//! child, which `Res` cannot do. Both real callers have a lens anyway: the
//! gallery has a model, and a plugin binds its parameters through one.
//!
//! No `View::draw` here. A row of rounded boxes with one highlighted is exactly
//! what CSS is for, and going through the stylesheet means the selected segment
//! gets the theme's transition for free (`.agents/rules/vizia.md`).

use std::sync::Arc;
use vizia::prelude::*;

pub struct SegmentedControl {}

impl SegmentedControl {
    /// `selected` is an index into `labels`.
    pub fn new<'a, L>(
        cx: &'a mut Context,
        selected: L,
        labels: &[&str],
        // `Send + Sync` because vizia's `on_press` stores the action where it
        // could be invoked from another thread.
        on_select: impl Fn(&mut EventContext, usize) + Send + Sync + 'static,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = usize> + Copy,
    {
        // One handler shared by every segment's closure.
        let on_select = Arc::new(on_select);
        let labels: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();

        Self {}
            .build(cx, move |cx| {
                for (index, label) in labels.into_iter().enumerate() {
                    let on_select = on_select.clone();
                    Label::new(cx, &label)
                        .class("segment")
                        .checked(selected.map(move |current| *current == index))
                        .on_press(move |cx| on_select(cx, index));
                }
            })
            .class("segmented")
    }
}

impl View for SegmentedControl {
    fn element(&self) -> Option<&'static str> {
        Some("nxesegmented")
    }
}
