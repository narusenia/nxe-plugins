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
    let painter = ui.painter_at(rect.expand(14.0));
    let labels = font::mono(FONT_VALUE);
    // Every 1-2-5 step gets a line; the decades are stronger and every step
    // that fits gets its figure.
    let mut last_label = f32::NEG_INFINITY;
    for (hz, text) in [
        (20.0, "20"),
        (50.0, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1_000.0, "1k"),
        (2_000.0, "2k"),
        (5_000.0, "5k"),
        (10_000.0, "10k"),
        (20_000.0, "20k"),
    ] {
        let x = rect.left() + x_of(hz) * rect.width();
        let decade = text.starts_with('1') || hz == 20.0;
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(
                1.0,
                if decade {
                    palette.line
                } else {
                    palette.line.at(0.5)
                }
                .color(),
            ),
        );
        if x - last_label >= 26.0 {
            last_label = x;
            painter.text(
                pos2(x, rect.bottom() + 2.0),
                Align2::CENTER_TOP,
                text,
                labels.clone(),
                if decade {
                    palette.muted
                } else {
                    palette.subtle
                }
                .color(),
            );
        }
    }
    let mut db = TOP_DB - 12.0;
    while db > FLOOR_DB {
        let y = rect.bottom() - (db - FLOOR_DB) / (TOP_DB - FLOOR_DB) * rect.height();
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.right(), y)],
            Stroke::new(1.0, palette.line.at(0.5).color()),
        );
        painter.text(
            pos2(rect.right() - 2.0, y - 1.0),
            Align2::RIGHT_BOTTOM,
            format!("{db:.0}"),
            labels.clone(),
            palette.subtle.color(),
        );
        db -= 12.0;
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
            // Silence stays on the floor: tilting it lifted the top end of a
            // silent spectrum by 19 dB.
            crate::decibels(*level).map_or(0.0, |db| {
                let tilted = db + tilt * (centre(index, BANDS) / 1_000.0).log2();
                ((tilted - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0.0, 1.0)
            })
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
    fn silence_is_on_the_floor_whatever_the_tilt() {
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(Default::default(), |ui| {
            let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(100.0, 100.0));
            figure(ui, &Palette::PEEK, rect, &[0.0f32; 8], 4.5);
            let shown: Vec<f32> = ui
                .data(|data| data.get_temp(ui.id().with("peek-spectrum-peaks")))
                .unwrap();
            assert!(shown.iter().all(|value| *value == 0.0), "{shown:?}");
        });
    }

    #[test]
    fn the_axis_is_logarithmic() {
        assert_eq!(x_of(LOW_HZ), 0.0);
        assert_eq!(x_of(HIGH_HZ), 1.0);
        assert!((x_of((LOW_HZ * HIGH_HZ).sqrt()) - 0.5).abs() < 1e-5);
    }
}
