//! The Doubler's editor.
//!
//! Layout follows `plugins/doubler/docs/specifications/ui.md`. This is the macro
//! layer (`DBL-9`): the header, the four big knobs, tone, and the footer. The
//! Voice Field (`DBL-10`), the Filter View (`DBL-14`) and the Detail table
//! (`DBL-11`) land in their own units; their space is left framed but empty so
//! the proportions are already right.

mod field;
mod param_bind;
mod tone;

use crate::params::DoublerParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::theme;
use std::sync::Arc;

/// The two heights `ui.md` fixes: closed, and with the Detail table open. Only
/// the closed one is reachable until `DBL-11`.
const WIDTH: u32 = 620;
const HEIGHT: u32 = 500;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<DoublerParams>,
}

impl Model for Ui {}

pub fn create(params: Arc<DoublerParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            header(cx);
            field::view(cx);
            macros(cx);
            tone::view(cx);
            footer(cx);
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
        Label::new(
            cx,
            nih_plug_vizia::widgets::param_base::ParamWidgetBase::make_lens(
                Ui::params,
                to_param,
                |param| param.to_string(),
            ),
        )
        .class("value");
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
        // `DBL-11` turns this into the Detail disclosure.
        Label::new(cx, "DETAIL").class("subtle").width(Pixels(96.0));
        macro_knob(cx, "MIX", |params| &params.mix, 34.0);
        macro_knob(cx, "OUTPUT", |params| &params.output, 34.0);
    })
    .class("row")
    .height(Auto);
}
