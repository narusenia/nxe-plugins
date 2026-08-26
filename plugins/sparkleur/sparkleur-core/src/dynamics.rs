//! What gain each band should be given, from how loud it is.
//!
//! **The core of the product** (`REQ-SPK-003`). Two compressors per band off
//! one detector: downward on what is above `T_down`, upward on what is below
//! `T_up`. The pair is what "up-and-down" means, and the upward half is the
//! half that needs machinery to be safe.
//!
//! ## Upward compression needs a ceiling and a floor
//!
//! Upward lifts **everything** under its threshold, so left bare it lifts the
//! noise floor and the tail of every release. The ceiling stops a silence being
//! raised all the way to the threshold; the floor fades the lift out below
//! `FLOOR_DB` so that what is not there stays not there. That fade is the
//! difference between "up-and-down" and the pumping OTT is known for on bass.
//!
//! **`LIFT` opens the floor** down to [`FLOOR_MIN_DB`] (`REQ-SPK-003`). Which
//! of the two limits actually binds depends on `CHARACTER`: with the POLISH and
//! GLOSS ratios the lift never reaches the ceiling before the floor fades it
//! out, and with CRUSH the ceiling binds first (`SPK-4`).
//!
//! ## Sub Protect is not a process
//!
//! It is the bottom band's ceiling being smaller (`REQ-SPK-008`), so it belongs
//! to whatever builds the [`Curve`] — `SPK-7`.
//!
//! ## Pure
//!
//! Nothing here has state. The properties that matter — that `SPARK` = 0 is
//! exactly nothing, that neither side ever inverts, that the knee has no corner
//! — are testable without running a signal, which is the same reason
//! `nxe_audio::guard::gain_of` is a free function.

use crate::crossover::BAND_COUNT;

/// The provisional thresholds, in dB **on the detector's scale** — which reads
/// a band's RMS plus the asymmetry of its follower (`SPK-3`). They are settled
/// by ear in `SPK-18`, and they cannot be calculated for exactly that reason.
pub const DOWN_THRESHOLD_DB: f32 = -18.0;
pub const UP_THRESHOLD_DB: f32 = -36.0;

/// Where the upward side stops working, and how far below that it has faded out
/// entirely.
pub const FLOOR_DB: f32 = -60.0;
pub const FADE_DB: f32 = 12.0;
/// The furthest `LIFT` may open the floor (`REQ-SPK-003`).
pub const FLOOR_MIN_DB: f32 = -90.0;

/// A gain in dB is `2^(dB / (20·log10(2)))`.
const DECIBELS_PER_OCTAVE_AMPLITUDE: f32 = 6.020_6;

/// The widest gain this will ever ask for, in dB either way.
///
/// **Not a taste limit — an arithmetic one.** Far past anything `CHARACTER` can
/// reach (the ceiling tops out at 15 dB), and there so that a hostile threshold
/// or trim cannot hand the audio path a multiplier of infinity. `SPK-4` found
/// that the hard way: a `GAIN` of 1e9 dB is a finite number of decibels and an
/// infinite gain.
pub const MAX_GAIN_DB: f32 = 48.0;

/// The shape of both curves — what `CHARACTER` decides (`SPK-5`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub down_threshold_db: f32,
    pub down_ratio: f32,
    pub up_threshold_db: f32,
    pub up_ratio: f32,
    /// The width of the soft knee, shared by both sides. Wide is gentle, 0 is a
    /// corner.
    pub knee_db: f32,
    /// The most the upward side may lift, in dB.
    pub ceiling_db: f32,
}

impl Curve {
    /// The middle of the `CHARACTER` axis (`dsp.md`). Until `SPK-5` builds the
    /// axis, this is what everything runs at.
    pub const GLOSS: Self = Self {
        down_threshold_db: DOWN_THRESHOLD_DB,
        down_ratio: 2.5,
        up_threshold_db: UP_THRESHOLD_DB,
        up_ratio: 1.5,
        knee_db: 6.0,
        ceiling_db: 9.0,
    };
}

