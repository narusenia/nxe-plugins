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

use nxe_audio::biquad::BandPass;
use nxe_audio::oversample::Factor;
use nxe_audio::shaper::Shaper;

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
/// documented compromise (`nxe_audio::oversample`); making it a different band as
/// well would make the switch audible.
pub const AIR_INPUT_CEILING: f32 = 0.25;

/// How far the macros are allowed to reach (`REQ-VEL-021`).
///
/// **What it changes is what the generator adds, not how hard it is driven.**
/// `VEL-18` measured the added layer at the top of every control and found it
/// **more than nine parts pass-through to one part harmonics** — the
/// normalisation in [`nxe_audio::shaper`] holds the curve's output at its
/// input's level, so most of what a band's fader adds is a band-limited copy of
/// the input. Turning that up is a level change, and a level change is what the
/// listener takes back out with `OUTPUT`.
///
/// So `Hard` adds [`Shaper::residual`] instead of [`Shaper::shape`]: the same
/// curve with the pass-through taken out and the rest normalised back up. At
/// the same fader position the harmonics arrive **9 dB louder** and the layer's
/// level does not move.
///
/// **`Soft` is the arithmetic that shipped**, not an approximation of it — it
/// calls the same `shape` on the same shaper.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Soft,
    Hard,
}

/// The top of `Hard`'s drive map.
///
/// **Six, and the reason is folding — the same reason it was six before**
/// (`VEL-2`, `REQ-VEL-020`). `Hard` lifts everything the curve made by about
/// 9 dB, and the folds are part of what it made. Measured through the real AIR
/// generator at the hard knee, worst case an 11 kHz tone, against the input:
///
/// ```text
/// drive   11 kHz fold   harmonics
///   6.0      −65.6 dB    −13.1 dB
///   7.0      −60.6 dB    −11.9 dB
///   8.0      −52.9 dB    −11.2 dB
/// ```
///
/// **Drive 8 breaks `REQ-VEL-005`'s −60 dB in `Hard`** and drive 7 sits on the
/// line with no margin. Six clears it by 5.5 dB and costs **1.95 dB** of
/// harmonics, because the residual is normalised — the drive changes which
/// harmonics far more than how many (`nxe_audio::shaper`).
///
/// This is the order `REQ-VEL-020` sets out, at its middle step: the input is
/// already tightened as far as it goes ([`AIR_INPUT_CEILING`] sits exactly on
/// AIR's own upper edge, `VEL-18`), so the next lever is the drive. The factor
/// is the last one and is not reached for here.
pub const HARD_DRIVE_MAX: f32 = 6.0;

