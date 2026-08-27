//! `DAMPING`: the high-frequency loss distance brings with it.
//!
//! **One control, two different amounts** (`REQ-VDP-005`). Putting the same
//! filter on the direct sound and on the reflections makes the whole thing
//! muffled rather than distant — what reads as distance is the *difference*
//! between how much the two lose.
//!
//! Specified in `plugins/vocal-depth/docs/specifications/dsp.md`, "DAMPING".
//!
//! **The direct side opens back up on a transient.** A consonant carries
//! further than the body of a note does, and without that the far end is a
//! cloth over the voice. The detector is already running for the presence band
//! (`crate::direct`), so this costs a coefficient rebuild and nothing else.

use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};

/// Where a corner sits with nothing asked of it.
const OPEN_HZ: f32 = 20_000.0;

/// How far a corner may fall, in octaves, at `amount` = 1.
///
/// **The ratio is the whole point** (`REQ-VDP-005`): 1.8 octaves apart at full
/// travel, which at 48 kHz is 8.7 kHz against 2.5 kHz.
const DIRECT_OCTAVES: f32 = 1.2;
const REFLECTED_OCTAVES: f32 = 3.0;

/// How much of `amount` each side feels at the near and far ends of
/// `distance`. The direct sound holds on to more of its top when the voice is
/// close; the reflections lose theirs either way.
///
/// **The spans were narrowed in `VDP-5` to keep the loudness gate.** `DEPTH`
/// moving the corners is what `REQ-VDP-002` asks for, but it is also what makes
/// `DEPTH` move the level: a lowpass takes a different share out of every
/// spectrum, so the normalisation cannot be right for all of them at once (the
/// same wall `VDP-3` hit with the presence band). At `0.35 / 0.50` a sparse
/// harmonic phrase moved **1.26 dB** across `DEPTH` with `DAMPING` open; at
/// `0.55 / 0.70`, with `depth::DAMPING_COMPENSATION` at 0.5, it moves
/// **0.98 dB** and pink noise 0.56 dB. **These are ear numbers otherwise** —
/// how much of the distance cue lives in the top end.
const DIRECT_NEAR: f32 = 0.55;
const DIRECT_SPAN: f32 = 0.45;
const REFLECTED_NEAR: f32 = 0.70;
const REFLECTED_SPAN: f32 = 0.30;

/// How far a full transient opens the direct corner back up.
const TRANSIENT_OCTAVES: f32 = 1.0;

/// A corner may not climb past this fraction of the rate, or the bilinear
/// transform stops meaning anything (`REQ-VDP-017`).
const NYQUIST_CEILING: f32 = 0.45;

/// How far the direct corner may drift from its coefficients before they are
/// rebuilt.
///
/// **The transient moves this corner per sample**, and a rebuild costs a
/// `sin_cos`. A twentieth of an octave is inaudible and bounds the step to
/// that — the same trade `crate::direct` makes for the presence gain.
const RETUNE_STEP: f32 = 1.035;

pub struct Damping {
    sample_rate: f32,
    direct: [Biquad; 2],
    reflected: [Biquad; 2],
    /// How far each side is asked to fall, in octaves below [`OPEN_HZ`].
    /// **Octaves rather than hertz**, because the transient adds to this and
    /// adding hertz to a corner is not what "one octave back" means — the first
    /// version added them in the wrong direction and opened the corner a whole
    /// octave too far (`VDP-5`).
    direct_octaves: f32,
    reflected_octaves: f32,
    /// What the direct coefficients were last built for.
    tuned_hz: f32,
    /// Whether either side is doing anything at all.
    active: bool,
}

