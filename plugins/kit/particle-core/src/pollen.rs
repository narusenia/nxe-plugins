//! Pollen: the input broken into fine grains and scattered just behind it
//! (`particle-kit-plan.md`).
//!
//! The input is kept for half a second. Grains are cut from it, each a
//! Hann-windowed slice read a little in the past, so a grain says what the
//! input said, at its pitch. **Density** is how many grains a second (5 to
//! 80), **Size** how long each is (20 to 80 ms), **Scatter** how far behind
//! (up to 250 ms) and how far to the sides they land. Each grain is read a few
//! cents off (±6), so the overlapping grains shimmer a little instead of
//! comb-filtering into one delayed copy.
//!
//! A grain's level is divided by the square root of how many overlap, so the
//! cloud is about as loud as the input whatever the Density. It is added to
//! the dry signal, `Wet` of it: at 0 the output is the input. The dry signal
//! is not delayed, so there is no latency to report.
//!
//! The buffer is allocated in `new`; nothing is allocated after.

use std::f32::consts::{FRAC_PI_4, TAU};

/// The most grains sounding at once. A grain due with none free waits for
/// the next.
pub const GRAINS: usize = 48;
/// What [`Pollen::grains`] writes per grain: pan, behind, level, progress.
pub const READING: usize = 4;
/// Grains a second, from no Density to full.
const RATE_HZ: (f32, f32) = (5.0, 80.0);
/// A grain's length, from no Size to full.
const SIZE_MS: (f32, f32) = (20.0, 80.0);
/// How far behind the input a grain may start, at full Scatter, and at least.
pub const BEHIND_MS: f32 = 250.0;
const LEAST_MS: f32 = 5.0;
/// How far off its pitch a grain is read, either way.
const DRIFT_CENTS: f32 = 6.0;
/// How far to the sides grains land at full Scatter, `0..=1` of the way.
const WIDTH: f32 = 0.9;
/// The input kept, in seconds: the farthest a grain starts behind, plus its
/// length, plus room.
const KEPT_S: f32 = 0.5;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `0..=1` each.
    pub density: f32,
    pub size: f32,
    pub scatter: f32,
    pub mix: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Grain {
    /// Where in the kept input it reads, as a position that may fall between
    /// samples.
    read: f32,
    /// How far it moves per sample: `1` is the input's own pitch.
    step: f32,
    length: f32,
    age: f32,
    gain: f32,
    pan: f32,
    left: f32,
    right: f32,
    /// How far behind it started, `0..=1` of [`BEHIND_MS`].
    behind: f32,
    sounding: bool,
}

pub struct Pollen {
    sample_rate: f32,
    kept: Vec<f32>,
    write: usize,
    grains: [Grain; GRAINS],
    /// Samples to the next grain.
    countdown: f32,
    random: u32,
}

