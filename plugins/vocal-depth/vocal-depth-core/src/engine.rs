//! The whole wet chain, plus the dry the mix crossfades against.
//!
//! ```text
//! out = OUTPUT · ((1 - MIX) · dry + MIX · g · (direct + reflections))
//! ```
//!
//! **The dry is branched at the entry and passes through nothing**
//! (`REQ-VDP-001`), which is the whole of the `MIX` = 0 bit-identity: the
//! filters on the direct path are not on the dry one.
//!
//! **`MIX` is a crossfade here, not an addition.** Air adds a layer
//! (`REQ-AIR-012`), but getting further away is taking presence and attack
//! *off* the voice — leave the untouched original in and the thing that was
//! removed is put straight back.
//!
//! Specified in `plugins/vocal-depth/docs/specifications/dsp.md`.
//!
//! **`CLARITY` is not in the normalisation, and cannot be** — it is the output
//! of a follower, so putting it there would make the normalisation depend on the
//! signal (`REQ-VDP-008`). What keeps that honest is that it does nothing on
//! ordinary material and its lift is capped (`crate::clarity`).

use crate::clarity::Clarity;
use crate::damping::Damping;
use crate::depth::{Macros, Probe, Resolved};
use crate::direct::Direct;
use crate::reflections::Reflections;
use crate::width::Width;

/// How fast the engine's own gains follow the parameters.
///
/// **The same 5 ms the other two units settled on**, and the same reason: a
/// gain that steps at block rate is a seam (`VDP-1`).
///
/// This is now the third place in the crate with a one-pole and a settled flag
/// (`reflections` smooths an array of weights, `direct` a single band gain,
/// this the normalisation). **They are not lifted into one block yet because
/// the shapes still differ** — the same call the workspace made about the power
/// followers, which waited until three callers agreed on what they wanted
/// (`docs/HANDOVER.md`).
///
/// **`mix` and `output` are deliberately *not* smoothed here.** They arrive
/// already smoothed from the caller, and smoothing them again breaks the one
/// promise they carry: a session saved with `MIX` = 0 has to be bit-identical
/// **from its first sample**, and an internal ramp from wherever the engine was
/// last makes the first five milliseconds wet (`VDP-4` found this through the
/// wrapper, where the engine's own test had hidden it by resetting afterwards).
const GAIN_SECONDS: f32 = 0.005;

/// Close enough to count as arrived. **Not smaller**: below about
/// `ulp(value) / coefficient` a one-pole stops changing the sum and never
/// finishes (`VDP-1`).
const GAIN_SETTLED: f32 = 1e-4;

/// One smoothed scalar.
#[derive(Clone, Copy)]
struct Smoothed {
    value: f32,
    target: f32,
    coefficient: f32,
    settling: bool,
}

