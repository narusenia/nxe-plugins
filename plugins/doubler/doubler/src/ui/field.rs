//! The Voice Field: the Doubler's shape layer as points on a half circle.
//!
//! Angle is where a voice sits, **radius is its level**, and the dot's size is
//! how far behind it is (`plugins/doubler/docs/specifications/ui.md`).
//!
//! Level on the radius rather than delay: a doubler's figure is read as "where
//! are the voices and how loud", the way Waves Doubler shows it. Delay keeps its
//! numbers in the Detail table and its rough size here.
//!
//! **The angle is the effective pan, not the shape value.** A voice's position
//! depends on `Spread` and on `Source`, and the point of the figure is to show
//! where the voices actually are. Dragging therefore converts back through
//! `doubler_core::pan_shape_for`, which is the inverse of the formula the DSP
//! uses — one formula, tested in both directions, rather than two that drift.
//!
//! **The radius is `Gain_i`'s normalized value.** The outer arc is the top of
//! the gain range (+6 dB) and the origin is the bottom (−24 dB), so the default
//! 0 dB puts every voice on the same ring — a symmetric figure out of the box.

use super::param_bind::Mirror;
use super::{Ui, UiEvent};
use crate::params::DoublerParams;
use doubler_core::{MAX_VOICES, Source, mirror_partner, pan_for, pan_shape_for};
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::polar::{FieldGesture, FieldPoint, PolarField};

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
                radius: shape.gain.unmodulated_normalized_value(),
                size: shape.delay.value(),
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
///
/// **Their radius is `Dry Gain`**, read on the same axis as the dots' own level,
/// so the dry sits in the picture with the voices rather than beside it. Both
/// markers show one value; dragging either writes it.
fn anchors_of(params: &DoublerParams) -> Vec<FieldPoint> {
    let source = source_of(params);
    let spread = params.spread.value();
    let radius = params.dry_gain.unmodulated_normalized_value();

    let markers = match source {
        Source::MonoSum => 1,
        Source::TrueStereo => 2,
    };

    (0..markers)
        .map(|index| FieldPoint {
            angle: pan_for(source, spread, 0.0, index),
            radius,
            ..FieldPoint::default()
        })
        .collect()
}

/// The parameters a dragged point writes.
struct VoiceHandles {
    pan: ParamWidgetBase,
    gain: ParamWidgetBase,
    /// The paired voice's parameters, written alongside the dragged voice's
    /// while mirroring is on (`REQ-DBL-014`). The angle inverts and the radius
    /// matches, so the partner point tracks the dragged one as its reflection.
    mirror_pan: ParamWidgetBase,
    mirror_gain: ParamWidgetBase,
}

impl VoiceHandles {
    /// The parameters this drag spans, so a gesture's boundaries cover exactly
    /// the ones it writes. A host that is told a gesture began on a parameter
    /// nothing then writes shows an edit that did not happen.
    fn written(&self, cx: &mut EventContext) -> Vec<&ParamWidgetBase> {
        let mut bases = vec![&self.pan, &self.gain];
        if Ui::mirror_pan.get(cx) {
            bases.push(&self.mirror_pan);
        }
        if Ui::mirror_gain.get(cx) {
            bases.push(&self.mirror_gain);
        }
        bases
    }
}

pub fn view(cx: &mut Context) {
    let handles: Vec<VoiceHandles> = (0..MAX_VOICES)
        .map(|index| VoiceHandles {
            pan: ParamWidgetBase::new(cx, Ui::params, move |params| &params.shape[index].pan),
            gain: ParamWidgetBase::new(cx, Ui::params, move |params| &params.shape[index].gain),
            mirror_pan: ParamWidgetBase::new(cx, Ui::params, move |params| {
                &params.shape[mirror_partner(index)].pan
            }),
            mirror_gain: ParamWidgetBase::new(cx, Ui::params, move |params| {
                &params.shape[mirror_partner(index)].gain
            }),
        })
        .collect();

    // Recomputed whenever any parameter changes, which is what makes the dots
    // follow automation and `Spread`.
    let points = Ui::params.map(|params| points_of(params));
    let anchors = Ui::params.map(|params| anchors_of(params));
    let source_and_spread = Ui::params.map(|params| (source_of(params), params.spread.value()));

    let dry = ParamWidgetBase::new(cx, Ui::params, |params| &params.dry_gain);

    PolarField::new(cx, points, anchors, move |cx, gesture| {
        let index = match gesture {
            FieldGesture::Begin(index)
            | FieldGesture::End(index)
            | FieldGesture::Reset(index)
            | FieldGesture::Change { index, .. } => index,
            FieldGesture::Hover(over) => {
                cx.emit(UiEvent::Hover(over));
                return;
            }
            // The source markers carry `Dry Gain`, on the same radial axis the
            // dots use for their own level.
            FieldGesture::AnchorBegin => {
                dry.begin_set_parameter(cx);
                return;
            }
            FieldGesture::AnchorChange(radius) => {
                dry.set_normalized_value(cx, radius);
                return;
            }
            FieldGesture::AnchorEnd => {
                dry.end_set_parameter(cx);
                return;
            }
            FieldGesture::AnchorReset => {
                dry.begin_set_parameter(cx);
                dry.set_normalized_value(cx, dry.default_normalized_value());
                dry.end_set_parameter(cx);
                return;
            }
        };
        let Some(voice) = handles.get(index) else {
            return;
        };

        match gesture {
            FieldGesture::Begin(_) => {
                for base in voice.written(cx) {
                    base.begin_set_parameter(cx);
                }
            }
            FieldGesture::End(_) => {
                for base in voice.written(cx) {
                    base.end_set_parameter(cx);
                }
            }
            FieldGesture::Reset(_) => {
                for base in voice.written(cx) {
                    base.begin_set_parameter(cx);
                    base.set_normalized_value(cx, base.default_normalized_value());
                    base.end_set_parameter(cx);
                }
            }
            FieldGesture::Change { angle, radius, .. } => {
                let (source, spread) = source_and_spread.get(cx);

                // A sideways drag has nothing to write when the mode and spread
                // leave the voices with no pan to give — every voice is centred
                // regardless of its shape. The level still moves.
                if let Some(shape) = pan_shape_for(source, spread, angle, index) {
                    // `Pan_i` runs `-1..=1`, so its normalized value is the
                    // shape mapped onto `0..=1`.
                    let normalized = (shape + 1.0) * 0.5;
                    voice.pan.set_normalized_value(cx, normalized);
                    if Ui::mirror_pan.get(cx) {
                        voice
                            .mirror_pan
                            .set_normalized_value(cx, Mirror::Opposite.apply(normalized));
                    }
                }

                voice.gain.set_normalized_value(cx, radius);
                if Ui::mirror_gain.get(cx) {
                    voice
                        .mirror_gain
                        .set_normalized_value(cx, Mirror::Same.apply(radius));
                }
            }
            _ => {}
        }
    })
    .height(Stretch(1.0))
    .width(Stretch(1.0));
}
