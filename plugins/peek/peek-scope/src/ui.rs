//! The Peek Scope window: the waveform, min to max per point.

use crate::params::PeekScopeParams;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use peek_ui::bind;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekScopeParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekScopeParams>) -> Box<dyn Editor> {
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
    let maxs = params.readings.maxs.read();
    peek_ui::heartbeat(ui, maxs.iter().any(|sample| sample.abs() > 1e-4));
    let (figure, band) = areas(ui, "Scope");
    peek_ui::scope::figure(ui, &PALETTE, figure, &params.readings);

    let ink = PALETTE.inverted();
    let mut cells = row(ui, band);
    bind::knob_for(&mut cells, ctx, &ink, params.window.id(), "Window", 28.0);
}
