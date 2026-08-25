//! The two filter shapes Tone needs: a shelving biquad for the wet bus, and a
//! one-pole for the per-voice colour.
//!
//! Both are a handful of lines of arithmetic, which is exactly the kind of
//! thing worth owning rather than depending on (`.agents/rules/rust.md`).
//!
//! See `plugins/doubler/docs/specifications/dsp.md`.

use std::f32::consts::TAU;

/// A one-pole lowpass. `process` returns the lowpassed sample; a highpass is
/// `x - lowpass(x)`, which the caller does — one struct with two meanings would
/// be easy to use for the wrong role by accident.
#[derive(Clone, Copy, Default)]
pub struct OnePole {
    coefficient: f32,
    state: f32,
}

impl OnePole {
    pub fn set_cutoff(&mut self, sample_rate: f32, hz: f32) {
        let nyquist = sample_rate * 0.5;
        let hz = hz.clamp(1.0, nyquist * 0.99);
        self.coefficient = 1.0 - (-TAU * hz / sample_rate).exp();
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state += self.coefficient * (input - self.state);
        self.state
    }
}

/// A biquad in direct form I, used for the two wet-bus shelves.
#[derive(Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for Biquad {
    /// Passes signal through untouched until a shelf is set.
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

/// Which end of the spectrum a shelf lifts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shelf {
    Low,
    High,
}

