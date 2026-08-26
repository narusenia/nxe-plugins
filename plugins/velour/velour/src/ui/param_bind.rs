//! The adapter between nih-plug's parameters and `nxe-ui`'s widgets.
//!
//! `nxe-ui` knows nothing about parameters — widgets take a value and a gesture
//! callback (`docs/specifications/architecture.md`). This module is the only
//! place that knows about both.
//!
//! **The Doubler has a file with the same name and the same functions.**
//! Deliberately not shared yet: the repository's rule is that something moves
//! into a common crate when a *third* consumer asks for it, not when a second
//! one could (`docs/specifications/architecture.md`). Sparkleur is the third,
//! and a `nxe-plug-ui` crate — one that may know both nih-plug and vizia,
//! unlike `nxe-ui` — is where these belong then. What must not happen is
//! putting them in `nxe-ui`, which would make the gallery link nih-plug and
//! stop being a standalone app.
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
pub fn apply(base: &ParamWidgetBase, cx: &mut EventContext, gesture: Gesture) {
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
        // Typing a value needs `nxe_ui::entry::ValueEntry`, which stops the
        // editor updating once it is mounted in a plugin rather than in the
        // gallery — cause not yet found (`docs/HANDOVER.md`). Until then the
        // host's own generic view is where a value gets typed.
        Gesture::Edit => {}
    }
}

/// The normalized value of a parameter, as a lens.
pub fn value_of<L, Params, P, F>(params: L, to_param: F) -> impl Lens<Target = f32>
where
    L: Lens<Target = Params> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
{
    ParamWidgetBase::make_lens(params, to_param, |param| {
        param.unmodulated_normalized_value()
    })
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
    let value = value_of(params, to_param);

    Knob::new(cx, value, move |cx, gesture| apply(&base, cx, gesture)).size(Pixels(size))
}

/// A bar bound to a parameter. Whether it grows from the middle or from the left
/// follows the parameter's range rather than taste.
pub fn bar<'a, L, Params, P, F>(
    cx: &'a mut Context,
    params: L,
    to_param: F,
    bipolar: bool,
) -> Handle<'a, Bar>
where
    L: Lens<Target = Params> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
{
    let base = ParamWidgetBase::new(cx, params, to_param);
    let value = value_of(params, to_param);
    let handler = move |cx: &mut EventContext, gesture: Gesture| apply(&base, cx, gesture);

    if bipolar {
        Bar::bipolar(cx, value, handler)
    } else {
        Bar::new(cx, value, handler)
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
    // `step_count` is the number of steps *between* values, so a two-way control
    // reports one.
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

/// A pill that is checked while a boolean parameter is on.
///
/// **Not a new widget** (`crates/nxe-ui/README.md`): `.segment` carries its own
/// `:checked` styling, so one `Label` with the class is the same thing a
/// `SegmentedControl` is made of. `UI-9`'s `ToggleSwitch` would be a sliding
/// switch, which is not what belongs in a table.
pub fn toggle<'a, L, Params, P, F>(
    cx: &'a mut Context,
    params: L,
    to_param: F,
    label: &'static str,
) -> Handle<'a, Label>
where
    L: Lens<Target = Params> + Copy,
    Params: 'static,
    P: Param + 'static,
    F: Fn(&Params) -> &P + Copy + 'static,
{
    let base = ParamWidgetBase::new(cx, params, to_param);
    let on = value_of(params, to_param).map(|value| *value > 0.5);

    Label::new(cx, label)
        .class("segment")
        .checked(on)
        .on_press(move |cx| {
            let next = if base.unmodulated_normalized_value() > 0.5 {
                0.0
            } else {
                1.0
            };
            // A press is one edit, so it carries its own boundaries — a write
            // with no `begin`/`end` around it is what makes a host record a
            // click as an orphaned automation point.
            base.begin_set_parameter(cx);
            base.set_normalized_value(cx, next);
            base.end_set_parameter(cx);
        })
}
