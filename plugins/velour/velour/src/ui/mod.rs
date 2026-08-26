//! Velour's editor.
//!
//! Layout follows `plugins/velour/docs/specifications/ui.md`.
//!
//! **One fixed window size, and tabs inside it.** The Doubler learned this the
//! expensive way: asking a host to resize the editor on a disclosure toggle
//! wedged it in Ableton (`plugins/doubler/docs/implementation/doubler-plan.md`).
//! Tabs need nothing from the host for a control to become reachable.

mod field;
mod param_bind;

use crate::params::VelourParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// The starting point from `ui.md`: the Doubler's 620 × 572 plus the width of
/// the meter strip. **Settled by looking at it in a host**, the way the
/// Doubler's was.
const WIDTH: u32 = 680;
const HEIGHT: u32 = 580;

/// How tall the swapped region is. Fixed, so switching tabs does not move
/// anything above it.
const TAB_HEIGHT: f32 = 190.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

const TAB_MAIN: usize = 0;
const TAB_ADVANCED: usize = 1;

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<VelourParams>,
    /// Which tab is showing.
    ///
    /// **Interface state, not a parameter.** Which tab was open does not change
    /// the sound, so it is not worth an id in the saved state — reopening on
    /// MAIN is the right default anyway.
    tab: usize,
    /// Which band the pointer is over, so the Advanced row and the region can
    /// mark each other. One value, both directions — the Doubler's
    /// `Ui::hovered` (`plugins/doubler/docs/specifications/ui.md`).
    hovered: Option<usize>,
    /// The two analysis layers the figure draws behind the regions. Empty until
    /// `VEL-15` fills them, which is also what an idle track looks like.
    dry: Curve,
    wet: Curve,
}

pub(crate) enum UiEvent {
    SelectTab(usize),
    Hover(Option<usize>),
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::SelectTab(tab) => self.tab = *tab,
            UiEvent::Hover(index) => self.hovered = *index,
        });
    }
}

pub fn create(
    params: Arc<VelourParams>,
    state: Arc<ViziaState>,
    sample_rate: Arc<AtomicU32>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        // **Read once, when the window opens.** The rate decides where AIR's
        // upper edge is capped (`velour_core::bands::AIR_INPUT_CEILING`), and
        // that is the only thing on screen that depends on it. A host that
        // changes rate with the editor open leaves the figure an octave out at
        // the very top until it is reopened; polling for it every frame would be
        // work for a case that does not happen mid-session.
        let host_rate = f32::from_bits(sample_rate.load(Ordering::Relaxed));

        Ui {
            tab: TAB_MAIN,
            hovered: None,
            dry: Curve::new(),
            wet: Curve::new(),
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);
            // The figure stays put above the tabs. It is what the plugin *is* —
            // hiding it behind a tab would leave the window with nothing to look
            // at (`ui.md`).
            field::view(cx, host_rate);
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
        .class("root")
        .child_space(Pixels(theme::SPACE_3))
        .row_between(Pixels(theme::SPACE_3));
    })
}

fn header(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // The shipped name, in full — `NAME` in `lib.rs`, the bundle, and the
        // host's plugin list all say the same thing.
        Label::new(cx, "NXE VELOUR").class("title");
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

/// The knob sizes. The six that shape the sound are the large ones; the two
/// that decide how much of it arrives are smaller and sit apart, because they
/// are a different question (`ui.md`).
const SHAPE_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            macro_knob(cx, "DRIVE", "How hard the curves are driven", |params| {
                &params.drive
            });
            macro_knob(cx, "BODY", "Weight, low harmonics", |params| &params.body);
            macro_knob(cx, "PRESENCE", "Forward, midrange harmonics", |params| {
                &params.presence
            });
            macro_knob(cx, "AIR", "Sheen, top harmonics", |params| &params.air);
            texture_knob(cx);
            macro_knob(cx, "DENSITY", "Levels the texture, not the voice", |params| {
                &params.density
            });
        })
        .class("row")
        .height(Auto);

        // `MIX` and `OUTPUT` are not part of the shape: one decides how much of
        // it is heard and the other how loud the result is. Centred and smaller,
        // so the row above reads as the instrument and this one as the tap.
        HStack::new(cx, |cx| {
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
            knob_block(cx, "MIX", "Dry against the added texture", OUTPUT_KNOB, |params| {
                &params.mix
            });
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

/// `VEL-14` fills this in. It is built rather than left out so the tab strip
/// switches to something.
fn advanced_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Label::new(cx, "ADVANCED").class("subtle");
    })
    .height(Auto)
    .width(Stretch(1.0))
    .display(Ui::tab.map(|tab| *tab == TAB_ADVANCED));
}

/// One labelled knob with its value underneath: the shape every macro control
/// takes.
fn knob_block<P, F>(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    size: f32,
    to_param: F,
) where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<VelourParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        param_bind::knob(cx, Ui::params, to_param, size)
            .tooltip(move |cx| theme::hint(cx, hint));
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
    F: Fn(&Arc<VelourParams>) -> &P + Copy + 'static,
{
    knob_block(cx, label, hint, SHAPE_KNOB, to_param);
}

/// `TEXTURE`'s three anchors, and where they sit on the axis
/// (`velour_core::texture`).
const ANCHORS: [(&str, f32); 3] = [("WARM", 0.0), ("CLEAR", 0.5), ("EDGE", 1.0)];

/// `TEXTURE`, with the three names under it instead of a percentage.
///
/// **The names are what the discrete modes were traded for** (`REQ-VEL-004`).
/// The axis is continuous, so the readout has to say where between them the
/// knob is — and a number cannot, because 40% means nothing until you know what
/// is at either end. The nearest name lights up.
///
/// Not tick marks on the knob's own track: that would be a change to
/// `nxe_ui::knob` — and every widget change owes the gallery a panel
/// (`.agents/rules/vizia.md`). Three labels say the same thing from outside.
fn texture_knob(cx: &mut Context) {
    VStack::new(cx, |cx| {
        param_bind::knob(cx, Ui::params, |params| &params.texture, SHAPE_KNOB)
            .tooltip(|cx| theme::hint(cx, "Warm through Clear to Edge"));
        Label::new(cx, "TEXTURE").class("label");

        HStack::new(cx, |cx| {
            for (name, position) in ANCHORS {
                Label::new(cx, name)
                    .class("value")
                    .toggle_class(
                        "accent",
                        Ui::params.map(move |params| nearest(params.texture.value()) == position),
                    )
                    .toggle_class(
                        "subtle",
                        Ui::params.map(move |params| nearest(params.texture.value()) != position),
                    );
            }
        })
        .width(Auto)
        .height(Auto)
        .col_between(Pixels(theme::SPACE_1));
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
fn nearest(position: f32) -> f32 {
    ANCHORS
        .iter()
        .map(|(_, anchor)| *anchor)
        .min_by(|a, b| {
            (a - position)
                .abs()
                .partial_cmp(&(b - position).abs())
                .expect("the anchors and the position are finite")
        })
        .unwrap_or(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearest_anchor_is_the_nearest_one() {
        assert_eq!(nearest(0.0), 0.0);
        assert_eq!(nearest(0.2), 0.0);
        assert_eq!(nearest(0.5), 0.5);
        assert_eq!(nearest(0.8), 1.0);
        assert_eq!(nearest(1.0), 1.0);
        // The midpoints fall to the lower name rather than flickering.
        assert_eq!(nearest(0.25), 0.0);
        assert_eq!(nearest(0.75), 0.5);
    }
}
