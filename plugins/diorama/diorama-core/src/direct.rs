//! The direct sound: the presence band and the transient detector.
//!
//! This is the side that makes a voice *close* (`REQ-DIO-004`). Air adds a
//! layer and never touches the source; **here the source itself is processed**,
//! because getting further away is taking presence and attack off it
//! (`REQ-DIO-001`).
//!
//! Specified in `plugins/diorama/docs/specifications/dsp.md`, "直接音".
//!
//! **Two things are deliberately not here.** The `DAMPING` lowpass is `DIO-5`,
//! and the side attenuation is `DIO-6` — that one belongs with the stereo unit
//! because the promise it has to keep (a mono sum with no comb in it) is
//! measured there, even though `dsp.md` draws it in this part of the chain.

use nxe_audio::biquad::{BandPass, Biquad, Coefficients};
use nxe_audio::envelope::{Power, coefficient};

/// The presence band. Where a vocal reads as near or far.
pub const PRESENCE_LOW_HZ: f32 = 2_000.0;
pub const PRESENCE_HIGH_HZ: f32 = 5_000.0;

/// The band as one peaking section: its geometric centre, and the `Q` that
/// gives it the same 1.32-octave width.
pub const PRESENCE_CENTRE_HZ: f32 = 3_162.0;
pub const PRESENCE_Q: f32 = 1.05;

/// How far the gain may drift from the coefficients before they are rebuilt.
///
/// **A peaking section carries its gain in its coefficients**, so a smoothly
/// moving gain means rebuilding them — and rebuilding costs a `sin_cos`. This
/// is the compromise: the gain is smoothed per sample, the coefficients are
/// rebuilt only when the two have drifted this far apart. 0.05 dB is 25 times
/// finer than anything audible, and it bounds the step to that.
const RETUNE_STEP_DB: f32 = 0.05;

/// What the band is worth at the two ends of closeness.
///
/// **Asymmetric** (`REQ-DIO-002`): adding presence runs into a ceiling, taking
/// it away does not.
const PRESENCE_CLOSE_DB: f32 = 4.0;
const PRESENCE_FAR_DB: f32 = -8.0;

/// How far the direct sound's **broadband** level falls as the voice goes away.
///
/// **The strongest distance cue there is, and it was missing.** `REQ-DIO-002`'s
/// table says the direct sound's share falls with distance, and the first
/// implementation only tilted the presence band — so the direct-to-reflected
/// ratio moved from +30 dB to +17 dB across the whole of `DEPTH` and a listener
/// heard "more effect", not "further away" (`DIO-14`). Real distance takes that
/// ratio to about 0 dB: the reverberant field catches up with the direct sound
/// and then passes it.
///
/// **The loudness normalisation handles this term exactly, for every
/// material.** A broadband gain scales every spectrum identically, which is the
/// one thing the presence band and the damping corners could not do
/// (`crate::depth::PRESENCE_COMPENSATION`) — so spending the distance cue here
/// rather than there makes the gate *more* robust, not less.
const LEVEL_CLOSE_DB: f32 = 0.0;
const LEVEL_FAR_DB: f32 = -9.0;

/// However the terms add up, the band does not move further than this. A
/// parameter is host-controlled input and `CLARITY` adds on top of it
/// (`REQ-DIO-016`).
const PRESENCE_LIMIT_DB: f32 = 10.0;

/// What the transient detector listens to.
///
/// **Band-passed before the followers, which is the condition for borrowing
/// Air's 6 dB range** (`AIR-5` states the same requirement). Two reasons, and
/// the second is a measurement:
///
/// - **Consonants live here.** What the direct path is sharpening or blunting
///   is the attack of a syllable, not the body of a note.
/// - **A sine's power oscillates at twice its frequency.** Full-range, a 2 ms
///   follower tracks that ripple and an 80 ms one does not, so a steady
///   **110 Hz** tone read as **0.204** of a transient — worth 0.6 dB on the
///   band, on a signal with no attack in it at all (`DIO-2`). Above 2 kHz the
///   ripple is far outside the fast follower's reach.
const DETECTION_LOW_HZ: f32 = 2_000.0;
const DETECTION_HIGH_HZ: f32 = 8_000.0;
/// How far the detection band may reach at a given rate, so the top stays
/// under Nyquist at 44.1 kHz (`air_core::follow` caps the same way).
const DETECTION_TOP_CEILING: f32 = 0.45;

