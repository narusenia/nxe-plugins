//! `DENSITY`: a compressor **on the generator bus's input and nowhere else**.
//!
//! **Move candidate**: nothing here knows about Velour (`REQ-VEL-015`).
//!
//! ## Why it is safe to push it all the way
//!
//! The dry path does not go through this (`REQ-VEL-001`), so the worst
//! `DENSITY` at 100% can do is flatten *how much texture gets added*. The
//! singer's dynamics are not in the signal being compressed. What every other
//! saturator has to promise in its manual — "it keeps your dynamics" — is here
//! a consequence of where the block sits (`REQ-VEL-007`).
//!
//! ## What it takes its reading from
//!
//! The shared detector (`crate::envelope`), which watches the **input**, before
//! this stage. Reading its own output would mean `EMOTION` stopped reacting as
//! `DENSITY` came up, because it would be reading a signal `DENSITY` had
//! already flattened (`REQ-VEL-008`). So there is no follower in here: the
//! ballistics are the detector's, and the gain below is a static function of
//! its reading.
//!
//! ## Why the makeup is static
//!
//! A makeup that tracked the reduction would be a second gain following the
//! signal, on top of the first — the two would multiply and the result is
//! pumping. This one is decided by the threshold and the ratio alone.

use crate::envelope::REFERENCE_DB;

/// The threshold at `DENSITY` = 0, and how far it travels by 100%
/// (`dsp.md`). **Ear-tuned** (`VEL-17`).
pub const THRESHOLD_TOP_DB: f32 = -6.0;
pub const THRESHOLD_TRAVEL_DB: f32 = 24.0;

/// The ratio goes `1:1` to `4:1`. At `1:1` the stage is arithmetically absent,
/// which is what makes "off" exact rather than nearly (`REQ-VEL-007`).
pub const RATIO_TRAVEL: f32 = 3.0;

/// `2^x` and `log2(x)` are cheaper than `powf`/`log10`, and a gain in dB is
/// `2^(dB / (20·log10(2)))`.
const DECIBELS_PER_OCTAVE: f32 = 6.020_6;

/// The compressor, resolved. **Built at block rate, evaluated per sample.**
pub struct Density {
    /// Linear amplitude, not dB: the per-sample path compares against the
    /// detector's linear reading.
    threshold: f32,
    /// `1 − 1/R`, the exponent that turns an overshoot ratio into a gain.
    exponent: f32,
    /// Linear, and already including the reduction the reference level takes.
    makeup: f32,
    active: bool,
}

impl Default for Density {
    fn default() -> Self {
        Self::new()
    }
}

impl Density {
    /// Off.
    pub fn new() -> Self {
        let mut density = Density {
            threshold: 1.0,
            exponent: 0.0,
            makeup: 1.0,
            active: false,
        };
        density.set(0.0);
        density
    }

    /// **Block rate.** `amount` is `0..=1`; out of range is clamped, because
    /// this arrives from a host (`REQ-VEL-016`).
    pub fn set(&mut self, amount: f32) {
        let amount = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.active = amount > 0.0;
        if !self.active {
            self.threshold = 1.0;
            self.exponent = 0.0;
            self.makeup = 1.0;
            return;
        }

        let threshold_db = THRESHOLD_TOP_DB - THRESHOLD_TRAVEL_DB * amount;
        let ratio = 1.0 + RATIO_TRAVEL * amount;
        self.exponent = 1.0 - 1.0 / ratio;
        self.threshold = decibels(threshold_db);

        // **Referenced to a vocal, not to full scale.** The textbook auto
        // makeup is `−T·(1 − 1/R)`, which restores 0 dBFS to 0 dBFS — and
        // therefore lifts a −18 dB vocal by about 13 dB at the top of the
        // range. That is not "the same loudness with the differences
        // flattened", it is a volume knob with a compressor attached, and the
        // acceptance condition for this unit is ±1 dB (`REQ-VEL-007`).
        //
        // Giving back exactly what the reference level loses puts the pivot
        // where the material is: a phrase at `REFERENCE_DB` comes out where it
        // went in, louder phrases come down, quieter ones come up. Which is the
        // whole point — the *differences* flatten, not the level.
        self.makeup = 1.0 / self.gain_of(decibels(REFERENCE_DB));
    }

