//! The Detail table: the shape layer as eight rows of four bars.
//!
//! **The bar is the shape value and the number is the effective one**
//! (`plugins/doubler/docs/specifications/ui.md`). Dragging a bar edits the
//! shape; moving a macro leaves the bars still and changes every number. Both
//! layers stay visible and neither overwrites the other (`REQ-DBL-007`).
//!
//! A row whose voice is not live is dimmed but **still editable** — setting up
//! eight voices while running four is a reasonable thing to want.

use super::{Ui, param_bind};
use crate::params::DoublerParams;
use doubler_core::{MAX_VOICES, Voices, pan_for};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::{font, theme};

/// How much a row fades when its voice is not live.
const DIMMED: f32 = 0.42;

/// Rows are a fixed height, not `Auto`.
///
/// `Auto` height with stretched child space is circular in Morphorm — the
/// height depends on the children, and the stretch depends on the height — and
/// it hangs the layout rather than resolving to something wrong. A table wants
/// uniform rows anyway.
const ROW_HEIGHT: f32 = 22.0;

const INDEX_WIDTH: f32 = 18.0;
const BAR_HEIGHT: f32 = 10.0;
const VALUE_WIDTH: f32 = 62.0;

fn live_count(params: &DoublerParams) -> usize {
    Voices::count(params.voices.value().into())
}

/// `L70` / `C` / `R32`, the way a pan is read rather than as a signed number.
fn pan_text(pan: f32) -> String {
    let amount = (pan.abs() * 100.0).round() as i32;
    if amount == 0 {
        "C".to_owned()
    } else if pan < 0.0 {
        format!("L{amount}")
    } else {
        format!("R{amount}")
    }
}

/// One cell: the bar, and the effective value under it.
fn cell<P, F, V>(cx: &mut Context, to_param: F, centred: bool, effective: V)
where
    P: Param + 'static,
    F: Fn(&std::sync::Arc<DoublerParams>) -> &P + Copy + 'static,
    V: Fn(&std::sync::Arc<DoublerParams>) -> String + 'static,
{
    HStack::new(cx, |cx| {
        param_bind::bar(cx, Ui::params, to_param, centred)
            .height(Pixels(BAR_HEIGHT))
            .width(Stretch(1.0));
        font::value(cx, Ui::params.map(effective))
            .width(Pixels(VALUE_WIDTH))
            .child_left(Stretch(1.0));
    })
    .width(Stretch(1.0))
    .height(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_2));
}

fn row(cx: &mut Context, index: usize) {
    HStack::new(cx, |cx| {
        font::value(cx, &format!("{}", index + 1))
            .class("subtle")
            .width(Pixels(INDEX_WIDTH));

        cell(
            cx,
            move |params| &params.shape[index].delay,
            false,
            move |params| {
                format!(
                    "{:.1}",
                    params.delay.value() * params.shape[index].delay.value()
                )
            },
        );
        cell(
            cx,
            move |params| &params.shape[index].detune,
            true,
            move |params| {
                format!(
                    "{:+.1}",
                    params.detune.value() * params.shape[index].detune.value()
                )
            },
        );
        cell(
            cx,
            move |params| &params.shape[index].pan,
            true,
            move |params| {
                pan_text(pan_for(
                    params.source.value().into(),
                    params.spread.value(),
                    params.shape[index].pan.value(),
                    index,
                ))
            },
        );
        cell(
            cx,
            move |params| &params.shape[index].gain,
            false,
            move |params| format!("{:+.1}", params.shape[index].gain.value()),
        );
    })
    .class("row")
    .height(Pixels(ROW_HEIGHT))
    // Dimming the whole row rather than each control keeps the bars and the
    // numbers consistent, and costs one modifier.
    .opacity(Ui::params.map(move |params| {
        if index < live_count(params) {
            1.0
        } else {
            DIMMED
        }
    }))
    // The Voice Field reports which dot the pointer is over; this is the other
    // half of that, and what replaced numbering the dots.
    .background_color(Ui::hovered.map(move |hovered| {
        if *hovered == Some(index) {
            theme::ELEVATED.vizia()
        } else {
            theme::CARD.vizia()
        }
    }));
}

pub fn view(cx: &mut Context) {
    VStack::new(cx, |cx| {
        Element::new(cx).class("panel-highlight");

        HStack::new(cx, |cx| {
            Label::new(cx, "").class("label").width(Pixels(INDEX_WIDTH));
            for name in ["DELAY", "DETUNE", "PAN", "GAIN"] {
                Label::new(cx, name).class("label").width(Stretch(1.0));
            }
        })
        .class("row")
        .height(Pixels(ROW_HEIGHT));

        for index in 0..MAX_VOICES {
            row(cx, index);
        }
    })
    .class("panel")
    .height(Auto)
    .display(Ui::tab.map(|tab| *tab == super::TAB_DETAIL));
}
