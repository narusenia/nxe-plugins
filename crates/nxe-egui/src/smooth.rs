//! Display-side smoothing for signals (`v2-plan.md`, decision 10).
//!
//! **Why a signal stair-steps.** The window reads the audio thread's handoff
//! at whatever rate the editor ticks, and draws at the display's rate. Read at
//! 30 Hz and drawn at 60, a meter moves on every other frame and stands still
//! on the rest — which the eye reads as a judder, not as a slower meter.
//!
//! **The fix is in the display, never in the DSP**: every drawn frame moves the
//! shown value toward the latest reading by an exponential step sized by the
//! frame's own `dt`. It moves on every frame, whatever the two rates are, and a
//! slow frame takes a larger step instead of falling behind.
//!
//! Glow and curves are drawn from the smoothed values, so they inherit it.

use egui::{Id, Ui};

/// The time constant most signals want: long enough to bridge a 30 Hz
/// reading, short enough that a transient still reads as one.
pub const TAU: f32 = 0.04;

/// The fastest a moving signal asks to be redrawn. **Not the display's rate**:
/// on a 120 Hz panel "every frame" is twice the work for motion the eye
/// already reads as continuous at 60, and the window's budget is one core's
/// 10 % (`v2-plan.md`, decision 9).
pub const FRAME: std::time::Duration = std::time::Duration::from_micros(16_667);

/// Below this the shown value is taken to have arrived, and repaints stop.
const SETTLED: f32 = 1e-4;

/// The shown values of one signal.
#[derive(Clone, Default, Debug)]
pub struct Smoother {
    shown: Vec<f32>,
}

impl Smoother {
    /// Moves every shown value toward its target by one frame of `dt`
    /// seconds. Returns whether anything is still moving.
    pub fn step(&mut self, targets: &[f32], dt: f32, tau: f32) -> bool {
        if self.shown.len() != targets.len() {
            // A new signal, or one that changed shape: start where it is.
            self.shown = targets.to_vec();
            return false;
        }
        let k = 1.0 - (-dt.max(0.0) / tau.max(1e-6)).exp();
        let mut moving = false;
        for (shown, target) in self.shown.iter_mut().zip(targets) {
            *shown += (target - *shown) * k;
            if (target - *shown).abs() > SETTLED {
                moving = true;
            } else {
                *shown = *target;
            }
        }
        moving
    }

    pub fn values(&self) -> &[f32] {
        &self.shown
    }
}

/// `targets` smoothed across frames, kept in egui's memory under `id`.
/// Asks for another frame while the values are still on their way.
pub fn smoothed(ui: &Ui, id: Id, targets: &[f32], tau: f32) -> Vec<f32> {
    let dt = ui.input(|input| input.stable_dt);
    let mut smoother: Smoother = ui.data(|data| data.get_temp(id)).unwrap_or_default();
    if smoother.step(targets, dt, tau) {
        ui.ctx().request_repaint_after(FRAME);
    }
    let values = smoother.values().to_vec();
    ui.data_mut(|data| data.insert_temp(id, smoother));
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The completion condition of `V2-6`: a reading that changes at 30 Hz,
    /// drawn at 60 Hz, moves on **every** frame rather than every other one.
    #[test]
    fn a_30_hz_reading_drawn_at_60_hz_moves_every_frame() {
        let mut smoother = Smoother::default();
        smoother.step(&[0.0], 0.0, TAU);
        let frame = 1.0 / 60.0;
        let mut last = 0.0;
        for index in 1..60 {
            // The reading updates on every second frame, ramping up.
            let reading = ((index + 1) / 2) as f32 * 0.02;
            smoother.step(&[reading], frame, TAU);
            let shown = smoother.values()[0];
            assert!(
                shown > last,
                "frame {index} did not move: {shown} after {last}"
            );
            last = shown;
        }
    }

    #[test]
    fn it_arrives_and_stops_asking_for_frames() {
        let mut smoother = Smoother::default();
        smoother.step(&[0.0], 0.0, TAU);
        let mut moving = true;
        for _ in 0..120 {
            moving = smoother.step(&[1.0], 1.0 / 60.0, TAU);
        }
        assert!(!moving);
        assert_eq!(smoother.values(), &[1.0]);
    }

    /// A slow frame takes a bigger step, so the shown value is where it would
    /// have been at 60 Hz.
    #[test]
    fn the_step_follows_the_frame_time() {
        let mut fast = Smoother::default();
        let mut slow = Smoother::default();
        fast.step(&[0.0], 0.0, TAU);
        slow.step(&[0.0], 0.0, TAU);
        fast.step(&[1.0], 1.0 / 60.0, TAU);
        fast.step(&[1.0], 1.0 / 60.0, TAU);
        slow.step(&[1.0], 2.0 / 60.0, TAU);
        assert!((fast.values()[0] - slow.values()[0]).abs() < 1e-5);
    }
}
