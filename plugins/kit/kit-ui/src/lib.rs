//! What every Kit's window shares (`nonlinear-kit-plan.md`, decision 4): the
//! binding to truce, the preset bar over a table of factory presets, and the
//! frame — the L, the preset bar at the top right, the knobs in the band.
//!
//! A Kit's own `ui.rs` declares its knobs and presets and draws its figure;
//! everything else is here.

use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::kit::{self, kit};
use nxe_egui::knob::{Control, knob};
use nxe_egui::preset::{PresetAction, preset_bar, step};
use nxe_egui::theme::{Palette, SPACE_2, SPACE_3, SPACE_4};
use truce_core::editor::PluginContext;
use truce_core::presets::{PresetRef, PresetScope, PresetStore};
use truce_core::state::hash_plugin_id;
use truce_params::Params;
use truce_utils::preset::PresetMeta;

/// The knobs' size in the band.
const KNOB: f32 = 28.0;
const PRESET_WIDTH: f32 = 180.0;

pub fn normalized<P: Params>(ctx: &PluginContext<P>, id: u32) -> f32 {
    ctx.params().get_normalized(id).unwrap_or(0.0) as f32
}

/// One edit, begun and ended.
pub fn set_once<P: Params>(ctx: &PluginContext<P>, id: u32, normalized: f64) {
    ctx.begin_edit(id);
    ctx.set_param(id, normalized.clamp(0.0, 1.0));
    ctx.end_edit(id);
}

fn info<P: Params>(ctx: &PluginContext<P>, id: u32) -> Option<truce_params::ParamInfo> {
    ctx.params()
        .param_infos()
        .into_iter()
        .find(|info| info.id == id)
}

/// A parameter's plain value, through its own range.
pub fn plain<P: Params>(ctx: &PluginContext<P>, id: u32) -> f64 {
    info(ctx, id).map_or(0.0, |info| {
        info.range.denormalize(f64::from(normalized(ctx, id)))
    })
}

/// Sets a parameter to a plain value, through its own range.
pub fn set_plain<P: Params>(ctx: &PluginContext<P>, id: u32, plain: f64) {
    if let Some(info) = info(ctx, id) {
        set_once(ctx, id, info.range.normalize(plain));
    }
}

/// A knob bound to one parameter: drag, reset and typed values.
pub fn knob_for<P: Params>(
    ui: &mut Ui,
    ctx: &PluginContext<P>,
    palette: &Palette,
    id: u32,
    name: &str,
    size: f32,
) {
    let control = knob(
        ui,
        palette,
        name,
        normalized(ctx, id),
        &ctx.format_param(id),
        size,
    );
    apply(ctx, id, &control);
}

fn apply<P: Params>(ctx: &PluginContext<P>, id: u32, control: &Control) {
    let gestures = &control.gestures;
    if gestures.begin {
        ctx.begin_edit(id);
    }
    if let Some(value) = gestures.change {
        ctx.set_param(id, f64::from(value));
    }
    if gestures.end {
        ctx.end_edit(id);
    }
    if gestures.reset
        && let Some(info) = info(ctx, id)
    {
        set_once(ctx, id, info.range.normalize(info.default_plain));
    }
    if let Some(typed) = control.typed.as_deref()
        && let Some(plain) = ctx.params().parse_value(id, typed.trim())
    {
        set_plain(ctx, id, plain);
    }
}

/// One factory preset: a name and a plain value per knob, in the knobs'
/// order. **The first is the defaults**, so "Default" means what it says.
pub struct Factory {
    pub name: &'static str,
    pub values: &'static [f64],
}

/// A Kit, as its window needs to know it.
pub struct Kit<'a> {
    /// The wordmark: the bare name.
    pub name: &'a str,
    /// The strip: the series the Kit is cut from (`DESIGN KIT`).
    pub label: &'a str,
    /// The name the host lists, for the user preset library (`Fold`).
    pub product: &'a str,
    /// The reverse-DNS id the state hash comes from (`com.nxe.fold`), so a
    /// preset saved by one format loads in another.
    pub plugin_id: &'a str,
    pub palette: &'a Palette,
    /// `(id, name)` per knob, left to right.
    pub knobs: &'a [(u32, &'a str)],
    pub factory: &'a [Factory],
}

