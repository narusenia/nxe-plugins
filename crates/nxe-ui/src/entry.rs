//! A number you can click and type into.
//!
//! **The number is the entry point, not a modifier key on the control.** A knob
//! reports [`Gesture::Edit`](crate::input::Gesture::Edit) when it is
//! `Cmd`-clicked, but a knob has nowhere to put a text field and knows nothing
//! about units or parsing — so the field lives where the figure already is, on
//! the label under the control, which is also where a user tries to type first.
//!
//! The caller supplies the text to show and takes the string back. Parsing is
//! its business: this crate has no idea what a cent or a decibel is
//! (`docs/specifications/architecture.md`).

use crate::font;
use std::sync::Arc;
use vizia::prelude::*;

type SubmitCallback = Arc<dyn Fn(&mut EventContext, &str) + Send + Sync>;

/// Derives `Lens` so the view can bind its children to its own state — a view
/// cannot rebuild its children from a plain field, and this is the shape vizia
/// offers for it.
#[derive(Lens)]
pub struct ValueEntry {
    editing: bool,
    on_submit: SubmitCallback,
}

enum EntryEvent {
    Begin,
    Submit(String),
    Cancel,
}

impl ValueEntry {
    /// `text` is what the number reads while it is not being edited.
    pub fn new<'a, L>(
        cx: &'a mut Context,
        text: L,
        on_submit: impl Fn(&mut EventContext, &str) + Send + Sync + 'static,
    ) -> Handle<'a, Self>
    where
        L: Lens<Target = String> + Copy,
    {
        Self {
            editing: false,
            on_submit: Arc::new(on_submit),
        }
        .build(cx, move |cx| {
            Binding::new(cx, ValueEntry::editing, move |cx, editing| {
                if editing.get(cx) {
                    Textbox::new(cx, text)
                        .class("value")
                        // Same reason as everywhere else: a family set through
                        // CSS does not select an embedded font in this vizia
                        // revision (`.agents/rules/vizia.md`).
                        .font_family(vec![FamilyOwned::Name(font::MONO.to_owned())])
                        .on_submit(|cx, string, success| {
                            if success {
                                cx.emit(EntryEvent::Submit(string));
                            } else {
                                cx.emit(EntryEvent::Cancel);
                            }
                        })
                        // Typing starts immediately with the old value selected,
                        // so the common case — replace it — is one keystroke.
                        .on_build(|cx| {
                            cx.emit(TextEvent::StartEdit);
                            cx.emit(TextEvent::SelectAll);
                        })
                        .width(Stretch(1.0))
                        .height(Auto);
                } else {
                    font::value(cx, text)
                        .class("editable")
                        .on_press(|cx| cx.emit(EntryEvent::Begin))
                        .width(Stretch(1.0))
                        .height(Auto);
                }
            });
        })
        .height(Auto)
    }
}

impl View for ValueEntry {
    fn element(&self) -> Option<&'static str> {
        Some("nxevalueentry")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|entry_event: &EntryEvent, meta| {
            match entry_event {
                EntryEvent::Begin => self.editing = true,
                EntryEvent::Submit(text) => {
                    (self.on_submit)(cx, text);
                    self.editing = false;
                }
                EntryEvent::Cancel => self.editing = false,
            }
            // These are this view's own business; nothing above it has a use
            // for them.
            meta.consume();
        });
    }
}
