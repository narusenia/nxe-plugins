//! Time across, frequency up, loudness as the accent's strength. A texture the
//! window keeps, new columns drawn at the right; nothing is redrawn but them.
//!
//! Two ways in. [`figure`] adds one column per window frame (30 Hz) from the
//! latest hand-off, so a column can skip a block the window did not see.
//! [`figure_frames`] takes every frame the audio thread made since the last
//! call, one column each — what Shear needs to show a lean of a few tens of
//! milliseconds. shortcut: Peek still uses the first; move it to the second
//! (a ring of frames from the audio thread) if the skipping shows.

use crate::spectrum::{Axis, hz_at, position, ticks, x_of};
use egui::{Align2, Color32, ColorImage, Rect, Stroke, TextureHandle, TextureOptions, Ui, pos2};
use nxe_egui::font;
use nxe_egui::theme::{FONT_VALUE, Palette};
use std::sync::Arc;

/// Columns kept by [`figure`]: 8 seconds at 30 Hz.
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
    columns: usize,
    /// The axis the columns so far were drawn on. A change clears them.
    axis: Axis,
}

/// The window's texture for `columns` columns, cleared when the axis or the
/// column count changes.
fn state(ui: &Ui, palette: &Palette, columns: usize, axis: Axis) -> (egui::Id, State) {
    let id = ui.id().with("peek-spectrogram");
    let blank = || vec![palette.ground.color(); columns * ROWS];
    let mut state: State = ui
        .data(|data| data.get_temp(id))
        .filter(|state: &State| state.columns == columns)
        .unwrap_or_else(|| {
            let pixels = blank();
            let texture = ui.ctx().load_texture(
                "peek-spectrogram",
                ColorImage::new([columns, ROWS], pixels.clone()),
                TextureOptions::LINEAR,
            );
            State {
                pixels: Arc::new(pixels),
                texture,
                last: 0.0,
                columns,
                axis,
            }
        });
    if state.axis != axis {
        state.axis = axis;
        state.pixels = Arc::new(blank());
    }
    (id, state)
}

/// Shifts the picture one column left and draws `levels` at the right.
fn push<const BANDS: usize>(
    state: &mut State,
    palette: &Palette,
    levels: &[f32; BANDS],
    floor: f32,
    ceiling: f32,
) {
    let columns = state.columns;
    let pixels = Arc::make_mut(&mut state.pixels);
    let span = (ceiling - floor).max(1.0);
    for row in 0..ROWS {
        let line = &mut pixels[row * columns..(row + 1) * columns];
        line.copy_within(1.., 0);
        // Row 0 is the top of the picture; each row reads the band its
        // frequency falls in.
        let hz = hz_at(1.0 - (row as f32 + 0.5) / ROWS as f32, state.axis);
        let band = ((x_of(hz) * BANDS as f32) as usize).min(BANDS - 1);
        // Squared, so the loud half of the range keeps its steps: linear,
        // music sat near the top and the picture went flat grey.
        let strength = crate::decibels(levels[band])
            .map_or(0.0, |db| ((db - floor) / span).clamp(0.0, 1.0))
            .powi(2);
        line[columns - 1] = palette.ground.mix(palette.accent, strength).color();
    }
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
    let (id, mut state) = state(ui, palette, COLUMNS, axis);
    // One column per 30th of a second, however often the window is drawn.
    let now = ui.input(|input| input.time);
    if now - state.last >= 1.0 / COLUMNS_PER_SECOND {
        state.last = now;
        push(&mut state, palette, levels, floor, ceiling);
        state.texture.set(
            ColorImage::new([COLUMNS, ROWS], state.pixels.to_vec()),
            TextureOptions::LINEAR,
        );
    }
    paint(
        ui,
        palette,
        rect,
        &state,
        COLUMNS as f32 / COLUMNS_PER_SECOND as f32,
    );
    ui.data_mut(|data| data.insert_temp(id, state));
}

/// One column per frame in `frames` (oldest first), `columns` of them across
/// `seconds`.
#[allow(clippy::too_many_arguments)]
pub fn figure_frames<const BANDS: usize>(
    ui: &Ui,
    palette: &Palette,
    rect: Rect,
    frames: &[[f32; BANDS]],
    floor: f32,
    ceiling: f32,
    axis: Axis,
    columns: usize,
    seconds: f32,
) {
    let columns = columns.max(2);
    let (id, mut state) = state(ui, palette, columns, axis);
    if !frames.is_empty() {
        for levels in frames {
            push(&mut state, palette, levels, floor, ceiling);
        }
        state.texture.set(
            ColorImage::new([columns, ROWS], state.pixels.to_vec()),
            TextureOptions::LINEAR,
        );
    }
    paint(ui, palette, rect, &state, seconds);
    ui.data_mut(|data| data.insert_temp(id, state));
}

/// The picture and its scales; `seconds` is what the width covers.
fn paint(ui: &Ui, palette: &Palette, rect: Rect, state: &State, seconds: f32) {
    let painter = ui.painter_at(rect);
    painter.image(
        state.texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );

    let labels = font::mono(FONT_VALUE);
    let mut last_label = f32::INFINITY;
    for (hz, text, strong) in ticks(state.axis) {
        let y = rect.bottom() - position(hz, state.axis) * rect.height();
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
    // A tick a second, or a quarter of one on a short picture; every other
    // one labelled.
    let step = if seconds > 4.0 { 1.0 } else { 0.25 };
    let per_second = rect.width() / seconds.max(f32::EPSILON);
    let count = (seconds / step).round() as usize;
    for tick in 0..=count {
        let at = tick as f32 * step;
        let x = rect.right() - at * per_second;
        painter.line_segment(
            [
                pos2(x, rect.bottom()),
                pos2(x, rect.bottom() - if tick % 2 == 0 { 6.0 } else { 3.0 }),
            ],
            Stroke::new(1.0, palette.muted.color()),
        );
        // The oldest tick sits on the edge, where its label would be cut.
        if tick % 2 == 0 && tick > 0 && x - rect.left() > 16.0 {
            let text = if step < 1.0 {
                format!("-{at:.1}s")
            } else {
                format!("-{at:.0}s")
            };
            painter.text(
                pos2(x, rect.bottom() - 8.0),
                Align2::CENTER_BOTTOM,
                text,
                labels.clone(),
                palette.subtle.color(),
            );
        }
    }
}