impl Damping {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            sample_rate,
            direct: [Biquad::new(Coefficients::PASS); 2],
            reflected: [Biquad::new(Coefficients::PASS); 2],
            direct_octaves: 0.0,
            reflected_octaves: 0.0,
            tuned_hz: f32::NAN,
            active: false,
        };
        built.set(0.0, 0.5);
        built
    }

    /// Resolves both corners. **Block rate.**
    ///
    /// `amount` is `DAMPING` and `distance` is `DEPTH`; both are expected in
    /// `0..=1` and clamped anyway (`REQ-VDP-016`).
    pub fn set(&mut self, amount: f32, distance: f32) {
        let amount = unit(amount);
        let distance = unit(distance);

        // **Exactly nothing at zero.** A control that cannot be switched off is
        // a control nobody can hear the effect of (`REQ-VDP-006` says the same
        // about `CLARITY`).
        self.active = amount > 0.0;
        if !self.active {
            self.direct_octaves = 0.0;
            self.reflected_octaves = 0.0;
            self.tune(Coefficients::PASS, Coefficients::PASS);
            self.tuned_hz = f32::NAN;
            return;
        }

        let direct_amount = amount * (DIRECT_NEAR + DIRECT_SPAN * distance);
        let reflected_amount = amount * (REFLECTED_NEAR + REFLECTED_SPAN * distance);

        self.direct_octaves = DIRECT_OCTAVES * direct_amount;
        self.reflected_octaves = REFLECTED_OCTAVES * reflected_amount;

        let reflected = Coefficients::lowpass(
            self.corner(self.reflected_octaves),
            BUTTERWORTH_Q,
            self.sample_rate,
        );
        for section in &mut self.reflected {
            section.set(reflected);
        }
        self.retune(self.corner(self.direct_octaves));
    }

    /// The direct sound, with the corner opened by however far the transient
    /// detector stands open. **Audio rate.**
    pub fn process_direct(&mut self, left: f32, right: f32, opening: f32) -> (f32, f32) {
        if !self.active {
            return (left, right);
        }

        // **Minus, not plus.** `direct_octaves` counts octaves *down*, and the
        // transient's job is to give some of them back.
        let wanted = self.corner(self.direct_octaves - TRANSIENT_OCTAVES * opening.clamp(0.0, 1.0));
        let ratio = wanted / self.tuned_hz;
        if !(1.0 / RETUNE_STEP..RETUNE_STEP).contains(&ratio) {
            self.retune(wanted);
        }

        (self.direct[0].process(left), self.direct[1].process(right))
    }

    /// The reflection bus. **Audio rate.** No transient opening here — a
    /// consonant that arrives from far away arrives on the direct sound.
    pub fn process_reflected(&mut self, left: f32, right: f32) -> (f32, f32) {
        if !self.active {
            return (left, right);
        }
        (
            self.reflected[0].process(left),
            self.reflected[1].process(right),
        )
    }

    /// The corner the parameters ask of the direct sound, for the loudness
    /// normalisation (`VDP-3`) and the readout (`REQ-VDP-018`).
    ///
    /// **`None` when `DAMPING` is zero**, which is not the same as a corner at
    /// 20 kHz: the audio path uses `Coefficients::PASS` there, and a
    /// normalisation that modelled a real 20 kHz lowpass instead would ask for
    /// 0.08 dB of gain on a chain that does nothing (`VDP-5`).
    ///
    /// **Without the transient**, which is signal-dependent and therefore not
    /// something the normalisation may see.
    pub fn direct_corner_hz(&self) -> Option<f32> {
        self.active.then(|| self.corner(self.direct_octaves))
    }

    pub fn reflected_corner_hz(&self) -> Option<f32> {
        self.active.then(|| self.corner(self.reflected_octaves))
    }

    pub fn reset(&mut self) {
        for section in &mut self.direct {
            section.reset();
        }
        for section in &mut self.reflected {
            section.reset();
        }
    }

    /// `OPEN_HZ` shifted down by `octaves`, held under Nyquist.
    fn corner(&self, octaves: f32) -> f32 {
        let hz = OPEN_HZ * 2.0f32.powf(-octaves.max(0.0));
        hz.min(self.sample_rate * NYQUIST_CEILING)
    }

    fn retune(&mut self, hz: f32) {
        let direct = Coefficients::lowpass(hz, BUTTERWORTH_Q, self.sample_rate);
        for section in &mut self.direct {
            section.set(direct);
        }
        self.tuned_hz = hz;
    }

    fn tune(&mut self, direct: Coefficients, reflected: Coefficients) {
        for section in &mut self.direct {
            section.set(direct);
        }
        for section in &mut self.reflected {
            section.set(reflected);
        }
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

    /// The energy above `hz`, measured by highpassing twice so the filter's own
    /// skirt is not what is being read (`AIR-2` paid for the single-stage
    /// version of this).
    fn energy_above(signal: &[f32], hz: f32) -> f32 {
        let coefficients = Coefficients::highpass(hz, BUTTERWORTH_Q, RATE);
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

    /// **The two sides lose different amounts** (`REQ-VDP-005`), and that is
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

    /// **A transient opens the direct corner back up** (`REQ-VDP-005`).
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

    /// **Full damping does not erase a consonant** (`REQ-VDP-005`). The band a
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
    /// stop meaning anything (`REQ-VDP-017`).
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

    /// Hostile values in, finite values out (`REQ-VDP-016`).
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
