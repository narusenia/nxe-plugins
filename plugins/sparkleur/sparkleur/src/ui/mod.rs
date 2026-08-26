//! Sparkleur's editor.
//!
//! Layout follows `plugins/sparkleur/docs/specifications/ui.md`.
//!
//! **One fixed window size, and tabs inside it.** The Doubler learned this the
//! expensive way: asking a host to resize the editor on a disclosure toggle
//! wedged it in Ableton. Tabs need nothing from the host for a control to
//! become reachable.
//!
//! **`SPK-12` is the macro layer only.** The Band Field, the transfer window
//! and the meters arrive in `SPK-13` and `SPK-14`, and the Advanced table in
//! `SPK-15`; until then the space above the tabs is empty rather than filled
//! with something that pretends to be them.

use crate::params::SparkleurParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, theme};
use std::sync::Arc;

/// Velour's window, which `ui.md` says to start from. **Confirmed on real
/// hardware, not here** — Velour began at 580 tall and came down to 528 once
/// it was looked at, because everything inside is a fixed height and a window
/// set too tall ends in a band of nothing.
const WIDTH: u32 = 680;
const HEIGHT: u32 = 528;

/// How tall the swapped region is. Fixed, so switching tabs does not move
/// anything above it.
///
/// Sized to MAIN, which is the taller of the two: a 52 px knob with its label
/// and value is 88, the second row of smaller knobs is 74, and 12 between them.
const TAB_HEIGHT: f32 = 180.0;

/// The knob sizes. The five that shape the sound are the large ones; the two
/// that decide how much of it arrives are smaller and sit apart, because they
/// are a different question (`ui.md`).
const SHAPE_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

/// The named points on the `CHARACTER` axis and where they sit
/// (`sparkleur_core::character`).
const ANCHORS: [(&str, f32); 3] = [("POLISH", 0.0), ("GLOSS", 0.5), ("CRUSH", 1.0)];

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

const TAB_MAIN: usize = 0;
const TAB_ADVANCED: usize = 1;

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<SparkleurParams>,
    /// Which tab is showing.
    ///
    /// **Interface state, not a parameter.** Which tab was open does not change
    /// the sound, so it is not worth an id in the saved state — reopening on
    /// MAIN is the right default anyway.
    tab: usize,
}

pub(crate) enum UiEvent {
    SelectTab(usize),
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::SelectTab(tab) => self.tab = *tab,
        });
    }
}

pub fn create(params: Arc<SparkleurParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            params: params.clone(),
            tab: TAB_MAIN,
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);

            // Where the figure goes (`SPK-13`, `SPK-14`). Left empty on
            // purpose: a placeholder that draws something is a picture nobody
            // can tell from a finished one.
            Element::new(cx).width(Stretch(1.0)).height(Stretch(1.0));

            tab_strip(cx);

            // Both tabs are built and one is hidden. Rebuilding on a switch
            // would drop the widgets' own state — a drag in progress, a hover —
            // for nothing.
            VStack::new(cx, |cx| {
                main_tab(cx);
                advanced_tab(cx);
            })
            .height(Pixels(TAB_HEIGHT))
            .width(Stretch(1.0));
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3))
        .child_space(Pixels(theme::SPACE_3));
    })
}

fn header(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // The shipped name, in full — `NAME` in `lib.rs`, the bundle, and the
        // host's plugin list all say the same thing.
        font::title(cx, "NXE SPARKLEUR");
    })
    .class("row")
    .height(Auto);
}

/// The tab strip is a segmented control: the same "one of these" choice as
/// everywhere else in this design, so it is the same widget.
fn tab_strip(cx: &mut Context) {
    HStack::new(cx, |cx| {
        SegmentedControl::new(cx, Ui::tab, &["MAIN", "ADVANCED"], |cx, tab| {
            cx.emit(UiEvent::SelectTab(tab));
        });
    })
    .class("row")
    .height(Auto);
}

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            macro_knob(cx, "SPARK", "How much of everything", |params| {
                &params.spark
            });
            character_knob(cx);
            macro_knob(cx, "BODY", "Lean on the low mids", |params| &params.body);
            macro_knob(cx, "AIR", "Lean on the top, and the sparkle", |params| {
                &params.air
            });
            macro_knob(
                cx,
                "SPEED",
                "Faster or slower than the character",
                |params| &params.speed,
            );
        })
        .class("row")
        .height(Auto);

        // `MIX` and `OUTPUT` are not part of the shape: one decides how much of
        // it is heard and the other how loud the result is. Centred and
        // smaller, so the row above reads as the instrument and this one as the
        // tap.
        HStack::new(cx, |cx| {
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
            knob_block(
                cx,
                "MIX",
                "Dry against the processed signal",
                OUTPUT_KNOB,
                |params| &params.mix,
            );
            knob_block(cx, "OUTPUT", "Level out", OUTPUT_KNOB, |params| {
                &params.output
            });
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        })
        .class("row")
        .height(Auto);
    })
    .height(Auto)
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_3))
    .display(Ui::tab.map(|tab| *tab == TAB_MAIN));
}

