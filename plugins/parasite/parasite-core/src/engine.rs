//! Parasite: an oscillator that lives on the input's pitch and feeds on it
//! (`parasite-plan.md`, decisions 3–7).
//!
//! The host (the input, as a mono sum) is tracked for pitch and level. The
//! parasite is up to three sine voices at that pitch times a ratio: one at
//! the pitch, and `Spread` octaves either side of it. How it attaches is the
//! `Species`:
//!
//! ```text
//! FM     m = x(t − D·p)            the host's phase is pushed by the parasite
//! Ring   m = x·(1 − I + I·p)       the host is multiplied by it
//! Sync   m = x·(1 − I + I·s)       s: a voice reset every host period,
//!                                  swept up by I, windowed so the reset is smooth
//! Voice  m = x + I·v               the parasite sings along
//! ```
//!
//! `I` is Infection. `Hunger` moves the output from `m` towards the
//! parasite's own tone `v` (its voices at the host's level), and grows over a
//! held note: a parasite feeds while the host sustains. `Resistance` is the
//! host fighting back — on an attack both fall away. With no pitch to follow
//! (a drum, a breath) the parasite keeps the last one and fades out.
//!
//! Everything runs through a fixed delay — the deepest FM push — so the
//! species switch without a jump and the dry signal lines up; the plugin
//! reports it as latency.

use crate::pitch::{self, Pitch};
use nxe_audio::envelope::Power;
use nxe_audio::{DelayLine, Envelope};
use std::f32::consts::{PI, TAU};

/// How the parasite attaches to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Species {
    Fm,
    Ring,
    Sync,
    Voice,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub species: Species,
    /// `0..=1`: how hard the parasite acts on the host.
    pub infection: f32,
    /// `0..=1`: how far the side voices sit, up to [`SPREAD_OCTAVES`].
    pub spread: f32,
    /// `0..=1`: how much of the host the parasite takes over.
    pub hunger: f32,
    /// `0..=1`: how far the voices wander off their ratios.
    pub mutation: f32,
    /// `0..=1`: how much an attack shakes the parasite off.
    pub resistance: f32,
    /// `0..=1`: how readily a pitch is believed.
    pub sensitivity: f32,
    /// `0..=1`.
    pub mix: f32,
    /// Linear gain on the output.
    pub output: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            species: Species::Fm,
            infection: 0.5,
            spread: 0.0,
            hunger: 0.3,
            mutation: 0.1,
            resistance: 0.5,
            sensitivity: 0.5,
            mix: 1.0,
            output: 1.0,
        }
    }
}

/// How far `Spread` = 1 puts the side voices, each way.
pub const SPREAD_OCTAVES: f32 = 2.0;
/// FM's index at full infection, for a component at the host's pitch.
const FM_INDEX: f32 = 3.0;
/// How far a voice drifts at full mutation, in semitones each way.
const MUTATION_SEMITONES: f32 = 7.0;
/// How fast the drift moves.
const MUTATION_HZ: f32 = 0.7;
/// How fast the parasite fades in on a pitch, and out without one.
const LATCH_SECONDS: f32 = 0.01;
const FADE_SECONDS: f32 = 0.3;
/// The glide between pitches.
const GLIDE_SECONDS: f32 = 0.015;
/// How long a held note takes to feed the parasite fully.
const FEED_SECONDS: f32 = 0.25;
/// The attack detector: two mean-square followers, and how far the short one
/// has to run ahead of the long one before it counts, and before it is a full
/// attack.
const ATTACK_FAST: f32 = 0.005;
const ATTACK_SLOW: f32 = 0.06;
const ATTACK_FROM: f32 = 1.3;
const ATTACK_FULL: f32 = 2.0;
/// Sync's sweep at full infection: the voice runs this much faster than
/// the host before each reset.
const SYNC_SWEEP: f32 = 3.0;
/// The parasite's own tone at the host's level: a sine's peak over its mean
/// absolute value.
const TONE_GAIN: f32 = PI / 2.0;
/// The window figure: how many points, over how many host periods.
pub const SCOPE_POINTS: usize = 256;
const SCOPE_PERIODS: f32 = 2.0;
const SCOPES_PER_SECOND: f32 = 30.0;

