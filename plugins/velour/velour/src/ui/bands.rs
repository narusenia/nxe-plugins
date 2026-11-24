//! The Bands tab: the per-band layer, one column per generator.
//!
//! **The second layer of the two-layer model** (`REQ-VEL-010`). The sidebar's
//! fader says how much of a band there is; a column here says what kind —
//! its texture's deviation from `TEXTURE`, and the drive-versus-level split.
//! Nothing in this file writes the macro layer.
//!
//! Solo is the last row, and **a band that is soloed lights its whole column
//! and its region in the figure**: on truce the solos are automatable, so a
//! latched one has to be impossible to miss.

use super::bind::{self, Ctx};
use super::field::NAMES;
use egui::{Align2, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::bar::bar;
use nxe_egui::font;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2};
use velour_core::engine::BAND_COUNT;

const HEAD: f32 = 14.0;
const ROW: f32 = 24.0;
const LABEL: f32 = 56.0;
const FIGURE: f32 = 54.0;
const ROWS: [(&str, bool); 2] = [("Texture", true), ("Bias", true)];

pub const HEIGHT: f32 = HEAD + ROW * 3.0;

pub fn bands(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let painter = ui.painter_at(rect);
    let column = (rect.width() - LABEL) / BAND_COUNT as f32;
    for (row, (name, _)) in ROWS.iter().chain(&[("Solo", false)]).enumerate() {
        painter.text(
            pos2(rect.left(), rect.top() + HEAD + ROW * (row as f32 + 0.5)),
            Align2::LEFT_CENTER,
            name,
            font::sans(FONT_VALUE),
            palette.muted.color(),
        );
    }
    for (band, name) in NAMES.iter().enumerate() {
        let x = rect.left() + LABEL + column * band as f32;
        let ids = params.band_ids(band);
        let soloed = bind::plain(ctx, ids[3]) > 0.5;
        if soloed {
            painter.rect_filled(
                Rect::from_min_size(pos2(x, rect.top()), vec2(column - SPACE_2, HEIGHT)),
                0.0,
                palette.dim.color(),
            );
        }
        font::tracked(
            &painter,
            pos2(x + SPACE_1, rect.top()),
            Align2::LEFT_TOP,
            name,
            FONT_EYEBROW,
            if soloed {
                palette.ink.color()
            } else {
                palette.subtle.color()
            },
        );
        for (row, (_, bipolar)) in ROWS.iter().enumerate() {
            let y = rect.top() + HEAD + ROW * row as f32;
            let id = ids[row + 1];
            let cell = Rect::from_min_size(
                pos2(x + SPACE_1, y),
                vec2(column - FIGURE - SPACE_2 - SPACE_1, ROW),
            );
            let value = bind::normalized(ctx, id);
            let mut child = ui.new_child(UiBuilder::new().max_rect(cell));
            let gestures = bar(&mut child, palette, value, *bipolar, cell.size(), 4.0);
            bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(value)]);
            painter.text(
                pos2(x + column - SPACE_2, y + ROW / 2.0),
                Align2::RIGHT_CENTER,
                ctx.format_param(id),
                font::mono(FONT_VALUE),
                palette.ink.color(),
            );
        }

        // Solo: a box that is filled when on.
        let y = rect.top() + HEAD + ROW * 2.0;
        let button = Rect::from_min_size(
            pos2(x + SPACE_1, y + SPACE_1),
            vec2(column - SPACE_2 - SPACE_1, ROW - SPACE_2),
        );
        let response = ui.interact(button, ui.id().with(("solo", band)), Sense::click());
        if response.clicked() {
            bind::set_once(ctx, ids[3], if soloed { 0.0 } else { 1.0 });
        }
        if soloed {
            painter.rect_filled(button, 0.0, palette.accent.color());
        } else {
            painter.rect_stroke(
                button,
                0.0,
                Stroke::new(
                    1.0,
                    if response.hovered() {
                        palette.muted.color()
                    } else {
                        palette.line.color()
                    },
                ),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            "Solo",
            font::sans(FONT_VALUE),
            if soloed {
                palette.ground.color()
            } else {
                palette.muted.color()
            },
        );
    }
}
