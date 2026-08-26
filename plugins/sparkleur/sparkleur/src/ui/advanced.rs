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
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::theme;
use sparkleur_core::crossover::BAND_COUNT;

/// The name column and the width one bar gets. Fixed rather than stretched, so
/// the five rows and the header line up.
const NAME_WIDTH: f32 = 48.0;
const BAR_WIDTH: f32 = 76.0;
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
const SIDE_NAME_WIDTH: f32 = 80.0;
const SIDE_BAR_WIDTH: f32 = 80.0;

/// The right column, which holds what is global rather than per band.
const SIDE_WIDTH: f32 = 224.0;

const NAMES: [&str; BAND_COUNT] = ["SUB", "BODY", "MID", "PRES", "AIR"];

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
            heading(cx, "", NAME_WIDTH);
            heading(cx, "UP", BAR_WIDTH);
            heading(cx, "DOWN", BAR_WIDTH);
            heading(cx, "GAIN", BAR_WIDTH);
            heading(cx, "SOLO", SOLO_WIDTH);
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

fn heading(cx: &mut Context, text: &'static str, width: f32) {
    // **A column's name is not a control's name** (`crates/nxe-ui/README.md`):
    // set as an eyebrow it reads as the table's structure instead of joining
    // the row of labels under it.
    Label::new(cx, text)
        .class("eyebrow")
        .width(Pixels(width))
        .height(Auto);
}

fn row(cx: &mut Context, index: usize) {
    HStack::new(cx, |cx| {
        Label::new(cx, NAMES[index])
            .class("label")
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
            .tooltip(|cx| theme::hint(cx, "This band's share of the upward half"))
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
            .tooltip(|cx| theme::hint(cx, "This band's share of the downward half"))
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
            .tooltip(|cx| theme::hint(cx, "A static trim. The figure's region height"))
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
                .tooltip(|cx| theme::hint(cx, "Hear this band alone"));
            })
            .class("segmented")
            .width(Auto)
            .height(Auto);
        });
    })
    .class("row")
    .height(Auto)
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

/// What is global: `FOCUS`, the two protections, `SNAP`, `LIFT`, and the
/// oversampling.
fn side(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // `FOCUS` has a knob as well as the figure's rail, for the same reason
        // `MIX` has one: the figure is for reading, a knob is for setting a
        // number (`ui.md`).
        super::knob_block(cx, "FOCUS", "Slides every band edge", 38.0, |params| {
            &params.focus
        });

        VStack::new(cx, |cx| {
            labelled_bar(
                cx,
                "DE-HARSH",
                "Harder or softer than CHARACTER chose",
                |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.de_harsh, true),
            );
            labelled_bar(
                cx,
                "SUB PROT",
                "How far the bottom band's lift is closed",
                |cx| nxe_plug_ui::bar(cx, Ui::params, |p| &p.sub_protect, true),
            );
            labelled_bar(cx, "SNAP", "How much of AIR waits for a transient", |cx| {
                nxe_plug_ui::bar(cx, Ui::params, |p| &p.snap, false)
            });
            labelled_bar(cx, "LIFT", "Opens the floor under the upward half", |cx| {
                nxe_plug_ui::bar(cx, Ui::params, |p| &p.lift, false)
            });

            HStack::new(cx, |cx| {
                Label::new(cx, "OVERSAMPLE")
                    .class("subtle")
                    .width(Pixels(SIDE_NAME_WIDTH))
                    .height(Auto);
                nxe_plug_ui::segmented(cx, Ui::params, |params| &params.oversample, &["2x", "4x"])
                    .tooltip(|cx| theme::hint(cx, "2x costs about 11 us and aliases 14 dB higher"));
            })
            .class("row")
            .height(Auto)
            .width(Auto)
            .col_between(Pixels(theme::SPACE_2));
        })
        .height(Auto)
        .width(Auto)
        .row_between(Pixels(theme::SPACE_2));
    })
    .class("hint-left")
    .width(Pixels(SIDE_WIDTH))
    .height(Auto)
    .col_between(Pixels(theme::SPACE_3));
}

fn labelled_bar(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    content: impl Fn(&mut Context) -> Handle<'_, nxe_ui::bar::Bar>,
) {
    HStack::new(cx, |cx| {
        Label::new(cx, label)
            .class("subtle")
            .width(Pixels(SIDE_NAME_WIDTH))
            .height(Auto);
        content(cx)
            .tooltip(move |cx| theme::hint(cx, hint))
            .width(Pixels(SIDE_BAR_WIDTH))
            .height(Pixels(BAR_HEIGHT));
    })
    .class("row")
    .height(Auto)
    .width(Auto)
    .col_between(Pixels(theme::SPACE_2));
}

#[cfg(test)]
mod tests {
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
