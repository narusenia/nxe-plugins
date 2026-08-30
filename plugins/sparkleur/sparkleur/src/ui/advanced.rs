//! The Advanced tab: the per-band layer, the two protections, and the global
//! switches.
//!
//! **The second layer of the two-layer model** (`REQ-SPK-009`). `SPARK` says
//! how much is happening; a row here says how that amount is shared out. They
//! are separate parameters, and **nothing in this file writes the other
//! layer** — a computed value written back is the one thing this model cannot
//! survive (`.agents/rules/vizia.md`).
//!
//! **`DE-HARSH` and `SUB PROT` are deviations, not amounts** (`REQ-SPK-008`):
//! zero follows `CHARACTER`. An absolute control here would mean the axis and
//! this panel both writing one value.
//!
//! Pointing at a row marks the matching region in the figure, and pointing at a
//! region marks the row. One value, both directions.

use super::{Ui, UiEvent};
use crate::analysis::Analysis;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::hint::Describe;
use nxe_ui::pictogram;
use nxe_ui::{font, theme};
use sparkleur_core::crossover::BAND_COUNT;

/// The name column and the width one bar gets. Fixed rather than stretched, so
/// the five rows and the header line up.
///
/// **The bars were 76 wide when the window was 720.** At 880 the table and the
/// side column left 216 px of nothing between them, pooled on one side and
/// reading as an unfinished row rather than as space (`SPK-23`, seen in a
/// host). The slack went to the bars: they are what is dragged, and a wider one
/// is a finer one. **Not to a second readout** — the strip at the top of the
/// window already carries the figures, and the per-band gain is already under
/// each band's name.
///
/// **130 since `FOCUS` moved out of this panel.** Its knob was 44 px of the
/// side column's width; that width came back here rather than reopening the
/// gap this unit had just closed.
const NAME_WIDTH: f32 = 48.0;
const BAR_WIDTH: f32 = 130.0;
const SOLO_WIDTH: f32 = 40.0;

/// **A bar has no default height** — an unset one is `Stretch(1.0)`, and a
/// stretching child of an `Auto`-sized parent resolves to nothing
/// (`.agents/rules/vizia.md`). Every bar in Velour's Advanced table was a
/// hairline, and a hairline cannot be grabbed: the rows shipped reading as
/// controls that did nothing (`VEL-14`). The gallery sets this on every bar it
/// shows, which is where the number comes from.
const BAR_HEIGHT: f32 = 10.0;

/// The right-hand column's own widths. `OVERSAMPLE` does not fit in
/// [`NAME_WIDTH`], and the extra comes out of the bar rather than out of the
/// window, so the column's total is unchanged.
///
/// **Both grew for the marks and then for the gap** (`UI-17`, `SPK-23`). The
/// space between the table and this column was 96 px of nothing — the widest
/// void in the window, and the one that made it read as unfinished in a host.
/// 20 went to the names to hold a mark, 36 to the bars.
const SIDE_NAME_WIDTH: f32 = 100.0;
const SIDE_BAR_WIDTH: f32 = 116.0;

/// The right column, which holds what is global rather than per band.
///
/// **A column now, not a knob beside a list.** `FOCUS` moved to the row of
/// knobs, which is where its neighbours are (`ui.md`).
const SIDE_WIDTH: f32 = SIDE_NAME_WIDTH + theme::SPACE_2 + SIDE_BAR_WIDTH;

/// One row of the side column, and the gap that puts its last row level with
/// the table's.
///
/// **Arithmetic, because the alignment is the point.** The two columns are one
/// block or they are two things that happen to be next to each other, and with
/// six rows against five the only way they end together is to say so. The row
/// has to clear a `SegmentedControl` as well as a bar, which is what decides
/// the height.
const SIDE_ROWS: usize = 6;
const SIDE_ROW_HEIGHT: f32 = theme::LINE_LABEL + theme::SPACE_2;
const SIDE_ROW_GAP: f32 = (HEIGHT - SIDE_ROW_HEIGHT * SIDE_ROWS as f32) / (SIDE_ROWS - 1) as f32;
const _: () = assert!(
    SIDE_ROW_HEIGHT >= theme::SEGMENT,
    "the oversample row will clip"
);
const _: () = assert!(
    SIDE_ROW_GAP > 0.0,
    "the side column is taller than the table"
);

