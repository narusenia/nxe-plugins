//! Meniscus: the input on a liquid surface (`plugins/meniscus/docs/implementation/meniscus-plan.md`).
//!
//! The input rings a **surface**: a feedback comb tuned to a pitch — the one
//! you set (**Free**) or the input's own (**Follow**; with no clear pitch the
//! last one holds) — so a note sounds through a body that hums at its
//! harmonics. Inside the loop a soft clip and a low-pass make the resonance
//! non-linear and darker as it rings.
//!
//! An attack (`nxe_audio::onset`, **Sensitivity**) **strikes the surface**: its
//! pitch drops by up to an octave at once, **Splash** of it, and springs back,
//! overshooting and wobbling as it settles. **Tension** is how fast it wobbles
//! (2 to 18 Hz), **Viscosity** how soon it settles — and how dark the surface
//! rings, as a thick liquid would. **Resonance** is how long the surface hums.
//! **Drops** lets Dew's drops fall on the same attacks, two octaves above the
//! surface's pitch.
//!
//! The surface and the drops are added to the dry signal, `Mix` of them: at 0
//! the output is the input. Nothing is delayed.
//!
//! Allocation-free once built: the comb's lines are sized in [`Meniscus::new`].

use crate::dew::{self, Dew};
use nxe_audio::onset::Onset;
use nxe_audio::pitch::Pitch;
use std::f32::consts::TAU;

/// The surface's pitch range, in Hz.
pub const PITCH_HZ: (f32, f32) = (30.0, 1_000.0);
/// Points of [`Meniscus::surface`], oldest first, and the seconds they span.
pub const HISTORY: usize = 96;
pub const HISTORY_S: f32 = 1.5;
/// The deepest strike at full Splash, in semitones.
pub const STRIKE: f32 = 12.0;
/// Samples between spring and coefficient updates.
const CONTROL: usize = 16;
/// The wobble's rate from no Tension to full, in Hz.
const TENSION_HZ: (f32, f32) = (2.0, 18.0);
/// The spring's damping ratio from no Viscosity to full.
const DAMPING: (f32, f32) = (0.06, 0.9);
/// The loop's low-pass, as the share of the new sample, from no Viscosity to
/// full: thin water rings bright, a thick liquid dark.
const BRIGHTNESS: (f32, f32) = (0.85, 0.3);
/// The comb's feedback at full Resonance.
const FEEDBACK: f32 = 0.97;
/// The right channel sits this far above the left, so the surface is wide.
const DETUNE_CENTS: f32 = 7.0;
/// The pitch's glide as Follow or the Pitch knob moves it.
const GLIDE_MS: f32 = 30.0;
/// How sure the pitch tracker must be before Follow believes it.
const CLEAR: f32 = 0.85;
/// The wet signal's ceiling, softly.
const CEILING: f32 = 1.5;
/// Where the drops settle over the surface's pitch.
const DROPS_OVER: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `0..=1`: how small a rise counts as an attack.
    pub sensitivity: f32,
    /// The surface's pitch in Free, in Hz.
    pub pitch: f32,
    pub follow: bool,
    /// `0..=1` each.
    pub tension: f32,
    pub viscosity: f32,
    pub splash: f32,
    pub resonance: f32,
    pub drops: f32,
    pub mix: f32,
}

/// One channel's comb.
struct Comb {
    line: Vec<f32>,
    write: usize,
    /// The loop's low-pass state.
    low: f32,
}

impl Comb {
    fn new(length: usize) -> Self {
        Self {
            line: vec![0.0; length],
            write: 0,
            low: 0.0,
        }
    }

    fn process(&mut self, x: f32, delay: f32, feedback: f32, bright: f32) -> f32 {
        let length = self.line.len();
        let delay = delay.clamp(2.0, (length - 2) as f32);
        let back = self.write as f32 + length as f32 - delay;
        let index = back.floor();
        let fraction = back - index;
        let a = self.line[index as usize % length];
        let b = self.line[(index as usize + 1) % length];
        let delayed = a + (b - a) * fraction;
        self.low += bright * (delayed.tanh() - self.low);
        let y = x + feedback * self.low;
        self.line[self.write] = if y.is_finite() { y } else { 0.0 };
        self.write = (self.write + 1) % length;
        delayed
    }

