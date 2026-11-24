//! The NXE Pumice editor, on the H4 window Sparkleur set (`v2-plan.md`, V2-9).
//!
//! ```text
//! ┌ sidebar ─────┬ presets ─────────────── Result│Delta  Low│Normal│High ┐
//! │ Pumice       ├ the figure, on the accent ──────────────────────────────┤
//! │ REDUCTION    │ spectrum · reduction · weight · nodes · range           │
//! │ 3 × 3 knobs  ├ node 1..6 · On · Freq / Width / Depth ────────── meters ┤
//! └──────────────┴──────────────────────────────────────────────────────────┘
//! ```
//!
//! **One screen, no tabs** (`REQ-PUM-013`): `DEPTH` and `SHARPNESS` are asked
//! together, so they cannot live in different halves of a disclosure.
//!
//! **The subject is the reduction**, the deepest cut in dB right now: a
//! process that works automatically has to show that it is working.

mod bind;
mod field;
mod nodes;
mod presets;

use crate::params::PumiceParams;
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

const PALETTE: Palette = Palette::PUMICE;
const KNOB: f32 = 40.0;
const RIGHT: f32 = WINDOW_WIDTH as f32 - SIDEBAR_WIDTH;
const DELTA_WIDTH: f32 = 128.0;
const QUALITY_WIDTH: f32 = 168.0;
const METERS: f32 = 56.0;
const ANALYSIS_POLL: std::time::Duration = std::time::Duration::from_millis(100);

const LINE_KNOB: f32 = nxe_egui::knob::block_height(KNOB);
/// **Arithmetic, not a number found by looking** (`.agents/rules/ui.md`):
/// the sidebar's three rows of knobs set it, as on every H4 window.
pub const HEIGHT: f32 =
    nxe_egui::sidebar::HEAD + 3.0 * LINE_KNOB + 2.0 * SPACE_3 + nxe_egui::sidebar::FOOT;
/// The figure takes what the strip and the node row leave. **Taller than the
/// other plugins' figures**, because it is the one that is worked in.
const FIGURE: f32 = HEIGHT - (nxe_egui::preset::HEIGHT + SPACE_3 + nodes::HEIGHT + SPACE_3);
const _: () = assert!(FIGURE >= 260.0, "the figure is squeezed");

