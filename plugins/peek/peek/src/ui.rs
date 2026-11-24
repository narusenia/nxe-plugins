//! The Peek window: every meter at once, on the Kit frame stretched to the
//! window. **The host may resize it** — the window never asks to change size
//! itself, which is what froze Ableton in v0 (`docs/HANDOVER.md`); everything
//! below is laid out from the rect it is given.
//!
//! ```text
//! ┌──┬─────────────────────────────────────────────────┐
//! │PK│ Peek                                             │
//! │  │ LEVEL       │ VU        │ LOUDNESS   │ STEREO    │
//! │  ├─────────────┴───────────┼────────────┼───────────┤
//! │NX│ SPECTRUM                │ SPECTROGRAM│ SCOPE     │
//! ├──┴─────────────────────────┴────────────┴───────────┤
//! │ TRUE PEAK  INTEGRATED  RANGE  CORRELATION   knobs   │
//! └─────────────────────────────────────────────────────┘
//! ```

use crate::params::{PeekParams, Target, Tilt};
use egui::{Align2, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::font;
use nxe_egui::kit::{self, kit};
use nxe_egui::readout::HEIGHT as READOUT;
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_2, SPACE_3, SPACE_4};
use peek_ui::{bind, decibels};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekParams>;

const PALETTE: Palette = Palette::PEEK;
const SIZE: (u32, u32) = (960, 600);
const MIN: (u32, u32) = (760, 480);
const MAX: (u32, u32) = (1920, 1200);
/// A cell's title row.
const TITLE: f32 = 22.0;

pub fn create(params: Arc<PeekParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, SIZE, view)
        .with_font_definitions(nxe_egui::font::definitions)
        .resizable(true)
        .min_size(MIN)
        .max_size(MAX)
        .into_editor()
}

/// Splits `rect` across at the given fractions.
fn columns(rect: Rect, fractions: &[f32]) -> Vec<Rect> {
    let mut left = rect.left();
    fractions
        .iter()
        .map(|fraction| {
            let right = left + rect.width() * fraction;
            let cell = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
            left = right;
            cell
        })
        .collect()
}

/// Draws a cell's title and rule and returns the area under it.
fn cell(ui: &Ui, rect: Rect, title: &str) -> Rect {
    let painter = ui.painter();
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, PALETTE.line.color()),
        egui::StrokeKind::Inside,
    );
    font::tracked(
        painter,
        rect.left_top() + vec2(SPACE_3, SPACE_2),
        Align2::LEFT_TOP,
        title,
        FONT_EYEBROW,
        PALETTE.muted.color(),
    );
    Rect::from_min_max(
        rect.left_top() + vec2(SPACE_3, TITLE + SPACE_2),
        rect.right_bottom() - vec2(SPACE_3, SPACE_3),
    )
}

