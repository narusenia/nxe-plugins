//! What a widget reported, sent on to the host.
//!
//! `nxe-egui` knows nothing about truce (`crates/nxe-egui/README.md`); this is
//! the only place the two meet. Every edit is bracketed by `begin_edit` and
//! `end_edit`, so a host records a drag as one automation move — and a reset or
//! a typed value as one too.

use crate::params::DioramaParams;
use nxe_egui::input::Gestures;
use nxe_egui::knob::Control;
use truce_core::editor::PluginContext;
use truce_params::Params;

pub type Ctx = PluginContext<DioramaParams>;

/// The parameter's normalized value, `0..=1`.
pub fn normalized(ctx: &Ctx, id: u32) -> f32 {
    ctx.params().get_normalized(id).unwrap_or(0.0) as f32
}

/// The parameter's plain value, in its own unit.
pub fn plain(ctx: &Ctx, id: u32) -> f32 {
    ctx.params().get_plain(id).unwrap_or(0.0) as f32
}

/// One edit, begun and ended: a reset, a typed value, a preset.
pub fn set_once(ctx: &Ctx, id: u32, normalized: f64) {
    ctx.begin_edit(id);
    ctx.set_param(id, normalized.clamp(0.0, 1.0));
    ctx.end_edit(id);
}

/// The default, normalized.
pub fn default_of(ctx: &Ctx, id: u32) -> f64 {
    ctx.params()
        .param_infos()
        .into_iter()
        .find(|info| info.id == id)
        .map_or(0.0, |info| info.range.normalize(info.default_plain))
}

/// A plain value, normalized through the parameter's own range.
pub fn normalize(ctx: &Ctx, id: u32, plain: f64) -> f64 {
    ctx.params()
        .param_infos()
        .into_iter()
        .find(|info| info.id == id)
        .map_or(0.0, |info| info.range.normalize(plain))
}

/// Sends what a value control reported. The drag's own gestures go straight
/// through; a reset and a typed value are each one bracketed edit.
pub fn gestures(ctx: &Ctx, ids: &[u32], gestures: &Gestures, values: &[f32]) {
    for (id, value) in ids.iter().zip(values) {
        if gestures.begin {
            ctx.begin_edit(*id);
        }
        if gestures.change.is_some() {
            ctx.set_param(*id, f64::from(*value));
        }
        if gestures.end {
            ctx.end_edit(*id);
        }
        if gestures.reset {
            set_once(ctx, *id, default_of(ctx, *id));
        }
    }
}

/// A knob or a bar bound to one parameter.
pub fn control(ctx: &Ctx, id: u32, control: &Control) {
    if let Some(value) = control.gestures.change {
        gestures(ctx, &[id], &control.gestures, &[value]);
    } else {
        gestures(ctx, &[id], &control.gestures, &[0.0]);
    }
    if let Some(typed) = control.typed.as_deref() {
        // The parameter parses its own text, units and all ("35 %", "-3 dB").
        if let Some(plain) = ctx.params().parse_value(id, typed.trim()) {
            set_once(ctx, id, normalize(ctx, id, plain));
        }
    }
}
