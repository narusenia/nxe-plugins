//! The Doubler 2 window, on the H4 layout (`design-system.md`).
//!
//! ```text
//! ┌──────────┬──────────────────────────────────────┐
//! │ Doubler  │ ◂ ▸ preset           [2|4|8] [Mono|St]│
//! │ CORREL.  ├──────────────────────────────────────┤
//! │   +0.62  │ Voice Field (the accent ground)      │
//! │ ┊┊┊┊┃┊┊┊ │                                      │
//! │ ◕   ◕    ├──────────────────────────────────────┤
//! │ ◕   ◕    │ [Tone|Voices]                        │
//! │ ◕   ◕    │ the shelves over the spectrum, or    │
//! │ NXE      │ the eight voices' shapes             │
//! └──────────┴──────────────────────────────────────┘
//! ```
//!
//! The sidebar and the field are one L of accent. The subject is the
//! correlation — **the one number a widener owes its user** (`nxe_dsp`): how
//! close the image is to cancelling in mono.
//!
//! The size is fixed and never asked of the host to change: a window that
//! resized itself froze Ableton in v0 (`docs/HANDOVER.md`).

mod bind;
mod field;
mod presets;
mod table;
mod tone;

use crate::params::DoublerParams;
use bind::Ctx;
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::knob::{block_height, knob};
use nxe_egui::preset::{PresetAction, preset_bar, step};
use nxe_egui::sidebar::{Subject, sidebar};
use nxe_egui::theme::{Palette, SIDEBAR_WIDTH, SPACE_2, SPACE_3, SPACE_4, WINDOW_WIDTH};
use nxe_egui::{meter, segment, smooth, window};
use std::sync::Arc;
use truce::prelude::*;
use truce_egui::EguiEditor;

const PALETTE: Palette = Palette::DOUBLER;
const KNOB: f32 = 40.0;
const FIGURE: f32 = 220.0;
const RIGHT: f32 = WINDOW_WIDTH as f32 - SIDEBAR_WIDTH;
const VOICES_WIDTH: f32 = 96.0;
const SOURCE_WIDTH: f32 = 140.0;
/// How often a silent window checks whether the audio has started.
const ANALYSIS_POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// **The window's height, added up** (`.agents/rules/ui.md`): whichever
/// column is taller, the sidebar with its three rows of macros or the right
/// column's parts.
const RIGHT_HEIGHT: f32 = nxe_egui::preset::HEIGHT
    + FIGURE
    + SPACE_3
    + segment::HEIGHT
    + SPACE_2
    + table::HEIGHT
    + SPACE_3;
const SIDEBAR_HEIGHT: f32 =
    nxe_egui::sidebar::HEAD + 3.0 * LINE_KNOB + 2.0 * SPACE_3 + nxe_egui::sidebar::FOOT;
pub const HEIGHT: f32 = if RIGHT_HEIGHT > SIDEBAR_HEIGHT {
    RIGHT_HEIGHT
} else {
    SIDEBAR_HEIGHT
};
const LINE_KNOB: f32 = nxe_egui::theme::LINE_LABEL + KNOB + nxe_egui::theme::LINE_VALUE;

