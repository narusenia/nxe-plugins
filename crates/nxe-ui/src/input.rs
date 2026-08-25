//! The pointer interaction every value control shares.
//!
//! Kept in one place because `Knob` and `Bar` have to behave identically — a
//! control that reacts differently to the same gesture than the one next to it
//! is the kind of thing nobody reports and everybody notices.
//!
//! vizia ships a `Knob`, and it is not used: it takes a `Lens` and offers only
//! `on_changing`. A plugin needs to tell its host when a gesture **starts and
//! ends**, or the host records an automation move as a scatter of unrelated
//! points instead of one edit. There is no hook for that, and no way to add
//! shift-fine, double-click reset, or type-a-value around a closed view.

use vizia::prelude::*;

/// Pixels of vertical travel that cover the whole range. Ear— eye-tuned: far
/// enough that a full sweep needs a deliberate drag, near enough that it fits
/// on a laptop trackpad in one gesture.
pub const TRAVEL: f32 = 200.0;

/// How much slower a fine drag moves.
pub const FINE: f32 = 0.2;

/// What the pointer asked for. The widget applies it and forwards it to the
/// caller, which is how a plugin turns `Begin` and `End` into host gesture
/// notifications.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Gesture {
    /// A drag started. Nothing has changed yet.
    Begin,
    /// A new normalized value, already clamped to `0..=1`.
    Change(f32),
    /// The drag finished.
    End,
    /// Go back to the default. The widget does not know what that is; the
    /// caller does.
    Reset,
    /// The user wants to type a value.
    Edit,
}

/// What a widget stores its caller's gesture handler as. Shared so `Knob` and
/// `Bar` do not each spell it out.
pub type GestureCallback = Box<dyn Fn(&mut EventContext, Gesture)>;

/// Vertical drag state.
#[derive(Default)]
pub struct Drag {
    active: bool,
    last_y: f32,
}

impl Drag {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The value after dragging `delta_y` pixels. Up increases, because the
    /// screen's y axis points down and knobs do not.
    ///
    /// Pure, so the arithmetic is testable without a window — which is most of
    /// what can go wrong here.
    pub fn value_after(value: f32, delta_y: f32, fine: bool) -> f32 {
        let scale = if fine { FINE } else { 1.0 };
        (value - delta_y / TRAVEL * scale).clamp(0.0, 1.0)
    }

    /// Feeds one vizia event and says what the widget should do.
    pub fn handle(
        &mut self,
        cx: &mut EventContext,
        event: &mut Event,
        value: f32,
    ) -> Option<Gesture> {
        let mut gesture = None;

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                // Cmd on macOS, Ctrl elsewhere. Double-click is already taken
                // by the reset, and right-click belongs to the host's own
                // parameter menu.
                if cx.modifiers().intersects(Modifiers::CTRL | Modifiers::LOGO) {
                    gesture = Some(Gesture::Edit);
                } else {
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.active = true;
                    self.last_y = cx.mouse().cursory;
                    gesture = Some(Gesture::Begin);
                }
                meta.consume();
            }

            WindowEvent::MouseMove(_, y) if self.active => {
                let delta = *y - self.last_y;
                self.last_y = *y;
                let fine = cx.modifiers().contains(Modifiers::SHIFT);
                gesture = Some(Gesture::Change(Self::value_after(value, delta, fine)));
            }

            WindowEvent::MouseUp(MouseButton::Left) if self.active => {
                cx.release();
                cx.set_active(false);
                self.active = false;
                gesture = Some(Gesture::End);
                meta.consume();
            }

            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                gesture = Some(Gesture::Reset);
                meta.consume();
            }

            _ => {}
        });

        gesture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_up_increases_the_value() {
        // Negative delta is upward on screen.
        assert!(Drag::value_after(0.5, -20.0, false) > 0.5);
        assert!(Drag::value_after(0.5, 20.0, false) < 0.5);
    }

    #[test]
    fn a_full_travel_covers_the_whole_range() {
        assert_eq!(Drag::value_after(0.0, -TRAVEL, false), 1.0);
        assert_eq!(Drag::value_after(1.0, TRAVEL, false), 0.0);
    }

    #[test]
    fn a_fine_drag_moves_less() {
        let coarse = Drag::value_after(0.5, -20.0, false) - 0.5;
        let fine = Drag::value_after(0.5, -20.0, true) - 0.5;
        assert!(fine > 0.0);
        assert!((fine / coarse - FINE).abs() < 1e-6);
    }

    /// A control must not wrap around or run off the end, whatever the pointer
    /// does.
    #[test]
    fn the_value_is_clamped() {
        for delta in [-10_000.0f32, -1.0, 0.0, 1.0, 10_000.0] {
            for start in [0.0f32, 0.5, 1.0] {
                let value = Drag::value_after(start, delta, false);
                assert!(
                    (0.0..=1.0).contains(&value),
                    "start {start}, delta {delta} gave {value}"
                );
            }
        }
    }
}