/// The two followers. **Both symmetric** — an asymmetric follower sits between
/// the mean and the peak, so putting one in the numerator of a ratio reads 3 dB
/// of transient out of a steady tone and leaves the gate open (`SPK-6`). The
/// release lives behind the ratio, in [`HOLD_SECONDS`].
const FAST_SECONDS: f32 = 0.002;
const SLOW_SECONDS: f32 = 0.080;

/// How much fast-over-slow counts as a full transient. The same 6 dB Air and
/// Sparkleur landed on (`air_core::follow::TRANSIENT_RANGE_DB`).
const SNAP_RANGE_DB: f32 = 6.0;

/// How long the detector holds what it found. This is the release the
/// followers do not have.
const HOLD_SECONDS: f32 = 0.060;

/// What a full transient is worth on the presence band, at either end of
/// closeness: `+3 dB` when near, `-3 dB` when far. Sharpening attacks is what
/// "close" sounds like, and blunting them is what "far" sounds like
/// (`REQ-DIO-002`).
const TRANSIENT_DB: f32 = 3.0;

/// How fast the static part of the band gain follows the parameters.
///
/// **The same lesson `DIO-1` paid for**: a gain that steps at block rate is a
/// seam, whether it is one gain or ten. The transient part needs no smoothing —
/// it comes out of a follower already.
const GAIN_SECONDS: f32 = 0.005;

/// Close enough to count as arrived. **Not smaller**: below about
/// `ulp(gain) / coefficient` a one-pole's step stops changing the sum and the
/// smoothing never finishes (`DIO-1`).
const GAIN_SETTLED: f32 = 1e-4;

/// Below this a follower's reading says nothing about a ratio.
const ENERGY_FLOOR: f32 = 1e-10;

/// `10·log10(x)` is this many times `log2(x)`. Borrowed from
/// `air_core::follow`, where the same ratio is taken per sample.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// What the caller asks for. All clamped: they arrive from a host
/// (`REQ-DIO-016`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Near (0) to far (1). Diorama drives this from `DEPTH`.
    pub distance: f32,
    /// How near the direct sound is *on top of* `distance`, `0..=1` with 0.5
    /// neutral. Diorama drives this from `DIRECT`, which `REQ-DIO-004`
    /// requires to move the band **and** the transient together.
    pub presence: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            distance: 0.5,
            presence: 0.5,
        }
    }
}

/// The transient detector: two power followers and a hold.
///
/// **Not a new detector** (`REQ-DIO-004`) — `nxe_audio::envelope::Power` with
/// two pairs of time constants, which is the same block Air and Sparkleur read.
struct Transient {
    band: BandPass,
    fast: Power,
    slow: Power,
    hold: f32,
    release: f32,
}

impl Transient {
    fn new(sample_rate: f32) -> Self {
        Self {
            band: BandPass::new(
                DETECTION_LOW_HZ,
                DETECTION_HIGH_HZ.min(sample_rate * DETECTION_TOP_CEILING),
                sample_rate,
            ),
            fast: Power::new(FAST_SECONDS, FAST_SECONDS, sample_rate),
            slow: Power::new(SLOW_SECONDS, SLOW_SECONDS, sample_rate),
            hold: 0.0,
            release: coefficient(HOLD_SECONDS, sample_rate),
        }
    }

    /// How far open the detector stands, `0..=1`.
    ///
    /// **Not exactly zero on a steady tone**, which is what `DIO-2` measured
    /// and `dsp.md` had claimed otherwise — see [`DETECTION_LOW_HZ`] for what
    /// the band-pass is there to stop. What matters for the normalisation in
    /// `DIO-3` is that the residue is bounded and small, not that it is zero
    /// (`SPK-3` is the same shape: a steady sine is a weak instrument for a
    /// time constant).
    fn push(&mut self, mono: f32) -> f32 {
        let detected = self.band.process(mono);
        let squared = detected * detected;
        let fast = self.fast.push(squared);
        let slow = self.slow.push(squared);

        let opening = if !(fast.is_finite() && slow.is_finite()) || slow < ENERGY_FLOOR {
            0.0
        } else {
            let excess_db = DECIBELS_PER_OCTAVE_POWER * (fast / slow).max(1e-20).log2();
            (excess_db / SNAP_RANGE_DB).clamp(0.0, 1.0)
        };

        // Attack is the followers'; the release is here, behind the ratio.
        if opening >= self.hold {
            self.hold = opening;
        } else {
            self.hold += (opening - self.hold) * self.release;
        }
        self.hold
    }

