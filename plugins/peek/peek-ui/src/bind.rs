//! What a widget reported, sent on to the host — the same few lines as each
//! Basic plugin's `bind.rs`, generic over the params so every Peek plugin
//! shares them.

use nxe_egui::knob::{Control, knob};
use nxe_egui::theme::Palette;
use truce_core::editor::PluginContext;
use truce_params::Params;

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

/// Sets a parameter to a plain value, through its own range.
pub fn set_plain<P: Params>(ctx: &PluginContext<P>, id: u32, plain: f64) {
    if let Some(info) = info(ctx, id) {
        set_once(ctx, id, info.range.normalize(plain));
    }
}

/// A knob bound to one parameter: drag, reset and typed values.
pub fn knob_for<P: Params>(
    ui: &mut egui::Ui,
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
