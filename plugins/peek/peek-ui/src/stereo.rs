//! A goniometer: recent sample pairs, turned 45° so mid is up and side is
//! across. A mono signal is a vertical line, a signal on one side lies along
//! that side's diagonal, and out of phase is flat.

use egui::{Align2, Rect, Stroke, Ui, vec2};
use nxe_egui::font;
use nxe_egui::theme::{FONT_EYEBROW, Palette};
use peek_core::meters::StereoReadings;
use std::f32::consts::FRAC_1_SQRT_2;

pub fn figure(ui: &Ui, palette: &Palette, rect: Rect, readings: &StereoReadings) {
    let painter = ui.painter_at(rect);
    let side = rect.width().min(rect.height());
    let square = Rect::from_center_size(rect.center(), vec2(side, side));
    let (centre, radius) = (square.center(), side * 0.5);
    let guide = Stroke::new(1.0, palette.line.color());
    painter.line_segment(
        [centre - vec2(0.0, radius), centre + vec2(0.0, radius)],
        guide,
    );
    painter.line_segment(
        [centre - vec2(radius, 0.0), centre + vec2(radius, 0.0)],
        guide,
    );
    let diagonal = radius * FRAC_1_SQRT_2;
    painter.line_segment(
        [
            centre + vec2(-diagonal, -diagonal),
            centre + vec2(diagonal, diagonal),
        ],
        guide,
    );
    painter.line_segment(
        [
            centre + vec2(diagonal, -diagonal),
            centre + vec2(-diagonal, diagonal),
        ],
        guide,
    );
    for (text, at) in [
        ("L", vec2(-diagonal, -diagonal)),
        ("R", vec2(diagonal, -diagonal)),
        ("M", vec2(0.0, -radius)),
        ("S", vec2(radius, 0.0)),
    ] {
        font::tracked(
            &painter,
            centre + at * 0.9,
            Align2::CENTER_CENTER,
            text,
            FONT_EYEBROW,
            palette.subtle.color(),
        );
    }

    let dot = palette.accent.at(0.55).color();
    for (left, right) in readings
        .lefts
        .read()
        .iter()
        .zip(readings.rights.read().iter())
    {
        let x = (right - left) * FRAC_1_SQRT_2;
        let y = (left + right) * FRAC_1_SQRT_2;
        let point = centre + vec2(x, -y) * radius;
        if square.contains(point) {
            painter.rect_filled(Rect::from_center_size(point, vec2(1.5, 1.5)), 0.0, dot);
        }
    }
}
