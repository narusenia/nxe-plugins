//! A row of mutually exclusive choices.
//!
//! Unlike [`Knob`](crate::knob::Knob) and [`Bar`](crate::bar::Bar), this takes a
//! **lens** rather than an `impl Res`. Each segment needs its own reactive
//! "am I the selected one" state, and that means mapping the selection per
//! child, which `Res` cannot do. Both real callers have a lens anyway: the
//! gallery has a model, and a plugin binds its parameters through one.
//!
//! No `View::draw` here. A row of boxes with one highlighted is exactly what CSS
//! is for, and going through the stylesheet means the selected segment gets the
//! theme's transition for free (`.agents/rules/vizia.md`).

use std::sync::Arc;
use vizia::prelude::*;

type SelectCallback = Arc<dyn Fn(&mut EventContext, usize) + Send + Sync>;

pub struct SegmentedControl {
    /// The view's own copy of the selection, kept in step with the lens by the
    /// bind below. Arrow keys need to know where they are moving from, and a
    /// lens cannot be read from `View::event` without storing it.
    selected: usize,
    count: usize,
    on_select: SelectCallback,
}

/// The lens telling the view what it now shows. Internal: the caller drives the
/// selection through its own lens, not through this.
enum SegmentedEvent {
    Selected(usize),
}

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
        // One handler shared by every segment's closure and by the key handling.
        let on_select: SelectCallback = Arc::new(on_select);
        let labels: Vec<String> = labels.iter().map(|label| (*label).to_owned()).collect();

        Self {
            selected: 0,
            count: labels.len(),
            on_select: on_select.clone(),
        }
        .build(cx, move |cx| {
            // The row itself takes focus, not the individual segments: the
            // choice is one control, and arrow keys move within it.
            let row = cx.current();
            selected.set_or_bind(cx, row, move |cx, value| {
                cx.emit_to(row, SegmentedEvent::Selected(value));
            });

            for (index, label) in labels.into_iter().enumerate() {
                let on_select = on_select.clone();
                Label::new(cx, &label)
                    .class("segment")
                    .checked(selected.map(move |current| *current == index))
                    .on_press(move |cx| {
                        // Clicking hands the keyboard to the row, so the arrow
                        // keys carry on from where the pointer left off.
                        cx.with_current(row, |cx| cx.focus());
                        on_select(cx, index);
                    });
            }
        })
        .class("segmented")
        .navigable(true)
    }
}

impl View for SegmentedControl {
    fn element(&self) -> Option<&'static str> {
        Some("nxesegmented")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|segmented_event, _| match segmented_event {
            SegmentedEvent::Selected(index) => self.selected = *index,
        });

        event.map(|window_event, meta| {
            // Left and right rather than up and down: the segments are a row,
            // and up/down belongs to whatever the row sits inside.
            let next = match window_event {
                WindowEvent::KeyDown(Code::ArrowLeft, _) => self.selected.checked_sub(1),
                WindowEvent::KeyDown(Code::ArrowRight, _) => {
                    Some(self.selected + 1).filter(|next| *next < self.count)
                }
                _ => return,
            };

            // At either end the key does nothing and is *not* consumed, so it
            // stays available to whatever else might want it.
            if let Some(next) = next {
                (self.on_select)(cx, next);
                meta.consume();
            }
        });
    }
}
