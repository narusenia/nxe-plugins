//! Dew: water drops on the input's attacks (`resonance-kit-plan.md`).
//!
//! An attack is the short power average jumping well above the long one. Each
//! attack lets fall one to six drops. A drop is a damped sine whose pitch
//! slides up and settles, the way a drop's bubble rings as it closes. **Size**
//! is how big the drops are — bigger is lower and longer — and **Splash** how
//! many fall, how far apart in time and stereo, and how loosely tuned. In
//! **Follow** a drop settles on the harmonic of the input's pitch nearest to
//! where its size puts it; with no clear pitch it falls as in **Free**.
//!
//! The drops are added to the dry signal, `Mix` of them: at 0 the output is the
//! input. Nothing is delayed, so there is no latency to report.
//!
//! Allocation-free once built.

use nxe_audio::pitch::Pitch;
use std::f32::consts::{FRAC_PI_4, TAU};

/// The most drops sounding at once. A new one takes the quietest slot.
pub const DROPS: usize = 8;
/// What [`Dew::drops`] writes per drop: pan, height, level, age.
pub const READING: usize = 4;

/// The smallest drop's settled pitch; Size takes it down [`SIZE_OCTAVES`].
const SMALL_HZ: f32 = 4_000.0;
const SIZE_OCTAVES: f32 = 3.0;
/// A drop starts this much under where it settles (`1 / (1 + RISE)`).
const RISE: f32 = 0.6;
/// How fast it settles, and how long it rings, from the smallest drop to the
/// largest. Both are time constants.
const RISE_MS: (f32, f32) = (3.0, 12.0);
const DECAY_MS: (f32, f32) = (25.0, 150.0);
/// How far apart in time a splash lets its drops fall, at full Splash.
const SPREAD_MS: f32 = 60.0;
/// How far off its size a drop may fall in Free, in semitones either way.
const JITTER: (f32, f32) = (1.0, 6.0);

/// The attack detector: two power averages, and the ratio between them that
/// counts as an attack (6 dB in level).
const FAST_MS: f32 = 2.0;
const SLOW_MS: f32 = 60.0;
const ATTACK_RATIO: f32 = 4.0;
/// Below this power (-50 dBFS) nothing is an attack.
const FLOOR: f32 = 1e-5;
/// After an attack, the next one waits at least this long.
const HOLDOFF_MS: f32 = 50.0;
/// How sure the pitch tracker must be before Follow believes it.
const CLEAR: f32 = 0.85;
/// The lowest and highest pitch the figure's height spans.
pub const FIGURE_HZ: (f32, f32) = (100.0, 8_000.0);

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `0..=1`: small and high to large and low.
    pub size: f32,
    /// `0..=1`: one drop to six, spread in time and stereo.
    pub splash: f32,
    /// `0..=1`: how much of the drops is added to the input.
    pub mix: f32,
    /// Settle on the input's harmonics rather than where Size puts them.
    pub follow: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Drop {
    /// Samples until it starts to sound.
    wait: u32,
    phase: f32,
    hz: f32,
    settle_hz: f32,
    rise: f32,
    amplitude: f32,
    decay: f32,
    left: f32,
    right: f32,
    pan: f32,
    age: u32,
}

impl Drop {
    fn sounding(&self) -> bool {
        self.wait == 0 && self.amplitude > 0.0
    }
}

pub struct Dew {
    sample_rate: f32,
    drops: [Drop; DROPS],
    fast: f32,
    slow: f32,
    fast_coefficient: f32,
    slow_coefficient: f32,
    holdoff: u32,
    pitch: Pitch,
    random: u32,
}