impl Biquad {
    /// RBJ shelving coefficients at slope 1.
    ///
    /// At `gain_db == 0` the numerator and denominator come out identical, so
    /// the filter is transparent without a bypass branch.
    pub fn set_shelf(&mut self, shelf: Shelf, sample_rate: f32, hz: f32, gain_db: f32) {
        let amplitude = 10.0f32.powf(gain_db / 40.0);
        let omega = TAU * hz.clamp(1.0, sample_rate * 0.49) / sample_rate;
        let cos = omega.cos();
        // Slope 1 collapses the usual `(A + 1/A)(1/S - 1) + 2` to just 2.
        let alpha = omega.sin() * 0.5 * 2.0f32.sqrt();
        let sqrt_amplitude = amplitude.sqrt();

        let sum = amplitude + 1.0;
        let difference = amplitude - 1.0;
        let slope = 2.0 * sqrt_amplitude * alpha;

        let (b0, b1, b2, a0, a1, a2) = match shelf {
            Shelf::Low => (
                amplitude * (sum - difference * cos + slope),
                2.0 * amplitude * (difference - sum * cos),
                amplitude * (sum - difference * cos - slope),
                sum + difference * cos + slope,
                -2.0 * (difference + sum * cos),
                sum + difference * cos - slope,
            ),
            Shelf::High => (
                amplitude * (sum + difference * cos + slope),
                -2.0 * amplitude * (difference + sum * cos),
                amplitude * (sum + difference * cos - slope),
                sum - difference * cos + slope,
                2.0 * (difference - sum * cos),
                sum - difference * cos - slope,
            ),
        };

        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    /// The filter's gain at `hz`, in dB.
    ///
    /// The editor draws the Tone curve with this, so the picture comes from the
    /// same coefficients the audio goes through rather than from a second
    /// formula that approximates them.
    pub fn magnitude_db(&self, hz: f32, sample_rate: f32) -> f32 {
        let omega = TAU * hz / sample_rate;
        let (sin1, cos1) = omega.sin_cos();
        let (sin2, cos2) = (2.0 * omega).sin_cos();

        // H(e^{-jw}) with the coefficients already normalized by a0.
        let numerator_real = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let numerator_imag = -(self.b1 * sin1 + self.b2 * sin2);
        let denominator_real = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let denominator_imag = -(self.a1 * sin1 + self.a2 * sin2);

        let numerator = numerator_real.hypot(numerator_imag);
        let denominator = denominator_real.hypot(denominator_imag);
        if denominator <= f32::MIN_POSITIVE {
            return 0.0;
        }
        20.0 * (numerator / denominator).log10()
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Steady-state gain at a frequency, measured rather than derived, so a
    /// wrong coefficient shows up as a wrong number.
    ///
    /// The ratio of **RMS** values, not of peaks: a few samples per cycle miss
    /// the true peak, so a peak measurement reads low at high frequencies —
    /// by a whole dB at 8 kHz. Taking the input's own RMS over the same window
    /// cancels that out whatever the frequency.
    fn gain_at(filter: &mut Biquad, hz: f32) -> f32 {
        let cycles = 200;
        let samples = (SR / hz * cycles as f32) as usize;
        let settle = samples / 4;
        let mut input_energy = 0.0f64;
        let mut output_energy = 0.0f64;

        for i in 0..samples {
            let input = (i as f32 * TAU * hz / SR).sin();
            let output = filter.process(input);
            // Skip the first few cycles: the filter has to settle first.
            if i > settle {
                input_energy += (input * input) as f64;
                output_energy += (output * output) as f64;
            }
        }

        (output_energy / input_energy).sqrt() as f32
    }

    fn db(gain: f32) -> f32 {
        20.0 * gain.log10()
    }

    /// A shelf at unity has to pass signal through untouched, or `Tone` at zero
    /// would colour the wet bus (`REQ-DBL-005`).
    ///
    /// Checked twice. The coefficients have to cancel **exactly** — at unity
    /// gain the numerator and denominator are the same polynomial, and that is
    /// arithmetic, not approximation. The signal then only has to survive the
    /// recursion, where the left-to-right summation of five terms that cancel
    /// loses a few bits; the residual is around −100 dB and is a property of
    /// direct form I rather than of the coefficients.
    #[test]
    fn a_shelf_at_zero_db_is_transparent() {
        for shelf in [Shelf::Low, Shelf::High] {
            let mut filter = Biquad::default();
            filter.set_shelf(shelf, SR, 200.0, 0.0);

            assert_eq!(filter.b0, 1.0, "{shelf:?}");
            assert_eq!(filter.b1, filter.a1, "{shelf:?}");
            assert_eq!(filter.b2, filter.a2, "{shelf:?}");

            for i in 0..10_000 {
                let input = (i as f32 * 0.37).sin();
                let output = filter.process(input);
                let error = db((output - input).abs().max(f32::MIN_POSITIVE));
                assert!(error < -90.0, "{shelf:?} left a {error:.1} dB residual");
            }
        }
    }

    /// A low shelf lifts the bottom by the requested amount and leaves the top
    /// alone.
    #[test]
    fn a_low_shelf_lifts_the_bottom_only() {
        for gain_db in [-12.0f32, -6.0, 6.0, 12.0] {
            let mut filter = Biquad::default();
            filter.set_shelf(Shelf::Low, SR, 200.0, gain_db);

            let low = db(gain_at(&mut filter, 20.0));
            filter.reset();
            let high = db(gain_at(&mut filter, 10_000.0));

            assert!(
                (low - gain_db).abs() < 0.5,
                "{gain_db} dB shelf gave {low:.2} dB at 20 Hz"
            );
            assert!(
                high.abs() < 0.5,
                "{gain_db} dB shelf gave {high:.2} dB at 10 kHz"
            );
        }
    }

    #[test]
    fn a_high_shelf_lifts_the_top_only() {
        for gain_db in [-12.0f32, -6.0, 6.0, 12.0] {
            let mut filter = Biquad::default();
            filter.set_shelf(Shelf::High, SR, 4_000.0, gain_db);

            let high = db(gain_at(&mut filter, 18_000.0));
            filter.reset();
            let low = db(gain_at(&mut filter, 100.0));

            assert!(
                (high - gain_db).abs() < 0.5,
                "{gain_db} dB shelf gave {high:.2} dB at 18 kHz"
            );
            assert!(
                low.abs() < 0.5,
                "{gain_db} dB shelf gave {low:.2} dB at 100 Hz"
            );
        }
    }

    /// The analytic magnitude has to agree with what the filter actually does
    /// to a sine. This is the check that keeps the drawn curve honest: if the
    /// two ever disagree, the picture is lying about the sound.
    #[test]
    fn the_magnitude_matches_what_the_filter_does() {
        for shelf in [Shelf::Low, Shelf::High] {
            for gain_db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
                let mut filter = Biquad::default();
                filter.set_shelf(shelf, SR, 500.0, gain_db);

                for hz in [30.0f32, 100.0, 500.0, 2_000.0, 8_000.0] {
                    let analytic = filter.magnitude_db(hz, SR);
                    filter.reset();
                    let measured = db(gain_at(&mut filter, hz));
                    assert!(
                        (analytic - measured).abs() < 0.3,
                        "{shelf:?} {gain_db} dB at {hz} Hz: analytic {analytic:.2}, \
                         measured {measured:.2}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_shelf_stays_stable_at_the_extremes() {
        for shelf in [Shelf::Low, Shelf::High] {
            for hz in [1.0f32, 20.0, 20_000.0, 100_000.0] {
                for gain_db in [-12.0f32, 12.0] {
                    let mut filter = Biquad::default();
                    filter.set_shelf(shelf, SR, hz, gain_db);
                    for i in 0..10_000 {
                        let out = filter.process((i as f32 * 0.1).sin());
                        assert!(out.is_finite(), "{shelf:?} at {hz} Hz went non-finite");
                    }
                }
            }
        }
    }

    /// The one-pole passes DC and is 3 dB down at its cutoff.
    #[test]
    fn the_one_pole_is_a_lowpass() {
        let mut filter = OnePole::default();
        filter.set_cutoff(SR, 1_000.0);

        for _ in 0..10_000 {
            filter.process(1.0);
        }
        assert!((filter.process(1.0) - 1.0).abs() < 1e-3, "DC is not passed");

        filter.reset();
        let cutoff_gain = {
            let mut peak = 0.0f32;
            for i in 0..48_000 {
                let out = filter.process((i as f32 * TAU * 1_000.0 / SR).sin());
                if i > 4_800 {
                    peak = peak.max(out.abs());
                }
            }
            peak
        };
        assert!(
            (db(cutoff_gain) + 3.0).abs() < 0.6,
            "cutoff gain is {:.2} dB",
            db(cutoff_gain)
        );
    }

    #[test]
    fn the_one_pole_cutoff_is_clamped_to_something_sane() {
        let mut filter = OnePole::default();
        for hz in [-100.0f32, 0.0, SR, SR * 10.0] {
            filter.set_cutoff(SR, hz);
            filter.reset();
            for i in 0..1000 {
                assert!(filter.process((i as f32 * 0.3).sin()).is_finite());
            }
        }
    }
}
