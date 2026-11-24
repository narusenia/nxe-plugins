//! The Fold window: the curve the signal is folded through, on the Kit frame.

use crate::params::FoldParams;
use egui::Ui;
use kit_ui::{Factory, Kit, Transfer};
use nonlinear_core::fold::shape;
use nxe_egui::kit;
use nxe_egui::theme::Palette;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<FoldParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Fold, Symmetry, Mix. The values are a first draft; settling them needs an
/// ear (`nonlinear-kit-plan.md`).
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[2.5, 0.0, 1.0],
    },
    Factory {
        name: "Gentle",
        values: &[1.5, 0.0, 0.6],
    },
    Factory {
        name: "Edge",
        values: &[3.5, 0.3, 1.0],
    },
    Factory {
        name: "Hollow",
        values: &[5.0, 0.0, 1.0],
    },
    Factory {
        name: "Tangle",
        values: &[8.0, 0.6, 1.0],
    },
];

pub fn knob_ids(params: &FoldParams) -> [u32; 3] {
    [params.fold.id(), params.symmetry.id(), params.mix.id()]
}

pub fn create(params: Arc<FoldParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [fold, symmetry, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Fold",
            label: "Design Kit",
            product: "Fold",
            plugin_id: "com.nxe.fold",
            palette: &PALETTE,
            knobs: &[(fold, "Fold"), (symmetry, "Symmetry"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let (d, s, m) = (
        kit_ui::plain(ctx, fold) as f32,
        kit_ui::plain(ctx, symmetry) as f32,
        kit_ui::plain(ctx, mix) as f32,
    );
    let transfer = Transfer::new(ui, &PALETTE, figure, ctx.get_meter(params.input.id()), 1.3);
    transfer.curve(ui, -1.3, 1.3, PALETTE.accent, 2.0, |x| {
        x + m * (shape(x, d, s) - x)
    });
}