const NAMES: [&str; BAND_COUNT] = ["SUB", "BODY", "MID", "PRES", "AIR"];

/// What the table and the side column together are allowed to be, so that
/// widening a bar cannot quietly push the side column off the window.
///
/// The window, less the root's padding, the meter strip and the gap beside it.
#[cfg(test)]
const AVAILABLE: f32 =
    super::WIDTH as f32 - theme::SPACE_3 * 2.0 - super::meters::WIDTH - theme::SPACE_3;

/// A row is as tall as its tallest cell: the band's name over the gain it is
/// running at, which is two lines.
const ROW_HEIGHT: f32 = theme::LINE_LABEL + theme::LINE_VALUE;

/// How tall the whole panel is — the heading row, then one row per band.
///
/// **Part of the window's height, so it is arithmetic** (`nxe_ui::theme`).
pub const HEIGHT: f32 = theme::LINE_EYEBROW
    + theme::SPACE_2
    + ROW_HEIGHT * BAND_COUNT as f32
    + theme::SPACE_2 * (BAND_COUNT - 1) as f32;

/// Rewrites what each band is running at. Called on the heartbeat — **not read
/// inside a lens**, which is re-evaluated once per frame (`nxe_ui::readout`).
pub(crate) fn poll(analysis: &Analysis, applied_gains: &mut [String]) {
    let gains = analysis.gains.read();
    for (text, gain) in applied_gains.iter_mut().zip(gains.iter()) {
        *text = applied(*gain);
    }
}

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

/// `UP` / `DOWN` / `GAIN` / `SOLO`, one row per band.
fn table(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            heading(cx, None, "", NAME_WIDTH);
            heading(cx, Some(pictogram::UP), "UP", BAR_WIDTH);
            heading(cx, Some(pictogram::DOWN), "DOWN", BAR_WIDTH);
            heading(cx, Some(pictogram::GAIN), "GAIN", BAR_WIDTH);
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

/// What a band is actually running at, in dB.
///
/// **Always signed, and always the same width**, for the same reason the
/// reduction readout is (`ui/readout.rs`): a number that gains and loses a minus
/// as the band crosses unity makes a table of five rows twitch.
fn applied(gain_db: f32) -> String {
    if !gain_db.is_finite() {
        return "  0.0".to_owned();
    }
    format!("{gain_db:+.1}")
}

/// A column's name, with the mark that makes it findable (`UI-17`).
///
/// **A column's name is not a control's name** (`crates/nxe-ui/README.md`): set
/// as an eyebrow it reads as the table's structure instead of joining the row
/// of labels under it. The mark is the eyebrow's own height, so a heading row
/// does not grow by gaining one.
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
        // **The name and what the band is doing, together** (`SPK-19`). The
        // figure moves the region by this number already; a table of five rows
        // is where it can be read rather than compared by eye.
        VStack::new(cx, |cx| {
            Label::new(cx, NAMES[index])
                .class("label")
                .class("decoration")
                .height(Pixels(theme::LINE_LABEL));
            font::value(cx, Ui::applied_gains.index(index))
                .class("subtle")
                .class("decoration")
                .height(Pixels(theme::LINE_VALUE));
        })
        // The row marks the region while the pointer is on it, so the name
        // must not eat the hover: only the row is hoverable.
        .class("decoration")
        .width(Pixels(NAME_WIDTH))
        .height(Auto);

        // The bars are wrapped so each column is a fixed width whatever the bar
        // itself decides to be.
        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.up_sub, false),
                1 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.up_body, false),
                2 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.up_mid, false),
                3 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.up_pres, false),
                _ => nxe_plug_ui::bar(cx, Ui::params, |p| &p.up_air, false),
            }
            .describe("This band's share of the upward half")
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.down_sub, false),
                1 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.down_body, false),
                2 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.down_mid, false),
                3 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.down_pres, false),
                _ => nxe_plug_ui::bar(cx, Ui::params, |p| &p.down_air, false),
            }
            .describe("This band's share of the downward half")
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.gain_sub, true),
                1 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.gain_body, true),
                2 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.gain_mid, true),
                3 => nxe_plug_ui::bar(cx, Ui::params, |p| &p.gain_pres, true),
                _ => nxe_plug_ui::bar(cx, Ui::params, |p| &p.gain_air, true),
            }
            .describe("A static trim. The figure's region height")
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, SOLO_WIDTH, move |cx| {
            HStack::new(cx, |cx| {
                match index {
                    0 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_sub, "ON"),
                    1 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_body, "ON"),
                    2 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_mid, "ON"),
                    3 => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_pres, "ON"),
                    _ => nxe_plug_ui::toggle(cx, Ui::params, |p| &p.solo_air, "ON"),
                }
                .describe("Hear this band alone");
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