impl Kit<'_> {
    fn store(&self) -> PresetStore {
        PresetStore::new("NXE", self.product, hash_plugin_id(self.plugin_id), None)
    }

    fn values_of(&self, preset: &Factory) -> Vec<(u32, f64)> {
        self.knobs
            .iter()
            .map(|(id, _)| *id)
            .zip(preset.values.iter().copied())
            .collect()
    }
}

/// Draws the frame, the preset bar and the knobs, and returns the figure's
/// area: under the wordmark, inside the margins.
pub fn window<P: Params>(ui: &mut Ui, ctx: &PluginContext<P>, kit_: &Kit) -> Rect {
    // truce-egui paints only on input or a parameter change, and the meters
    // behind the figure are neither: ask again every frame (`peek_ui::heartbeat`).
    ui.ctx().request_repaint_after(nxe_egui::smooth::FRAME);
    let areas = kit(ui, kit_.palette, kit_.name, kit_.label);
    let top = areas.figure.top() + SPACE_2;
    let mut bar = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        pos2(areas.figure.right() - PRESET_WIDTH - SPACE_3, top),
        vec2(PRESET_WIDTH, nxe_egui::preset::HEIGHT),
    )));
    presets(&mut bar, ctx, kit_);

    let ink = kit_.palette.inverted();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(areas.band.shrink2(vec2(SPACE_4, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4 * 2.0, 0.0);
    for (id, name) in kit_.knobs {
        knob_for(&mut row, ctx, &ink, *id, name, KNOB);
    }

    Rect::from_min_max(
        pos2(
            areas.figure.left() + SPACE_3,
            areas.figure.top() + kit::HEAD + SPACE_2,
        ),
        areas.figure.max - vec2(SPACE_3, SPACE_3),
    )
}

/// The preset bar: the factory presets, then the user's.
fn presets<P: Params>(ui: &mut Ui, ctx: &PluginContext<P>, kit_: &Kit) {
    let store = kit_.store();
    let user_id = ui.id().with("user-presets");
    let user: Vec<(String, String)> =
        match ui.data(|data| data.get_temp::<Vec<(String, String)>>(user_id)) {
            Some(user) => user,
            None => {
                let user: Vec<(String, String)> = user_presets(&store)
                    .into_iter()
                    .map(|preset| (preset.name, preset.uri))
                    .collect();
                ui.data_mut(|data| data.insert_temp(user_id, user.clone()));
                user
            }
        };
    let mut names: Vec<String> = kit_
        .factory
        .iter()
        .map(|preset| preset.name.to_owned())
        .collect();
    names.extend(user.iter().map(|(name, _)| name.clone()));

    let loaded_id = ui.id().with("loaded-user-preset");
    let current = kit_
        .factory
        .iter()
        .position(|preset| {
            kit_.values_of(preset)
                .iter()
                // Relative: 5000 Hz through a log range comes back a hundredth off.
                .all(|(id, value)| (plain(ctx, *id) - value).abs() < 1e-3 * value.abs().max(1.0))
        })
        .or_else(|| ui.data(|data| data.get_temp::<usize>(loaded_id)));

    let load = |ui: &mut Ui, index: usize| {
        if let Some(preset) = kit_.factory.get(index) {
            for (id, value) in kit_.values_of(preset) {
                set_plain(ctx, id, value);
            }
            ui.data_mut(|data| data.remove::<usize>(loaded_id));
        } else if let Some((_, uri)) = user.get(index - kit_.factory.len())
            && let Some(preset) = store.find(uri)
            && let Ok(state) = store.load(&preset.uri)
        {
            for (id, value) in state.params {
                set_plain(ctx, id, value);
            }
            ui.data_mut(|data| data.insert_temp(loaded_id, index));
        }
    };
    let width = ui.max_rect().width();
    match preset_bar(ui, kit_.palette, &names, current, false, width) {
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
        Some(PresetAction::Save(name)) if save(ctx, &store, &name).is_ok() => {
            ui.data_mut(|data| data.remove::<Vec<(String, String)>>(user_id));
        }
        _ => {}
    }
}

fn user_presets(store: &PresetStore) -> Vec<PresetRef> {
    store
        .enumerate()
        .into_iter()
        .filter(|preset| matches!(preset.scope, PresetScope::User))
        .collect()
}

/// Saves every parameter under `name`, as plain values.
fn save<P: Params>(ctx: &PluginContext<P>, store: &PresetStore, name: &str) -> Result<(), String> {
    let (ids, values) = ctx.params().collect_values();
    let pairs: Vec<(u32, f64)> = ids.into_iter().zip(values).collect();
    let meta = PresetMeta {
        name: name.to_owned(),
        category: "User".to_owned(),
        ..PresetMeta::default()
    };
    store
        .save(meta, &pairs, &[])
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

/// What a Kit's preset test checks: each factory preset has a value per
/// knob, inside the knob's range, and the first is the defaults.
pub fn check_factory<P: Params>(params: &P, knobs: &[u32], factory: &[Factory]) {
    let infos = params.param_infos();
    for (index, preset) in factory.iter().enumerate() {
        assert_eq!(preset.values.len(), knobs.len(), "{}", preset.name);
        for (id, plain) in knobs.iter().zip(preset.values) {
            let info = infos.iter().find(|info| info.id == *id).unwrap();
            let back = info.range.denormalize(info.range.normalize(*plain));
            assert!(
                (back - plain).abs() < 1e-6,
                "{} sets {} to {plain}, out of range",
                preset.name,
                info.name
            );
            if index == 0 {
                assert!(
                    (info.default_plain - plain).abs() < 1e-6,
                    "{} defaults to {}, not {plain}",
                    info.name,
                    info.default_plain
                );
            }
        }
    }
}

/// The figure every nonlinear Kit draws: the transfer curve, input across and
/// output up, both `−range..=range`, with the part of the input the signal
/// reaches shaded.
pub struct Transfer {
    rect: Rect,
    range: f32,
}

impl Transfer {
    /// Draws the grid, the identity line and the reach (`0..=1`, linear).
    pub fn new(ui: &Ui, palette: &Palette, rect: Rect, reach: f32, range: f32) -> Self {
        let transfer = Self { rect, range };
        let painter = ui.painter_at(rect);
        let reach = reach.clamp(0.0, range);
        if reach > 0.0 {
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(transfer.x(-reach), rect.top()),
                    pos2(transfer.x(reach), rect.bottom()),
                ),
                0.0,
                palette.dim.color(),
            );
        }
        let line = egui::Stroke::new(1.0, palette.line.color());
        let centre = rect.center();
        painter.line_segment(
            [pos2(rect.left(), centre.y), pos2(rect.right(), centre.y)],
            line,
        );
        painter.line_segment(
            [pos2(centre.x, rect.top()), pos2(centre.x, rect.bottom())],
            line,
        );
        for edge in [-1.0, 1.0] {
            let faint = egui::Stroke::new(1.0, palette.line.at(0.5).color());
            painter.line_segment(
                [
                    pos2(transfer.x(edge), rect.top()),
                    pos2(transfer.x(edge), rect.bottom()),
                ],
                faint,
            );
            painter.line_segment(
                [
                    pos2(rect.left(), transfer.y(edge)),
                    pos2(rect.right(), transfer.y(edge)),
                ],
                faint,
            );
        }
        transfer.curve(ui, -range, range, palette.line, 1.0, |x| x);
        transfer
    }

    fn x(&self, value: f32) -> f32 {
        self.rect.center().x + value / self.range * self.rect.width() * 0.5
    }

    fn y(&self, value: f32) -> f32 {
        self.rect.center().y - value / self.range * self.rect.height() * 0.5
    }

    /// Draws `f` over the input `from..=to`.
    pub fn curve(
        &self,
        ui: &Ui,
        from: f32,
        to: f32,
        colour: nxe_egui::theme::Token,
        width: f32,
        f: impl Fn(f32) -> f32,
    ) {
        const STEPS: usize = 240;
        let points: Vec<egui::Pos2> = (0..=STEPS)
            .map(|step| {
                let x = from + (to - from) * step as f32 / STEPS as f32;
                pos2(self.x(x), self.y(f(x).clamp(-self.range, self.range)))
            })
            .collect();
        ui.painter_at(self.rect).add(egui::Shape::line(
            points,
            egui::Stroke::new(width, colour.color()),
        ));
    }
}

