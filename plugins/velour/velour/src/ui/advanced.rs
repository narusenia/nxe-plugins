//! The Advanced tab: the per-band layer, the guards, `EMOTION`, and the two
//! global switches.
//!
//! **The second layer of the two-layer model** (`REQ-VEL-010`). The main tab's
//! knob says how much of a band there is; the row here says what kind. They are
//! separate parameters, and nothing in this file writes the other layer — a
//! computed value written back is the one thing this model cannot survive
//! (`.agents/rules/vizia.md`).
//!
//! Pointing at a row marks the matching region in the figure, and pointing at a
//! region marks the row. One value, both directions.

use super::{Ui, UiEvent};
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::hint::Describe;
use nxe_ui::pictogram;
use nxe_ui::theme;
use velour_core::BAND_COUNT;

/// The name column, and the width one bar gets. Fixed rather than stretched, so
/// the three rows and the header line up.
///
/// **160 since `VEL-20`.** The table and the side column left **204 px** of
/// nothing between them — the widest void in any of the five windows, and the
/// one that made this one read as unfinished. Velour has two bar columns where
/// Sparkleur has three, so the slack went to the bars on both sides: they are
/// what is dragged, and a wider one is a finer one (`SPK-23`).
const NAME_WIDTH: f32 = 64.0;
const BAR_WIDTH: f32 = 160.0;
const SOLO_WIDTH: f32 = 44.0;

/// **A bar has no default height** — an unset one is `Stretch(1.0)`, and a
/// stretching child of an `Auto`-sized parent resolves to nothing
/// (`.agents/rules/vizia.md`). Every bar here was a hairline, and a hairline
/// cannot be grabbed: the rows read as controls that did nothing. The gallery
/// sets this on every bar it shows, which is where the number comes from.
const BAR_HEIGHT: f32 = 10.0;

/// The right-hand column's own widths. `OVERSAMPLE` does not fit in
/// [`NAME_WIDTH`] — it was clipped to `OVERSAMPL` — and the extra comes out of
/// the bar rather than out of the window, so the column's total is unchanged
/// and the bars stay aligned with the segmented control under them.
///
/// **The name column also holds a mark now** (`UI-17`).
const SIDE_NAME_WIDTH: f32 = 100.0;
const SIDE_BAR_WIDTH: f32 = 160.0;

/// The right column, which holds what is global rather than per band.
///
/// **A column now, not a knob beside a list.** `FOCUS` moved to the row of
/// knobs, which is where its neighbours are (`ui.md`, `VEL-20`).
const SIDE_WIDTH: f32 = SIDE_NAME_WIDTH + theme::SPACE_2 + SIDE_BAR_WIDTH;

const NAMES: [&str; BAND_COUNT] = ["BODY", "PRES", "AIR"];

/// A row is as tall as its tallest cell, which is the solo switch.
const ROW_HEIGHT: f32 = theme::SEGMENT;

/// The table: a heading row, then one per band.
const TABLE_HEIGHT: f32 = theme::LINE_EYEBROW
    + theme::SPACE_2
    + ROW_HEIGHT * BAND_COUNT as f32
    + theme::SPACE_2 * (BAND_COUNT - 1) as f32;

/// How tall the whole panel is.
///
/// **Part of the window's height, so it is arithmetic** (`nxe_ui::theme`).
pub const HEIGHT: f32 = TABLE_HEIGHT;

/// The right-hand column: three labelled bars over the oversampling row, spaced
/// so the last one ends level with the table's last row.
///
/// **Arithmetic, because the alignment is the point.** Four rows against three
/// only end together if it is said out loud (`SPK-23`).
const SIDE_ROWS: usize = 4;
const SIDE_ROW_GAP: f32 = (TABLE_HEIGHT - ROW_HEIGHT * SIDE_ROWS as f32) / (SIDE_ROWS - 1) as f32;
const _: () = assert!(
    SIDE_ROW_GAP > 0.0,
    "the side column is taller than the table"
);

pub fn view(cx: &mut Context) {
    HStack::new(cx, |cx| {
        table(cx);
        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        side(cx);
    })
    .class("row")
    .height(Auto)
    .col_between(Pixels(theme::SPACE_3));
}

/// `Bias` / `Texture` / `Solo`, one row per band.
fn table(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            heading(cx, None, "", NAME_WIDTH);
            heading(cx, Some(pictogram::TRIM), "BIAS", BAR_WIDTH);
            heading(cx, Some(pictogram::TEXTURE), "TEXTURE", BAR_WIDTH);
            heading(cx, Some(pictogram::SOLO), "SOLO", SOLO_WIDTH);
        })
        .height(Auto)
        .width(Auto)
        .col_between(Pixels(theme::SPACE_2));

        for index in 0..BAND_COUNT {
            row(cx, index);
        }
    })
    .height(Auto)
    .width(Auto)
    .row_between(Pixels(theme::SPACE_2));
}

/// A column's name, with the mark that makes it findable (`UI-17`).
///
/// **A column's name is not a control's name** (`crates/nxe-ui/README.md`): set
/// as an eyebrow it reads as the table's structure instead of joining the row of
/// labels under it.
fn heading(cx: &mut Context, glyph: Option<pictogram::Glyph>, text: &'static str, width: f32) {
    match glyph {
        Some(glyph) => {
            pictogram::heading(cx, glyph, text).width(Pixels(width));
        }
        // The name column, which has no heading and is only here to hold the
        // table's first column open.
        None => {
            Label::new(cx, text)
                .class("eyebrow")
                .width(Pixels(width))
                .height(Pixels(theme::LINE_EYEBROW));
        }
    }
}

