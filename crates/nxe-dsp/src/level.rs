//! How loud one channel is, with meter ballistics.
//!
//! Feeds an in/out meter pair, which exists to answer one question: **is this
//! louder, or is it better?** A saturator always changes the level, so the
//! comparison is the whole point — and that makes it the reading's only real
//! requirement. The absolute numbers matter less than the two ends of the
//! comparison behaving identically, which one struct used twice guarantees.
//!
//! **Deliberately not a standard.** Naming VU, PPM, or K-system would oblige
//! the ballistics to match that standard's, and nothing here needs that.
//!
//! Amplitudes out, not decibels: where the floor of a display sits is the
//! display's decision (the same call [`crate::Spectrum`] makes).

/// How long the peak follower takes to fall to `1/e` with nothing feeding it.
/// Fast enough to track a phrase, slow enough that the bar is readable rather
/// than flickering.
const PEAK_RELEASE_SECONDS: f32 = 0.300;

/// How long the hold marker sits still before it starts to fall.
const HOLD_SECONDS: f32 = 1.500;

/// And how fast it falls once it does. Slower than the bar, so the marker stays
/// legible as a separate thing.
const HOLD_RELEASE_SECONDS: f32 = 1.000;

/// The averaging time of the RMS reading.
const RMS_SECONDS: f32 = 0.100;

/// One channel's level.
pub struct Level {
    /// Rises instantly, falls exponentially.
    peak: f32,
    /// The highest peak lately, held then released.
    hold: f32,
    hold_samples_left: u32,
    /// A one-pole average of the squared signal. Rooted in [`Level::rms`].
    mean_square: f32,
    peak_decay: f32,
    hold_decay: f32,
    hold_samples: u32,
    rms_coefficient: f32,
}

impl Level {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            peak: 0.0,
            hold: 0.0,
            hold_samples_left: 0,
            mean_square: 0.0,
            peak_decay: decay(PEAK_RELEASE_SECONDS, sample_rate),
            hold_decay: decay(HOLD_RELEASE_SECONDS, sample_rate),
            hold_samples: (HOLD_SECONDS * sample_rate) as u32,
            // A one-pole's step response reaches `1 - 1/e` in one time
            // constant, which is the same definition the decays above use.
            rms_coefficient: 1.0 - decay(RMS_SECONDS, sample_rate),
        }
    }

    /// Feeds one sample. Call for every frame on the channel being measured.
    pub fn push(&mut self, sample: f32) {
        let magnitude = sample.abs();

        // Instant attack. A meter that misses the transient it was watching for
        // is worse than one that holds it a moment too long.
        self.peak = if magnitude > self.peak {
            magnitude
        } else {
            self.peak * self.peak_decay
        };

        // **The hold lives here rather than in the display.** The hand-off to
        // the UI is latest-wins and drops readings (`crate::Handoff`), so a
        // display that tracked its own maximum would silently miss the peaks it
        // exists to catch.
        if magnitude >= self.hold {
            self.hold = magnitude;
            self.hold_samples_left = self.hold_samples;
        } else if self.hold_samples_left > 0 {
            self.hold_samples_left -= 1;
        } else {
            self.hold *= self.hold_decay;
        }

        self.mean_square += (sample * sample - self.mean_square) * self.rms_coefficient;
    }

    /// The peak follower, as an amplitude. `1.0` is full scale.
    pub fn peak(&self) -> f32 {
        self.peak
    }

    /// The held maximum, as an amplitude.
    pub fn hold(&self) -> f32 {
        self.hold
    }

    /// The averaged level, as an amplitude. A full-scale sine reads `1/√2`.
    pub fn rms(&self) -> f32 {
        self.mean_square.max(0.0).sqrt()
    }

    /// Forgets everything. For a transport stop, or reopening an editor onto a
    /// silent track.
    pub fn reset(&mut self) {
        self.peak = 0.0;
        self.hold = 0.0;
        self.hold_samples_left = 0;
        self.mean_square = 0.0;
    }
}

/// What a value is multiplied by each sample to fall to `1/e` in `seconds`.
fn decay(seconds: f32, sample_rate: f32) -> f32 {
    (-1.0 / (seconds * sample_rate)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn silence(level: &mut Level, seconds: f32) {
        for _ in 0..(seconds * SR) as usize {
            level.push(0.0);
        }
    }

    fn sine(level: &mut Level, amplitude: f32, seconds: f32) {
        for index in 0..(seconds * SR) as usize {
            level.push(amplitude * (TAU * 1_000.0 * index as f32 / SR).sin());
        }
    }

    #[test]
    fn a_full_scale_sine_reads_full_scale() {
        let mut level = Level::new(SR);
        sine(&mut level, 1.0, 0.5);
        assert!((level.peak() - 1.0).abs() < 0.01, "peak {}", level.peak());
    }

    /// The one number here that has a right answer rather than a chosen one.
    #[test]
    fn rms_of_a_sine_is_the_amplitude_over_root_two() {
        let mut level = Level::new(SR);
        sine(&mut level, 1.0, 1.0);
        let expected = 1.0 / 2.0f32.sqrt();
        assert!(
            (level.rms() - expected).abs() < 0.01,
            "rms {} vs {expected}",
            level.rms()
        );
    }

    #[test]
    fn silence_falls_away() {
        let mut level = Level::new(SR);
        sine(&mut level, 1.0, 0.2);
        silence(&mut level, 2.0);

        assert!(level.peak() < 0.01, "peak {}", level.peak());
        assert!(level.rms() < 0.01, "rms {}", level.rms());
    }

    /// The reason the hold is in the DSP and not the display.
    #[test]
    fn the_hold_outlasts_the_bar() {
        let mut level = Level::new(SR);
        level.push(1.0);
        silence(&mut level, PEAK_RELEASE_SECONDS * 3.0);

        assert!(level.peak() < 0.1, "the bar did not fall: {}", level.peak());
        assert!(level.hold() > 0.9, "the hold fell early: {}", level.hold());
    }

    #[test]
    fn the_hold_falls_eventually() {
        let mut level = Level::new(SR);
        level.push(1.0);
        silence(&mut level, HOLD_SECONDS + HOLD_RELEASE_SECONDS * 4.0);

        assert!(level.hold() < 0.1, "hold {}", level.hold());
    }

    #[test]
    fn a_quiet_signal_reads_quiet() {
        let mut loud = Level::new(SR);
        sine(&mut loud, 1.0, 1.0);

        let mut quiet = Level::new(SR);
        sine(&mut quiet, 0.1, 1.0);

        let ratio = quiet.rms() / loud.rms();
        assert!((0.08..=0.12).contains(&ratio), "the ratio was {ratio}");
    }

    /// The ballistics are defined in seconds, so they have to be the same in
    /// wall-clock terms whatever the host is running at.
    #[test]
    fn the_ballistics_are_the_same_at_every_sample_rate() {
        let mut remaining = Vec::new();

        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut level = Level::new(rate);
            level.push(1.0);
            for _ in 0..(PEAK_RELEASE_SECONDS * rate) as usize {
                level.push(0.0);
            }
            remaining.push(level.peak());
        }

        // One time constant leaves `1/e`.
        for value in &remaining {
            assert!(
                (value - std::f32::consts::E.recip()).abs() < 0.01,
                "{remaining:?}"
            );
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut level = Level::new(SR);
        sine(&mut level, 1.0, 0.2);
        level.reset();

        assert_eq!(level.peak(), 0.0);
        assert_eq!(level.hold(), 0.0);
        assert_eq!(level.rms(), 0.0);
    }
}