impl Smoothed {
    fn new(value: f32, sample_rate: f32) -> Self {
        Self {
            value,
            target: value,
            coefficient: nxe_audio::envelope::coefficient(GAIN_SECONDS, sample_rate),
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

pub struct Engine {
    sample_rate: f32,
    probe: Probe,
    direct: Direct,
    reflections: Reflections,
    damping: Damping,
    width: Width,
    clarity: Clarity,
    macros: Macros,
    /// The loudness normalisation, resolved from the parameters only.
    normalisation: Smoothed,
    /// Taken as given, not smoothed — see [`GAIN_SECONDS`].
    mix: f32,
    output: f32,
    /// The resolved normalisation before smoothing, for tests and the display.
    resolved_gain: f32,
    /// The last sample of each bus, **taken directly rather than as
    /// `out - dry`**: a subtraction throws away most of the quieter one's
    /// precision (`VEL-*` learned that measuring Velour's layer).
    last_direct: (f32, f32),
    last_reflected: (f32, f32),
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            sample_rate,
            probe: Probe::new(),
            direct: Direct::new(sample_rate),
            reflections: Reflections::new(sample_rate),
            damping: Damping::new(sample_rate),
            width: Width::new(sample_rate),
            clarity: Clarity::new(sample_rate),
            // Not the default: `set` returns early when nothing moved.
            macros: Macros {
                depth: f32::NAN,
                direct: f32::NAN,
                room: f32::NAN,
                damping: f32::NAN,
                width: f32::NAN,
                clarity: f32::NAN,
                mix: f32::NAN,
                output: f32::NAN,
            },
            normalisation: Smoothed::new(1.0, sample_rate),
            mix: 1.0,
            output: 1.0,
            resolved_gain: 1.0,
            last_direct: (0.0, 0.0),
            last_reflected: (0.0, 0.0),
        };
        built.set(Macros::default());
        built.normalisation.snap();
        built
    }

    /// Resolves the macros into every stage. **Block rate**, and it returns
    /// early when nothing moved.
    pub fn set(&mut self, macros: Macros) {
        let macros = macros.sanitised();
        if macros == self.macros {
            return;
        }

        self.direct.set(macros.direct_settings());
        self.reflections.set(macros.reflection_settings());
        self.damping.set(macros.damping, macros.depth);
        self.width.set(macros.width, macros.depth);
        self.clarity.set(macros.clarity, macros.depth);

        // **After the stages, not before.** The normalisation is resolved from
        // what they were actually given, so there is one place that decides
        // what `DEPTH` means.
        self.resolved_gain = self.probe.gain(
            Resolved {
                presence_db: self.direct.presence_db(),
                direct_level: self.direct.target_level(),
                tap_energy: self.reflections.tap_energy(),
                direct_corner_hz: self.damping.direct_corner_hz(),
                reflected_corner_hz: self.damping.reflected_corner_hz(),
                width_power_factor: self.width.reflected_power_factor(),
            },
            self.sample_rate,
        );
        self.normalisation.set(self.resolved_gain);
        self.mix = macros.mix;
        self.output = macros.output;

        self.macros = macros;
    }

    /// One stereo sample. **Audio rate.**
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // The dry, branched before anything (`REQ-VDP-001`).
        let dry = (left, right);

        // **Linked across the pair** (`REQ-VDP-011`): one reading off the mono
        // sum, so nothing here can move the image.
        let lift_db = self.clarity.push((left + right) * 0.5);

        let (direct_left, direct_right) = self.direct.process(left, right, lift_db);
        // **The damping is after the presence band, and it reads the same
        // opening** — a consonant that has to survive the distance is the same
        // event that sharpens the band (`REQ-VDP-005`).
        let (direct_left, direct_right) =
            self.damping
                .process_direct(direct_left, direct_right, self.direct.opening());
        // **Last on the direct path**, and the one operation here a mono sum
        // cannot see (`crate::width`).
        let (direct_left, direct_right) = self.width.direct(direct_left, direct_right);

        let (wet_left, wet_right) = self.reflections.process(left, right);
        let (wet_left, wet_right) = self.damping.process_reflected(wet_left, wet_right);
        let (wet_left, wet_right) = self.width.reflected(wet_left, wet_right);

        let normalisation = self.normalisation.next();
        let mix = self.mix;
        let output = self.output;

        // **After the normalisation**, because the meters answer "what is
        // coming out" and the normalisation is part of the answer.
        self.last_direct = (normalisation * direct_left, normalisation * direct_right);
        self.last_reflected = (normalisation * wet_left, normalisation * wet_right);

        let wet_left = normalisation * (direct_left + wet_left);
        let wet_right = normalisation * (direct_right + wet_right);

        // **Both ends are branches, not multiplications by zero.** At `mix` = 0
        // the promise is bit-identity (`REQ-VDP-001`) and a denormal in the wet
        // would break it; at `mix` = 1 the dry is meant to be gone, and
        // `0.0 · NaN` is `NaN`, so a host sending one non-finite sample would
        // poison a fully wet output that never used the dry at all.
        if mix <= 0.0 {
            return (output * dry.0, output * dry.1);
        }
        if mix >= 1.0 {
            return (output * wet_left, output * wet_right);
        }

        (
            output * ((1.0 - mix) * dry.0 + mix * wet_left),
            output * ((1.0 - mix) * dry.1 + mix * wet_right),
        )
    }

