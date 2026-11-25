//! The Glint window: every spark as a point of light where it rings — across
//! by its pan, up by its pitch — its rays as long as it is loud, shrinking as
//! it fades, on the Kit frame in the Colour Kit's aqua.

use crate::params::GlintParams;
use egui::{Align2, Rect, Stroke, Ui, pos2, vec2};
use kit_ui::{Factory, Kit};
use nxe_egui::kit;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2};
use particle_core::glint::{FIGURE_HZ, READING, SPARKS};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<GlintParams>;

const PALETTE: Palette = Palette::COLOUR_KIT;
/// The pitches the figure marks.
const GUIDES: [(f32, &str); 3] = [(2_500.0, "2.5k"), (5_000.0, "5k"), (10_000.0, "10k")];
/// A spark's longest ray, in points.
const RAY: f32 = 22.0;

/// Sensitivity, Height, Decay, Wet. Checked by ear on 2026-11-26.
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.4, 0.4],
    },
    Factory {
        name: "Frost",
        values: &[0.6, 0.85, 0.2, 0.3],
    },
    Factory {
        name: "Chime",
        values: &[0.4, 0.4, 0.8, 0.45],
    },
    Factory {
        name: "Shard",
        values: &[0.3, 0.65, 0.05, 0.55],
    },
    Factory {
        name: "Spray",
        values: &[0.85, 0.7, 0.3, 0.35],
    },
];

pub fn knob_ids(params: &GlintParams) -> [u32; 4] {
    [
        params.sensitivity.id(),
        params.height.id(),
        params.decay.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<GlintParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [sensitivity, height, decay, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Glint",
            label: "Colour Kit",
            product: "Glint",
            plugin_id: "com.nxe.glint",
            palette: &PALETTE,
            knobs: &[
                (sensitivity, "Sensitivity"),
                (height, "Height"),
                (decay, "Decay"),
                (mix, "Wet"),
            ],
            factory: &FACTORY,
        },
    );

    // Bypassed or stopped, the last sparks would stay lit: show none then.
    let (_, quiet) = kit_ui::arrivals(ui, "glint-frames", params.count.load(Ordering::Relaxed));
    let sparks = if quiet < kit_ui::STALE_SECONDS {
        params.sparks.read()
    } else {
        [0.0; SPARKS * READING]
    };
    sparkles(ui, figure, &sparks);
}

/// Where a height `0..=1` up the pitch span sits in `figure`.
fn y_of(figure: Rect, height: f32) -> f32 {
    figure.bottom() - figure.height() * (0.08 + 0.84 * height)
}

fn sparkles(ui: &Ui, figure: Rect, sparks: &[f32; SPARKS * READING]) {
    let painter = ui.painter_at(figure);
    let (low, high) = (FIGURE_HZ.0.ln(), FIGURE_HZ.1.ln());
    for (hz, label) in GUIDES {
        let y = y_of(figure, (hz.ln() - low) / (high - low));
        painter.line_segment(
            [pos2(figure.left(), y), pos2(figure.right(), y)],
            Stroke::new(1.0, PALETTE.line.at(0.5).color()),
        );
        painter.text(
            pos2(figure.left() + SPACE_2, y - 2.0),
            Align2::LEFT_BOTTOM,
            label,
            nxe_egui::font::mono(FONT_VALUE * 0.8),
            PALETTE.subtle.color(),
        );
    }
    for reading in sparks.chunks_exact(READING) {
        let [pan, height, level, age] = [reading[0], reading[1], reading[2], reading[3]];
        if level <= 0.0 {
            continue;
        }
        let centre = pos2(
            figure.center().x + pan * figure.width() * 0.4,
            y_of(figure, height),
        );
        let glow = (level * 8.0).min(1.0);
        // A spark flashes: its rays are longest in the first few
        // milliseconds and settle to its level.
        let flash = 1.0 + 0.6 * (-age * 40.0).exp();
        let ray = RAY * glow * flash;
        let colour = PALETTE.accent.at(glow).color();
        for (direction, length) in [
            (vec2(1.0, 0.0), 1.0),
            (vec2(0.0, 1.0), 1.0),
            (vec2(0.7, 0.7), 0.4),
            (vec2(0.7, -0.7), 0.4),
        ] {
            let reach = direction * ray * length;
            painter.line_segment([centre - reach, centre + reach], Stroke::new(1.0, colour));
        }
        painter.circle_filled(centre, 1.5 + glow, PALETTE.bright.at(glow).color());
    }
}