    fn reset(&mut self) {
        self.band.reset();
        self.fast.reset();
        self.slow.reset();
        self.hold = 0.0;
    }
}

/// One channel's presence section.
///
/// **A peaking section, not a subtracted band-pass.** `dsp.md` specified
/// `x + (G - 1) · BandPass(x)` because `nxe_audio::biquad` had no shelf, and
/// `DIO-3` measured what that costs: with `G < 1` the band-pass's phase turns
/// the subtraction into a **boost** at the skirts — worst at 8.6 kHz, +0.57 dB
/// — and a nominal 6 dB cut removed only 0.18 dB of pink-weighted power instead
/// of 0.71 dB. Far away is supposed to be *less* presence, so the shape had to
/// change and `Coefficients::peaking` was added
/// (`nxe_audio::biquad::Coefficients::peaking` carries the numbers).
struct Channel {
    section: Biquad,
}

impl Channel {
    fn new() -> Self {
        Self {
            section: Biquad::new(Coefficients::PASS),
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        self.section.process(input)
    }

    fn reset(&mut self) {
        self.section.reset();
    }
}

/// The direct path.
///
/// Allocation happens in [`Direct::new`] and nowhere else. Detection is linked
/// across the pair (`REQ-DIO-011`): one reading off the mono sum drives both
/// channels, so a transient cannot move the image.
pub struct Direct {
    sample_rate: f32,
    transient: Transient,
    channels: [Channel; 2],
    settings: Settings,
    /// `2·closeness - 1`: which way and how hard the transient leans.
    transient_span: f32,
    /// The last opening, for the display (`REQ-DIO-018`).
    opening: f32,
    /// What the parameters ask the band for, without the transient.
    static_db: f32,
    /// The broadband level, linear. Smoothed toward `level_target`.
    level: f32,
    level_target: f32,
    /// Where the smoothed gain has got to, in dB.
    gain_db: f32,
    /// What the coefficients were last built for.
    tuned_db: f32,
    settling: bool,
    coefficient: f32,
}

impl Direct {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            sample_rate,
            transient: Transient::new(sample_rate),
            channels: [Channel::new(), Channel::new()],
            // Not the default: `set` returns early when nothing moved.
            settings: Settings {
                distance: f32::NAN,
                presence: f32::NAN,
            },
            transient_span: 0.0,
            opening: 0.0,
            static_db: 0.0,
            level: 1.0,
            level_target: 1.0,
            gain_db: 0.0,
            tuned_db: f32::NAN,
            settling: false,
            coefficient: coefficient(GAIN_SECONDS, sample_rate),
        };
        built.set(Settings::default());
        built.snap();
        built
    }

    /// Puts the band where the parameters ask, without a ramp. For
    /// construction and [`reset`](Self::reset).
    fn snap(&mut self) {
        self.level = self.level_target;
        self.gain_db = self.static_db;
        self.settling = false;
        self.retune();
    }

    /// Rebuilds the coefficients for the gain the band is at now.
    fn retune(&mut self) {
        let coefficients = Coefficients::peaking(
            PRESENCE_CENTRE_HZ,
            PRESENCE_Q,
            self.gain_db,
            self.sample_rate,
        );
        for channel in &mut self.channels {
            channel.section.set(coefficients);
        }
        self.tuned_db = self.gain_db;
    }

    /// Resolves the settings into a band gain and a transient direction.
    /// **Block rate**, and it returns early when nothing moved.
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            distance: clamped(settings.distance),
            presence: clamped(settings.presence),
        };
        if settings == self.settings {
            return;
        }

        // One number for "how near this is", so `DIRECT` moves the band and
        // the transient together (`REQ-DIO-004`) instead of only the band.
        let closeness = ((1.0 - settings.distance) + (settings.presence - 0.5)).clamp(0.0, 1.0);

        self.static_db = (PRESENCE_FAR_DB + (PRESENCE_CLOSE_DB - PRESENCE_FAR_DB) * closeness)
            .clamp(-PRESENCE_LIMIT_DB, PRESENCE_LIMIT_DB);

        // **Broadband, and it falls with distance** — the cue `REQ-DIO-002`
        // asks for and the first implementation left out (`LEVEL_FAR_DB`).
        //
        // **From `distance` alone, not from `closeness`.** `DIRECT` is "how near
        // the voice itself sounds", which is presence and attack; letting it
        // move the broadband level too would make it a second `MIX`.
        let level_db = LEVEL_FAR_DB + (LEVEL_CLOSE_DB - LEVEL_FAR_DB) * (1.0 - settings.distance);
        self.level_target = 10.0f32.powf(level_db / 20.0);

        self.settling = true;
        self.transient_span = 2.0 * closeness - 1.0;
        self.settings = settings;
    }

    /// One stereo sample of direct sound. **Audio rate.**
    ///
    /// `clarity_lift_db` is what `crate::clarity` is putting back, in dB.
    /// **It arrives per sample rather than per block** because it comes out of a
    /// follower — and it lands on the same section as everything else, so it
    /// costs a coefficient rebuild and no new filter.
    pub fn process(&mut self, left: f32, right: f32, clarity_lift_db: f32) -> (f32, f32) {
        let left = finite(left);
        let right = finite(right);

        self.opening = self.transient.push((left + right) * 0.5);

        // The static part, smoothed. `DIO-1` paid for this: a gain that steps
        // at block rate is a seam.
        if self.settling {
            let remaining = self.static_db - self.gain_db;
            let level_remaining = self.level_target - self.level;
            if remaining.abs() < GAIN_SETTLED && level_remaining.abs() < GAIN_SETTLED {
                self.gain_db = self.static_db;
                self.level = self.level_target;
                self.settling = false;
            } else {
                self.gain_db += remaining * self.coefficient;
                self.level += level_remaining * self.coefficient;
            }
        }

        // The transient, in dB because the gain lives in the coefficients now.
        // **Exactly zero when the detector is shut**, which is what lets the
        // normalisation in `DIO-3` leave it out.
        let wanted_db = (self.gain_db
            + TRANSIENT_DB * self.opening * self.transient_span
            + finite(clarity_lift_db).clamp(0.0, PRESENCE_LIMIT_DB))
        .clamp(-PRESENCE_LIMIT_DB, PRESENCE_LIMIT_DB);
        if (wanted_db - self.tuned_db).abs() > RETUNE_STEP_DB {
            let held = self.gain_db;
            self.gain_db = wanted_db;
            self.retune();
            self.gain_db = held;
        }

        (
            self.channels[0].process(left) * self.level,
            self.channels[1].process(right) * self.level,
        )
    }

    /// How far open the transient detector is, `0..=1` (`REQ-DIO-018`).
    pub fn opening(&self) -> f32 {
        self.opening
    }

    /// The static part of the band gain in dB — what the parameters ask for,
    /// without the transient. `DIO-3` resolves the normalisation from this and
    /// `DIO-11` displays it.
    pub fn presence_db(&self) -> f32 {
        self.static_db
    }

    /// The broadband level the direct sound is asked for, linear.
    ///
    /// **The target rather than where it has got to**: the normalisation is
    /// resolved from the parameters at block rate (`REQ-DIO-008`).
    pub fn target_level(&self) -> f32 {
        self.level_target
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn reset(&mut self) {
        self.transient.reset();
        self.opening = 0.0;
        for channel in &mut self.channels {
            channel.reset();
        }
        self.snap();
    }
}

