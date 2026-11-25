//! Prisma: the input split into the notes you play
//! (`plugins/prisma/docs/implementation/prisma-plan.md`).
//!
//! Up to [`NOTES`] notes ring at once, each a bank of [`PARTIALS`]
//! resonators on its harmonics, rung by the input. The notes are the ones held
//! on MIDI (**Midi**), or Halo's chord over a root you set or the input's own
//! pitch (**Chord**). A released note stops taking the input and rings out.
//!
//! **Decay** is how long a note rings (T60, 0.3 to 12 s). **Inharmonic**
//! stretches the partials apart, as a stiff string or a bell does
//! (`k·√(1 + B·k²)`). **Coupling** lets the resonators ring each other: every
//! one hears the others too, so partials that coincide across notes feed each
//! other and the chord blooms after the input has stopped. **Tone** tilts the
//! upper partials from dark to bright, **Shimmer** sways each note slowly off
//! its pitch. **Freeze** holds the ring as it is: no decay, no new input.
//!
//! The resonators are Halo's (`crate::halo`): complex one-poles, a gain that
//! hears a note within a capture width of the pitch whatever the Decay, and a
//! swing capped against the input's recent peak, all from **Sensitivity**.
//!
//! The ring is added to the dry signal, `Mix` of it: at 0 the output is the
//! input. Nothing is delayed.
//!
//! Allocation-free once built.

use crate::halo::{Chord, ROOT_HZ};
use nxe_audio::pitch::Pitch;
use std::f32::consts::{FRAC_PI_4, TAU};

