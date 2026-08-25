//! The Filter View: the wet bus's Tone as a curve you grab.
//!
//! The curve comes from `doubler_core::tone_response_db`, which builds the same
//! shelves the audio goes through — the picture cannot drift from the sound
//! (`plugins/doubler/docs/specifications/ui.md`).
//!
//! **The two shelves have no knobs.** They are the handles on the curve. Their
//! frequencies are fixed, so a vertical drag is the whole gesture, and the shape
//! being edited is visible while editing it.

use super::{Ui, macro_knob};
use crate::params::DoublerParams;
use doubler_core::{MAX_VOICES, spread_band, tone_response_db};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::curve::{Curve, CurveView, Grip, Span};
use nxe_ui::input::Gesture;
use nxe_ui::theme;

/// The axis the caller owns, because the widget cannot label its own gridlines.
const LOW_HZ: f32 = 20.0;
const HIGH_HZ: f32 = 20_000.0;

/// The shelf frequencies, which are where the handles sit.
const SHELF_LOW_HZ: f32 = 200.0;
const SHELF_HIGH_HZ: f32 = 4_000.0;

/// The curve is drawn against ±12 dB, the same range the parameters have, so a
/// handle's height *is* its parameter's normalized value.
const RANGE_DB: f32 = 12.0;

/// How many points the curve is sampled at. Enough that a shelf's knee looks
/// like a curve rather than a corner.
const RESOLUTION: usize = 96;

/// The response is evaluated at this rate for display. The real rate only moves
/// the curve by a fraction of a dB at these frequencies, and plumbing it into
/// the editor would buy nothing a reader could see.
const DISPLAY_SAMPLE_RATE: f32 = 48_000.0;

const MARKS: [(f32, &str); 5] = [
    (20.0, "20"),
    (200.0, "200"),
    (1_000.0, "1k"),
    (5_000.0, "5k"),
    (20_000.0, "20k"),
];

/// Hz onto `0..=1` across the view, logarithmically.
fn axis_x(hz: f32) -> f32 {
    (hz / LOW_HZ).log10() / (HIGH_HZ / LOW_HZ).log10()
}

fn hz_at(x: f32) -> f32 {
    LOW_HZ * 10.0f32.powf(x * (HIGH_HZ / LOW_HZ).log10())
}

/// dB onto `0..=1` with the resting line at the centre.
fn axis_y(db: f32) -> f32 {
    (0.5 + db / (RANGE_DB * 2.0)).clamp(0.0, 1.0)
}

fn curve_of(params: &DoublerParams) -> Vec<Curve> {
    let low = params.tone_lo.value();
    let high = params.tone_hi.value();

    vec![
        (0..=RESOLUTION)
            .map(|step| {
                let x = step as f32 / RESOLUTION as f32;
                let db = tone_response_db(low, high, hz_at(x), DISPLAY_SAMPLE_RATE);
                (x, axis_y(db))
            })
            .collect(),
    ]
}

fn spans_of(params: &DoublerParams) -> Vec<Span> {
    let spread = params.tone_spread.value();
    let live: usize = doubler_core::Voices::count(params.voices.value().into());

    (0..MAX_VOICES.min(live))
        .filter_map(|index| spread_band(index, spread))
        .map(|(highpass, lowpass)| (axis_x(highpass), axis_x(lowpass)))
        .collect()
}

/// The handles' heights are the parameters' normalized values, because the
/// curve's range and the parameters' range are the same ±12 dB.
fn grips_of(params: &DoublerParams) -> Vec<Grip> {
    vec![
        (
            axis_x(SHELF_LOW_HZ),
            params.tone_lo.unmodulated_normalized_value(),
        ),
        (
            axis_x(SHELF_HIGH_HZ),
            params.tone_hi.unmodulated_normalized_value(),
        ),
    ]
}

pub fn view(cx: &mut Context) {
    let shelves = [
        ParamWidgetBase::new(cx, Ui::params, |params| &params.tone_lo),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.tone_hi),
    ];

    HStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            CurveView::new(
                cx,
                Ui::params.map(|params| curve_of(params)),
                Ui::params.map(|params| spans_of(params)),
                Ui::params.map(|params| grips_of(params)),
                MARKS.iter().map(|(hz, _)| axis_x(*hz)).collect(),
                move |cx, index, gesture| {
                    let Some(shelf) = shelves.get(index) else {
                        return;
                    };
                    match gesture {
                        Gesture::Begin => shelf.begin_set_parameter(cx),
                        Gesture::Change(value) => shelf.set_normalized_value(cx, value),
                        Gesture::End => shelf.end_set_parameter(cx),
                        Gesture::Reset => {
                            shelf.begin_set_parameter(cx);
                            shelf.set_normalized_value(cx, shelf.default_normalized_value());
                            shelf.end_set_parameter(cx);
                        }
                        Gesture::Edit => {}
                    }
                },
            )
            .height(Pixels(96.0))
            .width(Stretch(1.0));

            // The labels are placed with the same mapping the widget was given.
            // A widget cannot draw text at arbitrary positions, so this is the
            // caller's job (`.agents/rules/vizia.md`).
            HStack::new(cx, |cx| {
                for (hz, text) in MARKS {
                    Label::new(cx, text)
                        .class("subtle")
                        .position_type(PositionType::SelfDirected)
                        .left(Percentage(axis_x(hz) * 100.0));
                }
            })
            .height(Pixels(14.0))
            .width(Stretch(1.0));
        })
        .width(Stretch(1.0))
        .height(Auto);

        // A fixed column: `macro_knob` stretches, and two stretching children
        // would split the row in half — leaving the curve drawn across half the
        // width it is supposed to span.
        VStack::new(cx, |cx| {
            macro_knob(cx, "SPREAD", |params| &params.tone_spread, 34.0);
        })
        .width(Pixels(96.0))
        .height(Auto);
    })
    .class("row")
    .col_between(Pixels(theme::SPACE_3))
    .height(Auto);
}
