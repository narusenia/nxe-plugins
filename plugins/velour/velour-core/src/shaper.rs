//! The one curve family every generator uses.
//!
//! **Move candidate**: Sparkleur's Harmonic Sparkle is the same block, so this
//! module knows nothing about Velour (`REQ-VEL-015`).
//!
//! ```text
//! raw(x)   = f(k·x + β) − f(β)
//! shape(x) = raw(x) / g
//! ```
//!
//! `k` is drive and brings odd harmonics, `β` is an asymmetric bias and brings
//! even ones, and `f` blends a soft knee into a hard one under `h`
//! (`plugins/velour/docs/specifications/dsp.md`). Subtracting `f(β)` removes
//! the offset the bias would otherwise park on the output.
//!
//! **`g` is what makes the four control layers independent.** Without it,
//! turning drive up is also turning the band down, and then `DRIVE` and the
//! band's own level are two knobs fighting over one thing (`REQ-VEL-009`).
//!
//! ## `g` is not `k·f'(β)`
//!
//! The specification first normalised by the curve's slope at the origin, which
//! fixes the gain for *infinitesimal* signals. It does not survive contact with
//! a real one: a compressive curve loses fundamental as it saturates, so the
//! band still got quieter as drive came up — about 12 dB across the range at a
//! realistic input level. Normalising the small-signal gain measures the one
//! amplitude nobody sends.
//!
//! So `g` is the curve's **RMS gain for a sine at a reference amplitude**,
//! integrated over one period whenever the three controls change. That is a
//! fixed reference, not a measurement of the signal: the same settings always
//! behave the same way, whatever is playing. Program-dependent normalisation
//! would make `DRIVE` behave differently on every source, which is worse than
//! the problem it solves.
//!
//! **The trade is that it is only exact at the reference amplitude.** Four times
//! louder and drive costs 11.6 dB across its range; four times quieter and it
//! gains 9.1 dB. Both are pinned by tests rather than left to be discovered.
//! The reference sits where a mixed vocal sits, so the exactness is where the
//! plugin is used, and `DENSITY` narrows what is left by bringing the bus
//! toward a consistent level before the curve sees it (`REQ-VEL-007`).
//!
//! There is a normalisation with no drift at all — integrate against the
//! signal's own envelope instead of a fixed amplitude — and it is the wrong
//! answer twice over. It makes the same settings sound different on every
//! source, and it is a slow AGC on the generator, which would flatten exactly
//! the "sing louder, get more edge" behaviour `EMOTION` exists to produce
//! (`REQ-VEL-008`).

use std::f32::consts::TAU;

/// Drive. The bottom is not zero because `g` divides by it in the limit, and a
/// linear curve is what the bottom is *for* — at `DRIVE_MIN` the shaper passes
/// its input through with unity gain (`REQ-VEL-009`).
pub const DRIVE_MIN: f32 = 0.05;
pub const DRIVE_MAX: f32 = 20.0;

/// How far the bias can push the curve off centre.
///
/// **Bounded by the hard curve's knee, not by taste.** `f_hard` flattens at
/// `±1`, so a bias sitting on the knee leaves the curve with no slope for the
/// signal to ride, `g` collapses toward zero, and the division blows up. `0.8`
/// keeps a working slope at every hardness.
pub const BIAS_MAX: f32 = 0.8;

/// Where the soft curve reaches `±1`. The Padé form is exact there — `f_soft(3)`
/// is `1` and its slope is `0` — so clamping at this point leaves no corner.
const SOFT_LIMIT: f32 = 3.0;

/// How many points one period of the normalising integral is sampled at. The
/// error this leaves is pinned by a test; 64 is where it stops mattering.
const PROBE_POINTS: usize = 64;

/// The amplitude the RMS gain is normalised at: −12 dBFS peak, which is about
/// where a mixed vocal peaks.
pub const PROBE_AMPLITUDE: f32 = 0.25;

/// Below this `g` is treated as unusable and the shaper passes its input
/// through. Nothing in range should reach it; it is here so that no combination
/// of hostile values can divide by zero (`REQ-VEL-016`).
const GAIN_FLOOR: f32 = 1e-4;

/// The soft knee: a Padé approximant of `tanh`, clamped where it reaches `±1`.
///
/// **Not `tanh` itself.** One divide beats a transcendental on the audio path,
/// the error inside `±3` is under half a percent, and the approximant meets the
/// clamp with matching value *and* slope, which `tanh` scaled to do the same
/// would not.
fn f_soft(x: f32) -> f32 {
    if x <= -SOFT_LIMIT {
        return -1.0;
    }
    if x >= SOFT_LIMIT {
        return 1.0;
    }
    let squared = x * x;
    x * (27.0 + squared) / (27.0 + 9.0 * squared)
}