impl Mode {
    /// The top of the drive map for this mode.
    pub const fn drive_max(self) -> f32 {
        match self {
            Mode::Soft => nxe_audio::shaper::DRIVE_MAX,
            Mode::Hard => HARD_DRIVE_MAX,
        }
    }
}

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

    /// The span of the input this generator listens to, at a given `FOCUS`.
    ///
    /// **Public because the interface draws it** (`REQ-VEL-013`), and it comes
    /// from the same place the filters are tuned from — a picture built from a
    /// second copy of these numbers would drift from the sound the first time
    /// one of them moved.
    pub fn input_range(band: Band, focus: f32, host_rate: f32) -> (f32, f32) {
        let focus = if focus.is_finite() {
            focus.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let shift = (focus * FOCUS_OCTAVES).exp2();
        let edges = band.edges();
        let high = match band {
            // Capped, and the cap does not move with `FOCUS`: it belongs to the
            // sample rate, not to the voice's range.
            Band::Air => (edges.input_high * shift).min(host_rate * AIR_INPUT_CEILING),
            _ => edges.input_high * shift,
        };
        (edges.input_low * shift, high)
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

        let (input_low, input_high) = Generator::input_range(self.band, focus, self.host_rate);

        self.input.retune(input_low, input_high, rate);
        self.output
            .retune(edges.output_low * shift, edges.output_high * shift, rate);
    }

    /// One sample, at the oversampled rate. The level this band is added at is
    /// the caller's, because that is where it is smoothed.
    pub fn process(&mut self, input: f32, shaper: &Shaper, mode: Mode) -> f32 {
        let band = self.input.process(input);
        let shaped = match mode {
            // The same call that shipped, so `Soft` is bit-identical rather
            // than nearly (`REQ-VEL-021`).
            Mode::Soft => shaper.shape(band),
            Mode::Hard => shaper.residual(band),
        };
        self.output.process(shaped)
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

    /// The range the interface draws has to be the range the filters got
    /// (`REQ-VEL-013`). Checked through the generator's own state rather than by
    /// repeating the formula.
    #[test]
    fn the_drawn_range_is_the_tuned_range() {
        for band in BANDS {
            for focus in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
                let mut generator = Generator::new(band, 48_000.0, Factor::Four);
                generator.set_focus(focus);
                assert_eq!(generator.focus(), focus);

                let (low, high) = Generator::input_range(band, focus, 48_000.0);
                assert!(low < high, "{band:?} at {focus}: {low}..{high}");
                // AIR's ceiling is the only cap, and it is the host rate's
                // quarter — never above it whatever `FOCUS` does.
                assert!(high <= 48_000.0 * AIR_INPUT_CEILING || band != Band::Air);
            }
        }

        // A hostile focus draws the resting range rather than nothing.
        assert_eq!(
            Generator::input_range(Band::Body, f32::NAN, 48_000.0),
            Generator::input_range(Band::Body, 0.0, 48_000.0)
        );
    }

    use nxe_audio::oversample::{Factor, Oversampler};
    use nxe_audio::shaper::{DRIVE_MAX, PROBE_AMPLITUDE, Shaper};

    const HOST_RATE: f32 = 48_000.0;
    /// A tenth of a second at the host rate, so a bin is 10 Hz.
    const LENGTH: usize = 4_800;

    /// Runs a tone through one generator inside a real oversampled bus, and
    /// returns the settled output at the host rate.
    fn run(band: Band, hz: usize, drive: f32, hardness: f32, bias: f32) -> Vec<f32> {
        run_in(band, hz, drive, hardness, bias, Mode::Soft)
    }

    fn run_in(band: Band, hz: usize, drive: f32, hardness: f32, bias: f32, mode: Mode) -> Vec<f32> {
        let mut shaper = Shaper::new();
        shaper.set(drive, bias, hardness);

        let mut generator = Generator::new(band, HOST_RATE, Factor::Four);
        let mut oversampler = Oversampler::new();
        oversampler.set_factor(Factor::Four);

        let input = sine(PROBE_AMPLITUDE, hz / 5, LENGTH * 2);
        let output: Vec<f32> = input
            .iter()
            .map(|sample| {
                oversampler.process(*sample, |value| generator.process(value, &shaper, mode))
            })
            .collect();

        output[LENGTH..].to_vec()
    }

    /// The level of the tone itself, in dB relative to what went in.
    fn passband(band: Band, hz: usize) -> f32 {
        // Drive at the bottom, so the curve is linear and this measures the
        // filters alone.
        let output = run(band, hz, nxe_audio::shaper::DRIVE_MIN, 0.0, 0.0);
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
            .map(|sample| {
                oversampler.process(*sample, |value| {
                    generator.process(value, &shaper, Mode::Soft)
                })
            })
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
            let output = run(band, hz, 3.0, 0.0, nxe_audio::shaper::BIAS_MAX);
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
    /// (`nxe_audio::oversample`).
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

    /// The same sweep, referenced to the tone that went **in**.
    ///
    /// `Hard` takes the fundamental out, so "in dB below the tone that came
    /// out" divides by something that is no longer there. The input is what the
    /// listener compares a fold to anyway: the layer is added to an untouched
    /// dry path at unity (`REQ-VEL-001`).
    /// **At the top of that mode's own map**, which is the loudest a user can
    /// ask for. `Hard` cannot reach `DRIVE_MAX`; measuring it there would be
    /// measuring a setting the plugin does not have.
    fn alias_floor_against_input(band: Band, hz: usize, mode: Mode) -> f32 {
        alias_floor_at(band, hz, mode, mode.drive_max())
    }

    fn alias_floor_at(band: Band, hz: usize, mode: Mode, drive: f32) -> f32 {
        let output = run_in(band, hz, drive, 1.0, 0.0, mode);

        let mut worst = 0.0f32;
        for harmonic in 2..120usize {
            let true_hz = harmonic * hz;
            if true_hz < 24_000 {
                continue;
            }
            let folded = fold(true_hz);
            if folded == 0 || folded >= 20_000 || folded.is_multiple_of(hz) {
                continue;
            }
            worst = worst.max(amplitude(&output, folded / 10));
        }

        db_ratio(worst, PROBE_AMPLITUDE)
    }

    /// **The aliasing figure in both modes** (`REQ-VEL-005`, `VEL-19`).
    ///
    /// `Hard` lifts everything the curve made by about 9 dB and the folds are
    /// part of that, so the figure has to be re-measured rather than inherited.
    /// It is measured against the tone that went **in**: `Hard` takes the
    /// fundamental out, so "below the tone that came out" divides by something
    /// that is no longer there, and the input is what a fold is heard against
    /// anyway — the layer is added to an untouched dry path at unity.
    ///
    /// Read the numbers with:
    ///
    /// ```text
    /// cargo test -p velour-core both_modes -- --nocapture
    /// ```
    #[test]
    fn both_modes_stay_under_the_aliasing_target() {
        println!("\n  band          hz    soft(in)   hard(in)");
        for (band, hz) in [
            (Band::Body, 500usize),
            (Band::Presence, 5_000),
            (Band::Air, 11_000),
        ] {
            let soft = alias_floor_against_input(band, hz, Mode::Soft);
            let hard = alias_floor_against_input(band, hz, Mode::Hard);
            println!("  {band:?}  {hz:8} {soft:9.2} {hard:10.2}");
            assert!(soft < -60.0, "{band:?} at {hz} Hz: soft {soft:.1} dB");
            assert!(hard < -60.0, "{band:?} at {hz} Hz: hard {hard:.1} dB");
        }

        // **The band the ceiling exists for, swept.** `Hard` at drive 8 read
        // −52.9 dB here, which is what set `HARD_DRIVE_MAX`.
        println!("\n  AIR across the tone, hard(in)");
        for hz in [7_000usize, 9_000, 10_000, 11_000] {
            let level = alias_floor_against_input(Band::Air, hz, Mode::Hard);
            println!("  {hz:6} {level:9.2}");
            assert!(level < -60.0, "AIR at {hz} Hz folded at {level:.1} dB");
        }
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
                        generator.process(0.5, &shaper, Mode::Soft).is_finite(),
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
            generator.process(0.5, &shaper, Mode::Soft);
        }
        generator.reset();
        assert_eq!(generator.process(0.0, &shaper, Mode::Soft), 0.0);
    }
}
