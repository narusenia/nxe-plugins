//! The Prisma window, on the Colour frame (`nxe_egui::colour`). The figure is
//! a prism: the input comes in from the left as one white beam, and leaves
//! split into a ray for every partial ringing, in its note's colour, landing
//! on a screen at its pitch — higher partials bent further, as light is. The
//! screen is a pitch scale; each ray leaves a bar there as long as it rings.
//! Drawn in hairlines on the panel, no gradients: the only colour is the
//! notes'.

use crate::params::{PrismaParams, SHOWN, Source, Tuning};
use egui::{Align, Align2, Layout, Pos2, Rect, Shape, Stroke, Ui, UiBuilder, pos2, vec2};
use kit_ui::{Factory, Kit};
use nxe_egui::colour::{self, colour};
use nxe_egui::font;
use nxe_egui::segment::HEIGHT as ROW;
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_1, SPACE_2, SPACE_3, SPACE_4};
use resonance_core::prisma::{NOTES, PARTIALS, READING};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<PrismaParams>;

const PALETTE: Palette = Palette::COLOUR;
/// The chord's name in the strip.
const FONT_NAME: f32 = 28.0;
/// The screen's pitch span, low to high.
const SCREEN_HZ: (f32, f32) = (40.0, 8_000.0);
const GUIDES: [(f32, &str); 4] = [
    (100.0, "100"),
    (500.0, "500"),
    (1_000.0, "1k"),
    (5_000.0, "5k"),
];
/// The pitch classes' grid in the strip.
const GRID: f32 = 360.0;
/// A bar on the screen at full level, in points.
const BAR: f32 = 36.0;

/// The knobs in the window's order; the presets store them in it.
pub fn ids(params: &PrismaParams) -> [u32; 8] {
    [
        params.sensitivity.id(),
        params.decay.id(),
        params.inharmonic.id(),
        params.coupling.id(),
        params.tone.id(),
        params.shimmer.id(),
        params.mix.id(),
        params.output.id(),
    ]
}

const NAMES: [&str; 8] = [
    "Sensitivity",
    "Decay",
    "Inharmonic",
    "Coupling",
    "Tone",
    "Shimmer",
    "Wet",
    "Output",
];

/// Sensitivity, Decay, Inharmonic, Coupling, Tone, Shimmer, Wet, Output. A
/// first draft; settling them needs an ear.
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.0, 0.2, 0.5, 0.2, 0.5, 0.0],
    },
    Factory {
        name: "Glass",
        values: &[0.6, 0.6, 0.35, 0.3, 0.8, 0.15, 0.5, 0.0],
    },
    Factory {
        name: "Bell Choir",
        values: &[0.55, 0.75, 0.8, 0.4, 0.6, 0.3, 0.45, -2.0],
    },
    Factory {
        name: "Mycelium",
        values: &[0.5, 0.85, 0.2, 0.9, 0.4, 0.4, 0.5, -3.0],
    },
    Factory {
        name: "Halo",
        values: &[0.5, 0.4, 0.0, 0.0, 0.5, 0.3, 0.4, 0.0],
    },
];

