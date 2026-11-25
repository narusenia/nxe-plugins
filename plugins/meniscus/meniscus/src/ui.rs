//! The Meniscus window, on the Colour frame (`nxe_egui::colour`). The figure
//! is the surface seen edge on: its last second and a half spreading out from
//! the middle both ways, so a strike falls at the centre and its wobble runs
//! outwards as a ripple. The drops ring above it. The surface wears the colour
//! of the note it hums at; nothing else on the window has a hue.

use crate::params::{MeniscusParams, SHOWN, Tuning};
use egui::{Align, Align2, Layout, Pos2, Rect, Shape, Stroke, Ui, UiBuilder, pos2};
use kit_ui::{Factory, Kit};
use nxe_egui::colour::{self, colour};
use nxe_egui::font;
use nxe_egui::theme::{FONT_EYEBROW, FONT_LABEL, Palette, SPACE_2, SPACE_3, SPACE_4};
use resonance_core::dew::{self, DROPS};
use resonance_core::meniscus::HISTORY;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<MeniscusParams>;

const PALETTE: Palette = Palette::COLOUR;
/// The note's name in the strip.
const FONT_NOTE: f32 = 28.0;
/// The pitch classes' grid in the strip.
const GRID: f32 = 264.0;
/// How fast a drop's ring widens, in points a second.
const RIPPLE_SPEED: f32 = 160.0;

/// The knobs in the window's order; the presets store them in it.
pub fn ids(params: &MeniscusParams) -> [u32; 8] {
    [
        params.sensitivity.id(),
        params.tension.id(),
        params.viscosity.id(),
        params.splash.id(),
        params.resonance.id(),
        params.drops.id(),
        params.mix.id(),
        params.output.id(),
    ]
}

const NAMES: [&str; 8] = [
    "Sensitivity",
    "Tension",
    "Viscosity",
    "Splash",
    "Resonance",
    "Drops",
    "Wet",
    "Output",
];

/// Sensitivity, Tension, Viscosity, Splash, Resonance, Drops, Wet, Output. A
/// first draft; settling them needs an ear.
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.35, 0.4, 0.7, 0.0, 0.5, 0.0],
    },
    Factory {
        name: "Jelly",
        values: &[0.5, 0.25, 0.2, 0.6, 0.85, 0.0, 0.6, -2.0],
    },
    Factory {
        name: "Bubble",
        values: &[0.6, 0.7, 0.4, 0.5, 0.6, 0.7, 0.5, 0.0],
    },
    Factory {
        name: "Honey",
        values: &[0.4, 0.4, 0.9, 0.7, 0.8, 0.2, 0.5, 0.0],
    },
    Factory {
        name: "Mercury",
        values: &[0.6, 0.9, 0.1, 0.3, 0.9, 0.0, 0.4, -3.0],
    },
];

