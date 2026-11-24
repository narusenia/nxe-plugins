//! The Peek Level window, on the Kit layout (`nxe_egui::kit`).
//!
//! ```text
//! ┌──┬──────────────────────────────────┐
//! │PK│ Level                 dBFS −60…+3│
//! │  │ L ████████▓▓▓▓▓▓▓▓░░░░│          │
//! │  │ R ███████▓▓▓▓▓▓▓░░░░░░ │         │
//! │NX│   −60 −48 … −6 −3  0  +3         │
//! ├──┴──────────────────────────────────┤
//! │ PEAK   TRUE PEAK   RMS       CLEAR  │
//! └─────────────────────────────────────┘
//! ```
//!
//! The figure is `peek_ui::level`, the same one `Peek` draws. A click on the
//! band clears the maxima.

use crate::params::PeekLevelParams;
use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use nxe_egui::font;
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_2, SPACE_3, SPACE_4};
use peek_ui::{Reading, decibels};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekLevelParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekLevelParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let real = &ctx.params().readings;
    let silent;
    let readings = if peek_ui::live(ui, "level", &real.blocks) {
        real
    } else {
        silent = peek_core::meters::LevelReadings::default();
        &silent
    };
    peek_ui::heartbeat(ui, readings.peaks.read().iter().any(|peak| *peak > 1e-4));
    let palette = &PALETTE;
    let areas = kit(ui, palette, "Level", "Peek Kit");
    font::tracked(
        ui.painter(),
        pos2(areas.figure.right() - SPACE_3, areas.figure.top() + SPACE_3),
        Align2::RIGHT_TOP,
        "dBFS  −60 … +3",
        FONT_EYEBROW,
        palette.subtle.color(),
    );
    let figure = Rect::from_min_max(
        pos2(
            areas.figure.left() + SPACE_3,
            areas.figure.top() + kit::HEAD + SPACE_2,
        ),
        areas.figure.max - vec2(SPACE_4, SPACE_3),
    );
    peek_ui::level::figure(ui, palette, figure, readings);

    let band = areas.band.shrink2(vec2(SPACE_4, 0.0));
    let clear = ui.interact(areas.band, ui.id().with("clear"), Sense::click());
    if clear.clicked() {
        real.clears.fetch_add(1, Ordering::Relaxed);
    }
    let [peak, true_peak, rms] = real.maxima.read().map(decibels);
    peek_ui::readings(
        ui,
        palette,
        band,
        &[
            Reading {
                name: "Peak",
                value: peak,
                unit: "dB",
                alarm: false,
            },
            Reading {
                name: "True peak",
                value: true_peak,
                unit: "dBTP",
                // The one reading a meter has to make impossible to miss.
                alarm: true_peak.is_some_and(|db| db > 0.0),
            },
            Reading {
                name: "RMS",
                value: rms,
                unit: "dB",
                alarm: false,
            },
        ],
    );
    let ink = palette.inverted();
    font::tracked(
        ui.painter(),
        pos2(band.right(), band.center().y),
        Align2::RIGHT_CENTER,
        "Click to clear",
        FONT_EYEBROW,
        if clear.hovered() { ink.ink } else { ink.subtle }.color(),
    );
}