pub fn create(params: Arc<PrismaParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (colour::WIDTH as u32, colour::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    ui.ctx().request_repaint_after(nxe_egui::smooth::FRAME);
    let params = ctx.params();
    let ids = ids(params);
    let areas = colour(ui.painter(), ui.max_rect(), &PALETTE, "Prisma", "Colour");

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
            name: "Prisma",
            label: "Colour",
            product: "Prisma",
            plugin_id: "com.nxe.prisma",
            palette: &PALETTE,
            knobs: &knobs,
            factory: &FACTORY,
        },
    );

    let midi = params.source.index() as usize == Source::Midi as usize;
    let (_, quiet) = kit_ui::arrivals(ui, "prisma-frames", params.count.load(Ordering::Relaxed));
    let live = quiet < kit_ui::STALE_SECONDS;
    let mut shown = if live {
        params.shown.read()
    } else {
        [0.0; SHOWN]
    };
    // Stopped or silent, the chord still shows what it would ring, faintly.
    let ghost = !midi && shown.chunks_exact(READING).all(|r| r[1] < 1e-4);
    if ghost {
        let root = kit_ui::plain(ctx, params.root.id()) as f32;
        let ratios = crate::chord(params.chord.index() as usize).ratios();
        for (index, ratio) in ratios.iter().enumerate() {
            let note = 69.0 + 12.0 * (root * ratio / 440.0).log2();
            for k in 0..PARTIALS {
                let at = (index * PARTIALS + k) * READING;
                shown[at] = root * ratio * (k + 1) as f32;
                shown[at + 2] = note;
                shown[at + 3] = 1.0;
            }
        }
    }

    strip(ui, areas.strip, &shown);

    let row = Rect::from_min_max(
        pos2(areas.figure.left(), areas.figure.bottom() - ROW),
        areas.figure.max,
    );
    controls(ui, ctx, row);
    let figure = Rect::from_min_max(
        areas.figure.min,
        pos2(areas.figure.right(), row.top() - SPACE_3),
    );
    prism(ui, figure, &shown, ghost);
}

/// The chord's name, and the pitch classes ringing (`colour::classes`).
fn strip(ui: &Ui, rect: Rect, shown: &[f32; SHOWN]) {
    let painter = ui.painter();
    let mut notes: Vec<i32> = shown
        .chunks_exact(PARTIALS * READING)
        .filter(|chunk| chunk[0] > 0.0 && (chunk[3] > 0.0 || chunk[1] > 1e-3))
        .map(|chunk| chunk[2].round() as i32)
        .collect();
    notes.sort_unstable();
    notes.dedup();

    // Named from what rings, in either source; a cluster with no name
    // counts its notes instead.
    // Named from what rings, in either source: nothing is N.C., and a
    // cluster with no name counts its notes instead.
    let title = colour::chord_name(&notes).unwrap_or_else(|| match notes.len() {
        0 => "N.C.".to_owned(),
        count => format!("{count} notes"),
    });
    let y = rect.center().y;
    painter.text(
        pos2(rect.left(), y),
        Align2::LEFT_CENTER,
        title,
        font::medium(FONT_NAME),
        PALETTE.ink.color(),
    );
    let mut octaves = [0u8; 12];
    for note in &notes {
        octaves[note.rem_euclid(12) as usize] += 1;
    }
    let grid = Rect::from_min_max(pos2(rect.right() - GRID, rect.top()), rect.right_bottom());
    colour::classes(painter, &PALETTE, grid, &octaves);
}

/// Source, Chord, Tuning, Root and Freeze, in one row from the right.
fn controls(ui: &mut Ui, ctx: &Ctx, rect: Rect) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::right_to_left(Align::Center)),
    );
    row.spacing_mut().item_spacing.x = SPACE_4;
    let freeze = params.freeze.value();
    if nxe_egui::toggle::toggle(
        &mut row,
        &PALETTE,
        "Freeze",
        if freeze { "On" } else { "Off" },
        freeze,
    ) {
        kit_ui::set_plain(ctx, params.freeze.id(), if freeze { 0.0 } else { 1.0 });
    }
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
    let chord = params.chord.index() as usize;
    if let Some(picked) = nxe_egui::select::select(
        &mut row,
        &PALETTE,
        "Chord",
        &crate::params::Chord::NAMES,
        chord,
    ) {
        kit_ui::set_plain(ctx, params.chord.id(), picked as f64);
    }
    let source = params.source.index() as usize;
    if let Some(picked) =
        nxe_egui::select::select(&mut row, &PALETTE, "Source", &Source::NAMES, source)
    {
        kit_ui::set_plain(ctx, params.source.id(), picked as f64);
    }
}

/// Where `hz` lands on the screen, `0..=1` from the bottom.
fn height(hz: f32) -> f32 {
    let (low, high) = (SCREEN_HZ.0.ln(), SCREEN_HZ.1.ln());
    ((hz.max(1.0).ln() - low) / (high - low)).clamp(0.0, 1.0)
}

