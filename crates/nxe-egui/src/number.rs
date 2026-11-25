//! A figure in a box: drag it up or down to change it (Shift for fine), click
//! it to type (`crate::entry`). The click goes to the entry, which sits on
//! top, so there is no double-click reset here; type the value instead. For a value that wants no dial of its own, set
//! beside the control it belongs to (Dew's Pitch beside Free and Follow).
//! As tall as a [`crate::segment`], so the two sit in one row.

use crate::entry::value_entry;
use crate::input;
use crate::knob::Control;
use crate::segment::HEIGHT;
use crate::theme::Palette;
use egui::{Sense, Stroke, StrokeKind, Ui, Vec2};

/// `value` is normalised `0..=1`; `text` is what the box shows.
pub fn number(ui: &mut Ui, palette: &Palette, value: f32, text: &str, width: f32) -> Control {
    let (rect, block) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    let id = block.id.with("number");
    let response = ui.interact(rect, id, Sense::drag());
    let gestures = input::read(&response, value.clamp(0.0, 1.0));
    let active = response.hovered() || response.dragged();
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(
            1.0,
            if active { palette.accent } else { palette.line }.color(),
        ),
        StrokeKind::Inside,
    );
    let typed = value_entry(ui, palette, id.with("entry"), rect, text, gestures.edit);
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    Control { gestures, typed }
}
