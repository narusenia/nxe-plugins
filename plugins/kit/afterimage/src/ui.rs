//! The Afterimage window: how long a trace lasts at each frequency.

use crate::params::AfterimageParams;
use egui::Ui;
use kit_ui::{Factory, Frequency, Kit};
use nxe_egui::kit;
use nxe_egui::theme::Palette;
use spectral_core::afterimage::persistence_at;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<AfterimageParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Persistence, Damping, Mix. A first draft; settling them needs an ear.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[3.0, 0.5, 0.5],
    },
    Factory {
        name: "Glow",
        values: &[1.0, 0.7, 0.4],
    },
    Factory {
        name: "Haunt",
        values: &[8.0, 0.3, 0.6],
    },
    Factory {
        name: "Frozen",
        values: &[20.0, 0.0, 0.8],
    },
];

pub fn knob_ids(params: &AfterimageParams) -> [u32; 3] {
    [
        params.persistence.id(),
        params.damping.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<AfterimageParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let [persistence, damping, mix] = knob_ids(ctx.params());
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Afterimage",
            label: "Design Kit",
            product: "Afterimage",
            plugin_id: "com.nxe.afterimage",
            palette: &PALETTE,
            knobs: &[
                (persistence, "Persistence"),
                (damping, "Damping"),
                (mix, "Mix"),
            ],
            factory: &FACTORY,
        },
    );
    let p = kit_ui::plain(ctx, persistence) as f32;
    let d = kit_ui::plain(ctx, damping) as f32;
    let plot = Frequency::amount(ui, &PALETTE, figure, p, &format!("{p:.1} s"));
    plot.curve(ui, PALETTE.accent, 2.0, |hz| persistence_at(hz, p, d));
}
