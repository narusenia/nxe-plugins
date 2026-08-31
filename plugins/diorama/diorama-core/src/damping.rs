//! `DAMPING`: the high-frequency loss distance brings with it.
//!
//! **One control, two different amounts** (`REQ-DIO-005`). Putting the same
//! filter on the direct sound and on the reflections makes the whole thing
//! muffled rather than distant — what reads as distance is the *difference*
//! between how much the two lose.
//!
//! Specified in `plugins/diorama/docs/specifications/dsp.md`, "DAMPING".
//!
//! **The direct side opens back up on a transient.** A consonant carries
//! further than the body of a note does, and without that the far end is a
//! cloth over the voice. The detector is already running for the presence band
//! (`crate::direct`), so this costs a coefficient rebuild and nothing else.
//!
//! ## Two one-poles, not one biquad
//!
//! **The corner moves while signal is running through it**, and that rules out
//! a second-order section: its poles sit near the negative real axis, so
//! replacing its coefficients leaves the `y` history inconsistent with them and
//! **rings at half the sample rate**. `DIO-8` measured 7 to 12 times the
//! background roughness that way, and *finer* retuning made it worse.
//!
//! Two one-poles in series give the same 12 dB an octave with nothing but real
//! poles, so there is no mode to excite
//! (`nxe_audio::biquad::Coefficients::one_pole_lowpass`).

use nxe_audio::biquad::{Biquad, Coefficients};

/// Where a corner sits with nothing asked of it.
const OPEN_HZ: f32 = 20_000.0;

/// How far a corner may fall, in octaves, at `amount` = 1.
///
/// **The ratio is the whole point** (`REQ-DIO-005`): 1.6 octaves apart at full
/// travel, which is 3.8 kHz against 1.25 kHz.
///
/// **The first version stopped at 1.2 and 3.0 octaves, and it was inaudible on
/// a voice** (`DIO-14`): at the default `DAMPING` the direct corner moved
/// 15.9 kHz to 13.2 kHz across the whole of `DEPTH`, and a voice has almost
/// nothing up there. What distance actually sounds like starts rolling off
/// somewhere around 4 to 8 kHz.
const DIRECT_OCTAVES: f32 = 2.4;
const REFLECTED_OCTAVES: f32 = 4.0;

/// How much of `amount` each side feels at the near and far ends of
/// `distance`. The direct sound holds on to more of its top when the voice is
/// close; the reflections lose theirs either way.
///
/// **`DIO-5` narrowed these to protect the loudness gate, and `DIO-14` put them
/// back.** Narrowing them was the trade the gate decision had explicitly
/// rejected — shrinking the effect rather than relaxing the tolerance — and it
/// cost most of what `DEPTH` does to the top end. The gate is kept instead by
/// spending the distance cue on the direct sound's **broadband** level
/// (`crate::direct::LEVEL_FAR_DB`), which the normalisation compensates exactly
/// for every material.
const DIRECT_NEAR: f32 = 0.35;
const DIRECT_SPAN: f32 = 0.65;
const REFLECTED_NEAR: f32 = 0.40;
const REFLECTED_SPAN: f32 = 0.60;

/// How far a full transient opens the direct corner back up.
const TRANSIENT_OCTAVES: f32 = 1.0;

/// A corner may not climb past this fraction of the rate, or the bilinear
/// transform stops meaning anything (`REQ-DIO-017`).
const NYQUIST_CEILING: f32 = 0.45;

/// How far the direct corner may drift from its coefficients before they are
/// rebuilt.
///
/// **The transient moves this corner per sample**, and a rebuild costs a
/// `sin_cos`. A twentieth of an octave is inaudible and bounds the step to
/// that — the same trade `crate::direct` makes for the presence gain.
const RETUNE_STEP: f32 = 1.035;

