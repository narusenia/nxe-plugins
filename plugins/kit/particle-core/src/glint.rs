//! Glint: crystal sparks high above the input, on its attacks
//! (`particle-kit-plan.md`).
//!
//! An attack (`nxe_audio::onset`) lets [`PER_ATTACK`] sparks fly. A spark is a
//! short sine with a soft start and an exponential ring, near a height set by
//! **Height** (3 to 12 kHz). With a clear pitch in the input a spark sits on
//! one of its harmonics, so the sparks are in tune with the note; with none
//! they take a bell's inharmonic ratios around the height, which is what makes
//! a struck glass sound like glass. **A spark keeps settling while it rings**:
//! the tracker needs a few tens of milliseconds on a new note, so a spark
//! thrown on the attack starts on the bell and slides onto the note's harmonic
//! once the note is heard, as Dew's drops do. **Decay** is how long they ring (T60 30 ms
//! to 1.2 s; a higher spark rings a little shorter). **Sensitivity** is how
//! sharp a rise counts as an attack.
//!
//! A spark's level is the input's recent peak when it starts to sound, so it
//! follows how hard the note was hit. The sparks are added to the dry signal,
//! `Wet` of them: at 0 the output is the input. Nothing is delayed.
//!
//! Allocation-free once built.

use nxe_audio::onset::Onset;
use nxe_audio::pitch::Pitch;
use std::f32::consts::{FRAC_PI_4, TAU};

/// The most sparks sounding at once. A new one takes the quietest slot.
pub const SPARKS: usize = 16;
/// What [`Glint::sparks`] writes per spark: pan, height, level, age.
pub const READING: usize = 4;
/// Sparks per attack.
pub const PER_ATTACK: usize = 5;
/// The height a spark flies to, from no Height to full.
const HEIGHT_HZ: (f32, f32) = (3_000.0, 12_000.0);
/// How far either side of the height one attack's sparks may land together, in
/// octaves: the bell moves as a whole, so it stays a bell.
const SCATTER_OCTAVES: f32 = 0.3;
/// A bell's partials over its lowest, for a spark with no pitch to follow.
const BELL: [f32; PER_ATTACK] = [1.0, 1.19, 1.47, 1.66, 2.0];
/// T60 from no Decay to full.
const DECAY_MS: (f32, f32) = (30.0, 1_200.0);
/// How far apart in time one attack's sparks fly, and how long the first waits.
const SPREAD_MS: f32 = 25.0;
const FIRST_MS: f32 = 3.0;
/// A spark's soft start, so it does not click.
const ATTACK_MS: f32 = 0.5;
/// One attack's sparks together against the input's recent peak.
const GAIN: f32 = 0.6;
/// How fast the input's peak falls back.
const PEAK_RELEASE_MS: f32 = 300.0;
/// How sure the pitch tracker must be before a spark follows it.
const CLEAR: f32 = 0.85;
/// The wet signal's ceiling, softly.
const CEILING: f32 = 1.5;
/// How fast a spark slides onto a harmonic, as a time constant.
const SETTLE_MS: f32 = 4.0;
/// The pitch span the figure's height covers.
pub const FIGURE_HZ: (f32, f32) = (1_500.0, 20_000.0);

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `0..=1` each.
    pub sensitivity: f32,
    pub height: f32,
    pub decay: f32,
    pub mix: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Spark {
    hz: f32,
    /// Where it would land with no note to follow.
    wanted: f32,
    phase: f32,
    /// Samples before it sounds.
    wait: u32,
    /// Its share of the input's peak, taken when it starts to sound.
    gain: f32,
    amplitude: f32,
    decay: f32,
    age: u32,
    pan: f32,
    left: f32,
    right: f32,
}

pub struct Glint {
    sample_rate: f32,
    sparks: [Spark; SPARKS],
    onset: Onset,
    peak: f32,
    peak_release: f32,
    pitch: Pitch,
    settle: f32,
    random: u32,
}

