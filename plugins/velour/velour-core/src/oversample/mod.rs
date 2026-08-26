//! Running the generator bus at 2x or 4x, so the curves do not fold their own
//! harmonics back into the audible band.
//!
//! **Move candidate**: Sparkleur generates harmonics too, so this module knows
//! nothing about Velour (`REQ-VEL-015`).
//!
//! ## Why it is needed at all
//!
//! AIR shapes everything above 5 kHz. An 8 kHz input's fourth harmonic is at
//! 32 kHz, and at 48 kHz that folds to 16 kHz — inside the audible band, and
//! **unreachable by any filter afterwards**, because by then it *is* 16 kHz.
//!
//! ## Polyphase IIR halfbands, minimum phase
//!
//! Two stages of
//!
//! ```text
//! H(z) = 1/2 * [ A0(z^2) + z^-1 * A1(z^2) ]
//! ```
//!
//! where each `A` is a cascade of allpass sections. Coefficients come from
//! `scripts/design-oversampler.py`; both stages measure about 78 dB in their
//! stopbands with a passband that is flat to eight decimal places, because
//! every section is allpass and the passband cannot be anything else.
//!
//! **Not linear-phase FIR.** A linear-phase filter has bulk delay, which would
//! have to be reported to the host and matched on the dry path — and the dry
//! path is the one thing this plugin promises not to touch (`REQ-VEL-001`).
//! The generator bus is a texture layer, not half of a reconstruction, so phase
//! rotation near 20 kHz costs nothing audible. **That trade only exists because
//! the topology is parallel**; a crossover would have to keep its bands
//! phase-coherent to sum back correctly.
//!
//! ## There are no buffers
//!
//! The specification said to allocate the 4x buffers up front. There is nothing
//! to allocate: [`Oversampler::process`] takes a closure and runs it once per
//! oversampled sample, so the intermediate samples live in registers. Switching
//! between 2x and 4x is then a branch rather than a resize, and the unused
//! stage keeps its state so switching back continues rather than clicks.

mod coefficients;

pub use coefficients::{STAGE_ONE, STAGE_TWO};

/// A first-order allpass, `(a + z^-1) / (1 + a * z^-1)`.
///
/// The design is written in `z^-2` at the input rate, which is `z^-1` at the
/// rate each branch actually runs at — that is the whole point of the polyphase
/// form, and why a section here holds one sample of state rather than two.
#[derive(Clone, Copy, Default)]
struct Allpass {
    a: f32,
    x1: f32,
    y1: f32,
}