/// How fast a corner travels toward what the parameters ask for.
///
/// **Retuning keeps a filter's state, but it does not make the change smooth**:
/// `y[n]` starts with `b0 · x[n]`, so a `b0` that jumps puts a step in the
/// output — and switching the whole section in and out of bypass starts it from
/// empty state, which is worse. `DIO-8` measured a step in every macro at once
/// as **66 times** the background roughness, and this side was most of it.
const TRAVEL_SECONDS: f32 = 0.020;

/// Below this many octaves the section is given `Coefficients::PASS`, so zero
/// is exactly transparent.
///
/// **The sections keep running either way.** Skipping `process` while the corner
/// is open leaves their state empty, and the sample where they engage again then
/// starts from zeros in the middle of a waveform — which is a step, and was the
/// last one left in `DIO-8` at **31 times** the background roughness. With
/// `PASS` the state stays filled with the signal, and a near-identity filter
/// handed that history continues smoothly.
const BYPASS_OCTAVES: f32 = 1e-3;

/// How many one-poles each side puts in series. Two gives 12 dB an octave.
const STAGES: usize = 2;

pub struct Damping {
    sample_rate: f32,
    /// `[channel][stage]`.
    direct: [[Biquad; STAGES]; 2],
    reflected: [[Biquad; STAGES]; 2],
    /// How far each side is asked to fall, in octaves below [`OPEN_HZ`].
    ///
    /// **Octaves rather than hertz**, because the transient adds to this and
    /// adding hertz to a corner is not what "one octave back" means — the first
    /// version added them in the wrong direction and opened the corner a whole
    /// octave too far (`DIO-5`).
    direct_target: f32,
    reflected_target: f32,
    /// Where the two corners have got to. Smoothed toward the targets at audio
    /// rate ([`TRAVEL_SECONDS`]).
    direct_octaves: f32,
    reflected_octaves: f32,
    travel: f32,
    /// What each side's coefficients were last built for, in hertz.
    tuned_direct_hz: f32,
    tuned_reflected_hz: f32,
}

