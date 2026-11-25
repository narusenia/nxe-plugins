//! A figure you can click and type into (`UI-3`).
//!
//! **The figure is the entry point**, not a modifier on the control: it is
//! where a user tries to type first. Cmd-clicking the control opens the same
//! field.
//!
//! The caller supplies the text to show and takes the typed string back.
//! Parsing is its business — this crate has no idea what a cent is. v0 had to
//! take this back off the Doubler because the vizia binding froze the editor;
//! here the field is a frame-local `TextEdit` with nothing bound to it.

use crate::font;
use crate::theme::{FONT_VALUE, Palette};
use egui::{Align2, Id, Key, Rect, Sense, TextEdit, Ui};

/// Draws `text` in `rect`, or the field while one is open. Returns the typed
/// string when Enter is pressed; Escape or a click elsewhere cancels.
///
/// `open` opens the field this frame (a Cmd-click on the control).
pub fn value_entry(
    ui: &mut Ui,
    palette: &Palette,
    id: Id,
    rect: Rect,
    text: &str,
    open: bool,
) -> Option<String> {
    let response = ui.interact(rect, id.with("figure"), Sense::click());
    entry(ui, palette, id, rect, text, open || response.clicked())
}

/// [`value_entry`] without the click of its own: the field opens only when
/// `open` says so. For a control that wants every click on itself — a drag,
/// a double-click to reset — and opens the field on a Cmd-click
/// (`crate::number`).
pub fn entry(
    ui: &mut Ui,
    palette: &Palette,
    id: Id,
    rect: Rect,
    text: &str,
    open: bool,
) -> Option<String> {
    let mut buffer: Option<String> = ui.data(|data| data.get_temp(id));
    if buffer.is_none() && open {
        buffer = Some(text.to_owned());
    }

    let Some(mut editing) = buffer else {
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font::mono(FONT_VALUE),
            palette.ink.color(),
        );
        return None;
    };

    ui.painter().rect_filled(rect, 0.0, palette.track.color());
    // In a `Ui` of its own: `ui.put` would move the caller's cursor, and a
    // row laid out after it would jump while the field is open.
    let field = ui.new_child(egui::UiBuilder::new().max_rect(rect)).put(
        rect,
        TextEdit::singleline(&mut editing)
            .id(id.with("field"))
            .font(font::mono(FONT_VALUE))
            .text_color(palette.ink.color())
            .horizontal_align(egui::Align::Center)
            .frame(egui::Frame::NONE)
            .desired_width(rect.width()),
    );
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }

    let enter = ui.input(|input| input.key_pressed(Key::Enter));
    let cancel = ui.input(|input| input.key_pressed(Key::Escape));
    if field.lost_focus() || enter || cancel {
        ui.data_mut(|data| data.remove::<String>(id));
        return (enter && !cancel).then_some(editing);
    }
    ui.data_mut(|data| data.insert_temp(id, editing));
    None
}
