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
//! **`DAMPING` (`VDP-5`), the stereo width (`VDP-6`) and `CLARITY` (`VDP-7`)
//! are not wired yet.** The normalisation is written so each one drops into the
//! sum in `depth::Probe::gain` as another magnitude.

use crate::depth::{Macros, Probe};
use crate::direct::Direct;
use crate::reflections::Reflections;

/// How fast the engine's own gains follow the parameters.
///
/// **The same 5 ms the other two units settled on**, and the same reason: a
/// gain that steps at block rate is a seam (`VDP-1`).
///
/// This is now the third place in the crate with a one-pole and a settled flag
/// (`reflections` smooths an array of weights, `direct` a single band gain,
/// this two gains and a normalisation). **They are not lifted into one block
/// yet because the shapes still differ** — the same call the workspace made
/// about the power followers, which waited until three callers agreed on what
/// they wanted (`docs/HANDOVER.md`).
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
    macros: Macros,
    /// The loudness normalisation, resolved from the parameters only.
    normalisation: Smoothed,
    mix: Smoothed,
    output: Smoothed,
    /// The resolved normalisation before smoothing, for tests and the display.
    resolved_gain: f32,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let mut built = Self {
            sample_rate,
            probe: Probe::new(),
            direct: Direct::new(sample_rate),
            reflections: Reflections::new(sample_rate),
            // Not the default: `set` returns early when nothing moved.
            macros: Macros {
                depth: f32::NAN,
                direct: f32::NAN,
                room: f32::NAN,
                mix: f32::NAN,
                output: f32::NAN,
            },
            normalisation: Smoothed::new(1.0, sample_rate),
            mix: Smoothed::new(1.0, sample_rate),
            output: Smoothed::new(1.0, sample_rate),
            resolved_gain: 1.0,
        };
        built.set(Macros::default());
        built.normalisation.snap();
        built.mix.snap();
        built.output.snap();
        built
    }

    /// Resolves the macros into every stage. **Block rate**, and it returns
    /// early when nothing moved.
    pub fn set(&mut self, macros: Macros) {
        let macros = macros.sanitised();
        if macros == self.macros {
            return;
        }

        self.direct.set(macros.direct_settings(0.0));
        self.reflections.set(macros.reflection_settings());

        // **After the stages, not before.** The normalisation is resolved from
        // what they were actually given, so there is one place that decides
        // what `DEPTH` means.
        self.resolved_gain = self.probe.gain(
            self.direct.presence_db(),
            self.reflections.tap_energy(),
            self.sample_rate,
        );
        self.normalisation.set(self.resolved_gain);
        self.mix.set(macros.mix);
        self.output.set(macros.output);

        self.macros = macros;
    }

    /// One stereo sample. **Audio rate.**
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        // The dry, branched before anything (`REQ-VDP-001`).
        let dry = (left, right);

        let (direct_left, direct_right) = self.direct.process(left, right);
        let (wet_left, wet_right) = self.reflections.process(left, right);

        let normalisation = self.normalisation.next();
        let mix = self.mix.next();
        let output = self.output.next();

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

    /// How far open the transient detector is (`REQ-VDP-018`).
    pub fn opening(&self) -> f32 {
        self.direct.opening()
    }

    pub fn macros(&self) -> Macros {
        self.macros
    }

    pub fn reset(&mut self) {
        self.direct.reset();
        self.reflections.reset();
        self.normalisation.snap();
        self.mix.snap();
        self.output.snap();
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

    /// **The gate, on broadband material** (`REQ-VDP-008`): sweeping `DEPTH`
    /// may not move the level. Measured **0.14 dB** on pink and **0.19 dB** on
    /// white (`VDP-3`).
    #[test]
    fn depth_does_not_move_the_loudness_on_broadband_material() {
        for (name, signal) in [
            ("pink", harmonics::pink(0.3, 2 * RATE as usize)),
            ("white", harmonics::noise(0.3, 2 * RATE as usize)),
        ] {
            let spread = spread_db(&signal, |depth| at(depth, 0.5));
            assert!(
                spread < 0.5,
                "{name}: DEPTH moved the level by {spread:.2} dB"
            );
        }
    }

    /// **And it does not hold on a sparse harmonic phrase — 1.39 dB against the
    /// 0.5 dB `REQ-VDP-008` asks for.** This records the number rather than
    /// hiding it: it is real, the cause is understood, and what is left is a
    /// decision rather than a bug.
    ///
    /// Three explanations were measured and ruled out (`VDP-3`): it is not the
    /// tail, it is not the metric (a gated loudness reads the same 1.38 dB), and
    /// it is not tap coherence (vibrato and breath move it by 0.1 dB). It is
    /// that **the presence band holds 10.3 % of pink noise and 1.2 % of this
    /// phrase**, so a compensation forbidden from looking at the signal is right
    /// for one of them and wrong for the other.
    /// `depth::PRESENCE_COMPENSATION` carries the whole table.
    #[test]
    fn the_gate_does_not_yet_hold_on_a_sparse_harmonic_phrase() {
        let signal = phrase(2 * RATE as usize);
        let spread = spread_db(&signal, |depth| at(depth, 0.5));
        assert!(
            spread < 1.5,
            "the phrase moved by {spread:.2} dB, worse than the 1.39 dB on record"
        );
        assert!(
            spread > 0.5,
            "the phrase now moves only {spread:.2} dB — if that is real, \
             `REQ-VDP-008` is met and this test should become the gate"
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
                spread < 0.5,
                "mix {mix}: DEPTH moved the level by {spread:.2} dB"
            );
        }
    }

    /// `ROOM` is an amount, not a level (`REQ-VDP-008`).
    #[test]
    fn room_does_not_move_the_loudness() {
        let signal = harmonics::pink(0.3, 2 * RATE as usize);
        let spread = spread_db(&signal, |room| at(0.7, room));
        assert!(spread < 0.5, "ROOM moved the level by {spread:.2} dB");
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
        // **The way a host arrives at it.** nih-plug wakes its smoothers to the
        // parameter's value in `initialize`, so a session that was saved with
        // `MIX` = 0 starts there rather than sliding down to it — and the
        // promise is about the resting state, not about the five milliseconds
        // of a knob being turned (`REQ-VDP-001`).
        engine.reset();

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
                mix: f32::NAN,
                output: f32::NAN,
            },
            Macros {
                depth: f32::INFINITY,
                direct: -1e9,
                room: 1e9,
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
