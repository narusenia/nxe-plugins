//! Early reflections: a tapped delay line behind three allpass stages.
//!
//! **This module does not know what Vocal Depth is** (`REQ-VDP-003`). It takes
//! a [`Settings`] with two normalised numbers and a sample rate; it has never
//! heard of `DEPTH`, `ROOM` or a parameter struct. Three products want early
//! reflections — Vocal Depth, Vocal Glue's `Ambience Glue`, Impact's `SIZE` —
//! and the second one to ask moves this file into `nxe-audio` unchanged
//! (`REQ-VDP-015`).
//!
//! The design is specified in
//! `plugins/vocal-depth/docs/specifications/dsp.md`. Three points from it are
//! worth repeating next to the code:
//!
//! **Tap times never move.** [`Settings::distance`] moves a Gaussian window
//! over a fixed tap set instead of moving read positions. Moving them would
//! need interpolation, and interpolating does not fix the real problem: a read
//! position in motion is a Doppler shift, so sweeping distance would detune
//! the reflections. The window is continuous, so there is nothing to click.
//!
//! **The taps are read at whole samples.** That is the payoff of the point
//! above — 13 taps per channel per sample, without Catmull-Rom
//! (`DelayLine::read_whole`).
//!
//! **The allpasses come first.** Diffusing the input means one set of three
//! stages serves all 13 taps rather than one set per tap, and an allpass's
//! direct term is `-g·x`, which has no delay of its own — so putting them in
//! front cannot produce energy earlier than the first tap.
//!
//! What is *not* here: `DAMPING` (`VDP-5`) and the stereo width
//! (`VDP-6`). Both act on the reflection bus as a whole, so the engine owns
//! them; this module ends at the 200 Hz highpass.

use nxe_audio::DelayLine;
use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};

/// Taps per channel.
pub const TAPS: usize = 10;

/// Tap times, in milliseconds, for the left channel.
///
/// **Prime milliseconds, taken alternately with [`TAP_MS_RIGHT`].** Prime so
/// that no tap is a small integer multiple of another — when they are, the
/// comb notches of several taps land on the same frequencies and the
/// reflections take on a pitch. Alternate so that **the two channels share no
/// tap time at all**: that is the only mechanism decorrelating them, and it
/// costs no phase rotation (`REQ-VDP-007`).
const TAP_MS_LEFT: [f32; TAPS] = [11.0, 17.0, 23.0, 31.0, 41.0, 47.0, 59.0, 67.0, 73.0, 83.0];

/// Tap times, in milliseconds, for the right channel. See [`TAP_MS_LEFT`].
const TAP_MS_RIGHT: [f32; TAPS] = [13.0, 19.0, 29.0, 37.0, 43.0, 53.0, 61.0, 71.0, 79.0, 89.0];

/// The latest tap. **The tap set stops well before [`SPAN_MAX_MS`] on
/// purpose**: the diffusion tail rides on every tap, including the last one,
/// so a tap at 113 ms would put a third of the energy outside the range
/// `REQ-VDP-003` names — measured, `VDP-1`. Ending at 89 ms leaves the tail
/// room to finish inside it.
const TAP_MS_LAST: f32 = 89.0;

/// The tap tables have to end where [`TAP_MS_LAST`] says, and start inside the
/// range the requirement names. **Checked at compile time rather than in a
/// test** (`.agents/rules/rust.md`): a table edited by hand is exactly the kind
/// of thing that should fail the build, not a test run nobody had to execute.
const _: () = assert!(TAP_MS_RIGHT[TAPS - 1] == TAP_MS_LAST);
const _: () = assert!(TAP_MS_LEFT[TAPS - 1] <= TAP_MS_LAST);
const _: () = assert!(TAP_MS_LEFT[0] >= SPAN_MIN_MS && TAP_MS_RIGHT[0] >= SPAN_MIN_MS);

/// The tap the amplitude law is referenced to: the earliest one.
const T_REF_MS: f32 = 11.0;

/// The window the tap positions are normalised into — the range
/// `REQ-VDP-003` puts the reflections in.
const SPAN_MIN_MS: f32 = 10.0;
const SPAN_MAX_MS: f32 = 120.0;

