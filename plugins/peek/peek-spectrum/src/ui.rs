//! The Peek Spectrum window: the curve and its falling peak line, tilted.

use crate::params::PeekSpectrumParams;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekSpectrumParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekSpectrumParams>) -> Box<dyn Editor> {
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
    let params = ctx.params();
    let levels = params.readings.levels.read();
    peek_ui::heartbeat(ui, true);
    let (figure, band) = areas(ui, "Spectrum");
    let tilt = params.tilt.index() as usize;
    let plot = Rect::from_min_max(figure.min, figure.max - vec2(0.0, 14.0));
    peek_ui::spectrum::figure(
        ui,
        &PALETTE,
        plot,
        &levels,
        crate::params::Tilt::SLOPES[tilt],
    );

    let ink = PALETTE.inverted();
    let mut cells = row(ui, band);
    peek_ui::choice(
        &mut cells,
        ctx,
        &ink,
        params.tilt.id(),
        &["Flat", "3 dB", "4.5 dB"],
        tilt,
        220.0,
    );
}
