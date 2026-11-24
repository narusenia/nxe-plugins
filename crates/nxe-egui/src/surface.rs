//! The figure's surface: the panel whose ground is the accent, and the glow
//! that moving signals on it are allowed.
//!
//! **Inversion is a palette, not a second concept.** The closure is handed
//! [`Palette::inverted`]; everything drawn with it comes out right without
//! knowing it is on the accent. The figure itself — a spectrum, a distance
//! picture — is each plugin's; the surface is shared (`design-system.md`).

use crate::theme::{Palette, Token};
use egui::{Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

/// Allocates a `size` panel on the accent and draws `figure` into it with the
/// inverted palette. The painter is clipped to the panel.
pub fn figure<R>(
    ui: &mut Ui,
    palette: &Palette,
    size: Vec2,
    figure: impl FnOnce(&Painter, Rect, &Palette) -> R,
) -> (R, egui::Response) {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    let inverted = palette.inverted();
    painter.rect_filled(rect, 0.0, inverted.ground.color());
    (figure(&painter, rect, &inverted), response)
}

/// The bloom under a moving line: three wider, fainter strokes. **Only for
/// signals** — a static mark that glows is decoration.
pub fn glow(painter: &Painter, points: &[Pos2], colour: Token) {
    if points.len() < 2 {
        return;
    }
    for (width, alpha) in [(9.0, 0.06), (5.0, 0.12), (3.0, 0.2)] {
        painter.add(Shape::line(
            points.to_vec(),
            Stroke::new(width, colour.at(colour.alpha * alpha).color()),
        ));
    }
}