/// What the window shows besides the waves.
#[derive(Clone, Copy, Debug, Default)]
pub struct Readings {
    /// The pitch the parasite follows, Hz.
    pub hz: f32,
    /// `0..=1`: whether it has a pitch to follow, faded.
    pub presence: f32,
    /// `0..=1`: infection as it acts now.
    pub infection: f32,
    /// `0..=1`: hunger as it acts now, after feeding and resistance.
    pub hunger: f32,
}

/// White noise rounded off by one pole and scaled back to about `±1`.
struct Wander {
    state: u32,
    pole: f32,
    gain: f32,
    value: f32,
}

impl Wander {
    fn new(seed: u32, sample_rate: f32) -> Self {
        let pole = (-TAU * MUTATION_HZ / sample_rate).exp();
        Self {
            state: seed,
            pole,
            // A one-pole's output deviation from white noise of ±1 (σ = 1/√3).
            gain: ((1.0 + pole) / (1.0 - pole)).sqrt() * 3f32.sqrt() * 0.5,
            value: 0.0,
        }
    }

    fn next(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        let white = self.state as f32 / u32::MAX as f32 * 2.0 - 1.0;
        self.value = self.pole * self.value + (1.0 - self.pole) * white;
        (self.value * self.gain).clamp(-1.0, 1.0)
    }
}

pub struct Parasite {
    sample_rate: f32,
    pitch: Pitch,
    /// The followed pitch, as log2 Hz, gliding.
    log_hz: f32,
    target_log_hz: f32,
    presence: f32,
    presence_target: f32,
    latch: f32,
    fade: f32,
    glide: f32,
    level: Envelope,
    fast: Power,
    slow: Power,
    fed: f32,
    feed: f32,
    voices: [f32; 3],
    wanders: [Wander; 3],
    /// The host period's own phase, which Sync resets on.
    host_phase: f32,
    sync_phase: f32,
    lines: [DelayLine; 2],
    /// The fixed delay, in samples.
    centre: f32,
    readings: Readings,
    // The window figure.
    history: Vec<[f32; 2]>,
    history_at: usize,
    since_scope: usize,
    scope: Option<([f32; SCOPE_POINTS], [f32; SCOPE_POINTS])>,
}

impl Parasite {
    pub fn new(sample_rate: f32) -> Self {
        let centre = (FM_INDEX / (TAU * pitch::LOW_HZ) * sample_rate).ceil() + 2.0;
        let line = || DelayLine::new(sample_rate, 2.0 * centre / sample_rate + 0.001);
        let history = (SCOPE_PERIODS * sample_rate / pitch::LOW_HZ) as usize + 2;
        Self {
            sample_rate,
            pitch: Pitch::new(sample_rate),
            log_hz: 220f32.log2(),
            target_log_hz: 220f32.log2(),
            presence: 0.0,
            presence_target: 0.0,
            latch: nxe_audio::envelope::coefficient(LATCH_SECONDS, sample_rate),
            fade: nxe_audio::envelope::coefficient(FADE_SECONDS, sample_rate),
            glide: nxe_audio::envelope::coefficient(GLIDE_SECONDS, sample_rate),
            level: Envelope::new(0.005, 0.08, sample_rate),
            fast: Power::new(ATTACK_FAST, ATTACK_FAST, sample_rate),
            slow: Power::new(ATTACK_SLOW, ATTACK_SLOW, sample_rate),
            fed: 0.0,
            feed: nxe_audio::envelope::coefficient(FEED_SECONDS, sample_rate),
            voices: [0.0; 3],
            wanders: [
                Wander::new(0x9E37_79B9, sample_rate),
                Wander::new(0x85EB_CA6B, sample_rate),
                Wander::new(0xC2B2_AE35, sample_rate),
            ],
            host_phase: 0.0,
            sync_phase: 0.0,
            lines: [line(), line()],
            centre,
            readings: Readings::default(),
            history: vec![[0.0; 2]; history],
            history_at: 0,
            since_scope: 0,
            scope: None,
        }
    }

