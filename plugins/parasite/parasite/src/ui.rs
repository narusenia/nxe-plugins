//! The Parasite window, on the Design frame (`nxe_egui::design`). The figure
//! is two host periods of the input in grey and the output over it in the
//! accent, started on the host's period, so a held note stands still and
//! what the parasite did to it is the difference between the lines. The
//! table says what is driving the parasite now.

use crate::params::{ParasiteParams, Species};
use egui::{Align2, Pos2, Rect, Shape, Stroke, Ui, UiBuilder, pos2, vec2};
use kit_ui::{Factory, Kit};
use nxe_egui::design::{self, design};
use nxe_egui::font;
use nxe_egui::theme::{FONT_SUBJECT, FONT_VALUE, Palette, SPACE_2, SPACE_3, SPACE_4};
use parasite_core::SCOPE_POINTS;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<ParasiteParams>;

/// The drafting family's accent (`design-system.md`, Design 7).
const PALETTE: Palette = Palette::DESIGN_KIT;

/// Every parameter in the window's order; the presets store them in it.
pub fn ids(params: &ParasiteParams) -> [u32; 9] {
    [
        params.sensitivity.id(),
        params.resistance.id(),
        params.species.id(),
        params.infection.id(),
        params.spread.id(),
        params.hunger.id(),
        params.mutation.id(),
        params.mix.id(),
        params.output.id(),
    ]
}

const NAMES: [&str; 9] = [
    "Sensitivity",
    "Resistance",
    "Species",
    "Infection",
    "Spread",
    "Hunger",
    "Mutation",
    "Mix",
    "Output",
];

/// Sensitivity, Resistance, Species, Infection, Spread, Hunger, Mutation,
/// Mix, Output. A first draft; settling them needs an ear.
pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        values: &[0.5, 0.5, 0.0, 0.5, 0.0, 0.3, 0.1, 1.0, 0.0],
    },
    Factory {
        name: "Carrion",
        values: &[0.5, 0.3, 1.0, 0.8, 0.3, 0.6, 0.2, 0.7, 0.0],
    },
    Factory {
        name: "Choir Mite",
        values: &[0.6, 0.6, 3.0, 0.6, 0.6, 0.4, 0.3, 0.8, 0.0],
    },
    Factory {
        name: "Sync Tick",
        values: &[0.5, 0.8, 2.0, 0.7, 0.0, 0.2, 0.05, 1.0, 0.0],
    },
    Factory {
        name: "Drift",
        values: &[0.5, 0.4, 0.0, 0.4, 0.25, 0.5, 0.6, 0.6, 0.0],
    },
];