pub fn create(params: Arc<MeniscusParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (colour::WIDTH as u32, colour::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    // The figure moves without input or a parameter change (`kit_ui::window`).
    ui.ctx().request_repaint_after(nxe_egui::smooth::FRAME);
    let params = ctx.params();
    let ids = ids(params);
    let areas = colour(ui.painter(), ui.max_rect(), &PALETTE, "Meniscus", "Colour");

    for (index, (id, name)) in ids.iter().zip(NAMES).enumerate() {
        let mut cell = ui.new_child(
            UiBuilder::new()
                .max_rect(colour::cell(areas.controls, index))
                .layout(Layout::top_down(Align::Center)),
        );
        kit_ui::knob_for(&mut cell, ctx, &PALETTE, *id, name, colour::KNOB);
    }
    let knobs: Vec<(u32, &str)> = ids.iter().copied().zip(NAMES).collect();
    kit_ui::presets(
        &mut ui.new_child(UiBuilder::new().max_rect(areas.presets)),
        ctx,
        &Kit {
            name: "Meniscus",
            label: "Colour",
            product: "Meniscus",
            plugin_id: "com.nxe.meniscus",
            palette: &PALETTE,
            knobs: &knobs,
            factory: &FACTORY,
        },
    );

    // Bypassed or stopped, the last picture would stay up: show a still
    // surface then.
    let (_, quiet) = kit_ui::arrivals(ui, "meniscus-frames", params.count.load(Ordering::Relaxed));
    let shown = if quiet < kit_ui::STALE_SECONDS {
        params.shown.read()
    } else {
        [0.0; SHOWN]
    };
    let hz = if shown[SHOWN - 1] > 0.0 {
        shown[SHOWN - 1]
    } else {
        params.pitch.read()
    };
    let note = colour::note(colour::nearest(hz));

    strip(ui, ctx, areas.strip, hz);
    let (surface, drops) = shown[..SHOWN - 1].split_at(HISTORY);
    figure(ui, areas.figure, surface, drops, note);
}

/// The note the surface hums at, and the controls that choose it.
fn strip(ui: &mut Ui, ctx: &Ctx, rect: Rect, hz: f32) {
    let painter = ui.painter();
    let y = rect.center().y;
    let name = painter.text(
        pos2(rect.left(), y),
        Align2::LEFT_CENTER,
        nxe_egui::text::note(hz),
        font::medium(FONT_NOTE),
        PALETTE.ink.color(),
    );
    let figure = painter.text(
        pos2(name.right() + SPACE_3, y + 3.0),
        Align2::LEFT_CENTER,
        format!("{hz:.0} Hz"),
        font::mono(FONT_LABEL),
        PALETTE.muted.color(),
    );
    // The one pitch class the surface hums at, on the same grid as Prisma's.
    let mut octaves = [0u8; 12];
    octaves[colour::nearest(hz).rem_euclid(12) as usize] = 1;
    let left = figure.right() + SPACE_4 + SPACE_2;
    colour::classes(
        painter,
        &PALETTE,
        Rect::from_min_max(pos2(left, rect.top()), pos2(left + GRID, rect.bottom())),
        &octaves,
    );

    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
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

fn figure(ui: &Ui, rect: Rect, surface: &[f32], drops: &[f32], note: nxe_egui::theme::Token) {
    let painter = ui.painter_at(rect.expand(SPACE_4));
    font::tracked(
        &painter,
        rect.left_top(),
        Align2::LEFT_TOP,
        "Surface, last 1.5 s",
        FONT_EYEBROW,
        PALETTE.subtle.color(),
    );

    let rest = rect.top() + rect.height() * 0.55;
    let depth = rect.height() * 0.36;
    let hairline = Stroke::new(1.0, PALETTE.line.color());
    for (y, label) in [(rest, "Rest"), (rest + depth, "−12 st")] {
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], hairline);
        painter.text(
            pos2(rect.right(), y - SPACE_2 * 0.5),
            Align2::RIGHT_BOTTOM,
            label,
            font::mono(FONT_EYEBROW),
            PALETTE.subtle.color(),
        );
    }

    // Newest at the middle, older further out on both sides.
    let half = rect.width() * 0.5;
    let at = |index: usize, side: f32| {
        let away = (HISTORY - 1 - index) as f32 / (HISTORY - 1) as f32;
        pos2(
            rect.center().x + side * away * half,
            rest - surface[index].clamp(-1.0, 1.0) * depth,
        )
    };
    let line: Vec<Pos2> = (0..HISTORY)
        .map(|index| at(index, -1.0))
        .chain((0..HISTORY).rev().map(|index| at(index, 1.0)))
        .collect();
    // Concave where the surface dips, so filled as a strip of columns.
    for pair in line.windows(2) {
        painter.add(Shape::convex_polygon(
            vec![
                pair[0],
                pair[1],
                pos2(pair[1].x, rect.bottom()),
                pos2(pair[0].x, rect.bottom()),
            ],
            note.at(0.12).color(),
            Stroke::NONE,
        ));
    }
    painter.add(Shape::line(line, Stroke::new(1.5, note.color())));

    let (low, high) = (dew::FIGURE_HZ.0.ln(), dew::FIGURE_HZ.1.ln());
    for reading in drops.chunks_exact(dew::READING).take(DROPS) {
        let [pan, height, level, age] = [reading[0], reading[1], reading[2], reading[3]];
        if level <= 0.0 {
            continue;
        }
        let hz = (low + (high - low) * height).exp();
        let tint = colour::note(colour::nearest(hz));
        let centre = pos2(
            rect.center().x + pan * half * 0.8,
            rest - SPACE_3 - (rest - rect.top() - SPACE_4 * 2.0) * height,
        );
        let alpha = (level * 4.0).min(1.0);
        painter.circle_stroke(
            centre,
            3.0 + age * RIPPLE_SPEED,
            Stroke::new(1.0, tint.at(alpha).color()),
        );
        painter.circle_filled(centre, 2.0, tint.at(alpha).color());
    }
}