impl Pollen {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            kept: vec![0.0; (KEPT_S * sample_rate) as usize + 4],
            write: 0,
            grains: [Grain::default(); GRAINS],
            countdown: 0.0,
            random: 0x1B87_3593,
        }
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let mono = 0.5 * (input[0] + input[1]);
        let mono = if mono.is_finite() { mono } else { 0.0 };
        self.kept[self.write] = mono;
        self.write = (self.write + 1) % self.kept.len();

        self.countdown -= 1.0;
        if self.countdown <= 0.0 {
            let rate = lerp_log(RATE_HZ, settings.density);
            // Never quite even, or the grains would buzz at their rate.
            self.countdown += self.sample_rate / rate * (0.5 + self.next());
            self.sow(settings, rate);
        }

        let mut wet = [0.0; 2];
        let length = self.kept.len() as f32;
        for grain in &mut self.grains {
            if !grain.sounding {
                continue;
            }
            let window = 0.5 - 0.5 * (TAU * grain.age / grain.length).cos();
            let sample = read(&self.kept, grain.read) * window * grain.gain;
            wet[0] += sample * grain.left;
            wet[1] += sample * grain.right;
            grain.read = (grain.read + grain.step) % length;
            grain.age += 1.0;
            if grain.age >= grain.length {
                grain.sounding = false;
            }
        }
        let mix = settings.mix.clamp(0.0, 1.0);
        [input[0] + mix * wet[0], input[1] + mix * wet[1]]
    }

    /// Every slot as `[pan, behind, level, progress]`: pan `-1..=1`, behind
    /// `0..=1` of [`BEHIND_MS`], level its window now, progress `0..=1`
    /// through it. A slot not sounding reads level 0.
    pub fn grains(&self, out: &mut [f32; GRAINS * READING]) {
        for (grain, reading) in self.grains.iter().zip(out.chunks_exact_mut(READING)) {
            let progress = if grain.length > 0.0 {
                grain.age / grain.length
            } else {
                0.0
            };
            reading[0] = grain.pan;
            reading[1] = grain.behind;
            reading[2] = if grain.sounding {
                0.5 - 0.5 * (TAU * progress).cos()
            } else {
                0.0
            };
            reading[3] = progress.clamp(0.0, 1.0);
        }
    }

    /// Starts one grain, if a slot is free.
    fn sow(&mut self, settings: &Settings, rate: f32) {
        let Some(slot) = self.grains.iter().position(|grain| !grain.sounding) else {
            return;
        };
        let scatter = settings.scatter.clamp(0.0, 1.0);
        let length = lerp_log(SIZE_MS, settings.size) * 0.001 * self.sample_rate;
        let behind = scatter * self.next();
        let delay = (LEAST_MS + BEHIND_MS * behind) * 0.001 * self.sample_rate;
        let cents = (2.0 * self.next() - 1.0) * DRIFT_CENTS;
        let step = (cents / 1_200.0).exp2();
        // A grain read faster than the input catches up with the write
        // position by `length * (step - 1)`: start it at least that far back.
        let delay = delay.max(length * (step - 1.0) + 2.0);
        let kept = self.kept.len() as f32;
        let read = (self.write as f32 - delay).rem_euclid(kept);
        // How many grains overlap on average; the cloud's power is their sum.
        let overlap = (rate * length / self.sample_rate).max(1.0);
        let pan = (2.0 * self.next() - 1.0) * WIDTH * scatter;
        let angle = (pan + 1.0) * FRAC_PI_4;
        self.grains[slot] = Grain {
            read,
            step,
            length,
            age: 0.0,
            // A Hann window's power is 3/8 of a rectangle's.
            gain: (1.0 / (overlap * 0.375)).sqrt() * std::f32::consts::FRAC_1_SQRT_2,
            pan,
            left: angle.cos() * std::f32::consts::SQRT_2,
            right: angle.sin() * std::f32::consts::SQRT_2,
            behind,
            sounding: true,
        };
    }

    /// `0..1`, the same sequence every run.
    fn next(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        self.random as f32 / u32::MAX as f32
    }
}

/// From `range.0` at 0 to `range.1` at 1, evenly in ratio.
fn lerp_log(range: (f32, f32), t: f32) -> f32 {
    range.0 * (range.1 / range.0).powf(t.clamp(0.0, 1.0))
}

