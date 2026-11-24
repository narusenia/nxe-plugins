//! Peak, RMS and the held peak per channel, as horizontal bars on a dBFS
//! scale that runs to +3 so a true peak over full scale is visible.

use egui::{Align2, Rect, Stroke, Ui, pos2};
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2};
use nxe_egui::{font, smooth};
use peek_core::meters::LevelReadings;

const FLOOR_DB: f32 = -60.0;
const TOP_DB: f32 = 3.0;
const TICKS: [f32; 10] = [
    -60.0, -48.0, -36.0, -24.0, -18.0, -12.0, -6.0, -3.0, 0.0, 3.0,
];
/// Room for the channel names left of the bars, and the scale under them.
const NAMES: f32 = 18.0;
const SCALE: f32 = 22.0;

/// An amplitude as a place on the scale, `0..=1`.
pub fn position(amplitude: f32) -> f32 {
    crate::decibels(amplitude).map_or(0.0, position_db)
}

fn position_db(db: f32) -> f32 {
    ((db - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0.0, 1.0)
}

pub fn figure(ui: &Ui, palette: &Palette, rect: Rect, readings: &LevelReadings) {
    let (peaks, rms) = (readings.peaks.read(), readings.rms.read());
    let shown = smooth::smoothed(
        ui,
        ui.id().with("peek-level"),
        &[
            position(peaks[0]),
            position(peaks[1]),
            position(rms[0]),
            position(rms[1]),
        ],
        smooth::TAU,
    );
    let holds = readings.holds.read().map(position);
    let painter = ui.painter_at(rect);
    // Room at the right for the +3 label.
    let (left, right) = (rect.left() + NAMES, rect.right() - 10.0);
    let x_of = |at: f32| left + (right - left) * at;
    let gap = (rect.height() - SCALE) * 0.15;
    let bar = (rect.height() - SCALE - gap) * 0.5;
    let bottom = rect.top() + 2.0 * bar + gap;

    painter.rect_filled(
        Rect::from_min_max(
            pos2(x_of(position_db(0.0)), rect.top()),
            pos2(right, bottom),
        ),
        0.0,
        palette.dim.at(0.06).color(),
    );
    // Labels that would touch are left out, right to left, so 0 and +3 stay.
    let mut last_label = f32::INFINITY;
    for db in TICKS.into_iter().rev() {
        let x = x_of(position_db(db));
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, bottom)],
            Stroke::new(1.0, palette.line.color()),
        );
        if last_label - x < 22.0 {
            continue;
        }
        last_label = x;
        painter.text(
            pos2(x, bottom + SPACE_2),
            Align2::CENTER_TOP,
            if db > 0.0 {
                format!("+{db:.0}")
            } else {
                format!("{db:.0}")
            },
            font::mono(FONT_VALUE),
            if db >= 0.0 {
                palette.ink.color()
            } else {
                palette.subtle.color()
            },
        );
    }
    for (index, name) in ["L", "R"].into_iter().enumerate() {
        let y = rect.top() + index as f32 * (bar + gap);
        let cell = Rect::from_min_max(pos2(left, y), pos2(right, y + bar));
        painter.text(
            pos2(rect.left(), cell.center().y),
            Align2::LEFT_CENTER,
            name,
            font::medium(FONT_VALUE),
            palette.muted.color(),
        );
        painter.rect_filled(cell, 0.0, palette.track.color());
        let peak = x_of(shown[index]);
        painter.rect_filled(
            Rect::from_min_max(cell.min, pos2(peak, cell.bottom())),
            0.0,
            palette.accent.at(0.35).color(),
        );
        let inset = (bar * 0.2).min(SPACE_2);
        painter.rect_filled(
            Rect::from_min_max(
                pos2(left, y + inset),
                pos2(x_of(shown[2 + index]), cell.bottom() - inset),
            ),
            0.0,
            palette.accent.color(),
        );
        if shown[index] > 0.0 {
            painter.line_segment(
                [pos2(peak, cell.top()), pos2(peak, cell.bottom())],
                Stroke::new(2.0, palette.accent.color()),
            );
        }
        if holds[index] > 0.0 {
            let hold = x_of(holds[index]);
            painter.line_segment(
                [
                    pos2(hold, cell.top() - 3.0),
                    pos2(hold, cell.bottom() + 3.0),
                ],
                Stroke::new(1.0, palette.bright.color()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scale_sits_three_decibels_from_the_end() {
        assert_eq!(position(0.0), 0.0);
        assert!((position(1.0) - 60.0 / 63.0).abs() < 1e-6);
        assert_eq!(position(2.0), 1.0);
    }
}
