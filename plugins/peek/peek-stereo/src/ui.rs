//! The Peek Stereo window: the goniometer, and correlation and balance on the band.

use crate::params::PeekStereoParams;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekStereoParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekStereoParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

/// The figure's area under the wordmark, and the band's inner rect.
fn areas(ui: &Ui, name: &str) -> (Rect, Rect) {
    let areas = kit(ui, &PALETTE, name, "Peek Kit");
    let figure = Rect::from_min_max(
        pos2(
            areas.figure.left() + SPACE_3,
            areas.figure.top() + kit::HEAD + SPACE_2,
        ),
        areas.figure.max - vec2(SPACE_4, SPACE_3),
    );
    (figure, areas.band.shrink2(vec2(SPACE_4, 0.0)))
}

/// A child `Ui` laid out left to right inside `rect`.
fn row(ui: &mut Ui, rect: Rect) -> Ui {
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    row
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let real = &ctx.params().readings;
    let silent = Default::default();
    let readings = if peek_ui::live(ui, "stereo", &real.blocks) {
        real
    } else {
        &silent
    };
    let lefts = readings.lefts.read();
    peek_ui::heartbeat(ui, lefts.iter().any(|sample| sample.abs() > 1e-4));
    let (figure, band) = areas(ui, "Stereo");
    peek_ui::stereo::figure(ui, &PALETTE, figure, readings);

    let ink = PALETTE.inverted();
    let [correlation, balance] = readings.figures.read();
    let mut cells = row(ui, band);
    peek_ui::bipolar_cell(&mut cells, &ink, "Correlation", correlation, 168.0);
    peek_ui::bipolar_cell(&mut cells, &ink, "Balance", balance, 168.0);
}