/// Width of the Gaussian window [`Settings::distance`] slides over the taps.
///
/// Wide enough that six or more taps are always inside it. Narrower and the
/// middle of the range stands one tap up on its own, which is a slapback
/// rather than a room.
const WINDOW_SIGMA: f32 = 0.45;

/// Input diffusion, left channel: three Schroeder allpasses in series.
///
/// **Short, and `g` below a half.** The first design (11.9 ms, `g` = 0.55) put
/// the tail 53 dB down only 130 ms after the last tap, which measured as
/// **-4.6 dB of the energy beyond 120 ms** (`VDP-1`). What sets the extent is
/// the longest stage times how many passes it takes to decay.
const ALLPASS_MS_LEFT: [f32; 3] = [3.1, 5.3, 7.9];

/// Input diffusion, right channel. Mutually prime with the left set for the
/// same reason the taps are.
const ALLPASS_MS_RIGHT: [f32; 3] = [3.7, 6.1, 9.1];

/// Allpass coefficient. Every pass through a stage costs 6.9 dB.
const ALLPASS_G: f32 = 0.45;

/// Longest allpass delay a line has to serve, with room to round up.
const ALLPASS_LINE_SECONDS: f32 = 0.012;

/// The reflection bus is highpassed here.
///
/// Three reasons, and only the first is about how it sounds: a doubled
/// fundamental reads as a box rather than as distance; the loudness
/// normalisation adds the direct and reflected power as if they were
/// uncorrelated, which is only true above roughly 90 Hz for an 11 ms tap, so
/// the band where the assumption fails is left with no energy in it; and it
/// keeps the reflections out of the mid channel the direct sound owns
/// (`REQ-VDP-007`).
pub const HIGHPASS_HZ: f32 = 200.0;

/// Reflection level at `distance` 0 and 1, relative to the direct sound.
///
/// Asymmetric on purpose: getting closer runs out of room, getting further does
/// not (`REQ-VDP-002`).
///
/// **The far end is above unity, and it has to be** (`VDP-14`). What says "far"
/// is the direct-to-reflected ratio, and in a real space that ratio *crosses
/// zero*: the reverberant field catches up with the direct sound and then
/// passes it. The first version ended at `-4 dB` nominal, which after the tap
/// weights left the ratio at **+17 dB** across the whole of `DEPTH` — a
/// listener heard "more effect", not "further away".
const LEVEL_NEAR_DB: f32 = -20.0;
const LEVEL_FAR_DB: f32 = 2.0;

/// [`Settings::amount`] is a linear gain of `amount^AMOUNT_EXPONENT · SCALE`.
///
/// A dB map would put `-inf` at zero and need a branch to honour "exactly
/// silent at zero" (`REQ-VDP-003`); a power keeps that exact and continuous.
/// The exponent is what puts unity near the middle of the control.
const AMOUNT_EXPONENT: f32 = 1.5;
const AMOUNT_SCALE: f32 = 2.0;

/// Below this the output is treated as silence and the recursions are stopped.
///
/// A first-order tail never reaches zero, so "silent within three seconds"
/// (`REQ-VDP-003`) does not hold without a floor. nih-plug enables FTZ around
/// `process`, so inside a host this is free (`SPK-17`) — but the tests run
/// outside one.
const SILENCE_FLOOR: f32 = 1e-9;

/// How long a line has to be to serve the latest tap.
const LINE_SECONDS: f32 = SPAN_MAX_MS / 1000.0;

/// How fast a tap weight moves toward the one the settings ask for.
///
/// **The weights are smoothed, not the settings.** Changing a weight is
/// changing a gain, and doing it in one sample is a step — measured at eleven
/// times the background roughness before this existed (`VDP-1`). Smoothing
/// the settings instead would mean ten `exp` calls per sample; smoothing the
/// weights is one multiply-add each, and it is skipped entirely once they have
/// arrived.
///
/// **The bus gain is folded into the weights for the same reason.** It was a
/// separate multiply at first, and the seam did not go away: a step is a step
/// whether it is in one gain or in ten. What made that hard to see is that the
/// control experiment happened to jump the gain to `0.22` while the case under
/// test jumped it to `1.78` — **both a step of 0.78 around unity**, so the two
/// measurements agreed to four digits and neither looked like the outlier
/// (`VDP-1`).
const WEIGHT_SECONDS: f32 = 0.005;

