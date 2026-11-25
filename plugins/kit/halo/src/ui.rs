//! The Halo window: the chord as concentric rings around its root — the root
//! innermost, two octaves up outermost — each glowing as loud as it rings, on
//! the Kit frame in the Colour Kit's aqua.

use crate::params::{Chord, HaloParams, SHOWN, Tuning};
use egui::{Align, Align2, Layout, Rect, Stroke, Ui, UiBuilder, pos2};
use kit_ui::{Factory, Kit};
use nxe_egui::kit;
use nxe_egui::segment::HEIGHT;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2, SPACE_4};
use resonance_core::halo::{READING, SPAN_OCTAVES, VOICES};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<HaloParams>;

const PALETTE: Palette = Palette::COLOUR_KIT;
/// The innermost and outermost ring, as fractions of the figure's half height.
const RADIUS: (f32, f32) = (0.2, 0.68);

/// Sensitivity, Decay, Shimmer, Tone, Wet. A first draft; settling them needs an ear
/// (`resonance-kit-plan.md`).
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.3, 0.5, 0.4],
    },
    Factory {
        name: "Glow",
        values: &[0.4, 0.35, 0.15, 0.35, 0.3],
    },
    Factory {
        name: "Choir",
        values: &[0.6, 0.7, 0.5, 0.6, 0.5],
    },
    Factory {
        name: "Bloom",
        values: &[0.7, 0.9, 0.7, 0.75, 0.6],
    },
    Factory {
        name: "Ember",
        values: &[0.5, 0.6, 0.1, 0.1, 0.45],
    },
];

pub fn knob_ids(params: &HaloParams) -> [u32; 5] {
    [
        params.sensitivity.id(),
        params.decay.id(),
        params.shimmer.id(),
        params.tone.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<HaloParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let params = ctx.params();
    let [sensitivity, decay, shimmer, tone, mix] = knob_ids(params);
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Halo",
            label: "Colour Kit",
            product: "Halo",
            plugin_id: "com.nxe.halo",
            palette: &PALETTE,
            knobs: &[
                (sensitivity, "Sensitivity"),
                (decay, "Decay"),
                (shimmer, "Shimmer"),
                (tone, "Tone"),
                (mix, "Wet"),
            ],
            factory: &FACTORY,
        },
    );

    // Bypassed or stopped, the rings would stay lit: show them dark then.
    let (_, quiet) = kit_ui::arrivals(ui, "halo-frames", params.count.load(Ordering::Relaxed));
    let live = quiet < kit_ui::STALE_SECONDS;
    let shown = if live {
        params.shown.read()
    } else {
        [0.0; SHOWN]
    };
    let follow = params.tuning.index() as usize == Tuning::Follow as usize;
    let root = if live && shown[VOICES * READING] > 0.0 {
        shown[VOICES * READING]
    } else if follow {
        0.0
    } else {
        kit_ui::plain(ctx, params.root.id()) as f32
    };
    let ratios = crate::chord(params.chord.index() as usize).ratios();
    rings(
        ui,
        figure,
        ratios,
        &shown,
        root,
        ui.input(|input| input.time) as f32,
    );

    // The chord, Free or Follow, and the root Free uses, in one row from the
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
    kit_ui::number_for(&mut row, ctx, &PALETTE, params.root.id(), "Root");
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
    if let Some(picked) = nxe_egui::select::select(
        &mut row,
        &PALETTE,
        "Chord",
        &Chord::NAMES,
        params.chord.index() as usize,
    ) {
        kit_ui::set_plain(ctx, params.chord.id(), picked as f64);
    }
}

/// The rings, centred a little above the figure's middle so the row of
/// controls under them stays clear, and the root's name at the centre. The
/// rings come from the chord, so they show before anything rings; the glow
/// comes from the audio.
fn rings(ui: &Ui, figure: Rect, ratios: &[f32], shown: &[f32; SHOWN], root: f32, time: f32) {
    let painter = ui.painter_at(figure);
    let half = figure.height() * 0.5 - nxe_egui::segment::HEIGHT * 0.5;
    let centre = pos2(figure.center().x, figure.top() + half + SPACE_2);
    for (index, ratio) in ratios.iter().enumerate() {
        let height = (ratio.log2() / SPAN_OCTAVES).clamp(0.0, 1.0);
        let level = shown[index * READING + 1];
        let radius = half * (RADIUS.0 + (RADIUS.1 - RADIUS.0) * height);
        // A ring breathes a little with time, so a held chord still reads as alive.
        let radius = radius + (time * (0.7 + 0.13 * index as f32)).sin() * level.min(1.0) * 2.0;
        let glow = (level * 6.0).min(1.0);
        painter.circle_stroke(
            centre,
            radius,
            Stroke::new(1.0, PALETTE.line.at(0.6).color()),
        );
        if glow > 0.0 {
            painter.circle_stroke(
                centre,
                radius,
                Stroke::new(6.0, PALETTE.accent.at(0.18 * glow).color()),
            );
            painter.circle_stroke(
                centre,
                radius,
                Stroke::new(1.5, PALETTE.accent.at(glow).color()),
            );
        }
    }
    let name = if root > 0.0 {
        nxe_egui::text::note(root)
    } else {
        "—".to_owned()
    };
    painter.text(
        centre,
        Align2::CENTER_CENTER,
        name,
        nxe_egui::font::mono(FONT_VALUE * 1.3),
        PALETTE.muted.color(),
    );
}