    /// The fixed delay, in samples: the plugin's latency. A read at a delay
    /// of one is the sample just written, so it is one less than the read.
    pub fn latency(&self) -> usize {
        self.centre as usize - 1
    }

    pub fn readings(&self) -> Readings {
        self.readings
    }

    /// The host and the output over the last two host periods, starting on
    /// the host period's start, when a new one is ready.
    pub fn take_scope(&mut self) -> Option<([f32; SCOPE_POINTS], [f32; SCOPE_POINTS])> {
        self.scope.take()
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let input = input.map(|x| if x.is_finite() { x } else { 0.0 });
        let mono = (input[0] + input[1]) * 0.5;

        // Following the host.
        if self.pitch.push(mono) {
            // Sensitivity 0 believes only a clean tone, 1 almost anything.
            let threshold = 0.9 - 0.6 * settings.sensitivity.clamp(0.0, 1.0);
            if self.pitch.clarity() >= threshold {
                self.target_log_hz = self.pitch.hz().log2();
                self.presence_target = 1.0;
            } else {
                self.presence_target = 0.0;
            }
        }
        self.log_hz += (self.target_log_hz - self.log_hz) * self.glide;
        let rate = if self.presence_target > self.presence {
            self.latch
        } else {
            self.fade
        };
        self.presence += (self.presence_target - self.presence) * rate;
        let hz = self.log_hz.exp2();

        self.level.push(mono);
        let fast = self.fast.push(mono * mono);
        let slow = self.slow.push(mono * mono);
        let level = self.level.level();
        // How much of an attack this is: the short-term level ahead of the
        // long-term one. A held note's own ripple stays under the dead zone.
        let attack = if slow > 1e-12 {
            (((fast / slow).sqrt() - ATTACK_FROM) / (ATTACK_FULL - ATTACK_FROM)).clamp(0.0, 1.0)
        } else if fast > 1e-12 {
            1.0
        } else {
            0.0
        };
        let held = self.presence * (1.0 - attack);
        self.fed += (held - self.fed) * self.feed;
        let shaken = 1.0 - settings.resistance.clamp(0.0, 1.0) * attack;
        let infection = settings.infection.clamp(0.0, 1.0) * self.presence * shaken;
        let hunger = settings.hunger.clamp(0.0, 1.0) * self.fed * shaken;

        // The parasite.
        let spread = settings.spread.clamp(0.0, 1.0) * SPREAD_OCTAVES;
        let mutation = settings.mutation.clamp(0.0, 1.0) * MUTATION_SEMITONES / 12.0;
        let sides = (settings.spread * 4.0).clamp(0.0, 1.0);
        let mut voice = 0.0;
        for (index, phase) in self.voices.iter_mut().enumerate() {
            let octave = (index as f32 - 1.0) * spread + mutation * self.wanders[index].next();
            *phase = (*phase + hz * octave.exp2() / self.sample_rate).fract();
            let weight = if index == 1 { 1.0 } else { sides };
            voice += weight * (TAU * *phase).sin();
        }
        let voice = voice / (1.0 + 2.0 * sides);

        let previous = self.host_phase;
        self.host_phase = (self.host_phase + hz / self.sample_rate).fract();
        let wrapped = self.host_phase < previous;
        if wrapped {
            self.sync_phase = 0.0;
        }
        self.sync_phase =
            (self.sync_phase + hz * (1.0 + SYNC_SWEEP * infection) / self.sample_rate).fract();
        // Windowed by the host period, so the reset lands on silence.
        let synced = (TAU * self.sync_phase).sin() * (PI * self.host_phase).sin();

        let tone = match settings.species {
            Species::Sync => synced,
            _ => voice,
        } * level
            * TONE_GAIN;

        // shortcut: Ring and Sync are not oversampled, so a high voice with a wide
        // Spread folds back; run them 4x like `nonlinear-core` if it is heard.
        let push = FM_INDEX / (TAU * hz) * self.sample_rate * infection;
        let mut output = [0.0; 2];
        for (channel, line) in self.lines.iter_mut().enumerate() {
            line.write(input[channel]);
            let dry = line.read(self.centre);
            let host = match settings.species {
                Species::Fm => line.read(self.centre + push * voice),
                Species::Ring => dry * (1.0 - infection + infection * voice),
                Species::Sync => dry * (1.0 - infection + infection * synced),
                Species::Voice => dry + infection * tone,
            };
            let wet = host + (tone - host) * hunger;
            let mixed = dry + (wet - dry) * settings.mix.clamp(0.0, 1.0);
            output[channel] = mixed * settings.output;
        }

        self.readings = Readings {
            hz,
            presence: self.presence,
            infection,
            hunger,
        };
        self.record(output, wrapped);
        output
    }