/// A weight this close to its target is treated as arrived, which is what lets
/// a settled block cost nothing.
///
/// **Not smaller, for an arithmetic reason.** The step a one-pole takes is
/// `remaining · coefficient`, and once that falls below one ulp of the weight
/// the addition stops changing anything: convergence stalls at about
/// `ulp(w) / coefficient`, which for a weight near 0.3 and a 5 ms coefficient
/// is `7e-6`. A threshold of `1e-6` therefore never arrives, and the smoothing
/// runs on every sample for ever (`VDP-1`). This is 70 dB below the weights it
/// is applied to, so the snap it allows is inaudible.
const WEIGHT_SETTLED: f32 = 1e-4;

/// What the caller asks for. Both are `0..=1`, and both are clamped: they come
/// from a host (`REQ-VDP-016`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Near (0) to far (1). Moves the window over the tap set **and** the
    /// level of the bus. Vocal Depth drives this from `DEPTH`.
    pub distance: f32,
    /// How much reflection there is at all, independent of `distance`. Exactly
    /// zero at zero. Vocal Depth drives this from `ROOM`.
    pub amount: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distance: 0.5,
            amount: 0.5,
        }
    }
}

/// One Schroeder allpass: `y[n] = -g·x[n] + x[n-D] + g·y[n-D]`.
struct Allpass {
    line: DelayLine,
    delay: usize,
}

impl Allpass {
    fn new(milliseconds: f32, sample_rate: f32) -> Self {
        Self {
            line: DelayLine::new(sample_rate, ALLPASS_LINE_SECONDS),
            delay: samples_for(milliseconds, sample_rate),
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let delayed = self.line.read_whole(self.delay);
        let output = -ALLPASS_G * input + delayed;
        let write = input + ALLPASS_G * output;

        // Stopping the recursion, not clearing the buffer: the line drains on
        // its own once zeros are being written into it, and clearing would be
        // work proportional to its length on a sample that is already silent.
        let write = if write.abs() < SILENCE_FLOOR {
            0.0
        } else {
            write
        };
        self.line.write(write);

        output
    }

    fn reset(&mut self) {
        self.line.reset();
    }
}

/// One channel's reflections: diffusion, a line, its taps, and the highpass.
struct Channel {
    line: DelayLine,
    allpass: [Allpass; 3],
    highpass: Biquad,
    /// Whole-sample tap delays.
    taps: [usize; TAPS],
    /// Tap times in milliseconds, kept for the window and the amplitude law.
    tap_ms: [f32; TAPS],
    /// `window · amplitude · polarity · gain` per tap, where the weights are
    /// heading.
    targets: [f32; TAPS],
    /// Where they are now. Smoothed toward `targets` at audio rate.
    weights: [f32; TAPS],
    /// Whether any weight is still moving, so a settled block costs nothing.
    settling: bool,
    /// One-pole coefficient for the smoothing.
    weight_coefficient: f32,
}

impl Channel {
    fn new(tap_ms: [f32; TAPS], allpass_ms: [f32; 3], sample_rate: f32) -> Self {
        Self {
            line: DelayLine::new(sample_rate, LINE_SECONDS),
            allpass: [
                Allpass::new(allpass_ms[0], sample_rate),
                Allpass::new(allpass_ms[1], sample_rate),
                Allpass::new(allpass_ms[2], sample_rate),
            ],
            highpass: Biquad::new(Coefficients::highpass(
                HIGHPASS_HZ,
                BUTTERWORTH_Q,
                sample_rate,
            )),
            taps: std::array::from_fn(|i| samples_for(tap_ms[i], sample_rate)),
            tap_ms,
            targets: [0.0; TAPS],
            weights: [0.0; TAPS],
            settling: false,
            weight_coefficient: nxe_audio::envelope::coefficient(WEIGHT_SECONDS, sample_rate),
        }
    }