impl Default for Curve {
    fn default() -> Self {
        Self::GLOSS
    }
}

/// One band's share of the macro amount (`REQ-SPK-009`).
///
/// **`down` and `up` are weights, not amounts.** `SPARK` multiplies them, so
/// raising it deepens every band and leaves their proportions alone. `gain_db`
/// is not a weight — see [`band_gain_db`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub down: f32,
    pub up: f32,
    pub gain_db: f32,
}

impl Weights {
    pub const NEUTRAL: Self = Self {
        down: 1.0,
        up: 1.0,
        gain_db: 0.0,
    };
}

impl Default for Weights {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

/// Everything the gain computer needs that is not the level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub curve: Curve,
    pub weights: [Weights; BAND_COUNT],
    /// The macro amount, `0..=1`. **Zero is exactly off** (`REQ-SPK-009`).
    pub spark: f32,
    /// Where the upward side stops working, in dB — what `LIFT` moves.
    pub floor_db: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            curve: Curve::GLOSS,
            weights: [Weights::NEUTRAL; BAND_COUNT],
            spark: 0.0,
            floor_db: FLOOR_DB,
        }
    }
}

/// Every band's gain, in dB.
pub fn gains_db(settings: &Settings, levels_db: [f32; BAND_COUNT]) -> [f32; BAND_COUNT] {
    let mut gains = [0.0f32; BAND_COUNT];
    for ((gain, level), weights) in gains.iter_mut().zip(levels_db).zip(&settings.weights) {
        *gain = band_gain_db(
            level,
            &settings.curve,
            weights,
            settings.spark,
            settings.floor_db,
        );
    }
    gains
}

/// One band's gain, in dB.
///
/// **`gain_db` is added outside `SPARK`**, because it is a static trim and not
/// dynamics (`dsp.md`). The consequence is worth saying out loud: `SPARK` = 0
/// is amplitude-flat *at the default weights*, and a band the user has trimmed
/// keeps its trim.
pub fn band_gain_db(
    level_db: f32,
    curve: &Curve,
    weights: &Weights,
    spark: f32,
    floor_db: f32,
) -> f32 {
    let spark = finite(spark, 0.0).clamp(0.0, 1.0);
    let floor_db = finite(floor_db, FLOOR_DB);
    // A level that is not a number is read as "nothing there", which is the
    // side that does nothing.
    let level_db = finite(level_db, floor_db - FADE_DB);

    let down = -kneed_db(
        level_db - finite(curve.down_threshold_db, DOWN_THRESHOLD_DB),
        curve.down_ratio,
        curve.knee_db,
    );
    let up = kneed_db(
        finite(curve.up_threshold_db, UP_THRESHOLD_DB) - level_db,
        curve.up_ratio,
        curve.knee_db,
    )
    .min(finite(curve.ceiling_db, 0.0).max(0.0))
        * taper(level_db, floor_db);

    let weighted = down * finite(weights.down, 0.0).clamp(0.0, 1.0)
        + up * finite(weights.up, 0.0).clamp(0.0, 1.0);

    // Bounded, so that whatever comes out of here is a multiplier and not an
    // infinity (see `MAX_GAIN_DB`).
    (weighted * spark + finite(weights.gain_db, 0.0)).clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
}

/// How much of the upward lift survives at this level, `0..=1`.
///
/// One at `floor_db` and zero [`FADE_DB`] below it. **Without this, silence and
/// the tail of every release come up** (`REQ-SPK-003`).
pub fn taper(level_db: f32, floor_db: f32) -> f32 {
    ((level_db - (floor_db - FADE_DB)) / FADE_DB).clamp(0.0, 1.0)
}

