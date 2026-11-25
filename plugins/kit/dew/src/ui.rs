//! The Dew window: every drop as a ripple where it rings — across by its pan,
//! up by its pitch — widening as it ages and fading as it decays, on the Kit
//! frame in the Colour Kit's aqua.

use crate::params::{DewParams, Tuning};
use egui::{Align, Align2, Layout, Rect, Stroke, Ui, UiBuilder, pos2};
use kit_ui::{Factory, Kit};
use nxe_egui::kit;
use nxe_egui::segment::HEIGHT;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2, SPACE_4};
use resonance_core::dew::{DROPS, FIGURE_HZ, READING};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<DewParams>;

const PALETTE: Palette = Palette::COLOUR_KIT;
/// How fast a ripple widens, in points a second.
const RIPPLE_SPEED: f32 = 260.0;
/// The pitches the figure marks.
const GUIDES: [(f32, &str); 3] = [(250.0, "250"), (1_000.0, "1k"), (4_000.0, "4k")];

/// Sensitivity, Size, Splash, Wet. A first draft; settling them needs an ear
/// (`resonance-kit-plan.md`).
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.4, 0.4, 0.5],
    },
    Factory {
        name: "Drip",
        values: &[0.4, 0.75, 0.0, 0.6],
    },
    Factory {
        name: "Rain",
        values: &[0.7, 0.3, 1.0, 0.45],
    },
    Factory {
        name: "Mist",
        values: &[0.8, 0.1, 0.85, 0.3],
    },
    Factory {
        name: "Puddle",
        values: &[0.3, 1.0, 0.5, 0.7],
    },
];

pub fn knob_ids(params: &DewParams) -> [u32; 4] {
    [
        params.sensitivity.id(),
        params.size.id(),
        params.splash.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<DewParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [sensitivity, size, splash, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Dew",
            label: "Colour Kit",
            product: "Dew",
            plugin_id: "com.nxe.dew",
            palette: &PALETTE,
            knobs: &[
                (sensitivity, "Sensitivity"),
                (size, "Size"),
                (splash, "Splash"),
                (mix, "Wet"),
            ],
            factory: &FACTORY,
        },
    );

    // Bypassed or stopped, the last ripples would stay up: show none then.
    let (_, quiet) = kit_ui::arrivals(ui, "dew-frames", params.count.load(Ordering::Relaxed));
    let drops = if quiet < kit_ui::STALE_SECONDS {
        params.drops.read()
    } else {
        [0.0; DROPS * READING]
    };
    ripples(ui, figure, &drops);

    // Free or Follow, and the pitch either one aims at, in one row from the
    // right.
    let row = Rect::from_min_max(
        pos2(figure.left() + SPACE_2, figure.bottom() - HEIGHT - SPACE_2),
        pos2(figure.right() - SPACE_2, figure.bottom() - SPACE_2),
    );
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(row)
            .layout(Layout::right_to_left(Align::Center)),
    );
    row.spacing_mut().item_spacing.x = SPACE_4;
    kit_ui::number_for(&mut row, ctx, &PALETTE, params.pitch.id(), "Pitch");
    let tuning = params.tuning.index() as usize;
    if nxe_egui::toggle::toggle(
        &mut row,
        &PALETTE,
        "Tuning",
        Tuning::NAMES[tuning],
        tuning == Tuning::Follow as usize,
    ) {
        kit_ui::set_plain(ctx, params.tuning.id(), (1 - tuning) as f64);
    }
}

/// Where a height `0..=1` up the pitch span sits in `figure`.
fn y_of(figure: Rect, height: f32) -> f32 {
    figure.bottom() - figure.height() * (0.08 + 0.84 * height)
}

fn ripples(ui: &Ui, figure: Rect, drops: &[f32; DROPS * READING]) {
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
    for reading in drops.chunks_exact(READING) {
        let [pan, height, level, age] = [reading[0], reading[1], reading[2], reading[3]];
        if level <= 0.0 {
            continue;
        }
        let centre = pos2(
            figure.center().x + pan * figure.width() * 0.38,
            y_of(figure, height),
        );
        let alpha = (level * 4.0).min(1.0);
        let radius = 3.0 + age * RIPPLE_SPEED;
        painter.circle_stroke(
            centre,
            radius,
            Stroke::new(1.5, PALETTE.accent.at(alpha).color()),
        );
        painter.circle_stroke(
            centre,
            radius * 0.55,
            Stroke::new(1.0, PALETTE.accent.at(alpha * 0.5).color()),
        );
        painter.circle_filled(centre, 2.5, PALETTE.bright.at(alpha).color());
    }
}
