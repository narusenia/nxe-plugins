//! The Buckle window: the two sides of the loop. Straight through up to the
//! threshold, turned over from half of it, and the side the signal is on now
//! drawn strong.

use crate::params::BuckleParams;
use egui::Ui;
use kit_ui::{Factory, Kit, Transfer};
use nonlinear_core::buckle::{RELEASE, shape};
use nxe_egui::kit;
use nxe_egui::theme::Palette;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<BuckleParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Threshold, Snap, Mix. Checked by ear on 2026-11-26.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[-12.0, 0.75, 1.0],
    },
    Factory {
        name: "Crease",
        values: &[-6.0, 0.4, 0.8],
    },
    Factory {
        name: "Collapse",
        values: &[-18.0, 1.0, 1.0],
    },
    Factory {
        name: "Hairline",
        values: &[-30.0, 0.5, 0.5],
    },
];

pub fn knob_ids(params: &BuckleParams) -> [u32; 3] {
    [params.threshold.id(), params.snap.id(), params.mix.id()]
}

pub fn create(params: Arc<BuckleParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [threshold, snap, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Buckle",
            label: "Design Kit",
            product: "Buckle",
            plugin_id: "com.nxe.buckle",
            palette: &PALETTE,
            knobs: &[(threshold, "Threshold"), (snap, "Snap"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let t = 10f32.powf(kit_ui::plain(ctx, threshold) as f32 / 20.0);
    let (s, m) = (
        kit_ui::plain(ctx, snap) as f32,
        kit_ui::plain(ctx, mix) as f32,
    );
    let buckled = ctx.get_meter(params.buckled.id());
    let transfer = Transfer::new(ui, &PALETTE, figure, ctx.get_meter(params.input.id()), 1.0);
    let side = |on: bool| {
        if on {
            (PALETTE.accent, 2.0)
        } else {
            (PALETTE.deep, 1.0)
        }
    };
    let curve = |x: f32, b: f32| x + m * (shape(x, s, b) - x);
    let (colour, width) = side(buckled < 0.5);
    transfer.curve(ui, -t, t, colour, width, |x| curve(x, 0.0));
    let (colour, width) = side(buckled >= 0.5);
    for (from, to) in [(-1.0, -t * RELEASE), (t * RELEASE, 1.0)] {
        transfer.curve(ui, from, to, colour, width, |x| curve(x, 1.0));
    }
}
