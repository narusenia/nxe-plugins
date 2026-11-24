//! The lower row's two tabs: a row of knobs, and beside it what they are doing
//! right now.
//!
//! **Follow** holds the three detectors' deviations from `FOLLOW` — zero is
//! "as the macro says" (`REQ-AIR-010`) — and how far each is moving the layer.
//! **Curve** holds the harmonic half's own numbers, which `CHARACTER`
//! deliberately does not reach, and the guard, with what the guard is pulling
//! and whether the layer's width survives a fold.

use super::bind::{self, Ctx};
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::knob::{block_height, knob};
use nxe_egui::readout::readout;
use nxe_egui::smooth;
use nxe_egui::theme::{Palette, SPACE_2, SPACE_4};

const KNOBS: f32 = 3.0 * 56.0 + 2.0 * SPACE_4;

pub fn follow(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect, size: f32) {
    let params = ctx.params();
    knobs(
        ui,
        ctx,
        palette,
        rect,
        size,
        &[
            (params.follow_envelope.id(), "Envelope"),
            (params.follow_brightness.id(), "Brightness"),
            (params.follow_transient.id(), "Transient"),
        ],
    );
    let moving = smooth::smoothed(
        ui,
        ui.id().with("moving"),
        &params.analysis.follow.read(),
        smooth::TAU,
    );
    figures(
        ui,
        palette,
        rect,
        &[
            ("Envelope", format!("{:.0}", moving[0] * 100.0), "%"),
            ("Brightness", format!("{:.0}", moving[1] * 100.0), "%"),
            ("Transient", format!("{:.0}", moving[2] * 100.0), "%"),
        ],
    );
}

pub fn curve(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect, size: f32) {
    let params = ctx.params();
    knobs(
        ui,
        ctx,
        palette,
        rect,
        size,
        &[
            (params.drive.id(), "Drive"),
            (params.bias.id(), "Bias"),
            (params.guard.id(), "Guard"),
        ],
    );
    let live = smooth::smoothed(
        ui,
        ui.id().with("live"),
        &[
            params.analysis.guard.read()[0],
            params.analysis.correlation.read()[0],
        ],
        smooth::TAU,
    );
    figures(
        ui,
        palette,
        rect,
        &[
            ("Guard", format!("{:+.1}", live[0]), "dB"),
            ("Layer Corr.", format!("{:+.2}", live[1]), ""),
        ],
    );
}

pub const HEIGHT: f32 = block_height(40.0);

fn knobs(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect, size: f32, ids: &[(u32, &str)]) {
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                rect.min,
                vec2(KNOBS, block_height(size)),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for (id, name) in ids {
        let control = knob(
            &mut row,
            palette,
            name,
            bind::normalized(ctx, *id),
            &ctx.format_param(*id),
            size,
        );
        bind::control(ctx, *id, &control);
    }
}

fn figures(ui: &mut Ui, palette: &Palette, rect: Rect, figures: &[(&str, String, &str)]) {
    let left = rect.left() + KNOBS + SPACE_4;
    let width = (rect.right() - left - SPACE_4 * (figures.len() - 1) as f32) / figures.len() as f32;
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(left, rect.top() + SPACE_2),
                vec2(rect.right() - left, nxe_egui::readout::HEIGHT),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for (name, figure, unit) in figures {
        readout(&mut row, palette, name, figure, unit, width);
    }
}
