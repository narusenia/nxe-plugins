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
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// One size, tall enough for whichever tab needs the most room. Measured
/// against the built layout rather than estimated.
const WIDTH: u32 = 620;
const HEIGHT: u32 = 584;

/// How tall the swapped region is. Fixed, so the two tabs do not move the
/// footer under the pointer when you switch.
const TAB_HEIGHT: f32 = 386.0;

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
}

pub(crate) enum UiEvent {
    SelectTab(usize),
    Hover(Option<usize>),
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
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);

            // Both tabs are built and one is hidden. Rebuilding on every switch
            // would drop the widgets' own state — a drag in progress, a hover —
            // for nothing.
            VStack::new(cx, |cx| {
                main_tab(cx);
                detail::view(cx);
            })
            .height(Pixels(TAB_HEIGHT))
            .width(Stretch(1.0));

            footer(cx);
        })
        .class("root")
        .child_space(Pixels(theme::SPACE_3))
        .row_between(Pixels(theme::SPACE_3));
    })
}

fn header(cx: &mut Context) {
    VStack::new(cx, |cx| {
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

        // The tab strip is a segmented control: exactly the same "one of these"
        // choice as `Voices`, so it is the same widget rather than a new one.
        HStack::new(cx, |cx| {
            SegmentedControl::new(cx, Ui::tab, &["MAIN", "DETAIL"], |cx, tab| {
                cx.emit(UiEvent::SelectTab(tab));
            });
        })
        .class("row")
        .height(Auto);
    })
    .height(Auto)
    .row_between(Pixels(theme::SPACE_2));
}

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        field::view(cx);
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

/// `Mix` and `Output` apply to everything, so they stay put rather than hiding
/// behind a tab.
fn footer(cx: &mut Context) {
    HStack::new(cx, |cx| {
        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        macro_knob(cx, "MIX", |params| &params.mix, 34.0);
        macro_knob(cx, "OUTPUT", |params| &params.output, 34.0);
    })
    .class("row")
    .height(Auto);
}
