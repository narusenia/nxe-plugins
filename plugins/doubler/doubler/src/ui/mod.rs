//! The Doubler's editor.
//!
//! Layout follows `plugins/doubler/docs/specifications/ui.md`.
//!
//! **The window never resizes.** Asking the host to resize on a disclosure
//! toggle left the editor wedged in Ableton — the same layout works when the
//! window is opened at either size, and the gallery, which has no host and no
//! resize, is fine. So the sections are tabs inside one fixed window instead.
//! That also removes a whole class of question: no host has to agree to
//! anything for a control to become reachable.

mod detail;
mod field;
mod param_bind;
mod tone;

use crate::params::DoublerParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, icon, theme};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// One size, tall enough for whichever tab needs the most room. Measured
/// against the built layout rather than estimated.
const WIDTH: u32 = 620;
const HEIGHT: u32 = 572;

/// How tall the swapped region is. Fixed, so switching tabs does not move
/// anything above it.
const TAB_HEIGHT: f32 = 296.0;

/// The Voice Field's height, and the width of the column of global controls
/// beside it.
const FIELD_HEIGHT: f32 = 170.0;
const SIDE_WIDTH: f32 = 96.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

const TAB_MAIN: usize = 0;
const TAB_DETAIL: usize = 1;

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<DoublerParams>,
    /// Which tab is showing. The reactive copy of `params.detail_tab`: an
    /// `AtomicBool` cannot be observed by a lens, so the display binds to this
    /// and the atomic follows it for persistence.
    tab: usize,
    /// Which voice the pointer is over in the Voice Field, so the matching row
    /// can be highlighted. This is what stands in for numbering the dots
    /// (`plugins/doubler/docs/specifications/ui.md`).
    hovered: Option<usize>,
    /// Whether an edit mirrors onto the paired voice, per axis
    /// (`REQ-DBL-014`). The reactive copies of `params.mirror_*`, for the same
    /// reason `tab` is one.
    mirror_pan: bool,
    mirror_detune: bool,
    mirror_delay: bool,
    mirror_gain: bool,
}

/// Which axis a mirror switch controls — one per shape axis.
///
/// `Gain` is here because the figure's radius *is* the gain: without it,
/// dragging a point would visibly lean the image, which is the thing mirroring
/// exists to prevent. Leaning it on purpose is what turning this one off is for
/// (`REQ-DBL-014`).
#[derive(Clone, Copy)]
pub(crate) enum MirrorAxis {
    Pan,
    Detune,
    Delay,
    Gain,
}

pub(crate) enum UiEvent {
    SelectTab(usize),
    Hover(Option<usize>),
    ToggleMirror(MirrorAxis),
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::SelectTab(tab) => {
                self.tab = *tab;
                self.params
                    .detail_tab
                    .store(*tab == TAB_DETAIL, Ordering::Relaxed);
            }
            UiEvent::Hover(index) => self.hovered = *index,
            UiEvent::ToggleMirror(axis) => match axis {
                MirrorAxis::Pan => {
                    self.mirror_pan = !self.mirror_pan;
                    self.params
                        .mirror_pan
                        .store(self.mirror_pan, Ordering::Relaxed);
                }
                MirrorAxis::Detune => {
                    self.mirror_detune = !self.mirror_detune;
                    self.params
                        .mirror_detune
                        .store(self.mirror_detune, Ordering::Relaxed);
                }
                MirrorAxis::Delay => {
                    self.mirror_delay = !self.mirror_delay;
                    self.params
                        .mirror_delay
                        .store(self.mirror_delay, Ordering::Relaxed);
                }
                MirrorAxis::Gain => {
                    self.mirror_gain = !self.mirror_gain;
                    self.params
                        .mirror_gain
                        .store(self.mirror_gain, Ordering::Relaxed);
                }
            },
        });
    }
}

pub fn create(params: Arc<DoublerParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            tab: if params.detail_tab.load(Ordering::Relaxed) {
                TAB_DETAIL
            } else {
                TAB_MAIN
            },
            hovered: None,
            mirror_pan: params.mirror_pan.load(Ordering::Relaxed),
            mirror_detune: params.mirror_detune.load(Ordering::Relaxed),
            mirror_delay: params.mirror_delay.load(Ordering::Relaxed),
            mirror_gain: params.mirror_gain.load(Ordering::Relaxed),
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);
            // The figure stays put. It is what the plugin *is* — hiding it
            // behind a tab would leave the window with nothing to look at.
            field_row(cx);
            tab_strip(cx);

            // Both tabs are built and one is hidden. Rebuilding on every switch
            // would drop the widgets' own state — a drag in progress, a hover —
            // for nothing.
            VStack::new(cx, |cx| {
                main_tab(cx);
                detail::view(cx);
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
        Label::new(cx, "DOUBLER").class("value");
        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        param_bind::segmented(cx, Ui::params, |params| &params.voices, &["2", "4", "8"]);
        param_bind::segmented(
            cx,
            Ui::params,
            |params| &params.source,
            &["Mono Sum", "True Stereo"],
        );
    })
    .class("row")
    .height(Auto);
}

