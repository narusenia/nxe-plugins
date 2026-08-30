//! `WIDTH`: how far the space spreads, and how the voice stays put.
//!
//! **The reflections widen; the direct sound does not move** (`REQ-DIO-007`).
//! The two halves are different operations on purpose:
//!
//! - the reflections get a mid/side balance, which is what `WIDTH` controls;
//! - the direct sound gets its **side attenuated and nothing else**. Not placed
//!   in the side, not delayed — attenuated. That is what makes the promise
//!   below structural rather than a measurement that happened to come out well.
//!
//! ## The promise, and why it is not Air's
//!
//! Air can say "no comb in the mono sum" because it never rotates a phase
//! (`REQ-AIR-008`). Early reflections *are* delays, so that promise cannot be
//! made here. What replaces it (`REQ-DIO-007`):
//!
//! > The **direct sound** is in the mid channel only, so a mono sum leaves it
//! > untouched. The reflections lose level in a mono sum, and that is what
//! > happens to a real room.
//!
//! And "in the mid channel only" is provable rather than measurable: a mono sum
//! is `(L + R) / 2`, the side gain multiplies `(L - R) / 2`, and the two are
//! orthogonal — so **the side gain cannot appear in a mono sum at all**, at any
//! setting, for any input.
//!
//! Specified in `plugins/diorama/docs/specifications/dsp.md`, "WIDTH".

use nxe_audio::envelope::coefficient;

/// How narrow the direct sound gets at the near end, and how wide at the far
/// end. **Attenuation only** — the far end is 0 dB, not a boost.
const SIDE_NEAR_DB: f32 = -3.0;
const SIDE_FAR_DB: f32 = 0.0;

/// How much of `WIDTH` the reflections get at the near and far ends of
/// `distance`. **Never above 1** — a side gain over unity is a widener, and
/// this is a room.
const REFLECTED_NEAR: f32 = 0.6;
const REFLECTED_SPAN: f32 = 0.4;

/// How fast the two gains follow the parameters. **The same 5 ms as everything
/// else here**, and the same reason: a gain that steps at block rate is a seam
/// (`DIO-1`).
const GAIN_SECONDS: f32 = 0.005;

/// Close enough to count as arrived. **Not smaller** — below about
/// `ulp(value) / coefficient` a one-pole stops changing the sum (`DIO-1`).
const GAIN_SETTLED: f32 = 1e-4;

pub struct Width {
    /// The reflections' side gain.
    reflected: Ramp,
    /// The direct sound's side gain.
    direct: Ramp,
}

impl Width {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            reflected: Ramp::new(1.0, sample_rate),
            direct: Ramp::new(1.0, sample_rate),
        };
        built.set(1.0, 0.5);
        built.reflected.snap();
        built.direct.snap();
        built
    }

    /// Resolves both side gains. **Block rate.**
    pub fn set(&mut self, width: f32, distance: f32) {
        let width = unit(width);
        let distance = unit(distance);

        self.reflected
            .set(width * (REFLECTED_NEAR + REFLECTED_SPAN * distance));

        let side_db = SIDE_NEAR_DB + (SIDE_FAR_DB - SIDE_NEAR_DB) * distance;
        self.direct.set(10.0f32.powf(side_db / 20.0));
    }

    /// The reflection bus, widened. **Audio rate.**
    pub fn reflected(&mut self, left: f32, right: f32) -> (f32, f32) {
        balance(left, right, self.reflected.next())
    }

    /// The direct sound, with its side attenuated. **Audio rate.**
    pub fn direct(&mut self, left: f32, right: f32) -> (f32, f32) {
        balance(left, right, self.direct.next())
    }

    /// What the reflections' width does to their power, for the loudness
    /// normalisation (`DIO-3`).
    ///
    /// **This one term *is* signal-independent**, unlike the presence band and
    /// the damping corners. Mid/side changes a signal's power by an amount that
    /// depends on how correlated its two channels are — and the reflections'
    /// correlation is a property of **this** design rather than of the
    /// material: the two channels share no tap time, and `DIO-1` measured
    /// -0.006. For uncorrelated channels the factor is exactly `(1 + s²) / 2`.
    pub fn reflected_power_factor(&self) -> f32 {
        let s = self.reflected.target;
        (1.0 + s * s) / 2.0
    }

    /// The direct sound's side gain, for the display (`REQ-DIO-018`).
    pub fn direct_side_gain(&self) -> f32 {
        self.direct.target
    }

    pub fn reset(&mut self) {
        self.reflected.snap();
        self.direct.snap();
    }
}

