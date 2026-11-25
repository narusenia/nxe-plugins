//! The Fever window: the curve at the heat it has now, over the curve it
//! reaches when fully hot, with the heat as a number.

use crate::params::FeverParams;
use egui::{Align2, Ui, pos2};
use kit_ui::{Factory, Kit, Transfer};
use nonlinear_core::fever::{drive_and_bias, shape};
use nxe_egui::kit;
use nxe_egui::theme::{FONT_VALUE, Palette};
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<FeverParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Fever, Recovery, Mix. Checked by ear on 2026-11-26.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[0.5, 2.0, 1.0],
    },
    Factory {
        name: "Warm Up",
        values: &[0.3, 6.0, 0.7],
    },
    Factory {
        name: "Flush",
        values: &[0.8, 1.0, 1.0],
    },
    Factory {
        name: "Delirium",
        values: &[1.0, 10.0, 1.0],
    },
];

pub fn knob_ids(params: &FeverParams) -> [u32; 3] {
    [params.fever.id(), params.recovery.id(), params.mix.id()]
}

pub fn create(params: Arc<FeverParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [fever, recovery, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Fever",
            label: "Design Kit",
            product: "Fever",
            plugin_id: "com.nxe.fever",
            palette: &PALETTE,
            knobs: &[(fever, "Fever"), (recovery, "Recovery"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let m = kit_ui::plain(ctx, mix) as f32;
    let heat = ctx.get_meter(params.heat.id());
    let transfer = Transfer::new(ui, &PALETTE, figure, ctx.get_meter(params.input.id()), 1.3);
    for (at, colour, width) in [(1.0, PALETTE.deep, 1.0), (heat, PALETTE.accent, 2.0)] {
        let (drive, bias) = drive_and_bias(at, 0.0);
        transfer.curve(ui, -1.3, 1.3, colour, width, |x| {
            x + m * (shape(x, drive, bias) - x)
        });
    }
    ui.painter().text(
        pos2(figure.left() + 4.0, figure.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        format!("heat {:.0} %", heat * 100.0),
        nxe_egui::font::mono(FONT_VALUE),
        PALETTE.muted.color(),
    );
}
