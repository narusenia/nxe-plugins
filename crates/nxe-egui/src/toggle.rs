//! A switch between two states, as a word and a chip — `Morph [Spin]`. The
//! chip names the state the switch is in: filled with the accent when on,
//! a hairline when off. A click anywhere on it flips it. As tall as a
//! [`crate::segment`].

use crate::font;
use crate::segment::HEIGHT;
use crate::theme::{FONT_LABEL, Palette, SPACE_1, SPACE_2};
use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

/// Returns true when it was clicked this frame; the caller flips the value.
pub fn toggle(ui: &mut Ui, palette: &Palette, label: &str, text: &str, on: bool) -> bool {
    let painter = ui.painter();
    let word = painter.layout_no_wrap(
        label.to_owned(),
        font::sans(FONT_LABEL),
        palette.muted.color(),
    );
    let chip_width = painter
        .layout_no_wrap(text.to_owned(), font::sans(FONT_LABEL), palette.ink.color())
        .size()
        .x
        + 2.0 * SPACE_2;
    let gap = SPACE_1 * 1.5;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(word.size().x + gap + chip_width, HEIGHT),
        Sense::click(),
    );
    let chip = Rect::from_min_max(pos2(rect.right() - chip_width, rect.top()), rect.max);
    let painter = ui.painter();
    painter.galley(
        pos2(rect.left(), rect.center().y - word.size().y * 0.5),
        word,
        palette.muted.color(),
    );
    let colour = if on {
        painter.rect_filled(chip, 0.0, palette.accent.color());
        palette.ground
    } else {
        let edge = if response.hovered() {
            palette.accent
        } else {
            palette.line
        };
        painter.rect_stroke(
            chip,
            0.0,
            Stroke::new(1.0, edge.color()),
            StrokeKind::Inside,
        );
        if response.hovered() {
            palette.ink
        } else {
            palette.muted
        }
    };
    painter.text(
        chip.center(),
        Align2::CENTER_CENTER,
        text,
        font::sans(FONT_LABEL),
        colour.color(),
    );
    response.clicked()
}
