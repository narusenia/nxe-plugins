//! How alike the two channels are, from `+1` to `-1`.
//!
//! **The one number a widener owes its user.** Anything that makes a stereo
//! image out of one source — detuning a copy, delaying it, panning the pair
//! apart — does it by making the channels differ, and the far end of that is a
//! pair that cancels when summed to mono. The picture of where the energy sits
//! (`crate::PanScope`) says the image is wide; it cannot say whether the width
//! survives a mono fold. This can.
//!
//! | reading | what it means |
//! |---|---|
//! | `+1` | the channels are the same signal — mono |
//! | `0` | unrelated; wide, and it survives a fold |
//! | `-1` | one is the other inverted — **it disappears in mono** |
//!
//! ## Why a one-pole and not a window
//!
//! A block average over a fixed window is the textbook estimator, and it needs
//! a buffer and a modulo. Three one-poles need three multiply-adds and no
//! memory, and the reading is asked for thirty times a second by a display —
//! a smoothed estimate is what a display wants anyway, because an exact one
//! flickers.
//!
//! Amplitudes in, a plain ratio out: where a display puts `0` on its scale is
//! the display's decision (the same call [`crate::Level`] makes).

/// The averaging time. Long enough that a single transient does not swing the
/// reading, short enough to follow a section change.
const SECONDS: f32 = 0.400;

/// The point below which the reading is not a correlation but a division of two
/// very small numbers. Silence has no correlation, and reporting one from
/// rounding noise is worse than reporting none.
const FLOOR: f32 = 1e-9;

/// The largest amplitude that is squared rather than rejected.
///
/// **The squares overflow long before the inputs do.** A finite `1e30` squares
/// to `1e60`, which is not a finite `f32`, and `inf / inf` is a NaN — so
/// checking the input for finiteness is not enough on its own. A million is
/// six orders of magnitude past anything audio reaches, and its square has
/// twenty-six left before the exponent runs out.
const CEILING: f32 = 1e6;

pub struct Correlation {
    /// One-pole averages of `l·r`, `l²` and `r²`. The estimate is the first
    /// over the root of the product of the other two.
    product: f32,
    left_square: f32,
    right_square: f32,
    coefficient: f32,
}

impl Correlation {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            product: 0.0,
            left_square: 0.0,
            right_square: 0.0,
            // A one-pole's step response reaches `1 - 1/e` in one time
            // constant, which is the definition `Level` uses too.
            coefficient: 1.0 - (-1.0 / (SECONDS * sample_rate.max(1.0))).exp(),
        }
    }

    /// One frame. **Audio rate**, allocation-free.
    pub fn push(&mut self, left: f32, right: f32) {
        let (left, right) = (finite(left), finite(right));
        let step = |average: f32, value: f32| average + (value - average) * self.coefficient;
        self.product = step(self.product, left * right);
        self.left_square = step(self.left_square, left * left);
        self.right_square = step(self.right_square, right * right);
    }

    /// The reading, `-1..=1`. Zero while there is nothing to compare.
    pub fn value(&self) -> f32 {
        let energy = (self.left_square * self.right_square).max(0.0).sqrt();
        if energy < FLOOR {
            return 0.0;
        }
        (self.product / energy).clamp(-1.0, 1.0)
    }

    pub fn reset(&mut self) {
        self.product = 0.0;
        self.left_square = 0.0;
        self.right_square = 0.0;
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-CEILING, CEILING)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// Long enough for the averages to settle at this time constant.
    fn settled(pair: impl Fn(usize) -> (f32, f32)) -> f32 {
        let mut correlation = Correlation::new(RATE);
        for index in 0..(RATE as usize * 3) {
            let (left, right) = pair(index);
            correlation.push(left, right);
        }
        correlation.value()
    }

    fn tone(index: usize, hz: f32) -> f32 {
        (std::f32::consts::TAU * hz * index as f32 / RATE).sin()
    }

    /// **The three readings the scale is named by.**
    #[test]
    fn the_ends_and_the_middle_read_as_named() {
        let mono = settled(|i| {
            let s = tone(i, 220.0);
            (s, s)
        });
        assert!((mono - 1.0).abs() < 0.01, "the same signal read {mono:.3}");

        let inverted = settled(|i| {
            let s = tone(i, 220.0);
            (s, -s)
        });
        assert!(
            (inverted + 1.0).abs() < 0.01,
            "an inverted pair read {inverted:.3}"
        );

        // Two tones far enough apart that neither divides the other: their
        // product averages away.
        let unrelated = settled(|i| (tone(i, 220.0), tone(i, 317.0)));
        assert!(
            unrelated.abs() < 0.05,
            "unrelated signals read {unrelated:.3}"
        );
    }

    /// **Silence has no correlation**, and rounding noise must not be reported
    /// as one.
    #[test]
    fn silence_reads_zero_rather_than_a_ratio_of_nothings() {
        assert_eq!(settled(|_| (0.0, 0.0)), 0.0);
        // A signal on one side only is not a correlation either: there is
        // nothing on the other side to be alike.
        let one_sided = settled(|i| (tone(i, 220.0), 0.0));
        assert_eq!(one_sided, 0.0);
    }

    /// **It does not depend on how loud the pair is**, which is what makes it a
    /// correlation rather than a level.
    #[test]
    fn the_reading_does_not_depend_on_gain() {
        let readings: Vec<f32> = [0.001f32, 0.1, 1.0]
            .iter()
            .map(|gain| {
                let gain = *gain;
                settled(move |i| {
                    let s = tone(i, 220.0);
                    (s * gain, s * gain * 0.5)
                })
            })
            .collect();
        for reading in &readings {
            assert!(
                (reading - 1.0).abs() < 0.01,
                "gain changed the reading: {readings:?}"
            );
        }
    }

    /// Hostile input neither panics nor escapes the scale.
    #[test]
    fn hostile_values_stay_in_range() {
        let mut correlation = Correlation::new(RATE);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30] {
            correlation.push(value, value);
            let reading = correlation.value();
            assert!(
                reading.is_finite() && (-1.0..=1.0).contains(&reading),
                "{value} produced {reading}"
            );
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut correlation = Correlation::new(RATE);
        for index in 0..1_000 {
            let s = tone(index, 220.0);
            correlation.push(s, s);
        }
        correlation.reset();
        assert_eq!(correlation.value(), 0.0);
    }
}
