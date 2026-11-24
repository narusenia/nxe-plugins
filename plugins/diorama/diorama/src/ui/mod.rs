//! The Diorama 2 window, on the H4 layout (`design-system.md`).
//!
//! ```text
//! ┌───────────┬──────────────────────────────────────┐
//! │ Diorama   │ ◂ ▸ preset                           │
//! │ D / R     ├──────────────────────────────────────┤
//! │   +8.2 dB │ the arrivals against time            │
//! │ ┊┊┊┊┃┊┊┊  │ (the accent ground)                  │
//! │ ◕  ◕  ◕   │                                      │
//! │ ◕  ◕  ◕   ├──────────────────────────────────────┤
//! │ ◕  ◕      │ direct · reflected · clarity …  IN OUT│
//! │ NXE       │                                      │
//! └───────────┴──────────────────────────────────────┘
//! ```
//!
//! The subject is **the direct-to-reflected ratio**: how loud the voice is
//! against its room, which is what distance is heard as. **No tabs** — eight
//! controls fit in the sidebar, so the lower row only says what is happening
//! (`REQ-DIO-013`'s "one screen" survives the move).
//!
//! The size is fixed and never asked of the host to change: a window that
//! resized itself froze Ableton in v0 (`docs/HANDOVER.md`).

mod bind;
mod field;
mod presets;

use crate::params::DioramaParams;
use bind::Ctx;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::knob::knob;
use nxe_egui::preset::{PresetAction, preset_bar, step};
use nxe_egui::sidebar::{Subject, sidebar};
use nxe_egui::theme::{Palette, SIDEBAR_WIDTH, SPACE_2, SPACE_3, SPACE_4, WINDOW_WIDTH};
use nxe_egui::{meter, segment, smooth, window};
use std::sync::Arc;
use truce::prelude::*;
use truce_egui::EguiEditor;

const PALETTE: Palette = Palette::PARALLAX;
const KNOB: f32 = 40.0;
const RIGHT: f32 = WINDOW_WIDTH as f32 - SIDEBAR_WIDTH;
const METERS: f32 = 56.0;
/// How often a silent window checks whether the audio has started.
const ANALYSIS_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// The lower row: the taller of the two tabs.
const LOWER: f32 = nxe_egui::readout::HEIGHT + SPACE_2;

/// **The window's height, added up** (`.agents/rules/ui.md`). The sidebar's
/// three rows of macros set it, and the figure takes whatever the right
/// column's other parts leave — so the two columns end on the same line.
pub const HEIGHT: f32 =
    nxe_egui::sidebar::HEAD + 3.0 * LINE_KNOB + 2.0 * SPACE_3 + nxe_egui::sidebar::FOOT;
const FIGURE: f32 =
    HEIGHT - (nxe_egui::preset::HEIGHT + SPACE_3 + segment::HEIGHT + SPACE_2 + LOWER + SPACE_3);
const _: () = assert!(FIGURE >= 200.0, "the figure is squeezed");
const LINE_KNOB: f32 = nxe_egui::knob::block_height(KNOB);

