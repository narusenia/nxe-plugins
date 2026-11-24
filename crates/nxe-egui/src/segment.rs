//! A row of mutually exclusive choices. The selected one is filled with the
//! accent and its word set in the ground colour; the others are words between
//! hairlines.

use crate::font;
use crate::theme::{FONT_LABEL, Palette};
use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

/// How tall one segment is.
pub const HEIGHT: f32 = 20.0;

/// Returns the index clicked this frame, if any.
pub fn segment(
    ui: &mut Ui,
    palette: &Palette,
    choices: &[&str],
    selected: usize,
    width: f32,
) -> Option<usize> {
    let (rect, row) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::hover());
    let step = rect.width() / choices.len().max(1) as f32;
    let mut picked = None;
    for (index, choice) in choices.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + step * index as f32, rect.top()),
            Vec2::new(step, HEIGHT),
        );
        let response = ui.interact(cell, row.id.with(index), Sense::click());
        let on = index == selected;
        let painter = ui.painter();
        if on {
            painter.rect_filled(cell, 0.0, palette.accent.color());
        } else if response.hovered() {
            painter.rect_filled(cell, 0.0, palette.track.color());
        }
        let colour = if on {
            palette.ground
        } else if response.hovered() {
            palette.ink
        } else {
            palette.muted
        };
        painter.text(
            cell.center(),
            Align2::CENTER_CENTER,
            choice,
            font::sans(FONT_LABEL),
            colour.color(),
        );
        if response.clicked() && !on {
            picked = Some(index);
        }
    }
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, palette.line.color()),
        StrokeKind::Inside,
    );
    picked
}