    /// The normalisation the current settings resolve to, before smoothing.
    /// **A function of the parameters alone** (`REQ-VDP-008`).
    pub fn normalisation(&self) -> f32 {
        self.resolved_gain
    }

    /// The two buses as of the last sample, for the meters and the figure
    /// (`REQ-VDP-018`).
    pub fn buses(&self) -> ((f32, f32), (f32, f32)) {
        (self.last_direct, self.last_reflected)
    }

    /// Where each reflection arrives and how loud it is (`REQ-VDP-013`).
    pub fn pattern(&self) -> [(f32, f32); crate::reflections::TAPS] {
        self.reflections.pattern()
    }

    /// How far `CLARITY` is lifting, in dB. **Shown on screen** — a protection
    /// that works invisibly is a control that does nothing (`REQ-VDP-006`).
    pub fn clarity_lift_db(&self) -> f32 {
        self.clarity.lift_db()
    }

    /// How far open the transient detector is (`REQ-VDP-018`).
    pub fn opening(&self) -> f32 {
        self.direct.opening()
    }

    pub fn macros(&self) -> Macros {
        self.macros
    }

    /// The two damping corners, for the display (`REQ-VDP-018`). `None` on a
    /// side means it is passing everything through.
    pub fn damping_corners_hz(&self) -> (Option<f32>, Option<f32>) {
        (
            self.damping.direct_corner_hz(),
            self.damping.reflected_corner_hz(),
        )
    }