    /// Rebuilds the tap weight targets for a new distance and bus gain.
    fn resolve(&mut self, distance: f32, gain: f32) {
        for i in 0..TAPS {
            let position = (self.tap_ms[i] - SPAN_MIN_MS) / (SPAN_MAX_MS - SPAN_MIN_MS);
            let offset = position - distance;
            let window = (-(offset * offset) / (2.0 * WINDOW_SIGMA * WINDOW_SIGMA)).exp();

            // Energy falling as 1/t: a later tap came off a further surface.
            let amplitude = (T_REF_MS / self.tap_ms[i]).sqrt();

            // Alternating polarity. All one sign and the taps pile up at the
            // bottom of the spectrum, which reads as proximity, not distance.
            let polarity = if i % 2 == 0 { 1.0 } else { -1.0 };

            self.targets[i] = window * amplitude * polarity * gain;
        }
        self.settling = true;
    }

    /// Puts the weights where they are heading, without a ramp. For
    /// construction and [`reset`](Self::reset) — a fresh instance should not
    /// fade its first five milliseconds in.
    fn snap(&mut self) {
        self.weights = self.targets;
        self.settling = false;
    }

    /// One step of the weight smoothing. Returns immediately once every weight
    /// has arrived.
    fn settle(&mut self) {
        if !self.settling {
            return;
        }
        let mut moving = false;
        for i in 0..TAPS {
            let remaining = self.targets[i] - self.weights[i];
            if remaining.abs() < WEIGHT_SETTLED {
                self.weights[i] = self.targets[i];
            } else {
                self.weights[i] += remaining * self.weight_coefficient;
                moving = true;
            }
        }
        self.settling = moving;
    }

    /// `Σ wᵢ²` — the incoherent tap energy the loudness normalisation needs
    /// (`VDP-3`). Exposed rather than recomputed there so the tap table has
    /// one owner.
    ///
    /// Reported off the **targets**, not the smoothed weights: the
    /// normalisation is resolved at block rate from the parameters, and the
    /// five milliseconds the weights take to arrive are not something it
    /// should see.
    fn tap_energy(&self) -> f32 {
        self.targets.iter().map(|w| w * w).sum()
    }

    fn process(&mut self, input: f32) -> f32 {
        // One sanitising step at the entry, not a guard on every filter
        // (`SPK-9`). A single non-finite sample would otherwise latch the line
        // and the allpasses for good.
        let input = if input.is_finite() { input } else { 0.0 };

        self.settle();

        let mut diffused = input;
        for stage in &mut self.allpass {
            diffused = stage.process(diffused);
        }
        self.line.write(diffused);

        let mut summed = 0.0;
        for i in 0..TAPS {
            summed += self.weights[i] * self.line.read_whole(self.taps[i]);
        }

        let mut output = self.highpass.process(summed);
        if output.abs() < SILENCE_FLOOR {
            output = 0.0;
            // Only once the taps have gone quiet too, so a real signal on its
            // way through a zero crossing is not treated as the end of one.
            if summed == 0.0 {
                self.highpass.reset();
            }
        }
        output
    }

