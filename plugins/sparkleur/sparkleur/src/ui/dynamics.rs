//! The Dynamics tab: the controls that shape *how* the bands move, and one
//! band's transfer curve.
//!
//! **`DE-HARSH` and `SUB PROT` are deviations, not amounts** (`REQ-SPK-008`):
//! zero follows `CHARACTER`.
//!
//! **The curve is input level against output level**, which is what a
//! compressor is read by: under the diagonal is compression, over it is lift.
//! It comes from `sparkleur_core::engine::transfer_db`, the same gain computer
//! the audio path runs, and the dot is where the band is sitting right now.
//! Read only: from a dragged point there is no way to say which of two
//! thresholds and two ratios moved.

use super::bind::{self, Ctx};
use super::field::NAMES;
use egui::{Align2, Rect, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::knob::{block_height, knob};
use nxe_egui::readout::readout;
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_2, SPACE_4};
use nxe_egui::{font, segment, smooth};
use sparkleur_core::engine::transfer_db;

/// The curve's range, both axes.
const LOW_DB: f32 = -72.0;
const HIGH_DB: f32 = 0.0;
pub const KNOBS_WIDTH: f32 = 4.0 * 56.0 + 3.0 * SPACE_4;

pub fn dynamics(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect, knob_size: f32) {
    let params = ctx.params();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                rect.min,
                vec2(KNOBS_WIDTH, block_height(knob_size)),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for (id, name) in [
        (params.lift.id(), "Lift"),
        (params.punch.id(), "Punch"),
        (params.de_harsh.id(), "De-Harsh"),
        (params.sub_protect.id(), "Sub Protect"),
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

    // What the two dynamic parts are doing right now.
    let sparkle = params.analysis.sparkle.read()[0];
    let de_harsh = params.analysis.de_harsh.read()[0];
    let live = smooth::smoothed(ui, ui.id().with("live"), &[sparkle, de_harsh], smooth::TAU);
    let width = (KNOBS_WIDTH - SPACE_4) / 2.0;
    let mut figures = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(rect.left(), rect.top() + block_height(knob_size) + SPACE_2),
                vec2(KNOBS_WIDTH, nxe_egui::readout::HEIGHT),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    figures.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    readout(
        &mut figures,
        palette,
        "Sparkle",
        &format!("{:.0}", live[0] * 100.0),
        "%",
        width,
    );
    readout(
        &mut figures,
        palette,
        "De-Harsh",
        &format!("{:+.1}", live[1]),
        "dB",
        width,
    );

    // The transfer curve, for a band picked above it — the soloed one until
    // another is picked.
    let area = Rect::from_min_max(
        pos2(rect.left() + KNOBS_WIDTH + SPACE_4, rect.top()),
        rect.max,
    );
    let picked_id = ui.id().with("curve-band");
    let soloed = (0..NAMES.len()).find(|band| bind::plain(ctx, params.band_ids(*band)[3]) > 0.5);
    let band: usize = ui
        .data(|data| data.get_temp(picked_id))
        .or(soloed)
        .unwrap_or(3);
    let mut tabs = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(
        area.min,
        vec2(area.width(), segment::HEIGHT),
    )));
    let short = ["Sub", "Body", "Mid", "Pres", "Air"];
    if let Some(picked) = segment::segment(&mut tabs, palette, &short, band, area.width()) {
        ui.data_mut(|data| data.insert_temp(picked_id, picked));
    }

    let plot = Rect::from_min_max(
        pos2(area.left(), area.top() + segment::HEIGHT + SPACE_2),
        area.max,
    );
    let painter = ui.painter_at(plot);
    let hair = Stroke::new(1.0, palette.line.color());
    painter.rect_stroke(plot, 0.0, hair, egui::StrokeKind::Inside);
    painter.line_segment([plot.left_bottom(), plot.right_top()], hair);
    let to_unit = |db: f32| ((db - LOW_DB) / (HIGH_DB - LOW_DB)).clamp(0.0, 1.0);

    let shape = params.display_shape();
    let levels = params.display_levels();
    let points: Vec<(f32, f32)> = (0..=36)
        .map(|step| {
            let input = LOW_DB + (HIGH_DB - LOW_DB) * step as f32 / 36.0;
            (
                to_unit(input),
                to_unit(transfer_db(&shape, &levels, band, input)),
            )
        })
        .collect();
    curve(&painter, plot, &points, palette.accent, None, false);

    // Where the band is sitting: its detector's level, and what it is moved by.
    let level = params.analysis.levels.read()[band];
    let gain = params.analysis.gains.read()[band];
    let now = smooth::smoothed(
        ui,
        ui.id().with(("now", band)),
        &[level, level + gain],
        smooth::TAU,
    );
    if now[0] > LOW_DB {
        let at = pos2(
            plot.left() + plot.width() * to_unit(now[0]),
            plot.bottom() - plot.height() * to_unit(now[1]),
        );
        painter.circle_filled(at, 3.5, palette.ink.color());
    }
    font::tracked(
        &painter,
        plot.left_top() + vec2(SPACE_2, SPACE_2),
        Align2::LEFT_TOP,
        &format!("{} · in → out", NAMES[band]),
        FONT_EYEBROW,
        palette.subtle.color(),
    );
}
