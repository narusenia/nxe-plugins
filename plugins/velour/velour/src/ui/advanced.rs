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
use nxe_ui::theme;
use velour_core::BAND_COUNT;

/// The name column, and the width one bar gets. Fixed rather than stretched, so
/// the three rows and the header line up.
const NAME_WIDTH: f32 = 64.0;
const BAR_WIDTH: f32 = 104.0;
const SOLO_WIDTH: f32 = 44.0;

/// **A bar has no default height** — an unset one is `Stretch(1.0)`, and a
/// stretching child of an `Auto`-sized parent resolves to nothing
/// (`.agents/rules/vizia.md`). Every bar here was a hairline, and a hairline
/// cannot be grabbed: the rows read as controls that did nothing. The gallery
/// sets this on every bar it shows, which is where the number comes from.
const BAR_HEIGHT: f32 = 10.0;

/// The right-hand column's own widths. `OVERSAMPLE` does not fit in
/// [`NAME_WIDTH`] — it was clipped to `OVERSAMPL` — and the sixteen pixels come
/// out of the bar rather than out of the window, so the column's total is
/// unchanged and the bars stay aligned with the segmented control under them.
const SIDE_NAME_WIDTH: f32 = 80.0;
const SIDE_BAR_WIDTH: f32 = 88.0;

/// The right column, which holds what is global rather than per band.
const SIDE_WIDTH: f32 = 232.0;

const NAMES: [&str; BAND_COUNT] = ["BODY", "PRES", "AIR"];

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
            heading(cx, "", NAME_WIDTH);
            heading(cx, "BIAS", BAR_WIDTH);
            heading(cx, "TEXTURE", BAR_WIDTH);
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
    Label::new(cx, text)
        .class("subtle")
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
                0 => super::param_bind::bar(cx, Ui::params, |params| &params.bias_body, true),
                1 => super::param_bind::bar(cx, Ui::params, |params| &params.bias_presence, true),
                _ => super::param_bind::bar(cx, Ui::params, |params| &params.bias_air, true),
            }
            .tooltip(|cx| theme::hint(cx, "Deeper curve added quieter, or the reverse"))
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, BAR_WIDTH, move |cx| {
            match index {
                0 => super::param_bind::bar(cx, Ui::params, |params| &params.texture_body, true),
                1 => {
                    super::param_bind::bar(cx, Ui::params, |params| &params.texture_presence, true)
                }
                _ => super::param_bind::bar(cx, Ui::params, |params| &params.texture_air, true),
            }
            .tooltip(|cx| theme::hint(cx, "This band's deviation from TEXTURE"))
            .width(Stretch(1.0))
            .height(Pixels(BAR_HEIGHT));
        });

        cell(cx, SOLO_WIDTH, move |cx| {
            HStack::new(cx, |cx| {
                match index {
                    0 => super::param_bind::toggle(cx, Ui::params, |p| &p.solo_body, "ON"),
                    1 => super::param_bind::toggle(cx, Ui::params, |p| &p.solo_presence, "ON"),
                    _ => super::param_bind::toggle(cx, Ui::params, |p| &p.solo_air, "ON"),
                }
                .tooltip(|cx| theme::hint(cx, "Hear this band's layer alone, dry muted"));
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

/// What is global: `FOCUS`, the two guards, `EMOTION`, and the oversampling.
fn side(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // `FOCUS` has a knob as well as the figure's rail, for the same reason
        // `MIX` has one: the figure is for reading, a knob is for setting a
        // number (`ui.md`).
        super::knob_block(cx, "FOCUS", "Slides every band edge", 38.0, |params| {
            &params.focus
        });

        VStack::new(cx, |cx| {
            labelled_bar(cx, "HARSH", "How far the harsh guard may pull", |cx| {
                super::param_bind::bar(cx, Ui::params, |params| &params.guard_harsh, false)
            });
            labelled_bar(cx, "SIB", "How far the sibilance guard may pull", |cx| {
                super::param_bind::bar(cx, Ui::params, |params| &params.guard_sib, false)
            });
            labelled_bar(
                cx,
                "EMOTION",
                "How much the singing moves the curves",
                |cx| super::param_bind::bar(cx, Ui::params, |params| &params.emotion, false),
            );

            HStack::new(cx, |cx| {
                Label::new(cx, "OVERSAMPLE")
                    .class("subtle")
                    .width(Pixels(SIDE_NAME_WIDTH))
                    .height(Auto);
                super::param_bind::segmented(
                    cx,
                    Ui::params,
                    |params| &params.oversample,
                    &["2x", "4x"],
                )
                .tooltip(|cx| theme::hint(cx, "2x costs less and aliases about 14 dB higher"));
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
