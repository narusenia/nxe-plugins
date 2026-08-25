//! The Doubler's editor.
//!
//! Layout follows `plugins/doubler/docs/specifications/ui.md`. This is the macro
//! layer (`DBL-9`): the header, the four big knobs, tone, and the footer. The
//! Voice Field (`DBL-10`), the Filter View (`DBL-14`) and the Detail table
//! (`DBL-11`) land in their own units; their space is left framed but empty so
//! the proportions are already right.

mod detail;
mod field;
mod param_bind;
mod tone;

use crate::params::DoublerParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::GuiContextEvent;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::{font, icon, theme};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The two heights, closed and with the Detail table open.
///
/// Measured against the built layout rather than estimated: the numbers the
/// specification started with were too small and cut the footer off. If a
/// section changes height, these move with it — a window smaller than its
/// contents makes controls unreachable, not just untidy.
const WIDTH: u32 = 620;
const HEIGHT_CLOSED: u32 = 584;
const HEIGHT_OPEN: u32 = 892;

/// The size is a function of the plugin's own state, so reopening a project
/// restores the height the Detail toggle left behind.
pub fn default_state(detail_open: Arc<AtomicBool>) -> Arc<ViziaState> {
    ViziaState::new(move || {
        if detail_open.load(Ordering::Relaxed) {
            (WIDTH, HEIGHT_OPEN)
        } else {
            (WIDTH, HEIGHT_CLOSED)
        }
    })
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<DoublerParams>,
    /// The reactive copy of `params.detail_open`. An `AtomicBool` cannot be
    /// observed by a lens, so the display binds to this and the atomic follows.
    detail_open: bool,
    /// Which voice the pointer is over in the Voice Field, so the matching row
    /// can be highlighted. This is what stands in for numbering the dots
    /// (`plugins/doubler/docs/specifications/ui.md`).
    hovered: Option<usize>,
}

pub(crate) enum UiEvent {
    ToggleDetail,
    Hover(Option<usize>),
}

impl Model for Ui {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::ToggleDetail => {
                self.detail_open = !self.detail_open;
                self.params
                    .detail_open
                    .store(self.detail_open, Ordering::Relaxed);
                // Re-asks the size function, which now answers differently.
                cx.emit(GuiContextEvent::Resize);
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
            detail_open: params.detail_open.load(Ordering::Relaxed),
            hovered: None,
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);
            field::view(cx);
            macros(cx);
            tone::view(cx);
            footer(cx);
            detail::view(cx);
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
fn footer(cx: &mut Context) {
    HStack::new(cx, |cx| {
        HStack::new(cx, |cx| {
            icon::label(
                cx, // The chevron points the way the panel will move.
                "",
            )
            .bind(Ui::detail_open, |handle, open| {
                let glyph = if open.get(&handle) {
                    icon::CHEVRON_UP
                } else {
                    icon::CHEVRON_DOWN
                };
                handle.text(glyph);
            });
            Label::new(cx, "DETAIL").class("label");
        })
        .class("hoverable")
        .width(Pixels(96.0))
        .height(Pixels(22.0))
        .col_between(Pixels(theme::SPACE_1))
        .on_press(|cx| cx.emit(UiEvent::ToggleDetail));
        macro_knob(cx, "MIX", |params| &params.mix, 34.0);
        macro_knob(cx, "OUTPUT", |params| &params.output, 34.0);
    })
    .class("row")
    .height(Auto);
}
