//! Pulling the layer back when what was added is too much for the music.
//!
//! `nxe_audio::guard` with Air's numbers (`REQ-AIR-009`). No new mechanism —
//! what is Air's is where it listens and what it is allowed to touch.
//!
//! ## It can only stop adding
//!
//! Sparkleur's De-Harsh can pull one of five bands down because the signal is
//! split. Air's is additive, so **the only thing it can reach is its own
//! layer** — and that is the correct consequence of the topology rather than a
//! shortfall (`REQ-AIR-001`). If the source is harsh on its own, that is
//! Sparkleur's job.
//!
//! ## It listens to the output, one sample late
//!
//! The question is whether **the result** is too bright, not whether the input
//! was, so the detector is fed the output. That makes it a feedback loop, and a
//! one-sample delay is what breaks it — irrelevant against time constants in
//! milliseconds, and the gain only ever moves downward, so it cannot run away.
//!
//! ## Fixed threshold, always on
//!
//! "Is what I added too much" is a judgement, not a preference, so no macro
//! scales it (`REQ-AIR-009`). Sparkleur put De-Harsh on `SPARK` because that
//! product promises "brighter without hurting" — a promise with a degree in it.
//! Air's promise has no degree.

use nxe_audio::guard::{Band, GuardedBand, RelativeGuard, Settings as GuardSettings};

use crate::follow::bands_of;

/// How far above the reference the layer's band may sit before the guard
/// starts pulling.
///
/// **Placed from a distribution, not from taste** (`AIR-6`, the lesson of
/// `SPK-18`): Sparkleur shipped a threshold that sat below every broadband
/// material's ratio and pulled 1.3 dB out of ordinary music at the default
/// setting.
pub const THRESHOLD_DB: f32 = 9.0;

/// How far the Advanced deviation may move the threshold.
pub const THRESHOLD_RANGE_DB: f32 = 9.0;

/// Fast enough to catch one syllable, slow enough not to chatter.
const ATTACK_SECONDS: f32 = 0.005;
const RELEASE_SECONDS: f32 = 0.150;
/// How many dB of reduction per dB of excess.
const SLOPE: f32 = 1.0;
/// Past this the layer is gone rather than protected.
const MAX_REDUCTION_DB: f32 = 12.0;

/// The layer's protection.
pub struct Excess {
    guard: RelativeGuard<1>,
    sample_rate: f32,
    focus: f32,
    amount: f32,
}

impl Excess {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        Self {
            guard: RelativeGuard::new(settings_at(0.0, 0.0, sample_rate), sample_rate),
            sample_rate,
            focus: 0.0,
            amount: 1.0,
        }
    }

    /// **Block rate.** `deviation` is bipolar and rests at zero.
    ///
    /// The two ends do different things on purpose: **downward it fades the
    /// guard out**, reaching exactly nothing at the stop, and **upward it
    /// lowers the threshold**. One number that both disables and strengthens
    /// has to be built this way — a threshold alone can never reach "off", and
    /// an amount alone can never reach "stricter".
    pub fn set(&mut self, focus: f32, deviation: f32) {
        let focus = finite(focus, 0.0).clamp(-1.0, 1.0);
        let deviation = finite(deviation, 0.0).clamp(-1.0, 1.0);
        self.amount = (1.0 + deviation).clamp(0.0, 1.0);
        // **Retuned, never rebuilt.** A relative detector whose reference
        // follower has just been emptied reads infinitely harsh until it fills,
        // which would make moving `FOCUS` audible as a hole (`SPK-18`).
        if focus != self.focus {
            self.focus = focus;
            let (detection, reference) = bands_of(focus, self.sample_rate);
            self.guard.retune(
                Band {
                    low_hz: reference.0,
                    high_hz: reference.1,
                },
                [Band {
                    low_hz: detection.0,
                    high_hz: detection.1,
                }],
                self.sample_rate,
            );
        }
        self.guard.set_thresholds([threshold_db(deviation)]);
    }

    /// One frame of the **output's** mono sum. **Audio rate.**
    pub fn push(&mut self, mono: f32) {
        self.guard.push(finite(mono, 0.0), [self.amount]);
    }

    /// What the layer's gain should be multiplied by.
    pub fn gain(&self) -> f32 {
        self.guard.gain(0)
    }

    /// How far it is pulling, in dB — for the display. Zero when it is doing
    /// nothing (`REQ-AIR-018`).
    pub fn reduction_db(&self) -> f32 {
        self.guard.reduction_db(0)
    }

    pub fn reset(&mut self) {
        self.guard.reset();
    }
}

