//! Second-order sections, Butterworth-aligned.
//!
//! Written for Velour and moved out for Sparkleur (`SPK-1`), so it knows
//! about neither (`REQ-VEL-015`, `REQ-SPK-015`).
//!
//! Direct form 1 with the RBJ cookbook coefficients, the same shape
//! `nxe_dsp::Spectrum` uses. Retuning writes new coefficients and **leaves the
//! state alone**, which is what lets `FOCUS` slide the band edges without a step
//! in the signal.

use std::f32::consts::TAU;

/// Butterworth: the flattest passband a single section has.
pub const BUTTERWORTH_Q: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// How close to Nyquist a corner may sit before the coefficients stop meaning
/// anything. Past this a low-pass becomes a pass-through and a high-pass
/// becomes silence, which is the limit each is heading toward anyway.
const NYQUIST_MARGIN: f32 = 0.49;

#[derive(Clone, Copy, Default)]
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

/// The five coefficients, without the state — what a retune replaces.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Coefficients {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Coefficients {
    /// Passes everything through unchanged.
    pub const PASS: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// Passes nothing.
    pub const SILENT: Self = Self {
        b0: 0.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    pub fn lowpass(hz: f32, q: f32, sample_rate: f32) -> Self {
        // A corner above Nyquist is a low-pass that low-passes nothing.
        if !(hz.is_finite() && hz > 0.0) || hz >= sample_rate * NYQUIST_MARGIN {
            return Self::PASS;
        }

        let (sin, cos, alpha, a0) = shared(hz, q, sample_rate);
        let b = (1.0 - cos) * 0.5;
        let _ = sin;
        Self {
            b0: b / a0,
            b1: (1.0 - cos) / a0,
            b2: b / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    pub fn highpass(hz: f32, q: f32, sample_rate: f32) -> Self {
        // A corner above Nyquist is a high-pass with nothing left above it.
        if hz >= sample_rate * NYQUIST_MARGIN {
            return Self::SILENT;
        }
        if !(hz.is_finite() && hz > 0.0) {
            return Self::PASS;
        }

        let (sin, cos, alpha, a0) = shared(hz, q, sample_rate);
        let b = (1.0 + cos) * 0.5;
        let _ = sin;
        Self {
            b0: b / a0,
            b1: -(1.0 + cos) / a0,
            b2: b / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    /// A peaking section: `gain_db` at `hz`, unity away from it.
    ///
    /// **Written for a band that has to come *down*** (`VDP-3`). Everything
    /// before it here added parallel bands — `x + (G - 1) · BandPass(x)` — and
    /// that shape is fine while `G > 1`, but with `G < 1` the subtraction runs
    /// into the band-pass's phase: where its response is near `±90°`,
    /// `|1 + (G - 1) · H|` is **above** one, so a "cut" band bumps at both
    /// skirts. Measured on Vocal Depth's presence band: a nominal 6 dB cut took
    /// **0.18 dB** of pink-weighted power away as a subtracted band-pass and
    /// **0.71 dB** as a peaking section — three quarters of the cut was going
    /// back in through the skirts. A minimum-phase section cuts what it says it
    /// cuts.
    ///
    /// RBJ cookbook. `q` is `f0 / bandwidth`, so a 1.32-octave band is about
    /// 1.05.
    pub fn peaking(hz: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        if !(hz.is_finite() && hz > 0.0) || hz >= sample_rate * NYQUIST_MARGIN {
            return Self::PASS;
        }
        if !gain_db.is_finite() || gain_db == 0.0 {
            return Self::PASS;
        }

        // Amplitude at the peak is `10^(dB/20)`; the cookbook's `A` is its
        // square root because it appears once in the numerator and once in the
        // denominator.
        let amplitude = 10.0f32.powf(gain_db.clamp(-48.0, 48.0) / 40.0);
        let (sin, cos, alpha, _) = shared(hz, q, sample_rate);
        let _ = sin;

        let a0 = 1.0 + alpha / amplitude;
        Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: -2.0 * cos / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / amplitude) / a0,
        }
    }
}

impl Coefficients {
    /// The section's complex response at `hz`, as `(real, imaginary)`.
    ///
    /// **Complex, not just a magnitude.** The caller that asked for this
    /// evaluates a *parallel* band — `1 + (G - 1) · H(f)` — and a band-pass has
    /// phase, so summing magnitudes there answers a different question
    /// (`vocal_depth_core::depth`, `VDP-3`).
    ///
    /// `H(e^{jω}) = (b0 + b1·e^{-jω} + b2·e^{-2jω}) / (1 + a1·e^{-jω} +
    /// a2·e^{-2jω})`.
    pub fn response(&self, hz: f32, sample_rate: f32) -> (f32, f32) {
        let w = TAU * hz / sample_rate;
        let (sin1, cos1) = w.sin_cos();
        let (sin2, cos2) = (2.0 * w).sin_cos();

        // e^{-jω} is (cos ω, -sin ω).
        let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
        let num_im = -(self.b1 * sin1 + self.b2 * sin2);
        let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
        let den_im = -(self.a1 * sin1 + self.a2 * sin2);

        let divisor = den_re * den_re + den_im * den_im;
        if divisor < 1e-20 {
            return (0.0, 0.0);
        }
        (
            (num_re * den_re + num_im * den_im) / divisor,
            (num_im * den_re - num_re * den_im) / divisor,
        )
    }

    /// `|H(f)|`.
    pub fn magnitude(&self, hz: f32, sample_rate: f32) -> f32 {
        let (re, im) = self.response(hz, sample_rate);
        re.hypot(im)
    }
}

fn shared(hz: f32, q: f32, sample_rate: f32) -> (f32, f32, f32, f32) {
    let w0 = TAU * hz / sample_rate;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * q.max(1e-3));
    (sin, cos, alpha, 1.0 + alpha)
}

impl Biquad {
    pub fn new(coefficients: Coefficients) -> Self {
        let mut biquad = Self::default();
        biquad.set(coefficients);
        biquad
    }

    /// Replaces the coefficients. **The state stays**, so a corner can be moved
    /// while signal is running through it.
    pub fn set(&mut self, coefficients: Coefficients) {
        self.b0 = coefficients.b0;
        self.b1 = coefficients.b1;
        self.b2 = coefficients.b2;
        self.a1 = coefficients.a1;
        self.a2 = coefficients.a2;
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

    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// A band-pass built from two second-order sections.
///
/// Shared because both the generators and the guards want the same thing: keep a
/// range, and be able to move its edges without a step
/// (`velour_core::bands`, `crate::guard`).
#[derive(Clone, Copy)]
pub struct BandPass {
    high: Biquad,
    low: Biquad,
}

impl BandPass {
    pub fn new(low_hz: f32, high_hz: f32, sample_rate: f32) -> Self {
        Self {
            high: Biquad::new(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate)),
            low: Biquad::new(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate)),
        }
    }

    /// Replaces both sets of coefficients and **keeps the state**, so an edge
    /// can be moved while signal is running through it.
    pub fn retune(&mut self, low_hz: f32, high_hz: f32, sample_rate: f32) {
        self.high
            .set(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate));
        self.low
            .set(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate));
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.low.process(self.high.process(input))
    }

    pub fn reset(&mut self) {
        self.high.reset();
        self.low.reset();
    }

    /// The complex response a band with these edges has at `hz`.
    ///
    /// An associated function rather than a method because the band does not
    /// keep its edges — and building the two sections the same way
    /// [`new`](Self::new) does is what stops the answer from drifting away from
    /// the filter it describes.
    pub fn response(low_hz: f32, high_hz: f32, hz: f32, sample_rate: f32) -> (f32, f32) {
        let high = Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate);
        let low = Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate);

        let (a_re, a_im) = high.response(hz, sample_rate);
        let (b_re, b_im) = low.response(hz, sample_rate);
        (a_re * b_re - a_im * b_im, a_re * b_im + a_im * b_re)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A peaking section hits its gain at its centre, comes back to unity away
    /// from it, and — the reason it exists — **a cut actually removes power**
    /// where a parallel band-pass did not (`VDP-3`).
    #[test]
    fn a_peaking_section_boosts_and_cuts_where_it_says() {
        const RATE: f32 = 48_000.0;
        const CENTRE: f32 = 3_162.0;

        for gain_db in [-12.0f32, -6.0, 6.0, 12.0] {
            let coefficients = Coefficients::peaking(CENTRE, 1.05, gain_db, RATE);

            let at_centre = 20.0 * coefficients.magnitude(CENTRE, RATE).log10();
            assert!(
                (at_centre - gain_db).abs() < 0.1,
                "{gain_db} dB asked for, {at_centre:.2} dB at the centre"
            );

            for far in [50.0f32, 20_000.0] {
                let away = 20.0 * coefficients.magnitude(far, RATE).log10();
                assert!(
                    away.abs() < 0.5,
                    "{gain_db} dB moved {far} Hz by {away:.2} dB"
                );
            }
        }

        // Unity gain is exactly a pass-through, so "no presence change" costs
        // nothing and cannot colour anything.
        assert_eq!(
            Coefficients::peaking(CENTRE, 1.05, 0.0, RATE),
            Coefficients::PASS
        );
    }

    /// The comparison that moved Vocal Depth off the parallel band: with a
    /// **negative** gain, a subtracted band-pass gives most of the cut back
    /// through its skirts, and a peaking section does not.
    #[test]
    fn a_subtracted_band_pass_does_not_cut_but_a_peaking_section_does() {
        const RATE: f32 = 48_000.0;
        let gain = 10.0f32.powf(-6.0 / 20.0);

        let mut parallel_power = 0.0;
        let mut peaking_power = 0.0;
        let mut total_weight = 0.0;
        let peaking = Coefficients::peaking(3_162.0, 1.05, -6.0, RATE);

        // Pink over a log grid is **uniform weights**: energy per octave is
        // what is constant, and a log step is a constant fraction of an octave.
        for step in 0..64 {
            let hz = 20.0 * (1_000.0f32.ln() * step as f32 / 63.0).exp();
            let weight = 1.0;
            total_weight += weight;

            let (re, im) = BandPass::response(2_000.0, 5_000.0, hz, RATE);
            let parallel = (1.0 + (gain - 1.0) * re).hypot((gain - 1.0) * im);
            parallel_power += weight * parallel * parallel;

            let magnitude = peaking.magnitude(hz, RATE);
            peaking_power += weight * magnitude * magnitude;
        }
        parallel_power /= total_weight;
        peaking_power /= total_weight;

        let parallel_db = 10.0 * parallel_power.log10();
        let peaking_db = 10.0 * peaking_power.log10();
        // Measured: **-0.18 dB against -0.71 dB** for the same nominal 6 dB
        // cut. The subtracted band-pass gives three quarters of the cut back
        // through its skirts.
        assert!(
            parallel_db > -0.3,
            "the subtracted band-pass took {parallel_db:.2} dB away, more than \
             this comparison assumes"
        );
        assert!(
            peaking_db < parallel_db - 0.3,
            "the peaking section ({peaking_db:.2} dB) did not cut meaningfully \
             more than the subtracted band-pass ({parallel_db:.2} dB)"
        );

        // And the mechanism, directly: where the band-pass's phase has come
        // round, a subtracted band is **louder** than its input. The worst of
        // it sits at **8.6 kHz, +0.57 dB** — well above the band being cut,
        // which is why listening for it inside 2-5 kHz would miss it.
        let worst = (100..20_000)
            .step_by(50)
            .map(|hz| {
                let (re, im) = BandPass::response(2_000.0, 5_000.0, hz as f32, RATE);
                (1.0 + (gain - 1.0) * re).hypot((gain - 1.0) * im)
            })
            .fold(0.0f32, f32::max);
        assert!(
            worst > 1.05,
            "the skirt bump this test is about did not appear: {worst:.4}"
        );
    }

    /// A second-order Butterworth is 3.01 dB down at its corner, and the
    /// response has to agree with what the filter actually does — this is the
    /// doubled arithmetic the rules warn about (`.agents/rules/rust.md`): the
    /// coefficients are used once by `process` and once by `response`.
    #[test]
    fn the_response_matches_the_filter() {
        const RATE: f32 = 48_000.0;

        for (name, coefficients, corner) in [
            (
                "lowpass",
                Coefficients::lowpass(1_000.0, BUTTERWORTH_Q, RATE),
                1_000.0,
            ),
            (
                "highpass",
                Coefficients::highpass(1_000.0, BUTTERWORTH_Q, RATE),
                1_000.0,
            ),
        ] {
            let at_corner = 20.0 * coefficients.magnitude(corner, RATE).log10();
            assert!(
                (at_corner + 3.01).abs() < 0.1,
                "{name} at its corner: {at_corner:.2} dB"
            );

            // Against the filter itself: drive it with a sine and compare
            // amplitudes once it has settled.
            for hz in [200.0f32, 1_000.0, 5_000.0] {
                let predicted = coefficients.magnitude(hz, RATE);
                let mut filter = Biquad::new(coefficients);
                let mut peak = 0.0f32;
                let length = (RATE / hz * 200.0) as usize;
                for index in 0..length {
                    let phase = index as f32 * TAU * hz / RATE;
                    let out = filter.process(phase.sin());
                    if index > length / 2 {
                        peak = peak.max(out.abs());
                    }
                }
                assert!(
                    (peak - predicted).abs() < 0.02,
                    "{name} at {hz} Hz: predicted {predicted:.4}, measured {peak:.4}"
                );
            }
        }
    }

    /// The band's response is the product of its two sections, and a parallel
    /// band built from it is flat when its gain is unity.
    #[test]
    fn the_band_response_is_flat_at_unity() {
        const RATE: f32 = 48_000.0;

        for hz in [100.0f32, 2_000.0, 3_500.0, 5_000.0, 12_000.0] {
            let (re, im) = BandPass::response(2_000.0, 5_000.0, hz, RATE);
            // 1 + (G - 1)·H with G = 1 is exactly 1, whatever H is.
            let parallel = (1.0 + 0.0 * re).hypot(0.0 * im);
            assert!((parallel - 1.0).abs() < 1e-6, "{hz} Hz: {parallel}");

            let magnitude = re.hypot(im);
            assert!(magnitude <= 1.05, "{hz} Hz: band gain {magnitude}");
        }

        // And it keeps its band: down in the middle, gone at both ends.
        let middle = {
            let (re, im) = BandPass::response(2_000.0, 5_000.0, 3_200.0, RATE);
            re.hypot(im)
        };
        let below = {
            let (re, im) = BandPass::response(2_000.0, 5_000.0, 200.0, RATE);
            re.hypot(im)
        };
        let above = {
            let (re, im) = BandPass::response(2_000.0, 5_000.0, 18_000.0, RATE);
            re.hypot(im)
        };
        assert!(middle > 0.8, "middle of the band: {middle}");
        assert!(below < 0.02, "below the band: {below}");
        assert!(above < 0.02, "above the band: {above}");
    }
    use crate::harmonics::{amplitude, db_ratio, sine};

    const RATE: f32 = 48_000.0;
    /// A tenth of a second, so a bin is 10 Hz.
    const LENGTH: usize = 4_800;

    fn run(coefficients: Coefficients, hz: usize) -> f32 {
        let mut biquad = Biquad::new(coefficients);
        // Twice the length, so the settled half holds `hz / 10` whole cycles and
        // the bin it is measured in lands exactly on the tone.
        let input = sine(1.0, hz / 5, LENGTH * 2);
        let output: Vec<f32> = input.iter().map(|s| biquad.process(*s)).collect();
        db_ratio(amplitude(&output[LENGTH..], hz / 10), 1.0)
    }

    #[test]
    fn a_lowpass_is_flat_below_and_falls_above() {
        let coefficients = Coefficients::lowpass(1_000.0, BUTTERWORTH_Q, RATE);
        assert!(run(coefficients, 100).abs() < 0.2);
        // Butterworth is 3 dB down at the corner and 12 dB per octave after.
        assert!((run(coefficients, 1_000) + 3.0).abs() < 0.5);
        assert!(run(coefficients, 4_000) < -20.0);
    }

    #[test]
    fn a_highpass_is_flat_above_and_falls_below() {
        let coefficients = Coefficients::highpass(1_000.0, BUTTERWORTH_Q, RATE);
        assert!(run(coefficients, 10_000).abs() < 0.2);
        assert!((run(coefficients, 1_000) + 3.0).abs() < 0.5);
        assert!(run(coefficients, 250) < -20.0);
    }

    /// `FOCUS` can push a corner past Nyquist, and coefficients computed up there
    /// are not "steeper" — they are meaningless, and an unstable section is a
    /// burst of noise rather than a filter.
    #[test]
    fn a_corner_above_nyquist_becomes_the_limit_it_is_heading_for() {
        assert_eq!(
            Coefficients::lowpass(40_000.0, BUTTERWORTH_Q, RATE),
            Coefficients::PASS
        );
        assert_eq!(
            Coefficients::highpass(40_000.0, BUTTERWORTH_Q, RATE),
            Coefficients::SILENT
        );
    }

    #[test]
    fn hostile_corners_do_not_produce_nonsense() {
        for hz in [f32::NAN, f32::INFINITY, -1.0, 0.0, 1e9] {
            for build in [
                Coefficients::lowpass as fn(f32, f32, f32) -> Coefficients,
                Coefficients::highpass,
            ] {
                let mut biquad = Biquad::new(build(hz, BUTTERWORTH_Q, RATE));
                for _ in 0..64 {
                    assert!(biquad.process(0.5).is_finite(), "{hz} broke it");
                }
            }
        }
    }

    /// What makes `FOCUS` movable: writing new coefficients must not clear the
    /// state, or every turn of the knob would put a step in the signal.
    #[test]
    fn retuning_keeps_the_state() {
        let mut biquad = Biquad::new(Coefficients::lowpass(1_000.0, BUTTERWORTH_Q, RATE));
        for _ in 0..256 {
            biquad.process(1.0);
        }
        let before = biquad.process(1.0);
        biquad.set(Coefficients::lowpass(1_100.0, BUTTERWORTH_Q, RATE));
        let after = biquad.process(1.0);
        assert!((after - before).abs() < 0.05, "{before} jumped to {after}");
    }
}