    pub fn reset(&mut self) {
        self.direct.reset();
        self.reflections.reset();
        self.damping.reset();
        self.width.reset();
        self.clarity.reset();
        self.normalisation.snap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics;

    const RATE: f32 = 48_000.0;
    /// Long enough for the reflection line to fill and every smoother to
    /// arrive. A relative detector reads nonsense before it settles
    /// (`SPK-18`).
    const DISCARD: usize = (RATE as usize) / 4;

    fn at(depth: f32, room: f32) -> Macros {
        Macros {
            depth,
            room,
            ..Macros::default()
        }
    }

    /// A stand-in for a sung phrase: a fundamental with harmonics, under a
    /// syllabic envelope. **Not a steady tone** — the gate has to hold on
    /// material that starts and stops.
    fn phrase(length: usize) -> Vec<f32> {
        let mut signal = vec![0.0; length];
        for (index, sample) in signal.iter_mut().enumerate() {
            let t = index as f32 / RATE;
            let voice: f32 = [
                (200.0, 1.0f32),
                (400.0, 0.5),
                (600.0, 0.3),
                (1_200.0, 0.2),
                (2_400.0, 0.12),
                (4_800.0, 0.06),
                (9_600.0, 0.03),
            ]
            .iter()
            .map(|(hz, amplitude)| amplitude * (t * hz * std::f32::consts::TAU).sin())
            .sum();

            // Four syllables a second, each with an attack and a decay.
            let syllable = (t * 4.0).fract();
            let envelope = if syllable < 0.1 {
                syllable / 0.1
            } else {
                ((1.0 - syllable) / 0.9).max(0.0)
            };
            *sample = 0.2 * voice * envelope;
        }
        signal
    }

    /// The same phrase with breath in it, which is what a voice actually has
    /// above 8 kHz. **It barely changes the answer** (`VDP-5` measured 1.26
    /// against 1.21 dB before the constants moved), which is how the sparse
    /// stack was ruled out as "too top-poor to be fair".
    fn phrase_with_breath(length: usize) -> Vec<f32> {
        let breath = harmonics::pink(1.0, length);
        phrase(length)
            .iter()
            .zip(&breath)
            .map(|(voice, noise)| voice + 0.06 * noise)
            .collect()
    }

    fn rms(engine: &mut Engine, signal: &[f32]) -> f32 {
        let mut energy = 0.0;
        let mut counted = 0usize;
        for (index, &sample) in signal.iter().enumerate() {
            let (left, right) = engine.process(sample, sample);
            if index >= DISCARD {
                energy += left * left + right * right;
                counted += 2;
            }
        }
        (energy / counted as f32).sqrt()
    }

    /// The spread of the output level across a control, in dB.
    ///
    /// **Peak to peak, which is twice the `±` the requirement is written in**:
    /// `REQ-VDP-008`'s `±1.0 dB` is a 2.0 dB window and therefore a spread of
    /// 2.0. Every number recorded in this crate is a spread — the two were
    /// conflated once, which made the gate read as failing by 0.9 dB when it was
    /// missing the *original* `±0.5 dB` by 0.2 (`VDP-7`).
    fn spread_db(signal: &[f32], settings: impl Fn(f32) -> Macros) -> f32 {
        let levels: Vec<f32> = (0..=10)
            .map(|step| {
                let mut engine = Engine::new(RATE);
                engine.set(settings(step as f32 / 10.0));
                rms(&mut engine, signal)
            })
            .collect();

        let lowest = levels.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = levels.iter().copied().fold(0.0f32, f32::max);
        20.0 * (highest / lowest).log10()
    }

    /// **The gate** (`REQ-VDP-008`): sweeping `DEPTH` may not move the level.
    ///
    /// **The bound is 1.0 dB, and it used to be 0.5.** `VDP-3` measured that no
    /// single compensation reaches 0.5 dB across materials — the presence band
    /// holds 10.3 % of pink noise and 1.2 % of a sparse harmonic phrase, so a
    /// gain forbidden from looking at the signal (`REQ-VDP-008`) is right for
    /// one of them and wrong for the other. The requirement was relaxed to
    /// 1.0 dB with `depth::PRESENCE_COMPENSATION` at 0.6, which is the setting
    /// that minimises the worst case.
    ///
    /// **The materials are the ones `REQ-VDP-008` names — pink noise and a
    /// voice.** White is measured too, in its own test, and deliberately not in
    /// the gate: its energy density *rises* with frequency, so it is not a
    /// stand-in for anything a vocal processor will see, and the probe is pink
    /// for exactly that reason.
    ///
    /// **The bound is 1.8 where the requirement allows 2.0**, so a drift shows
    /// up here before it breaks the promise.
    ///
    /// **`VDP-14` spent most of the margin on purpose.** A listener reported
    /// that the old ranges did not read as distance at all, and the fix was to
    /// widen every cue: the direct sound now loses 9 dB broadband, the
    /// reflections end above unity, and the damping corner reaches 3.8 kHz. The
    /// worst case over pink noise and a voice went from ±0.50 dB to **±0.83 dB**
    /// — which is what the tolerance was relaxed to `±1.0` for.
    ///
    /// Measured spreads (peak to peak) at `DAMPING` 0 / 0.5 / 1: **pink 0.80 /
    /// 1.22 / 1.66**, **phrase 1.12 / 1.36 / 1.46**.
    #[test]
    fn depth_does_not_move_the_loudness() {
        let length = 2 * RATE as usize;
        for (name, signal) in [
            ("pink", harmonics::pink(0.3, length)),
            ("phrase", phrase(length)),
            ("phrase + breath", phrase_with_breath(length)),
        ] {
            for damping in [0.0f32, 0.5, 1.0] {
                let spread = spread_db(&signal, |depth| Macros {
                    depth,
                    room: 0.5,
                    damping,
                    ..Macros::default()
                });
                assert!(
                    spread < 1.8,
                    "{name} at DAMPING {damping}: DEPTH moved the level by {spread:.2} dB \
                     peak to peak"
                );
            }
        }
    }

    /// **What the normalisation cannot do, recorded rather than hidden.**
    /// White noise moves **4.4 dB** across `DEPTH` with `DAMPING` open, and no
    /// setting of `depth::DAMPING_COMPENSATION` reaches it — a lowpass takes
    /// most of white's energy and a fraction of a voice's, and a gain that may
    /// not look at the signal (`REQ-VDP-008`) has to pick one.
    ///
    /// This exists so the number cannot drift without somebody noticing.
    #[test]
    fn white_noise_is_outside_what_the_normalisation_can_hold() {
        let signal = harmonics::noise(0.3, 2 * RATE as usize);

        let closed = spread_db(&signal, |depth| Macros {
            depth,
            room: 0.5,
            damping: 0.0,
            ..Macros::default()
        });
        assert!(
            closed < 1.2,
            "with DAMPING shut even white holds (0.80 on record): {closed:.2} dB"
        );

        let open = spread_db(&signal, |depth| Macros {
            depth,
            room: 0.5,
            damping: 1.0,
            ..Macros::default()
        });
        // **7.6 dB after `VDP-14`**, up from 4.4: widening the damping range
        // put more of the distance cue exactly where white noise keeps its
        // energy.
        assert!(
            (4.0..10.0).contains(&open),
            "white with DAMPING open moved {open:.2} dB, not the 7.6 on record"
        );
    }

    /// And it holds at every point of `MIX`, not only fully wet.
    #[test]
    fn the_gate_holds_at_every_mix() {
        let signal = harmonics::pink(0.3, 2 * RATE as usize);
        for mix in [0.25f32, 0.5, 0.75, 1.0] {
            let spread = spread_db(&signal, |depth| Macros {
                depth,
                room: 0.5,
                mix,
                ..Macros::default()
            });
            assert!(
                spread < 1.8,
                "mix {mix}: DEPTH moved the level by {spread:.2} dB peak to peak"
            );
        }
    }

    /// **The gate holds with `DAMPING` open too** (`REQ-VDP-005`'s last
    /// condition). The corners are in the normalisation, so this is the test
    /// that they are wired into it rather than merely computed.
    #[test]
    fn damping_does_not_move_the_loudness() {
        let signal = harmonics::pink(0.3, 2 * RATE as usize);

        let spread = spread_db(&signal, |depth| Macros {
            depth,
            room: 0.5,
            damping: 1.0,
            ..Macros::default()
        });
        assert!(
            spread < 1.8,
            "with DAMPING open, DEPTH moved the level by {spread:.2} dB peak to peak"
        );

        let across_damping = spread_db(&signal, |amount| Macros {
            depth: 0.7,
            room: 0.5,
            damping: amount,
            ..Macros::default()
        });
        assert!(
            across_damping < 1.8,
            "DAMPING itself moved the level by {across_damping:.2} dB peak to peak"
        );
    }

    /// `ROOM` is an amount, not a level (`REQ-VDP-008`).
    #[test]
    fn room_does_not_move_the_loudness() {
        let signal = harmonics::pink(0.3, 2 * RATE as usize);
        // Measured **1.48 dB peak to peak** (= ±0.74) after `VDP-14` widened
        // the reflection range: `ROOM` reaches above unity now, so there is more
        // of it for the normalisation to be approximately right about.
        let spread = spread_db(&signal, |room| at(0.7, room));
        assert!(
            spread < 1.8,
            "ROOM moved the level by {spread:.2} dB peak to peak"
        );
    }

    /// **The measurement has to be able to fail** (`VEL-10`). With the
    /// normalisation held at one, the same sweep has to move the level a lot —
    /// otherwise the three tests above are measuring nothing.
    #[test]
    fn the_sweep_moves_the_level_without_the_normalisation() {
        let signal = harmonics::pink(0.3, 2 * RATE as usize);

        let levels: Vec<f32> = (0..=10)
            .map(|step| {
                let mut engine = Engine::new(RATE);
                engine.set(at(step as f32 / 10.0, 0.5));
                engine.resolved_gain = 1.0;
                engine.normalisation.set(1.0);
                engine.normalisation.snap();
                rms(&mut engine, &signal)
            })
            .collect();

        let lowest = levels.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = levels.iter().copied().fold(0.0f32, f32::max);
        let spread = 20.0 * (highest / lowest).log10();
        // Measured **1.29 dB** on pink against **0.14 dB** with the
        // normalisation in (`VDP-3`) — a factor of nine.
        assert!(
            spread > 1.0,
            "without the normalisation the sweep only moved {spread:.2} dB, so \
             the gate tests prove nothing"
        );
    }

    /// **The normalisation is a function of the parameters, nothing else**
    /// (`REQ-VDP-008`). Four materials, one number.
    #[test]
    fn the_normalisation_does_not_depend_on_the_signal() {
        let materials = [
            harmonics::pink(0.3, RATE as usize),
            harmonics::noise(0.3, RATE as usize),
            harmonics::tone(0.3, 220.0, RATE, RATE as usize),
            phrase(RATE as usize),
        ];

        let mut resolved: Option<f32> = None;
        for material in &materials {
            let mut engine = Engine::new(RATE);
            engine.set(at(0.8, 0.6));
            for &sample in material {
                engine.process(sample, sample);
            }
            let gain = engine.normalisation();
            match resolved {
                None => resolved = Some(gain),
                Some(first) => assert_eq!(gain, first, "the normalisation followed the signal"),
            }
        }
        assert!(resolved.unwrap() > 0.0);
    }

    /// `MIX` = 0 is bit-identical, which is the one transparency promise every
    /// plugin in the line keeps (`REQ-VDP-001`).
    #[test]
    fn mix_zero_is_bit_identical() {
        let mut engine = Engine::new(RATE);
        engine.set(Macros {
            depth: 0.9,
            room: 1.0,
            mix: 0.0,
            ..Macros::default()
        });
        // **No `reset` first, on purpose.** `mix` is taken as given rather
        // than smoothed here, so the promise holds from the first sample —
        // which is what a session saved with `MIX` = 0 needs (`VDP-4`).

        let left_in = harmonics::noise(0.5, 8_192);
        let right_in: Vec<f32> = left_in.iter().rev().copied().collect();
        for (&left, &right) in left_in.iter().zip(&right_in) {
            let (out_left, out_right) = engine.process(left, right);
            assert_eq!(out_left, left);
            assert_eq!(out_right, right);
        }
    }

    /// Hostile values in, finite values out (`REQ-VDP-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut engine = Engine::new(RATE);

        for macros in [
            Macros {
                depth: f32::NAN,
                direct: f32::NAN,
                room: f32::NAN,
                damping: f32::NAN,
                width: f32::NAN,
                clarity: f32::NAN,
                mix: f32::NAN,
                output: f32::NAN,
            },
            Macros {
                depth: f32::INFINITY,
                direct: -1e9,
                room: 1e9,
                damping: f32::INFINITY,
                width: -1e9,
                clarity: 1e9,
                mix: f32::NEG_INFINITY,
                output: 1e9,
            },
        ] {
            engine.set(macros);
            assert!(engine.normalisation().is_finite());
        }

        // Fully wet, so what is measured is the chain rather than the dry.
        engine.reset();
        engine.set(Macros {
            depth: 0.5,
            room: 0.5,
            mix: 1.0,
            ..Macros::default()
        });
        engine.reset();
        for sample in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e9, -1e9, 0.5] {
            let (left, right) = engine.process(sample, sample);
            assert!(left.is_finite() && right.is_finite(), "{sample} came back");
        }

        // **The dry is a different promise.** It is a wire from the input to
        // the output (`REQ-VDP-001`), so a host that sends a NaN gets one back
        // — the same call Sparkleur made, and for the same reason: cleaning the
        // dry would mean it is not a wire.
        engine.set(Macros {
            depth: 0.5,
            room: 0.5,
            mix: 0.0,
            ..Macros::default()
        });
        engine.reset();
        let (left, _) = engine.process(f32::NAN, f32::NAN);
        assert!(
            left.is_nan(),
            "the dry path was cleaned; it is meant to be a wire"
        );

        let tone = harmonics::tone(0.5, 440.0, RATE, 8_192);
        let mut energy = 0.0;
        for &sample in &tone {
            let (left, _) = engine.process(sample, sample);
            assert!(left.is_finite());
            energy += left * left;
        }
        assert!(energy > 0.0, "the engine latched on silence");
    }
}
