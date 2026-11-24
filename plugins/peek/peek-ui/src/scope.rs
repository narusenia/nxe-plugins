//! The waveform, oldest at the left. One vertical stroke per point from its
//! minimum to its maximum, so a long window still shows the envelope.

use egui::{Rect, Stroke, Ui, pos2};
use nxe_egui::theme::Palette;
use peek_core::meters::ScopeReadings;

pub fn figure(ui: &Ui, palette: &Palette, rect: Rect, readings: &ScopeReadings) {
    let painter = ui.painter_at(rect);
    let middle = rect.center().y;
    let half = rect.height() * 0.5;
    for level in [-0.5, 0.5] {
        let y = middle - level * half;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, palette.line.at(0.5).color()),
        );
    }
    painter.line_segment(
        [pos2(rect.left(), middle), pos2(rect.right(), middle)],
        Stroke::new(1.0, palette.line.color()),
    );
    let (mins, maxs) = (readings.mins.read(), readings.maxs.read());
    let step = rect.width() / mins.len() as f32;
    let stroke = Stroke::new(step.max(1.0), palette.accent.color());
    for (index, (low, high)) in mins.iter().zip(maxs.iter()).enumerate() {
        let x = rect.left() + (index as f32 + 0.5) * step;
        let top = middle - high.clamp(-1.0, 1.0) * half;
        let bottom = (middle - low.clamp(-1.0, 1.0) * half).max(top + 1.0);
        painter.line_segment([pos2(x, top), pos2(x, bottom)], stroke);
    }
}
