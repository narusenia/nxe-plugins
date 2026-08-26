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

/// How an axis reaches its mirror partner (`REQ-DBL-014`).
///
/// The two cases follow from the axis, not from taste: an axis that runs either
/// side of zero mirrors by changing sign, and one that only runs upward has no
/// sign to change, so its mirror image is the same distance out. The bar's own
/// shape follows the same split, which is why this decides that too.
#[derive(Clone, Copy)]
pub enum Mirror {
    /// Bipolar: the partner takes the opposite sign. `Pan`, `Detune`.
    Opposite,
    /// Unipolar: the partner takes the same value. `Delay`.
    Same,
}

impl Mirror {
    /// The partner's normalized value. Both mirrored bipolar axes run
    /// `-1..=1`, so flipping the sign is reflecting the normalized value about
    /// the middle.
    pub fn apply(self, normalized: f32) -> f32 {
        match self {
            Mirror::Opposite => 1.0 - normalized,
            Mirror::Same => normalized,
        }
    }

    /// The same gesture as it applies to the partner. Only the value moves; the
    /// boundaries and the reset are the partner's too, so the host sees one
    /// gesture per parameter rather than a stray write between someone else's.
    fn applied(self, gesture: Gesture) -> Gesture {
        match gesture {
            Gesture::Change(value) => Gesture::Change(self.apply(value)),
            other => other,
        }
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

/// A bar that also writes its mirror partner while `mirror` is on.
pub fn mirrored_bar<'a, L, M, Params, P, F, G>(
    cx: &'a mut Context,
    params: L,
    mirror: M,
    kind: Mirror,
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

    let handler = move |cx: &mut EventContext, gesture: Gesture| {
        apply(&base, cx, gesture);
        // Read per gesture rather than latched at `Begin`: the toggle cannot be
        // reached mid-drag with one pointer, so there is no state to keep.
        if mirror.get(cx) {
            apply(&partner, cx, kind.applied(gesture));
        }
    };

    match kind {
        Mirror::Opposite => Bar::bipolar(cx, value, handler),
        Mirror::Same => Bar::new(cx, value, handler),
    }
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