    fn clear(&mut self) {
        self.line.fill(0.0);
        self.low = 0.0;
    }
}

pub struct Meniscus {
    sample_rate: f32,
    combs: [Comb; 2],
    onset: Onset,
    pitch: Pitch,
    dew: Dew,
    /// Whether Drops was up on the last frame; at 0 the drops stop where
    /// they were, and must not show.
    dropping: bool,
    countdown: usize,
    /// The surface's displacement in semitones, and how fast it moves.
    displacement: f32,
    velocity: f32,
    /// The pitch in use, gliding, and the last one Follow believed.
    hz: f32,
    followed: f32,
    glide: f32,
    /// Per control tick, from Settings.
    delay: [f32; 2],
    feedback: f32,
    scale: f32,
    bright: f32,
    history: [f32; HISTORY],
    /// Where the next point goes, and control ticks per point.
    written: usize,
    ticks: usize,
    per_point: usize,
}

impl Meniscus {
    pub fn new(sample_rate: f32) -> Self {
        // The longest delay: the lowest pitch, struck an octave down.
        let length = (sample_rate / (PITCH_HZ.0 * (-STRIKE / 12.0).exp2())) as usize + 4;
        let point_s = HISTORY_S / HISTORY as f32;
        Self {
            sample_rate,
            combs: [Comb::new(length), Comb::new(length)],
            onset: Onset::new(sample_rate),
            pitch: Pitch::new(sample_rate),
            dew: Dew::new(sample_rate),
            dropping: false,
            countdown: 0,
            displacement: 0.0,
            velocity: 0.0,
            hz: 110.0,
            followed: 110.0,
            glide: 1.0 - (-(CONTROL as f32) / (GLIDE_MS * 0.001 * sample_rate)).exp(),
            delay: [sample_rate / 110.0; 2],
            feedback: 0.0,
            scale: 1.0,
            bright: 1.0,
            history: [0.0; HISTORY],
            written: 0,
            ticks: 0,
            per_point: ((point_s * sample_rate / CONTROL as f32).round() as usize).max(1),
        }
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let input = input.map(|x| if x.is_finite() { x } else { 0.0 });
        let mono = 0.5 * (input[0] + input[1]);
        self.pitch.push(mono);
        if self.onset.push(mono, settings.sensitivity) {
            // Struck: the surface drops at once, however it was moving.
            self.displacement =
                (self.displacement - STRIKE * settings.splash.clamp(0.0, 1.0)).max(-STRIKE);
            self.velocity = 0.0;
        }
        if self.countdown == 0 {
            self.control(settings);
            self.countdown = CONTROL;
        }
        self.countdown -= 1;

        let mut wet = [0.0; 2];
        for (channel, comb) in self.combs.iter_mut().enumerate() {
            wet[channel] = comb.process(
                input[channel] * self.scale,
                self.delay[channel],
                self.feedback,
                self.bright,
            );
        }
        if !(wet[0].is_finite() && wet[1].is_finite()) {
            self.combs.iter_mut().for_each(Comb::clear);
            wet = [0.0; 2];
        }

        let drops = settings.drops.clamp(0.0, 1.0);
        self.dropping = drops > 0.0;
        let dropped = if drops > 0.0 {
            let dew = dew::Settings {
                sensitivity: settings.sensitivity,
                pitch: self.hz * DROPS_OVER,
                size: settings.viscosity,
                splash: settings.splash,
                mix: 1.0,
                follow: settings.follow,
            };
            let out = self.dew.process(input, &dew);
            [out[0] - input[0], out[1] - input[1]]
        } else {
            [0.0; 2]
        };

        let mix = settings.mix.clamp(0.0, 1.0);
        let soft = |x: f32| CEILING * (x / CEILING).tanh();
        [
            input[0] + mix * soft(wet[0] + drops * dropped[0]),
            input[1] + mix * soft(wet[1] + drops * dropped[1]),
        ]
    }

