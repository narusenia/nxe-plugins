//! The Tone view: the wet bus's shelves as a curve, over the wet bus's own
//! spectrum (`specifications/ui.md`, Filter View).
//!
//! **The shelves are edited on the curve**, by dragging the handle at each
//! shelf's frequency up or down — there are no Tone knobs. `Tone Spread` is a
//! knob beside it, and what it does shows as the bands under the curve, one
//! per live voice. At zero the filters are bypassed and no band is drawn.

use super::bind::{self, Ctx};
use crate::analysis::{HIGH_HZ, LOW_HZ};
use doubler_core::{MAX_VOICES, SHELF_HIGH_HZ, SHELF_LOW_HZ, spread_band, tone_response_db};
use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_1, SPACE_2};
use nxe_egui::{font, input, smooth};

/// The curve's dB range, top to bottom.
const RANGE_DB: f32 = 12.0;
/// The spectrum's floor.
const FLOOR_DB: f32 = -72.0;
const SAMPLE_RATE: f32 = 48_000.0;

/// Frequency → `0..=1` across the view, log.
pub fn x_of(hz: f32) -> f32 {
    ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

fn hz_at(x: f32) -> f32 {
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(x)
}

fn y_of_db(db: f32) -> f32 {
    (0.5 + db / (2.0 * RANGE_DB)).clamp(0.0, 1.0)
}

pub fn tone(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let painter = ui.painter_at(rect);
    let plot = Rect::from_min_max(rect.min + vec2(0.0, 14.0), rect.max - vec2(0.0, 12.0));

    let hair = Stroke::new(1.0, palette.line.color());
    painter.line_segment(
        [
            pos2(plot.left(), plot.center().y),
            pos2(plot.right(), plot.center().y),
        ],
        hair,
    );
    for (hz, label) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        let x = plot.left() + plot.width() * x_of(hz);
        painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], hair);
        painter.text(
            pos2(x, rect.bottom()),
            Align2::CENTER_BOTTOM,
            label,
            font::mono(FONT_EYEBROW),
            palette.subtle.color(),
        );
    }
    font::tracked(
        &painter,
        rect.left_top(),
        Align2::LEFT_TOP,
        "Tone",
        FONT_EYEBROW,
        palette.subtle.color(),
    );

    // The wet bus's spectrum, smoothed for display.
    let levels = params.analysis.spectrum.read();
    let bands: Vec<f32> = levels
        .iter()
        .map(|level| ((20.0 * level.max(1e-9).log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0))
        .collect();
    let bands = smooth::smoothed(ui, ui.id().with("spectrum"), &bands, smooth::TAU);
    let points: Vec<(f32, f32)> = bands
        .iter()
        .enumerate()
        .map(|(band, y)| (band as f32 / (bands.len() - 1).max(1) as f32, *y))
        .collect();
    curve(
        &painter,
        plot,
        &points,
        palette.accent.at(0.5),
        Some(palette.dim),
        false,
    );

    // Tone Spread's bands, one row per live voice, under the curve.
    let tone_spread = bind::plain(ctx, params.tone_spread.id());
    let live = doubler_core::Voices::count(params.voices.value().into()).min(MAX_VOICES);
    for index in 0..live {
        if let Some((high, low)) = spread_band(index, tone_spread) {
            let y = plot.bottom() - SPACE_1 - index as f32 * 3.0;
            painter.line_segment(
                [
                    pos2(plot.left() + plot.width() * x_of(high), y),
                    pos2(plot.left() + plot.width() * x_of(low), y),
                ],
                Stroke::new(1.0, palette.muted.at(0.6).color()),
            );
        }
    }

    // The shelves, from the same coefficients the audio goes through.
    let tone_lo = bind::plain(ctx, params.tone_lo.id());
    let tone_hi = bind::plain(ctx, params.tone_hi.id());
    let response: Vec<(f32, f32)> = (0..=48)
        .map(|step| {
            let x = step as f32 / 48.0;
            (
                x,
                y_of_db(tone_response_db(tone_lo, tone_hi, hz_at(x), SAMPLE_RATE)),
            )
        })
        .collect();
    curve(&painter, plot, &response, palette.ink, None, false);

    for (id, hz, db) in [
        (params.tone_lo.id(), SHELF_LOW_HZ, tone_lo),
        (params.tone_hi.id(), SHELF_HIGH_HZ, tone_hi),
    ] {
        let at = pos2(
            plot.left() + plot.width() * x_of(hz),
            plot.bottom() - plot.height() * y_of_db(db),
        );
        let response = ui.interact(
            Rect::from_center_size(at, vec2(20.0, 20.0)),
            ui.id().with(("shelf", id)),
            Sense::click_and_drag(),
        );
        let gestures = input::read(&response, bind::normalized(ctx, id));
        bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(0.0)]);
        let colour = if response.hovered() || response.dragged() {
            palette.ink
        } else {
            palette.accent
        };
        painter.rect_filled(
            Rect::from_center_size(at, vec2(7.0, 7.0)),
            0.0,
            colour.color(),
        );
        painter.text(
            at + vec2(0.0, -SPACE_2),
            Align2::CENTER_BOTTOM,
            ctx.format_param(id),
            font::mono(FONT_EYEBROW),
            palette.muted.color(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_runs_twenty_to_twenty_thousand() {
        assert!(x_of(LOW_HZ).abs() < 1e-6);
        assert!((x_of(HIGH_HZ) - 1.0).abs() < 1e-6);
        assert!((hz_at(x_of(1_000.0)) - 1_000.0).abs() < 0.5);
    }
}