/// `out = M ± s·S`. At `s` = 1 this is the identity, bit for bit.
fn balance(left: f32, right: f32, side_gain: f32) -> (f32, f32) {
    if side_gain == 1.0 {
        return (left, right);
    }
    let mid = (left + right) * 0.5;
    let side = (left - right) * 0.5 * side_gain;
    (mid + side, mid - side)
}

/// One smoothed gain.
struct Ramp {
    value: f32,
    target: f32,
    coefficient: f32,
    settling: bool,
}

impl Ramp {
    fn new(value: f32, sample_rate: f32) -> Self {
        Self {
            value,
            target: value,
            coefficient: coefficient(GAIN_SECONDS, sample_rate),
            settling: false,
        }
    }

    fn set(&mut self, target: f32) {
        if target != self.target {
            self.target = target;
            self.settling = true;
        }
    }

    fn next(&mut self) -> f32 {
        if self.settling {
            let remaining = self.target - self.value;
            if remaining.abs() < GAIN_SETTLED {
                self.value = self.target;
                self.settling = false;
            } else {
                self.value += remaining * self.coefficient;
            }
        }
        self.value
    }

    fn snap(&mut self) {
        self.value = self.target;
        self.settling = false;
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics;

    const RATE: f32 = 48_000.0;

    /// **The side gain cannot reach a mono sum.** Not "measured flat" —
    /// arithmetically unable to, because `(L + R) / 2` and `(L - R) / 2` are
    /// orthogonal. This is the whole of `REQ-DIO-007`'s promise about the direct
    /// sound, and it holds for every setting and every input.
    #[test]
    fn the_direct_sound_survives_a_mono_sum_untouched() {
        let left = harmonics::pink(0.5, 8_192);
        let right = harmonics::noise(0.5, 8_192);

        for distance in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut width = Width::new(RATE);
            width.set(1.0, distance);
            width.reset();

            for index in 0..left.len() {
                let (out_left, out_right) = width.direct(left[index], right[index]);
                let summed = (out_left + out_right) * 0.5;
                let expected = (left[index] + right[index]) * 0.5;
                assert!(
                    (summed - expected).abs() < 1e-6,
                    "distance {distance}, sample {index}: {summed} against {expected}"
                );
            }
        }
    }

    /// A `WIDTH` of one is the identity, bit for bit — so a session that does
    /// not want the space widened pays nothing for the option.
    #[test]
    fn full_width_is_bit_identical_on_the_reflections() {
        let mut width = Width::new(RATE);
        width.set(1.0, 1.0);
        width.reset();

        let left = harmonics::pink(0.5, 4_096);
        let right = harmonics::noise(0.5, 4_096);
        for index in 0..left.len() {
            assert_eq!(
                width.reflected(left[index], right[index]),
                (left[index], right[index])
            );
        }
    }

    /// **Zero leaves the two channels identical** (`REQ-DIO-011`).
    #[test]
    fn zero_width_collapses_the_reflections_to_mono() {
        let mut width = Width::new(RATE);
        width.set(0.0, 1.0);
        width.reset();

        let left = harmonics::pink(0.5, 4_096);
        let right = harmonics::noise(0.5, 4_096);
        for index in 0..left.len() {
            let (out_left, out_right) = width.reflected(left[index], right[index]);
            assert_eq!(out_left, out_right);
        }
    }