impl Glint {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            sparks: [Spark::default(); SPARKS],
            onset: Onset::new(sample_rate),
            peak: 0.0,
            peak_release: (-1.0 / (PEAK_RELEASE_MS * 0.001 * sample_rate)).exp(),
            pitch: Pitch::new(sample_rate),
            settle: 1.0 - (-1.0 / (SETTLE_MS * 0.001 * sample_rate)).exp(),
            random: 0x6C8E_9CF5,
        }
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let mono = 0.5 * (input[0] + input[1]);
        let mono = if mono.is_finite() { mono } else { 0.0 };
        self.pitch.push(mono);
        self.peak = (self.peak * self.peak_release).max(mono.abs());
        if self.onset.push(mono, settings.sensitivity) {
            self.fly(settings);
        }

        let attack = ATTACK_MS * 0.001 * self.sample_rate;
        let peak = self.peak.min(1.0);
        let followed = (self.pitch.clarity() > CLEAR).then(|| self.pitch.hz());
        let ceiling = 0.45 * self.sample_rate;
        let mut wet = [0.0; 2];
        for spark in &mut self.sparks {
            if spark.wait > 0 {
                spark.wait -= 1;
                if spark.wait == 0 {
                    spark.amplitude = spark.gain * peak;
                }
                continue;
            }
            if spark.amplitude <= 0.0 {
                continue;
            }
            if let Some(f0) = followed {
                let target = harmonic(spark.wanted, f0).min(ceiling);
                spark.hz += self.settle * (target - spark.hz);
            }
            spark.phase = (spark.phase + spark.hz / self.sample_rate).fract();
            let start = (spark.age as f32 / attack).min(1.0);
            let sample = (TAU * spark.phase).sin() * spark.amplitude * start;
            wet[0] += sample * spark.left;
            wet[1] += sample * spark.right;
            spark.amplitude *= spark.decay;
            spark.age += 1;
            if spark.amplitude < 1e-5 {
                spark.amplitude = 0.0;
            }
        }
        let mix = settings.mix.clamp(0.0, 1.0);
        let soft = |x: f32| CEILING * (x / CEILING).tanh();
        [input[0] + mix * soft(wet[0]), input[1] + mix * soft(wet[1])]
    }

    /// Every slot as `[pan, height, level, age]`: pan `-1..=1`, height `0..=1`
    /// up [`FIGURE_HZ`], level the spark's amplitude, age in seconds. A slot
    /// not sounding reads level 0.
    pub fn sparks(&self, out: &mut [f32; SPARKS * READING]) {
        let span = (FIGURE_HZ.1 / FIGURE_HZ.0).ln();
        for (spark, reading) in self.sparks.iter().zip(out.chunks_exact_mut(READING)) {
            let sounding = spark.wait == 0 && spark.amplitude > 0.0;
            reading[0] = spark.pan;
            reading[1] = ((spark.hz.max(FIGURE_HZ.0) / FIGURE_HZ.0).ln() / span).clamp(0.0, 1.0);
            reading[2] = if sounding { spark.amplitude } else { 0.0 };
            reading[3] = spark.age as f32 / self.sample_rate;
        }
    }

    /// Lets one attack's sparks fly.
    fn fly(&mut self, settings: &Settings) {
        let height = settings.height.clamp(0.0, 1.0);
        let centre = HEIGHT_HZ.0 * (HEIGHT_HZ.1 / HEIGHT_HZ.0).powf(height);
        let t60 = DECAY_MS.0 * (DECAY_MS.1 / DECAY_MS.0).powf(settings.decay.clamp(0.0, 1.0));
        let followed = (self.pitch.clarity() > CLEAR).then(|| self.pitch.hz());
        let ceiling = 0.45 * self.sample_rate;
        let shift = ((2.0 * self.next() - 1.0) * SCATTER_OCTAVES).exp2();
        for (index, partial) in BELL.iter().enumerate() {
            let wanted = centre * shift * partial / BELL[PER_ATTACK - 1].sqrt();
            let hz = followed
                .map_or(wanted, |f0| harmonic(wanted, f0))
                .min(ceiling);
            // A higher spark rings a little shorter, as a struck bar's do.
            let ring = t60 * (0.7 + 0.6 * self.next()) * (centre / hz).sqrt();
            let wait = FIRST_MS + SPREAD_MS * index as f32 / PER_ATTACK as f32 * self.next();
            let pan = (2.0 * self.next() - 1.0) * 0.8;
            let angle = (pan + 1.0) * FRAC_PI_4;
            let gain = GAIN / (PER_ATTACK as f32).sqrt() * (0.6 + 0.4 * self.next());
            let slot = self.quietest();
            self.sparks[slot] = Spark {
                hz,
                wanted,
                phase: 0.0,
                wait: ((wait * 0.001 * self.sample_rate) as u32).max(1),
                gain,
                amplitude: 0.0,
                decay: (-6.91 / (ring * 0.001 * self.sample_rate)).exp(),
                age: 0,
                pan,
                left: angle.cos(),
                right: angle.sin(),
            };
        }
    }

    /// The slot to reuse: a silent one, or else the quietest.
    fn quietest(&self) -> usize {
        self.sparks
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                let level = |s: &Spark| if s.wait > 0 { f32::MAX } else { s.amplitude };
                level(a).total_cmp(&level(b))
            })
            .map_or(0, |(index, _)| index)
    }

    /// `0..1`, the same sequence every run.
    fn next(&mut self) -> f32 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 17;
        self.random ^= self.random << 5;
        self.random as f32 / u32::MAX as f32
    }
}

