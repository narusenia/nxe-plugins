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
use doubler_core::{MAX_VOICES, Voices, mirror_partner, pan_for};
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::{font, theme};
use param_bind::Mirror;

/// How much a row fades when its voice is not live.
const DIMMED: f32 = 0.42;

/// Rows are a fixed height, not `Auto`.
///
/// `Auto` height with stretched child space is circular in Morphorm — the
/// height depends on the children, and the stretch depends on the height — and
/// it hangs the layout rather than resolving to something wrong. A table wants
/// uniform rows anyway.
const ROW_HEIGHT: f32 = 22.0;

/// The padding the rows carry themselves.
///
/// The panel's own `child-space` is dropped: a table wants its rows flush to the
/// card's edges, so the top highlight is the card's top edge and a hovered row
/// is a band across the whole width rather than a floating strip.
const ROW_PAD: f32 = 12.0;

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

/// One column of the table.
///
/// Data rather than a pair of accessor closures: each accessor is its own
/// anonymous type, so passing "the parameter and its partner, or neither"
/// generically needs a type nobody can name. Matching here keeps every closure
/// concrete.
#[derive(Clone, Copy)]
enum Column {
    Delay,
    Detune,
    Pan,
    Gain,
}

const COLUMNS: [Column; 4] = [Column::Delay, Column::Detune, Column::Pan, Column::Gain];

impl Column {
    fn label(self) -> &'static str {
        match self {
            Column::Delay => "DELAY",
            Column::Detune => "DETUNE",
            Column::Pan => "PAN",
            Column::Gain => "GAIN",
        }
    }
}

/// What the number under a bar reads: the macro applied to the shape, which is
/// the value the DSP is actually using.
fn effective_text(params: &std::sync::Arc<DoublerParams>, index: usize, column: Column) -> String {
    let shape = &params.shape[index];
    match column {
        Column::Delay => format!("{:.1}", params.delay.value() * shape.delay.value()),
        Column::Detune => format!("{:+.1}", params.detune.value() * shape.detune.value()),
        Column::Pan => pan_text(pan_for(
            params.source.value().into(),
            params.spread.value(),
            shape.pan.value(),
            index,
        )),
        Column::Gain => format!("{:+.1}", shape.gain.value()),
    }
}

/// One cell: the bar, and the effective value under it.
///
/// Every column mirrors, each under its own switch (`REQ-DBL-014`). Leaning the
/// image on purpose — one voice quieter than its partner — is what turning the
/// `GAIN` switch off is for, rather than the column never mirroring.
fn cell(cx: &mut Context, index: usize, column: Column) {
    let partner = mirror_partner(index);

    HStack::new(cx, |cx| {
        let bar = match column {
            Column::Gain => param_bind::mirrored_bar(
                cx,
                Ui::params,
                Ui::mirror_gain,
                Mirror::Same,
                move |p| &p.shape[index].gain,
                move |p| &p.shape[partner].gain,
            ),
            Column::Delay => param_bind::mirrored_bar(
                cx,
                Ui::params,
                Ui::mirror_delay,
                Mirror::Same,
                move |p| &p.shape[index].delay,
                move |p| &p.shape[partner].delay,
            ),
            Column::Detune => param_bind::mirrored_bar(
                cx,
                Ui::params,
                Ui::mirror_detune,
                Mirror::Opposite,
                move |p| &p.shape[index].detune,
                move |p| &p.shape[partner].detune,
            ),
            Column::Pan => param_bind::mirrored_bar(
                cx,
                Ui::params,
                Ui::mirror_pan,
                Mirror::Opposite,
                move |p| &p.shape[index].pan,
                move |p| &p.shape[partner].pan,
            ),
        };
        bar.height(Pixels(BAR_HEIGHT)).width(Stretch(1.0));

        // The table's numbers are the **effective** values — the macro times the
        // shape — and nothing owns that product, so there is nothing to type
        // into. Editing happens on the bar beside it.
        font::value(
            cx,
            Ui::params.map(move |params| effective_text(params, index, column)),
        )
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

        for column in COLUMNS {
            cell(cx, index, column);
        }
    })
    .class("row")
    .height(Pixels(ROW_HEIGHT))
    .child_left(Pixels(ROW_PAD))
    .child_right(Pixels(ROW_PAD))
    // Dimming the whole row rather than each control keeps the bars and the
    // numbers consistent, and costs one modifier.
    .opacity(Ui::params.map(move |params| {
        if index < live_count(params) {
            1.0
        } else {
            DIMMED
        }
    }))
    // Both directions run through the same piece of state: whoever the pointer
    // is over sets it, and both the table and the figure read it. That is what
    // ties a row to a dot without numbering the dots.
    .on_hover(move |cx| cx.emit(super::UiEvent::Hover(Some(index))))
    .on_hover_out(|cx| cx.emit(super::UiEvent::Hover(None)))
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
            for column in COLUMNS {
                Label::new(cx, column.label())
                    .class("label")
                    .width(Stretch(1.0));
            }
        })
        .class("row")
        .height(Pixels(ROW_HEIGHT))
        .child_left(Pixels(ROW_PAD))
        .child_right(Pixels(ROW_PAD));

        for index in 0..MAX_VOICES {
            row(cx, index);
        }
    })
    .class("panel")
    .height(Auto)
    // A table, not a stack of cards: nine rows with the panel's 12 px between
    // them came to 339 px in a 296 px tab. The rows are a fixed height and
    // carry their own padding, so both go to zero here.
    .child_space(Pixels(0.0))
    .child_bottom(Pixels(theme::SPACE_1))
    .row_between(Pixels(0.0))
    .display(Ui::tab.map(|tab| *tab == super::TAB_DETAIL));
}
