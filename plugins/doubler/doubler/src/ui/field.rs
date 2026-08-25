//! The Voice Field: the Doubler's shape layer as points on a half circle.
//!
//! Angle is where a voice sits, radius is how far behind it is, and the dot's
//! size is its level (`plugins/doubler/docs/specifications/ui.md`).
//!
//! **The angle is the effective pan, not the shape value.** A voice's position
//! depends on `Spread` and on `Source`, and the point of the figure is to show
//! where the voices actually are. Dragging therefore converts back through
//! `doubler_core::pan_shape_for`, which is the inverse of the formula the DSP
//! uses — one formula, tested in both directions, rather than two that drift.
//!
//! **The radius is the shape value directly.** The outer arc *is* the `Delay`
//! macro, so a voice at `Delay_i` = 1 sits on it whatever the macro says.

use super::Ui;
use crate::params::DoublerParams;
use doubler_core::{MAX_VOICES, Source, pan_for, pan_shape_for};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::polar::{FieldGesture, FieldPoint, PolarField};

/// How far off the origin the source markers sit, so they are visible rather
/// than buried under the innermost dot.
const ANCHOR_RADIUS: f32 = 0.06;

/// Reads the discrete source mode out of its parameter.
fn source_of(params: &DoublerParams) -> Source {
    params.source.value().into()
}

fn points_of(params: &DoublerParams) -> Vec<FieldPoint> {
    let source = source_of(params);
    let spread = params.spread.value();
    let live = params.voices.value().into();
    let live: usize = doubler_core::Voices::count(live);

    (0..MAX_VOICES)
        .map(|index| {
            let shape = &params.shape[index];
            FieldPoint {
                angle: pan_for(source, spread, shape.pan.value(), index),
                radius: shape.delay.value(),
                size: shape.gain.unmodulated_normalized_value(),
                anchor: match source {
                    Source::MonoSum => 0,
                    Source::TrueStereo => index % 2,
                },
                enabled: index < live,
            }
        })
        .collect()
}

/// One marker under `MonoSum`, two under `TrueStereo`. They sit where a voice
/// with no pan offset would, so raising `Spread` visibly pushes the two sources
/// apart.
fn anchors_of(params: &DoublerParams) -> Vec<FieldPoint> {
    let source = source_of(params);
    let spread = params.spread.value();

    match source {
        Source::MonoSum => vec![FieldPoint {
            angle: 0.0,
            radius: 0.0,
            ..FieldPoint::default()
        }],
        Source::TrueStereo => (0..2)
            .map(|index| FieldPoint {
                angle: pan_for(source, spread, 0.0, index),
                radius: ANCHOR_RADIUS,
                ..FieldPoint::default()
            })
            .collect(),
    }
}

/// The two parameters a dragged point writes.
struct VoiceHandles {
    pan: ParamWidgetBase,
    delay: ParamWidgetBase,
}

pub fn view(cx: &mut Context) {
    let handles: Vec<VoiceHandles> = (0..MAX_VOICES)
        .map(|index| VoiceHandles {
            pan: ParamWidgetBase::new(cx, Ui::params, move |params| &params.shape[index].pan),
            delay: ParamWidgetBase::new(cx, Ui::params, move |params| &params.shape[index].delay),
        })
        .collect();

    // Recomputed whenever any parameter changes, which is what makes the dots
    // follow automation and `Spread`.
    let points = Ui::params.map(|params| points_of(params));
    let anchors = Ui::params.map(|params| anchors_of(params));
    let source_and_spread = Ui::params.map(|params| (source_of(params), params.spread.value()));

    PolarField::new(cx, points, anchors, move |cx, gesture| {
        let index = match gesture {
            FieldGesture::Begin(index)
            | FieldGesture::End(index)
            | FieldGesture::Reset(index)
            | FieldGesture::Change { index, .. } => index,
            // Cross-highlighting the Detail table arrives with the table
            // itself (`DBL-11`).
            FieldGesture::Hover(_) => return,
        };
        let Some(voice) = handles.get(index) else {
            return;
        };

        match gesture {
            FieldGesture::Begin(_) => {
                voice.pan.begin_set_parameter(cx);
                voice.delay.begin_set_parameter(cx);
            }
            FieldGesture::End(_) => {
                voice.pan.end_set_parameter(cx);
                voice.delay.end_set_parameter(cx);
            }
            FieldGesture::Reset(_) => {
                for base in [&voice.pan, &voice.delay] {
                    base.begin_set_parameter(cx);
                    base.set_normalized_value(cx, base.default_normalized_value());
                    base.end_set_parameter(cx);
                }
            }
            FieldGesture::Change { angle, radius, .. } => {
                let (source, spread) = source_and_spread.get(cx);

                // A sideways drag has nothing to write when the mode and spread
                // leave the voices with no pan to give — every voice is centred
                // regardless of its shape. The radius still moves.
                if let Some(shape) = pan_shape_for(source, spread, angle, index) {
                    // `Pan_i` runs `-1..=1`, so its normalized value is the
                    // shape mapped onto `0..=1`.
                    voice.pan.set_normalized_value(cx, (shape + 1.0) * 0.5);
                }
                voice.delay.set_normalized_value(cx, radius);
            }
            FieldGesture::Hover(_) => {}
        }
    })
    .height(Pixels(170.0))
    .width(Stretch(1.0));
}
