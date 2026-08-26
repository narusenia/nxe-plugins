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
use nxe_ui::bar::Bar;
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

/// A mirrored value: the two mirrored axes both run symmetrically about zero,
/// so inverting the sign is the same as reflecting the normalized value about
/// the middle (`REQ-DBL-014`).
pub fn reflect(normalized: f32) -> f32 {
    1.0 - normalized
}

/// The same gesture as it applies to the mirror partner. Only the value moves;
/// the boundaries and the reset are the partner's too, so the host sees one
/// gesture per parameter rather than a stray write between someone else's.
fn reflected(gesture: Gesture) -> Gesture {
    match gesture {
        Gesture::Change(value) => Gesture::Change(reflect(value)),
        other => other,
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

/// A bar bound to a parameter. `centred` fills from the middle, for a value
/// that runs either side of zero.
pub fn bar<'a, L, Params, P, F>(
    cx: &'a mut Context,
    params: L,
    to_param: F,
    centred: bool,
) -> Handle<'a, Bar>
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
    let handler = move |cx: &mut EventContext, gesture: Gesture| apply(&base, cx, gesture);

    if centred {
        Bar::bipolar(cx, value, handler)
    } else {
        Bar::new(cx, value, handler)
    }
}

/// A bar that also writes its mirror partner while `mirror` is on.
///
/// Always bipolar: the only axes this is used for are the ones that run either
/// side of zero, which is what makes reflecting them meaningful.
pub fn mirrored_bar<'a, L, M, Params, P, F, G>(
    cx: &'a mut Context,
    params: L,
    mirror: M,
    to_param: F,
    to_partner: G,
) -> Handle<'a, Bar>
where
    L: Lens<Target = Params> + Copy,
    M: Lens<Target = bool> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
    G: Fn(&Params) -> &P + Copy + 'static,
{
    let base = ParamWidgetBase::new(cx, params, to_param);
    let partner = ParamWidgetBase::new(cx, params, to_partner);
    let value = ParamWidgetBase::make_lens(params, to_param, |param| {
        param.unmodulated_normalized_value()
    });

    Bar::bipolar(cx, value, move |cx, gesture| {
        apply(&base, cx, gesture);
        // Read per gesture rather than latched at `Begin`: the toggle cannot be
        // reached mid-drag with one pointer, so there is no state to keep.
        if mirror.get(cx) {
            apply(&partner, cx, reflected(gesture));
        }
    })
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