fn prism(ui: &Ui, rect: Rect, shown: &[f32; SHOWN], ghost: bool) {
    let painter = ui.painter_at(rect.expand(SPACE_2));
    font::tracked(
        &painter,
        rect.left_top(),
        Align2::LEFT_TOP,
        "Partials, by note",
        FONT_EYEBROW,
        PALETTE.subtle.color(),
    );
    let hairline = Stroke::new(1.0, PALETTE.line.color());

    // The screen: a pitch scale at the right, its guides across to the bars.
    let screen_x = rect.right() - BAR - SPACE_4 * 2.0;
    let top = rect.top() + SPACE_4 + SPACE_2;
    let bottom = rect.bottom();
    let y_of = |h: f32| bottom - (bottom - top) * h;
    painter.line_segment([pos2(screen_x, top), pos2(screen_x, bottom)], hairline);
    for (hz, label) in GUIDES {
        let y = y_of(height(hz));
        painter.line_segment(
            [pos2(screen_x - SPACE_1, y), pos2(screen_x + SPACE_1, y)],
            hairline,
        );
        painter.text(
            pos2(rect.right(), y),
            Align2::RIGHT_CENTER,
            label,
            font::mono(FONT_EYEBROW),
            PALETTE.subtle.color(),
        );
    }

    // The prism: an equilateral triangle, apex up, a third of the way in.
    let middle = (top + bottom) * 0.5;
    let side = (bottom - top) * 0.62;
    let rise = side * 3f32.sqrt() * 0.5;
    let centre_x = rect.left() + (screen_x - rect.left()) * 0.36;
    let apex = pos2(centre_x, middle - rise * 0.5);
    let base_left = pos2(centre_x - side * 0.5, middle + rise * 0.5);
    let base_right = pos2(centre_x + side * 0.5, middle + rise * 0.5);
    painter.add(Shape::convex_polygon(
        vec![apex, base_right, base_left],
        PALETTE.ink.at(0.03).color(),
        Stroke::new(1.0, PALETTE.ink.at(0.6).color()),
    ));

    let along = |from: Pos2, to: Pos2, t: f32| from + (to - from) * t;
    // The beam meets the left face just under its middle, rising a little.
    let entry = along(apex, base_left, 0.58);
    let total: f32 = shown.chunks_exact(READING).map(|r| r[1]).sum();
    let beam = if ghost {
        0.25
    } else {
        (0.35 + total * 3.0).min(1.0)
    };
    painter.line_segment(
        [pos2(rect.left(), entry.y + side * 0.12), entry],
        Stroke::new(1.0 + beam, PALETTE.ink.at(beam).color()),
    );

    // Every partial: in the glass from the entry, then out of the right face
    // (higher bent further, so lower on the face) to its pitch on the screen.
    let mut rays: Vec<(f32, f32, f32)> = shown
        .chunks_exact(READING)
        .take(NOTES * PARTIALS)
        .filter(|r| r[0] > 0.0)
        .map(|r| (r[0], r[1], r[2]))
        .collect();
    // Quiet first, so a loud ray is drawn over them.
    rays.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (hz, level, note) in rays {
        let h = height(hz);
        let tint = colour::note(note.round() as i32);
        let strength = if ghost {
            0.22
        } else {
            (0.18 + level * 6.0).min(1.0)
        };
        let exit = along(apex, base_right, 0.3 + 0.5 * (1.0 - h));
        let land = pos2(screen_x, y_of(h));
        painter.line_segment(
            [entry, exit],
            Stroke::new(1.0, tint.at(strength * 0.35).color()),
        );
        painter.line_segment([exit, land], Stroke::new(1.0, tint.at(strength).color()));
        let length = if ghost {
            3.0
        } else {
            (level * 6.0).min(1.0) * BAR
        };
        painter.line_segment(
            [land, land + vec2(length.max(2.0), 0.0)],
            Stroke::new(2.0, tint.at(strength).color()),
        );
    }
}