    /// The surface's displacement over the last [`HISTORY_S`], oldest first,
    /// `-1..=1` of [`STRIKE`] (down is negative).
    pub fn surface(&self, out: &mut [f32; HISTORY]) {
        for (index, point) in out.iter_mut().enumerate() {
            *point = self.history[(self.written + index) % HISTORY];
        }
    }

    /// The drops, as `Dew::drops` writes them.
    /// None while Drops is at 0.
    pub fn drops(&self, out: &mut [f32; dew::DROPS * dew::READING]) {
        if self.dropping {
            self.dew.drops(out);
        } else {
            out.fill(0.0);
        }
    }

    /// The surface's pitch at rest, in Hz.
    pub fn hz(&self) -> f32 {
        self.hz
    }

    fn control(&mut self, settings: &Settings) {
        let dt = CONTROL as f32 / self.sample_rate;
        let tension = settings.tension.clamp(0.0, 1.0);
        let viscosity = settings.viscosity.clamp(0.0, 1.0);
        let omega = TAU * TENSION_HZ.0 * (TENSION_HZ.1 / TENSION_HZ.0).powf(tension);
        let zeta = DAMPING.0 + (DAMPING.1 - DAMPING.0) * viscosity;
        // Semi-implicit Euler: stable at these rates and steps.
        self.velocity +=
            (-omega * omega * self.displacement - 2.0 * zeta * omega * self.velocity) * dt;
        self.displacement = (self.displacement + self.velocity * dt).clamp(-STRIKE, STRIKE);

        if settings.follow && self.pitch.clarity() > CLEAR {
            self.followed = self.pitch.hz();
        }
        let target = if settings.follow {
            self.followed
        } else {
            settings.pitch
        }
        .clamp(PITCH_HZ.0, PITCH_HZ.1);
        self.hz *= (target / self.hz).powf(self.glide);

        let hz = self.hz * (self.displacement / 12.0).exp2();
        let detune = (DETUNE_CENTS / 1_200.0).exp2();
        self.delay = [self.sample_rate / hz, self.sample_rate / (hz * detune)];
        let feedback = FEEDBACK * settings.resonance.clamp(0.0, 1.0);
        self.feedback = feedback;
        // Unity power for a broad input, whatever the feedback.
        self.scale = (1.0 - feedback * feedback).sqrt();
        self.bright = BRIGHTNESS.0 + (BRIGHTNESS.1 - BRIGHTNESS.0) * viscosity;

        self.ticks += 1;
        if self.ticks >= self.per_point {
            self.ticks = 0;
            self.history[self.written] = self.displacement / STRIKE;
            self.written = (self.written + 1) % HISTORY;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        sensitivity: 0.5,
        pitch: 110.0,
        follow: false,
        tension: 0.5,
        viscosity: 0.3,
        splash: 0.5,
        resonance: 0.8,
        drops: 0.0,
        mix: 1.0,
    };

    fn run(
        settings: &Settings,
        seconds: f32,
        signal: impl Fn(usize) -> f32,
    ) -> (Meniscus, Vec<f32>) {
        let mut engine = Meniscus::new(RATE);
        let out = (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                engine.process([x, x], settings)[0] - x
            })
            .collect();
        (engine, out)
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|x| x * x).sum()
    }

    fn noise(index: usize) -> f32 {
        let mut x = (index as u32).wrapping_mul(0x9E37_79B9) ^ 0x2545_F491;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x as f32 / u32::MAX as f32 - 0.5) * 0.2
    }

    /// A short burst every 400 ms.
    fn clicks(index: usize) -> f32 {
        let at = index % (RATE as usize * 2 / 5);
        if at < 48 {
            (TAU * 1_000.0 * at as f32 / RATE).sin() * 0.5
        } else {
            0.0
        }
    }

    #[test]
    fn silence_stays_silent() {
        assert_eq!(energy(&run(&SETTINGS, 0.5, |_| 0.0).1), 0.0);
    }

    #[test]
    fn no_mix_is_the_input() {
        let dry = Settings {
            mix: 0.0,
            drops: 1.0,
            ..SETTINGS
        };
        assert_eq!(energy(&run(&dry, 1.0, clicks).1), 0.0);
    }

    #[test]
    fn the_surface_hums_at_its_pitch() {
        // Noise through the comb: the wet part's spectrum peaks at the pitch,
        // not halfway between harmonics.
        let still = Settings {
            splash: 0.0,
            ..SETTINGS
        };
        let (_, wet) = run(&still, 1.0, noise);
        let tail = &wet[wet.len() / 2..];
        let power = |hz: f32| {
            let (mut re, mut im) = (0.0, 0.0);
            for (n, x) in tail.iter().enumerate() {
                let phase = TAU * hz * n as f32 / RATE;
                re += x * phase.cos();
                im += x * phase.sin();
            }
            re * re + im * im
        };
        let on = power(110.0) + power(220.0) + power(330.0);
        let off = power(165.0) + power(275.0) + power(385.0);
        assert!(on > 10.0 * off, "on {on}, off {off}");
    }

    #[test]
    fn a_strike_drops_the_surface_and_it_settles() {
        // Before the first swing back (a quarter of a 6 Hz wobble is ~40 ms).
        let (engine, _) = run(&SETTINGS, 0.015, clicks);
        assert!(engine.displacement < -2.0, "{}", engine.displacement);
        // Long after one strike the surface is back at rest.
        let (engine, _) = run(
            &SETTINGS,
            1.5,
            |index| if index < 48 { clicks(index) } else { 0.0 },
        );
        assert!(engine.displacement.abs() < 0.1, "{}", engine.displacement);
    }

    #[test]
    fn it_overshoots_with_little_viscosity() {
        let thin = Settings {
            viscosity: 0.0,
            ..SETTINGS
        };
        let mut engine = Meniscus::new(RATE);
        let mut highest: f32 = 0.0;
        for index in 0..(RATE as usize) {
            let x = if index < 48 { clicks(index) } else { 0.0 };
            engine.process([x, x], &thin);
            highest = highest.max(engine.displacement);
        }
        assert!(highest > 1.0, "{highest}");
    }

    #[test]
    fn drops_add_to_the_surface() {
        let without = energy(&run(&SETTINGS, 1.0, clicks).1);
        let with = energy(
            &run(
                &Settings {
                    drops: 1.0,
                    ..SETTINGS
                },
                1.0,
                clicks,
            )
            .1,
        );
        assert!(with > without * 1.05, "{with} vs {without}");
    }

    #[test]
    fn bounded_at_full_everything() {
        let wild = Settings {
            tension: 1.0,
            viscosity: 0.0,
            splash: 1.0,
            resonance: 1.0,
            drops: 1.0,
            sensitivity: 1.0,
            ..SETTINGS
        };
        let (_, wet) = run(&wild, 2.0, |index| noise(index) * 5.0 + clicks(index));
        assert!(
            wet.iter()
                .all(|x| x.is_finite() && x.abs() <= 2.0 * CEILING + 0.01)
        );
    }

    #[test]
    fn no_drops_show_at_zero() {
        let mut engine = Meniscus::new(RATE);
        let up = Settings {
            drops: 1.0,
            ..SETTINGS
        };
        for index in 0..4_800 {
            let x = clicks(index);
            engine.process([x, x], &up);
        }
        let mut out = [0.0; dew::DROPS * dew::READING];
        engine.drops(&mut out);
        assert!(out.iter().any(|x| *x != 0.0));
        let x = clicks(4_800);
        engine.process(
            [x, x],
            &Settings {
                drops: 0.0,
                ..SETTINGS
            },
        );
        engine.drops(&mut out);
        assert!(out.iter().all(|x| *x == 0.0));
    }

    #[test]
    fn the_history_records_the_strike() {
        let (engine, _) = run(
            &SETTINGS,
            0.3,
            |index| if index < 48 { clicks(index) } else { 0.0 },
        );
        let mut out = [0.0; HISTORY];
        engine.surface(&mut out);
        assert!(out.iter().any(|x| *x < -0.1), "{out:?}");
    }
}