fn row(cx: &mut Context, index: usize) {
    HStack::new(cx, |cx| {
        Label::new(cx, NAMES[index])
            .class("label")
            // The row marks the region while the pointer is on it, so the name
            // must not eat the hover: only the row is hoverable.
            .class("decoration")
            .width(Pixels(NAME_WIDTH))
            .height(Pixels(theme::LINE_LABEL));

        // The bars are wrapped so each column is a fixed width whatever the bar
        // itself decides to be.
        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => nxe_plug_ui::bar(cx, Ui::params, |params| &params.bias_body, true),
                1 => nxe_plug_ui::bar(cx, Ui::params, |params| &params.bias_presence, true),
                _ => nxe_plug_ui::bar(cx, Ui::params, |params| &params.bias_air, true),
            }
            .describe("Deeper curve added quieter, or the reverse")
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => nxe_plug_ui::bar(cx, Ui::params, |params| &params.texture_body, true),
                1 => nxe_plug_ui::bar(cx, Ui::params, |params| &params.texture_presence, true),
                _ => nxe_plug_ui::bar(cx, Ui::params, |params| &params.texture_air, true),
            }
            .describe("This band's deviation from TEXTURE")
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, SOLO_WIDTH, move |cx| {
            HStack::new(cx, |cx| {
                match index {
                    0 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_body, "ON"),
                    1 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_presence, "ON"),
                    _ => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_air, "ON"),
                }
                .describe("Hear this band's layer alone, dry muted");
            })
            .class("segmented")
            .width(Auto)
            .height(Auto);
        });
    })
    .class("row")
    .height(Pixels(ROW_HEIGHT))
    .width(Auto)
    .col_between(Pixels(theme::SPACE_2))
    .on_hover(move |cx| cx.emit(UiEvent::Hover(Some(index))))
    .on_hover_out(move |cx| cx.emit(UiEvent::Hover(None)));
}

/// A fixed-width box around one control, so the columns line up.
fn cell(cx: &mut Context, width: f32, content: impl Fn(&mut Context)) {
    HStack::new(cx, |cx| content(cx))
        .width(Pixels(width))
        .height(Auto)
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0));
}

/// What is global: the two guards, `EMOTION`, and the oversampling.
///
/// **`FOCUS` is not here any more** (`VEL-20`). It sat as a knob beside this
/// list, which left the list short of the table and the knob orphaned from the
/// row of knobs it belongs to.
///
/// **Both guards carry the same mark.** They are one operation on two bands —
/// the marks name kinds, not parameters (`nxe_ui::pictogram`).
fn side(cx: &mut Context) {
    VStack::new(cx, |cx| {
        labelled_bar(
            cx,
            pictogram::DE_HARSH,
            "HARSH",
            "How far the harsh guard may pull",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |params| &params.guard_harsh, false),
        );
        labelled_bar(
            cx,
            pictogram::DE_HARSH,
            "SIB",
            "How far the sibilance guard may pull",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |params| &params.guard_sib, false),
        );
        labelled_bar(
            cx,
            pictogram::FOLLOW,
            "EMOTION",
            "How much the singing moves the curves",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |params| &params.emotion, false),
        );

        HStack::new(cx, |cx| {
            pictogram::label(cx, pictogram::OVERSAMPLE, "OVERSAMPLE")
                .width(Pixels(SIDE_NAME_WIDTH));
            nxe_plug_ui::segmented(cx, Ui::params, |params| &params.oversample, &["2x", "4x"])
                .describe("2x costs less and aliases about 14 dB higher");
        })
        .class("row")
        .height(Pixels(ROW_HEIGHT))
        .width(Auto)
        .col_between(Pixels(theme::SPACE_2))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0));
    })
    .class("hint-left")
    .width(Pixels(SIDE_WIDTH))
    .height(Pixels(HEIGHT))
    .row_between(Pixels(SIDE_ROW_GAP));
}

fn labelled_bar(
    cx: &mut Context,
    glyph: pictogram::Glyph,
    label: &'static str,
    hint: &'static str,
    content: impl Fn(&mut Context) -> Handle<'_, nxe_ui::bar::Bar>,
) {
    HStack::new(cx, |cx| {
        pictogram::label(cx, glyph, label).width(Pixels(SIDE_NAME_WIDTH));
        content(cx)
            .describe(hint)
            .width(Pixels(SIDE_BAR_WIDTH))
            .height(Pixels(BAR_HEIGHT));
    })
    .class("row")
    .height(Pixels(ROW_HEIGHT))
    .width(Auto)
    .col_between(Pixels(theme::SPACE_2))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

/// What the table and the side column together are allowed to be, so that
/// widening a bar cannot quietly push the side column off the window.
///
/// The window, less the root's padding, the meter strip and the gap beside it.
#[cfg(test)]
const AVAILABLE: f32 =
    super::WIDTH as f32 - theme::SPACE_3 * 2.0 - super::meters::WIDTH - theme::SPACE_3;

#[cfg(test)]
mod tests {
    use super::{AVAILABLE, BAR_WIDTH, NAME_WIDTH, SIDE_WIDTH, SOLO_WIDTH, theme};

    /// **A layout that overflows still lays out** (`.agents/rules/ui.md`), so
    /// nothing fails when a column is pushed off the window — it is simply not
    /// there. This is the assertion that notices.
    ///
    /// **Two gaps, not one.** The row is table, spacer, side; the spacer is
    /// what is left over and it must not be negative.
    #[test]
    fn the_row_fits_the_window() {
        let table = NAME_WIDTH + BAR_WIDTH * 2.0 + SOLO_WIDTH + theme::SPACE_2 * 3.0;
        let fixed = table + SIDE_WIDTH + theme::SPACE_3 * 2.0;
        assert!(
            fixed <= AVAILABLE,
            "the advanced row is {fixed} wide in a {AVAILABLE} space"
        );
    }
}
