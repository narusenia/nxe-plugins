//! Halo: a ring of tuned resonance around the input (`resonance-kit-plan.md`).
//!
//! A bank of resonators, one per note of a chord built on a root, rung by the
//! input. The root is a pitch you set (**Free**) or the input's own pitch
//! (**Follow**; with no clear pitch the last one holds). The chord is the
//! root's octaves and fifth, with a major or minor third if asked for, over
//! two octaves, in just intonation.
//!
//! **Decay** is how long a resonator rings (T60, 0.3 to 8 s). **Shimmer**
//! sways each note slowly off its pitch, each at its own rate. **Tone** tilts
//! the upper notes from dark to bright. The resonance is added to the dry
//! signal, `Mix` of it: at 0 the output is the input, and nothing is delayed.
//!
//! Each resonator is a complex one-pole (`y = r·e^{iω}·y + (1 − r)·x`): its
//! gain at its own pitch is one, so a sung note on the root rings back at the
//! level it came in, and its pitch can glide without the clicks a biquad's
//! recomputed coefficients make. The coefficients are refreshed every
//! [`CONTROL`] samples.
//!
//! Allocation-free once built.

use nxe_audio::pitch::Pitch;
use std::f32::consts::{FRAC_PI_4, TAU};

/// The most notes a chord has.
pub const VOICES: usize = 7;
/// What [`Halo::rings`] writes per voice: height, level.
pub const READING: usize = 2;
/// Samples between coefficient updates.
const CONTROL: usize = 32;

/// The chords, as ratios over the root, low to high, in just intonation.
const FIFTH: &[f32] = &[1.0, 1.5, 2.0, 3.0, 4.0];
const MAJOR: &[f32] = &[1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0];
const MINOR: &[f32] = &[1.0, 1.2, 1.5, 2.0, 2.4, 3.0, 4.0];

/// T60 from no Decay to full, in seconds.
const DECAY_S: (f32, f32) = (0.3, 8.0);
/// How far Shimmer sways a note at full, in cents, and how fast the slowest
/// and quickest sway.
const SHIMMER_CENTS: f32 = 18.0;
const SHIMMER_HZ: (f32, f32) = (0.15, 0.6);
/// Tone's tilt: a note's weight is `ratio^-tilt`, from this at no Tone to 0.
const DARK_TILT: f32 = 1.6;
/// The root's glide in Follow.
const GLIDE_MS: f32 = 40.0;
/// How sure the pitch tracker must be before Follow moves the root.
const CLEAR: f32 = 0.85;
/// The root's range, and the figure's span above it (two octaves).
pub const ROOT_HZ: (f32, f32) = (40.0, 1_000.0);
pub const SPAN_OCTAVES: f32 = 2.0;
/// The wet signal's ceiling, softly.
const CEILING: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chord {
    Fifth,
    Major,
    Minor,
}

