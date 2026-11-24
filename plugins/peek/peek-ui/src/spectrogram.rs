//! Time across, frequency up, loudness as the accent's strength. A texture the
//! window keeps, one new column per frame drawn at the right; nothing is
//! redrawn but the column.
//!
//! shortcut: one column per window frame (30 Hz) from the latest hand-off, so
//! a column can skip a block the window did not see; a ring of frames from the
//! audio thread (the plan's `nxe-spectral`) is the upgrade if that shows.

use crate::spectrum::{Axis, hz_at, position, ticks, x_of};
use egui::{Align2, Color32, ColorImage, Rect, Stroke, TextureHandle, TextureOptions, Ui, pos2};
use nxe_egui::font;
use nxe_egui::theme::{FONT_VALUE, Palette};
use std::sync::Arc;

/// Columns kept: 8 seconds at 30 Hz.
pub const COLUMNS: usize = 240;
const COLUMNS_PER_SECOND: f64 = 30.0;
/// Rows in the picture. More than the bands, so the linear axis, which gives
/// the low bands only a few rows, still has rows for the top.
const ROWS: usize = 160;

#[derive(Clone)]
struct State {
    pixels: Arc<Vec<Color32>>,
    texture: TextureHandle,
    last: f64,
    /// The axis the columns so far were drawn on. A change clears them.
    axis: Axis,
}

/// `levels` are amplitudes per band (log-spaced, low first); `floor` and
/// `ceiling` in dB set where the accent starts and where it is full.
#[allow(clippy::too_many_arguments)]
pub fn figure<const BANDS: usize>(
    ui: &Ui,
    palette: &Palette,
    rect: Rect,
    levels: &[f32; BANDS],
    floor: f32,
    ceiling: f32,
    axis: Axis,
) {
    let id = ui.id().with("peek-spectrogram");
    let blank = || vec![palette.ground.color(); COLUMNS * ROWS];
    let mut state: State = ui.data(|data| data.get_temp(id)).unwrap_or_else(|| {
        let pixels = blank();
        let texture = ui.ctx().load_texture(
            "peek-spectrogram",
            ColorImage::new([COLUMNS, ROWS], pixels.clone()),
            TextureOptions::LINEAR,
        );
        State {
            pixels: Arc::new(pixels),
            texture,
            last: 0.0,
            axis,
        }
    });
    if state.axis != axis {
        state.axis = axis;
        state.pixels = Arc::new(blank());
    }

    // One column per 30th of a second, however often the window is drawn.
    let now = ui.input(|input| input.time);
    if now - state.last >= 1.0 / COLUMNS_PER_SECOND {
        state.last = now;
        let pixels = Arc::make_mut(&mut state.pixels);
        let span = (ceiling - floor).max(1.0);
        for row in 0..ROWS {
            let line = &mut pixels[row * COLUMNS..(row + 1) * COLUMNS];
            line.copy_within(1.., 0);
            // Row 0 is the top of the picture; each row reads the band its
            // frequency falls in.
            let hz = hz_at(1.0 - (row as f32 + 0.5) / ROWS as f32, axis);
            let band = ((x_of(hz) * BANDS as f32) as usize).min(BANDS - 1);
            // Squared, so the loud half of the range keeps its steps: linear,
            // music sat near the top and the picture went flat grey.
            let strength = crate::decibels(levels[band])
                .map_or(0.0, |db| ((db - floor) / span).clamp(0.0, 1.0))
                .powi(2);
            line[COLUMNS - 1] = palette.ground.mix(palette.accent, strength).color();
        }
        state.texture.set(
            ColorImage::new([COLUMNS, ROWS], pixels.clone()),
            TextureOptions::LINEAR,
        );
    }
    let painter = ui.painter_at(rect);
    painter.image(
        state.texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    let labels = font::mono(FONT_VALUE);
    let mut last_label = f32::INFINITY;
    for (hz, text, strong) in ticks(axis) {
        let y = rect.bottom() - position(hz, axis) * rect.height();
        painter.line_segment(
            [
                pos2(rect.left(), y),
                pos2(rect.left() + if strong { 8.0 } else { 4.0 }, y),
            ],
            Stroke::new(1.0, palette.muted.color()),
        );
        if last_label - y >= 14.0 && y > rect.top() + 6.0 && y < rect.bottom() - 20.0 {
            last_label = y;
            painter.text(
                pos2(rect.left() + 10.0, y),
                Align2::LEFT_CENTER,
                text,
                labels.clone(),
                palette.muted.color(),
            );
        }
    }
    // A second a tick along the bottom, every other one labelled.
    let per_second = rect.width() / (COLUMNS as f32 / COLUMNS_PER_SECOND as f32);
    for second in 0..=(COLUMNS / COLUMNS_PER_SECOND as usize) {
        let x = rect.right() - second as f32 * per_second;
        painter.line_segment(
            [
                pos2(x, rect.bottom()),
                pos2(x, rect.bottom() - if second % 2 == 0 { 6.0 } else { 3.0 }),
            ],
            Stroke::new(1.0, palette.muted.color()),
        );
        // The oldest tick sits on the edge, where its label would be cut.
        if second % 2 == 0 && second > 0 && x - rect.left() > 12.0 {
            painter.text(
                pos2(x, rect.bottom() - 8.0),
                Align2::CENTER_BOTTOM,
                format!("-{second}s"),
                labels.clone(),
                palette.subtle.color(),
            );
        }
    }
    ui.data_mut(|data| data.insert_temp(id, state));
}