/// The kept input at a position between samples, linearly.
fn read(kept: &[f32], at: f32) -> f32 {
    let index = at as usize % kept.len();
    let next = (index + 1) % kept.len();
    let fraction = at.fract();
    kept[index] + (kept[next] - kept[index]) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        density: 0.5,
        size: 0.5,
        scatter: 0.5,
        mix: 1.0,
    };

    fn run(
        settings: &Settings,
        seconds: f32,
        signal: impl Fn(usize) -> f32,
    ) -> (Pollen, Vec<[f32; 2]>) {
        let mut pollen = Pollen::new(RATE);
        let wet = (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                let out = pollen.process([x, x], settings);
                [out[0] - x, out[1] - x]
            })
            .collect();
        (pollen, wet)
    }

    fn rms(samples: impl Iterator<Item = f32>) -> f32 {
        let (sum, count) = samples.fold((0.0, 0), |(sum, count), x| (sum + x * x, count + 1));
        (sum / count.max(1) as f32).sqrt()
    }

    fn sine(i: usize) -> f32 {
        (TAU * 440.0 * i as f32 / RATE).sin() * 0.3
    }

    #[test]
    fn silence_stays_silent() {
        let (_, wet) = run(&SETTINGS, 0.5, |_| 0.0);
        assert!(wet.iter().all(|s| s[0] == 0.0 && s[1] == 0.0));
    }

    #[test]
    fn no_mix_is_the_input() {
        let (_, wet) = run(
            &Settings {
                mix: 0.0,
                ..SETTINGS
            },
            0.5,
            sine,
        );
        assert!(wet.iter().all(|s| s[0] == 0.0 && s[1] == 0.0));
    }

    #[test]
    fn the_cloud_is_about_as_loud_as_the_input_at_any_density() {
        let input = 0.3 / 2f32.sqrt();
        for density in [0.0, 0.5, 1.0] {
            let (_, wet) = run(
                &Settings {
                    density,
                    ..SETTINGS
                },
                2.0,
                sine,
            );
            let level = rms(wet[48_000..].iter().map(|s| 0.5 * (s[0] + s[1])));
            assert!(
                level > input * 0.3 && level < input * 2.0,
                "{density}: {level} vs {input}"
            );
        }
    }

    #[test]
    fn the_grains_keep_the_pitch() {
        let (_, wet) = run(
            &Settings {
                density: 1.0,
                ..SETTINGS
            },
            1.5,
            sine,
        );
        let left: Vec<f32> = wet[24_000..].iter().map(|s| s[0]).collect();
        let changes = left
            .windows(2)
            .filter(|pair| (pair[0] > 0.0) != (pair[1] > 0.0))
            .count();
        let hz = changes as f32 * RATE / left.len() as f32 / 2.0;
        assert!((hz / 440.0 - 1.0).abs() < 0.03, "{hz} Hz");
    }

    #[test]
    fn no_scatter_lands_in_the_middle() {
        let (_, wet) = run(
            &Settings {
                scatter: 0.0,
                ..SETTINGS
            },
            0.5,
            sine,
        );
        assert!(wet.iter().all(|s| (s[0] - s[1]).abs() < 1e-6));
    }

    #[test]
    fn scatter_spreads_to_the_sides() {
        let (_, wet) = run(
            &Settings {
                scatter: 1.0,
                ..SETTINGS
            },
            1.0,
            sine,
        );
        let side = rms(wet.iter().map(|s| s[0] - s[1]));
        assert!(side > 0.01, "{side}");
    }

    #[test]
    fn a_burst_comes_back_behind_the_input() {
        // A short burst, then silence: with full Scatter some grains are
        // still reading it well after it ended.
        let burst = |i: usize| if i < 2_400 { sine(i) } else { 0.0 };
        let (_, wet) = run(
            &Settings {
                scatter: 1.0,
                ..SETTINGS
            },
            0.5,
            burst,
        );
        let late = rms(wet[9_600..14_400].iter().map(|s| s[0]));
        assert!(late > 0.005, "{late}");
    }

    #[test]
    fn the_same_input_gives_the_same_cloud() {
        assert_eq!(run(&SETTINGS, 0.3, sine).1, run(&SETTINGS, 0.3, sine).1);
    }

    #[test]
    fn it_stays_bounded() {
        let wild = Settings {
            density: 1.0,
            size: 1.0,
            scatter: 0.0,
            mix: 1.0,
        };
        let (_, wet) = run(&wild, 1.0, |i| if i % 2 == 0 { 1.0 } else { -1.0 });
        let peak = wet
            .iter()
            .fold(0.0f32, |p, s| p.max(s[0].abs()).max(s[1].abs()));
        assert!(peak.is_finite() && peak < 8.0, "{peak}");
    }

    #[test]
    fn the_readings_show_what_sounds() {
        // Dense enough that some grain is always sounding.
        let (pollen, _) = run(
            &Settings {
                density: 1.0,
                ..SETTINGS
            },
            0.3,
            sine,
        );
        let mut grains = [0.0; GRAINS * READING];
        pollen.grains(&mut grains);
        assert!(grains.chunks_exact(READING).any(|r| r[2] > 0.0));
        assert!(
            grains
                .chunks_exact(READING)
                .all(|r| (0.0..=1.0).contains(&r[1]) && (0.0..=1.0).contains(&r[3]))
        );
    }
}
