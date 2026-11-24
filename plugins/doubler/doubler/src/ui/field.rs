//! The Voice Field: the figure the window is about (`specifications/ui.md`).
//!
//! A half disc seen from the listener. **Angle is where a voice sits, radius is
//! how loud it is** — and both are the voice's own effective values, so every
//! macro that matters moves the picture (decision 18):
//!
//! - `Spread` opens and closes the fan of dots (`pan_for`, the DSP's formula)
//! - `Delay` sets each dot's size — the effective delay, macro × shape
//! - `Detune` draws a bar through each dot, as wide as the voice is detuned
//! - `Humanize` makes the dots drift, as far as the wobble reaches
//! - `Mix` is the dry's ▲, nearer the centre as the dry goes down
//!
//! Behind the dots, the output's energy by direction (`analysis.pan`), smoothed
//! for display (`nxe_egui::smooth`).
//!
//! Dragging a dot writes its `Pan` and `Gain`; its pair follows as a mirror
//! image (`REQ-DBL-014`), which keeps the image from leaning. Double-click puts
//! the voice back to its default shape.

use super::bind::{self, Ctx};
use crate::analysis::PAN_BINS;
use doubler_core::{MAX_VOICES, Source, mirror_partner, pan_for, pan_shape_for};
use egui::{Align2, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use nxe_egui::font;
use nxe_egui::smooth;
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_3};
use std::f32::consts::PI;

/// The largest dot, at the longest delay.
const DOT: f32 = 9.0;
/// The detune bar at the macro's full 50 cents.
const DETUNE_BAR: f32 = 64.0;
/// How far a dot drifts at full `Humanize`, in points.
const DRIFT: f32 = 4.0;

struct Geometry {
    origin: Pos2,
    radius: f32,
}

impl Geometry {
    fn of(rect: Rect) -> Self {
        let radius = (rect.height() - 2.0 * SPACE_3 - 14.0).min(rect.width() / 2.0 - 40.0);
        Self {
            origin: pos2(rect.center().x, rect.bottom() - SPACE_3 - 14.0),
            radius,
        }
    }

    /// `pan` in `-1..=1` (left to right), `level` in `0..=1` (centre to rim).
    fn at(&self, pan: f32, level: f32) -> Pos2 {
        let angle = PI * (1.0 - (pan.clamp(-1.0, 1.0) + 1.0) / 2.0);
        self.origin + self.radius * level.clamp(0.0, 1.0) * vec2(angle.cos(), -angle.sin())
    }

    /// The inverse of [`Self::at`].
    fn of_point(&self, point: Pos2) -> (f32, f32) {
        let offset = point - self.origin;
        let angle = (-offset.y).max(0.0).atan2(offset.x);
        let pan = 1.0 - 2.0 * angle / PI;
        (
            pan.clamp(-1.0, 1.0),
            (offset.length() / self.radius).clamp(0.0, 1.0),
        )
    }
}

