//! The Shear window: how late each frequency comes out.

use crate::params::ShearParams;
use egui::Ui;
use kit_ui::{Factory, Frequency, Kit};
use nxe_egui::kit;
use nxe_egui::theme::Palette;
use spectral_core::shear::delay;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<ShearParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Shear, Length, Mix. A first draft; settling them needs an ear.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[0.5, 250.0, 1.0],
    },
    Factory {
        name: "Chirp",
        values: &[1.0, 60.0, 1.0],
    },
    Factory {
        name: "Low Lag",
        values: &[-0.8, 400.0, 0.8],
    },
    Factory {
        name: "Smear",
        values: &[1.0, 1000.0, 0.6],
    },
];

pub fn knob_ids(params: &ShearParams) -> [u32; 3] {
    [params.shear.id(), params.length.id(), params.mix.id()]
}

pub fn create(params: Arc<ShearParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let [shear, length, mix] = knob_ids(ctx.params());
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Shear",
            label: "Design Kit",
            product: "Shear",
            plugin_id: "com.nxe.shear",
            palette: &PALETTE,
            knobs: &[(shear, "Shear"), (length, "Length"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let s = kit_ui::plain(ctx, shear) as f32;
    let seconds = kit_ui::plain(ctx, length) as f32 / 1_000.0;
    let plot = Frequency::amount(
        ui,
        &PALETTE,
        figure,
        seconds,
        &format!("{:.0} ms", seconds * 1_000.0),
    );
    plot.curve(ui, PALETTE.accent, 2.0, |hz| delay(hz, s, seconds));
}
