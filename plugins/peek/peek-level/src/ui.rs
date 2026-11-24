//! The Peek Level window, on the Kit layout (`nxe_egui::kit`).
//!
//! ```text
//! ┌──┬──────────────────────────────────┐
//! │PK│ Level                 dBFS −60…+3│
//! │  │ L ████████▓▓▓▓▓▓▓▓░░░░│          │
//! │  │ R ███████▓▓▓▓▓▓▓░░░░░░ │         │
//! │NX│   −60 −48 … −6 −3  0  +3         │
//! ├──┴──────────────────────────────────┤
//! │ PEAK   TRUE PEAK   RMS    CLICK TO  │
//! │ −1.2   +0.4        −14.2  RESET     │
//! └─────────────────────────────────────┘
//! ```
//!
//! Per channel, the bar is the peak, the narrower bar inside it the RMS, and
//! the line the held peak. **The scale runs to +3 dBFS** so a true peak over
//! full scale has somewhere to be seen.

use crate::params::PeekLevelParams;
use egui::{Align2, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::readout::{figure, readout};
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_2, SPACE_3, SPACE_4};
use nxe_egui::{font, smooth, window};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekLevelParams>;

const PALETTE: Palette = Palette::PEEK;
const FLOOR_DB: f32 = -60.0;
const TOP_DB: f32 = 3.0;
const TICKS: [f32; 10] = [
    -60.0, -48.0, -36.0, -24.0, -18.0, -12.0, -6.0, -3.0, 0.0, 3.0,
];
const BAR: f32 = 48.0;
const GAP: f32 = 24.0;
/// The left margin for the channel names.
const NAMES: f32 = 22.0;
const CELL: f32 = 76.0;
const ANALYSIS_POLL: std::time::Duration = std::time::Duration::from_millis(100);

pub fn create(params: Arc<PeekLevelParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !window(ui) {
        return;
    }
    let analysis = &ctx.params().analysis;
    let peaks = analysis.peaks.read();
    let sounding = peaks.iter().any(|peak| *peak > 1e-4);
    ui.ctx().request_repaint_after(if sounding {
        smooth::FRAME
    } else {
        ANALYSIS_POLL
    });
    let palette = &PALETTE;
    let areas = kit(ui, palette, "Level", "Peek Kit");

    // The corner says what the scale is.
    font::tracked(
        ui.painter(),
        pos2(areas.figure.right() - SPACE_3, areas.figure.top() + SPACE_3),
        Align2::RIGHT_TOP,
        "dBFS  −60 … +3",
        FONT_EYEBROW,
        palette.subtle.color(),
    );

    let shown = smooth::smoothed(
        ui,
        ui.id().with("levels"),
        &[
            position(peaks[0]),
            position(peaks[1]),
            position(analysis.rms.read()[0]),
            position(analysis.rms.read()[1]),
        ],
        smooth::TAU,
    );
    let holds = analysis.holds.read().map(position);
    let left = areas.figure.left() + SPACE_3 + NAMES;
    let right = areas.figure.right() - SPACE_4;
    let top = areas.figure.top() + kit::HEAD + SPACE_2;
    let painter = ui.painter_at(areas.figure);
    let x_of = |at: f32| left + (right - left) * at;

    // Over full scale is a region of its own, faintly lit.
    let over = Rect::from_min_max(
        pos2(x_of(position(1.0)), top),
        pos2(right, top + 2.0 * BAR + GAP),
    );
    painter.rect_filled(over, 0.0, palette.dim.at(0.06).color());
    for db in TICKS {
        let x = x_of(position_db(db));
        painter.line_segment(
            [pos2(x, top), pos2(x, top + 2.0 * BAR + GAP)],
            Stroke::new(1.0, palette.line.color()),
        );
        painter.text(
            pos2(x, top + 2.0 * BAR + GAP + SPACE_2),
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
        let y = top + index as f32 * (BAR + GAP);
        let bar = Rect::from_min_max(pos2(left, y), pos2(right, y + BAR));
        painter.text(
            pos2(areas.figure.left() + SPACE_3, bar.center().y),
            Align2::LEFT_CENTER,
            name,
            font::medium(FONT_VALUE),
            palette.muted.color(),
        );
        painter.rect_filled(bar, 0.0, palette.track.color());
        let peak = x_of(shown[index]);
        painter.rect_filled(
            Rect::from_min_max(bar.min, pos2(peak, bar.bottom())),
            0.0,
            palette.accent.at(0.35).color(),
        );
        painter.rect_filled(
            Rect::from_min_max(
                pos2(left, y + SPACE_2),
                pos2(x_of(shown[2 + index]), bar.bottom() - SPACE_2),
            ),
            0.0,
            palette.accent.color(),
        );
        if shown[index] > 0.0 {
            painter.line_segment(
                [pos2(peak, bar.top()), pos2(peak, bar.bottom())],
                Stroke::new(2.0, palette.accent.color()),
            );
        }
        if holds[index] > 0.0 {
            let hold = x_of(holds[index]);
            painter.line_segment(
                [pos2(hold, bar.top() - 3.0), pos2(hold, bar.bottom() + 3.0)],
                Stroke::new(1.0, palette.bright.color()),
            );
        }
    }

    band(ui, ctx, palette, areas.band);
}

/// The maxima since the last clear. A click anywhere on the band clears them.
fn band(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let analysis = &ctx.params().analysis;
    let response = ui.interact(rect, ui.id().with("clear"), Sense::click());
    if response.clicked() {
        analysis.clears.fetch_add(1, Ordering::Relaxed);
    }
    let [peak, true_peak, rms] = analysis.maxima.read().map(decibels);
    let ink = palette.inverted();
    let inner = rect.shrink2(vec2(SPACE_4, 0.0));
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(
                    inner.left(),
                    rect.center().y - nxe_egui::readout::HEIGHT * 0.5,
                ),
                vec2(inner.width(), nxe_egui::readout::HEIGHT),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    let cells: [(&str, Option<f32>, &str); 3] = [
        ("Peak", peak, "dB"),
        ("True peak", true_peak, "dBTP"),
        ("RMS", rms, "dB"),
    ];
    for (name, value, unit) in cells {
        // A true peak over full scale is drawn dark on the band: the one
        // reading a meter has to make impossible to miss.
        if name == "True peak" && value.is_some_and(|db| db > 0.0) {
            let at = row.cursor().min;
            let cell = Rect::from_min_size(at, vec2(CELL, nxe_egui::readout::HEIGHT))
                .expand2(vec2(SPACE_2 * 0.5, SPACE_2 * 0.5));
            row.painter().rect_filled(cell, 0.0, ink.ink.color());
            readout(&mut row, palette, name, &figure(value, 1), unit, CELL);
        } else {
            readout(&mut row, &ink, name, &figure(value, 1), unit, CELL);
        }
    }
    font::tracked(
        ui.painter(),
        pos2(inner.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        "Click to clear",
        FONT_EYEBROW,
        if response.hovered() {
            ink.ink.color()
        } else {
            ink.subtle.color()
        },
    );
}

/// An amplitude as decibels, `None` for silence.
fn decibels(amplitude: f32) -> Option<f32> {
    (amplitude > 1e-6).then(|| 20.0 * amplitude.log10())
}

/// An amplitude as a place on the scale, `0..=1`.
pub fn position(amplitude: f32) -> f32 {
    decibels(amplitude).map_or(0.0, position_db)
}

fn position_db(db: f32) -> f32 {
    ((db - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0.0, 1.0)
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