/// A segmented choice at the top right of a cell.
fn corner(ui: &mut Ui, ctx: &Ctx, rect: Rect, id: u32, names: &[&str], index: usize, width: f32) {
    let mut child = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        pos2(rect.right() - width - SPACE_2, rect.top() + 3.0),
        vec2(width, nxe_egui::segment::HEIGHT),
    )));
    peek_ui::choice(&mut child, ctx, &PALETTE, id, names, index, width);
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let readings = &params.readings;
    // The spectrogram scrolls in silence too.
    peek_ui::heartbeat(ui, true);
    // Not a Kit: the strip names what the window is for.
    let areas = kit(ui, &PALETTE, "Peek", "Metering");
    let grid = Rect::from_min_max(
        pos2(areas.figure.left(), areas.figure.top() + kit::HEAD),
        areas.figure.max,
    );
    let middle = grid.center().y;
    let top = Rect::from_min_max(grid.min, pos2(grid.right(), middle));
    let bottom = Rect::from_min_max(pos2(grid.left(), middle), grid.max);

    let upper = columns(top, &[0.3, 0.24, 0.28, 0.18]);
    let reference = params.reference.value();
    peek_ui::level::figure(ui, &PALETTE, cell(ui, upper[0], "Level"), &readings.level);
    let vu = readings
        .vu
        .vu
        .read()
        .map(|amplitude| peek_ui::vu::vu_of(amplitude, reference));
    peek_ui::vu::arc(ui, &PALETTE, cell(ui, upper[1], "VU"), vu);
    let target = params.target.index() as usize;
    peek_ui::loudness::figure(
        ui,
        &PALETTE,
        cell(ui, upper[2], "Loudness"),
        readings.loudness.values.read(),
        Target::LUFS[target],
        false,
    );
    corner(
        ui,
        ctx,
        upper[2],
        params.target.id(),
        &Target::NAMES,
        target,
        112.0,
    );
    peek_ui::stereo::figure(ui, &PALETTE, cell(ui, upper[3], "Stereo"), &readings.stereo);

    let lower = columns(bottom, &[0.42, 0.33, 0.25]);
    let tilt = params.tilt.index() as usize;
    let spectrum = cell(ui, lower[0], "Spectrum");
    peek_ui::spectrum::figure(
        ui,
        &PALETTE,
        Rect::from_min_max(spectrum.min, spectrum.max - vec2(0.0, 14.0)),
        &readings.spectrum.levels.read(),
        Tilt::SLOPES[tilt],
    );
    corner(
        ui,
        ctx,
        lower[0],
        params.tilt.id(),
        &["Flat", "3 dB", "4.5 dB"],
        tilt,
        132.0,
    );
    peek_ui::spectrogram::figure(
        ui,
        &PALETTE,
        cell(ui, lower[1], "Spectrogram"),
        &readings.spectrogram.levels.read(),
        -72.0,
        -18.0,
    );
    peek_ui::scope::figure(ui, &PALETTE, cell(ui, lower[2], "Scope"), &readings.scope);

    band(ui, ctx, areas.band);
}

/// The figures that need a number, the controls, and a click to clear the
/// level maxima and the integrated loudness together.
fn band(ui: &mut Ui, ctx: &Ctx, rect: Rect) {
    let params = ctx.params();
    let readings = &params.readings;
    let clear = ui.interact(rect, ui.id().with("clear"), Sense::click());
    if clear.clicked() {
        readings.level.clears.fetch_add(1, Ordering::Relaxed);
        readings.loudness.clears.fetch_add(1, Ordering::Relaxed);
    }
    let ink = PALETTE.inverted();
    let inner = rect.shrink2(vec2(SPACE_4, 0.0));
    let true_peak = decibels(readings.level.maxima.read()[1]);
    let loudness = readings.loudness.values.read();
    let finite = |value: f32| value.is_finite().then_some(value);
    peek_ui::readings(
        ui,
        &PALETTE,
        inner,
        &[
            peek_ui::Reading {
                name: "True peak",
                value: true_peak,
                unit: "dBTP",
                alarm: true_peak.is_some_and(|db| db > 0.0),
            },
            peek_ui::Reading {
                name: "Integrated",
                value: finite(loudness[2]),
                unit: "LUFS",
                alarm: false,
            },
            peek_ui::Reading {
                name: "Range",
                value: Some(loudness[3]),
                unit: "LU",
                alarm: false,
            },
            peek_ui::Reading {
                name: "Max M",
                value: finite(loudness[4]),
                unit: "LUFS",
                alarm: false,
            },
        ],
    );

    // The right-hand side: correlation, the two knobs, and how to clear.
    let right = Rect::from_min_max(pos2(inner.right() - 366.0, inner.top()), inner.max);
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(right)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    peek_ui::bipolar_cell(
        &mut row,
        &ink,
        "Correlation",
        readings.stereo.figures.read()[0],
        136.0,
    );
    bind::knob_for(&mut row, ctx, &ink, params.reference.id(), "0 VU", 28.0);
    bind::knob_for(&mut row, ctx, &ink, params.window.id(), "Scope", 28.0);
    let (hint, _) = row.allocate_exact_size(vec2(56.0, READOUT), Sense::hover());
    font::tracked(
        ui.painter(),
        hint.right_center(),
        Align2::RIGHT_CENTER,
        "Clear",
        FONT_EYEBROW,
        if clear.hovered() { ink.ink } else { ink.subtle }.color(),
    );
}
