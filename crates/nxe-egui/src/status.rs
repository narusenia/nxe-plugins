//! The strip along the bottom: one line about whatever the pointer is on.
//!
//! **Empty at rest** — a strip that is always talking stops being read. Where
//! it sits in the H4 layout, and whether it is inverted, is `V2-9`'s to
//! decide (`design-system.md`); this draws it on the plain ground.

use crate::font;
use crate::theme::{FONT_LABEL, LINE_LABEL, Palette, SPACE_1, SPACE_3};
use egui::{Align2, Sense, Stroke, Ui, pos2, vec2};

pub const HEIGHT: f32 = LINE_LABEL + SPACE_1 * 2.0;

pub fn status(ui: &mut Ui, palette: &Palette, width: f32, hint: Option<&str>) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::hover());
    let painter = ui.painter();
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, palette.line.color()),
    );
    if let Some(hint) = hint {
        painter.text(
            pos2(rect.left() + SPACE_3, rect.center().y),
            Align2::LEFT_CENTER,
            hint,
            font::sans(FONT_LABEL),
            palette.muted.color(),
        );
    }
}