pub fn create(params: Arc<DoublerParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (WINDOW_WIDTH, HEIGHT.ceil() as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !window(ui) {
        return;
    }
    let palette = &PALETTE;
    let time = ui.input(|input| input.time) as f32;
    // **truce-egui only paints when input or a parameter changes**, and the
    // analysis handoffs are neither: a window opened on a silent track would
    // never notice the playback start. So the window looks again every
    // `ANALYSIS_POLL`; while there is sound it runs at the smoothing's 30 Hz,
    // which is also what keeps `Humanize`'s drift moving.
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

    // The sidebar, with the macros.
    let correlation = ctx.params().analysis.correlation.read()[0];
    let correlation =
        smooth::smoothed(ui, ui.id().with("correlation"), &[correlation], smooth::TAU)[0];
    let figure = format!("{correlation:+.2}");
    let subject = Subject {
        name: "Correlation",
        figure: &figure,
        unit: "",
        position: (correlation + 1.0) / 2.0,
    };
    let mut column = ui.new_child(
        UiBuilder::new().max_rect(Rect::from_min_size(full.min, vec2(SIDEBAR_WIDTH, HEIGHT))),
    );
    column.spacing_mut().item_spacing = vec2(0.0, 0.0);
    sidebar(
        &mut column,
        palette,
        "Doubler",
        &subject,
        HEIGHT,
        |ui, ink| {
            let params = ctx.params();
            let rows = [
                [(params.detune.id(), "Detune"), (params.delay.id(), "Delay")],
                [
                    (params.spread.id(), "Spread"),
                    (params.humanize.id(), "Humanize"),
                ],
                [(params.mix.id(), "Mix"), (params.output.id(), "Output")],
            ];
            ui.spacing_mut().item_spacing = vec2(0.0, SPACE_3);
            for row in rows {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
                    for (id, name) in row {
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
    // inverted palette (`nxe_egui::surface`). Drawn here rather than through
    // `surface::figure` because the field takes drags as well as paints.
    let field_rect = Rect::from_min_size(pos2(left, y), vec2(RIGHT, FIGURE));
    let ink = palette.inverted();
    let painter = ui.painter_at(field_rect);
    painter.rect_filled(field_rect, 0.0, ink.ground.color());
    field::field(ui, ctx, &painter, field_rect, &ink, time);
    y += FIGURE + SPACE_3;

    let tab_id = ui.id().with("tab");
    let tab: usize = ui.data(|data| data.get_temp(tab_id)).unwrap_or(0);
    let mut tabs = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(160.0, segment::HEIGHT),
    )));
    if let Some(picked) = segment::segment(&mut tabs, palette, &["Tone", "Voices"], tab, 160.0) {
        ui.data_mut(|data| data.insert_temp(tab_id, picked));
    }
    y += segment::HEIGHT + SPACE_2;

    let body = Rect::from_min_size(
        pos2(left + SPACE_3, y),
        vec2(RIGHT - 2.0 * SPACE_3, table::HEIGHT),
    );
    if tab == 0 {
        tone_row(ui, ctx, palette, body);
    } else {
        table::table(ui, ctx, palette, body);
    }
}

/// The preset bar, and the two switches that change what the field shows.
fn top_strip(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_2, 0.0);
    let preset_width = rect.width() - VOICES_WIDTH - SOURCE_WIDTH - 2.0 * SPACE_2;
    preset_row(&mut row, ctx, palette, preset_width);

    let voices = params.voices.index() as usize;
    if let Some(picked) =
        segment::segment(&mut row, palette, &["2", "4", "8"], voices, VOICES_WIDTH)
    {
        bind::set_once(
            ctx,
            params.voices.id(),
            bind::normalize(ctx, params.voices.id(), picked as f64),
        );
    }
    let source = params.source.index() as usize;
    if let Some(picked) =
        segment::segment(&mut row, palette, &["Mono", "Stereo"], source, SOURCE_WIDTH)
    {
        bind::set_once(
            ctx,
            params.source.id(),
            bind::normalize(ctx, params.source.id(), picked as f64),
        );
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

/// The Tone tab: the shelves over the spectrum, `Tone Spread`, the meters.
fn tone_row(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let side = 140.0;
    tone::tone(
        ui,
        ctx,
        palette,
        Rect::from_min_max(rect.min, pos2(rect.right() - side - SPACE_4, rect.bottom())),
    );

    let mut knob_ui = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        pos2(rect.right() - side, rect.top()),
        vec2(64.0, block_height(KNOB)),
    )));
    let id = params.tone_spread.id();
    let control = knob(
        &mut knob_ui,
        palette,
        "Tone Spread",
        bind::normalized(ctx, id),
        &ctx.format_param(id),
        KNOB,
    );
    bind::control(ctx, id, &control);

    // In and out, left and right, read at the handoff's rate and smoothed.
    let peaks = smooth::smoothed(
        ui,
        ui.id().with("peaks"),
        &ctx.params().analysis.peaks.read().map(level_of),
        smooth::TAU,
    );
    let holds = ctx.params().analysis.holds.read().map(level_of);
    let meters = Rect::from_min_max(
        pos2(rect.right() - 56.0, rect.top() + 4.0),
        pos2(rect.right(), rect.bottom() - 4.0),
    );
    for (index, (peak, hold)) in peaks.iter().zip(holds).enumerate() {
        let x = meters.left() + index as f32 * 14.0 + if index >= 2 { 6.0 } else { 0.0 };
        meter::meter(
            ui.painter(),
            palette,
            Rect::from_min_size(pos2(x, meters.top()), vec2(6.0, meters.height())),
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
        let views = [
            include_str!("mod.rs"),
            include_str!("field.rs"),
            include_str!("tone.rs"),
            include_str!("table.rs"),
            include_str!("presets.rs"),
        ]
        .concat();
        let fields: Vec<&str> = PARAMS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("pub "))
            .filter(|rest| rest.contains(": FloatParam") || rest.contains(": EnumParam<"))
            .filter_map(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
            .collect();
        assert!(
            fields.len() >= 13,
            "the parameter scan found too little: {fields:?}"
        );
        for field in fields {
            // A voice's four are reached by number through `voice_ids`; `delay`
            // and `detune` are also macros and are found by name either way.
            let by_voice = matches!(field, "pan" | "gain") && views.contains("voice_ids(");
            assert!(
                views.contains(&format!(".{field}.id()")) || by_voice,
                "`{field}` has no control in the window"
            );
        }
    }
}
