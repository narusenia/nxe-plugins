//! The waveform, oldest at the left. One vertical stroke per point from its
//! minimum to its maximum, so a long window still shows the envelope.

use egui::{Align2, Rect, Stroke, Ui, pos2};
use nxe_egui::font;
use nxe_egui::theme::{FONT_VALUE, Palette};
use peek_core::meters::ScopeReadings;

/// `window_ms` is how much time the picture covers, for the time scale.
pub fn figure(ui: &Ui, palette: &Palette, rect: Rect, readings: &ScopeReadings, window_ms: f32) {
    let painter = ui.painter_at(rect);
    let labels = font::mono(FONT_VALUE);
    let middle = rect.center().y;
    let half = rect.height() * 0.5;
    // Amplitude: quarters, the half and full scale labelled.
    for quarter in -4i32..=4 {
        let level = quarter as f32 / 4.0;
        let y = middle - level * half;
        let strong = quarter == 0;
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(
                1.0,
                if strong {
                    palette.line
                } else {
                    palette.line.at(0.45)
                }
                .color(),
            ),
        );
        if quarter % 2 == 0 && quarter.abs() < 4 {
            painter.text(
                pos2(rect.left() + 2.0, y - 1.0),
                Align2::LEFT_BOTTOM,
                if quarter == 0 {
                    "0".to_owned()
                } else {
                    format!("{level:+.1}")
                },
                labels.clone(),
                palette.subtle.color(),
            );
        }
    }
    // Time: ten divisions, the ends and the middle labelled.
    for division in 0..=10 {
        let x = rect.left() + rect.width() * division as f32 / 10.0;
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, palette.line.at(0.35).color()),
        );
        if division == 5 || division == 0 {
            let ago = window_ms * (1.0 - division as f32 / 10.0);
            painter.text(
                pos2(x + 2.0, rect.bottom() - 1.0),
                Align2::LEFT_BOTTOM,
                format!("-{ago:.0} ms"),
                labels.clone(),
                palette.subtle.color(),
            );
        }
    }

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
