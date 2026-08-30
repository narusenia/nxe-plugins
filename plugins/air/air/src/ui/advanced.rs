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
use nxe_ui::hint::Describe;
use nxe_ui::pictogram;
use nxe_ui::theme;

/// A row is one line: the control's name beside its bar.
const ROW_HEIGHT: f32 = theme::LINE_LABEL;
const BAR_HEIGHT: f32 = 10.0;
/// **Both grew in `AIR-14`.** The name column holds a mark now (`UI-17`), and
/// the bars took the rest: the two columns and the side left **252 px** of
/// nothing between them, the widest void of the five windows. A wider bar is a
/// finer one, and it is what is dragged.
const NAME_WIDTH: f32 = 76.0;
const BAR_WIDTH: f32 = 140.0;
const COLUMN_WIDTH: f32 = NAME_WIDTH + theme::SPACE_2 + BAR_WIDTH;

/// How wide a detector's live reading is, and the column that holds one.
///
/// **The three gauges were on the strip under the header** and moved here in
/// `AIR-14`: a reading of what `ENV` is doing belongs beside the `ENV`
/// deviation it explains, and seven cells plus a sentence do not fit on the
/// status bar (`ui/readout.rs`).
const GAUGE_WIDTH: f32 = 56.0;
const FOLLOW_COLUMN_WIDTH: f32 = COLUMN_WIDTH + theme::SPACE_2 + GAUGE_WIDTH;

/// The right-hand column, which is a knob and a two-way choice rather than
/// bars. **It is the taller of the two**, so it decides the panel's height.
const SIDE_HEIGHT: f32 = knob_block_height(OUTPUT_KNOB);

/// Wide enough for the eyebrow over it and for both segments under it.
const OVERSAMPLE_WIDTH: f32 = 104.0;
/// Wide enough for the word `OUTPUT` under the knob, which is wider than the
/// knob is.
const OUTPUT_WIDTH: f32 = 72.0;
const SIDE_WIDTH: f32 = OVERSAMPLE_WIDTH + theme::SPACE_4 + OUTPUT_WIDTH;

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
            settings_column(cx);
            follow_column(cx);
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
/// reach** (`REQ-AIR-010`) — and the protection's deviation.
///
/// **Three columns did not fit.** Two of 148 px, the gaps and the side column
/// come to 536 of the 628 the panel has; three came to 700, and the overflow
/// put the output knob underneath the meters. Every name here says what it is
/// on its own, so the column that holds them does not need one.
fn settings_column(cx: &mut Context) {
    column(cx, COLUMN_WIDTH, |cx| {
        row(cx, pictogram::DRIVE, "DRIVE", None, |cx| {
            bar(cx, "How hard the curve is driven", false, |params| {
                &params.drive
            })
        });
        row(cx, pictogram::TRIM, "BIAS", None, |cx| {
            bar(cx, "How far off centre the curve sits", false, |params| {
                &params.bias
            })
        });
        row(cx, pictogram::DE_HARSH, "GUARD", None, |cx| {
            bar(cx, "Down to nothing, up to stricter", true, |params| {
                &params.guard
            })
        });
    });
}

/// Each detector's depth **as a deviation from `FOLLOW`**. Zero is "as the
/// macro says", which is how two controls avoid writing one value
/// (`REQ-AIR-010`).
/// **All three carry the same mark.** They are one operation on three
/// detectors, and the marks name kinds rather than parameters
/// (`nxe_ui::pictogram`). Each row's gauge is that detector's live reading.
fn follow_column(cx: &mut Context) {
    column(cx, FOLLOW_COLUMN_WIDTH, |cx| {
        row(cx, pictogram::FOLLOW, "ENV", Some(0), |cx| {
            bar(cx, "Answer to how loud the input is", true, |params| {
                &params.follow_envelope
            })
        });
        row(cx, pictogram::FOLLOW, "BRT", Some(1), |cx| {
            bar(cx, "Answer to how bright the input is", true, |params| {
                &params.follow_brightness
            })
        });
        row(cx, pictogram::FOLLOW, "TRN", Some(2), |cx| {
            bar(cx, "Answer to attacks only", true, |params| {
                &params.follow_transient
            })
        });
    });
}