impl Damping {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            sample_rate,
            direct: [[Biquad::new(Coefficients::PASS); STAGES]; 2],
            reflected: [[Biquad::new(Coefficients::PASS); STAGES]; 2],
            direct_target: 0.0,
            reflected_target: 0.0,
            direct_octaves: 0.0,
            reflected_octaves: 0.0,
            travel: nxe_audio::envelope::coefficient(TRAVEL_SECONDS, sample_rate),
            tuned_direct_hz: f32::NAN,
            tuned_reflected_hz: f32::NAN,
        };
        built.set(0.0, 0.5);
        built.reset();
        built
    }

    /// Resolves both targets. **Block rate.**
    ///
    /// `amount` is `DAMPING` and `distance` is `DEPTH`; both are expected in
    /// `0..=1` and clamped anyway (`REQ-DIO-016`).
    ///
    /// **Exactly nothing at zero** — a control that cannot be switched off is
    /// one nobody can hear the effect of (`REQ-DIO-006` says the same about
    /// `CLARITY`) — and the corners *travel* there rather than jumping, so
    /// switching it off is not a step either.
    pub fn set(&mut self, amount: f32, distance: f32) {
        let amount = unit(amount);
        let distance = unit(distance);

        let direct_amount = amount * (DIRECT_NEAR + DIRECT_SPAN * distance);
        let reflected_amount = amount * (REFLECTED_NEAR + REFLECTED_SPAN * distance);

        self.direct_target = DIRECT_OCTAVES * direct_amount;
        self.reflected_target = REFLECTED_OCTAVES * reflected_amount;
    }

    /// The direct sound, with the corner opened by however far the transient
    /// detector stands open. **Audio rate.**
    pub fn process_direct(&mut self, left: f32, right: f32, opening: f32) -> (f32, f32) {
        self.direct_octaves = travel(self.direct_octaves, self.direct_target, self.travel);

        // **Minus, not plus.** The octaves count *down*, and the transient's job
        // is to give some of them back.
        let wanted = self.corner(self.direct_octaves - TRANSIENT_OCTAVES * opening.clamp(0.0, 1.0));
        if needs_retune(wanted, self.tuned_direct_hz) {
            let coefficients = self.coefficients(self.direct_octaves, wanted);
            for channel in &mut self.direct {
                for stage in channel {
                    stage.set(coefficients);
                }
            }
            self.tuned_direct_hz = wanted;
        }

        (
            run(&mut self.direct[0], left),
            run(&mut self.direct[1], right),
        )
    }

    /// The reflection bus. **Audio rate.** No transient opening here — a
    /// consonant that arrives from far away arrives on the direct sound.
    pub fn process_reflected(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.reflected_octaves = travel(self.reflected_octaves, self.reflected_target, self.travel);

        let wanted = self.corner(self.reflected_octaves);
        if needs_retune(wanted, self.tuned_reflected_hz) {
            let coefficients = self.coefficients(self.reflected_octaves, wanted);
            for channel in &mut self.reflected {
                for stage in channel {
                    stage.set(coefficients);
                }
            }
            self.tuned_reflected_hz = wanted;
        }

        (
            run(&mut self.reflected[0], left),
            run(&mut self.reflected[1], right),
        )
    }

    /// The corner the parameters ask of the direct sound, for the loudness
    /// normalisation (`DIO-3`) and the readout (`REQ-DIO-018`).
    ///
    /// **`None` when `DAMPING` is zero**, which is not the same as a corner at
    /// 20 kHz: the audio path bypasses the section there, and a normalisation
    /// that modelled a real 20 kHz lowpass instead would ask for 0.08 dB of gain
    /// on a chain that does nothing (`DIO-5`).
    ///
    /// **The target, not where the corner has got to**, and **without the
    /// transient** — both of those move per sample, and the normalisation is
    /// resolved from the parameters (`REQ-DIO-008`).
    pub fn direct_corner_hz(&self) -> Option<f32> {
        (self.direct_target >= BYPASS_OCTAVES).then(|| self.corner(self.direct_target))
    }

    pub fn reflected_corner_hz(&self) -> Option<f32> {
        (self.reflected_target >= BYPASS_OCTAVES).then(|| self.corner(self.reflected_target))
    }

    /// The magnitude a side with this corner has at `hz`.
    ///
    /// **One owner for the shape.** The loudness normalisation has to model
    /// exactly what the audio path does (`crate::depth::Probe::gain`), and a
    /// second copy of "two one-poles, each widened by `STAGE_WIDENING`" is the
    /// doubled arithmetic the rules warn about.
    pub fn magnitude(corner_hz: f32, hz: f32, sample_rate: f32) -> f32 {
        let stage = Coefficients::one_pole_lowpass(corner_hz * STAGE_WIDENING, sample_rate);
        stage.magnitude(hz, sample_rate).powi(STAGES as i32)
    }

    /// Clears the filters and puts both corners where the parameters ask,
    /// without a ramp — what a host does when it loads a session.
    pub fn reset(&mut self) {
        for channel in &mut self.direct {
            for stage in channel {
                stage.reset();
            }
        }
        for channel in &mut self.reflected {
            for stage in channel {
                stage.reset();
            }
        }
        self.direct_octaves = self.direct_target;
        self.reflected_octaves = self.reflected_target;
        self.tuned_direct_hz = f32::NAN;
        self.tuned_reflected_hz = f32::NAN;
    }

    /// One stage's coefficients, or `PASS` when the corner is fully open.
    ///
    /// **The corner is raised so that two stages together are 3 dB down where
    /// one stage would be.** Cascading two identical one-poles doubles the
    /// attenuation in dB, so without this the pair would sit 6 dB down at the
    /// number the readout shows.
    fn coefficients(&self, octaves: f32, wanted: f32) -> Coefficients {
        if octaves < BYPASS_OCTAVES {
            Coefficients::PASS
        } else {
            Coefficients::one_pole_lowpass(wanted * STAGE_WIDENING, self.sample_rate)
        }
    }

    /// `OPEN_HZ` shifted down by `octaves`, held under Nyquist.
    fn corner(&self, octaves: f32) -> f32 {
        let hz = OPEN_HZ * 2.0f32.powf(-octaves.max(0.0));
        hz.min(self.sample_rate * NYQUIST_CEILING)
    }
}