/// A gain in dB as a plain multiplier. Never zero, never infinite.
pub fn linear(gain_db: f32) -> f32 {
    let gain_db = finite(gain_db, 0.0).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
    (gain_db / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2()
}

/// How far one side moves the gain, as a positive number of dB.
///
/// `excess_db` is how far past the threshold the level is **in the direction
/// that side acts** — above it for downward, below it for upward — so the two
/// are mirror images of one function.
///
/// The knee is the usual quadratic: it meets the flat part and the sloped part
/// with the same value *and the same slope*, which is what makes the gain have
/// no corner to hear.
fn kneed_db(excess_db: f32, ratio: f32, knee_db: f32) -> f32 {
    // Below 1 is expansion, which is a different product. Clamping here means a
    // hostile ratio flattens the side rather than inverting it.
    let slope = 1.0 - 1.0 / finite(ratio, 1.0).max(1.0);
    let knee_db = finite(knee_db, 0.0).max(0.0);
    let half = knee_db * 0.5;
    let excess_db = finite(excess_db, -1.0);

    if knee_db <= f32::EPSILON || excess_db >= half {
        slope * excess_db.max(0.0)
    } else if excess_db <= -half {
        0.0
    } else {
        slope * (excess_db + half).powi(2) / (2.0 * knee_db)
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURVE: Curve = Curve::GLOSS;
    const NEUTRAL: Weights = Weights::NEUTRAL;

    fn gain(level_db: f32) -> f32 {
        band_gain_db(level_db, &CURVE, &NEUTRAL, 1.0, FLOOR_DB)
    }

    /// **`SPARK` = 0 is exactly nothing** (`REQ-SPK-009`) — not nearly, at any
    /// level and whatever the curve says.
    #[test]
    fn spark_at_zero_is_exactly_zero_db() {
        for ratio in [1.0f32, 2.5, 6.0, 100.0] {
            let curve = Curve {
                down_ratio: ratio,
                up_ratio: ratio,
                ..CURVE
            };
            for level in [-100.0f32, -60.0, -36.0, -18.0, -6.0, 0.0, 12.0] {
                let reading = band_gain_db(level, &curve, &NEUTRAL, 0.0, FLOOR_DB);
                assert_eq!(reading, 0.0, "ratio {ratio} at {level} dB gave {reading}");
            }
        }
    }

    /// The downward side compresses by the ratio it was given, measured the way
    /// the completion condition says: how much the output moves for 12 dB in.
    #[test]
    fn the_downward_ratio_is_what_it_says() {
        for ratio in [1.5f32, 2.5, 6.0] {
            let curve = Curve {
                down_ratio: ratio,
                ..CURVE
            };
            let at = |level| band_gain_db(level, &curve, &NEUTRAL, 1.0, FLOOR_DB);

            // Both well above the knee, so the slope is the ratio's alone.
            let low = curve.down_threshold_db + 12.0;
            let expected = -12.0 * (1.0 - 1.0 / ratio);
            let measured = at(low + 12.0) - at(low);
            assert!(
                (measured - expected).abs() < 0.01,
                "ratio {ratio}: 12 dB in moved the gain {measured:.3} dB, wanted {expected:.3}"
            );
            // And the output does still rise: a compressor is not a limiter.
            assert!(measured > -12.0, "ratio {ratio} swallowed the input whole");
        }
    }

    #[test]
    fn the_upward_ratio_is_what_it_says() {
        for ratio in [1.2f32, 1.5, 3.0] {
            let curve = Curve {
                up_ratio: ratio,
                ceiling_db: 100.0,
                ..CURVE
            };
            let at = |level| band_gain_db(level, &curve, &NEUTRAL, 1.0, FLOOR_MIN_DB);

            // Below the knee and above the floor's fade.
            let high = curve.up_threshold_db - 12.0;
            let expected = 12.0 * (1.0 - 1.0 / ratio);
            let measured = at(high - 12.0) - at(high);
            assert!(
                (measured - expected).abs() < 0.01,
                "ratio {ratio}: 12 dB down lifted {measured:.3} dB, wanted {expected:.3}"
            );
        }
    }

    /// **The ceiling holds, and it is reachable** — a bound nothing ever hits
    /// would prove nothing.
    #[test]
    fn the_lift_stops_at_the_ceiling_and_does_reach_it() {
        // With the floor open, the CRUSH ratio runs into the ceiling.
        let curve = Curve {
            up_ratio: 3.0,
            ceiling_db: 15.0,
            ..CURVE
        };
        let mut reached = false;
        for step in 0..=90 {
            let level = -step as f32;
            let reading = band_gain_db(level, &curve, &NEUTRAL, 1.0, FLOOR_MIN_DB);
            assert!(
                reading <= curve.ceiling_db + 1e-4,
                "{level} dB lifted {reading:.3}, past the {} dB ceiling",
                curve.ceiling_db
            );
            reached |= (reading - curve.ceiling_db).abs() < 1e-3;
        }
        assert!(
            reached,
            "the ceiling was never reached, so the bound is vacuous"
        );
    }

    /// **Which limit binds is a `CHARACTER` question.** With the gentle ratios
    /// the floor fades the lift out before the ceiling ever applies; with CRUSH
    /// the ceiling applies first. Worth pinning, because it decides which knob
    /// a listener is actually hearing (`SPK-18`).
    #[test]
    fn the_gentle_ratios_never_reach_their_ceiling_at_the_default_floor() {
        for (ratio, ceiling) in [(1.2f32, 6.0f32), (1.5, 9.0)] {
            let curve = Curve {
                up_ratio: ratio,
                ceiling_db: ceiling,
                ..CURVE
            };
            let highest = (0..=100)
                .map(|step| band_gain_db(-step as f32, &curve, &NEUTRAL, 1.0, FLOOR_DB))
                .fold(f32::MIN, f32::max);
            assert!(
                highest < ceiling - 0.5,
                "ratio {ratio} reached {highest:.2} dB against a {ceiling} dB ceiling"
            );
        }

        // CRUSH does reach its ceiling even at the default floor.
        let crush = Curve {
            up_ratio: 3.0,
            ceiling_db: 15.0,
            ..CURVE
        };
        let highest = (0..=100)
            .map(|step| band_gain_db(-step as f32, &crush, &NEUTRAL, 1.0, FLOOR_DB))
            .fold(f32::MIN, f32::max);
        assert!(highest > 14.5, "CRUSH only reached {highest:.2} dB");
    }

    /// **Silence stays silent** (`REQ-SPK-003`). This is the difference between
    /// up-and-down compression and pumping.
    #[test]
    fn silence_is_not_lifted() {
        for level in [-100.0f32, -90.0, -80.0, -72.0] {
            assert_eq!(gain(level), 0.0, "{level} dB was lifted");
        }
    }

    /// And `LIFT` is what lets it be — the escape hatch the product needs to
    /// stand in for VO-TT (`REQ-SPK-003`).
    #[test]
    fn opening_the_floor_lets_the_quiet_come_up() {
        let level = -80.0;
        assert_eq!(gain(level), 0.0);

        let lifted = band_gain_db(level, &CURVE, &NEUTRAL, 1.0, FLOOR_MIN_DB);
        assert!(lifted > 5.0, "opening the floor only gave {lifted:.2} dB");
    }

    /// **No corner at the threshold.** A second difference finds a kink that a
    /// first difference smooths over.
    #[test]
    fn the_knee_has_no_corner() {
        const STEP: f32 = 0.1;

        fn worst_bend(curve: &Curve) -> f32 {
            let at = |level| band_gain_db(level, curve, &NEUTRAL, 1.0, FLOOR_DB);
            let mut worst = 0.0f32;
            // Across both thresholds, and clear of the floor's fade.
            for index in 0..500 {
                let level = -50.0 + index as f32 * STEP;
                let bend = (at(level - STEP) - 2.0 * at(level) + at(level + STEP)).abs();
                worst = worst.max(bend);
            }
            worst
        }

        let soft = worst_bend(&CURVE);
        assert!(soft < 5e-3, "the soft knee bent {soft:.5} dB");

        // And the measurement can fail: a knee of zero is a corner, and it
        // shows up an order of magnitude larger.
        let hard = worst_bend(&Curve {
            knee_db: 0.0,
            ..CURVE
        });
        assert!(
            hard > soft * 10.0,
            "a hard knee bent {hard:.5}, no worse than the soft one"
        );
    }

    /// Per-band weights scale the dynamics and nothing else (`REQ-SPK-009`).
    #[test]
    fn the_weights_scale_linearly() {
        for level in [-48.0f32, -30.0, -6.0, 6.0] {
            let full = gain(level);
            for weight in [0.0f32, 0.25, 0.5, 1.0] {
                let weights = Weights {
                    down: weight,
                    up: weight,
                    gain_db: 0.0,
                };
                let reading = band_gain_db(level, &CURVE, &weights, 1.0, FLOOR_DB);
                assert!(
                    (reading - full * weight).abs() < 1e-4,
                    "{level} dB at weight {weight}: {reading:.4}, wanted {:.4}",
                    full * weight
                );
            }
        }
    }

    /// And so does `SPARK`, which is the same relationship one layer up.
    #[test]
    fn spark_scales_the_whole_correction() {
        for level in [-48.0f32, -30.0, -6.0, 6.0] {
            let full = gain(level);
            for spark in [0.0f32, 0.25, 0.5, 1.0] {
                let reading = band_gain_db(level, &CURVE, &NEUTRAL, spark, FLOOR_DB);
                assert!(
                    (reading - full * spark).abs() < 1e-4,
                    "{level} dB at {spark}"
                );
            }
        }
    }

    /// **The per-band `GAIN` is a trim, not dynamics.** It sits outside `SPARK`
    /// (`dsp.md`), so a band the user has trimmed keeps its trim with the
    /// dynamics turned all the way off.
    #[test]
    fn the_per_band_gain_survives_spark_at_zero() {
        let weights = Weights {
            gain_db: -3.0,
            ..NEUTRAL
        };
        for level in [-100.0f32, -30.0, 0.0] {
            let reading = band_gain_db(level, &CURVE, &weights, 0.0, FLOOR_DB);
            assert_eq!(reading, -3.0, "{level} dB lost the trim");
        }
    }

    /// Between the two thresholds neither side is acting, so the gain is zero —
    /// the band the material was already sitting in is the one left alone.
    #[test]
    fn the_two_sides_do_not_overlap() {
        for level in [-30.0f32, -27.0, -24.0] {
            assert_eq!(gain(level), 0.0, "{level} dB was moved");
        }
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 0.0, 1e9];
        for level in wild {
            for value in wild {
                let curve = Curve {
                    down_threshold_db: value,
                    down_ratio: value,
                    up_threshold_db: value,
                    up_ratio: value,
                    knee_db: value,
                    ceiling_db: value,
                };
                let weights = Weights {
                    down: value,
                    up: value,
                    gain_db: value,
                };
                for spark in wild {
                    let reading = band_gain_db(level, &curve, &weights, spark, value);
                    assert!(
                        reading.abs() <= MAX_GAIN_DB,
                        "{level}/{value}/{spark} gave {reading}"
                    );
                    // **The bug this test found**: a finite number of decibels
                    // is not a finite gain.
                    let multiplier = linear(reading);
                    assert!(
                        multiplier > 0.0 && multiplier.is_finite(),
                        "{reading} dB became {multiplier}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_band_is_computed_from_its_own_weights() {
        let mut settings = Settings {
            spark: 1.0,
            ..Settings::default()
        };
        settings.weights[2] = Weights {
            down: 0.0,
            up: 0.0,
            gain_db: 0.0,
        };

        let gains = gains_db(&settings, [-6.0; BAND_COUNT]);
        for (band, reading) in gains.iter().enumerate() {
            if band == 2 {
                assert_eq!(*reading, 0.0, "the muted band moved");
            } else {
                assert!(*reading < -1.0, "band {band} did nothing: {reading}");
            }
        }
    }

    #[test]
    fn a_gain_of_zero_db_is_exactly_unity() {
        assert_eq!(linear(0.0), 1.0);
        assert!((linear(-6.0206) - 0.5).abs() < 1e-4);
        assert!((linear(6.0206) - 2.0).abs() < 1e-4);
    }
}
