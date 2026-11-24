//! Time across, frequency up, loudness as the accent's strength. A texture the
//! window keeps, one new column per frame drawn at the right; nothing is
//! redrawn but the column.
//!
//! shortcut: one column per window frame (30 Hz) from the latest hand-off, so
//! a column can skip a block the window did not see; a ring of frames from the
//! audio thread (the plan's `nxe-spectral`) is the upgrade if that shows.

use egui::{Color32, ColorImage, Rect, TextureHandle, TextureOptions, Ui, pos2};
use nxe_egui::theme::Palette;
use std::sync::Arc;

/// Columns kept: about 8 seconds at 30 Hz.
pub const COLUMNS: usize = 240;

#[derive(Clone)]
struct State {
    pixels: Arc<Vec<Color32>>,
    texture: TextureHandle,
    rows: usize,
    last: f64,
}

/// `levels` are amplitudes per band, low first; `floor` and `ceiling` in dB
/// set where the accent starts and where it is full.
pub fn figure<const BANDS: usize>(
    ui: &Ui,
    palette: &Palette,
    rect: Rect,
    levels: &[f32; BANDS],
    floor: f32,
    ceiling: f32,
) {
    let id = ui.id().with("peek-spectrogram");
    let mut state: State = ui.data(|data| data.get_temp(id)).unwrap_or_else(|| {
        let pixels = vec![palette.ground.color(); COLUMNS * BANDS];
        let texture = ui.ctx().load_texture(
            "peek-spectrogram",
            ColorImage::new([COLUMNS, BANDS], pixels.clone()),
            TextureOptions::LINEAR,
        );
        State {
            pixels: Arc::new(pixels),
            texture,
            rows: BANDS,
            last: 0.0,
        }
    });
    // One column per 30th of a second, however often the window is drawn.
    let now = ui.input(|input| input.time);
    if now - state.last >= 1.0 / 30.0 && state.rows == BANDS {
        state.last = now;
        let pixels = Arc::make_mut(&mut state.pixels);
        let span = (ceiling - floor).max(1.0);
        for row in 0..BANDS {
            let line = &mut pixels[row * COLUMNS..(row + 1) * COLUMNS];
            line.copy_within(1.., 0);
            // Row 0 is the top of the picture: the highest band.
            let level = levels[BANDS - 1 - row];
            // Squared, so the loud half of the range keeps its steps: linear,
            // music sat near the top and the picture went flat grey.
            let strength = crate::decibels(level)
                .map_or(0.0, |db| ((db - floor) / span).clamp(0.0, 1.0))
                .powi(2);
            line[COLUMNS - 1] = palette.ground.mix(palette.accent, strength).color();
        }
        state.texture.set(
            ColorImage::new([COLUMNS, BANDS], pixels.clone()),
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
    for (hz, text) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let y = rect.bottom() - crate::spectrum::x_of(hz) * rect.height();
        painter.line_segment(
            [pos2(rect.left(), y), pos2(rect.left() + 6.0, y)],
            egui::Stroke::new(1.0, palette.muted.color()),
        );
        painter.text(
            pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_CENTER,
            text,
            nxe_egui::font::mono(nxe_egui::theme::FONT_VALUE),
            palette.muted.color(),
        );
    }
    ui.data_mut(|data| data.insert_temp(id, state));
}