/// The threshold at one deviation, in dB.
fn threshold_db(deviation: f32) -> f32 {
    THRESHOLD_DB - deviation.max(0.0) * THRESHOLD_RANGE_DB
}

/// Where it listens, at one `FOCUS` position.
///
/// **The same two bands the brightness detector uses** (`crate::follow`), which
/// is what makes "the detection band follows `FOCUS`" one fact rather than two
/// that can drift apart — and what keeps the reference from ever containing the
/// detection band (`SPK-18`).
fn settings_at(focus: f32, deviation: f32, sample_rate: f32) -> GuardSettings<1> {
    let (detection, reference) = bands_of(focus, sample_rate);
    GuardSettings {
        reference: Band {
            low_hz: reference.0,
            high_hz: reference.1,
        },
        bands: [GuardedBand {
            band: Band {
                low_hz: detection.0,
                high_hz: detection.1,
            },
            threshold_db: threshold_db(deviation),
        }],
        attack_seconds: ATTACK_SECONDS,
        release_seconds: RELEASE_SECONDS,
        slope: SLOPE,
        max_reduction_db: MAX_REDUCTION_DB,
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Engine, Shape};
    use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
    use nxe_audio::harmonics::{at_dbfs, noise, pink, tone};

    const RATE: f32 = 48_000.0;
    const SEED: u32 = 0x4149_5236;
    const MATERIAL_DBFS: f32 = -18.0;
    const SECONDS: f32 = 3.0;

    fn length() -> usize {
        (RATE * SECONDS) as usize
    }

    fn shaped(signal: Vec<f32>, hz: f32) -> Vec<f32> {
        let mut sections = [Biquad::new(Coefficients::highpass(hz, BUTTERWORTH_Q, RATE)); 2];
        signal
            .into_iter()
            .map(|sample| sections.iter_mut().fold(sample, |x, f| f.process(x)))
            .collect()
    }

    /// Eleven partials of a 220 Hz note — the other half of "spectrally
    /// ordinary", and the one that is not noise.
    fn series() -> Vec<f32> {
        let mut sum = vec![0.0f32; length()];
        for harmonic in 1..12usize {
            let partial = tone(
                1.0 / harmonic as f32,
                220.0 * harmonic as f32,
                RATE,
                length(),
            );
            for (slot, value) in sum.iter_mut().zip(partial) {
                *slot += value;
            }
        }
        at_dbfs(sum, MATERIAL_DBFS)
    }

    /// The worst pull over a run, with the settling discarded.
    ///
    /// **A relative detector reads infinitely harsh until its reference fills**
    /// (`SPK-18`), so the first quarter-second is not a measurement.
    fn worst_pull_db(signal: &[f32], surface: f32, deviation: f32) -> f32 {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape {
            depths: [0.5; 3],
            guard: deviation,
            ..Shape::default()
        });
        let skip = (RATE * 0.25) as usize;
        let mut worst = 0.0f32;
        for (index, sample) in signal.iter().enumerate() {
            engine.process((*sample, *sample), surface, 1.0);
            if index >= skip {
                worst = worst.min(engine.guard_reduction_db());
            }
        }
        worst
    }

    /// **Exactly zero on ordinary material** (`REQ-AIR-009`). This is the test
    /// Sparkleur shipped without: its threshold sat below every broadband
    /// material's ratio and pulled 1.3 dB out of ordinary music at the default
    /// setting, and the two-tone test it did have passed anyway because a
    /// signal with nothing in the detection band passes at any threshold
    /// (`SPK-18`).
    #[test]
    fn ordinary_material_is_not_touched_at_all() {
        for surface in [0.35f32, 1.0] {
            for (name, signal) in [
                ("pink", at_dbfs(pink(1.0, length()), MATERIAL_DBFS)),
                ("a harmonic series", series()),
            ] {
                let worst = worst_pull_db(&signal, surface, 0.0);
                assert_eq!(worst, 0.0, "{name} at SURFACE {surface} lost {worst:.2} dB");
            }
        }
    }

    /// And it does fire, or the test above is measuring a guard that cannot
    /// work at all (`VEL-10`).
    ///
    /// Measured: high-passed pink is pulled the full **12 dB**, white **1.3**.
    /// White is not the proxy for ordinary material and never was — it puts
    /// four times the energy in a two-octave band as in a half-octave one
    /// purely because of the width (`nxe_audio::harmonics::pink`).
    #[test]
    fn bright_material_is_pulled() {
        let bright = at_dbfs(shaped(pink(1.0, length()), 4_000.0), MATERIAL_DBFS);
        let worst = worst_pull_db(&bright, 1.0, 0.0);
        assert!(worst < -6.0, "bright material only lost {worst:.2} dB");

        let white = at_dbfs(noise(1.0, length()), MATERIAL_DBFS);
        assert!(worst_pull_db(&white, 1.0, 0.0) < 0.0);
    }

    /// **It reads a ratio, so it does not move with input gain**
    /// (`REQ-AIR-009`) — the same property the brightness detector has, and for
    /// the same reason.
    #[test]
    fn the_pull_does_not_move_with_input_gain() {
        let loud = worst_pull_db(
            &at_dbfs(shaped(pink(1.0, length()), 4_000.0), -6.0),
            1.0,
            0.0,
        );
        let quiet = worst_pull_db(
            &at_dbfs(shaped(pink(1.0, length()), 4_000.0), -30.0),
            1.0,
            0.0,
        );
        assert!(
            (loud - quiet).abs() < 0.2,
            "24 dB of input moved the pull from {loud:.2} to {quiet:.2} dB"
        );
    }

    /// The deviation reaches **exactly off** at its stop (`REQ-AIR-009`).
    #[test]
    fn the_lowest_deviation_disables_it_completely() {
        let bright = at_dbfs(shaped(pink(1.0, length()), 4_000.0), MATERIAL_DBFS);
        assert_eq!(worst_pull_db(&bright, 1.0, -1.0), 0.0);
        // …and the other end is stricter rather than the same.
        let strict = worst_pull_db(&at_dbfs(noise(1.0, length()), MATERIAL_DBFS), 1.0, 1.0);
        let ordinary = worst_pull_db(&at_dbfs(noise(1.0, length()), MATERIAL_DBFS), 1.0, 0.0);
        assert!(strict < ordinary, "{strict:.2} against {ordinary:.2} dB");
    }

    /// **It only ever touches the layer** (`REQ-AIR-009`). With no layer to
    /// pull, the plugin is still the wire it promises to be — which is the
    /// thing a protection is most likely to break.
    #[test]
    fn it_never_reaches_the_original() {
        let bright = at_dbfs(shaped(pink(1.0, length()), 4_000.0), MATERIAL_DBFS);
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape {
            depths: [0.5; 3],
            ..Shape::default()
        });
        for sample in bright {
            assert_eq!(engine.process((sample, sample), 0.0, 1.0), (sample, sample));
        }
    }

    /// `FOCUS` carries the detection band with it, so a layer moved above the
    /// detector cannot happen (`REQ-AIR-009`).
    #[test]
    fn the_detection_band_follows_focus() {
        let corners: Vec<f32> = [-1.0f32, 0.0, 1.0]
            .iter()
            .map(|focus| bands_of(*focus, RATE).0.0)
            .collect();
        assert!(
            corners[0] < corners[1] && corners[1] < corners[2],
            "{corners:?}"
        );

        // And moving it is a retune rather than a rebuild: the followers keep
        // their state, so the guard does not read "infinitely harsh" for a
        // moment every time the knob moves (`SPK-18`).
        let mut excess = Excess::new(RATE);
        for sample in at_dbfs(pink(1.0, 24_000), MATERIAL_DBFS) {
            excess.push(sample);
        }
        excess.set(1.0, 0.0);
        excess.push(0.0);
        assert_eq!(excess.reduction_db(), 0.0, "moving FOCUS opened a hole");
    }

    #[test]
    fn hostile_settings_neither_panic_nor_produce_nonsense() {
        let mut excess = Excess::new(RATE);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9] {
            excess.set(value, value);
            excess.push(value);
            assert!(
                excess.gain().is_finite(),
                "{value} produced {}",
                excess.gain()
            );
            assert!(excess.reduction_db().is_finite());
        }
    }
}