impl Dew {
    pub fn new(sample_rate: f32) -> Self {
        let coefficient = |ms: f32| 1.0 - (-1.0 / (ms * 0.001 * sample_rate)).exp();
        Self {
            sample_rate,
            drops: [Drop::default(); DROPS],
            fast: 0.0,
            slow: 0.0,
            fast_coefficient: coefficient(FAST_MS),
            slow_coefficient: coefficient(SLOW_MS),
            holdoff: 0,
            pitch: Pitch::new(sample_rate),
            random: 0x2545_F491,
        }
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let mono = 0.5 * (input[0] + input[1]);
        let mono = if mono.is_finite() { mono } else { 0.0 };
        self.pitch.push(mono);

        let power = mono * mono;
        self.fast += self.fast_coefficient * (power - self.fast);
        self.slow += self.slow_coefficient * (power - self.slow);
        if self.holdoff > 0 {
            self.holdoff -= 1;
        } else if self.fast > FLOOR && self.fast > ATTACK_RATIO * self.slow {
            self.splash(self.fast.sqrt().min(1.0), settings);
            self.holdoff = self.samples(HOLDOFF_MS);
        }

        let mut wet = [0.0; 2];
        for drop in &mut self.drops {
            if drop.wait > 0 {
                drop.wait -= 1;
                continue;
            }
            if drop.amplitude <= 0.0 {
                continue;
            }
            drop.hz += drop.rise * (drop.settle_hz - drop.hz);
            drop.phase = (drop.phase + drop.hz / self.sample_rate).fract();
            let sample = (TAU * drop.phase).sin() * drop.amplitude;
            wet[0] += sample * drop.left;
            wet[1] += sample * drop.right;
            drop.amplitude *= drop.decay;
            drop.age += 1;
            if drop.amplitude < 1e-5 {
                drop.amplitude = 0.0;
            }
        }
        let mix = settings.mix.clamp(0.0, 1.0);
        [input[0] + mix * wet[0], input[1] + mix * wet[1]]
    }

    /// Every slot as `[pan, height, level, age]`: pan `-1..=1`, height `0..=1`
    /// up the figure's pitch span, level the drop's amplitude, age in seconds.
    /// A slot not sounding reads level 0.
    pub fn drops(&self, out: &mut [f32; DROPS * READING]) {
        let (low, high) = (FIGURE_HZ.0.ln(), FIGURE_HZ.1.ln());
        for (drop, reading) in self.drops.iter().zip(out.chunks_exact_mut(READING)) {
            if drop.sounding() {
                reading[0] = drop.pan;
                reading[1] = ((drop.hz.max(1.0).ln() - low) / (high - low)).clamp(0.0, 1.0);
                reading[2] = drop.amplitude;
                reading[3] = drop.age as f32 / self.sample_rate;
            } else {
                reading.fill(0.0);
            }
        }
    }

    fn samples(&self, ms: f32) -> u32 {
        (ms * 0.001 * self.sample_rate) as u32
    }

    /// Lets the drops of one attack fall.
    fn splash(&mut self, level: f32, settings: &Settings) {
        let size = settings.size.clamp(0.0, 1.0);
        let splash = settings.splash.clamp(0.0, 1.0);
        let count = 1 + (splash * 5.0).round() as usize;
        let followed = (settings.follow && self.pitch.clarity() > CLEAR).then(|| self.pitch.hz());
        let rise = 1.0 - (-1.0 / (self.lerp(RISE_MS, size) * 0.001 * self.sample_rate)).exp();
        let decay = (-1.0 / (self.lerp(DECAY_MS, size) * 0.001 * self.sample_rate)).exp();
        let jitter = self.lerp(JITTER, splash);
        for index in 0..count {
            let wander = jitter * (2.0 * self.next() - 1.0) / 12.0;
            let wanted = SMALL_HZ * (-SIZE_OCTAVES * size + wander).exp2();
            let settle_hz = settle(wanted, followed).min(0.45 * self.sample_rate);
            let pan = splash * (2.0 * self.next() - 1.0);
            let angle = (pan + 1.0) * FRAC_PI_4;
            let wait = if index == 0 {
                0
            } else {
                (self.next() * SPREAD_MS * splash * 0.001 * self.sample_rate) as u32
            };
            // Later drops of a splash fall quieter.
            let amplitude = level * (0.5 + 0.5 * self.next()) / (1.0 + index as f32 * 0.5);
            let slot = self.quietest();
            self.drops[slot] = Drop {
                wait,
                phase: 0.0,
                hz: settle_hz / (1.0 + RISE),
                settle_hz,
                rise,
                amplitude,
                decay,
                left: angle.cos(),
                right: angle.sin(),
                pan,
                age: 0,
            };
        }
    }

    fn quietest(&self) -> usize {
        (0..DROPS)
            .min_by(|a, b| {
                let loud = |drop: &Drop| {
                    if drop.wait > 0 {
                        f32::MAX
                    } else {
                        drop.amplitude
                    }
                };
                loud(&self.drops[*a]).total_cmp(&loud(&self.drops[*b]))
            })
            .unwrap_or(0)
    }

