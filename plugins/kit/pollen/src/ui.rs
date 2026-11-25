//! The Pollen window: every grain as a mote of pollen — across by its pan, up
//! by how far behind the input it reads — swelling and fading with its
//! window and drifting up a little as it plays, on the Kit frame in the
//! Colour Kit's aqua.

use crate::params::PollenParams;
use egui::{Align2, Rect, Stroke, Ui, pos2};
use kit_ui::{Factory, Kit};
use nxe_egui::kit;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2};
use particle_core::pollen::{BEHIND_MS, GRAINS, READING};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PollenParams>;

const PALETTE: Palette = Palette::COLOUR_KIT;
/// How far behind the figure marks, in ms.
const GUIDES: [f32; 3] = [0.0, BEHIND_MS * 0.5, BEHIND_MS];
/// A grain's largest radius, and how far it drifts up while it plays.
const RADIUS: f32 = 5.0;
const DRIFT: f32 = 14.0;

/// Density, Size, Scatter, Wet. Checked by ear on 2026-11-26.
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.5, 0.4],
    },
    Factory {
        name: "Haze",
        values: &[0.8, 0.7, 0.35, 0.3],
    },
    Factory {
        name: "Dust",
        values: &[0.35, 0.1, 0.6, 0.45],
    },
    Factory {
        name: "Cloud",
        values: &[1.0, 1.0, 1.0, 0.5],
    },
    Factory {
        name: "Fuzz",
        values: &[0.9, 0.25, 0.15, 0.35],
    },
];

pub fn knob_ids(params: &PollenParams) -> [u32; 4] {
    [
        params.density.id(),
        params.size.id(),
        params.scatter.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<PollenParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [density, size, scatter, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Pollen",
            label: "Colour Kit",
            product: "Pollen",
            plugin_id: "com.nxe.pollen",
            palette: &PALETTE,
            knobs: &[
                (density, "Density"),
                (size, "Size"),
                (scatter, "Scatter"),
                (mix, "Wet"),
            ],
            factory: &FACTORY,
        },
    );

    // Bypassed or stopped, the last grains would hang there: show none then.
    let (_, quiet) = kit_ui::arrivals(ui, "pollen-frames", params.count.load(Ordering::Relaxed));
    let grains = if quiet < kit_ui::STALE_SECONDS {
        params.grains.read()
    } else {
        [0.0; GRAINS * READING]
    };
    motes(ui, figure, &grains);
}

/// Where `0..=1` of the way behind sits in `figure`: now at the bottom.
fn y_of(figure: Rect, behind: f32) -> f32 {
    figure.bottom() - figure.height() * (0.1 + 0.75 * behind)
}

fn motes(ui: &Ui, figure: Rect, grains: &[f32; GRAINS * READING]) {
    let painter = ui.painter_at(figure);
    for ms in GUIDES {
        let y = y_of(figure, ms / BEHIND_MS);
        painter.line_segment(
            [pos2(figure.left(), y), pos2(figure.right(), y)],
            Stroke::new(1.0, PALETTE.line.at(0.5).color()),
        );
        painter.text(
            pos2(figure.left() + SPACE_2, y - 2.0),
            Align2::LEFT_BOTTOM,
            format!("{ms:.0} ms"),
            nxe_egui::font::mono(FONT_VALUE * 0.8),
            PALETTE.subtle.color(),
        );
    }
    for reading in grains.chunks_exact(READING) {
        let [pan, behind, level, progress] = [reading[0], reading[1], reading[2], reading[3]];
        if level <= 0.0 {
            continue;
        }
        let centre = pos2(
            figure.center().x + pan * figure.width() * 0.42,
            y_of(figure, behind) - DRIFT * progress,
        );
        painter.circle_filled(
            centre,
            RADIUS * level + 1.0,
            PALETTE.accent.at(0.25 * level).color(),
        );
        painter.circle_filled(centre, 1.0 + level * 1.5, PALETTE.bright.at(level).color());
    }
}
