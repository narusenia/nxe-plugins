//! The figure: what is arriving, what is being taken out, and where the user
//! said it may happen.
//!
//! **The figure's subject is the reduction curve** (`REQ-PUM-013`). Pumice
//! works automatically, and a process that works automatically and cannot be
//! seen reads as one that is not working — the mistake `REQ-SPK-008` records
//! about Sparkleur's guard.
//!
//! **The curves come from the engine, not from the parameters.** The reduction
//! drawn here is the gain the audio got (`pumice_core::display`); computing it
//! again from the same knobs is how a figure starts telling a different story
//! from the sound.
//!
//! ## Full width, and that is a decision about the axis
//!
//! The figure spans the window rather than sharing a row, because **the axis
//! that needs the pixels is frequency**. 880 px over ten octaves is 88 px per
//! octave; a two-panel layout with the figure on one side would leave 40, and
//! placing six nodes by hand is a horizontal job. The vertical axis carries
//! depth, which is coarse.

use super::{Ui, UiEvent};
use crate::params::{self, PumiceParams};
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::node::{NodeField, NodeFieldModifiers, NodeGesture};
use nxe_ui::theme;
use std::sync::Arc;

/// **The same height as every other plugin's figure.** Two figures tuned
/// separately are each right on their own and wrong beside each other
/// (`.agents/rules/ui.md`).
pub const HEIGHT: f32 = 200.0;

/// The axis labels' own line, under the plot.
pub const MARKS_HEIGHT: f32 = theme::LINE_VALUE;

/// The level labels' own column, beside the plot. Three characters and a sign.
pub const LEVELS_WIDTH: f32 = 22.0;

/// The spectrum's window, and where its lines go.
///
/// **The vertical axis had no scale at all**, so a peak's height said nothing:
/// 6 dB above its neighbours and 20 drew the same picture until you compared
/// them. These are the spectrum's decibels — the reduction hangs from the top
/// in its own units and the weight is read against the middle, but *level* is
/// what a person means by a vertical axis on a spectrum.
pub const LEVELS: [(f32, &str); 4] = [(0.0, "0"), (-24.0, "-24"), (-48.0, "-48"), (-72.0, "-72")];

/// Where the gridlines and the labels go.
const MARKS: [(f32, &str); 6] = [
    (20.0, "20"),
    (100.0, "100"),
    (500.0, "500"),
    (2_000.0, "2k"),
    (8_000.0, "8k"),
    (20_000.0, "20k"),
];

/// A level in dB as a height in the figure. **The window owns the scale**
/// (`super::SPECTRUM_FLOOR_DB`), so this takes it rather than repeating it.
pub fn level_position(db: f32, floor_db: f32, ceiling_db: f32) -> f32 {
    ((db - floor_db) / (ceiling_db - floor_db)).clamp(0.0, 1.0)
}

/// The total span of the axis, in octaves — what a node's width in octaves is
/// measured against when it becomes a width in the figure.
pub fn axis_octaves() -> f32 {
    (params::HIGH_HZ / params::LOW_HZ).log2()
}

/// One node's four parameters, so a gesture can write them.
struct Bases {
    enabled: ParamWidgetBase,
    freq: ParamWidgetBase,
    width: ParamWidgetBase,
    depth: ParamWidgetBase,
}