    fn reset(&mut self) {
        self.line.reset();
        for stage in &mut self.allpass {
            stage.reset();
        }
        self.highpass.reset();
        self.snap();
    }
}

/// Early reflections for a stereo pair.
///
/// Allocation happens in [`Reflections::new`] and nowhere else, so
/// [`process`](Self::process) can run on the audio thread
/// (`.agents/rules/rust.md`). A sample rate change means a new instance.
pub struct Reflections {
    channels: [Channel; 2],
    settings: Settings,
    gain: f32,
}

impl Reflections {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            channels: [
                Channel::new(TAP_MS_LEFT, ALLPASS_MS_LEFT, sample_rate),
                Channel::new(TAP_MS_RIGHT, ALLPASS_MS_RIGHT, sample_rate),
            ],
            // Deliberately not the default: `set` returns early when nothing
            // moved, so the stored settings must differ from the first call's.
            settings: Settings {
                distance: f32::NAN,
                amount: f32::NAN,
            },
            gain: 0.0,
        };
        built.set(Settings::default());
        for channel in &mut built.channels {
            channel.snap();
        }
        built
    }

    /// Resolves `settings` into tap weights and a bus gain. **Block rate.**
    ///
    /// Returns without recomputing when nothing moved, so holding a knob
    /// costs nothing (`Shaper::set` does the same).
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            distance: clamped(settings.distance),
            amount: clamped(settings.amount),
        };
        if settings == self.settings {
            return;
        }

        let level_db = LEVEL_NEAR_DB + (LEVEL_FAR_DB - LEVEL_NEAR_DB) * settings.distance;
        let amount = settings.amount.powf(AMOUNT_EXPONENT) * AMOUNT_SCALE;
        self.gain = amount * 10.0f32.powf(level_db / 20.0);

        for channel in &mut self.channels {
            channel.resolve(settings.distance, self.gain);
        }

        self.settings = settings;
    }

    /// One stereo sample in, one stereo pair of reflections out. **Audio
    /// rate.** The direct sound is not in here: the caller adds it.
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        (
            self.channels[0].process(left),
            self.channels[1].process(right),
        )
    }

    /// The resolved gain of the bus, for the loudness normalisation
    /// (`VDP-3`). Exactly zero when `amount` is zero.
    pub fn gain(&self) -> f32 {
        self.gain
    }

    /// Where each tap arrives and how loud it is, for the figure
    /// (`REQ-VDP-013`).
    ///
    /// **The left channel's set**, normalised: the position is a share of
    /// `SPAN_MAX_MS` and the level is the weight against the loudest one the
    /// design can produce. Drawing both channels would put two stems on almost
    /// every arrival and say nothing the readout does not.
    ///
    /// **From the weights, not from the parameters.** A figure computed from
    /// `DEPTH` would agree with the sound only as long as nobody changed the
    /// window; this one cannot disagree.
    pub fn pattern(&self) -> [(f32, f32); TAPS] {
        let channel = &self.channels[0];
        std::array::from_fn(|index| {
            let position = channel.tap_ms[index] / SPAN_MAX_MS;
            (position, channel.weights[index].abs())
        })
    }

    /// `Σ wᵢ²`, averaged over the two channels — the incoherent tap energy the
    /// loudness normalisation needs (`VDP-3`). **The bus gain is already in
    /// it**, so this is the whole `(gain · r)² · Σ wᵢ² aᵢ²` term of the
    /// formula in `dsp.md`, not just its tail.
    pub fn tap_energy(&self) -> f32 {
        (self.channels[0].tap_energy() + self.channels[1].tap_energy()) / 2.0
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }
}

/// Whole samples for a time in milliseconds.
///
/// **Times are milliseconds and lengths are samples** (`REQ-VDP-017`): holding
/// a tap in samples would halve the room at 96 kHz.
fn samples_for(milliseconds: f32, sample_rate: f32) -> usize {
    (milliseconds * 1e-3 * sample_rate).round().max(1.0) as usize
}

