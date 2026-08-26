//! The three parallel generators: band-limit, shape, band-limit.
//!
//! Each one takes the whole input, keeps the part it is responsible for, runs it
//! through the curve, and keeps the part of the *result* that is worth adding
//! (`plugins/velour/docs/specifications/dsp.md`). What comes out is added to an
//! untouched dry path, never blended with it (`REQ-VEL-001`).
//!
//! **The input and output bands are not the same band**, and that is the point.
//! BODY looks at 90–520 Hz but keeps its harmonics up to 2 kHz — cutting the
//! output at 520 Hz would throw away everything the curve just made.
//!
//! **The output high-pass is not optional.** An asymmetric curve fed a
//! symmetric signal returns a non-zero mean, and subtracting the curve's value
//! at rest only removes the standing part of it (`Shaper`'s tests say so
//! directly). Without it the dry path gets a slow DC wander added to it.

use crate::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
use crate::oversample::Factor;
use crate::shaper::Shaper;

/// How far `FOCUS` moves the band edges, in octaves either way.
///
/// One octave covers what it has to: a male fundamental near 110 Hz against a
/// female one near 220 (`REQ-VEL-002`).
pub const FOCUS_OCTAVES: f32 = 1.0;

/// The ceiling on what reaches AIR's curve, as a fraction of the sample rate.
///
/// **This is an anti-aliasing measure, not a band edge.** Harmonics of content
/// above it fold as they are *created* — the ninth harmonic of 20 kHz is 180 kHz,
/// which does not fit at 192 kHz internally and lands on 12 kHz before any
/// filter can see it. No oversampling factor in reach fixes that, and nothing up
/// there needs exciting anyway: AIR's job is to make high harmonics out of
/// high-mid content.
///
/// A fraction of the **host's** rate rather than a frequency, because what
/// decides the aliasing is the ratio of the input frequency to the internal
/// rate — and at a fixed factor those move together. At 96 kHz the ceiling
/// doubles and the behaviour is identical.
///
/// **Not a fraction of the internal rate**, which would tighten it at 2x and
/// therefore move AIR's band when the factor changed. 2x is already the
/// documented compromise (`crate::oversample`); making it a different band as
/// well would make the switch audible.
pub const AIR_INPUT_CEILING: f32 = 0.25;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Band {
    Body,
    Presence,
    Air,
}

pub const BANDS: [Band; 3] = [Band::Body, Band::Presence, Band::Air];

/// One band's four corners in Hz, before `FOCUS`.
struct Edges {
    input_low: f32,
    input_high: f32,
    output_low: f32,
    output_high: f32,
}

impl Band {
    /// Where each generator listens and what it keeps
    /// (`dsp.md`, "帯域生成器").
    fn edges(self) -> Edges {
        match self {
            Band::Body => Edges {
                input_low: 90.0,
                input_high: 520.0,
                // 30 Hz is the DC blocker; 2 kHz keeps the first few harmonics
                // of a 520 Hz fundamental and drops the rest.
                output_low: 30.0,
                output_high: 2_000.0,
            },
            Band::Presence => Edges {
                input_low: 480.0,
                input_high: 5_200.0,
                output_low: 300.0,
                output_high: 12_000.0,
            },
            Band::Air => Edges {
                input_low: 4_800.0,
                // Capped by `AIR_INPUT_CEILING`, which is the whole reason this
                // band has an upper input edge at all.
                input_high: 12_000.0,
                output_low: 4_000.0,
                // The top of hearing. AIR has no upper edge of its own; this
                // keeps the structure uniform and stops the very top running
                // into the decimator unfiltered.
                output_high: 20_000.0,
            },
        }
    }

    /// Where this band sits on the curve family, as multipliers on the drive and
    /// the bias (`dsp.md`). BODY is gentle and even-heavy, AIR is fine and hard.
    pub fn curve_multipliers(self) -> (f32, f32) {
        match self {
            //          bias, drive
            Band::Body => (1.3, 0.6),
            Band::Presence => (1.0, 1.0),
            Band::Air => (0.5, 1.6),
        }
    }
}

/// A band-pass built from two second-order sections.
#[derive(Clone, Copy)]
struct BandPass {
    high: Biquad,
    low: Biquad,
}