/// The most notes ringing at once.
pub const NOTES: usize = 8;
/// Resonators per note, on its harmonics 1 to 4.
pub const PARTIALS: usize = 4;
/// What [`Prisma::partials`] writes per resonator: Hz, level, the note as a
/// MIDI number (fractional in Chord), and whether the note is held.
pub const READING: usize = 4;
/// Samples between coefficient updates.
const CONTROL: usize = 32;
/// T60 from no Decay to full, in seconds.
const DECAY_S: (f32, f32) = (0.3, 12.0);
/// T60 under Freeze: long enough to be still.
const FROZEN_S: f32 = 600.0;
/// Inharmonic's `B` at full.
const STIFFNESS: f32 = 0.03;
/// How much of the others a resonator hears at full Coupling, and how much
/// slower the cap falls then (a share of the resonators' own fall).
const COUPLING: f32 = 0.5;
const OUTLAST: f32 = 0.6;
/// Halo's capture width and cap (`crate::halo`), from no Sensitivity to full.
const CAPTURE: (f32, f32) = (0.01, 0.08);
const CAP: (f32, f32) = (1.0, 4.0);
/// Tone's tilt: a partial's weight is `k^-tilt`, from this at no Tone to 0.
const DARK_TILT: f32 = 1.6;
const SHIMMER_CENTS: f32 = 18.0;
const SHIMMER_HZ: (f32, f32) = (0.15, 0.6);
/// The root's glide in Chord with Follow, and how sure the tracker must be.
const GLIDE_MS: f32 = 40.0;
const CLEAR: f32 = 0.85;
/// The wet signal's ceiling, softly.
const CEILING: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Chord,
    Midi,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub source: Source,
    /// Chord: its notes, its root in Hz, and whether the root follows the input.
    pub chord: Chord,
    pub root: f32,
    pub follow: bool,
    /// `0..=1` each.
    pub sensitivity: f32,
    pub decay: f32,
    pub inharmonic: f32,
    pub coupling: f32,
    pub tone: f32,
    pub shimmer: f32,
    pub mix: f32,
    pub freeze: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Resonator {
    re: f32,
    im: f32,
    a: f32,
    b: f32,
    gain: f32,
    weight: f32,
    hz: f32,
    level: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct Note {
    /// As a MIDI number; 0 Hz when the slot is empty.
    midi: f32,
    held: bool,
    /// When it was struck, to steal the oldest.
    age: u32,
    left: f32,
    right: f32,
    sway: f32,
    sway_step: f32,
    partials: [Resonator; PARTIALS],
}

impl Note {
    fn ringing(&self) -> bool {
        self.partials.iter().any(|p| p.level > 1e-4)
    }
}

pub struct Prisma {
    sample_rate: f32,
    notes: [Note; NOTES],
    source: Source,
    countdown: usize,
    root: f32,
    followed: f32,
    glide: f32,
    level_fall: f32,
    r: f32,
    /// The input's peak, and how fast it falls: with the resonators, or
    /// slower under Coupling so the ring may outlast its own decay.
    peak: f32,
    peak_fall: f32,
    cap: f32,
    coupling: f32,
    /// The resonators' summed output last sample, for Coupling.
    last: f32,
    clock: u32,
    pitch: Pitch,
}

impl Prisma {
    pub fn new(sample_rate: f32) -> Self {
        let mut notes = [Note::default(); NOTES];
        for (index, note) in notes.iter_mut().enumerate() {
            let side = if index % 2 == 0 { -1.0 } else { 1.0 };
            let pan = side * (0.15 + 0.08 * index as f32).min(0.7);
            let angle = (pan + 1.0) * FRAC_PI_4;
            note.left = angle.cos();
            note.right = angle.sin();
            let t = index as f32 / (NOTES - 1) as f32;
            let hz = SHIMMER_HZ.0 + (SHIMMER_HZ.1 - SHIMMER_HZ.0) * t;
            note.sway_step = TAU * hz * CONTROL as f32 / sample_rate;
            note.sway = TAU * 0.37 * index as f32;
        }
        Self {
            sample_rate,
            notes,
            // Chord rebuilds its notes every tick, so starting here loses nothing.
            source: Source::Midi,
            countdown: 0,
            root: 220.0,
            followed: 220.0,
            glide: 1.0 - (-(CONTROL as f32) / (GLIDE_MS * 0.001 * sample_rate)).exp(),
            level_fall: (-(CONTROL as f32) / (0.15 * sample_rate)).exp(),
            r: 0.0,
            peak: 0.0,
            peak_fall: 0.0,
            cap: CAP.0,
            coupling: 0.0,
            last: 0.0,
            clock: 0,
            pitch: Pitch::new(sample_rate),
        }
    }

    /// A MIDI note held. It takes an empty slot, else the oldest released,
    /// else the oldest.
    pub fn note_on(&mut self, midi: u8) {
        self.clock = self.clock.wrapping_add(1);
        let midi = f32::from(midi);
        let slot = self
            .notes
            .iter()
            .position(|n| n.held && n.midi == midi)
            .or_else(|| self.notes.iter().position(|n| !n.held && !n.ringing()))
            .or_else(|| {
                (0..NOTES)
                    .filter(|i| !self.notes[*i].held)
                    .min_by_key(|i| self.notes[*i].age)
            })
            .unwrap_or_else(|| (0..NOTES).min_by_key(|i| self.notes[*i].age).unwrap_or(0));
        let note = &mut self.notes[slot];
        if note.midi != midi {
            note.partials = [Resonator::default(); PARTIALS];
        }
        note.midi = midi;
        note.held = true;
        note.age = self.clock;
        // The new tuning from the next sample, not the next control tick.
        self.countdown = 0;
    }

    pub fn note_off(&mut self, midi: u8) {
        let midi = f32::from(midi);
        for note in &mut self.notes {
            if note.held && note.midi == midi {
                note.held = false;
            }
        }
        self.countdown = 0;
    }

    /// One stereo frame.
    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let mono = 0.5 * (input[0] + input[1]);
        let mono = if mono.is_finite() { mono } else { 0.0 };
        if settings.follow && settings.source == Source::Chord {
            self.pitch.push(mono);
        }
        if self.countdown == 0 {
            self.control(settings);
            self.countdown = CONTROL;
        }
        self.countdown -= 1;

        if !settings.freeze {
            self.peak = mono.abs().max(self.peak * self.peak_fall);
        }
        let cap = self.cap * self.peak;
        let heard = if settings.freeze { 0.0 } else { mono };
        let share = 1.0 / ((NOTES * PARTIALS) as f32).sqrt();
        let others = self.coupling * self.last;
        let mut wet = [0.0; 2];
        let mut total = 0.0;
        for note in &mut self.notes {
            let excite = if note.held || settings.source == Source::Chord {
                heard
            } else {
                0.0
            };
            let mut out = 0.0;
            for p in &mut note.partials {
                // Everyone else, not itself.
                let x = excite + others - self.coupling * share * p.re;
                let mut re = p.a * p.re - p.b * p.im + p.gain * x;
                let mut im = p.b * p.re + p.a * p.im;
                let swing = re * re + im * im;
                if swing > cap * cap {
                    let scale = cap / swing.sqrt();
                    re *= scale;
                    im *= scale;
                }
                p.re = re;
                p.im = im;
                out += re * p.weight;
                total += re;
            }
            wet[0] += out * note.left;
            wet[1] += out * note.right;
        }
        self.last = total * share;
        if !(wet[0].is_finite() && wet[1].is_finite() && self.last.is_finite()) {
            for note in &mut self.notes {
                note.partials = [Resonator::default(); PARTIALS];
            }
            self.last = 0.0;
            wet = [0.0; 2];
        }
        let mix = settings.mix.clamp(0.0, 1.0);
        let soft = |x: f32| CEILING * (x / CEILING).tanh();
        [input[0] + mix * soft(wet[0]), input[1] + mix * soft(wet[1])]
    }

    /// Every resonator, note by note, as `[hz, level, note, held]`. An empty
    /// slot reads all 0.
    pub fn partials(&self, out: &mut [f32; NOTES * PARTIALS * READING]) {
        for (note, chunk) in self
            .notes
            .iter()
            .zip(out.chunks_exact_mut(PARTIALS * READING))
        {
            for (p, reading) in note.partials.iter().zip(chunk.chunks_exact_mut(READING)) {
                if p.hz > 0.0 {
                    reading[0] = p.hz;
                    reading[1] = p.level;
                    reading[2] = note.midi;
                    reading[3] = if note.held { 1.0 } else { 0.0 };
                } else {
                    reading.fill(0.0);
                }
            }
        }
    }

    fn control(&mut self, settings: &Settings) {
        if settings.source != self.source {
            // Neither source's notes carry over to the other.
            for note in &mut self.notes {
                *note = Note {
                    partials: [Resonator::default(); PARTIALS],
                    midi: 0.0,
                    held: false,
                    ..*note
                };
            }
            self.source = settings.source;
        }
        if settings.source == Source::Chord {
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
            let ratios = settings.chord.ratios();
            let root = 69.0 + 12.0 * (self.root / 440.0).log2();
            for (index, note) in self.notes.iter_mut().enumerate() {
                match ratios.get(index) {
                    Some(ratio) => {
                        note.midi = root + 12.0 * ratio.log2();
                        note.held = true;
                    }
                    None => {
                        note.midi = 0.0;
                        note.held = false;
                    }
                }
            }
        }

        let t60 = if settings.freeze {
            FROZEN_S
        } else {
            DECAY_S.0 * (DECAY_S.1 / DECAY_S.0).powf(settings.decay.clamp(0.0, 1.0))
        };
        let r = (-6.91 / (t60 * self.sample_rate)).exp();
        self.r = r;
        let sensitivity = settings.sensitivity.clamp(0.0, 1.0);
        let capture = CAPTURE.0 * (CAPTURE.1 / CAPTURE.0).powf(sensitivity);
        self.cap = CAP.0 * (CAP.1 / CAP.0).powf(sensitivity);
        let coupling = settings.coupling.clamp(0.0, 1.0);
        self.coupling = COUPLING * coupling;
        self.peak_fall = r.powf(1.0 - OUTLAST * coupling);
        let stiffness = STIFFNESS * settings.inharmonic.clamp(0.0, 1.0).powi(2);
        let tilt = DARK_TILT * (1.0 - settings.tone.clamp(0.0, 1.0));
        let sway = SHIMMER_CENTS * settings.shimmer.clamp(0.0, 1.0) / 1_200.0;
        let nyquist = 0.45 * self.sample_rate;
        let sounding = self.notes.iter().filter(|n| n.midi > 0.0).count().max(1);
        let norm = 1.0 / (sounding as f32).sqrt();
        for note in &mut self.notes {
            note.sway = (note.sway + note.sway_step) % TAU;
            if note.midi <= 0.0 {
                note.partials.iter_mut().for_each(|p| p.hz = 0.0);
                continue;
            }
            let hz = 440.0 * ((note.midi - 69.0) / 12.0).exp2() * (sway * note.sway.sin()).exp2();
            for (k, p) in note.partials.iter_mut().enumerate() {
                let k = (k + 1) as f32;
                p.hz = hz * k * (1.0 + stiffness * k * k).sqrt();
                let omega = TAU * p.hz.min(nyquist) / self.sample_rate;
                p.a = r * omega.cos();
                p.b = r * omega.sin();
                p.gain = omega * capture;
                p.weight = if p.hz < nyquist {
                    norm * k.powf(-tilt)
                } else {
                    0.0
                };
                let now = (p.re * p.re + p.im * p.im).sqrt() * p.weight;
                p.level = (p.level * self.level_fall).max(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;
    const SETTINGS: Settings = Settings {
        source: Source::Midi,
        chord: Chord::Fifth,
        root: 220.0,
        follow: false,
        sensitivity: 0.5,
        decay: 0.5,
        inharmonic: 0.0,
        coupling: 0.0,
        tone: 0.5,
        shimmer: 0.0,
        mix: 1.0,
        freeze: false,
    };

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|x| x * x).sum()
    }

    fn sine(hz: f32) -> impl Fn(usize) -> f32 {
        move |i| (TAU * hz * i as f32 / RATE).sin() * 0.3
    }

    fn run(
        engine: &mut Prisma,
        settings: &Settings,
        seconds: f32,
        signal: impl Fn(usize) -> f32,
    ) -> Vec<f32> {
        (0..(seconds * RATE) as usize)
            .map(|index| {
                let x = signal(index);
                engine.process([x, x], settings)[0] - x
            })
            .collect()
    }

    fn held(notes: &[u8]) -> Prisma {
        let mut engine = Prisma::new(RATE);
        notes.iter().for_each(|n| engine.note_on(*n));
        engine
    }

    #[test]
    fn silence_stays_silent() {
        let mut engine = held(&[57, 60, 64]);
        assert_eq!(energy(&run(&mut engine, &SETTINGS, 0.5, |_| 0.0)), 0.0);
    }

    #[test]
    fn no_mix_is_the_input() {
        let mut engine = held(&[57]);
        let dry = Settings {
            mix: 0.0,
            ..SETTINGS
        };
        assert_eq!(energy(&run(&mut engine, &dry, 0.5, sine(220.0))), 0.0);
    }

    #[test]
    fn no_notes_held_rings_nothing() {
        let mut engine = Prisma::new(RATE);
        assert_eq!(energy(&run(&mut engine, &SETTINGS, 0.5, sine(220.0))), 0.0);
    }

    #[test]
    fn a_held_note_picks_its_pitch_out() {
        // A3 = 220 Hz held: a 220 Hz input rings it far more than 300 Hz does.
        let on = energy(&run(&mut held(&[57]), &SETTINGS, 0.5, sine(220.0)));
        let off = energy(&run(&mut held(&[57]), &SETTINGS, 0.5, sine(300.0)));
        assert!(on > 20.0 * off, "on {on}, off {off}");
    }

    #[test]
    fn a_released_note_rings_out_and_takes_no_more() {
        // The shortest Decay, so 1.75 s is many T60s.
        let short = Settings {
            decay: 0.0,
            ..SETTINGS
        };
        let mut engine = held(&[57]);
        run(&mut engine, &short, 0.3, sine(220.0));
        engine.note_off(57);
        let after = run(&mut engine, &short, 2.0, sine(220.0));
        let (start, end) = after.split_at(after.len() / 8);
        assert!(energy(start) > 0.0);
        assert!(
            energy(end) < 0.01 * energy(start),
            "{} vs {}",
            energy(end),
            energy(start)
        );
    }

    #[test]
    fn freeze_holds_the_ring() {
        let mut engine = held(&[57]);
        run(&mut engine, &SETTINGS, 0.3, sine(220.0));
        let frozen = Settings {
            freeze: true,
            ..SETTINGS
        };
        let tail = run(&mut engine, &frozen, 2.0, |_| 0.0);
        let (start, end) = tail.split_at(tail.len() / 2);
        assert!(
            energy(end) > 0.8 * energy(start),
            "{} vs {}",
            energy(end),
            energy(start)
        );
    }

    #[test]
    fn inharmonic_stretches_the_partials() {
        let mut out = [0.0; NOTES * PARTIALS * READING];
        let mut engine = held(&[57]);
        run(&mut engine, &SETTINGS, 0.01, |_| 0.0);
        engine.partials(&mut out);
        assert!((out[3 * READING] / out[0] - 4.0).abs() < 1e-3);
        let stiff = Settings {
            inharmonic: 1.0,
            ..SETTINGS
        };
        run(&mut engine, &stiff, 0.01, |_| 0.0);
        engine.partials(&mut out);
        assert!(
            out[3 * READING] / out[0] > 4.5,
            "{}",
            out[3 * READING] / out[0]
        );
    }

    #[test]
    fn coupling_lets_the_chord_bloom() {
        // A3 and A2 share 220 Hz. Once the input stops, the coupled pair keeps
        // feeding each other and outlasts the pair left alone.
        let tail = |coupling: f32| {
            let mut engine = held(&[57, 45]);
            let settings = Settings {
                coupling,
                ..SETTINGS
            };
            run(&mut engine, &settings, 0.5, sine(220.0));
            energy(&run(&mut engine, &settings, 1.5, |_| 0.0)[24_000..])
        };
        assert!(
            tail(1.0) > 1.5 * tail(0.0),
            "{} vs {}",
            tail(1.0),
            tail(0.0)
        );
    }

    #[test]
    fn chord_needs_no_midi() {
        let chord = Settings {
            source: Source::Chord,
            chord: Chord::Major,
            ..SETTINGS
        };
        let mut engine = Prisma::new(RATE);
        assert!(energy(&run(&mut engine, &chord, 0.5, sine(220.0))) > 1.0);
    }

    #[test]
    fn bounded_at_full_everything() {
        let wild = Settings {
            sensitivity: 1.0,
            decay: 1.0,
            inharmonic: 1.0,
            coupling: 1.0,
            tone: 1.0,
            shimmer: 1.0,
            ..SETTINGS
        };
        let mut engine = held(&[36, 48, 55, 60, 64, 67, 72, 76]);
        let wet = run(&mut engine, &wild, 2.0, |i| {
            sine(130.0)(i) * 3.0 + sine(523.0)(i)
        });
        assert!(
            wet.iter()
                .all(|x| x.is_finite() && x.abs() <= CEILING + 1e-3)
        );
    }
}
