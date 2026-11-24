//! The Advanced tab: the two guards and `EMOTION`, and what the guards are
//! holding back right now.
//!
//! **The guards are exposed rather than hidden** (`REQ-VEL-006`): fully
//! automatic protection means a user who turns `AIR` up and hears nothing has
//! no way to find out why. The figure sinks the region a guard holds back; the
//! readouts here say by how much, which is what decides whether the setting is
//! wrong or the material is.
//!
//! **`EMOTION` lives here, and zero is exactly static** (`REQ-VEL-008`): it is
//! the plugin's differentiation, and nobody can judge it without switching it
//! off to compare.

use super::bind::{self, Ctx};
use egui::{Rect, Ui, UiBuilder, pos2, vec2};
use nxe_egui::knob::{block_height, knob};
use nxe_egui::readout::readout;
use nxe_egui::smooth;
use nxe_egui::theme::{Palette, SPACE_2, SPACE_4};

pub const WIDTH: f32 = 3.0 * 56.0 + 2.0 * SPACE_4;

pub fn advanced(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect, knob_size: f32) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                rect.min,
                vec2(WIDTH, block_height(knob_size)),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for (id, name) in [
        (params.guard_harsh.id(), "Harsh Guard"),
        (params.guard_sib.id(), "Sib Guard"),
        (params.emotion.id(), "Emotion"),
    ] {
        let control = knob(
            &mut row,
            palette,
            name,
            bind::normalized(ctx, id),
            &ctx.format_param(id),
            knob_size,
        );
        bind::control(ctx, id, &control);
    }

    let guards = params.analysis.guards.read();
    let guards = smooth::smoothed(ui, ui.id().with("held"), &guards, smooth::TAU);
    let width = (rect.width() - WIDTH - 2.0 * SPACE_4) / 2.0;
    let mut figures = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(rect.left() + WIDTH + SPACE_4, rect.top() + SPACE_2),
                vec2(rect.width() - WIDTH - SPACE_4, nxe_egui::readout::HEIGHT),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    figures.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for (name, held) in [("Harsh", guards[0]), ("Sib", guards[1])] {
        readout(
            &mut figures,
            palette,
            name,
            &format!("{held:+.1}"),
            "dB",
            width,
        );
    }
}
