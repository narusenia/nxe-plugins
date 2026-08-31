//! The one line that says what the pointer is over.
//!
//! **A plugin window is a wall of abbreviations**, and the two ways out of that
//! are both worse than this one: a manual nobody opens, and a tooltip that puts
//! a second layer over the plane every time the mouse rests. Here the sentence
//! always appears in the same place — the right of the header — so the eye
//! learns one spot and the window keeps its quiet.
//!
//! **The description is written by the caller, not by the widget.** A `Knob`
//! has no idea it is `DRIVE`; the window that built it does.
//!
//! **Keep the sentence short.** The header lays the wordmark and this line out
//! on one row with a stretch between them; there is no truncation in this vizia
//! revision, so a sentence long enough to run out of room pushes the wordmark
//! instead of being cut. One clause, no full stop.
//!
//! ```no_run
//! # use nxe_ui::hint::Describe;
//! # use vizia::prelude::*;
//! # fn build(cx: &mut Context) {
//! Label::new(cx, "SPARK").describe("how much of the effect is applied");
//! # }
//! ```

use vizia::prelude::*;

/// What the pointer is over. Empty when it is over nothing that describes
/// itself, which is most of the window.
#[derive(Lens, Clone, Default)]
pub struct Hint {
    pub text: String,
}

pub enum HintEvent {
    Show(&'static str),
    /// **Carries what is being cleared**, not just "clear". The pointer enters
    /// the next control before it leaves the last one often enough that a bare
    /// clear wipes the sentence that just arrived, and the header goes empty
    /// while the mouse is sitting on a control.
    Clear(&'static str),
}

impl Hint {
    /// **Its own method so it can be tested.** `Model::event` needs an
    /// `EventContext`, which needs a running application; the decision this
    /// makes is the part that can be got wrong.
    pub fn apply(&mut self, event: &HintEvent) {
        match event {
            HintEvent::Show(text) => self.text = (*text).to_owned(),
            HintEvent::Clear(text) => {
                if self.text == *text {
                    self.text.clear();
                }
            }
        }
    }
}

impl Model for Hint {
    fn event(&mut self, _: &mut EventContext, event: &mut Event) {
        event.map(|hint: &HintEvent, meta| {
            self.apply(hint);
            meta.consume();
        });
    }
}

/// Give a control a sentence, shown in the header while the pointer is on it.
pub trait Describe {
    fn describe(self, text: &'static str) -> Self;
}

impl<V: View> Describe for Handle<'_, V> {
    fn describe(self, text: &'static str) -> Self {
        self.on_hover(move |cx| cx.emit(HintEvent::Show(text)))
            .on_hover_out(move |cx| cx.emit(HintEvent::Clear(text)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hovering_a_control_puts_its_sentence_up() {
        let mut hint = Hint::default();
        hint.apply(&HintEvent::Show("how much"));
        assert_eq!(hint.text, "how much");
        hint.apply(&HintEvent::Clear("how much"));
        assert!(hint.text.is_empty());
    }

    /// **The pointer enters the next control before it leaves the last one.**
    /// A clear that did not say what it was clearing would wipe the sentence
    /// that had just arrived, and the header would go blank with the mouse
    /// sitting on a control.
    #[test]
    fn leaving_the_previous_control_does_not_wipe_the_new_one() {
        let mut hint = Hint::default();
        hint.apply(&HintEvent::Show("the first"));
        hint.apply(&HintEvent::Show("the second"));
        hint.apply(&HintEvent::Clear("the first"));
        assert_eq!(hint.text, "the second");
    }
}