/// The figure the spectral Kits draw: frequency across on a log axis,
/// 20 Hz to 20 kHz, and something per frequency up — another frequency on the
/// same log axis ([`Frequency::hz`]) or a linear amount from zero to `top`.
pub struct Frequency {
    rect: Rect,
    /// `None`: the vertical axis is frequency too.
    top: Option<f32>,
}

const LOW_HZ: f32 = 20.0;
const HIGH_HZ: f32 = 20_000.0;

fn log_position(hz: f32) -> f32 {
    ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

impl Frequency {
    /// Up is an amount, `0..=top`, labelled with `label` at the top.
    pub fn amount(ui: &Ui, palette: &Palette, rect: Rect, top: f32, label: &str) -> Self {
        let plot = Self {
            rect,
            top: Some(top.max(f32::EPSILON)),
        };
        plot.grid(ui, palette);
        ui.painter_at(rect.expand(2.0)).text(
            rect.left_top() + vec2(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            label,
            nxe_egui::font::mono(nxe_egui::theme::FONT_VALUE),
            palette.muted.color(),
        );
        plot
    }

    /// Up is frequency too, with the identity drawn faint.
    pub fn hz(ui: &Ui, palette: &Palette, rect: Rect) -> Self {
        let plot = Self { rect, top: None };
        plot.grid(ui, palette);
        plot.curve(ui, palette.line, 1.0, |hz| hz);
        plot
    }

    fn grid(&self, ui: &Ui, palette: &Palette) {
        let painter = ui.painter_at(self.rect.expand(14.0));
        let line = egui::Stroke::new(1.0, palette.line.color());
        painter.rect_stroke(self.rect, 0.0, line, egui::StrokeKind::Inside);
        for (hz, text) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
            let x = self.x(hz);
            painter.line_segment(
                [pos2(x, self.rect.top()), pos2(x, self.rect.bottom())],
                line,
            );
            painter.text(
                pos2(x + 3.0, self.rect.bottom() - 2.0),
                egui::Align2::LEFT_BOTTOM,
                text,
                nxe_egui::font::mono(nxe_egui::theme::FONT_VALUE),
                palette.subtle.color(),
            );
            if self.top.is_none() {
                let y = self.y(hz);
                painter.line_segment(
                    [pos2(self.rect.left(), y), pos2(self.rect.right(), y)],
                    line,
                );
            }
        }
    }

    fn x(&self, hz: f32) -> f32 {
        self.rect.left() + log_position(hz) * self.rect.width()
    }

    fn y(&self, value: f32) -> f32 {
        let at = match self.top {
            Some(top) => (value / top).clamp(0.0, 1.0),
            None => log_position(value),
        };
        self.rect.bottom() - at * self.rect.height()
    }

    /// Shades `low..=high` across the whole height.
    pub fn band(&self, ui: &Ui, palette: &Palette, low: f32, high: f32) {
        let (low, high) = (low.min(high), low.max(high));
        ui.painter_at(self.rect).rect_filled(
            Rect::from_min_max(
                pos2(self.x(low), self.rect.top()),
                pos2(self.x(high), self.rect.bottom()),
            ),
            0.0,
            palette.dim.color(),
        );
    }

    /// Draws `f` across the whole frequency range.
    pub fn curve(
        &self,
        ui: &Ui,
        colour: nxe_egui::theme::Token,
        width: f32,
        f: impl Fn(f32) -> f32,
    ) {
        const STEPS: usize = 240;
        let points: Vec<egui::Pos2> = (0..=STEPS)
            .map(|step| {
                let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(step as f32 / STEPS as f32);
                pos2(self.x(hz), self.y(f(hz)))
            })
            .collect();
        ui.painter_at(self.rect.expand(1.0)).add(egui::Shape::line(
            points,
            egui::Stroke::new(width, colour.color()),
        ));
    }
}

impl Frequency {
    /// Fills under `f` across the whole frequency range.
    pub fn fill(&self, ui: &Ui, colour: nxe_egui::theme::Token, f: impl Fn(f32) -> f32) {
        const STEPS: usize = 160;
        let painter = ui.painter_at(self.rect);
        let width = self.rect.width() / STEPS as f32;
        for step in 0..STEPS {
            let at = (step as f32 + 0.5) / STEPS as f32;
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(at);
            let x = self.rect.left() + step as f32 * width;
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(x, self.y(f(hz))),
                    pos2(x + width + 0.5, self.rect.bottom()),
                ),
                0.0,
                colour.color(),
            );
        }
    }
}