impl BandPass {
    fn new(low_hz: f32, high_hz: f32, sample_rate: f32) -> Self {
        Self {
            high: Biquad::new(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate)),
            low: Biquad::new(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate)),
        }
    }

    fn retune(&mut self, low_hz: f32, high_hz: f32, sample_rate: f32) {
        self.high
            .set(Coefficients::highpass(low_hz, BUTTERWORTH_Q, sample_rate));
        self.low
            .set(Coefficients::lowpass(high_hz, BUTTERWORTH_Q, sample_rate));
    }

    fn process(&mut self, input: f32) -> f32 {
        self.low.process(self.high.process(input))
    }

    fn reset(&mut self) {
        self.high.reset();
        self.low.reset();
    }
}

/// One generator's filters. **Per channel** — the curve is shared, because a
/// [`Shaper`] holds no per-sample state.
///
/// It is built from the **host's** rate and the oversampling factor rather than
/// from the rate it runs at, because it needs both: the coefficients belong to
/// the internal rate, and AIR's input ceiling belongs to the host's.
#[derive(Clone)]
pub struct Generator {
    band: Band,
    host_rate: f32,
    factor: Factor,
    focus: f32,
    input: BandPass,
    output: BandPass,
}

impl Generator {
    pub fn new(band: Band, host_rate: f32, factor: Factor) -> Self {
        let mut generator = Self {
            band,
            host_rate,
            factor,
            focus: f32::NAN,
            input: BandPass::new(0.0, 0.0, host_rate),
            output: BandPass::new(0.0, 0.0, host_rate),
        };
        generator.retune(0.0);
        generator
    }

    /// The rate the filters run at.
    pub fn internal_rate(&self) -> f32 {
        self.host_rate
            * match self.factor {
                Factor::Two => 2.0,
                Factor::Four => 4.0,
            }
    }

