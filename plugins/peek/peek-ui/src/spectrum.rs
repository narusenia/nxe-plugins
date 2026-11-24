//! The spectrum as a curve over a log frequency axis, with a slowly falling
//! peak line above it. **The tilt** turns the curve so pink noise reads flat:
//! music falls with frequency, and an untilted spectrum hides the top.

use egui::{Align2, Rect, Stroke, Ui, pos2};
use nxe_egui::curve::curve;
use nxe_egui::theme::{FONT_VALUE, Palette};
use nxe_egui::{font, smooth};
use peek_core::meters::{HIGH_HZ, LOW_HZ};

pub const FLOOR_DB: f32 = -84.0;
pub const TOP_DB: f32 = 0.0;
/// How fast the peak line falls, in dB a second.
const FALL_DB_PER_SECOND: f32 = 12.0;

/// Where `hz` sits across the axis, `0..=1`.
pub fn x_of(hz: f32) -> f32 {
    ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

/// A band's centre, for `bands` bands spread like `nxe_dsp::Spectrum`'s.
pub fn centre(index: usize, bands: usize) -> f32 {
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf((index as f32 + 0.5) / bands as f32)
}

pub fn axis(ui: &Ui, palette: &Palette, rect: Rect) {
    let painter = ui.painter_at(rect.expand(12.0));
    for (hz, text) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let x = rect.left() + x_of(hz) * rect.width();
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, palette.line.color()),
        );
        painter.text(
            pos2(x, rect.bottom() + 2.0),
            Align2::CENTER_TOP,
            text,
            font::mono(FONT_VALUE),
            palette.subtle.color(),
        );
    }
}

/// `levels` are amplitudes per band; `tilt` is dB per octave around 1 kHz.
pub fn figure<const BANDS: usize>(
    ui: &Ui,
    palette: &Palette,
    rect: Rect,
    levels: &[f32; BANDS],
    tilt: f32,
) {
    let db: Vec<f32> = levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let tilted = crate::decibels(*level).unwrap_or(FLOOR_DB - 12.0)
                + tilt * (centre(index, BANDS) / 1_000.0).log2();
            ((tilted - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0.0, 1.0)
        })
        .collect();
    let shown = smooth::smoothed(ui, ui.id().with("peek-spectrum"), &db, smooth::TAU);

    // The peak line, kept by the window: it is a drawing aid, not a reading.
    let id = ui.id().with("peek-spectrum-peaks");
    let dt = ui.input(|input| input.stable_dt).min(0.1);
    let fall = FALL_DB_PER_SECOND / (TOP_DB - FLOOR_DB) * dt;
    let mut peaks: Vec<f32> = ui
        .data(|data| data.get_temp(id))
        .unwrap_or_else(|| vec![0.0; BANDS]);
    peaks.resize(BANDS, 0.0);
    for (peak, value) in peaks.iter_mut().zip(&shown) {
        *peak = (*peak - fall).max(*value);
    }
    ui.data_mut(|data| data.insert_temp(id, peaks.clone()));

    axis(ui, palette, rect);
    let painter = ui.painter_at(rect);
    let points = |values: &[f32]| -> Vec<(f32, f32)> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| (x_of(centre(index, BANDS)), *value))
            .collect()
    };
    curve(&painter, rect, &points(&peaks), palette.muted, None, false);
    curve(
        &painter,
        rect,
        &points(&shown),
        palette.accent,
        Some(palette.dim),
        true,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_is_logarithmic() {
        assert_eq!(x_of(LOW_HZ), 0.0);
        assert_eq!(x_of(HIGH_HZ), 1.0);
        assert!((x_of((LOW_HZ * HIGH_HZ).sqrt()) - 0.5).abs() < 1e-5);
    }
}