pub fn view(cx: &mut Context) {
    // **Built once, outside the callback.** A `ParamWidgetBase` is a pointer
    // resolved against the parameter map; making one per gesture would resolve
    // twenty-four of them on every mouse move.
    let bases: Arc<Vec<Bases>> = Arc::new(
        (0..pumice_core::NODES)
            .map(|index| Bases {
                enabled: ParamWidgetBase::new(cx, Ui::params, move |params: &Arc<PumiceParams>| {
                    &params.nodes[index].enabled
                }),
                freq: ParamWidgetBase::new(cx, Ui::params, move |params: &Arc<PumiceParams>| {
                    &params.nodes[index].freq
                }),
                width: ParamWidgetBase::new(cx, Ui::params, move |params: &Arc<PumiceParams>| {
                    &params.nodes[index].width
                }),
                depth: ParamWidgetBase::new(cx, Ui::params, move |params: &Arc<PumiceParams>| {
                    &params.nodes[index].depth
                }),
            })
            .collect(),
    );

    VStack::new(cx, move |cx| {
        let bases = bases.clone();
        // The plot, and the level labels in a column beside it.
        HStack::new(cx, move |cx| {
            let bases = bases.clone();
            NodeField::new(
                cx,
                Ui::nodes,
                Ui::weight,
                MARKS
                    .iter()
                    .map(|(hz, _)| params::hz_to_position(*hz))
                    .collect(),
                LEVELS
                    .iter()
                    .map(|(db, _)| {
                        level_position(*db, super::SPECTRUM_FLOOR_DB, super::SPECTRUM_CEILING_DB)
                    })
                    .collect(),
                move |cx, gesture| {
                    handle(&bases, cx, gesture);
                },
            )
            .analysis(Ui::spectrum)
            .reduction(Ui::reduction)
            .height(Stretch(1.0))
            .width(Stretch(1.0));

            // **The caller places them**, with the caller's own mapping — the
            // widget cannot draw text at arbitrary positions (`nxe_ui::node`).
            VStack::new(cx, |cx| {
                for (db, text) in LEVELS {
                    Label::new(cx, text)
                        .class("subtle")
                        .position_type(PositionType::SelfDirected)
                        .top(Percentage(
                            (1.0 - level_position(
                                db,
                                super::SPECTRUM_FLOOR_DB,
                                super::SPECTRUM_CEILING_DB,
                            )) * 100.0,
                        ));
                }
            })
            .width(Pixels(LEVELS_WIDTH))
            .height(Stretch(1.0));
        })
        .height(Pixels(HEIGHT - MARKS_HEIGHT - theme::SPACE_1))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_1));

        // The axis labels are the caller's, placed with the caller's own
        // mapping — the widget cannot draw text at arbitrary positions
        // (`nxe_ui::node`).
        HStack::new(cx, |cx| {
            for (hz, text) in MARKS {
                Label::new(cx, text)
                    .class("subtle")
                    .position_type(PositionType::SelfDirected)
                    .left(Percentage(params::hz_to_position(hz) * 100.0));
            }
        })
        .height(Pixels(MARKS_HEIGHT))
        // Short by the level column, so the frequency marks line up with the
        // plot rather than with the row.
        .width(Stretch(1.0))
        .right(Pixels(LEVELS_WIDTH + theme::SPACE_1));
    })
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1));
}

/// Writes what the pointer asked for, and asks the model to re-read.
///
/// **Every write is followed by a re-read** (`nxe_ui::node`): the widget does
/// not move its own points, so what the figure shows is whatever came back out
/// of the parameters — clamped, capped at six, and in the parameters' own
/// resolution.
fn handle(bases: &[Bases], cx: &mut EventContext, gesture: NodeGesture) {
    match gesture {
        NodeGesture::Begin(index) => {
            if let Some(node) = bases.get(index) {
                node.freq.begin_set_parameter(cx);
                node.width.begin_set_parameter(cx);
                node.depth.begin_set_parameter(cx);
            }
        }

        NodeGesture::Change { index, x, y } => {
            if let Some(node) = bases.get(index) {
                node.freq.set_normalized_value(cx, x.clamp(0.0, 1.0));
                // `y` is the position on the figure and `depth` is bipolar, so
                // the parameter's own normalized value *is* the height.
                node.depth.set_normalized_value(cx, y.clamp(0.0, 1.0));
            }
        }

        NodeGesture::Width { index, half_width } => {
            if let Some(node) = bases.get(index) {
                // Half a width in figure units back to a whole width in
                // octaves, and then to the parameter's position.
                let octaves = (half_width * 2.0 * axis_octaves())
                    .clamp(params::NARROWEST_OCTAVES, params::WIDEST_OCTAVES);
                node.width
                    .set_normalized_value(cx, params::octaves_to_position(octaves));
            }
        }

        NodeGesture::End(index) => {
            if let Some(node) = bases.get(index) {
                node.freq.end_set_parameter(cx);
                node.width.end_set_parameter(cx);
                node.depth.end_set_parameter(cx);
            }
        }

        // **The first one that is off, and nothing is evicted when full**
        // (`REQ-PUM-004`). Six is what a host can store, so six is the answer;
        // quietly taking someone's node away to make room for a click would be
        // worse than the click doing nothing.
        NodeGesture::Add { x, y } => {
            let free = bases
                .iter()
                .position(|node| node.enabled.unmodulated_normalized_value() <= 0.5);
            if let Some(index) = free {
                let node = &bases[index];
                for base in [&node.enabled, &node.freq, &node.depth, &node.width] {
                    base.begin_set_parameter(cx);
                }
                node.freq.set_normalized_value(cx, x.clamp(0.0, 1.0));
                node.depth.set_normalized_value(cx, y.clamp(0.0, 1.0));
                // A width that reads as one feature rather than as a spike.
                node.width
                    .set_normalized_value(cx, params::octaves_to_position(0.5));
                node.enabled.set_normalized_value(cx, 1.0);
                for base in [&node.enabled, &node.freq, &node.depth, &node.width] {
                    base.end_set_parameter(cx);
                }
            }
        }

        NodeGesture::Remove(index) => {
            if let Some(node) = bases.get(index) {
                node.enabled.begin_set_parameter(cx);
                node.enabled.set_normalized_value(cx, 0.0);
                node.enabled.end_set_parameter(cx);
            }
        }

        NodeGesture::Hover(over) => {
            cx.emit(UiEvent::Hover(over));
            return;
        }
    }

    cx.emit(UiEvent::Sync);
}
