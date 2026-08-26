//! Mirrored editing: one bar that writes its partner as well.
//!
//! **The only part of the parameter binding that is Doubler's** (`REQ-DBL-014`).
//! Everything else — the gesture bridge, knobs, plain bars, segmented controls —
//! moved to `nxe_plug_ui` when Sparkleur asked for a third copy (`SPK-11`).
//! This stayed because no other plugin has two voices to keep opposite.

use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_plug_ui::{apply, value_of};
use nxe_ui::bar::Bar;
use nxe_ui::input::Gesture;

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
    let value = value_of(params, to_param);

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