pub fn create(params: Arc<DioramaParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (WINDOW_WIDTH, HEIGHT.ceil() as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !window(ui) {
        return;
    }
    let palette = &PALETTE;
    // **truce-egui only paints when input or a parameter changes**, and the
    // analysis handoffs are neither. The window looks again every
    // `ANALYSIS_POLL`; while there is sound it runs at the smoothing's 30 Hz.
    let sounding = ctx
        .params()
        .analysis
        .peaks
        .read()
        .iter()
        .any(|peak| *peak > 1e-4);
    ui.ctx().request_repaint_after(if sounding {
        smooth::FRAME
    } else {
        ANALYSIS_POLL
    });
    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
    let full = ui.max_rect();

    // The sidebar: the direct-to-reflected ratio, and the macros.
    let resolved = field::resolve(ctx.params());
    let ratio = field::direct_to_reflected(&resolved);
    let figure = nxe_egui::readout::figure(ratio, 1);
    let subject = Subject {
        name: "D / R",
        figure: &figure,
        unit: "dB",
        position: ratio.map_or(1.0, |ratio| ((ratio + 6.0) / 30.0).clamp(0.0, 1.0)),
    };
    let mut column = ui.new_child(
        UiBuilder::new().max_rect(Rect::from_min_size(full.min, vec2(SIDEBAR_WIDTH, HEIGHT))),
    );
    column.spacing_mut().item_spacing = vec2(0.0, 0.0);
    sidebar(
        &mut column,
        palette,
        "Diorama",
        &subject,
        HEIGHT,
        |ui, ink| {
            let params = ctx.params();
            let rows: [&[(u32, &str)]; 3] = [
                &[
                    (params.depth.id(), "Depth"),
                    (params.direct.id(), "Direct"),
                    (params.room.id(), "Room"),
                ],
                &[
                    (params.damping.id(), "Damping"),
                    (params.width.id(), "Width"),
                    (params.clarity.id(), "Clarity"),
                ],
                &[(params.mix.id(), "Mix"), (params.output.id(), "Output")],
            ];
            ui.spacing_mut().item_spacing = vec2(0.0, SPACE_3);
            for row in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(SPACE_2, 0.0);
                    for (id, name) in row.iter().copied() {
                        let control = knob(
                            ui,
                            ink,
                            name,
                            bind::normalized(ctx, id),
                            &ctx.format_param(id),
                            KNOB,
                        );
                        bind::control(ctx, id, &control);
                    }
                });
            }
        },
    );

    // The right column.
    let left = full.left() + SIDEBAR_WIDTH;
    let mut y = full.top();
    let strip = Rect::from_min_size(pos2(left, y), vec2(RIGHT, nxe_egui::preset::HEIGHT));
    top_strip(ui, ctx, palette, strip);
    y += nxe_egui::preset::HEIGHT;

    // The field is the other half of the L: the accent ground, drawn with the
    // inverted palette.
    let field_rect = Rect::from_min_size(pos2(left, y), vec2(RIGHT, FIGURE));
    let ink = palette.inverted();
    let painter = ui.painter_at(field_rect);
    painter.rect_filled(field_rect, 0.0, ink.ground.color());
    field::field(ui, ctx, &painter, field_rect, &ink, &resolved);
    y += FIGURE + SPACE_3;

    let body = Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(RIGHT - 2.0 * SPACE_3 - METERS - SPACE_4, LOWER),
    );
    readouts(ui, ctx, palette, body);
    meters(
        ui,
        ctx,
        palette,
        Rect::from_min_size(pos2(body.right() + SPACE_4, y), vec2(METERS, LOWER)),
    );
}

/// What the room is doing right now: the two buses, measured, and the two
/// things that work on the voice by distance.
fn readouts(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let analysis = &ctx.params().analysis;
    let buses = analysis.buses.read();
    // The buses are measured, so with nothing playing they have nothing to
    // say — a dash, not a confident `+0.0`.
    let sounding = analysis.peaks.read().iter().any(|peak| *peak > 1e-4);
    let live = smooth::smoothed(
        ui,
        ui.id().with("live"),
        &[
            buses[0].max(-90.0),
            buses[1].max(-90.0),
            analysis.clarity.read()[0],
            analysis.correlation.read()[0],
        ],
        smooth::TAU,
    );
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    let width = (rect.width() - 3.0 * SPACE_4) / 4.0;
    for (name, figure, unit) in [
        (
            "Direct",
            nxe_egui::readout::figure(sounding.then_some(live[0]), 1),
            "dB",
        ),
        (
            "Reflected",
            nxe_egui::readout::figure(sounding.then_some(live[1]), 1),
            "dB",
        ),
        ("Clarity", format!("{:+.1}", live[2]), "dB"),
        ("Room Corr.", format!("{:+.2}", live[3]), ""),
    ] {
        nxe_egui::readout::readout(&mut row, palette, name, &figure, unit, width);
    }
}

/// The preset bar, across the whole strip: Diorama has no switches.
fn top_strip(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    preset_row(&mut row, ctx, palette, rect.width());
}