pub fn create(params: Arc<PumiceParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (WINDOW_WIDTH, HEIGHT.ceil() as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !window(ui) {
        return;
    }
    let palette = &PALETTE;
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

    let reduction = ctx.params().analysis.readouts.read()[0];
    let cutting = reduction < -0.05;
    let shown = smooth::smoothed(
        ui,
        ui.id().with("reduction"),
        &[if cutting { reduction } else { 0.0 }],
        smooth::TAU,
    )[0];
    let figure = nxe_egui::readout::figure(cutting.then_some(shown), 1);
    let subject = Subject {
        name: "Reduction",
        figure: &figure,
        unit: "dB",
        position: (shown / field::REDUCTION_FLOOR_DB).clamp(0.0, 1.0),
    };

    let mut column = ui.new_child(
        UiBuilder::new().max_rect(Rect::from_min_size(full.min, vec2(SIDEBAR_WIDTH, HEIGHT))),
    );
    column.spacing_mut().item_spacing = vec2(0.0, 0.0);
    sidebar(
        &mut column,
        palette,
        "Pumice",
        &subject,
        HEIGHT,
        |ui, ink| {
            let params = ctx.params();
            let rows: [&[(u32, &str)]; 3] = [
                &[
                    (params.depth.id(), "Depth"),
                    (params.sharpness.id(), "Sharpness"),
                    (params.speed.id(), "Speed"),
                ],
                &[
                    (params.threshold.id(), "Threshold"),
                    (params.mix.id(), "Mix"),
                    (params.output.id(), "Output"),
                ],
                &[(params.low.id(), "Low"), (params.high.id(), "High")],
            ];
            ui.spacing_mut().item_spacing = vec2(0.0, SPACE_3);
            for row in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(SPACE_2, 0.0);
                    for (id, name) in row {
                        let control = knob(
                            ui,
                            ink,
                            name,
                            bind::normalized(ctx, *id),
                            &ctx.format_param(*id),
                            KNOB,
                        );
                        bind::control(ctx, *id, &control);
                    }
                });
            }
        },
    );

    let left = full.left() + SIDEBAR_WIDTH;
    let mut y = full.top();
    let strip = Rect::from_min_size(pos2(left, y), vec2(RIGHT, nxe_egui::preset::HEIGHT));
    top_strip(ui, ctx, palette, strip);
    y += nxe_egui::preset::HEIGHT;

    let selected_id = ui.id().with("node");
    let selected: usize = ui.data(|data| data.get_temp(selected_id)).unwrap_or(0);
    let field_rect = Rect::from_min_size(pos2(left, y), vec2(RIGHT, FIGURE));
    let ink = palette.inverted();
    let painter = ui.painter_at(field_rect);
    painter.rect_filled(field_rect, 0.0, ink.ground.color());
    let mut picked = field::field(ui, ctx, &painter, field_rect, &ink, selected);
    y += FIGURE + SPACE_3;

    let body = Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(RIGHT - 2.0 * SPACE_3 - METERS - SPACE_4, nodes::HEIGHT),
    );
    picked = nodes::nodes(ui, ctx, palette, body, selected).or(picked);
    if let Some(index) = picked {
        ui.data_mut(|data| data.insert_temp(selected_id, index));
    }
    meters(
        ui,
        ctx,
        palette,
        Rect::from_min_size(pos2(body.right() + SPACE_4, y), vec2(METERS, nodes::HEIGHT)),
    );
}

/// Presets, then what you listen to, then how big the transform is.
fn top_strip(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_2, 0.0);
    let preset_width = rect.width() - DELTA_WIDTH - QUALITY_WIDTH - 2.0 * SPACE_2;
    preset_row(&mut row, ctx, palette, preset_width);
    let delta = usize::from(params.delta.value());
    if let Some(picked) =
        segment::segment(&mut row, palette, &["Result", "Delta"], delta, DELTA_WIDTH)
    {
        bind::set_once(ctx, params.delta.id(), picked as f64);
    }
    if let Some(picked) = segment::segment(
        &mut row,
        palette,
        &["Low", "Normal", "High"],
        params.quality.index() as usize,
        QUALITY_WIDTH,
    ) {
        let id = params.quality.id();
        bind::set_once(ctx, id, bind::normalize(ctx, id, picked as f64));
    }
}

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

/// IN L, IN R, OUT L, OUT R.
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
            Rect::from_min_size(pos2(x, rect.top()), vec2(6.0, rect.height())),
            *peak,
            hold,
            true,
        );
    }
}

fn level_of(amplitude: f32) -> f32 {
    ((20.0 * amplitude.max(1e-9).log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    /// **Every parameter has somewhere to be touched** (`SPK-19`). The node
    /// fields live in `node!`, and the window reaches all of them through
    /// `node_ids(`.
    #[test]
    fn every_parameter_is_on_screen() {
        const PARAMS: &str = include_str!("../params.rs");
        let views = [
            include_str!("mod.rs"),
            include_str!("field.rs"),
            include_str!("nodes.rs"),
        ]
        .concat();
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
        assert_eq!(fields.len(), 14, "the parameter scan moved: {fields:?}");
        let node_fields = ["enabled", "freq", "width", "depth"];
        for (index, field) in fields.iter().enumerate() {
            let by_node = index < node_fields.len() && views.contains("node_ids(");
            assert!(
                views.contains(&format!(".{field}.id()")) || by_node,
                "`{field}` has no control in the window"
            );
        }
    }
}