    /// **Per sample.** `level` is the detector's linear reading
    /// (`nxe_audio::Envelope::level`).
    ///
    /// Exactly `1.0` while the amount is zero, so an untouched control costs
    /// one comparison and changes nothing.
    pub fn gain(&self, level: f32) -> f32 {
        if !self.active {
            return 1.0;
        }
        self.gain_of(level) * self.makeup
    }

    /// The reduction alone, before the makeup. `1.0` below the threshold.
    fn gain_of(&self, level: f32) -> f32 {
        // The NaN check is not defensive noise: this is compared against a
        // reading, and `level <= threshold` is false for a NaN, which would
        // send it into the logarithm.
        if level.is_nan() || level <= self.threshold {
            return 1.0;
        }
        // `(threshold / level)^exponent`, which is below one because the level
        // is above the threshold.
        (self.exponent * (self.threshold / level).log2()).exp2()
    }
}

fn decibels(value: f32) -> f32 {
    (value / DECIBELS_PER_OCTAVE).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn density_at(amount: f32) -> Density {
        let mut density = Density::new();
        density.set(amount);
        density
    }

    fn db(value: f32) -> f32 {
        DECIBELS_PER_OCTAVE * value.log2()
    }

    /// **The acceptance condition** (`REQ-VEL-007`): off is off, at every level.
    #[test]
    fn zero_is_exactly_no_reduction_and_no_makeup() {
        let density = density_at(0.0);
        for level_db in [-60.0f32, -18.0, -6.0, 0.0, 6.0] {
            assert_eq!(density.gain(decibels(level_db)), 1.0, "{level_db} dB");
        }
    }

    /// **Referenced to a vocal**: the level the material sits at comes out
    /// where it went in, whatever the amount. This is the ±1 dB condition, and
    /// it holds exactly rather than approximately.
    #[test]
    fn the_reference_level_passes_at_unity() {
        for amount in [0.0f32, 0.1, 0.5, 0.9, 1.0] {
            let gain = density_at(amount).gain(decibels(REFERENCE_DB));
            assert!(
                db(gain).abs() < 0.01,
                "amount {amount}: {:+.3} dB",
                db(gain)
            );
        }
    }

    /// What the control is *for*: the span between a quiet phrase and a loud one
    /// gets smaller, and it keeps getting smaller as the knob goes up.
    #[test]
    fn it_narrows_the_gap_between_a_quiet_phrase_and_a_loud_one() {
        let quiet = decibels(-30.0);
        let loud = decibels(-6.0);

        let mut previous = f32::INFINITY;
        for amount in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let density = density_at(amount);
            let span = db(density.gain(loud) * loud) - db(density.gain(quiet) * quiet);
            assert!(span > 0.0, "amount {amount} inverted the order");
            assert!(
                span < previous,
                "amount {amount}: {span:.2} dB is not below {previous:.2} dB"
            );
            previous = span;
        }
        // 24 dB in, and the top of the range has to actually do something about
        // it.
        assert!(
            previous < 12.0,
            "the top of the range only reached {previous:.2} dB"
        );
    }

    /// A compressor, not an expander: nothing above the threshold comes out
    /// louder than it went in relative to the reference.
    #[test]
    fn it_never_turns_a_loud_phrase_up() {
        for amount in [0.25f32, 0.5, 1.0] {
            let density = density_at(amount);
            for level_db in [-18.0f32, -12.0, -6.0, 0.0] {
                let gain = db(density.gain(decibels(level_db)));
                assert!(
                    gain <= 0.01,
                    "amount {amount} at {level_db} dB: {gain:+.2} dB"
                );
            }
        }
    }

    /// And below the threshold the makeup is all there is, so a quiet phrase is
    /// lifted by a constant — the reason it is static (no pumping).
    #[test]
    fn below_the_threshold_the_gain_is_constant() {
        let density = density_at(1.0);
        let low = density.gain(decibels(-70.0));
        let higher = density.gain(decibels(-50.0));
        assert_eq!(low, higher);
        assert!(low > 1.0, "the makeup did not lift a quiet phrase: {low}");
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        for amount in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1.0, 2.0] {
            let density = density_at(amount);
            for level in [0.0f32, 1.0, 1e9, f32::NAN, f32::INFINITY] {
                let gain = density.gain(level);
                assert!(gain.is_finite() || !level.is_finite(), "{amount} / {level}");
            }
        }
    }
}