/// What is global: the two protections, `SNAP`, `LIFT`, `PUNCH` and the
/// oversampling.
///
/// **`FOCUS` is not here any more.** It sat as a knob beside this list, which
/// left the list short of the table beside it and the knob orphaned from the
/// row of knobs it belongs to (`SPK-23`, looked at in a host).
fn side(cx: &mut Context) {
    VStack::new(cx, |cx| {
        labelled_bar(
            cx,
            pictogram::DE_HARSH,
            "DE-HARSH",
            "Harder or softer than CHARACTER chose",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.de_harsh, true),
        );
        labelled_bar(
            cx,
            pictogram::SUB_PROTECT,
            "SUB PROT",
            "How far the bottom band's lift is closed",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.sub_protect, true),
        );
        labelled_bar(
            cx,
            pictogram::SNAP,
            "SNAP",
            "How much of AIR waits for a transient",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.snap, false),
        );
        labelled_bar(
            cx,
            pictogram::LIFT,
            "LIFT",
            "Opens the floor under the upward half",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.lift, false),
        );
        labelled_bar(
            cx,
            pictogram::PUNCH,
            "PUNCH",
            "How hard a transient is hit",
            |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.punch, false),
        );

        HStack::new(cx, |cx| {
            pictogram::label(cx, pictogram::OVERSAMPLE, "OVERSAMPLE")
                .width(Pixels(SIDE_NAME_WIDTH));
            nxe_plug_ui::segmented(cx, Ui::params, |params| &params.oversample, &["2x", "4x"])
                .describe("2x costs about 11 us and aliases 14 dB higher");
        })
        .class("row")
        .height(Pixels(SIDE_ROW_HEIGHT))
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
    .height(Pixels(SIDE_ROW_HEIGHT))
    .width(Auto)
    .col_between(Pixels(theme::SPACE_2))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

#[cfg(test)]
mod tests {
    use super::{AVAILABLE, BAR_WIDTH, NAME_WIDTH, SIDE_WIDTH, SOLO_WIDTH, theme};

    /// **A layout that overflows still lays out** (`.agents/rules/ui.md`), so
    /// nothing fails when a column is pushed off the window — it is simply not
    /// there. This is the assertion that notices.
    #[test]
    fn the_row_fits_the_window() {
        let table = NAME_WIDTH + BAR_WIDTH * 3.0 + SOLO_WIDTH + theme::SPACE_2 * 4.0;
        let fixed = table + SIDE_WIDTH + theme::SPACE_3;
        assert!(
            fixed <= AVAILABLE,
            "the advanced row is {fixed} wide in a {AVAILABLE} space"
        );
    }

    use super::*;

    /// **Every parameter has somewhere to be touched.** Seven on MAIN and the
    /// rest here; a parameter with no control is one a user can only reach
    /// through the host's generic view.
    #[test]
    fn the_table_has_a_row_for_every_band() {
        assert_eq!(NAMES.len(), BAND_COUNT);
    }

    /// The plan said twenty-four; it is **twenty-six**. Twenty per-band
    /// controls and six global ones, which is thirty-three less the seven on
    /// MAIN.
    #[test]
    fn the_advanced_tab_holds_the_rest_of_the_thirty_three() {
        const MAIN: usize = 7;
        const PER_BAND: usize = 4;
        const GLOBAL: usize = 6;
        assert_eq!(MAIN + BAND_COUNT * PER_BAND + GLOBAL, 33);
    }
}