    fn lerp(&self, (from, to): (f32, f32), t: f32) -> f32 {
        from + (to - from) * t
    }

    /// `0..1`, deterministic: the same input gives the same drops.
    fn next(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        self.random as f32 / u32::MAX as f32
    }
}

/// Where a drop that wants `wanted` Hz settles: there, or on the harmonic of
/// `followed` nearest to it.
pub fn settle(wanted: f32, followed: Option<f32>) -> f32 {
    match followed {
        Some(hz) if hz > 0.0 => hz * (wanted / hz).round().max(1.0),
        _ => wanted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        size: 0.5,
        splash: 0.5,
        mix: 1.0,
        follow: false,
    };

    /// Runs `signal` through, returning the added part (output minus input).
    fn added(settings: &Settings, seconds: f32, signal: impl Fn(usize) -> f32) -> Vec<f32> {
        let mut dew = Dew::new(RATE);
        (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                dew.process([x, x], settings)[0] - x
            })
            .collect()
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|x| x * x).sum()
    }

    /// A short burst every 200 ms.
    fn clicks(index: usize) -> f32 {
        let at = index % (RATE as usize / 5);
        if at < 48 {
            (TAU * 1_000.0 * at as f32 / RATE).sin() * 0.5
        } else {
            0.0
        }
    }

    #[test]
    fn silence_stays_silent() {
        assert_eq!(energy(&added(&SETTINGS, 0.5, |_| 0.0)), 0.0);
    }

    #[test]
    fn an_attack_lets_drops_fall() {
        let wet = added(&SETTINGS, 1.0, clicks);
        assert!(energy(&wet) > 1.0, "{}", energy(&wet));
    }

    #[test]
    fn no_mix_is_the_input() {
        let dry = Settings {
            mix: 0.0,
            ..SETTINGS
        };
        assert_eq!(energy(&added(&dry, 1.0, clicks)), 0.0);
    }

    #[test]
    fn a_held_note_lets_nothing_more_fall() {
        // The onset is an attack; after the drops ring out a steady sine is not.
        let wet = added(&SETTINGS, 1.5, |i| {
            (TAU * 220.0 * i as f32 / RATE).sin() * 0.5
        });
        let tail = &wet[RATE as usize..];
        assert!(energy(tail) < 1e-6, "{}", energy(tail));
    }

    #[test]
    fn a_bigger_drop_rings_lower() {
        let small = settle(SMALL_HZ * (-SIZE_OCTAVES * 0.0f32).exp2(), None);
        let large = settle(SMALL_HZ * (-SIZE_OCTAVES * 1.0f32).exp2(), None);
        assert!(large < small);
        assert!((large - 500.0).abs() < 1.0);
    }

    #[test]
    fn follow_settles_on_a_harmonic() {
        let hz = settle(1_000.0, Some(220.0));
        assert!((hz / 220.0 - (hz / 220.0).round()).abs() < 1e-4);
        assert!((hz - 1_100.0).abs() < 1e-3);
        // Under the fundamental it settles on the fundamental.
        assert_eq!(settle(100.0, Some(220.0)), 220.0);
        // No clear pitch, no change.
        assert_eq!(settle(1_000.0, None), 1_000.0);
    }

    #[test]
    fn the_same_input_gives_the_same_drops() {
        assert_eq!(added(&SETTINGS, 0.5, clicks), added(&SETTINGS, 0.5, clicks));
    }

    #[test]
    fn the_drops_stay_bounded() {
        let wild = Settings {
            size: 0.0,
            splash: 1.0,
            mix: 1.0,
            follow: false,
        };
        let wet = added(&wild, 2.0, |i| if i % 2_400 < 4 { 1.0 } else { 0.0 });
        let peak = wet.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
        assert!(peak.is_finite() && peak < 4.0, "{peak}");
    }

    #[test]
    fn the_readings_show_what_sounds() {
        let mut dew = Dew::new(RATE);
        let mut readings = [0.0; DROPS * READING];
        for index in 0..(RATE as usize / 100) {
            let x = clicks(index);
            dew.process([x, x], &SETTINGS);
        }
        dew.drops(&mut readings);
        assert!(readings.chunks_exact(READING).any(|r| r[2] > 0.0));
        assert!(
            readings
                .chunks_exact(READING)
                .all(|r| (0.0..=1.0).contains(&r[1]))
        );
    }
}