/// The preset bar: the factory presets, then the user's.
fn preset_row(ui: &mut Ui, ctx: &Ctx, palette: &Palette, width: f32) {
    let store = presets::store();
    let user_id = ui.id().with("user-presets");
    let user: Vec<(String, String)> =
        match ui.data(|data| data.get_temp::<Vec<(String, String)>>(user_id)) {
            Some(user) => user,
            None => {
                let user: Vec<(String, String)> = presets::user_presets(&store)
                    .into_iter()
                    .map(|preset| (preset.name, preset.uri))
                    .collect();
                ui.data_mut(|data| data.insert_temp(user_id, user.clone()));
                user
            }
        };
    let mut names: Vec<String> = presets::FACTORY
        .iter()
        .map(|preset| preset.name.to_owned())
        .collect();
    names.extend(user.iter().map(|(name, _)| name.clone()));

    let loaded_id = ui.id().with("loaded-user-preset");
    let current = presets::FACTORY
        .iter()
        .position(|preset| presets::matches(ctx, preset))
        .or_else(|| ui.data(|data| data.get_temp::<usize>(loaded_id)));

    let load = |ui: &mut Ui, index: usize| {
        if let Some(preset) = presets::FACTORY.get(index) {
            presets::apply(ctx, &presets::values_of(ctx.params(), preset));
            ui.data_mut(|data| data.remove::<usize>(loaded_id));
        } else if let Some((_, uri)) = user.get(index - presets::FACTORY.len())
            && let Some(preset) = store.find(uri)
            && presets::load(ctx, &store, &preset).is_ok()
        {
            ui.data_mut(|data| data.insert_temp(loaded_id, index));
        }
    };
    match preset_bar(ui, palette, &names, current, false, width) {
        Some(PresetAction::Previous) => {
            if let Some(index) = step(current, names.len(), false) {
                load(ui, index);
            }
        }
        Some(PresetAction::Next) => {
            if let Some(index) = step(current, names.len(), true) {
                load(ui, index);
            }
        }
        Some(PresetAction::Load(index)) => load(ui, index),
        Some(PresetAction::Save(name)) if presets::save(ctx, &store, &name).is_ok() => {
            ui.data_mut(|data| data.remove::<Vec<(String, String)>>(user_id));
        }
        _ => {}
    }
}

/// IN and OUT, left and right, on one scale — **"is this better, or just
/// louder"** is the judgement a dynamics processor is used for.
fn meters(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let peaks = smooth::smoothed(
        ui,
        ui.id().with("peaks"),
        &ctx.params().analysis.peaks.read().map(level_of),
        smooth::TAU,
    );
    let holds = ctx.params().analysis.holds.read().map(level_of);
    for (index, (peak, hold)) in peaks.iter().zip(holds).enumerate() {
        let x = rect.left() + index as f32 * 14.0 + if index >= 2 { 6.0 } else { 0.0 };
        meter::meter(
            ui.painter(),
            palette,
            Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(6.0, rect.height() - 8.0)),
            *peak,
            hold,
            true,
        );
    }
}

/// An amplitude as a meter's fill: −60 dB at the bottom, 0 dB at the top.
fn level_of(amplitude: f32) -> f32 {
    ((20.0 * amplitude.max(1e-9).log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    /// **Every parameter has a control** (`.agents/rules/ui.md`). A parameter
    /// reachable only through the host's generic view is never mentioned by
    /// the window, and nothing else notices.
    #[test]
    fn every_parameter_is_on_screen() {
        const PARAMS: &str = include_str!("../params.rs");
        let views = [include_str!("mod.rs"), include_str!("field.rs")].concat();
        let fields: Vec<&str> = PARAMS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub "))
            .filter(|rest| {
                rest.contains(": FloatParam")
                    || rest.contains(": EnumParam<")
                    || rest.contains(": BoolParam")
            })
            .filter_map(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
            .collect();
        assert_eq!(fields.len(), 8, "the parameter scan moved: {fields:?}");
        for field in fields {
            assert!(
                views.contains(&format!(".{field}.id()")),
                "`{field}` has no control in the window"
            );
        }
    }
}