/// The figure, with the two controls that apply to everything stacked beside
/// it. `Mix` and `Output` belong next to what they act on, not under a tab.
fn field_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // The toggle sits over the figure's top corner rather than in the flow,
        // so turning mirroring on does not cost the figure any height.
        HStack::new(cx, |cx| {
            field::view(cx);
            mirror_switches(cx);
        })
        .width(Stretch(1.0))
        .height(Stretch(1.0));

        VStack::new(cx, |cx| {
            macro_knob(cx, "MIX", |params| &params.mix, 34.0);
            macro_knob(cx, "OUTPUT", |params| &params.output, 34.0);
        })
        .width(Pixels(SIDE_WIDTH))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0));
    })
    .height(Pixels(FIELD_HEIGHT))
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_3));
}

/// One mirror switch: a pill that is checked while that axis mirrors.
///
/// Not a new widget. `SegmentedControl` is `.segment` labels inside a
/// `.segmented` row, and one label on its own is the same thing — including
/// `:checked` recolouring it, which only works because the label carries the
/// class itself (`plugins/doubler/docs/implementation/doubler-plan.md`).
/// `UI-9`'s `ToggleSwitch` would be a sliding switch, which is not what belongs
/// beside a figure.
fn mirror_switch(
    cx: &mut Context,
    label: &'static str,
    on: impl Lens<Target = bool>,
    axis: MirrorAxis,
) {
    HStack::new(cx, |cx| {
        Label::new(cx, label)
            .class("segment")
            .checked(on)
            .on_press(move |cx| cx.emit(UiEvent::ToggleMirror(axis)));
    })
    .class("segmented");
}

/// The three mirror switches, over the figure's empty top corner.
///
/// **One pill per axis, not a master switch with exceptions.** A master plus a
/// per-axis opt-out would mean two pieces of state deciding one write, and
/// "mirror is on but delay is not mirroring" is a state a reader has to
/// reconstruct. Separate pills say it outright.
///
/// They are separate `.segmented` groups rather than one, because one group is
/// how this interface says "pick exactly one of these" everywhere else.
fn mirror_switches(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon::label(cx, icon::FLIP_HORIZONTAL_2);
        mirror_switch(cx, "PAN", Ui::mirror_pan, MirrorAxis::Pan);
        mirror_switch(cx, "DETUNE", Ui::mirror_detune, MirrorAxis::Detune);
        mirror_switch(cx, "DELAY", Ui::mirror_delay, MirrorAxis::Delay);
        mirror_switch(cx, "GAIN", Ui::mirror_gain, MirrorAxis::Gain);
    })
    .position_type(PositionType::SelfDirected)
    // Stretching the space to its left is how Morphorm right-aligns something
    // that sizes to its content. Both axes need saying: an unset size is
    // `Stretch(1.0)`, not "size to content" (`.agents/rules/vizia.md`).
    .left(Stretch(1.0))
    .top(Pixels(0.0))
    .width(Auto)
    .height(Auto)
    .col_between(Pixels(theme::SPACE_1))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

/// The tab strip is a segmented control: exactly the same "one of these" choice
/// as `Voices`, so it is the same widget rather than a new one.
fn tab_strip(cx: &mut Context) {
    HStack::new(cx, |cx| {
        SegmentedControl::new(cx, Ui::tab, &["MAIN", "DETAIL"], |cx, tab| {
            cx.emit(UiEvent::SelectTab(tab));
        });
    })
    .class("row")
    .height(Auto);
}

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        macros(cx);
        tone::view(cx);
    })
    .height(Auto)
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_3))
    .display(Ui::tab.map(|tab| *tab == TAB_MAIN));
}

/// One labelled knob with its value underneath, which is the shape every macro
/// control takes.
pub(crate) fn macro_knob<P, F>(cx: &mut Context, label: &'static str, to_param: F, size: f32)
where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<DoublerParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        param_bind::knob(cx, Ui::params, to_param, size);
        Label::new(cx, label).class("label");
        font::value(
            cx,
            nih_plug_vizia::widgets::param_base::ParamWidgetBase::make_lens(
                Ui::params,
                to_param,
                |param| param.to_string(),
            ),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn macros(cx: &mut Context) {
    HStack::new(cx, |cx| {
        macro_knob(cx, "DETUNE", |params| &params.detune, 56.0);
        macro_knob(cx, "DELAY", |params| &params.delay, 56.0);
        macro_knob(cx, "SPREAD", |params| &params.spread, 56.0);
        macro_knob(cx, "HUMANIZE", |params| &params.humanize, 56.0);
    })
    .class("row")
    .height(Auto);
}