    /// Keeps the delayed host and the output for the window figure, and on a
    /// host period's start cuts the last two periods into a picture.
    fn record(&mut self, output: [f32; 2], wrapped: bool) {
        let host = (self.lines[0].read(self.centre) + self.lines[1].read(self.centre)) * 0.5;
        let out = (output[0] + output[1]) * 0.5;
        let length = self.history.len();
        self.history[self.history_at] = [host, out];
        self.history_at = (self.history_at + 1) % length;
        self.since_scope += 1;
        let due = self.since_scope as f32 >= self.sample_rate / SCOPES_PER_SECOND;
        if !(wrapped && due) {
            return;
        }
        self.since_scope = 0;
        let span = (SCOPE_PERIODS * self.sample_rate / self.readings.hz.max(pitch::LOW_HZ))
            .min((length - 2) as f32);
        let mut host_points = [0.0; SCOPE_POINTS];
        let mut out_points = [0.0; SCOPE_POINTS];
        for point in 0..SCOPE_POINTS {
            // Back from the newest sample, oldest first.
            let back = span * (1.0 - point as f32 / (SCOPE_POINTS - 1) as f32);
            let whole = back.floor();
            let fraction = back - whole;
            let at = |offset: usize| self.history[(self.history_at + length - 1 - offset) % length];
            let (newer, older) = (at(whole as usize), at(whole as usize + 1));
            host_points[point] = newer[0] + (older[0] - newer[0]) * fraction;
            out_points[point] = newer[1] + (older[1] - newer[1]) * fraction;
        }
        self.scope = Some((host_points, out_points));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn sine(index: usize, hz: f32) -> f32 {
        (TAU * hz * index as f32 / RATE).sin() * 0.5
    }

    /// The level of `hz` in `samples`, by Goertzel, as a sine's amplitude.
    fn level_at(samples: &[f32], hz: f32) -> f32 {
        let coefficient = 2.0 * (TAU * hz / RATE).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for x in samples {
            let s0 = x + coefficient * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coefficient * s1 * s2).max(0.0).sqrt() * 2.0 / samples.len() as f32
    }

    fn run(settings: &Settings, signal: impl Fn(usize) -> f32) -> Vec<f32> {
        let mut parasite = Parasite::new(RATE);
        (0..RATE as usize)
            .map(|index| {
                let x = signal(index);
                parasite.process([x, x], settings)[0]
            })
            .collect()
    }

    fn tail(samples: &[f32]) -> &[f32] {
        &samples[samples.len() / 2..]
    }

    #[test]
    fn mix_zero_is_the_input_late_by_the_latency() {
        let settings = Settings {
            mix: 0.0,
            ..Settings::default()
        };
        let mut parasite = Parasite::new(RATE);
        let latency = parasite.latency();
        let input: Vec<f32> = (0..4_000).map(|i| sine(i, 330.0)).collect();
        let output: Vec<f32> = input
            .iter()
            .map(|x| parasite.process([*x, *x], &settings)[0])
            .collect();
        for index in latency..input.len() {
            assert!((output[index] - input[index - latency]).abs() < 1e-3);
        }
    }

    #[test]
    fn ring_on_a_tone_moves_it_up_an_octave() {
        let settings = Settings {
            species: Species::Ring,
            infection: 1.0,
            hunger: 0.0,
            mutation: 0.0,
            ..Settings::default()
        };
        let out = run(&settings, |i| sine(i, 220.0));
        let (fundamental, octave) = (level_at(tail(&out), 220.0), level_at(tail(&out), 440.0));
        assert!(
            octave > fundamental * 4.0,
            "220 {fundamental}, 440 {octave}"
        );
    }

    #[test]
    fn fm_on_a_tone_adds_harmonics() {
        let settings = Settings {
            species: Species::Fm,
            infection: 1.0,
            hunger: 0.0,
            mutation: 0.0,
            ..Settings::default()
        };
        let clean = run(
            &Settings {
                infection: 0.0,
                ..settings
            },
            |i| sine(i, 220.0),
        );
        let out = run(&settings, |i| sine(i, 220.0));
        assert!(
            level_at(tail(&out), 440.0) > 0.05,
            "{}",
            level_at(tail(&out), 440.0)
        );
        assert!(level_at(tail(&clean), 440.0) < 0.005);
    }

    #[test]
    fn every_species_stays_finite_and_bounded() {
        for species in [Species::Fm, Species::Ring, Species::Sync, Species::Voice] {
            let settings = Settings {
                species,
                infection: 1.0,
                hunger: 1.0,
                spread: 1.0,
                mutation: 1.0,
                ..Settings::default()
            };
            let out = run(&settings, |i| {
                let hz = 100.0 + 400.0 * (i as f32 / RATE);
                ((hz * i as f32 / RATE).fract() * 2.0 - 1.0) * 0.8
            });
            let peak = out.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
            assert!(out.iter().all(|x| x.is_finite()), "{species:?}");
            assert!(peak < 4.0, "{species:?}: {peak}");
        }
    }

    #[test]
    fn hunger_feeds_on_a_held_note() {
        let settings = Settings {
            species: Species::Ring,
            infection: 0.0,
            hunger: 1.0,
            ..Settings::default()
        };
        let mut parasite = Parasite::new(RATE);
        for index in 0..RATE as usize / 20 {
            let x = sine(index, 220.0);
            parasite.process([x, x], &settings);
        }
        let early = parasite.readings().hunger;
        for index in 0..RATE as usize {
            let x = sine(index, 220.0);
            parasite.process([x, x], &settings);
        }
        let late = parasite.readings().hunger;
        assert!(
            late > 0.9 && early < late * 0.6,
            "early {early}, late {late}"
        );
    }

    #[test]
    fn without_a_pitch_it_fades_out() {
        let mut parasite = Parasite::new(RATE);
        let settings = Settings::default();
        for index in 0..RATE as usize / 2 {
            let x = sine(index, 220.0);
            parasite.process([x, x], &settings);
        }
        assert!(parasite.readings().presence > 0.9);
        let mut state = 0x2468_ACE1_u32;
        for _ in 0..RATE as usize {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let x = (state as f32 / u32::MAX as f32 - 0.5) * 0.5;
            parasite.process([x, x], &settings);
        }
        let readings = parasite.readings();
        assert!(readings.presence < 0.1, "{}", readings.presence);
        // The pitch is kept, not dropped to wherever the noise pointed.
        assert!((readings.hz / 220.0 - 1.0).abs() < 0.05, "{}", readings.hz);
    }

    #[test]
    fn the_scope_starts_on_a_period() {
        let mut parasite = Parasite::new(RATE);
        let settings = Settings::default();
        let mut scope = None;
        for index in 0..RATE as usize / 2 {
            let x = sine(index, 220.0);
            parasite.process([x, x], &settings);
            if let Some(taken) = parasite.take_scope() {
                scope = Some(taken);
            }
        }
        let (host, _) = scope.expect("a scope");
        assert!(host.iter().any(|x| x.abs() > 0.4));
    }
}