/// The harmonic of `f0` nearest to `hz`.
fn harmonic(hz: f32, f0: f32) -> f32 {
    f0 * (hz / f0).round().max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        sensitivity: 0.5,
        height: 0.5,
        decay: 0.4,
        mix: 1.0,
    };

    fn run(settings: &Settings, seconds: f32, signal: impl Fn(usize) -> f32) -> (Glint, Vec<f32>) {
        let mut glint = Glint::new(RATE);
        let wet = (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                glint.process([x, x], settings)[0] - x
            })
            .collect();
        (glint, wet)
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|x| x * x).sum()
    }

    /// Sign changes per second over two: the rough pitch of what is there.
    fn brightness(samples: &[f32]) -> f32 {
        let changes = samples
            .windows(2)
            .filter(|pair| (pair[0] > 0.0) != (pair[1] > 0.0))
            .count();
        changes as f32 * RATE / samples.len() as f32 / 2.0
    }

    /// A pluck every half second: a click, then a falling 440 Hz.
    fn plucks(i: usize) -> f32 {
        let t = (i % 24_000) as f32 / RATE;
        (TAU * 440.0 * t).sin() * 0.5 * (-t * 8.0).exp()
    }

    #[test]
    fn silence_stays_silent() {
        assert_eq!(energy(&run(&SETTINGS, 0.5, |_| 0.0).1), 0.0);
    }

    #[test]
    fn no_mix_is_the_input() {
        let dry = Settings {
            mix: 0.0,
            ..SETTINGS
        };
        assert_eq!(energy(&run(&dry, 1.0, plucks).1), 0.0);
    }

    #[test]
    fn an_attack_throws_high_sparks() {
        let (_, wet) = run(&SETTINGS, 0.4, plucks);
        assert!(energy(&wet) > 1e-3, "{}", energy(&wet));
        let bright = brightness(&wet[..4_800]);
        assert!(bright > 2_000.0, "{bright} Hz");
    }

    #[test]
    fn a_held_tone_throws_nothing_more() {
        let tone = |i: usize| (TAU * 440.0 * i as f32 / RATE).sin() * 0.3;
        let (_, wet) = run(
            &Settings {
                decay: 0.0,
                ..SETTINGS
            },
            1.0,
            tone,
        );
        assert_eq!(energy(&wet[24_000..]), 0.0);
    }

    #[test]
    fn height_moves_the_sparks_up() {
        let at = |height| {
            let (_, wet) = run(&Settings { height, ..SETTINGS }, 0.2, plucks);
            brightness(&wet[..4_800])
        };
        assert!(at(1.0) > at(0.0) * 1.8, "{} {}", at(1.0), at(0.0));
    }

    #[test]
    fn decay_is_how_long_they_ring() {
        let tail = |decay| {
            let (_, wet) = run(&Settings { decay, ..SETTINGS }, 0.4, |i| {
                if i < 24_000 { plucks(i) } else { 0.0 }
            });
            energy(&wet[9_600..19_200])
        };
        assert!(tail(1.0) > tail(0.0) * 100.0, "{} {}", tail(1.0), tail(0.0));
    }

    #[test]
    fn with_a_clear_note_the_sparks_are_its_harmonics() {
        // Long enough for the tracker to hear the note: every spark sounding
        // has slid onto a harmonic of it.
        let (glint, _) = run(
            &Settings {
                decay: 1.0,
                ..SETTINGS
            },
            0.7,
            plucks,
        );
        let followed = glint.sparks.iter().filter(|s| s.amplitude > 0.0).all(|s| {
            let harmonic = s.hz / 440.0;
            (harmonic - harmonic.round()).abs() < 0.02
        });
        assert!(
            followed,
            "{:?}",
            glint.sparks.iter().map(|s| s.hz).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_same_input_throws_the_same_sparks() {
        assert_eq!(run(&SETTINGS, 0.5, plucks).1, run(&SETTINGS, 0.5, plucks).1);
    }

    #[test]
    fn they_stay_bounded() {
        let wild = Settings {
            sensitivity: 1.0,
            decay: 1.0,
            ..SETTINGS
        };
        let (_, wet) = run(&wild, 2.0, |i| if i % 960 < 4 { 1.0 } else { 0.0 });
        let peak = wet.iter().fold(0.0f32, |p, x| p.max(x.abs()));
        assert!(peak.is_finite() && peak <= CEILING, "{peak}");
    }

    #[test]
    fn the_readings_show_what_sounds() {
        let (glint, _) = run(&SETTINGS, 0.05, plucks);
        let mut sparks = [0.0; SPARKS * READING];
        glint.sparks(&mut sparks);
        let sounding = sparks.chunks_exact(READING).filter(|r| r[2] > 0.0).count();
        assert!(sounding > 0);
        assert!(
            sparks
                .chunks_exact(READING)
                .all(|r| (0.0..=1.0).contains(&r[1]))
        );
    }
}