/// The hard knee: a cubic soft clip that hits its ceiling at `±1`, so it
/// saturates far sooner than [`f_soft`] and gives up higher odd harmonics.
fn f_hard(x: f32) -> f32 {
    if x <= -1.0 {
        return -1.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    1.5 * x - 0.5 * x * x * x
}

/// The curve at hardness `h`.
///
/// A blend of two curves rather than one curve with an exponent on its knee:
/// the exponent form needs a `pow` **per sample**, and this needs two
/// polynomials and a lerp.
fn f(x: f32, hardness: f32) -> f32 {
    let hard = hardness.clamp(0.0, 1.0);
    (1.0 - hard) * f_soft(x) + hard * f_hard(x)
}

/// One generator's curve, resolved.
///
/// Cheap to evaluate and expensive to build, which is the right way round: the
/// three controls move at block rate and the curve is evaluated per sample.
/// **One instance serves both channels** — there is no per-sample state here.
#[derive(Clone)]
pub struct Shaper {
    drive: f32,
    bias: f32,
    hardness: f32,
    /// `f(β)`, subtracted so the bias leaves no standing offset.
    offset: f32,
    /// `1/g`.
    scale: f32,
    /// One period of the probe sine. Held rather than recomputed so that
    /// changing a control costs polynomials and no transcendentals.
    probe: [f32; PROBE_POINTS],
}

impl Default for Shaper {
    fn default() -> Self {
        Self::new()
    }
}

impl Shaper {
    /// A linear shaper: drive at the bottom, no bias, no hardness.
    pub fn new() -> Self {
        let probe = std::array::from_fn(|index| {
            PROBE_AMPLITUDE * (TAU * index as f32 / PROBE_POINTS as f32).sin()
        });

        let mut shaper = Self {
            drive: f32::NAN,
            bias: f32::NAN,
            hardness: f32::NAN,
            offset: 0.0,
            scale: 1.0,
            probe,
        };
        shaper.set(DRIVE_MIN, 0.0, 0.0);
        shaper
    }

    /// Resolves the curve. **Block rate.**
    ///
    /// Out-of-range values are clamped rather than rejected: these arrive from a
    /// host, which is free to send anything (`REQ-VEL-016`).
    ///
    /// Returns without recomputing when nothing moved, so holding a knob still
    /// costs nothing.
    pub fn set(&mut self, drive: f32, bias: f32, hardness: f32) {
        let drive = clamp_or(drive, DRIVE_MIN, DRIVE_MAX, DRIVE_MIN);
        let bias = clamp_or(bias, 0.0, BIAS_MAX, 0.0);
        let hardness = clamp_or(hardness, 0.0, 1.0, 0.0);

        if drive == self.drive && bias == self.bias && hardness == self.hardness {
            return;
        }

        self.drive = drive;
        self.bias = bias;
        self.hardness = hardness;
        self.offset = f(bias, hardness);

        // The RMS the curve returns for the probe sine, against the RMS the
        // probe sine went in with.
        let mut energy = 0.0;
        for sample in &self.probe {
            let shaped = f(drive * sample + bias, hardness) - self.offset;
            energy += shaped * shaped;
        }
        let out = (energy / PROBE_POINTS as f32).sqrt();
        let reference = PROBE_AMPLITUDE / 2.0f32.sqrt();
        let gain = out / reference;

        self.scale = if gain > GAIN_FLOOR { 1.0 / gain } else { 1.0 };
    }

    /// One sample through the curve. **Audio rate.**
    pub fn shape(&self, input: f32) -> f32 {
        (f(self.drive * input + self.bias, self.hardness) - self.offset) * self.scale
    }

    pub fn drive(&self) -> f32 {
        self.drive
    }

    pub fn bias(&self) -> f32 {
        self.bias
    }

    pub fn hardness(&self) -> f32 {
        self.hardness
    }
}

/// Clamps into `low..=high`, falling back to `fallback` for a value that is not
/// a number. `f32::clamp` panics on a NaN bound and propagates a NaN value, and
/// a NaN reaching the coefficients would silence the plugin until it is
/// reloaded.
fn clamp_or(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{amplitude, db_ratio, mean, rms, sine};

    /// Cycles and length for the measurements: whole cycles so nothing leaks
    /// between bins, and long enough that the tenth harmonic still has one.
    const CYCLES: usize = 8;
    const LENGTH: usize = 2048;

    /// The controls, sampled across their ranges. Every claim below is checked
    /// against the whole grid, not against one flattering point.
    const DRIVES: [f32; 6] = [0.05, 0.5, 2.0, 5.0, 10.0, 20.0];
    const BIASES: [f32; 4] = [0.0, 0.25, 0.5, 0.8];
    const HARDNESSES: [f32; 3] = [0.0, 0.5, 1.0];

    fn run(shaper: &Shaper, input: &[f32]) -> Vec<f32> {
        input.iter().map(|sample| shaper.shape(*sample)).collect()
    }

    fn shaper_at(drive: f32, bias: f32, hardness: f32) -> Shaper {
        let mut shaper = Shaper::new();
        shaper.set(drive, bias, hardness);
        shaper
    }

    /// **The gate** (`VEL-1`, `REQ-VEL-003`).
    ///
    /// The whole four-layer parameter model rests on drive changing *how*
    /// distorted a band is without changing *how much* of it is added. At the
    /// amplitude the curve is normalised for, that has to hold across every
    /// combination of the three controls.
    #[test]
    fn drive_does_not_change_the_level() {
        let input = sine(PROBE_AMPLITUDE, CYCLES, LENGTH);
        let reference = rms(&input);

        for bias in BIASES {
            for hardness in HARDNESSES {
                for drive in DRIVES {
                    let shaper = shaper_at(drive, bias, hardness);
                    let drift = db_ratio(rms(&run(&shaper, &input)), reference);
                    assert!(
                        drift.abs() < 0.3,
                        "drive {drive}, bias {bias}, hardness {hardness}: {drift:+.2} dB"
                    );
                }
            }
        }
    }

    /// The honest limit of the gate above, pinned so a change to it is a
    /// decision rather than a surprise.
    ///
    /// The normalisation is exact at one amplitude. Four times that peaks at
    /// full scale, and there the curve really is saturating — which is what it
    /// is for. **Measured: −11.6 dB from the bottom of the drive range to the
    /// top.** That is the price of a fixed reference, and the alternatives are
    /// worse: normalising against the signal would make `DRIVE` behave
    /// differently on every source.
    ///
    /// It is also less exposed in use than it looks. The generator bus sits
    /// behind `DENSITY`'s compressor, whose whole job is to bring what reaches
    /// the curve closer to a consistent level (`REQ-VEL-007`).
    #[test]
    fn a_much_louder_signal_still_loses_some_level_as_drive_rises() {
        let input = sine(PROBE_AMPLITUDE * 4.0, CYCLES, LENGTH);
        let reference = rms(&input);

        let quiet_drive = db_ratio(rms(&run(&shaper_at(0.05, 0.0, 0.0), &input)), reference);
        let loud_drive = db_ratio(rms(&run(&shaper_at(20.0, 0.0, 0.0), &input)), reference);

        assert!(quiet_drive.abs() < 0.3, "{quiet_drive:+.2} dB at the bottom");
        assert!(loud_drive < quiet_drive, "it did not fall: {loud_drive:+.2} dB");
        assert!(loud_drive > -13.0, "it fell further than expected: {loud_drive:+.2} dB");
    }

    /// And the same reading from the other side. Material *quieter* than the
    /// reference drifts the other way: the compensation was computed for a
    /// curve that saturates, and down here it barely does, so it over-corrects.
    /// **Measured: +9.1 dB.**
    ///
    /// Taken together with the test above, the reference amplitude is not a
    /// detail — it is where the plugin's central claim is exactly true, and it
    /// degrades either side. `PROBE_AMPLITUDE` is therefore on the list of
    /// constants `VEL-17` settles by ear (`dsp.md`).
    #[test]
    fn a_much_quieter_signal_drifts_the_other_way() {
        let input = sine(PROBE_AMPLITUDE * 0.25, CYCLES, LENGTH);
        let reference = rms(&input);

        let loud_drive = db_ratio(rms(&run(&shaper_at(20.0, 0.0, 0.0), &input)), reference);
        assert!(loud_drive > 0.0, "it did not rise: {loud_drive:+.2} dB");
        assert!(loud_drive < 11.0, "it rose further than expected: {loud_drive:+.2} dB");
    }

    /// The bottom of the drive range is the "no harmonics" setting, and that has
    /// to be a genuine pass-through — otherwise adding a band with drive down is
    /// a coloured band boost rather than a clean one (`REQ-VEL-009`).
    #[test]
    fn the_bottom_of_the_drive_range_is_linear() {
        let shaper = shaper_at(DRIVE_MIN, 0.0, 0.0);
        for input in [-0.5f32, -0.1, -0.01, 0.0, 0.01, 0.1, 0.5] {
            let output = shaper.shape(input);
            assert!(
                (output - input).abs() < 1e-3,
                "{input} came out as {output}"
            );
        }
    }

    #[test]
    fn bias_brings_even_harmonics() {
        let input = sine(PROBE_AMPLITUDE, CYCLES, LENGTH);
        let mut previous = -1.0;

        for bias in BIASES {
            let output = run(&shaper_at(4.0, bias, 0.0), &input);
            let ratio = amplitude(&output, CYCLES * 2) / amplitude(&output, CYCLES);
            assert!(ratio > previous, "bias {bias} gave H2/H1 {ratio}, was {previous}");
            previous = ratio;
        }

        // And none at all without it: an odd curve cannot make an even harmonic.
        let output = run(&shaper_at(4.0, 0.0, 0.0), &input);
        assert!(amplitude(&output, CYCLES * 2) / amplitude(&output, CYCLES) < 1e-3);
    }

    #[test]
    fn drive_brings_odd_harmonics() {
        let input = sine(PROBE_AMPLITUDE, CYCLES, LENGTH);
        let mut previous = -1.0;

        for drive in DRIVES {
            let output = run(&shaper_at(drive, 0.0, 0.0), &input);
            let ratio = amplitude(&output, CYCLES * 3) / amplitude(&output, CYCLES);
            assert!(ratio > previous, "drive {drive} gave H3/H1 {ratio}, was {previous}");
            previous = ratio;
        }
    }

    /// What `TEXTURE` is actually moving when it goes from Warm to Edge.
    #[test]
    fn hardness_brings_more_harmonics_at_the_same_drive() {
        let input = sine(PROBE_AMPLITUDE, CYCLES, LENGTH);
        let mut previous = -1.0;

        for hardness in HARDNESSES {
            let output = run(&shaper_at(4.0, 0.0, hardness), &input);
            let ratio = amplitude(&output, CYCLES * 3) / amplitude(&output, CYCLES);
            assert!(
                ratio > previous,
                "hardness {hardness} gave H3/H1 {ratio}, was {previous}"
            );
            previous = ratio;
        }
    }

    /// Subtracting `f(β)` removes the offset at rest, and that is all it
    /// removes: an asymmetric curve fed a symmetric signal still returns a
    /// non-zero mean. **This is why every generator needs a high-pass on its
    /// output** (`dsp.md`), and the test says so rather than leaving it to be
    /// found later.
    #[test]
    fn an_unbiased_curve_leaves_no_dc_and_a_biased_one_does() {
        let input = sine(PROBE_AMPLITUDE, CYCLES, LENGTH);

        let straight = mean(&run(&shaper_at(4.0, 0.0, 0.0), &input));
        assert!(straight.abs() < 1e-4, "unbiased left {straight}");

        let biased = mean(&run(&shaper_at(4.0, 0.8, 0.0), &input));
        assert!(biased.abs() > 1e-3, "biased left only {biased}");
    }

    /// A host can send anything, including values no UI can produce.
    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e9,
            1e9,
            -1.0,
            0.0,
        ];

        for drive in wild {
            for bias in wild {
                for hardness in wild {
                    let shaper = shaper_at(drive, bias, hardness);
                    for input in [-1e9f32, -1.0, 0.0, 1.0, 1e9, f32::NAN] {
                        let output = shaper.shape(input);
                        // A NaN in is allowed to come out; anything else must be
                        // a real number bounded by the curve's own ceiling.
                        if input.is_finite() {
                            assert!(
                                output.is_finite(),
                                "{drive}/{bias}/{hardness} on {input} gave {output}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn out_of_range_controls_are_clamped_rather_than_kept() {
        let shaper = shaper_at(1e9, 1e9, 1e9);
        assert_eq!(shaper.drive(), DRIVE_MAX);
        assert_eq!(shaper.bias(), BIAS_MAX);
        assert_eq!(shaper.hardness(), 1.0);

        let shaper = shaper_at(f32::NAN, f32::NAN, f32::NAN);
        assert_eq!(shaper.drive(), DRIVE_MIN);
        assert_eq!(shaper.bias(), 0.0);
        assert_eq!(shaper.hardness(), 0.0);
    }

    /// Holding a knob still must not cost anything, so `set` has to notice that
    /// nothing moved — and noticing must not change the answer.
    #[test]
    fn setting_the_same_values_twice_changes_nothing() {
        let mut shaper = Shaper::new();
        shaper.set(6.0, 0.4, 0.7);
        let before: Vec<f32> = (0..16).map(|i| shaper.shape(i as f32 / 16.0)).collect();
        shaper.set(6.0, 0.4, 0.7);
        let after: Vec<f32> = (0..16).map(|i| shaper.shape(i as f32 / 16.0)).collect();
        assert_eq!(before, after);
    }

    /// The two knees have to meet their clamps without a corner, or the curve
    /// has a discontinuity in slope that shows up as a burst of high harmonics
    /// at one particular amplitude.
    #[test]
    fn both_knees_reach_their_ceilings_smoothly() {
        assert!((f_soft(SOFT_LIMIT) - 1.0).abs() < 1e-6);
        assert!((f_soft(SOFT_LIMIT - 1e-3) - 1.0).abs() < 1e-4);
        assert!((f_hard(1.0) - 1.0).abs() < 1e-6);
        assert!((f_hard(1.0 - 1e-3) - 1.0).abs() < 1e-4);
    }
}
