//! The controls that are not asked every time: the curve's own numbers, each
//! detector's deviation from `FOLLOW`, the protection's deviation, and the two
//! output-side choices.
//!
//! **Not behind a tab** (`REQ-AIR-013`, `SPK-19`). Sparkleur's tabs hid
//! seventeen of thirty-five controls behind a click; fifteen fit on one screen,
//! and a deviation that cannot be seen beside the macro it deviates from is a
//! deviation from nothing.

use super::{OUTPUT_KNOB, Ui, knob_block, knob_block_height};
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::theme;

/// A row is one line: the control's name beside its bar.
const ROW_HEIGHT: f32 = theme::LINE_LABEL;
const BAR_HEIGHT: f32 = 10.0;
const NAME_WIDTH: f32 = 56.0;
const BAR_WIDTH: f32 = 84.0;
const COLUMN_WIDTH: f32 = NAME_WIDTH + theme::SPACE_2 + BAR_WIDTH;

/// The right-hand column, which is a knob and a two-way choice rather than
/// bars. **It is the taller of the two**, so it decides the panel's height.
const SIDE_HEIGHT: f32 = knob_block_height(OUTPUT_KNOB);

/// How tall the whole panel is.
///
/// **Part of the window's height, so it is arithmetic** (`.agents/rules/ui.md`).
pub const HEIGHT: f32 = theme::LINE_EYEBROW + theme::SPACE_2 + SIDE_HEIGHT;

pub fn view(cx: &mut Context) {
    VStack::new(cx, |cx| {
        // **A region's name is an eyebrow over a rule**, not a label: at label
        // size it joins the row of control names under it and stops reading as
        // structure (`crates/nxe-ui/README.md`).
        Label::new(cx, "ADVANCED")
            .class("eyebrow")
            .height(Pixels(theme::LINE_EYEBROW));

        HStack::new(cx, |cx| {
            curve_column(cx);
            follow_column(cx);
            protect_column(cx);
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
            side(cx);
        })
        .height(Pixels(SIDE_HEIGHT))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_4));
    })
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_2));
}

/// The curve's own two numbers — **the ones `CHARACTER` deliberately does not
/// reach** (`REQ-AIR-010`).
fn curve_column(cx: &mut Context) {
    column(cx, |cx| {
        row(cx, "DRIVE", |cx| {
            bar(cx, "How hard the curve is driven", false, |params| {
                &params.drive
            })
        });
        row(cx, "BIAS", |cx| {
            bar(cx, "How far off centre the curve sits", false, |params| {
                &params.bias
            })
        });
    });
}

/// Each detector's depth **as a deviation from `FOLLOW`**. Zero is "as the
/// macro says", which is how two controls avoid writing one value
/// (`REQ-AIR-010`).
fn follow_column(cx: &mut Context) {
    column(cx, |cx| {
        row(cx, "ENV", |cx| {
            bar(cx, "Answer to how loud the input is", true, |params| {
                &params.follow_envelope
            })
        });
        row(cx, "BRT", |cx| {
            bar(cx, "Answer to how bright the input is", true, |params| {
                &params.follow_brightness
            })
        });
        row(cx, "TRN", |cx| {
            bar(cx, "Answer to attacks only", true, |params| {
                &params.follow_transient
            })
        });
    });
}

fn protect_column(cx: &mut Context) {
    column(cx, |cx| {
        row(cx, "GUARD", |cx| {
            bar(cx, "Down to nothing, up to stricter", true, |params| {
                &params.guard
            })
        });
    });
}

fn side(cx: &mut Context) {
    HStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            Label::new(cx, "OVERSAMPLE")
                .class("eyebrow")
                .height(Pixels(theme::LINE_EYEBROW));
            HStack::new(cx, |cx| {
                nxe_plug_ui::segmented(cx, Ui::params, |params| &params.oversample, &["2x", "4x"])
                    .tooltip(|cx| {
                        theme::hint(cx, "2x is a saving, not an equal: 14 dB more folding")
                    });
            })
            .class("segmented")
            .width(Auto)
            .height(Auto);
        })
        .width(Auto)
        .height(Auto)
        .row_between(Pixels(theme::SPACE_1));

        knob_block(cx, "OUTPUT", "Level out", OUTPUT_KNOB, |params| {
            &params.output
        });
    })
    .class("row")
    .width(Auto)
    .height(Pixels(SIDE_HEIGHT))
    .col_between(Pixels(theme::SPACE_4));
}

fn column(cx: &mut Context, content: impl Fn(&mut Context)) {
    VStack::new(cx, |cx| content(cx))
        .width(Pixels(COLUMN_WIDTH))
        .height(Auto)
        .row_between(Pixels(theme::SPACE_2));
}

/// One labelled bar.
///
/// **The bar is given an explicit height by its caller.** A custom-drawn widget
/// with no size of its own collapses to a hairline inside an `Auto` row — it
/// still draws and still binds, and it cannot be hit. Velour's whole Advanced
/// table shipped that way (`.agents/rules/vizia.md`).
fn row(cx: &mut Context, name: &'static str, build: impl Fn(&mut Context) + 'static) {
    HStack::new(cx, move |cx| {
        Label::new(cx, name)
            .class("label")
            .width(Pixels(NAME_WIDTH))
            .height(Pixels(theme::LINE_LABEL));
        // Wrapped so the column is a fixed width whatever the bar decides to
        // be, and so the bar sits on the line's centre rather than its top.
        HStack::new(cx, move |cx| build(cx))
            .width(Pixels(BAR_WIDTH))
            .height(Pixels(theme::LINE_LABEL))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
    })
    .class("row")
    .width(Stretch(1.0))
    .height(Pixels(ROW_HEIGHT))
    .col_between(Pixels(theme::SPACE_2));
}

/// A bar bound to one parameter, sized and explained.
fn bar<P, F>(cx: &mut Context, hint: &'static str, bipolar: bool, to_param: F)
where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&std::sync::Arc<crate::params::AirParams>) -> &P + Copy + 'static,
{
    nxe_plug_ui::bar(cx, Ui::params, to_param, bipolar)
        .tooltip(move |cx| theme::hint(cx, hint))
        .width(Stretch(1.0))
        .height(Pixels(BAR_HEIGHT));
}
