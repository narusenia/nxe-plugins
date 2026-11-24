//! The Peek VU window: the needle on its arc, or two bars; the look is a parameter.

use crate::params::PeekVuParams;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use peek_ui::bind;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PeekVuParams>;

const PALETTE: Palette = Palette::PEEK;

pub fn create(params: Arc<PeekVuParams>) -> Box<dyn Editor> {
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
use nxe_egui::readout::{figure as text_of, readout};

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let live = peek_ui::live(ui, "vu", &params.readings.blocks);
    let amplitudes = if live {
        params.readings.vu.read()
    } else {
        [0.0; 2]
    };
    peek_ui::heartbeat(ui, amplitudes.iter().any(|amplitude| *amplitude > 1e-4));
    let (figure, band) = areas(ui, "VU");
    let reference = params.reference.value();
    let vu = amplitudes.map(|amplitude| peek_ui::vu::vu_of(amplitude, reference));

    let look = params.look.index() as usize;
    let mut corner = row(
        ui,
        Rect::from_min_size(
            pos2(figure.right() - 96.0, figure.top() - kit::HEAD + SPACE_2),
            vec2(96.0, nxe_egui::segment::HEIGHT),
        ),
    );
    peek_ui::choice(
        &mut corner,
        ctx,
        &PALETTE,
        params.look.id(),
        &["Arc", "Line"],
        look,
        96.0,
    );
    if look == 0 {
        peek_ui::vu::arc(ui, &PALETTE, figure, vu);
    } else {
        peek_ui::vu::line(ui, &PALETTE, figure, vu);
    }

    let ink = PALETTE.inverted();
    let mut cells = row(ui, band);
    for (name, (value, amplitude)) in ["L", "R"].into_iter().zip(vu.into_iter().zip(amplitudes)) {
        readout(
            &mut cells,
            &ink,
            name,
            &text_of((amplitude > 1e-6).then_some(value), 1),
            "VU",
            64.0,
        );
    }
    peek_ui::bipolar_cell(
        &mut cells,
        &ink,
        "Correlation",
        if live {
            params.readings.correlation.read()[0]
        } else {
            0.0
        },
        136.0,
    );
    bind::knob_for(&mut cells, ctx, &ink, params.reference.id(), "0 VU", 28.0);
}