    /// Slides every edge together. **Block rate**, and it returns without
    /// recomputing when nothing moved — holding the knob still has to be free,
    /// and building four sets of coefficients per sample would not be.
    pub fn set_focus(&mut self, focus: f32) {
        let focus = if focus.is_finite() {
            focus.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        if focus == self.focus {
            return;
        }
        self.retune(focus);
    }

    /// Changing the factor changes the rate the sections run at, so the same
    /// corner frequencies need different coefficients. **The corners do not
    /// move**, so the response is the same and the switch stays inaudible.
    pub fn set_factor(&mut self, factor: Factor) {
        if factor == self.factor {
            return;
        }
        self.factor = factor;
        let focus = self.focus;
        self.focus = f32::NAN;
        self.retune(focus);
    }

    fn retune(&mut self, focus: f32) {
        self.focus = focus;

        let rate = self.internal_rate();
        let shift = (focus * FOCUS_OCTAVES).exp2();
        let edges = self.band.edges();

        let input_high = match self.band {
            // Capped, and the cap does not move with `FOCUS`: it belongs to the
            // sample rate, not to the voice's range.
            Band::Air => (edges.input_high * shift).min(self.host_rate * AIR_INPUT_CEILING),
            _ => edges.input_high * shift,
        };

        self.input.retune(edges.input_low * shift, input_high, rate);
        self.output
            .retune(edges.output_low * shift, edges.output_high * shift, rate);
    }

    /// One sample, at the oversampled rate. The level this band is added at is
    /// the caller's, because that is where it is smoothed.
    pub fn process(&mut self, input: f32, shaper: &Shaper) -> f32 {
        let band = self.input.process(input);
        self.output.process(shaper.shape(band))
    }

    pub fn band(&self) -> Band {
        self.band
    }

    pub fn focus(&self) -> f32 {
        self.focus
    }

    pub fn factor(&self) -> Factor {
        self.factor
    }

    pub fn reset(&mut self) {
        self.input.reset();
        self.output.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{amplitude, db_ratio, mean, sine};
    use crate::oversample::{Factor, Oversampler};
    use crate::shaper::{DRIVE_MAX, PROBE_AMPLITUDE, Shaper};

    const HOST_RATE: f32 = 48_000.0;
    /// A tenth of a second at the host rate, so a bin is 10 Hz.
    const LENGTH: usize = 4_800;

    /// Runs a tone through one generator inside a real oversampled bus, and
    /// returns the settled output at the host rate.
    fn run(band: Band, hz: usize, drive: f32, hardness: f32, bias: f32) -> Vec<f32> {
        let mut shaper = Shaper::new();
        shaper.set(drive, bias, hardness);

        let mut generator = Generator::new(band, HOST_RATE, Factor::Four);
        let mut oversampler = Oversampler::new();
        oversampler.set_factor(Factor::Four);

        let input = sine(PROBE_AMPLITUDE, hz / 5, LENGTH * 2);
        let output: Vec<f32> = input
            .iter()
            .map(|sample| oversampler.process(*sample, |value| generator.process(value, &shaper)))
            .collect();

        output[LENGTH..].to_vec()
    }

    /// The level of the tone itself, in dB relative to what went in.
    fn passband(band: Band, hz: usize) -> f32 {
        // Drive at the bottom, so the curve is linear and this measures the
        // filters alone.
        let output = run(band, hz, crate::shaper::DRIVE_MIN, 0.0, 0.0);
        db_ratio(amplitude(&output, hz / 10), PROBE_AMPLITUDE)
    }

    #[test]
    fn each_band_hears_its_own_range_and_not_the_others() {
        // A frequency in the middle of each band, and how much the other two
        // let through there.
        // **The bands overlap on purpose** — parallel generators have no reason
        // to tile the axis (`dsp.md`), and second-order edges are gentle. What
        // has to hold is that each band is clearly the loudest in its own
        // middle, not that the others are silent there.
        for (band, hz) in [
            (Band::Body, 200usize),
            (Band::Presence, 1_500),
            (Band::Air, 10_000),
        ] {
            let own = passband(band, hz);
            assert!(
                own > -3.0,
                "{band:?} only passed {own:.1} dB of its own {hz} Hz"
            );

            for other in BANDS {
                if other == band {
                    continue;
                }
                let leak = passband(other, hz);
                assert!(
                    leak < own - 10.0,
                    "{other:?} passed {leak:.1} dB at {hz} Hz against {band:?}'s {own:.1} dB"
                );
            }
        }
    }

    #[test]
    fn focus_slides_the_edges_by_an_octave() {
        // 90 Hz is BODY's lower corner, so it is 3 dB down at rest. An octave
        // down and it is well inside the band; an octave up and it is well out.
        let mut generator = Generator::new(Band::Body, HOST_RATE, Factor::Four);
        assert_eq!(generator.focus(), 0.0);

        generator.set_focus(-1.0);
        assert_eq!(generator.focus(), -1.0);
        generator.set_focus(1.0);
        assert_eq!(generator.focus(), 1.0);

        // 90 Hz is BODY's resting corner. An octave down it is inside the band;
        // an octave up it is a full octave below a 12 dB/oct edge.
        let low = level_at(Band::Body, 90, -1.0);
        let high = level_at(Band::Body, 90, 1.0);
        assert!(low > high + 10.0, "{low:.1} dB against {high:.1} dB");
    }

    fn level_at(band: Band, hz: usize, focus: f32) -> f32 {
        let shaper = Shaper::new();
        let mut generator = Generator::new(band, HOST_RATE, Factor::Four);
        generator.set_focus(focus);
        let mut oversampler = Oversampler::new();

        let input = sine(PROBE_AMPLITUDE, hz / 5, LENGTH * 2);
        let output: Vec<f32> = input
            .iter()
            .map(|sample| oversampler.process(*sample, |value| generator.process(value, &shaper)))
            .collect();

        db_ratio(amplitude(&output[LENGTH..], hz / 10), PROBE_AMPLITUDE)
    }

    /// What the output high-pass is for. `Shaper`'s own test shows a biased
    /// curve leaving DC behind; this is where it gets removed.
    #[test]
    fn a_biased_curve_leaves_no_dc_at_the_output() {
        for (band, hz) in [
            (Band::Body, 220usize),
            (Band::Presence, 1_500),
            (Band::Air, 8_000),
        ] {
            let output = run(band, hz, 3.0, 0.0, crate::shaper::BIAS_MAX);
            let offset = mean(&output);
            assert!(offset.abs() < 1e-4, "{band:?} left {offset} of DC");
        }
    }

    /// **The aliasing figure `REQ-VEL-005` is about, and what sets
    /// `DRIVE_MAX`** (`REQ-VEL-020`).
    ///
    /// Measured through the real generators at the hard knee and full drive,
    /// with AIR's input ceiling in place. The worst case is an 11 kHz tone into
    /// AIR — just under the ceiling, where the folding is steepest and still
    /// measurable: **−63 dB**.
    ///
    /// Without the ceiling a 20 kHz tone sits at −44 dB *at any drive*, because
    /// its ninth harmonic folds as it is created and nothing downstream can
    /// reach it. That is what the ceiling is for, and it is what let the drive
    /// ceiling go back up from 6 to 8.
    #[test]
    fn the_worst_case_band_stays_under_the_aliasing_target() {
        for (band, hz) in [
            (Band::Body, 500usize),
            (Band::Presence, 5_000),
            (Band::Air, 11_000),
        ] {
            let level = alias_floor(band, hz);
            assert!(
                level < -60.0,
                "{band:?} at {hz} Hz aliased at {level:.1} dB"
            );
        }
    }

    /// The loudest measurable fold below 20 kHz, in dB below the tone.
    ///
    /// Folds landing on a multiple of the tone are skipped: they sit on top of a
    /// real harmonic and cannot be told apart from it. Folds above 20 kHz are
    /// skipped because the halfbands' transition bands deliberately allow them
    /// (`crate::oversample`).
    fn alias_floor(band: Band, hz: usize) -> f32 {
        let output = run(band, hz, DRIVE_MAX, 1.0, 0.0);
        let reference = amplitude(&output, hz / 10);

        let mut worst = 0.0f32;
        for harmonic in 2..120usize {
            let true_hz = harmonic * hz;
            // Below Nyquist it is a real harmonic, not a fold.
            if true_hz < 24_000 {
                continue;
            }
            let folded = fold(true_hz);
            if folded == 0 || folded >= 20_000 || folded.is_multiple_of(hz) {
                continue;
            }
            worst = worst.max(amplitude(&output, folded / 10));
        }

        db_ratio(worst, reference)
    }

    fn fold(hz: usize) -> usize {
        let wrapped = hz % 48_000;
        if wrapped > 24_000 {
            48_000 - wrapped
        } else {
            wrapped
        }
    }

    /// The ceiling has to hold whatever `FOCUS` does, or the aliasing figure
    /// above is only true at one knob position.
    ///
    /// The reading moves by a little either way because `FOCUS` does move AIR's
    /// *output* high-pass, which is a band edge and is supposed to. What must
    /// not happen is 18 kHz arriving at the curve.
    #[test]
    fn focus_cannot_lift_airs_input_ceiling() {
        for focus in [-1.0f32, 0.0, 1.0] {
            let level = level_at(Band::Air, 18_000, focus);
            assert!(
                level < -6.0,
                "focus {focus}: 18 kHz got through at {level:.1} dB"
            );
        }

        let rest = level_at(Band::Air, 18_000, 0.0);
        let raised = level_at(Band::Air, 18_000, 1.0);
        assert!(
            raised < rest + 3.0,
            "focus lifted 18 kHz from {rest:.1} to {raised:.1} dB"
        );
    }

    #[test]
    fn hostile_focus_values_do_not_produce_nonsense() {
        let shaper = Shaper::new();
        for focus in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9] {
            for band in BANDS {
                let mut generator = Generator::new(band, HOST_RATE, Factor::Four);
                generator.set_focus(focus);
                for _ in 0..256 {
                    assert!(
                        generator.process(0.5, &shaper).is_finite(),
                        "{band:?} broke on focus {focus}"
                    );
                }
            }
        }
    }

    #[test]
    fn reset_clears_it() {
        let shaper = Shaper::new();
        let mut generator = Generator::new(Band::Presence, HOST_RATE, Factor::Four);
        for _ in 0..1024 {
            generator.process(0.5, &shaper);
        }
        generator.reset();
        assert_eq!(generator.process(0.0, &shaper), 0.0);
    }
}
