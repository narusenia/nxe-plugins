//! Buckle: the waveform turns over when it crosses the threshold
//! (`nonlinear-kit-plan.md`, decision 7).
//!
//! Below the threshold the signal passes as it is. When the waveform crosses
//! it, the channel **buckles**: the curve turns to `x·(1 − 2·snap)`, upside
//! down at full snap. It stays buckled until the waveform falls below half the
//! threshold, so the way up and the way down take different curves — a
//! hysteresis loop, which is where the harmonics come from. The turn takes
//! [`SNAP_SECONDS`] rather than one sample: a hard step aliases even at 4x.

use crate::{Channel, OVERSAMPLE, mix};

/// Where a buckled channel lets go, as a fraction of the threshold (−6 dB).
pub const RELEASE: f32 = 0.5;
/// How long the turn takes.
const SNAP_SECONDS: f32 = 0.0005;

/// The curve on each side of the loop: `buckled` is `0..=1`.
pub fn shape(x: f32, snap: f32, buckled: f32) -> f32 {
    x * (1.0 - 2.0 * snap * buckled)
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Linear amplitude.
    pub threshold: f32,
    /// `0..=1`.
    pub snap: f32,
    /// `0..=1`.
    pub mix: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Loop {
    buckled: bool,
    amount: f32,
}

impl Loop {
    fn step(&mut self, x: f32, threshold: f32, glide: f32) -> f32 {
        let level = x.abs();
        if level > threshold {
            self.buckled = true;
        } else if level < threshold * RELEASE {
            self.buckled = false;
        }
        let target = if self.buckled { 1.0 } else { 0.0 };
        self.amount += (target - self.amount) * glide;
        self.amount
    }
}

pub struct Buckle {
    channels: [Channel; 2],
    loops: [Loop; 2],
    glide: f32,
}

impl Buckle {
    pub fn new(sample_rate: f32) -> Self {
        let oversampled = sample_rate * OVERSAMPLE as f32;
        Self {
            channels: [Channel::new(sample_rate), Channel::new(sample_rate)],
            loops: [Loop::default(); 2],
            glide: 1.0 - (-1.0 / (SNAP_SECONDS * oversampled)).exp(),
        }
    }

    /// How buckled the louder channel is, `0..=1`.
    pub fn buckled(&self) -> f32 {
        self.loops[0].amount.max(self.loops[1].amount)
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let Settings {
            threshold,
            snap,
            mix: amount,
        } = *settings;
        let glide = self.glide;
        let mut output = [0.0; 2];
        for (index, (channel, state)) in self.channels.iter_mut().zip(&mut self.loops).enumerate() {
            output[index] = channel.process(input[index], |value| {
                let buckled = state.step(value, threshold, glide);
                mix(value, shape(value, snap, buckled), amount)
            });
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::{rms, sine};

    const RATE: f32 = 48_000.0;

    fn run(amplitude: f32, settings: &Settings) -> (Vec<f32>, Vec<f32>) {
        let mut buckle = Buckle::new(RATE);
        (0..24_000)
            .map(|index| {
                let x = sine(index, 220.0, amplitude, RATE);
                (x, buckle.process([x, x], settings)[0])
            })
            .unzip()
    }

    const FULL: Settings = Settings {
        threshold: 0.5,
        snap: 1.0,
        mix: 1.0,
    };

    #[test]
    fn below_the_threshold_it_passes() {
        let (dry, wet) = run(0.4, &FULL);
        let ratio = rms(&wet[4_800..]) / rms(&dry[4_800..]);
        assert!((ratio - 1.0).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn above_the_threshold_it_turns_over() {
        // The peaks of a loud sine come out upside down: the output's largest
        // excursion is on the other side from the input's.
        let (dry, wet) = run(0.9, &FULL);
        let tail = 4_800..;
        let correlation: f32 = dry[tail.clone()]
            .iter()
            .zip(&wet[tail])
            .map(|(a, b)| a * b)
            .sum();
        assert!(correlation < 0.0, "{correlation}");
    }

    #[test]
    fn the_loop_holds_until_half_the_threshold() {
        let mut state = Loop::default();
        state.step(0.6, 0.5, 1.0);
        assert!(state.buckled);
        state.step(0.3, 0.5, 1.0);
        assert!(state.buckled, "still above the release");
        state.step(0.2, 0.5, 1.0);
        assert!(!state.buckled);
    }

    #[test]
    fn no_snap_is_no_change() {
        assert_eq!(shape(0.8, 0.0, 1.0), 0.8);
    }
}