fn advanced_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        // `SPK-15`. Empty rather than stubbed, for the same reason as the
        // figure.
        Label::new(cx, "ADVANCED").class("label");
    })
    .height(Auto)
    .width(Stretch(1.0))
    .display(Ui::tab.map(|tab| *tab == TAB_ADVANCED));
}

/// One labelled knob with its value underneath: the shape every macro control
/// takes.
pub(crate) fn knob_block<P, F>(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    size: f32,
    to_param: F,
) where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<SparkleurParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        nxe_plug_ui::knob(cx, Ui::params, to_param, size).tooltip(move |cx| theme::hint(cx, hint));
        Label::new(cx, label).class("label");
        font::value(
            cx,
            ParamWidgetBase::make_lens(Ui::params, to_param, |param| param.to_string()),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn macro_knob<P, F>(cx: &mut Context, label: &'static str, hint: &'static str, to_param: F)
where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<SparkleurParams>) -> &P + Copy + 'static,
{
    knob_block(cx, label, hint, SHAPE_KNOB, to_param);
}

/// `CHARACTER` reads as **the nearest anchor and a percentage**, on one line.
///
/// Velour tried three names side by side and it failed three ways — the accent
/// was a fill rather than a weight, the selected one changed size, and the
/// column they needed clipped the knob beside it (`ui.md`). One line says the
/// same thing and cannot do any of that.
fn character_knob(cx: &mut Context) {
    VStack::new(cx, |cx| {
        nxe_plug_ui::knob(cx, Ui::params, |params| &params.character, SHAPE_KNOB)
            .tooltip(|cx| theme::hint(cx, "POLISH through GLOSS to CRUSH"));
        Label::new(cx, "CHARACTER").class("label");
        font::value(
            cx,
            Ui::params.map(|params| {
                format!("{} {}", nearest(params.character.value()), params.character)
            }),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

/// Which anchor a position is closest to. Ties cannot happen: the anchors are
/// evenly spaced, so the midpoints are the only ambiguous values and they fall
/// to the lower name.
fn nearest(position: f32) -> &'static str {
    ANCHORS
        .iter()
        .min_by(|(_, a), (_, b)| {
            (a - position)
                .abs()
                .partial_cmp(&(b - position).abs())
                .expect("the anchors and the position are finite")
        })
        .map(|(name, _)| *name)
        .unwrap_or("GLOSS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_anchor_is_the_nearest_one() {
        assert_eq!(nearest(0.0), "POLISH");
        assert_eq!(nearest(0.2), "POLISH");
        assert_eq!(nearest(0.5), "GLOSS");
        assert_eq!(nearest(0.8), "CRUSH");
        assert_eq!(nearest(1.0), "CRUSH");
        // The midpoints fall to the lower name rather than flickering between
        // two as a knob is dragged through them.
        assert_eq!(nearest(0.25), "POLISH");
        assert_eq!(nearest(0.75), "GLOSS");
    }

    /// **The default reads "GLOSS 27 %", not "POLISH"** — and that is worth
    /// knowing rather than asserting away.
    ///
    /// `REQ-SPK-006` puts the default at 0.25–0.30, "toward POLISH". By the
    /// readout's own rule the nearest anchor to 0.27 is GLOSS (0.23 away
    /// against POLISH's 0.27), so the name over-claims a little. Both halves
    /// are as specified; they were specified separately. **`SPK-18` decides**
    /// which one moves — the default under 0.25, or the readout naming the
    /// anchor it is walking away from.
    #[test]
    fn the_default_reads_as_the_nearer_anchor_even_when_it_leans_the_other_way() {
        let default = sparkleur_core::character::DEFAULT_POSITION;
        assert_eq!(nearest(default), "GLOSS");
        // It is genuinely nearer, which is the whole of the oddity.
        assert!((default - 0.5).abs() < default);
    }

    /// The anchors are the axis's, not a second copy of them.
    #[test]
    fn the_anchors_match_the_axis() {
        for (_, position) in ANCHORS {
            let character = sparkleur_core::character::at(position);
            assert!(character.curve.down_ratio.is_finite());
        }
        // POLISH is gentler than CRUSH, which is what the names mean.
        assert!(
            sparkleur_core::character::at(ANCHORS[0].1).curve.down_ratio
                < sparkleur_core::character::at(ANCHORS[2].1).curve.down_ratio
        );
    }
}