/// **Both halves are given a width.** `knob_block` asks for `Stretch(1.0)`, and
/// **a stretching child of an `Auto`-sized parent resolves to zero** — so the
/// knob, its label and its value all drew from the same origin, on top of the
/// segmented control beside them and out past the meters
/// (`.agents/rules/vizia.md`).
fn side(cx: &mut Context) {
    HStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            pictogram::heading(cx, pictogram::OVERSAMPLE, "OVERSAMPLE");
            HStack::new(cx, |cx| {
                nxe_plug_ui::segmented(cx, Ui::params, |params| &params.oversample, &["2x", "4x"])
                    .describe("2x is a saving, not an equal: 14 dB more folding");
            })
            .class("segmented")
            .width(Auto)
            .height(Auto);
        })
        .width(Pixels(OVERSAMPLE_WIDTH))
        .height(Auto)
        .row_between(Pixels(theme::SPACE_1));

        HStack::new(cx, |cx| {
            knob_block(cx, "OUTPUT", "Level out", OUTPUT_KNOB, |params| {
                &params.output
            });
        })
        .width(Pixels(OUTPUT_WIDTH))
        .height(Pixels(SIDE_HEIGHT));
    })
    // **Not `.class("row")`.** It carries `child-top: 1s` / `child-bottom: 1s`,
    // which are two more stretches for a fixed height to be divided among, and
    // both children here already have heights of their own
    // (`.agents/rules/vizia.md`).
    .width(Pixels(SIDE_WIDTH))
    .height(Pixels(SIDE_HEIGHT))
    .col_between(Pixels(theme::SPACE_4));
}

fn column(cx: &mut Context, width: f32, content: impl Fn(&mut Context)) {
    VStack::new(cx, |cx| content(cx))
        .width(Pixels(width))
        .height(Auto)
        .row_between(Pixels(theme::SPACE_2));
}

/// One labelled bar.
///
/// **The bar is given an explicit height by its caller.** A custom-drawn widget
/// with no size of its own collapses to a hairline inside an `Auto` row — it
/// still draws and still binds, and it cannot be hit. Velour's whole Advanced
/// table shipped that way (`.agents/rules/vizia.md`).
fn row(
    cx: &mut Context,
    glyph: pictogram::Glyph,
    name: &'static str,
    gauge: Option<usize>,
    build: impl Fn(&mut Context) + 'static,
) {
    HStack::new(cx, move |cx| {
        pictogram::label(cx, glyph, name).width(Pixels(NAME_WIDTH));
        // Wrapped so the column is a fixed width whatever the bar decides to
        // be, and so the bar sits on the line's centre rather than its top.
        HStack::new(cx, move |cx| build(cx))
            .width(Pixels(BAR_WIDTH))
            .height(Pixels(theme::LINE_LABEL))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));

        // The detector's own reading, beside the deviation it explains.
        if let Some(index) = gauge {
            HStack::new(cx, move |cx| {
                nxe_ui::meter::Meter::horizontal(cx, Ui::gauges.index(index), 0.0, Vec::new())
                    .width(Pixels(GAUGE_WIDTH))
                    .height(Pixels(theme::RULE_GAUGE));
            })
            .width(Pixels(GAUGE_WIDTH))
            .height(Pixels(theme::LINE_LABEL))
            .child_top(Stretch(1.0))
            .child_bottom(Stretch(1.0));
        }
    })
    // **Not `.class("row")`**: both children have a height of their own, and
    // the class's two stretches would divide what is left over between them
    // (`.agents/rules/vizia.md`).
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
        .describe(hint)
        .width(Stretch(1.0))
        .height(Pixels(BAR_HEIGHT));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::meters;

    /// How much width the panel actually has: the window, less the root's
    /// padding on both sides, less the meter strip and the gap before it.
    const AVAILABLE: f32 =
        super::super::WIDTH as f32 - theme::SPACE_3 * 2.0 - meters::WIDTH - theme::SPACE_3;

    /// **The fixed content has to fit, and a stretch cannot rescue it.**
    ///
    /// Morphorm does not shrink a `Pixels` child: past the edge it simply draws
    /// past the edge. Three bar columns came to 700 against 628 and the output
    /// knob ended up underneath the meters — **and nothing in the suite
    /// noticed**, because a layout that overflows still lays out
    /// (`.agents/rules/ui.md`).
    #[test]
    fn the_panel_fits_the_window() {
        // Two bar columns, the spacer, and the side: three gaps between four
        // children.
        let fixed = COLUMN_WIDTH + FOLLOW_COLUMN_WIDTH + SIDE_WIDTH + theme::SPACE_4 * 3.0;
        assert!(
            fixed <= AVAILABLE,
            "the advanced panel is {fixed} wide in a {AVAILABLE} space"
        );
    }

    /// The side column is the taller half, which is what the panel's height is
    /// derived from.
    #[test]
    fn the_side_is_what_sets_the_height() {
        const ROWS: f32 = 3.0;
        let bars = ROW_HEIGHT * ROWS + theme::SPACE_2 * (ROWS - 1.0);
        assert!(bars <= SIDE_HEIGHT, "{bars} against {SIDE_HEIGHT}");
        assert_eq!(HEIGHT, theme::LINE_EYEBROW + theme::SPACE_2 + SIDE_HEIGHT);
    }
}
