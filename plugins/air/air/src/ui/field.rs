//! The figure, on the accent ground: what came in, what was added, and where
//! the layer is set to sit.
//!
//! **Only an additive plugin can draw this** (`ui.md`). The layer is a signal
//! of its own (`Engine::layer`), so it is drawn as a line over the source's
//! wash rather than inferred from what the source became.
//!
//! **The placement is drawn whether or not anything plays** (`v2-plan.md`,
//! decision 18): a block from the corner — `air_core::noise::corner_of`, the
//! function both halves are tuned from — up to the top of the axis.
//!
//! - `FOCUS` moves its left edge, `SURFACE` its height
//! - `BLEND` splits it: harmonics (vertical strokes, made from the source and
//!   landing on its harmonics) below, noise (grains) above
//! - `CHARACTER` sets how close the strokes and the grains sit — the knee on
//!   the harmonic side, the grain on the noise side
//! - `WIDTH` is the bar across the top, as wide as the noise is spread
//! - `FOLLOW` is the three detectors at the left: an outline at the depth set,
//!   filled by how far each is moving the layer right now
//!
//! Dragging the block sets `SURFACE`.

use super::bind::{self, Ctx};
use crate::analysis::{HIGH_HZ, LOW_HZ};
use air_core::follow::DETECTORS;
use air_core::noise::corner_of;
use egui::{Align2, Painter, Rect, Sense, Stroke, Ui, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use nxe_egui::{font, input, smooth};

const FLOOR_DB: f32 = -72.0;
const SAMPLE_RATE: f32 = 48_000.0;
const LABEL: f32 = 16.0;
const DETECTOR_NAMES: [&str; DETECTORS] = ["Env", "Brt", "Trn"];

/// Frequency → `0..=1` across the figure, log.
pub fn x_of(hz: f32) -> f32 {
    ((hz.max(LOW_HZ) / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

/// How far apart the strokes and grains sit, in pixels, at a `CHARACTER`.
pub fn pitch_of(character: f32) -> f32 {
    let character = if character.is_finite() {
        character.clamp(0.0, 1.0)
    } else {
        0.5
    };
    10.0 - 6.0 * character
}

fn spectrum(levels: &[f32]) -> Vec<(f32, f32)> {
    let count = (levels.len() - 1).max(1) as f32;
    levels
        .iter()
        .enumerate()
        .map(|(band, level)| {
            (
                band as f32 / count,
                ((20.0 * level.max(1e-9).log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0),
            )
        })
        .collect()
}

pub fn field(ui: &mut Ui, ctx: &Ctx, painter: &Painter, rect: Rect, ink: &Palette) {
    let params = ctx.params();
    let plot = Rect::from_min_max(
        rect.min + vec2(SPACE_3, LABEL + SPACE_1),
        rect.max - vec2(SPACE_3, LABEL),
    );

    // The placement, behind the signals.
    let surface = bind::plain(ctx, params.surface.id());
    let blend = bind::plain(ctx, params.blend.id());
    let character = bind::plain(ctx, params.character.id());
    let width = bind::plain(ctx, params.width.id());
    let corner = corner_of(bind::plain(ctx, params.focus.id()), SAMPLE_RATE);
    let left = plot.left() + plot.width() * x_of(corner);
    let block = Rect::from_min_max(
        pos2(left, plot.bottom() - plot.height() * surface),
        plot.right_bottom(),
    );
    painter.rect_filled(block, 0.0, ink.dim.color());
    let split = block.bottom() - block.height() * (1.0 - blend);
    let pitch = pitch_of(character);
    let stroke = Stroke::new(1.0, ink.deep.color());
    let mut x = block.left() + pitch / 2.0;
    while x < block.right() {
        painter.line_segment([pos2(x, split), pos2(x, block.bottom())], stroke);
        let mut y = block.top() + pitch / 2.0;
        while y < split {
            painter.circle_filled(pos2(x, y), 0.9, ink.deep.color());
            y += pitch;
        }
        x += pitch;
    }
    painter.line_segment(
        [pos2(block.left(), split), pos2(block.right(), split)],
        Stroke::new(1.0, ink.line.color()),
    );
    painter.line_segment(
        [block.left_top(), block.left_bottom()],
        Stroke::new(1.0, ink.ink.color()),
    );
    // Width, as a bar across the block's top.
    let half = block.width() / 2.0 * width;
    painter.line_segment(
        [
            pos2(block.center().x - half, block.top()),
            pos2(block.center().x + half, block.top()),
        ],
        Stroke::new(3.0, ink.ink.color()),
    );

    let id = params.surface.id();
    let response = ui.interact(
        block.union(Rect::from_min_max(
            pos2(left, plot.top()),
            plot.right_bottom(),
        )),
        ui.id().with("block"),
        Sense::click_and_drag(),
    );
    let gestures = input::read(&response, bind::normalized(ctx, id));
    bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(0.0)]);

    // What came in as a wash, what was added as a line over it.
    let dry = smooth::smoothed(
        ui,
        ui.id().with("dry"),
        &spectrum(&params.analysis.dry.read())
            .iter()
            .map(|(_, y)| *y)
            .collect::<Vec<_>>(),
        smooth::TAU,
    );
    let layer = smooth::smoothed(
        ui,
        ui.id().with("layer"),
        &spectrum(&params.analysis.layer.read())
            .iter()
            .map(|(_, y)| *y)
            .collect::<Vec<_>>(),
        smooth::TAU,
    );
    let count = (dry.len() - 1).max(1) as f32;
    let to_points = |levels: &[f32]| -> Vec<(f32, f32)> {
        levels
            .iter()
            .enumerate()
            .map(|(band, y)| (band as f32 / count, *y))
            .collect()
    };
    curve(
        painter,
        plot,
        &to_points(&dry),
        ink.subtle,
        Some(ink.dim),
        false,
    );
    let sounding = layer.iter().any(|level| *level > 0.02);
    curve(painter, plot, &to_points(&layer), ink.ink, None, sounding);

    // The detectors: depth set as an outline, movement now as the fill.
    let depths = params.display_depths();
    let moving = smooth::smoothed(
        ui,
        ui.id().with("follow"),
        &params.analysis.follow.read(),
        smooth::TAU,
    );
    for (index, name) in DETECTOR_NAMES.iter().enumerate() {
        let column = Rect::from_min_size(
            pos2(plot.left() + index as f32 * 22.0, plot.top()),
            vec2(10.0, 56.0),
        );
        let set = Rect::from_min_max(
            pos2(
                column.left(),
                column.bottom() - column.height() * depths[index],
            ),
            column.right_bottom(),
        );
        painter.rect_stroke(
            set,
            0.0,
            Stroke::new(1.0, ink.ink.color()),
            egui::StrokeKind::Inside,
        );
        let now = moving[index].clamp(0.0, 1.0) * depths[index];
        painter.rect_filled(
            Rect::from_min_max(
                pos2(column.left(), column.bottom() - column.height() * now),
                column.right_bottom(),
            ),
            0.0,
            ink.ink.color(),
        );
        painter.text(
            pos2(column.center().x, column.bottom() + SPACE_1),
            Align2::CENTER_TOP,
            name,
            font::sans(FONT_EYEBROW),
            ink.muted.color(),
        );
    }

    for (hz, label) in [(1_000.0, "1k"), (5_000.0, "5k"), (10_000.0, "10k")] {
        let x = plot.left() + plot.width() * x_of(hz);
        painter.text(
            pos2(x, rect.bottom() - SPACE_1),
            Align2::CENTER_BOTTOM,
            label,
            font::mono(FONT_EYEBROW),
            ink.subtle.color(),
        );
    }
    painter.text(
        pos2(left + SPACE_2, rect.top() + SPACE_1),
        Align2::LEFT_TOP,
        format!("{corner:.0} Hz"),
        font::mono(FONT_VALUE),
        ink.ink.color(),
    );
    let guard = params.analysis.guard.read()[0];
    if guard < -0.1 {
        painter.text(
            pos2(plot.right(), rect.top() + SPACE_1),
            Align2::RIGHT_TOP,
            format!("Guard {guard:.1} dB"),
            font::mono(FONT_VALUE),
            ink.ink.color(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// More `CHARACTER` packs the strokes and grains closer, and the pitch
    /// never reaches zero — a zero step would never leave the loop.
    #[test]
    fn character_packs_the_texture() {
        assert!(pitch_of(1.0) < pitch_of(0.0));
        assert!(pitch_of(1.0) >= 4.0);
        assert!(pitch_of(f32::NAN) >= 4.0);
    }
}