impl Chord {
    /// The chord's notes as ratios over the root, low to high.
    pub fn ratios(self) -> &'static [f32] {
        match self {
            Self::Fifth => FIFTH,
            Self::Major => MAJOR,
            Self::Minor => MINOR,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// The root in Free, in Hz.
    pub root: f32,
    pub follow: bool,
    pub chord: Chord,
    /// `0..=1` each.
    pub decay: f32,
    pub shimmer: f32,
    pub tone: f32,
    pub mix: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Voice {
    re: f32,
    im: f32,
    /// `r·cos ω`, `r·sin ω`, `1 − r`.
    a: f32,
    b: f32,
    gain: f32,
    weight: f32,
    left: f32,
    right: f32,
    sway: f32,
    sway_step: f32,
    level: f32,
}

pub struct Halo {
    sample_rate: f32,
    voices: [Voice; VOICES],
    chord: Chord,
    count: usize,
    countdown: usize,
    /// The root in use, gliding.
    root: f32,
    followed: f32,
    glide: f32,
    level_fall: f32,
    pitch: Pitch,
}

impl Halo {
    pub fn new(sample_rate: f32) -> Self {
        let mut voices = [Voice::default(); VOICES];
        for (index, voice) in voices.iter_mut().enumerate() {
            // Alternate sides, wider for higher notes; sway rates spread
            // between the slowest and the quickest so no two move together.
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let pan = side * (0.15 + 0.1 * index as f32).min(0.7);
            let angle = (pan + 1.0) * FRAC_PI_4;
            voice.left = angle.cos();
            voice.right = angle.sin();
            let t = index as f32 / (VOICES - 1) as f32;
            let hz = SHIMMER_HZ.0 + (SHIMMER_HZ.1 - SHIMMER_HZ.0) * t;
            voice.sway_step = TAU * hz * CONTROL as f32 / sample_rate;
            voice.sway = TAU * 0.37 * index as f32;
        }
        Self {
            sample_rate,
            voices,
            chord: Chord::Fifth,
            count: 0,
            countdown: 0,
            root: 220.0,
            followed: 220.0,
            glide: 1.0 - (-(CONTROL as f32) / (GLIDE_MS * 0.001 * sample_rate)).exp(),
            level_fall: (-(CONTROL as f32) / (0.15 * sample_rate)).exp(),
            pitch: Pitch::new(sample_rate),
        }
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let mono = 0.5 * (input[0] + input[1]);
        let mono = if mono.is_finite() { mono } else { 0.0 };
        if settings.follow {
            self.pitch.push(mono);
        }
        if self.countdown == 0 {
            self.control(settings);
            self.countdown = CONTROL;
        }
        self.countdown -= 1;

        let mut wet = [0.0; 2];
        for voice in &mut self.voices[..self.count] {
            let re = voice.a * voice.re - voice.b * voice.im + voice.gain * mono;
            let im = voice.b * voice.re + voice.a * voice.im;
            voice.re = re;
            voice.im = im;
            let out = re * voice.weight;
            wet[0] += out * voice.left;
            wet[1] += out * voice.right;
        }
        if !(wet[0].is_finite() && wet[1].is_finite()) {
            self.voices.iter_mut().for_each(|voice| {
                voice.re = 0.0;
                voice.im = 0.0;
            });
            wet = [0.0; 2];
        }
        let mix = settings.mix.clamp(0.0, 1.0);
        let soft = |x: f32| CEILING * (x / CEILING).tanh();
        [input[0] + mix * soft(wet[0]), input[1] + mix * soft(wet[1])]
    }

    /// Every voice as `[height, level]`: height `0..=1` up the two octaves
    /// over the root, level how loud it rings. A voice the chord does not use
    /// reads level 0.
    pub fn rings(&self, out: &mut [f32; VOICES * READING]) {
        out.fill(0.0);
        for ((voice, ratio), reading) in self.voices[..self.count]
            .iter()
            .zip(self.chord.ratios())
            .zip(out.chunks_exact_mut(READING))
        {
            reading[0] = (ratio.log2() / SPAN_OCTAVES).clamp(0.0, 1.0);
            reading[1] = voice.level;
        }
    }

    /// The root in use, in Hz.
    pub fn root(&self) -> f32 {
        self.root
    }

    fn control(&mut self, settings: &Settings) {
        let ratios = settings.chord.ratios();
        if ratios.len() != self.count {
            // A voice joining starts silent rather than with another chord's ring.
            for voice in &mut self.voices[self.count.min(ratios.len())..] {
                voice.re = 0.0;
                voice.im = 0.0;
            }
        }
        self.count = ratios.len();
        self.chord = settings.chord;

        if settings.follow && self.pitch.clarity() > CLEAR {
            self.followed = self.pitch.hz();
        }
        let target = if settings.follow {
            self.followed
        } else {
            settings.root
        }
        .clamp(ROOT_HZ.0, ROOT_HZ.1);
        self.root *= (target / self.root).powf(self.glide);

        let t60 = DECAY_S.0 * (DECAY_S.1 / DECAY_S.0).powf(settings.decay.clamp(0.0, 1.0));
        let r = (-6.91 / (t60 * self.sample_rate)).exp();
        let tilt = DARK_TILT * (1.0 - settings.tone.clamp(0.0, 1.0));
        let sway = SHIMMER_CENTS * settings.shimmer.clamp(0.0, 1.0) / 1_200.0;
        let nyquist = 0.45 * self.sample_rate;
        let norm = 1.0 / (ratios.len() as f32).sqrt();
        for (voice, ratio) in self.voices.iter_mut().zip(ratios) {
            voice.sway = (voice.sway + voice.sway_step) % TAU;
            let hz = self.root * ratio * (sway * voice.sway.sin()).exp2();
            let omega = TAU * hz.min(nyquist) / self.sample_rate;
            voice.a = r * omega.cos();
            voice.b = r * omega.sin();
            voice.gain = 1.0 - r;
            voice.weight = if hz < nyquist {
                norm * ratio.powf(-tilt)
            } else {
                0.0
            };
            let now = (voice.re * voice.re + voice.im * voice.im).sqrt() * voice.weight;
            voice.level = (voice.level * self.level_fall).max(now);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        root: 220.0,
        follow: false,
        chord: Chord::Fifth,
        decay: 0.5,
        shimmer: 0.0,
        tone: 0.5,
        mix: 1.0,
    };

    fn run(settings: &Settings, seconds: f32, signal: impl Fn(usize) -> f32) -> (Halo, Vec<f32>) {
        let mut halo = Halo::new(RATE);
        let wet = (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                halo.process([x, x], settings)[0] - x
            })
            .collect();
        (halo, wet)
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|x| x * x).sum()
    }

    fn sine(hz: f32) -> impl Fn(usize) -> f32 {
        move |i| (TAU * hz * i as f32 / RATE).sin() * 0.3
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
        assert_eq!(energy(&run(&dry, 0.5, sine(220.0)).1), 0.0);
    }

    #[test]
    fn a_note_on_the_chord_rings_and_one_off_it_does_not() {
        let on = energy(&run(&SETTINGS, 1.0, sine(330.0)).1);
        let off = energy(&run(&SETTINGS, 1.0, sine(311.0)).1);
        assert!(on > off * 10.0, "{on} {off}");
    }

    #[test]
    fn decay_is_how_long_it_rings() {
        let tail = |decay| {
            let settings = Settings { decay, ..SETTINGS };
            let burst = |i: usize| if i < 4_800 { sine(220.0)(i) } else { 0.0 };
            energy(&run(&settings, 1.5, burst).1[24_000..])
        };
        assert!(tail(1.0) > tail(0.0) * 100.0);
    }

    #[test]
    fn a_ringing_note_comes_back_at_about_its_level() {
        // A long sine on the root: the root's resonator gives it back at
        // about its own level, give or take the other notes and the tilt.
        let settings = Settings {
            decay: 0.3,
            ..SETTINGS
        };
        let (_, wet) = run(&settings, 2.0, sine(220.0));
        let rms = (energy(&wet[48_000..]) / 48_000.0).sqrt();
        let input = 0.3 / 2f32.sqrt();
        assert!(rms > input * 0.1 && rms < input * 2.0, "{rms} vs {input}");
    }

    #[test]
    fn follow_moves_the_root_to_the_input() {
        let settings = Settings {
            follow: true,
            ..SETTINGS
        };
        let (halo, _) = run(&settings, 1.0, |i| {
            ((330.0 * i as f32 / RATE).fract() * 2.0 - 1.0) * 0.3
        });
        assert!((halo.root() / 330.0 - 1.0).abs() < 0.01, "{}", halo.root());
    }

    #[test]
    fn the_chords_have_their_thirds() {
        assert!(MAJOR.contains(&1.25) && !MAJOR.contains(&1.2));
        assert!(MINOR.contains(&1.2) && !MINOR.contains(&1.25));
        assert!(!FIFTH.contains(&1.25) && !FIFTH.contains(&1.2));
    }

    #[test]
    fn it_stays_bounded() {
        let wild = Settings {
            decay: 1.0,
            tone: 1.0,
            shimmer: 1.0,
            chord: Chord::Major,
            ..SETTINGS
        };
        let (_, wet) = run(&wild, 2.0, |i| if i % 480 < 240 { 1.0 } else { -1.0 });
        let peak = wet.iter().fold(0.0f32, |peak, x| peak.max(x.abs()));
        assert!(peak.is_finite() && peak <= CEILING, "{peak}");
    }

    #[test]
    fn the_rings_show_what_sounds() {
        let (halo, _) = run(&SETTINGS, 0.5, sine(220.0));
        let mut rings = [0.0; VOICES * READING];
        halo.rings(&mut rings);
        assert!(rings[1] > 0.0);
        assert!(
            rings
                .chunks_exact(READING)
                .all(|r| (0.0..=1.0).contains(&r[0]))
        );
    }
}