/// Draws the field into `rect` (already the accent ground) and handles drags.
pub fn field(ui: &mut Ui, ctx: &Ctx, painter: &Painter, rect: Rect, ink: &Palette, time: f32) {
    let params = ctx.params();
    let geometry = Geometry::of(rect);
    let source: Source = params.source.value().into();
    let spread = bind::plain(ctx, params.spread.id());
    let live = doubler_core::Voices::count(params.voices.value().into()).min(MAX_VOICES);

    // The rim, the centre line and the half-way ring: the grid this is read on.
    let hair = Stroke::new(1.0, ink.line.color());
    let rim: Vec<Pos2> = (0..=64)
        .map(|step| geometry.at(-1.0 + 2.0 * step as f32 / 64.0, 1.0))
        .collect();
    let half: Vec<Pos2> = rim
        .iter()
        .map(|p| geometry.origin + (*p - geometry.origin) * 0.5)
        .collect();
    painter.add(Shape::line(rim, hair));
    painter.add(Shape::line(half, hair));
    painter.line_segment([geometry.at(-1.0, 1.0), geometry.at(1.0, 1.0)], hair);
    painter.line_segment([geometry.origin, geometry.at(0.0, 1.0)], hair);
    for (text, pan) in [("L", -1.0), ("C", 0.0), ("R", 1.0)] {
        let at = geometry.at(pan, 1.0) + vec2(0.0, if pan == 0.0 { -10.0 } else { 10.0 });
        painter.text(
            at,
            Align2::CENTER_CENTER,
            text,
            font::sans(FONT_EYEBROW),
            ink.muted.color(),
        );
    }

    // The energy by direction, as wedges whose depth is the level.
    let energy = smooth::smoothed(
        ui,
        ui.id().with("pan-energy"),
        &params.analysis.pan.read(),
        smooth::TAU,
    );
    let peak = energy.iter().copied().fold(1e-6f32, f32::max);
    for (bin, level) in energy.iter().enumerate() {
        let from = -1.0 + 2.0 * bin as f32 / PAN_BINS as f32;
        let to = from + 2.0 / PAN_BINS as f32;
        let depth = (level / peak).sqrt().clamp(0.0, 1.0);
        if depth < 0.02 {
            continue;
        }
        painter.add(Shape::convex_polygon(
            vec![
                geometry.origin,
                geometry.at(from, depth),
                geometry.at(to, depth),
            ],
            ink.ink.at(0.10).color(),
            Stroke::NONE,
        ));
    }

    // The dry, one ▲ per source. Dragging it toward the centre raises `Mix`.
    let dry = 1.0 - bind::normalized(ctx, params.mix.id());
    let anchors = match source {
        Source::MonoSum => 1,
        Source::TrueStereo => 2,
    };
    for anchor in 0..anchors {
        let at = geometry.at(pan_for(source, spread, 0.0, anchor), dry);
        let response = ui.interact(
            Rect::from_center_size(at, Vec2::splat(18.0)),
            ui.id().with(("dry", anchor)),
            Sense::click_and_drag(),
        );
        triangle(
            painter,
            at,
            if response.hovered() {
                ink.ink
            } else {
                ink.ink.at(0.8)
            },
        );
        let mix = params.mix.id();
        if response.drag_started() {
            ctx.begin_edit(mix);
        }
        if let Some(pointer) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let (_, level) = geometry.of_point(pointer);
            ctx.set_param(mix, f64::from(1.0 - level));
        }
        if response.drag_stopped() {
            ctx.end_edit(mix);
        }
        if response.double_clicked() {
            bind::set_once(ctx, params.mix.id(), bind::default_of(ctx, params.mix.id()));
        }
    }

    // The voices.
    let delay_macro = bind::normalized(ctx, params.delay.id());
    let detune_macro = bind::normalized(ctx, params.detune.id());
    let humanize = bind::normalized(ctx, params.humanize.id());
    for index in 0..live {
        let [delay, detune, pan, gain] = params.voice_ids(index);
        let shape_pan = bind::plain(ctx, pan);
        let level = bind::normalized(ctx, gain);
        let drift = DRIFT
            * humanize
            * vec2(
                (time * 1.7 + index as f32 * 2.1).sin(),
                (time * 1.3 + index as f32 * 1.4).cos(),
            );
        let at = geometry.at(pan_for(source, spread, shape_pan, index), level) + drift;
        let size = 3.0 + DOT * delay_macro * bind::normalized(ctx, delay);
        // A step along the accent per pair, so the two a mirrored drag moves
        // together are findable among eight dots.
        let tint = ink.ink.at(1.0 - 0.15 * (index / 2) as f32);

        let width = DETUNE_BAR * detune_macro * bind::plain(ctx, detune).abs();
        if width > 0.5 {
            painter.line_segment(
                [at - vec2(width / 2.0, 0.0), at + vec2(width / 2.0, 0.0)],
                Stroke::new(1.5, tint.color()),
            );
        }

        let response = ui.interact(
            Rect::from_center_size(at, Vec2::splat((size * 2.0).max(16.0))),
            ui.id().with(("voice", index)),
            Sense::click_and_drag(),
        );
        painter.circle_filled(at, size, tint.color());
        if response.hovered() || response.dragged() {
            painter.circle_stroke(at, size + 3.0, Stroke::new(1.0, ink.ink.color()));
        }

        let partner = mirror_partner(index);
        let [_, _, partner_pan, partner_gain] = params.voice_ids(partner);
        let ids = [pan, gain, partner_pan, partner_gain];
        if response.drag_started() {
            ids.iter().for_each(|id| ctx.begin_edit(*id));
        }
        if let Some(pointer) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let (target, level) = geometry.of_point(pointer);
            // Under MonoSum with no Spread every voice sits in the centre and
            // nothing sideways can move it; the level still can.
            if let Some(shape) = pan_shape_for(source, spread, target, index) {
                ctx.set_param(pan, bind::normalize(ctx, pan, f64::from(shape)));
            }
            if let Some(shape) = pan_shape_for(source, spread, -target, partner) {
                ctx.set_param(
                    partner_pan,
                    bind::normalize(ctx, partner_pan, f64::from(shape)),
                );
            }
            ctx.set_param(gain, f64::from(level));
            ctx.set_param(partner_gain, f64::from(level));
        }
        if response.drag_stopped() {
            ids.iter().for_each(|id| ctx.end_edit(*id));
        }
        if response.double_clicked() {
            for id in [delay, detune, pan, gain] {
                bind::set_once(ctx, id, bind::default_of(ctx, id));
            }
        }
    }

    font::tracked(
        painter,
        rect.left_top() + vec2(SPACE_3, SPACE_3),
        Align2::LEFT_TOP,
        "Voices",
        FONT_EYEBROW,
        ink.muted.color(),
    );
}

fn triangle(painter: &Painter, at: Pos2, colour: nxe_egui::theme::Token) {
    painter.add(Shape::convex_polygon(
        vec![
            at + vec2(0.0, -6.0),
            at + vec2(6.0, 5.0),
            at + vec2(-6.0, 5.0),
        ],
        colour.color(),
        Stroke::NONE,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_point_maps_back_to_where_it_came_from() {
        let geometry = Geometry::of(Rect::from_min_size(Pos2::ZERO, vec2(660.0, 220.0)));
        for pan in [-1.0, -0.4, 0.0, 0.7, 1.0] {
            for level in [0.1, 0.5, 1.0] {
                let (back, back_level) = geometry.of_point(geometry.at(pan, level));
                assert!(
                    (back - pan).abs() < 1e-3 && (back_level - level).abs() < 1e-3,
                    "{pan} {level} → {back} {back_level}"
                );
            }
        }
    }
}
