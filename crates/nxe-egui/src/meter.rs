//! A level bar with a peak-hold mark. One bar per call; where the scale's floor
//! sits, and whether it is in decibels, is the caller's — values come in
//! normalized.
//!
//! **A moving signal, so it may glow** (`design-system.md`): the head of the
//! fill carries a short bloom. The fill itself stays flat — its length is the
//! quantity.

use crate::surface::glow;
use crate::theme::Palette;
use egui::{Painter, Rect, Stroke, pos2};

pub fn meter(
    painter: &Painter,
    palette: &Palette,
    rect: Rect,
    level: f32,
    peak: f32,
    vertical: bool,
) {
    painter.rect_filled(rect, 0.0, palette.track.color());
    let level = level.clamp(0.0, 1.0);
    let peak = peak.clamp(0.0, 1.0);
    let (fill, head) = if vertical {
        let top = rect.bottom() - rect.height() * level;
        (
            Rect::from_min_max(pos2(rect.left(), top), rect.right_bottom()),
            [pos2(rect.left(), top), pos2(rect.right(), top)],
        )
    } else {
        let right = rect.left() + rect.width() * level;
        (
            Rect::from_min_max(rect.left_top(), pos2(right, rect.bottom())),
            [pos2(right, rect.top()), pos2(right, rect.bottom())],
        )
    };
    painter.rect_filled(fill, 0.0, palette.accent.color());
    if level > 0.0 {
        glow(painter, &head, palette.bright);
    }
    let mark = if vertical {
        let y = rect.bottom() - rect.height() * peak;
        [pos2(rect.left(), y), pos2(rect.right(), y)]
    } else {
        let x = rect.left() + rect.width() * peak;
        [pos2(x, rect.top()), pos2(x, rect.bottom())]
    };
    if peak > 0.0 {
        painter.line_segment(mark, Stroke::new(1.0, palette.ink.color()));
    }
}