fn clamped(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics;

    const RATE: f32 = 48_000.0;
    /// Long enough for the slow follower and the hold to forget the onset.
    /// **A relative detector reads infinite transient until it has settled**
    /// (`SPK-18`).
    const DISCARD: usize = (RATE as usize) / 4;

    fn at(distance: f32, presence: f32) -> Settings {
        Settings { distance, presence }
    }

    fn render(direct: &mut Direct, input: &[f32]) -> Vec<f32> {
        input.iter().map(|&s| direct.process(s, s, 0.0).0).collect()
    }

    /// The band's energy in the presence range, measured by filtering the
    /// output with the same band the module uses.
    fn band_energy(signal: &[f32]) -> f32 {
        let mut band = BandPass::new(PRESENCE_LOW_HZ, PRESENCE_HIGH_HZ, RATE);
        signal
            .iter()
            .skip(DISCARD)
            .map(|&s| {
                let filtered = band.process(s);
                filtered * filtered
            })
            .sum()
    }

    /// A burst train: 5 ms of tone every 100 ms. Percussive enough to open the
    /// detector, and repeated so the measurement is not one accident.
    fn bursts(length: usize) -> Vec<f32> {
        let tone = harmonics::tone(0.7, 3_000.0, RATE, length);
        let period = (RATE * 0.1) as usize;
        let on = (RATE * 0.005) as usize;
        tone.iter()
            .enumerate()
            .map(|(i, &s)| if i % period < on { s } else { 0.0 })
            .collect()
    }

    /// `DIRECT` has to move the band **and** the transient depth
    /// (`REQ-DIO-004`). Moving only the band would make it a presence knob
    /// with a misleading name.
    #[test]
    fn presence_moves_the_band_and_the_transient_together() {
        let mut near = Direct::new(RATE);
        near.set(at(0.5, 1.0));
        let mut far = Direct::new(RATE);
        far.set(at(0.5, 0.0));

        assert!(
            near.presence_db() > far.presence_db() + 6.0,
            "the band barely moved: {} against {}",
            near.presence_db(),
            far.presence_db()
        );

        // And the transient leans the other way at the two ends: sharpening
        // when near, blunting when far.
        assert!(
            near.transient_span > 0.5,
            "near does not sharpen: {}",
            near.transient_span
        );
        assert!(
            far.transient_span < -0.5,
            "far does not blunt: {}",
            far.transient_span
        );

        let signal = bursts(RATE as usize);
        let sharpened = band_energy(&render(&mut near, &signal));
        let blunted = band_energy(&render(&mut far, &signal));
        let difference = 10.0 * (sharpened / blunted).log10();
        assert!(
            difference > 6.0,
            "the two ends are only {difference:.1} dB apart"
        );
    }

    /// **What the transient costs the loudness budget on a steady signal.**
    /// `DIO-3` leaves the transient out of a normalisation that has to be
    /// signal-independent (`REQ-DIO-008`) and gives it 0.3 dB of the ±0.5 dB
    /// gate; this is the measurement that has to fit in that.
    ///
    /// **Measured against the same run with the transient turned off**, which
    /// isolates it from the static band gain — the alternative, reading the
    /// opening and multiplying by `TRANSIENT_DB`, answers a different question
    /// and answers it pessimistically: a pure 110 Hz tone opens the detector
    /// 0.204 on nothing but what leaks through the detection band's highpass,
    /// and 0.6 dB of gain on a band with no energy in it costs the output
    /// nothing (`DIO-2`).
    #[test]
    fn a_steady_signal_stays_inside_the_loudness_budget() {
        let cost_db = |signal: &[f32]| {
            let mut with = Direct::new(RATE);
            with.set(at(1.0, 0.0)); // full blunting: the largest lean there is
            let mut without = Direct::new(RATE);
            without.set(at(1.0, 0.0));
            without.transient_span = 0.0;

            let energy = |direct: &mut Direct| -> f32 {
                signal
                    .iter()
                    .enumerate()
                    .map(|(index, &s)| {
                        let (left, _) = direct.process(s, s, 0.0);
                        if index < DISCARD { 0.0 } else { left * left }
                    })
                    .sum()
            };
            let moved = energy(&mut with);
            let still = energy(&mut without);
            10.0 * (moved / still).log10()
        };

        for hz in [110.0f32, 220.0, 440.0, 880.0, 3_000.0] {
            let measured = cost_db(&harmonics::tone(0.5, hz, RATE, RATE as usize));
            assert!(
                measured.abs() < 0.3,
                "a steady {hz} Hz tone moved the output by {measured:.2} dB"
            );
        }

        let measured = cost_db(&harmonics::pink(0.5, RATE as usize));
        assert!(
            measured.abs() < 0.3,
            "pink noise moved the output by {measured:.2} dB"
        );
    }

    /// And the detector itself stays shut on a signal that actually has
    /// presence content — which is the case the detection band-pass is for.
    #[test]
    fn a_stationary_signal_keeps_the_detector_shut() {
        let mut direct = Direct::new(RATE);
        direct.set(at(0.0, 0.5));

        let pink = harmonics::pink(0.5, RATE as usize);
        render(&mut direct, &pink);
        assert!(
            direct.opening() < 0.2,
            "pink noise read as a transient: {}",
            direct.opening()
        );

        let tone = harmonics::tone(0.5, 3_000.0, RATE, RATE as usize);
        render(&mut direct, &tone);
        assert!(
            direct.opening() < 0.05,
            "a steady tone inside the detection band read as a transient: {}",
            direct.opening()
        );
    }

    /// A burst train has to open it. Without this the test above passes on a
    /// detector that is simply broken (`VEL-10`).
    #[test]
    fn a_burst_train_opens_the_detector() {
        let mut direct = Direct::new(RATE);
        direct.set(at(0.0, 0.5));

        let signal = bursts(RATE as usize);
        let mut peak = 0.0f32;
        for (index, &sample) in signal.iter().enumerate() {
            direct.process(sample, sample, 0.0);
            if index > DISCARD {
                peak = peak.max(direct.opening());
            }
        }
        assert!(peak > 0.9, "the detector barely opened: {peak:.3}");
    }

    /// A ratio of two followers cannot depend on the input level
    /// (`REQ-DIO-004`). Measured the way Air measures the same property:
    /// against the range the reading is normalised by.
    #[test]
    fn the_detector_ignores_input_gain() {
        let opening_at = |gain: f32| {
            let mut direct = Direct::new(RATE);
            direct.set(at(0.0, 0.5));
            let signal: Vec<f32> = bursts(RATE as usize).iter().map(|s| s * gain).collect();

            let mut peak = 0.0f32;
            for (index, &sample) in signal.iter().enumerate() {
                direct.process(sample, sample, 0.0);
                if index > DISCARD {
                    peak = peak.max(direct.opening());
                }
            }
            peak
        };

        let reference = opening_at(1.0);
        for gain in [10.0f32.powf(-12.0 / 20.0), 10.0f32.powf(12.0 / 20.0)] {
            let measured = opening_at(gain);
            assert!(
                (measured - reference).abs() < 0.2 / SNAP_RANGE_DB,
                "gain {gain}: {measured} against {reference}"
            );
        }
    }

    /// The followers are symmetric, which is the shape `SPK-6` says a ratio
    /// needs. Measured on the fast one: a step up and a step down have to take
    /// the same number of samples.
    #[test]
    fn the_followers_are_symmetric() {
        let mut follower = Power::new(FAST_SECONDS, FAST_SECONDS, RATE);

        let target = 0.25f32;
        let mut rising = 0i32;
        while follower.energy() < target * (1.0 - 1.0 / std::f32::consts::E) {
            follower.push(target);
            rising += 1;
        }

        // **Settle before turning round.** Measuring the fall from where the
        // rise stopped starts it at 63 % rather than 100 %, and one time
        // constant of that is 0.54 τ — which read as a 96-against-52
        // asymmetry in a follower that is symmetric (`DIO-2`).
        for _ in 0..(RATE * FAST_SECONDS * 20.0) as usize {
            follower.push(target);
        }
        let settled = follower.energy();

        let mut falling = 0i32;
        while follower.energy() > settled / std::f32::consts::E {
            follower.push(0.0);
            falling += 1;
        }

        assert!(
            (rising - falling).abs() <= 2,
            "asymmetric: {rising} up, {falling} down"
        );
    }

    /// Blunting is bounded by construction — `TRANSIENT_DB` and nothing more —
    /// so the consonant is still there when the voice is far away
    /// (`REQ-DIO-005` hands `CLARITY` whatever is left).
    #[test]
    fn blunting_does_not_erase_the_attack() {
        let signal = bursts(RATE as usize);

        let with = |span_settings: Settings| {
            let mut direct = Direct::new(RATE);
            direct.set(span_settings);
            band_energy(&render(&mut direct, &signal))
        };

        // **Distance held still, `DIRECT` moved.** `distance` also moves the
        // broadband level now (`LEVEL_FAR_DB`), and this test is about the
        // transient's share of the band — so the two ends are chosen to put
        // `closeness` at 1 and 0 while leaving the level alone (`DIO-14`).
        let near = with(at(0.5, 1.0));
        let far = with(at(0.5, 0.0));

        // `closeness` runs 1 to 0 across those two, so the static band gain
        // spans its whole range.
        let static_difference = PRESENCE_CLOSE_DB - PRESENCE_FAR_DB;
        let total = 10.0 * (near / far).log10();
        let from_transient = total - static_difference;

        assert!(
            from_transient < 2.0 * TRANSIENT_DB + 1.0,
            "the transient moved the attack by {from_transient:.1} dB, more than \
             the {:.1} dB it is allowed",
            2.0 * TRANSIENT_DB
        );
        assert!(
            from_transient > 1.0,
            "the transient did not move the attack at all: {from_transient:.1} dB"
        );
    }

    /// A parameter step, end to end in one sample, does not arrive as a seam.
    ///
    /// **Two things keep it smooth, and the second turned out to be the
    /// stronger one.** The gain in dB is smoothed per sample; and a peaking
    /// section is *retuned* rather than scaled, and retuning keeps the state,
    /// so the output walks to its new level over the section's ring time
    /// instead of jumping (`nxe_audio::biquad` says the same thing about
    /// `FOCUS`). Taking the smoothing out is therefore **not** a usable
    /// control — measured 0.99, no seam at all (`DIO-2`).
    ///
    /// **So the control jumps the output the way a plain gain would**, which is
    /// what the measurement has to be able to catch (`VEL-10`).
    #[test]
    fn a_parameter_step_does_not_click() {
        let tone = harmonics::tone(0.5, 3_000.0, RATE, 24_000);
        // **Not a whole number of cycles in.** 3 kHz at 48 kHz is 16 samples a
        // cycle, so a step at 12 000 lands exactly on a zero crossing: the
        // band output is near zero there, the seam has nothing to multiply,
        // and even the control measured 1.00 (`DIO-2`).
        let step_at = 12_007;

        let roughness = |as_a_plain_gain: bool| {
            let mut direct = Direct::new(RATE);
            direct.set(at(1.0, 0.0));

            let mut rendered = Vec::with_capacity(tone.len());
            let mut extra = 1.0f32;
            for (index, &sample) in tone.iter().enumerate() {
                if index == step_at {
                    // End to end: -8 dB of presence to +4 dB.
                    direct.set(at(0.0, 1.0));
                    if as_a_plain_gain {
                        // **Big enough to be unmistakable.** The step under
                        // test now moves the broadband level as well, so a
                        // control of the same size as the band change alone no
                        // longer stands out from it (`DIO-14`).
                        extra = 10.0f32.powf(24.0 / 20.0);
                    }
                }
                rendered.push(extra * direct.process(sample, sample, 0.0).0);
                // One sample only: a *step* also raises everything after it,
                // which lifts the background the seam is being compared
                // against and hides itself (measured 1.05, `DIO-2`). What this
                // control has to establish is that a discontinuity of this size
                // would be seen.
                extra = 1.0;
            }

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

        let control = roughness(true);
        assert!(
            control > 2.0,
            "the control did not produce a seam: {control:.2}"
        );

        let measured = roughness(false);
        assert!(
            measured < 1.5,
            "the step left a seam: {measured:.2} (control {control:.2})"
        );
    }

    /// Hostile values in, finite values out, and nothing latched
    /// (`REQ-DIO-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut direct = Direct::new(RATE);

        for settings in [
            Settings {
                distance: f32::NAN,
                presence: f32::NAN,
            },
            Settings {
                distance: f32::INFINITY,
                presence: -1e9,
            },
            Settings {
                distance: 1e9,
                presence: f32::NEG_INFINITY,
            },
        ] {
            direct.set(settings);
            assert!(direct.presence_db().is_finite());
            assert!(direct.presence_db().abs() <= PRESENCE_LIMIT_DB);
        }

        direct.reset();
        direct.set(at(0.5, 0.5));
        for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e9, -1e9, 0.5] {
            let (left, right) = direct.process(sample, sample, 0.0);
            assert!(left.is_finite() && right.is_finite(), "{sample} came back");
        }

        let tone = harmonics::tone(0.5, 440.0, RATE, 8_192);
        let rendered = render(&mut direct, &tone);
        assert!(rendered.iter().all(|s| s.is_finite()));
        let energy: f32 = rendered.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "the band latched on silence");
    }
}
