//! The pointer interaction every value control shares.
//!
//! One place, because a knob and a bar that react differently to the same
//! gesture is the kind of thing nobody reports and everybody notices.
//!
//! - **Vertical drag** on every value control, bars included: sliding sideways
//!   past the end of one bar into the next is a misfire waiting to happen
//! - **Shift** for a fine drag
//! - **Double-click** resets — the caller knows the default, the widget does not
//! - **Cmd-click** (Ctrl elsewhere) asks to type a value (`UI-3`)
//!
//! The widget never writes the value itself. It reports [`Gestures`], and the
//! caller sends them on — which is how `Begin` and `End` reach the host as one
//! automation edit instead of a scatter of points.

use egui::Response;

/// Pixels of vertical travel that cover the whole range.
pub const TRAVEL: f32 = 200.0;

/// How much slower a fine drag moves.
pub const FINE: f32 = 0.2;

/// What the pointer did to a control this frame. More than one can be set in
/// one frame — a drag can begin and move at once.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Gestures {
    pub begin: bool,
    /// A new normalized value, already clamped to `0..=1`.
    pub change: Option<f32>,
    pub end: bool,
    pub reset: bool,
    /// The user wants to type a value.
    pub edit: bool,
}

impl Gestures {
    pub fn any(&self) -> bool {
        *self != Self::default()
    }
}

/// The value after dragging `delta_y` pixels. Up increases.
pub fn value_after(value: f32, delta_y: f32, fine: bool) -> f32 {
    let scale = if fine { FINE } else { 1.0 };
    (value - delta_y / TRAVEL * scale).clamp(0.0, 1.0)
}

/// Reads one control's response. `response` has to sense click and drag.
pub fn read(response: &Response, value: f32) -> Gestures {
    let modifiers = response.ctx.input(|input| input.modifiers);
    let mut gestures = Gestures::default();
    if response.double_clicked() {
        gestures.reset = true;
        return gestures;
    }
    if response.clicked() && modifiers.command {
        gestures.edit = true;
        return gestures;
    }
    if response.drag_started() && !modifiers.command {
        gestures.begin = true;
    }
    if response.dragged() && !modifiers.command {
        let delta = response.drag_delta().y;
        if delta != 0.0 {
            gestures.change = Some(value_after(value, delta, modifiers.shift));
        }
    }
    if response.drag_stopped() && !modifiers.command {
        gestures.end = true;
    }
    gestures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_up_increases_the_value() {
        assert!(value_after(0.5, -20.0, false) > 0.5);
        assert!(value_after(0.5, 20.0, false) < 0.5);
    }

    #[test]
    fn a_full_travel_covers_the_whole_range() {
        assert_eq!(value_after(0.0, -TRAVEL, false), 1.0);
        assert_eq!(value_after(1.0, TRAVEL, false), 0.0);
    }

    #[test]
    fn a_fine_drag_moves_less() {
        let coarse = value_after(0.5, -20.0, false) - 0.5;
        let fine = value_after(0.5, -20.0, true) - 0.5;
        assert!(fine > 0.0);
        assert!((fine / coarse - FINE).abs() < 1e-6);
    }

    #[test]
    fn the_value_is_clamped() {
        for delta in [-10_000.0f32, -1.0, 0.0, 1.0, 10_000.0] {
            for start in [0.0f32, 0.5, 1.0] {
                let value = value_after(start, delta, false);
                assert!((0.0..=1.0).contains(&value), "{start} {delta} → {value}");
            }
        }
    }
}
