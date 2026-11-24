//! The Band Field: the three generators as regions on a log frequency axis, on
//! the accent ground.
//!
//! **The reason the parallel topology was chosen shows up here** (`ui.md`): the
//! dry spectrum and the spectrum of what is being added to it are two layers,
//! drawn at once — the dry as a wash, the added layer as a line over it.
//!
//! Each region is one generator, spanning `Generator::input_range` — the
//! function the filters are tuned from, so `FOCUS` slides the picture and the
//! sound together. Inside it:
//!
//! - **a column as tall as the band's fader**, sunk by what its guard is
//!   pulling (`REQ-VEL-006`): a region that sinks says why `AIR` "does
//!   nothing". Dragging the region moves the fader
//! - **the band's curve**, from `velour_core::engine::curve_for`, the function
//!   the audio path is tuned from. This is what makes the macros visible with
//!   nothing playing (`v2-plan.md`, decision 18): `DRIVE` bends it, `TEXTURE`
//!   and the per-band texture move it between even and odd, a bias leans it,
//!   and `Hard` sharpens it

use super::bind::{self, Ctx};
use crate::analysis::{HIGH_HZ, LOW_HZ};
use egui::{Align2, Painter, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use nxe_egui::{font, input, smooth, surface};
use velour_core::bands::{BANDS, Generator};
use velour_core::engine::{BAND_COUNT, curve_for};
use velour_core::guard::MAX_REDUCTION_DB;

pub const NAMES: [&str; BAND_COUNT] = ["Body", "Presence", "Air"];

const FLOOR_DB: f32 = -72.0;
/// The axis is drawn at a fixed rate: the spans barely move with it, and the
/// window does not know the host's.
const SAMPLE_RATE: f32 = 48_000.0;
const LABEL: f32 = 16.0;
const GLYPH: f32 = 44.0;

/// Frequency → `0..=1` across the figure, log.
pub fn x_of(hz: f32) -> f32 {
    ((hz.max(LOW_HZ) / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

/// What a guard's reduction sinks its region by, as a fraction of the region.
/// **Body has no guard** (`velour_core::engine`).
pub fn sunk(band: usize, guards: &[f32; 2]) -> f32 {
    let reduction = match band {
        0 => return 0.0,
        1 => guards[0],
        _ => guards[1],
    };
    (-reduction / MAX_REDUCTION_DB).clamp(0.0, 1.0)
}

/// The band's resting curve, `0..=1` both ways, normalized to its own peak so
/// the shape reads at any drive.
pub fn glyph(params: &crate::params::VelourParams, band: usize) -> Vec<(f32, f32)> {
    let shaper = curve_for(&params.display_shape(), band, 0.0).shaper();
    let outputs: Vec<f32> = (0..=32)
        .map(|step| shaper.shape(step as f32 / 16.0 - 1.0))
        .collect();
    let peak = outputs
        .iter()
        .fold(0.0f32, |peak, output| peak.max(output.abs()))
        .max(1e-6);
    outputs
        .iter()
        .enumerate()
        .map(|(step, output)| (step as f32 / 32.0, (output / peak + 1.0) * 0.5))
        .collect()
}

fn spectrum(levels: &[f32]) -> Vec<f32> {
    levels
        .iter()
        .map(|level| ((20.0 * level.max(1e-9).log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0))
        .collect()
}

fn points(levels: &[f32]) -> Vec<(f32, f32)> {
    levels
        .iter()
        .enumerate()
        .map(|(band, y)| (band as f32 / (levels.len() - 1).max(1) as f32, *y))
        .collect()
}

pub fn field(ui: &mut Ui, ctx: &Ctx, painter: &Painter, rect: Rect, ink: &Palette) {
    let params = ctx.params();
    let plot = Rect::from_min_max(
        rect.min + vec2(SPACE_3, LABEL + SPACE_1),
        rect.max - vec2(SPACE_3, LABEL),
    );
    let x_at = |hz: f32| plot.left() + plot.width() * x_of(hz);

    // What came in as a wash, what is added as a line over it.
    let dry = spectrum(&params.analysis.dry.read());
    let dry = smooth::smoothed(ui, ui.id().with("dry"), &dry, smooth::TAU);
    curve(
        painter,
        plot,
        &points(&dry),
        ink.subtle,
        Some(ink.dim),
        false,
    );
    let wet = spectrum(&params.analysis.wet.read());
    let wet = smooth::smoothed(ui, ui.id().with("wet"), &wet, smooth::TAU);
    let sounding = wet.iter().any(|level| *level > 0.02);
    curve(painter, plot, &points(&wet), ink.ink, None, sounding);

    let guards = params.analysis.guards.read();
    let guards = smooth::smoothed(ui, ui.id().with("guards"), &guards, smooth::TAU);
    let guards = [guards[0], guards[1]];
    let solos: [bool; BAND_COUNT] =
        std::array::from_fn(|band| bind::plain(ctx, params.band_ids(band)[3]) > 0.5);
    let soloing = solos.iter().any(|on| *on);
    let focus = bind::plain(ctx, params.focus.id());

    for (band, name) in NAMES.iter().enumerate() {
        let (low, high) = Generator::input_range(BANDS[band], focus, SAMPLE_RATE);
        let (left, right) = (x_at(low), x_at(high));
        let region = Rect::from_min_max(pos2(left, plot.top()), pos2(right, plot.bottom()));
        let heard = !soloing || solos[band];
        let alpha = if heard { 1.0 } else { 0.3 };
        let hair = Stroke::new(1.0, ink.line.at(ink.line.alpha * alpha).color());
        painter.line_segment([region.left_top(), region.left_bottom()], hair);
        painter.line_segment([region.right_top(), region.right_bottom()], hair);

        // The fader, sunk by the guard.
        let level_id = params.band_ids(band)[0];
        let level = bind::plain(ctx, level_id);
        let held = sunk(band, &guards);
        let column = Rect::from_min_max(
            pos2(left + SPACE_1, plot.bottom() - plot.height() * level),
            pos2(right - SPACE_1, plot.bottom()),
        );
        painter.rect_filled(column, 0.0, ink.dim.at(ink.dim.alpha * alpha).color());
        let top = column.top() + column.height() * held;
        painter.line_segment(
            [pos2(column.left(), top), pos2(column.right(), top)],
            Stroke::new(2.0, ink.ink.at(alpha).color()),
        );
        if held > 0.01 {
            // What the guard took, hatched off the top of the column.
            painter.rect_filled(
                Rect::from_min_max(column.left_top(), pos2(column.right(), top)),
                0.0,
                ink.track.color(),
            );
        }

        let response = ui.interact(
            region,
            ui.id().with(("band", band)),
            Sense::click_and_drag(),
        );
        let gestures = input::read(&response, bind::normalized(ctx, level_id));
        bind::gestures(
            ctx,
            &[level_id],
            &gestures,
            &[gestures.change.unwrap_or(0.0)],
        );
        if response.hovered() || response.dragged() {
            painter.rect_stroke(
                region.shrink(1.0),
                0.0,
                Stroke::new(1.0, ink.ink.at(0.5).color()),
                egui::StrokeKind::Inside,
            );
        }

        // The curve, in a square at the top of the region.
        let side = GLYPH.min(region.width() - 2.0 * SPACE_2).max(16.0);
        let square = Rect::from_center_size(
            pos2(region.center().x, plot.top() + SPACE_2 + side / 2.0),
            vec2(side, side),
        );
        painter.rect_filled(square, 0.0, ink.ground.color());
        painter.rect_stroke(
            square,
            0.0,
            Stroke::new(1.0, ink.line.color()),
            egui::StrokeKind::Inside,
        );
        let line: Vec<Pos2> = glyph(params, band)
            .into_iter()
            .map(|(x, y)| {
                pos2(
                    square.left() + square.width() * x,
                    square.bottom() - square.height() * y,
                )
            })
            .collect();
        painter.add(egui::Shape::line(
            line.clone(),
            Stroke::new(1.5, ink.ink.at(alpha).color()),
        ));
        if sounding && heard {
            surface::glow(painter, &line, ink.ink.at(0.5));
        }

        let centre = region.center().x;
        font::tracked(
            painter,
            pos2(centre, rect.top() + SPACE_1),
            Align2::CENTER_TOP,
            name,
            FONT_EYEBROW,
            if solos[band] {
                ink.ink.color()
            } else {
                ink.muted.at(ink.muted.alpha * alpha).color()
            },
        );
        let figure = if held > 0.01 {
            format!("{:.0}%  {:+.1} dB", level * 100.0, guards[band.min(2) - 1])
        } else {
            format!("{:.0}%", level * 100.0)
        };
        painter.text(
            pos2(centre, rect.bottom() - SPACE_1),
            Align2::CENTER_BOTTOM,
            figure,
            font::mono(FONT_VALUE),
            ink.ink.at(alpha).color(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::VelourParams;
    use truce_params::Params;

    /// The figure sinks by exactly what the guard is pulling, and Body never
    /// sinks because nothing guards it.
    #[test]
    fn a_region_sinks_by_its_guard() {
        let guards = [-MAX_REDUCTION_DB / 2.0, -MAX_REDUCTION_DB];
        assert_eq!(sunk(0, &guards), 0.0);
        assert!((sunk(1, &guards) - 0.5).abs() < 1e-6);
        assert!((sunk(2, &guards) - 1.0).abs() < 1e-6);
        assert_eq!(sunk(1, &[3.0, 0.0]), 0.0, "a guard cannot push");
    }

    /// **Decision 18**: turning `DRIVE` changes the picture with nothing
    /// playing.
    #[test]
    fn drive_bends_the_curve() {
        let params = VelourParams::default();
        params.set_plain(params.drive.id(), 0.05);
        let gentle = glyph(&params, 1);
        params.set_plain(params.drive.id(), 1.0);
        let driven = glyph(&params, 1);
        let moved = gentle
            .iter()
            .zip(&driven)
            .map(|(a, b)| (a.1 - b.1).abs())
            .fold(0.0f32, f32::max);
        assert!(moved > 0.02, "the curve moved by {moved}");
    }
}