pub fn create(params: Arc<ParasiteParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (design::WIDTH as u32, design::HEIGHT as u32), view)
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
    let areas = design(
        ui,
        &PALETTE,
        "Parasite",
        "Design",
        ["Host", "Parasite", "Output"],
    );

    let knobs: Vec<(u32, &str)> = ids.iter().copied().zip(NAMES).collect();
    // The presets sit on the head's accent band, so they take its inverted palette.
    let band = PALETTE.inverted();
    let kit = Kit {
        name: "Parasite",
        label: "Design",
        product: "Parasite",
        plugin_id: "com.nxe.parasite",
        palette: &band,
        knobs: &knobs,
        factory: &FACTORY,
    };
    let presets = Rect::from_min_size(
        pos2(
            areas.presets.right() - 180.0,
            areas.presets.center().y - nxe_egui::preset::HEIGHT * 0.5,
        ),
        vec2(180.0, nxe_egui::preset::HEIGHT),
    );
    kit_ui::presets(
        &mut ui.new_child(UiBuilder::new().max_rect(presets)),
        ctx,
        &kit,
    );

    // Bypassed or stopped, the last picture would stay up: show silence then.
    let (_, quiet) = kit_ui::arrivals(ui, "parasite-scopes", params.count.load(Ordering::Relaxed));
    let live = quiet < kit_ui::STALE_SECONDS;
    let [hz, presence, infection, hunger] = if live {
        params.readings.read()
    } else {
        [0.0; 4]
    };
    let (host, output) = if live {
        (params.host.read(), params.output_wave.read())
    } else {
        ([0.0; SCOPE_POINTS], [0.0; SCOPE_POINTS])
    };
    figure(ui, areas.figure, &host, &output, hz, presence, hunger);

    let column = |ui: &mut Ui, rect: Rect| {
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.spacing_mut().item_spacing = vec2(0.0, SPACE_3);
        child
    };
    let [host_area, parasite_area, output_area] = areas.columns;
    let mut host_column = column(ui, host_area);
    for (id, name) in [(ids[0], NAMES[0]), (ids[1], NAMES[1])] {
        kit_ui::slider_for(&mut host_column, ctx, &PALETTE, id, name, host_area.width());
    }
    let mut parasite_column = column(ui, parasite_area);
    let index = params.species.index() as usize;
    // The pictures say what each is; the name of the one in force sits on
    // the column's title line.
    design::caps(
        ui.painter(),
        pos2(parasite_area.right(), parasite_area.top() - SPACE_2),
        Align2::RIGHT_BOTTOM,
        Species::NAMES[index.min(Species::NAMES.len() - 1)],
        PALETTE.accent.color(),
    );
    if let Some(picked) = design::glyphs(
        &mut parasite_column,
        &PALETTE,
        &Species::NAMES,
        index,
        parasite_area.width(),
        species_glyph,
    ) {
        kit_ui::set_plain(ctx, ids[2], picked as f64);
    }
    for slot in 3..7 {
        kit_ui::slider_for(
            &mut parasite_column,
            ctx,
            &PALETTE,
            ids[slot],
            NAMES[slot],
            parasite_area.width(),
        );
    }
    let mut output_column = column(ui, output_area);
    for slot in 7..9 {
        kit_ui::slider_for(
            &mut output_column,
            ctx,
            &PALETTE,
            ids[slot],
            NAMES[slot],
            output_area.width(),
        );
    }

    let following = presence > 0.05;
    let rows = [
        (
            "Host pitch",
            "Parasite pitch",
            if following {
                let (name, frequency) = note(hz);
                format!("{name} · {frequency}")
            } else {
                "—".to_owned()
            },
        ),
        ("Host clarity", "Presence", percent(presence)),
        ("Held note", "Hunger", percent(hunger)),
        ("Attack", "Infection", percent(infection)),
    ];
    for (row, (from, to, figure)) in rows.iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(
                areas.feeds.left(),
                areas.feeds.top() + row as f32 * design::ROW,
            ),
            vec2(areas.feeds.width(), design::ROW),
        );
        design::feed(ui, &PALETTE, rect, from, to, figure);
    }
}

/// Each species as a small wave: FM bunches and spreads, Ring swells inside
/// an envelope, Sync jumps back to its start, Voice is a second wave along
/// the first.
fn species_glyph(painter: &egui::Painter, rect: Rect, colour: egui::Color32, index: usize) {
    use std::f32::consts::{PI, TAU};
    const STEPS: usize = 48;
    let wave = |f: &dyn Fn(f32) -> f32, offset: f32| -> Vec<Pos2> {
        (0..=STEPS)
            .map(|step| {
                let t = step as f32 / STEPS as f32;
                pos2(
                    rect.left() + rect.width() * t,
                    rect.center().y + offset - f(t) * rect.height(),
                )
            })
            .collect()
    };
    let stroke = Stroke::new(1.5, colour);
    match index {
        0 => {
            let fm = |t: f32| (TAU * 3.0 * t + 2.0 * (TAU * t).sin()).sin() * 0.5;
            painter.add(Shape::line(wave(&fm, 0.0), stroke));
        }
        1 => {
            let ring = |t: f32| (TAU * 5.0 * t).sin() * (PI * t).sin() * 0.5;
            painter.add(Shape::line(wave(&ring, 0.0), stroke));
        }
        2 => {
            // Two periods of the host, the voice restarting at each.
            for half in 0..2 {
                let from = half as f32 * 0.5;
                let points: Vec<Pos2> = (0..=STEPS / 2)
                    .map(|step| {
                        let u = step as f32 / (STEPS / 2) as f32;
                        pos2(
                            rect.left() + rect.width() * (from + u * 0.5),
                            rect.center().y - (TAU * 1.6 * u).sin() * rect.height() * 0.5,
                        )
                    })
                    .collect();
                painter.add(Shape::line(points, stroke));
            }
        }
        _ => {
            let low = |t: f32| (TAU * 2.0 * t).sin() * 0.3;
            let high = |t: f32| (TAU * 4.0 * t).sin() * 0.2;
            painter.add(Shape::line(wave(&low, rect.height() * 0.2), stroke));
            painter.add(Shape::line(wave(&high, -rect.height() * 0.3), stroke));
        }
    }
}

fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

/// The note's name and the frequency: `("A3", "220.0 Hz")`.
fn note(hz: f32) -> (String, String) {
    (nxe_egui::text::note(hz), format!("{hz:.1} Hz"))
}

/// The host in grey and the output over it, two host periods across, on the
/// frame's grid; above them what is followed, below how much is eaten.
fn figure(
    ui: &Ui,
    rect: Rect,
    host: &[f32; SCOPE_POINTS],
    output: &[f32; SCOPE_POINTS],
    hz: f32,
    presence: f32,
    hunger: f32,
) {
    let painter = ui.painter_at(rect);
    let inner = rect.shrink(SPACE_4);
    let middle = rect.center().y;
    // The loudest of the two fills a third of the height either way, so a
    // quiet host still shows and the readouts stay clear.
    let peak = host
        .iter()
        .chain(output)
        .fold(0.0f32, |peak, x| peak.max(x.abs()))
        .max(0.05);
    let scale = rect.height() * 0.3 / peak;
    let line = |values: &[f32; SCOPE_POINTS]| -> Vec<Pos2> {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                pos2(
                    rect.left() + rect.width() * index as f32 / (SCOPE_POINTS - 1) as f32,
                    middle - value * scale,
                )
            })
            .collect()
    };
    painter.add(Shape::line(
        line(host),
        Stroke::new(1.5, PALETTE.muted.at(0.8).color()),
    ));
    painter.add(Shape::line(
        line(output),
        Stroke::new(2.0, PALETTE.accent.at(0.35 + 0.65 * presence).color()),
    ));

    // What is followed: the note large, the frequency under it.
    let following = presence > 0.05;
    design::caps(
        &painter,
        inner.left_top(),
        Align2::LEFT_TOP,
        "Host",
        PALETTE.muted.color(),
    );
    let (name, frequency) = if following {
        note(hz)
    } else {
        ("—".to_owned(), "no pitch".to_owned())
    };
    let big = painter.text(
        inner.left_top() + vec2(0.0, 14.0),
        Align2::LEFT_TOP,
        name,
        font::drafting_medium(FONT_SUBJECT * 0.8),
        PALETTE.ink.color(),
    );
    painter.text(
        pos2(inner.left(), big.bottom()),
        Align2::LEFT_TOP,
        frequency,
        font::mono(FONT_VALUE),
        PALETTE.muted.color(),
    );

    // How much the parasite has eaten.
    design::caps(
        &painter,
        inner.right_top(),
        Align2::RIGHT_TOP,
        "Feeding",
        PALETTE.accent.color(),
    );
    painter.text(
        inner.right_top() + vec2(0.0, 14.0),
        Align2::RIGHT_TOP,
        format!("{:.0}", hunger * 100.0),
        font::drafting_medium(FONT_SUBJECT * 0.8),
        PALETTE.accent.color(),
    );
    // A gauge along the foot, the same quantity.
    let foot = Rect::from_min_max(
        pos2(inner.left(), inner.bottom() - 2.0),
        pos2(inner.right(), inner.bottom()),
    );
    painter.rect_filled(foot, 0.0, PALETTE.line.color());
    painter.rect_filled(
        Rect::from_min_max(
            foot.min,
            pos2(foot.left() + foot.width() * hunger, foot.bottom()),
        ),
        0.0,
        PALETTE.accent.color(),
    );
}

#[cfg(test)]
mod tests {
    use super::note;

    #[test]
    fn notes_are_named() {
        assert_eq!(note(440.0), ("A4".to_owned(), "440.0 Hz".to_owned()));
        assert_eq!(note(261.6).0, "C4");
    }
}
