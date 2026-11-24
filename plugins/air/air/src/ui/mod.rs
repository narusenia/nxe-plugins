//! The Air 2 window, on the H4 layout (`design-system.md`).
//!
//! ```text
//! ┌───────────┬──────────────────────────────────────┐
//! │ Air       │ ◂ ▸ preset                    [2x|4x]│
//! │ LAYER     ├──────────────────────────────────────┤
//! │  -24.0 dB │ the figure (the accent ground):      │
//! │ ┊┊┊┊┃┊┊┊  │ source, layer, and where it sits    │
//! │ ◕  ◕  ◕   ├──────────────────────────────────────┤
//! │ ◕  ◕  ◕   │ [Follow|Curve]                IN OUT │
//! │ ◕  ◕      │ three knobs, and what they do now    │
//! │ NXE       │                                      │
//! └───────────┴──────────────────────────────────────┘
//! ```
//!
//! The subject is **the layer**: how loud what Air adds is, against what came
//! in. An additive plugin is read by how much it is adding.
//!
//! The size is fixed and never asked of the host to change: a window that
//! resized itself froze Ableton in v0 (`docs/HANDOVER.md`).

mod bind;
mod field;
mod panel;
mod presets;

use crate::params::AirParams;
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

const PALETTE: Palette = Palette::AIR;
const KNOB: f32 = 40.0;
const RIGHT: f32 = WINDOW_WIDTH as f32 - SIDEBAR_WIDTH;
const FACTOR_WIDTH: f32 = 80.0;
const METERS: f32 = 56.0;
/// How often a silent window checks whether the audio has started.
const ANALYSIS_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// The lower row: the taller of the two tabs.
const LOWER: f32 = panel::HEIGHT;

/// **The window's height, added up** (`.agents/rules/ui.md`). The sidebar's
/// three rows of macros set it, and the figure takes whatever the right
/// column's other parts leave — so the two columns end on the same line.
pub const HEIGHT: f32 =
    nxe_egui::sidebar::HEAD + 3.0 * LINE_KNOB + 2.0 * SPACE_3 + nxe_egui::sidebar::FOOT;
const FIGURE: f32 =
    HEIGHT - (nxe_egui::preset::HEIGHT + SPACE_3 + segment::HEIGHT + SPACE_2 + LOWER + SPACE_3);
const _: () = assert!(FIGURE >= 200.0, "the figure is squeezed");
const LINE_KNOB: f32 = nxe_egui::knob::block_height(KNOB);

pub fn create(params: Arc<AirParams>) -> Box<dyn Editor> {
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

    // The sidebar: the layer against the input, and the macros.
    let layer = layer_db(
        &ctx.params().analysis.layer.read(),
        &ctx.params().analysis.dry.read(),
    );
    let shown = smooth::smoothed(
        ui,
        ui.id().with("layer"),
        &[layer.unwrap_or(-60.0)],
        smooth::TAU,
    )[0];
    let figure = nxe_egui::readout::figure(layer.map(|_| shown), 1);
    let subject = Subject {
        name: "Layer",
        figure: &figure,
        unit: "dB",
        position: ((shown + 60.0) / 60.0).clamp(0.0, 1.0),
    };
    let mut column = ui.new_child(
        UiBuilder::new().max_rect(Rect::from_min_size(full.min, vec2(SIDEBAR_WIDTH, HEIGHT))),
    );
    column.spacing_mut().item_spacing = vec2(0.0, 0.0);
    sidebar(&mut column, palette, "Air", &subject, HEIGHT, |ui, ink| {
        let params = ctx.params();
        let rows: [&[(u32, &str)]; 3] = [
            &[
                (params.surface.id(), "Surface"),
                (params.blend.id(), "Blend"),
                (params.character.id(), "Character"),
            ],
            &[
                (params.focus.id(), "Focus"),
                (params.width.id(), "Width"),
                (params.follow.id(), "Follow"),
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
    });

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
    field::field(ui, ctx, &painter, field_rect, &ink);
    y += FIGURE + SPACE_3;

    let tab_id = ui.id().with("tab");
    let tab: usize = ui.data(|data| data.get_temp(tab_id)).unwrap_or(0);
    let mut tabs = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(180.0, segment::HEIGHT),
    )));
    if let Some(picked) = segment::segment(&mut tabs, palette, &["Follow", "Curve"], tab, 180.0) {
        ui.data_mut(|data| data.insert_temp(tab_id, picked));
    }
    y += segment::HEIGHT + SPACE_2;

    let body = Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(RIGHT - 2.0 * SPACE_3 - METERS - SPACE_4, LOWER),
    );
    if tab == 0 {
        panel::follow(ui, ctx, palette, body, KNOB);
    } else {
        panel::curve(ui, ctx, palette, body, KNOB);
    }
    meters(
        ui,
        ctx,
        palette,
        Rect::from_min_size(pos2(body.right() + SPACE_4, y), vec2(METERS, LOWER)),
    );
}

/// The preset bar, and the oversampling switch.
fn top_strip(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_2, 0.0);
    let preset_width = rect.width() - FACTOR_WIDTH - SPACE_2;
    preset_row(&mut row, ctx, palette, preset_width);

    let (id, index) = (params.oversample.id(), params.oversample.index() as usize);
    if let Some(picked) = segment::segment(&mut row, palette, &["2x", "4x"], index, FACTOR_WIDTH) {
        bind::set_once(ctx, id, bind::normalize(ctx, id, picked as f64));
    }
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

/// How loud the added layer is against the input, in dB, from the two
/// spectra's energy. `None` while nothing is coming in.
fn layer_db(layer: &[f32], dry: &[f32]) -> Option<f32> {
    let energy = |levels: &[f32]| levels.iter().map(|level| level * level).sum::<f32>();
    let dry = energy(dry);
    (dry > 1e-12).then(|| (10.0 * (energy(layer).max(1e-12) / dry).log10()).max(-60.0))
}

/// An amplitude as a meter's fill: −60 dB at the bottom, 0 dB at the top.
fn level_of(amplitude: f32) -> f32 {
    ((20.0 * amplitude.max(1e-9).log10() + 60.0) / 60.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::layer_db;

    #[test]
    fn the_layer_is_its_energy_against_the_input() {
        assert_eq!(layer_db(&[0.0; 4], &[0.0; 4]), None);
        let layer = layer_db(&[0.1; 4], &[1.0; 4]).unwrap();
        assert!((layer + 20.0).abs() < 1e-3, "{layer}");
    }

    /// **Every parameter has a control** (`.agents/rules/ui.md`). A parameter
    /// reachable only through the host's generic view is never mentioned by
    /// the window, and nothing else notices.
    #[test]
    fn every_parameter_is_on_screen() {
        const PARAMS: &str = include_str!("../params.rs");
        let views = [
            include_str!("mod.rs"),
            include_str!("field.rs"),
            include_str!("panel.rs"),
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
        assert_eq!(fields.len(), 15, "the parameter scan moved: {fields:?}");
        for field in fields {
            assert!(
                views.contains(&format!(".{field}.id()")),
                "`{field}` has no control in the window"
            );
        }
    }
}
