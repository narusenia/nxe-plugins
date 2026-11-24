//! The Band Field: the five bands as regions on a log frequency axis, on the
//! accent ground.
//!
//! **How far each band is being moved is the subject** (`ui.md`). A split
//! topology has nothing "added" to show; what a multiband compressor is asked
//! about is how far each band is lifted or pulled. So unity is a line across
//! the middle, and each region carries two things either side of it:
//!
//! - **the reach** — a pale span from what the band does to a loud signal to
//!   what it does to a quiet one, from `sparkleur_core::engine::transfer_db`,
//!   the gain computer the audio runs. This is what makes the macros visible
//!   with nothing playing (`v2-plan.md`, decision 18): `SPARK` grows every
//!   span, `CHARACTER` reshapes them, `BODY` and `AIR` tilt them, `LIFT` opens
//!   the quiet end, and the band weights and trims move one band's
//! - **the move** — a solid bar from unity to what is applied right now
//!
//! A region's span comes from `sparkleur_core::crossover::edges_for`, **the
//! function the filters are tuned from**, so `FOCUS` slides the picture and
//! the sound together. Dragging a region up or down sets that band's trim.

use super::bind::{self, Ctx};
use crate::analysis::{HIGH_HZ, LOW_HZ};
use egui::{Align2, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use nxe_egui::{font, input, smooth, surface};
use sparkleur_core::crossover::{BAND_COUNT, edges_for};
use sparkleur_core::engine::transfer_db;
use truce::params::FloatParamReadF32;

pub const NAMES: [&str; BAND_COUNT] = ["Sub", "Body", "Mid", "Presence", "Air"];

/// The figure's dB range either side of unity.
const RANGE_DB: f32 = 12.0;
/// What "quiet" and "loud" mean for the reach: a band sitting well under the
/// upward threshold, and one well over the downward one.
const QUIET_DB: f32 = -48.0;
const LOUD_DB: f32 = -6.0;
const FLOOR_DB: f32 = -72.0;
/// The axis is drawn at a fixed rate: the edges barely move with it, and the
/// window does not know the host's.
const SAMPLE_RATE: f32 = 48_000.0;
const LABEL: f32 = 16.0;

/// Frequency → `0..=1` across the figure, log.
pub fn x_of(hz: f32) -> f32 {
    ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

/// Each band's span in Hz, low to high.
pub fn spans(focus: f32) -> [(f32, f32); BAND_COUNT] {
    let edges = edges_for(focus, SAMPLE_RATE);
    std::array::from_fn(|band| {
        let low = if band == 0 { LOW_HZ } else { edges[band - 1] };
        let high = if band == BAND_COUNT - 1 {
            HIGH_HZ
        } else {
            edges[band]
        };
        (low, high)
    })
}

/// What the band does to a quiet and to a loud signal, in dB: `(lift, cut)`.
pub fn reach(params: &crate::params::SparkleurParams, band: usize) -> (f32, f32) {
    let shape = params.display_shape();
    let levels = params.display_levels();
    (
        transfer_db(&shape, &levels, band, QUIET_DB) - QUIET_DB,
        transfer_db(&shape, &levels, band, LOUD_DB) - LOUD_DB,
    )
}

pub fn field(ui: &mut Ui, ctx: &Ctx, painter: &Painter, rect: Rect, ink: &Palette) {
    let params = ctx.params();
    let plot = Rect::from_min_max(
        rect.min + vec2(SPACE_3, LABEL + SPACE_1),
        rect.max - vec2(SPACE_3, LABEL),
    );
    let y_of = |db: f32| plot.center().y - (db / RANGE_DB).clamp(-1.0, 1.0) * plot.height() / 2.0;
    let x_at = |hz: f32| plot.left() + plot.width() * x_of(hz);

    // What came in, as the backdrop.
    let dry = params.analysis.dry.read();
    let dry: Vec<f32> = dry
        .iter()
        .map(|level| ((20.0 * level.max(1e-9).log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0))
        .collect();
    let dry = smooth::smoothed(ui, ui.id().with("dry"), &dry, smooth::TAU);
    let points: Vec<(f32, f32)> = dry
        .iter()
        .enumerate()
        .map(|(band, y)| (band as f32 / (dry.len() - 1).max(1) as f32, *y))
        .collect();
    curve(painter, plot, &points, ink.subtle, Some(ink.dim), false);

    let hair = Stroke::new(1.0, ink.line.color());
    painter.line_segment(
        [pos2(plot.left(), y_of(0.0)), pos2(plot.right(), y_of(0.0))],
        Stroke::new(1.0, ink.muted.color()),
    );

    let gains = params.analysis.gains.read();
    let gains = smooth::smoothed(ui, ui.id().with("gains"), &gains, smooth::TAU);
    let solos: [bool; BAND_COUNT] =
        std::array::from_fn(|band| bind::plain(ctx, params.band_ids(band)[3]) > 0.5);
    let soloing = solos.iter().any(|on| *on);
    let spans = spans(params.focus.value());

    for (band, (low, high)) in spans.iter().enumerate() {
        let (left, right) = (x_at(*low), x_at(*high));
        if band > 0 {
            painter.line_segment([pos2(left, rect.top()), pos2(left, rect.bottom())], hair);
        }
        let region = Rect::from_min_max(pos2(left, plot.top()), pos2(right, plot.bottom()));
        let heard = !soloing || solos[band];
        let alpha = if heard { 1.0 } else { 0.3 };
        let column = region.shrink2(vec2((region.width() * 0.18).min(14.0), 0.0));

        // The reach: what the macros allow this band, quiet to loud.
        let (lift, cut) = reach(params, band);
        painter.rect_filled(
            Rect::from_x_y_ranges(column.x_range(), y_of(lift)..=y_of(cut)),
            0.0,
            ink.dim.at(ink.dim.alpha * alpha).color(),
        );
        for db in [lift, cut] {
            painter.line_segment(
                [
                    pos2(column.left(), y_of(db)),
                    pos2(column.right(), y_of(db)),
                ],
                Stroke::new(1.0, ink.deep.at(ink.deep.alpha * alpha).color()),
            );
        }

        // The move: what is applied right now.
        let gain = gains[band];
        let bar = Rect::from_x_y_ranges(
            column.shrink2(vec2(column.width() * 0.25, 0.0)).x_range(),
            y_of(gain.max(0.0))..=y_of(gain.min(0.0)),
        );
        painter.rect_filled(bar, 0.0, ink.ink.at(0.85 * alpha).color());

        // Dragging the region sets the band's trim.
        let gain_id = params.band_ids(band)[2];
        let response = ui.interact(
            region,
            ui.id().with(("band", band)),
            Sense::click_and_drag(),
        );
        let gestures = input::read(&response, bind::normalized(ctx, gain_id));
        bind::gestures(
            ctx,
            &[gain_id],
            &gestures,
            &[gestures.change.unwrap_or(0.0)],
        );
        if response.hovered() || response.dragged() {
            painter.rect_stroke(
                region.shrink(1.0),
                0.0,
                Stroke::new(1.0, ink.line.color()),
                egui::StrokeKind::Inside,
            );
        }

        let centre = (left + right) / 2.0;
        font::tracked(
            painter,
            pos2(centre, rect.top() + SPACE_1),
            Align2::CENTER_TOP,
            NAMES[band],
            FONT_EYEBROW,
            if solos[band] {
                ink.ink.color()
            } else {
                ink.muted.at(ink.muted.alpha * alpha).color()
            },
        );
        painter.text(
            pos2(centre, rect.bottom() - SPACE_1),
            Align2::CENTER_BOTTOM,
            format!("{gain:+.1}"),
            font::mono(FONT_VALUE),
            ink.ink.at(alpha).color(),
        );
    }

    // The Sparkle gate: a glow across the top of the bands it feeds, as bright
    // as the gate is open. **The moment it lights** is what tells a static
    // exciter from this one (`REQ-SPK-007`).
    let opening = params.analysis.sparkle.read()[0];
    let opening = smooth::smoothed(ui, ui.id().with("sparkle"), &[opening], smooth::TAU)[0];
    let from = x_at(spans[3].0);
    let line: Vec<Pos2> = vec![pos2(from, plot.top()), pos2(plot.right(), plot.top())];
    if opening > 0.01 {
        surface::glow(painter, &line, ink.ink.at(opening.min(1.0)));
    }
    painter.line_segment(
        [line[0], line[1]],
        Stroke::new(2.0, ink.ink.at(0.15 + 0.85 * opening.min(1.0)).color()),
    );

    // De-Harsh, where it pulls: the presence band.
    let de_harsh = params.analysis.de_harsh.read()[0];
    if de_harsh < -0.1 {
        let (low, high) = spans[3];
        painter.text(
            pos2((x_at(low) + x_at(high)) / 2.0, plot.top() + SPACE_2),
            Align2::CENTER_TOP,
            format!("De-Harsh {de_harsh:.1}"),
            font::mono(FONT_EYEBROW),
            ink.ink.color(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::SparkleurParams;
    use truce_params::Params;

    #[test]
    fn the_bands_tile_the_axis() {
        let spans = spans(0.0);
        assert_eq!(spans[0].0, LOW_HZ);
        assert_eq!(spans[BAND_COUNT - 1].1, HIGH_HZ);
        for pair in spans.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
    }

    /// **Decision 18**: turning `SPARK` changes the picture with nothing
    /// playing. If the reach did not move, the figure would be silent about
    /// the one knob everybody turns first.
    #[test]
    fn spark_moves_the_reach() {
        let params = SparkleurParams::default();
        let before = reach(&params, 3);
        params.set_plain(params.spark.id(), 1.0);
        let after = reach(&params, 3);
        assert!(after.0 > before.0, "lift {before:?} → {after:?}");
        assert!(after.1 < before.1, "cut {before:?} → {after:?}");
    }
}