/// How much higher each stage's corner sits so that [`STAGES`] of them are
/// 3 dB down where the readout says.
///
/// For two one-poles: each has to be 1.5 dB down at the corner, which is at
/// `1.55` times its own.
const STAGE_WIDENING: f32 = 1.55;

/// Runs one channel through its stages.
fn run(stages: &mut [Biquad; STAGES], input: f32) -> f32 {
    let mut value = input;
    for stage in stages {
        value = stage.process(value);
    }
    value
}

/// One step of a corner toward its target, snapping when a one-pole would
/// otherwise stall (`DIO-1`).
fn travel(value: f32, target: f32, coefficient: f32) -> f32 {
    let remaining = target - value;
    if remaining.abs() < 1e-4 {
        target
    } else {
        value + remaining * coefficient
    }
}

/// Whether a corner has drifted far enough from its coefficients to be worth a
/// rebuild.
fn needs_retune(wanted: f32, tuned: f32) -> bool {
    if !tuned.is_finite() {
        return true;
    }
    let ratio = wanted / tuned;
    !(1.0 / RETUNE_STEP..RETUNE_STEP).contains(&ratio)
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

    /// The energy above `hz`, measured by highpassing twice so the filter's own
    /// skirt is not what is being read (`AIR-2` paid for the single-stage
    /// version of this).
    fn energy_above(signal: &[f32], hz: f32) -> f32 {
        let coefficients = Coefficients::highpass(hz, nxe_audio::biquad::BUTTERWORTH_Q, RATE);
        let mut stages = [Biquad::new(coefficients); 4];
        signal
            .iter()
            .map(|&sample| {
                let mut value = sample;
                for stage in &mut stages {
                    value = stage.process(value);
                }
                value * value
            })
            .sum()
    }

    fn render(damping: &mut Damping, signal: &[f32], opening: f32, reflected: bool) -> Vec<f32> {
        signal
            .iter()
            .map(|&sample| {
                if reflected {
                    damping.process_reflected(sample, sample).0
                } else {
                    damping.process_direct(sample, sample, opening).0
                }
            })
            .collect()
    }

    /// **Zero is exactly nothing.** Not "a corner at 20 kHz" — the coefficients
    /// are `PASS`, so the samples come back untouched, and the corners report
    /// `None` so the normalisation models the same thing.
    #[test]
    fn zero_is_exactly_transparent() {
        let mut damping = Damping::new(RATE);
        damping.set(0.0, 1.0);
        assert_eq!(damping.direct_corner_hz(), None);
        assert_eq!(damping.reflected_corner_hz(), None);

        let noise = harmonics::noise(0.5, 4_096);
        for &sample in &noise {
            assert_eq!(
                damping.process_direct(sample, sample, 0.0),
                (sample, sample)
            );
            assert_eq!(damping.process_reflected(sample, sample), (sample, sample));
        }
    }

    /// **The two sides lose different amounts** (`REQ-DIO-005`), and that is
    /// what separates distance from a cloth over the voice. Measured on the
    /// corners and on the signal.
    #[test]
    fn the_two_sides_are_damped_by_different_amounts() {
        let mut damping = Damping::new(RATE);
        damping.set(1.0, 1.0);

        let direct = damping.direct_corner_hz().expect("damping is on");
        let reflected = damping.reflected_corner_hz().expect("damping is on");
        let octaves = (direct / reflected).log2();
        assert!(
            octaves > 1.5,
            "the corners are only {octaves:.2} octaves apart ({direct:.0} Hz against {reflected:.0} Hz)"
        );

        let pink = harmonics::pink(0.5, RATE as usize);
        let direct_out = render(&mut damping, &pink, 0.0, false);
        let reflected_out = render(&mut damping, &pink, 0.0, true);
        let difference = 10.0
            * (energy_above(&direct_out, 6_000.0) / energy_above(&reflected_out, 6_000.0)).log10();
        assert!(
            difference > 6.0,
            "above 6 kHz the two sides differ by only {difference:.1} dB"
        );
    }

    /// **A transient opens the direct corner back up** (`REQ-DIO-005`).
    /// Measured against the same run with the opening held shut, which is the
    /// control this needs (`VEL-10`).
    #[test]
    fn a_transient_keeps_the_top_of_the_direct_sound() {
        let pink = harmonics::pink(0.5, RATE as usize);

        let mut shut = Damping::new(RATE);
        shut.set(1.0, 1.0);
        let closed = render(&mut shut, &pink, 0.0, false);

        let mut open = Damping::new(RATE);
        open.set(1.0, 1.0);
        let opened = render(&mut open, &pink, 1.0, false);

        let kept = 10.0 * (energy_above(&opened, 8_000.0) / energy_above(&closed, 8_000.0)).log10();
        assert!(
            kept > 3.0,
            "a full transient only kept {kept:.1} dB more above 8 kHz"
        );
    }

    /// **Full damping does not erase a consonant** (`REQ-DIO-005`). The band a
    /// consonant lives in is quieter, not gone.
    #[test]
    fn full_damping_leaves_a_consonant_audible() {
        let pink = harmonics::pink(0.5, RATE as usize);

        let mut off = Damping::new(RATE);
        off.set(0.0, 1.0);
        let dry = render(&mut off, &pink, 0.0, false);

        let mut on = Damping::new(RATE);
        on.set(1.0, 1.0);
        // **Opening 0**, which is what a sustained vowel gives it. The
        // transient case is the test above; this one asks whether the band is
        // still there when nothing is helping it.
        let wet = render(&mut on, &pink, 0.0, false);

        let lost = 10.0 * (energy_above(&wet, 4_000.0) / energy_above(&dry, 4_000.0)).log10();
        assert!(
            lost > -12.0,
            "full damping took {lost:.1} dB out of the consonant band"
        );
        assert!(
            lost < -1.0,
            "full damping took nothing out of the consonant band: {lost:.1} dB"
        );
    }

    /// The corners stay under Nyquist at every rate, so the coefficients never
    /// stop meaning anything (`REQ-DIO-017`).
    #[test]
    fn the_corners_stay_under_nyquist_at_every_rate() {
        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut damping = Damping::new(rate);
            for amount in [0.0f32, 0.25, 0.5, 1.0] {
                damping.set(amount, 0.5);
                let ceiling = rate * NYQUIST_CEILING + 1.0;
                if let Some(hz) = damping.direct_corner_hz() {
                    assert!(hz <= ceiling, "rate {rate}, amount {amount}: {hz} Hz");
                }
                if let Some(hz) = damping.reflected_corner_hz() {
                    assert!(hz <= ceiling, "rate {rate}, amount {amount}: {hz} Hz");
                }
            }
        }
    }

    /// Hostile values in, finite values out (`REQ-DIO-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut damping = Damping::new(RATE);
        for (amount, distance) in [
            (f32::NAN, 0.5f32),
            (f32::INFINITY, f32::NEG_INFINITY),
            (1e9, -1e9),
        ] {
            damping.set(amount, distance);
            assert!(damping.direct_corner_hz().is_none_or(f32::is_finite));
            assert!(damping.reflected_corner_hz().is_none_or(f32::is_finite));
        }

        damping.set(1.0, 1.0);
        for sample in [f32::NAN, f32::INFINITY, 1e9, 0.5] {
            let (left, _) = damping.process_direct(sample, sample, f32::NAN);
            let _ = left;
        }
        // A non-finite sample latches a biquad, which is why the engine
        // sanitises at its entry (`SPK-9`) — what this pins is that a hostile
        // *parameter* cannot do it.
        damping.reset();
        damping.set(1.0, 0.5);
        let tone = harmonics::tone(0.5, 1_000.0, RATE, 4_096);
        let rendered = render(&mut damping, &tone, f32::NAN, false);
        assert!(rendered.iter().all(|s| s.is_finite()));
    }
}