    /// The reflections spread as the voice goes away, and the direct sound goes
    /// the other way — narrow when close (`REQ-DIO-002`'s table).
    ///
    /// **This is the mirrored pair the rules warn about**: the sign of either
    /// mapping is invisible in anything but a test like this.
    #[test]
    fn distance_widens_the_reflections_and_releases_the_direct_sound() {
        let mut near = Width::new(RATE);
        near.set(1.0, 0.0);
        let mut far = Width::new(RATE);
        far.set(1.0, 1.0);

        assert!(
            far.reflected.target > near.reflected.target,
            "the reflections did not widen: {} against {}",
            far.reflected.target,
            near.reflected.target
        );
        assert!(
            near.direct.target < far.direct.target,
            "the direct sound was not narrower when close: {} against {}",
            near.direct.target,
            far.direct.target
        );
        // And the direct side is only ever attenuated, never boosted.
        assert!(far.direct.target <= 1.0);
    }

    /// The power factor the normalisation uses has to be what the operation
    /// actually does to uncorrelated channels — **the doubled arithmetic the
    /// rules warn about**, written once here and once in `depth::Probe`.
    #[test]
    fn the_power_factor_matches_the_operation() {
        // **Two genuinely uncorrelated channels.** `pink` is `noise` put
        // through a filter, so the two of them are *correlated* — the first
        // version of this test used one of each and measured 0.739 where 0.5
        // was predicted, which is the correlation, not a wrong factor
        // (`DIO-6`). Reversing one channel decorrelates it (`AIR-1` does the
        // same).
        let left = harmonics::noise(0.5, 1 << 16);
        let right: Vec<f32> = left.iter().rev().copied().collect();

        for target in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let mut width = Width::new(RATE);
            width.set(target, 1.0);
            width.reset();

            let mut before = 0.0;
            let mut after = 0.0;
            for index in 0..left.len() {
                let (out_left, out_right) = width.reflected(left[index], right[index]);
                before += left[index] * left[index] + right[index] * right[index];
                after += out_left * out_left + out_right * out_right;
            }

            let measured = after / before;
            let predicted = width.reflected_power_factor();
            let difference = 10.0 * (measured / predicted).log10();
            assert!(
                difference.abs() < 0.3,
                "width {target}: measured {measured:.4}, predicted {predicted:.4} \
                 ({difference:.2} dB apart)"
            );
        }
    }

    /// The spectral version of the promise above, with the control the plan
    /// asks for (`DIO-6`).
    ///
    /// **The structural argument is the real proof** — a side gain is
    /// orthogonal to a mono sum — but a measurement that cannot fail proves
    /// nothing either (`VEL-10`), so this shows the same measurement finding a
    /// comb the moment one is put there.
    ///
    /// **The input is centred**, which is both the realistic case for a voice
    /// and the only case where a comb can form at all: two *uncorrelated*
    /// channels sum by power, so no phase relationship between them can notch
    /// anything. The first version of this test fed it decorrelated noise and
    /// its control could not produce a comb however the allpass was tuned —
    /// 2.5 dB, which read as "no comb" (`DIO-6`).
    ///
    /// The control is an **allpass**, not a delay, and a **short** one: notch
    /// spacing is `1 / delay`, so a long allpass combs finer than a third of an
    /// octave and averages away inside the bands (`AIR-1` paid for that).
    #[test]
    fn a_mono_sum_of_the_direct_sound_has_no_comb_in_it() {
        const BANDS: usize = 24;
        let signal = harmonics::noise(0.5, 1 << 17);

        // The spread of the per-band ratio between two mono sums, in dB.
        let spread = |processed: &[f32], reference: &[f32]| {
            let mut a = nxe_dsp::Spectrum::<BANDS>::new(RATE, 100.0, 16_000.0);
            let mut b = nxe_dsp::Spectrum::<BANDS>::new(RATE, 100.0, 16_000.0);
            for index in 0..processed.len() {
                a.push(processed[index]);
                b.push(reference[index]);
            }
            let (one, two) = (a.levels(), b.levels());
            let ratios: Vec<f32> = (0..BANDS)
                .map(|band| 20.0 * (one[band].max(1e-12) / two[band].max(1e-12)).log10())
                .collect();
            let lowest = ratios.iter().copied().fold(f32::INFINITY, f32::min);
            let highest = ratios.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            highest - lowest
        };

        // The narrowest setting, which is where the side gain is furthest from
        // one and therefore where a comb would be worst if one could exist.
        let mut width = Width::new(RATE);
        width.set(1.0, 0.0);
        width.reset();
        let summed: Vec<f32> = signal
            .iter()
            .map(|&sample| {
                let (left, right) = width.direct(sample, sample);
                (left + right) * 0.5
            })
            .collect();
        let measured = spread(&summed, &signal);

        // The control: one channel through a short allpass before the sum.
        let delay = (RATE * 0.00025) as usize;
        let mut line = vec![0.0f32; delay];
        let mut cursor = 0usize;
        let combed: Vec<f32> = signal
            .iter()
            .map(|&sample| {
                let delayed = line[cursor];
                let out = -0.9 * sample + delayed;
                line[cursor] = sample + 0.9 * out;
                cursor = (cursor + 1) % delay;
                (sample + out) * 0.5
            })
            .collect();
        let control = spread(&combed, &signal);

        assert!(
            control > 6.0,
            "the control did not produce a comb: {control:.2} dB"
        );
        assert!(
            measured < 0.5,
            "the direct sound combed its own mono sum by {measured:.2} dB \
             (control {control:.2})"
        );
    }

    /// **The reflections decorrelate as the voice goes away**
    /// (`REQ-DIO-007`), measured on the pair that actually ships: the tap sets
    /// provide the decorrelation and `WIDTH` decides how much of it is let
    /// through.
    #[test]
    fn distance_lowers_the_correlation_of_the_reflections() {
        let signal = harmonics::noise(0.5, RATE as usize);

        let correlation_at = |distance: f32| {
            let mut reflections = crate::Reflections::new(RATE);
            reflections.set(crate::reflections::Settings {
                distance,
                amount: 1.0,
            });
            let mut width = Width::new(RATE);
            width.set(1.0, distance);
            width.reset();

            let mut correlation = nxe_dsp::Correlation::new(RATE);
            for (index, &sample) in signal.iter().enumerate() {
                let (left, right) = reflections.process(sample, sample);
                let (left, right) = width.reflected(left, right);
                // A quarter of a second for the line to fill and the followers
                // to settle (`SPK-18`).
                if index > RATE as usize / 4 {
                    correlation.push(left, right);
                }
            }
            correlation.value()
        };

        let near = correlation_at(0.0);
        let far = correlation_at(1.0);
        assert!(
            far < near,
            "the reflections did not decorrelate with distance: {far:.3} against {near:.3}"
        );
    }

    /// **A source that sits off centre stays off centre, on the same side**
    /// (`REQ-DIO-007`).
    ///
    /// What the requirement forbids is the *image moving*, not the width
    /// changing — narrowing the direct sound is in `REQ-DIO-002`'s table, and a
    /// side attenuation necessarily pulls a panned source toward the middle.
    /// What it may not do is move it asymmetrically or swap the sides, and that
    /// is what this pins.
    #[test]
    fn a_source_off_centre_stays_on_its_own_side() {
        let signal = harmonics::noise(0.5, 8_192);

        for distance in [0.0f32, 0.5, 1.0] {
            let mut width = Width::new(RATE);
            width.set(1.0, distance);
            width.reset();

            let mut left_energy = 0.0;
            let mut right_energy = 0.0;
            for &sample in &signal {
                // Hard left in.
                let (left, right) = width.direct(sample, 0.0);
                left_energy += left * left;
                right_energy += right * right;
            }
            assert!(
                left_energy > right_energy,
                "distance {distance}: a hard-left source came out with {left_energy} left \
                 and {right_energy} right"
            );
        }
    }

    /// Hostile values in, finite values out (`REQ-DIO-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut width = Width::new(RATE);
        for (value, distance) in [
            (f32::NAN, 0.5f32),
            (f32::INFINITY, f32::NEG_INFINITY),
            (1e9, -1e9),
        ] {
            width.set(value, distance);
            assert!(width.reflected_power_factor().is_finite());
            assert!(width.direct_side_gain().is_finite());
        }

        width.set(0.5, 0.5);
        width.reset();
        for sample in [1e9f32, -1e9, 0.5] {
            let (left, right) = width.reflected(sample, -sample);
            assert!(left.is_finite() && right.is_finite());
        }
    }
}
