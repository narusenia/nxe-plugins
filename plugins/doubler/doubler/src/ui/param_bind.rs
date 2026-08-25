//! The adapter between nih-plug's parameters and `nxe-ui`'s widgets.
//!
//! `nxe-ui` knows nothing about parameters — widgets take a value and a gesture
//! callback (`docs/specifications/architecture.md`). This module is the only
//! place that knows about both, and it is small on purpose.
//!
//! The important part is that a widget's `Begin` and `End` become
//! `begin_set_parameter` / `end_set_parameter`. Without them a host records a
//! drag as a scatter of unrelated automation points instead of one edit.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::input::Gesture;
use nxe_ui::knob::Knob;
use nxe_ui::segmented::SegmentedControl;

/// Applies a gesture to a parameter. Shared by every control so they all treat
/// a reset — and a gesture boundary — the same way.
fn apply(base: &ParamWidgetBase, cx: &mut EventContext, gesture: Gesture) {
    match gesture {
        Gesture::Begin => base.begin_set_parameter(cx),
        Gesture::Change(value) => base.set_normalized_value(cx, value),
        Gesture::End => base.end_set_parameter(cx),
        Gesture::Reset => {
            // A reset is a gesture of its own, not a value written mid-drag.
            base.begin_set_parameter(cx);
            base.set_normalized_value(cx, base.default_normalized_value());
            base.end_set_parameter(cx);
        }
        // Typing a value needs an inline text field, which `nxe-ui` does not
        // have yet (`UI-3`). Until then the host's own generic UI is where a
        // value gets typed.
        Gesture::Edit => {}
    }
}

/// A knob bound to a parameter.
pub fn knob<'a, L, Params, P, F>(
    cx: &'a mut Context,
    params: L,
    to_param: F,
    size: f32,
) -> Handle<'a, Knob>
where
    L: Lens<Target = Params> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
{
    let base = ParamWidgetBase::new(cx, params, to_param);
    let value = ParamWidgetBase::make_lens(params, to_param, |param| {
        param.unmodulated_normalized_value()
    });

    Knob::new(cx, value, move |cx, gesture| apply(&base, cx, gesture)).size(Pixels(size))
}

/// A segmented control bound to a stepped parameter.
///
/// Type-agnostic on purpose: the index comes from the parameter's own
/// normalized value and step count, so this works for any stepped parameter
/// without knowing which enum backs it.
pub fn segmented<'a, L, Params, P, F>(
    cx: &'a mut Context,
    params: L,
    to_param: F,
    labels: &[&str],
) -> Handle<'a, SegmentedControl>
where
    L: Lens<Target = Params> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
{
    let base = ParamWidgetBase::new(cx, params, to_param);
    // `step_count` is the number of steps *between* values, so a three-way
    // control reports two.
    let steps = base.step_count().unwrap_or(1).max(1) as f32;

    let selected = ParamWidgetBase::make_lens(params, to_param, move |param| {
        (param.unmodulated_normalized_value() * steps).round() as usize
    });

    SegmentedControl::new(cx, selected, labels, move |cx, index| {
        base.begin_set_parameter(cx);
        base.set_normalized_value(cx, index as f32 / steps);
        base.end_set_parameter(cx);
    })
}