impl Allpass {
    fn new(a: f32) -> Self {
        Self {
            a,
            x1: 0.0,
            y1: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.a * (input - self.y1) + self.x1;
        self.x1 = input;
        self.y1 = output;
        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

/// The two allpass branches of one halfband, interleaved: sorted ascending, the
/// even-indexed coefficients form one branch and the odd-indexed ones the other.
#[derive(Clone)]
struct Branches<const N: usize> {
    sections: [Allpass; N],
}

impl<const N: usize> Branches<N> {
    fn new(coefficients: [f32; N]) -> Self {
        Self {
            sections: std::array::from_fn(|index| Allpass::new(coefficients[index])),
        }
    }

    /// Runs `input` through the even-indexed sections.
    fn even(&mut self, input: f32) -> f32 {
        let mut value = input;
        let mut index = 0;
        while index < N {
            value = self.sections[index].process(value);
            index += 2;
        }
        value
    }

    /// Runs `input` through the odd-indexed sections.
    fn odd(&mut self, input: f32) -> f32 {
        let mut value = input;
        let mut index = 1;
        while index < N {
            value = self.sections[index].process(value);
            index += 2;
        }
        value
    }

    fn reset(&mut self) {
        for section in &mut self.sections {
            section.reset();
        }
    }
}

/// One sample in, two out.
///
/// **No gain correction.** Zero-stuffing then filtering would halve the
/// amplitude, but each polyphase branch is allpass and so has unity gain at
/// every frequency — each output phase is a full-amplitude copy.
#[derive(Clone)]
pub struct Upsampler2x<const N: usize> {
    branches: Branches<N>,
}

impl<const N: usize> Upsampler2x<N> {
    pub fn new(coefficients: [f32; N]) -> Self {
        Self {
            branches: Branches::new(coefficients),
        }
    }

    pub fn process(&mut self, input: f32) -> [f32; 2] {
        // **Which branch produces which output phase is not a free choice.**
        // Swapping these leaves a round trip working and a low sine intact, and
        // still puts the image at −24 dB instead of −88 — a nonlinearity fed
        // that is intermodulating with a copy of its own input.
        // `the_upsampler_leaves_no_image` is what holds this in place.
        let first = self.branches.even(input);
        let second = self.branches.odd(input);
        [first, second]
    }

    pub fn reset(&mut self) {
        self.branches.reset();
    }
}

/// Two samples in, one out.
///
/// The `0.5` is what makes it unity at DC: both branches pass DC untouched, so
/// their sum is twice the input.
#[derive(Clone)]
pub struct Downsampler2x<const N: usize> {
    branches: Branches<N>,
}

impl<const N: usize> Downsampler2x<N> {
    pub fn new(coefficients: [f32; N]) -> Self {
        Self {
            branches: Branches::new(coefficients),
        }
    }

    pub fn process(&mut self, input: [f32; 2]) -> f32 {
        // The later sample takes the branch without the extra delay, which is
        // what puts the two phases back in step. Swapping these two lines still
        // passes DC and still round-trips a low sine; what it destroys is the
        // stopband, so `aliasing_is_pushed_below_the_target` is the test that
        // holds this in place.
        let even = self.branches.even(input[1]);
        let odd = self.branches.odd(input[0]);
        0.5 * (even + odd)
    }

    pub fn reset(&mut self) {
        self.branches.reset();
    }
}

/// How far up to run.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Factor {
    Two,
    #[default]
    Four,
}

const STAGE_ONE_SECTIONS: usize = STAGE_ONE.len();
const STAGE_TWO_SECTIONS: usize = STAGE_TWO.len();

/// The oversampled bus. One per channel.
#[derive(Clone)]
pub struct Oversampler {
    up_one: Upsampler2x<STAGE_ONE_SECTIONS>,
    up_two: Upsampler2x<STAGE_TWO_SECTIONS>,
    down_two: Downsampler2x<STAGE_TWO_SECTIONS>,
    down_one: Downsampler2x<STAGE_ONE_SECTIONS>,
    factor: Factor,
}

impl Default for Oversampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Oversampler {
    pub fn new() -> Self {
        Self {
            up_one: Upsampler2x::new(STAGE_ONE),
            up_two: Upsampler2x::new(STAGE_TWO),
            down_two: Downsampler2x::new(STAGE_TWO),
            down_one: Downsampler2x::new(STAGE_ONE),
            factor: Factor::default(),
        }
    }

    /// **The unused stage keeps its state.** Clearing it would put a step in the
    /// signal on the way back, which is the one thing a quality switch must not
    /// do.
    pub fn set_factor(&mut self, factor: Factor) {
        self.factor = factor;
    }

    pub fn factor(&self) -> Factor {
        self.factor
    }

    /// Runs `body` once per oversampled sample and returns the one decimated
    /// sample. Allocation-free, and nothing is buffered.
    ///
    /// A non-finite input is dropped rather than fed in. These filters are
    /// recursive: one NaN would sit in their state for ever, and since the dry
    /// path never passes through here the failure would be a wet bus that went
    /// silent and stayed silent while the plugin looked like it was working.
    pub fn process(&mut self, input: f32, mut body: impl FnMut(f32) -> f32) -> f32 {
        if !input.is_finite() {
            return 0.0;
        }

        let [first, second] = self.up_one.process(input);

        match self.factor {
            Factor::Two => {
                let first = body(first);
                let second = body(second);
                self.down_one.process([first, second])
            }
            Factor::Four => {
                let [a, b] = self.up_two.process(first);
                let [c, d] = self.up_two.process(second);

                let a = body(a);
                let b = body(b);
                let c = body(c);
                let d = body(d);

                let first = self.down_two.process([a, b]);
                let second = self.down_two.process([c, d]);
                self.down_one.process([first, second])
            }
        }
    }

    pub fn reset(&mut self) {
        self.up_one.reset();
        self.up_two.reset();
        self.down_two.reset();
        self.down_one.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{amplitude, db_ratio, rms, sine};
    use crate::shaper::Shaper;

    /// A tenth of a second at 48 kHz, so a bin is 10 Hz and every frequency
    /// used below lands on one exactly — no leakage to argue with.
    const HOST_RATE: usize = 48_000;
    const LENGTH: usize = HOST_RATE / 10;

    /// Above this, a fold is allowed to land: it is inside the halfbands'
    /// transition bands, and nobody hears it.
    const AUDIBLE_LIMIT_HZ: usize = 20_000;

    fn identity(sample: f32) -> f32 {
        sample
    }

    #[test]
    fn dc_comes_out_at_unity() {
        let mut up = Upsampler2x::new(STAGE_ONE);
        let mut down = Downsampler2x::new(STAGE_ONE);

        // Long enough for the recursive sections to settle.
        let mut last = 0.0;
        for _ in 0..4096 {
            let pair = up.process(1.0);
            last = down.process(pair);
        }
        assert!((last - 1.0).abs() < 1e-4, "dc came out as {last}");
    }

    /// **The test that catches a swapped phase assignment.**
    ///
    /// Upsampling puts an image of every component at `48 kHz − f`, and the
    /// filter's job is to remove it. A round trip cannot see this: a swap on the
    /// way up cancels against the matching swap on the way down. What sees it is
    /// the oversampled signal itself, and what suffers from it is the
    /// nonlinearity in between, which intermodulates with the image.
    ///
    /// Measured: −88 dB. Swapped: −24 dB.
    #[test]
    fn the_upsampler_leaves_no_image() {
        let input = sine(1.0, 100, LENGTH);
        let mut up = Upsampler2x::new(STAGE_ONE);

        let mut doubled = Vec::with_capacity(LENGTH * 2);
        for sample in &input {
            let [first, second] = up.process(*sample);
            doubled.push(first);
            doubled.push(second);
        }

        // The settled half is 4800 samples at 96 kHz, so a bin is 20 Hz.
        let settled = &doubled[LENGTH..];
        let fundamental = amplitude(settled, 50);
        let image = amplitude(settled, 2_350);
        let rejection = db_ratio(image, fundamental);
        assert!(rejection < -70.0, "the image sat at {rejection:.1} dB");
    }

    /// The designed figure, measured rather than trusted: a sine inside the
    /// stopband, fed at the doubled rate, must come out buried.
    #[test]
    fn the_stopband_measures_what_it_was_designed_for() {
        // 30 kHz at 96 kHz is past stage one's stopband edge of 28 kHz. It folds
        // to 18 kHz on the way down, which is where to look for it.
        let doubled = sine(1.0, 3_000, LENGTH * 2);
        let mut down = Downsampler2x::new(STAGE_ONE);

        let output: Vec<f32> = doubled
            .chunks_exact(2)
            .map(|pair| down.process([pair[0], pair[1]]))
            .collect();

        // The first samples are the filters settling.
        let settled = &output[output.len() / 2..];
        let leaked = amplitude(settled, 1_800 / 2);
        let attenuation = db_ratio(leaked, 1.0);
        assert!(attenuation < -70.0, "only {attenuation:.1} dB");
    }

    #[test]
    fn the_passband_comes_through_untouched() {
        let doubled = sine(0.5, 100, LENGTH * 2);
        let mut down = Downsampler2x::new(STAGE_ONE);

        let output: Vec<f32> = doubled
            .chunks_exact(2)
            .map(|pair| down.process([pair[0], pair[1]]))
            .collect();

        let settled = &output[output.len() / 2..];
        let level = amplitude(settled, 50);
        assert!((level - 0.5).abs() < 1e-3, "1 kHz came out at {level}");
    }

    #[test]
    fn a_signal_survives_the_round_trip() {
        for factor in [Factor::Two, Factor::Four] {
            let input = sine(0.5, 100, LENGTH);
            let mut oversampler = Oversampler::new();
            oversampler.set_factor(factor);

            let output: Vec<f32> = input
                .iter()
                .map(|sample| oversampler.process(*sample, identity))
                .collect();

            let settled = &output[output.len() / 2..];
            let level = amplitude(settled, 50);
            assert!(
                (level - 0.5).abs() < 2e-3,
                "{factor:?} gave {level} instead of 0.5"
            );
        }
    }

    /// **What the two factors are worth** — the oversampler's own figure, not
    /// the plugin's.
    ///
    /// A raw curve on a bare 10 kHz tone, with none of the band-limiting the
    /// plugin puts in front of it. The number `REQ-VEL-005` is about is measured
    /// through a real generator instead (`crate::bands`); this is here to say
    /// what choosing between 2x and 4x actually buys.
    ///
    /// Measured at the hard knee and full drive: **4x −58 dB, 2x −44 dB.**
    ///
    /// The 2x figure is not a filter failure. At 96 kHz internally the curve's
    /// own harmonics already run past Nyquist as they are created, and nothing
    /// downstream can undo that. It is why 4x is the default, and why 2x is a
    /// cost saving rather than an equal.
    #[test]
    fn four_times_is_worth_about_fourteen_decibels() {
        let worst_four = worst_alias(Factor::Four);
        let worst_two = worst_alias(Factor::Two);

        assert!(worst_four < -50.0, "4x left aliasing at {worst_four:.1} dB");
        assert!(
            worst_two < -35.0 && worst_two > -55.0,
            "2x measured {worst_two:.1} dB, which is not the expected shape"
        );
        assert!(
            worst_four < worst_two - 8.0,
            "4x bought only {:.1} dB over 2x",
            worst_two - worst_four
        );
    }

    /// The loudest non-harmonic component, in dB below the fundamental.
    fn worst_alias(factor: Factor) -> f32 {
        const FUNDAMENTAL_HZ: usize = 10_000;
        let cycles = FUNDAMENTAL_HZ / 10;

        // The worst case the plugin can reach: the hard knee at full drive.
        let mut shaper = Shaper::new();
        shaper.set(crate::shaper::DRIVE_MAX, 0.0, 1.0);

        let mut oversampler = Oversampler::new();
        oversampler.set_factor(factor);

        // Twice the cycles over twice the length, so the frequency is the same
        // and the settled half still holds `cycles` whole periods.
        let input = sine(crate::shaper::PROBE_AMPLITUDE, cycles * 2, LENGTH * 2);
        let output: Vec<f32> = input
            .iter()
            .map(|sample| oversampler.process(*sample, |value| shaper.shape(value)))
            .collect();
        let settled = &output[LENGTH..];

        let reference = amplitude(settled, cycles);
        let mut worst = 0.0f32;

        // Where each harmonic of the input lands after folding into the output's
        // band. The second harmonic is at 20 kHz, still under Nyquist, so it is
        // a real component rather than an alias.
        //
        // **Only what lands under 20 kHz counts.** The halfbands' transition
        // bands deliberately let content fold into 20..24 kHz — that is what
        // makes them cheap enough to be free, and it is written down as a
        // decision rather than discovered here (`dsp.md`).
        for harmonic in 3..40usize {
            let hz = harmonic * FUNDAMENTAL_HZ;
            let folded = fold(hz, HOST_RATE);
            if folded == 0 || folded >= AUDIBLE_LIMIT_HZ {
                continue;
            }
            // High harmonics eventually fold straight back onto the fundamental
            // — the 23rd and 25th of 10 kHz both land on 10 kHz at this rate.
            // They cannot be measured apart from it, and they cannot be heard
            // as an artefact either: they add to the note.
            if folded == FUNDAMENTAL_HZ {
                continue;
            }
            worst = worst.max(amplitude(settled, folded / 10));
        }

        db_ratio(worst, reference)
    }

    /// Where `hz` ends up after sampling at `rate`.
    fn fold(hz: usize, rate: usize) -> usize {
        let wrapped = hz % rate;
        if wrapped > rate / 2 {
            rate - wrapped
        } else {
            wrapped
        }
    }

    /// A quality switch that puts a step in the signal is worse than no switch.
    #[test]
    fn switching_the_factor_does_not_click() {
        let input = sine(0.5, 100, LENGTH);
        let mut oversampler = Oversampler::new();
        oversampler.set_factor(Factor::Four);

        let mut previous = 0.0;
        let mut worst_jump = 0.0f32;

        for (index, sample) in input.iter().enumerate() {
            if index == LENGTH / 2 {
                oversampler.set_factor(Factor::Two);
            }
            let output = oversampler.process(*sample, identity);
            // Well past the settling at the start, so the only step that could
            // show up is the switch itself.
            if index > LENGTH / 4 {
                worst_jump = worst_jump.max((output - previous).abs());
            }
            previous = output;
        }

        // One sample of a 1 kHz sine at 0.5 amplitude moves about 0.065.
        assert!(worst_jump < 0.1, "a jump of {worst_jump} appeared");
    }

    /// A recursive filter that takes a NaN never recovers, and the failure would
    /// be silent — the dry path keeps playing.
    #[test]
    fn a_non_finite_sample_does_not_poison_the_filters() {
        let mut oversampler = Oversampler::new();

        for hostile in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            oversampler.reset();
            assert_eq!(oversampler.process(hostile, identity), 0.0);

            let input = sine(0.5, 100, LENGTH);
            let output: Vec<f32> = input
                .iter()
                .map(|sample| oversampler.process(*sample, identity))
                .collect();
            let settled = &output[output.len() / 2..];
            assert!(
                (amplitude(settled, 50) - 0.5).abs() < 2e-3,
                "{hostile} left the filters broken"
            );
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut oversampler = Oversampler::new();
        for _ in 0..256 {
            oversampler.process(1.0, identity);
        }
        oversampler.reset();
        // With every state at zero, the first sample out is whatever the
        // filters make of a single input, which is small — not the settled 1.0.
        assert!(oversampler.process(0.0, identity).abs() < 1e-6);
    }

    /// The coefficients are generated, so a mistake in the generator would be
    /// invisible here. This pins the two properties the script promises.
    #[test]
    fn the_coefficients_are_sorted_and_inside_the_unit_interval() {
        for stage in [STAGE_ONE.as_slice(), STAGE_TWO.as_slice()] {
            for value in stage {
                assert!(*value > 0.0 && *value < 1.0, "{value} is not a stable pole");
            }
            for pair in stage.windows(2) {
                assert!(pair[0] < pair[1], "{stage:?} is not sorted");
            }
            // A coefficient this close to 1 is a pole at |z| = 0.995. The script
            // spends a section to avoid it; this makes sure it stays spent.
            assert!(
                stage.iter().all(|value| *value < 0.95),
                "{stage:?} is marginal"
            );
        }
    }

    /// Not a claim about the filters — a reminder that `rms` and `sine` are the
    /// measurement, and a broken measurement would make every test above pass.
    #[test]
    fn the_measurement_still_works() {
        let signal = sine(1.0, 100, LENGTH);
        assert!((rms(&signal) - 1.0 / 2.0f32.sqrt()).abs() < 1e-3);
        assert!((amplitude(&signal, 100) - 1.0).abs() < 1e-3);
    }
}
