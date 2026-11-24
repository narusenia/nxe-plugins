//! The Peek Loudness window: the integrated figure, the history against the target, and the moving readings on the band. A click on the band clears the integrated reading.

use crate::params::PeekLoudnessParams;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekLoudnessParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekLoudnessParams>) -> Box<dyn Editor> {
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

use egui::{Align2, Sense};
use nxe_egui::font;
use nxe_egui::theme::FONT_EYEBROW;
use peek_ui::Reading;
use std::sync::atomic::Ordering;

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let values = params.readings.values.read();
    peek_ui::heartbeat(ui, true);
    let (figure, band) = areas(ui, "Loudness");
    let target = params.target.index() as usize;
    let mut corner = row(
        ui,
        Rect::from_min_size(
            pos2(figure.right() - 160.0, figure.top() - kit::HEAD + SPACE_2),
            vec2(160.0, nxe_egui::segment::HEIGHT),
        ),
    );
    peek_ui::choice(
        &mut corner,
        ctx,
        &PALETTE,
        params.target.id(),
        &crate::params::Target::NAMES,
        target,
        160.0,
    );
    peek_ui::loudness::figure(
        ui,
        &PALETTE,
        figure,
        values,
        crate::params::Target::LUFS[target],
        true,
    );

    let clear = ui.interact(
        band.expand2(vec2(SPACE_4, 0.0)),
        ui.id().with("clear"),
        Sense::click(),
    );
    if clear.clicked() {
        params.readings.clears.fetch_add(1, Ordering::Relaxed);
    }
    let finite = |value: f32| value.is_finite().then_some(value);
    peek_ui::readings(
        ui,
        &PALETTE,
        band,
        &[
            Reading {
                name: "Momentary",
                value: finite(values[0]),
                unit: "LUFS",
                alarm: false,
            },
            Reading {
                name: "Short-term",
                value: finite(values[1]),
                unit: "LUFS",
                alarm: false,
            },
            Reading {
                name: "Range",
                value: Some(values[3]),
                unit: "LU",
                alarm: false,
            },
        ],
    );
    let ink = PALETTE.inverted();
    font::tracked(
        ui.painter(),
        pos2(band.right(), band.center().y),
        Align2::RIGHT_CENTER,
        "Clear",
        FONT_EYEBROW,
        if clear.hovered() { ink.ink } else { ink.subtle }.color(),
    );
}
