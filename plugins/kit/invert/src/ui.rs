//! The Invert window: where each frequency comes out, against where it went
//! in, with the band that turns over shaded.

use crate::params::InvertParams;
use egui::Ui;
use kit_ui::{Factory, Frequency, Kit};
use nxe_egui::kit;
use nxe_egui::theme::Palette;
use spectral_core::invert::mirror;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<InvertParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Low, High, Mix. Checked by ear on 2026-11-26.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[200.0, 5000.0, 1.0],
    },
    Factory {
        name: "Upside Down",
        values: &[20.0, 20000.0, 1.0],
    },
    Factory {
        name: "Mids Turned",
        values: &[500.0, 3000.0, 0.7],
    },
    Factory {
        name: "Top Flip",
        values: &[4000.0, 16000.0, 0.8],
    },
];

pub fn knob_ids(params: &InvertParams) -> [u32; 3] {
    [params.low.id(), params.high.id(), params.mix.id()]
}

pub fn create(params: Arc<InvertParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let [low, high, mix] = knob_ids(ctx.params());
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Invert",
            label: "Design Kit",
            product: "Invert",
            plugin_id: "com.nxe.invert",
            palette: &PALETTE,
            knobs: &[(low, "Low"), (high, "High"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let (l, h) = (
        kit_ui::plain(ctx, low) as f32,
        kit_ui::plain(ctx, high) as f32,
    );
    let plot = Frequency::hz(ui, &PALETTE, figure);
    plot.band(ui, &PALETTE, l, h);
    plot.curve(ui, PALETTE.accent, 2.0, |hz| mirror(hz, l, h));
}