/// Clamps into `0..=1`, sending a non-finite value to zero. A host is free to
/// send anything (`REQ-VDP-016`).
fn clamped(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const RATES: [f32; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];

    fn far() -> Settings {
        Settings {
            distance: 1.0,
            amount: 1.0,
        }
    }

    /// Runs `input` through both channels and returns the left output.
    fn render(reflections: &mut Reflections, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&s| reflections.process(s, s).0).collect()
    }

    fn impulse(length: usize) -> Vec<f32> {
        let mut signal = vec![0.0; length];
        signal[0] = 1.0;
        signal
    }

    fn energy(signal: &[f32]) -> f32 {
        signal.iter().map(|s| s * s).sum()
    }

    /// `amount` = 0 has to be **exactly** silent, not nearly
    /// (`REQ-VDP-003`). A `-120 dB` bus is still a bus.
    #[test]
    fn zero_amount_is_exactly_silent() {
        let mut reflections = Reflections::new(RATE);
        reflections.set(Settings {
            distance: 1.0,
            amount: 0.0,
        });

        assert_eq!(reflections.gain(), 0.0);
        assert_eq!(reflections.tap_energy(), 0.0);

        // The weights ramp there rather than jumping, so give them the ramp
        // (`WEIGHT_SECONDS`) before asserting. The requirement is about the
        // resting state — what it forbids is a bus that is merely very quiet.
        let noise = nxe_audio::harmonics::noise(1.0, 8_192);
        let settle = (RATE * WEIGHT_SECONDS * 20.0) as usize;
        for &sample in &noise[..settle] {
            reflections.process(sample, sample);
        }
        for &sample in &noise[settle..] {
            let (left, right) = reflections.process(sample, sample);
            assert_eq!(left, 0.0, "reflections at amount 0");
            assert_eq!(right, 0.0, "reflections at amount 0");
        }
    }

    /// Nothing arrives before the first tap, and the bulk of the response
    /// lives inside the range `REQ-VDP-003` names. **The share beyond 120 ms
    /// is a measured number, recorded in `dsp.md`** — the allpass tail rides
    /// on every tap, including the late ones, so it is not zero.
    #[test]
    fn the_response_starts_at_the_first_tap_and_does_not_become_a_tail() {
        let mut reflections = Reflections::new(RATE);
        reflections.set(far());

        // Four seconds, so a tail would have room to show itself.
        let rendered = render(&mut reflections, &impulse(4 * RATE as usize));

        let first_tap = samples_for(TAP_MS_LEFT[0], RATE);
        // The highpass is minimum-phase, so allow it the couple of samples it
        // needs to respond rather than asserting on the exact sample.
        let before = &rendered[..first_tap - 2];
        assert_eq!(
            energy(before),
            0.0,
            "energy arrived before the first tap at {} ms",
            TAP_MS_LEFT[0]
        );

        let total = energy(&rendered);
        let at = |ms: f32| samples_for(ms, RATE);
        let share = |ms: f32| 10.0 * (energy(&rendered[at(ms)..]) / total).log10();

        let beyond_120 = share(120.0);
        let beyond_250 = share(250.0);

        // Measured -25.5 dB and -140.7 dB (`VDP-1`, recorded in `dsp.md`).
        // The thresholds sit where a change of design would show, not at the
        // measurement.
        assert!(
            beyond_120 < -20.0,
            "beyond 120 ms: {beyond_120:.1} dB (250 ms {beyond_250:.1})"
        );
        assert!(beyond_250 < -100.0, "beyond 250 ms: {beyond_250:.1} dB");

        // And it ends outright rather than tapering: exactly zero, which the
        // silence floor is what makes possible.
        assert_eq!(
            energy(&rendered[at(500.0)..]),
            0.0,
            "still ringing half a second in"
        );
    }

    /// Sweeping `distance` moves weights, not read positions, so there is
    /// nothing to click. Measured as a second difference, because an absolute
    /// jump cannot tell a step apart from the signal (`AIR-7`).
    ///
    /// **The control comes first**: a version of the same measurement that
    /// jumps the gain by a quarter has to fail, or the assert below is
    /// measuring nothing (`VEL-10`).
    #[test]
    fn sweeping_distance_does_not_click() {
        let tone = nxe_audio::harmonics::tone(0.5, 220.0, RATE, 24_000);

        let roughness = |step_at: usize, snap: bool| {
            let mut reflections = Reflections::new(RATE);
            reflections.set(Settings {
                distance: 0.0,
                amount: 1.0,
            });

            let mut rendered = Vec::with_capacity(tone.len());
            for (index, &sample) in tone.iter().enumerate() {
                if index == step_at {
                    // A quarter of the range in one block, which is more than
                    // the wrapper's smoother would ever hand over at once.
                    reflections.set(Settings {
                        distance: 0.25,
                        amount: 1.0,
                    });
                    if snap {
                        // The control: the same move with the smoothing taken
                        // out, which is what the module did before `VDP-1`
                        // measured it.
                        for channel in &mut reflections.channels {
                            channel.snap();
                        }
                    }
                }
                rendered.push(reflections.process(sample, sample).0);
            }

            // Largest second difference in the 64 samples around the step,
            // against the largest anywhere else.
            let second = |i: usize| rendered[i + 2] - 2.0 * rendered[i + 1] + rendered[i];
            let near = (step_at - 32..step_at + 32)
                .map(|i| second(i).abs())
                .fold(0.0f32, f32::max);
            let far = (1_000..step_at - 64)
                .chain(step_at + 64..rendered.len() - 3)
                .map(|i| second(i).abs())
                .fold(0.0f32, f32::max);
            near / far
        };

        let control = roughness(12_000, true);
        assert!(
            control > 2.0,
            "the control did not produce a seam: {control:.2}"
        );

        // Measured 0.36 against a control of 88 (`VDP-1`) — the smoothed move
        // is quieter than the tone's own curvature, and the unsmoothed one is
        // 244 times rougher.
        let measured = roughness(12_000, false);
        assert!(
            measured < 1.0,
            "sweeping distance left a seam: {measured:.2} (control {control:.2})"
        );
    }

    /// Silence in, **exactly** zero out, within three seconds
    /// (`REQ-VDP-003`). A first-order tail settles on a denormal instead of on
    /// zero unless something cuts it (`SPK-16`).
    #[test]
    fn silence_converges_to_exact_zero() {
        let mut reflections = Reflections::new(RATE);
        reflections.set(far());

        let noise = nxe_audio::harmonics::noise(1.0, RATE as usize);
        for &sample in &noise {
            reflections.process(sample, sample);
        }

        let tail = render(&mut reflections, &vec![0.0; 3 * RATE as usize]);
        let last = tail.len() - 1;
        assert_eq!(tail[last], 0.0, "still ringing after three seconds");

        // And it got there well before the deadline, so the margin is visible
        // rather than assumed.
        let settled = tail
            .iter()
            .position(|&s| s == 0.0)
            .expect("never reached exactly zero");
        // Measured 0.150 s against a budget of three (`VDP-1`).
        assert!(
            settled < RATE as usize / 2,
            "took {:.3} s to reach zero",
            settled as f32 / RATE
        );
    }

    /// The taps are the same milliseconds at every rate, which is what stops
    /// 96 kHz from halving the room (`REQ-VDP-017`).
    #[test]
    fn tap_times_are_the_same_milliseconds_at_every_rate() {
        for rate in RATES {
            for (index, nominal) in TAP_MS_LEFT.iter().enumerate() {
                let actual = samples_for(*nominal, rate) as f32 / rate * 1000.0;
                assert!(
                    (actual - nominal).abs() < 0.05,
                    "rate {rate}, tap {index}: {actual} ms for {nominal} ms"
                );
            }
        }
    }

    /// Rate independence measured on the signal, not on the table above.
    /// **Noise, not a steady tone** — the reflection delays are fixed in
    /// samples, so a tone's phase relationships rotate with the rate and the
    /// measurement drifts for a reason that is not a bug (`SPK-9`).
    #[test]
    fn the_response_has_the_same_shape_at_every_rate() {
        let energies: Vec<f32> = RATES
            .iter()
            .map(|&rate| {
                let mut reflections = Reflections::new(rate);
                reflections.set(far());
                let noise = nxe_audio::harmonics::noise(1.0, rate as usize);
                let rendered = render(&mut reflections, &noise);
                // Energy per sample, so the different lengths compare.
                energy(&rendered) / rendered.len() as f32
            })
            .collect();

        let reference = energies[1];
        for (rate, measured) in RATES.iter().zip(&energies) {
            let difference = 10.0 * (measured / reference).log10();
            // Measured within 0.08 dB over the four rates (`VDP-1`).
            assert!(
                difference.abs() < 0.5,
                "rate {rate}: {difference:.2} dB against 48 kHz"
            );
        }
    }

    /// A window centred on the far end has to weight the late taps more than
    /// the early ones, and the near end the other way round. This is the
    /// mirrored value the rules warn about: the sign of `position - distance`
    /// is invisible in anything but a test like this.
    #[test]
    fn distance_moves_the_window_over_the_taps() {
        let mut reflections = Reflections::new(RATE);

        reflections.set(Settings {
            distance: 0.0,
            amount: 1.0,
        });
        let near: Vec<f32> = reflections.channels[0]
            .targets
            .iter()
            .map(|w| w.abs())
            .collect();

        reflections.set(far());
        let distant: Vec<f32> = reflections.channels[0]
            .targets
            .iter()
            .map(|w| w.abs())
            .collect();

        assert!(
            near[0] > near[TAPS - 1],
            "near: first tap {} is not above the last {}",
            near[0],
            near[TAPS - 1]
        );
        assert!(
            distant[TAPS - 1] > distant[0],
            "far: last tap {} is not above the first {}",
            distant[TAPS - 1],
            distant[0]
        );
    }

    /// The gain is exactly zero at `amount` = 0 and monotone above it, and the
    /// exposed tap energy is what the normalisation in `VDP-3` will square.
    #[test]
    fn the_exposed_gain_and_energy_describe_the_bus() {
        let mut reflections = Reflections::new(RATE);

        let mut previous = -1.0;
        for step in 0..=10 {
            reflections.set(Settings {
                distance: 0.5,
                amount: step as f32 / 10.0,
            });
            let gain = reflections.gain();
            assert!(gain > previous, "gain not monotone at amount {step}0 %");
            previous = gain;
        }

        reflections.set(far());
        let expected: f32 = reflections.channels[0]
            .targets
            .iter()
            .map(|w| w * w)
            .sum::<f32>();
        let energy = reflections.tap_energy();
        assert!(
            energy > 0.0 && (energy / expected - 1.0).abs() < 0.5,
            "tap energy {energy} against the left channel's {expected}"
        );
    }

    /// The weights arrive, and stop costing anything once they have. Without
    /// the second half a settled instance would keep doing the smoothing
    /// arithmetic on every sample for ever.
    #[test]
    fn the_weights_arrive_and_then_stop_moving() {
        let mut reflections = Reflections::new(RATE);
        reflections.set(far());
        assert!(reflections.channels[0].settling, "set did not start a ramp");

        // A one-pole is inside `WEIGHT_SETTLED` of its target after about
        // fourteen time constants; twenty leaves margin.
        for _ in 0..(RATE * WEIGHT_SECONDS * 20.0) as usize {
            reflections.process(0.0, 0.0);
        }

        assert!(!reflections.channels[0].settling, "still ramping");
        assert_eq!(
            reflections.channels[0].weights,
            reflections.channels[0].targets
        );
    }

    /// The two channels share no tap time, so the reflections decorrelate
    /// without a single phase rotation (`REQ-VDP-007`). `VDP-6` owns the
    /// promise; this is the cheap check that the mechanism exists at all.
    #[test]
    fn the_two_channels_decorrelate() {
        let mut reflections = Reflections::new(RATE);
        reflections.set(far());

        let mut correlation = nxe_dsp::Correlation::new(RATE);
        let noise = nxe_audio::harmonics::noise(1.0, RATE as usize);
        // The same signal into both channels: any decorrelation is the tap
        // sets, not the input.
        for &sample in &noise {
            let (left, right) = reflections.process(sample, sample);
            correlation.push(left, right);
        }

        // Measured -0.006 (`VDP-1`): the two tap sets share no time, so this
        // is as close to independent as two rooms get.
        let value = correlation.value();
        assert!(value.abs() < 0.1, "channels stayed correlated: {value:.3}");
    }

    /// Hostile values: nothing non-finite comes out, and nothing latches
    /// (`REQ-VDP-016`). A single NaN sample used to be enough to stop a whole
    /// plugin (`SPK-9`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut reflections = Reflections::new(RATE);

        for settings in [
            Settings {
                distance: f32::NAN,
                amount: f32::NAN,
            },
            Settings {
                distance: f32::INFINITY,
                amount: -1e9,
            },
            Settings {
                distance: 1e9,
                amount: f32::NEG_INFINITY,
            },
        ] {
            reflections.set(settings);
            assert!(reflections.gain().is_finite());
            let resolved = reflections.settings();
            assert!((0.0..=1.0).contains(&resolved.distance));
            assert!((0.0..=1.0).contains(&resolved.amount));
        }

        reflections.reset();
        reflections.set(far());
        for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e9, -1e9, 0.5] {
            let (left, right) = reflections.process(sample, sample);
            assert!(left.is_finite() && right.is_finite(), "{sample} came back");
        }

        // And it recovers: a finite signal after the hostile one still gets
        // through, which is what "does not latch" means.
        let tone = nxe_audio::harmonics::tone(0.5, 440.0, RATE, 8_192);
        let rendered = render(&mut reflections, &tone);
        assert!(rendered.iter().all(|s| s.is_finite()));
        assert!(energy(&rendered) > 0.0, "the line latched on silence");
    }
}
