//! Everything wired together: split, detect, decide, apply, add the layer,
//! blend back against the untouched input.
//!
//! **No DSP is invented here.** The units above own the arithmetic; this owns
//! the order it happens in, which channel sees which state, and what moves per
//! block against what moves per sample.
//!
//! ## What is per channel and what is shared
//!
//! Filters and gains are per channel; **detection is linked** (`REQ-SPK-011`).
//! A gain that moved on one side only would throw the image sideways, and that
//! is the one thing this product must not do. Because a crossover is linear,
//! the mono sum's bands are the mean of the two channels' bands — so linking
//! costs an addition rather than a third crossover (`SPK-3`).
//!
//! ## Two layers of macro
//!
//! `SPARK` is the amount; `BODY` and `AIR` are bipolar tilts that say where to
//! lean (`REQ-SPK-009`). A tilt multiplies the amount for **one band** and
//! leaves the Advanced weights alone, which is the rule the UI conventions
//! state: the macro scales the per-band values, it does not write them
//! (`.agents/rules/vizia.md`).
//!
//! `AIR` leans on the top band's dynamics **and** on the Sparkle layer, because
//! both are what "sheen" means here.

use crate::character::{self, Character};
use crate::crossover::{BAND_COUNT, Crossover};
use crate::detector::Detector;
use crate::dynamics::{self, Curve, FLOOR_DB, FLOOR_MIN_DB, Weights, linear};
use crate::protect::{self, DeHarsh, GUARDED_BAND};
use crate::sparkle::{self, Sparkle};
use nxe_audio::oversample::Factor;

/// Which band each macro tilt leans on (`REQ-SPK-002`).
const BODY_BAND: usize = 1;
const AIR_BAND: usize = BAND_COUNT - 1;

/// What moves once per block.
///
/// **Read with `next_step(samples)`**, not `next()` — a smoother advances one
/// sample per call, so reading it once per block would stretch every ramp by
/// the block length (`VEL-5`).
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// `0..=1`, POLISH to CRUSH.
    pub character: f32,
    /// `-1..=1`, slides every band boundary.
    pub focus: f32,
    /// **Bipolar around what `CHARACTER` chose**, so zero is "as the axis says"
    /// and the two never write one value (`REQ-SPK-005`, `REQ-SPK-008`).
    pub speed: f32,
    /// How much of the Sparkle layer is handed to the transient, `0..=1`.
    pub snap: f32,
    /// How far the upward floor is opened, `0..=1`.
    pub lift: f32,
    /// Bipolar deviations from what `CHARACTER` chose. Both ride on `SPARK`:
    /// the protection arrives with the brightness rather than being found in a
    /// panel afterwards (`REQ-SPK-008`).
    pub de_harsh: f32,
    pub sub_protect: f32,
    /// Per-band weights and trim, in band order.
    pub up: [f32; BAND_COUNT],
    pub down: [f32; BAND_COUNT],
    pub gain_db: [f32; BAND_COUNT],
    pub solo: [bool; BAND_COUNT],
    pub factor: Factor,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            character: character::DEFAULT_POSITION,
            focus: 0.0,
            speed: 0.0,
            snap: 0.5,
            lift: 0.0,
            de_harsh: 0.0,
            sub_protect: 0.0,
            up: [1.0; BAND_COUNT],
            down: [1.0; BAND_COUNT],
            gain_db: [0.0; BAND_COUNT],
            solo: [false; BAND_COUNT],
            factor: Factor::default(),
        }
    }
}

/// What moves every sample, because it multiplies the signal.
#[derive(Clone, Copy, Debug)]
pub struct Levels {
    /// The amount. **Zero is exactly nothing** (`REQ-SPK-009`).
    pub spark: f32,
    /// Bipolar tilts, `-1..=1`. Zero leaves the band on `SPARK`.
    pub body: f32,
    pub air: f32,
    /// `0..=1`. **Zero is bit identical to the input** (`REQ-SPK-001`).
    pub mix: f32,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            spark: 0.0,
            body: 0.0,
            air: 0.0,
            mix: 1.0,
        }
    }
}

pub struct Engine {
    crossover: [Crossover; 2],
    detector: Detector,
    sparkle: [Sparkle; 2],
    de_harsh: DeHarsh,
    curve: Curve,
    weights: [Weights; BAND_COUNT],
    floor_db: f32,
    de_harsh_amount: f32,
    solo: [bool; BAND_COUNT],
    /// What was actually applied to each band, in dB — the picture's subject
    /// (`REQ-SPK-018`).
    gains_db: [f32; BAND_COUNT],
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        let crossover = [Crossover::new(sample_rate), Crossover::new(sample_rate)];
        let edges = crossover[0].edges();
        let mut engine = Self {
            crossover,
            detector: Detector::new(sample_rate, edges),
            sparkle: [Sparkle::new(sample_rate), Sparkle::new(sample_rate)],
            de_harsh: DeHarsh::new(sample_rate),
            curve: Curve::GLOSS,
            weights: [Weights::NEUTRAL; BAND_COUNT],
            floor_db: FLOOR_DB,
            de_harsh_amount: 0.0,
            solo: [false; BAND_COUNT],
            gains_db: [0.0; BAND_COUNT],
        };
        engine.set_shape(&Shape::default());
        engine
    }

    /// **Block rate.** Every coefficient and every resolved setting is built
    /// here; [`process`] does arithmetic and nothing else.
    ///
    /// [`process`]: Self::process
    pub fn set_shape(&mut self, shape: &Shape) {
        let character = character::at(shape.character);

        for crossover in &mut self.crossover {
            crossover.set_focus(shape.focus);
        }
        let edges = self.crossover[0].edges();

        self.detector.set_speed(
            protect::amount_of(character.speed_centre, shape.speed),
            edges,
        );

        let sub_protect = protect::amount_of(character.sub_protect, shape.sub_protect);
        let scales = protect::ceiling_scales(sub_protect);
        self.weights = std::array::from_fn(|band| Weights {
            down: shape.down[band],
            up: shape.up[band],
            gain_db: shape.gain_db[band],
            ceiling_scale: scales[band],
        });

        self.curve = character.curve;
        self.floor_db = floor_of(shape.lift);
        self.de_harsh_amount = protect::amount_of(character.de_harsh, shape.de_harsh);
        self.solo = shape.solo;

        for sparkle in &mut self.sparkle {
            sparkle.set(sparkle::Settings {
                snap: shape.snap,
                bias: character.bias,
                hardness: character.hardness,
                // The layer must not fall below the band it came from, and
                // `FOCUS` moves where that is (`SPK-6`).
                edge_hz: edges[BAND_COUNT - 2],
                factor: shape.factor,
            });
        }
    }

    /// One frame in, one frame out. **Audio rate**, allocation-free.
    pub fn process(&mut self, input: (f32, f32), levels: &Levels) -> (f32, f32) {
        let (dry_left, dry_right) = input;

        let bands = [
            self.crossover[0].split(dry_left),
            self.crossover[1].split(dry_right),
        ];

        // Linked detection, for free: the split is linear, so the mono sum's
        // bands are the mean of the channels' bands (`REQ-SPK-011`).
        self.detector.push(std::array::from_fn(|band| {
            (bands[0][band] + bands[1][band]) * 0.5
        }));
        // **De-Harsh rides on `SPARK`.** The concept document's
        // "more Spark, more harshness suppression" is this multiplication
        // (`REQ-SPK-008`), and it is also what makes `SPARK` = 0 amplitude-flat
        // — without it a lone 2 kHz tone is over the guard's threshold against
        // its own reference band and gets pulled with the dynamics turned off
        // (`SPK-8` found that the hard way).
        let de_harsh = self.de_harsh_amount * finite(levels.spark).clamp(0.0, 1.0);
        self.de_harsh.push((dry_left + dry_right) * 0.5, de_harsh);

        let soloing = self.solo.iter().any(|on| *on);
        let mut wet = (0.0f32, 0.0f32);

        for (band, (left, right)) in bands[0].into_iter().zip(bands[1]).enumerate() {
            // Computed even for a band that is not being heard, so the picture
            // keeps moving while something is soloed.
            let gain_db = dynamics::band_gain_db(
                self.detector.decibels(band),
                &self.curve,
                &self.weights[band],
                spark_for(band, levels),
                self.floor_db,
            );
            self.gains_db[band] = gain_db;

            if soloing && !self.solo[band] {
                continue;
            }

            let mut gain = linear(gain_db);
            if band == GUARDED_BAND {
                gain *= self.de_harsh.gain();
            }
            wet.0 += left * gain;
            wet.1 += right * gain;
        }

        // The layer rides on the top band. Fed even when that band is not being
        // heard, because its followers are what the gate is made of and
        // restarting them on a solo would be audible when the solo came off.
        let air = if soloing && !self.solo[AIR_BAND] {
            0.0
        } else {
            amount_of(levels.spark, levels.air)
        };
        wet.0 += self.sparkle[0].process(bands[0][AIR_BAND], air);
        wet.1 += self.sparkle[1].process(bands[1][AIR_BAND], air);

        // **`MIX` = 0 is the input, exactly** (`REQ-SPK-001`): `dry + 0·x` is
        // `dry` for any finite `x`, and the dry path branched before the split
        // so there is nothing else in the way.
        let mix = finite(levels.mix).clamp(0.0, 1.0);
        (
            dry_left + mix * (wet.0 - dry_left),
            dry_right + mix * (wet.1 - dry_right),
        )
    }

    /// What was applied to each band, in dB. Positive is upward compression.
    pub fn gains_db(&self) -> [f32; BAND_COUNT] {
        self.gains_db
    }

    /// How far De-Harsh is pulling, in dB — zero when it is doing nothing.
    pub fn de_harsh_db(&self) -> f32 {
        self.de_harsh.reduction_db()
    }

    /// How far the Sparkle gate stands open, `0..=1`.
    pub fn sparkle_opening(&self) -> f32 {
        self.sparkle[0].opening()
    }

    /// The band boundaries as they currently stand, in Hz.
    pub fn edges(&self) -> [f32; BAND_COUNT - 1] {
        self.crossover[0].edges()
    }

    pub fn reset(&mut self) {
        for crossover in &mut self.crossover {
            crossover.reset();
        }
        for sparkle in &mut self.sparkle {
            sparkle.reset();
        }
        self.detector.reset();
        self.de_harsh.reset();
        self.gains_db = [0.0; BAND_COUNT];
    }
}

/// `LIFT` as the floor it opens to, in dB.
fn floor_of(lift: f32) -> f32 {
    let lift = finite(lift).clamp(0.0, 1.0);
    FLOOR_DB + (FLOOR_MIN_DB - FLOOR_DB) * lift
}

/// The amount for one band: `SPARK`, tilted.
///
/// A tilt of zero leaves the band on `SPARK` exactly, `+1` doubles it and `-1`
/// silences it. `band_gain_db` clamps, so the top of the tilt saturates rather
/// than running away.
fn spark_for(band: usize, levels: &Levels) -> f32 {
    match band {
        BODY_BAND => amount_of(levels.spark, levels.body),
        AIR_BAND => amount_of(levels.spark, levels.air),
        _ => finite(levels.spark).clamp(0.0, 1.0),
    }
}

fn amount_of(spark: f32, tilt: f32) -> f32 {
    (finite(spark).clamp(0.0, 1.0) * (1.0 + finite(tilt).clamp(-1.0, 1.0))).clamp(0.0, 1.0)
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// The character an engine is currently set to — for the picture and for tests.
pub fn character_at(position: f32) -> Character {
    character::at(position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics::{amplitude, bin_of, db_ratio, tone};

    const RATE: f32 = 48_000.0;

    fn material(length: usize) -> Vec<f32> {
        let mut mixed = vec![0.0f32; length];
        for hz in [60.0f32, 220.0, 800.0, 3_000.0, 12_000.0] {
            for (sample, value) in mixed.iter_mut().zip(tone(0.2, hz, RATE, length)) {
                *sample += value;
            }
        }
        mixed
    }

    fn working() -> Levels {
        Levels {
            spark: 1.0,
            ..Levels::default()
        }
    }

    /// **The one transparency promise this product makes** (`REQ-SPK-001`).
    #[test]
    fn mix_at_zero_is_bit_identical() {
        let mut engine = Engine::new(RATE);
        let levels = Levels {
            mix: 0.0,
            ..working()
        };
        for sample in material(4_800) {
            let (left, right) = engine.process((sample, sample * 0.5), &levels);
            assert_eq!(left, sample, "the left channel moved");
            assert_eq!(right, sample * 0.5, "the right channel moved");
        }
    }

    /// **`SPARK` = 0 is amplitude-flat, and the phase does rotate** — the
    /// honest consequence of splitting, written into the requirement rather
    /// than hidden (`REQ-SPK-001`).
    #[test]
    fn spark_at_zero_is_flat_but_not_bit_identical() {
        let mut engine = Engine::new(RATE);
        let levels = Levels::default();
        let length = RATE as usize;

        for hz in [
            20.0f32, 60.0, 200.0, 700.0, 2_000.0, 5_000.0, 12_000.0, 18_000.0,
        ] {
            let input = tone(0.5, hz, RATE, length);
            for sample in &input {
                engine.process((*sample, *sample), &levels);
            }
            let output: Vec<f32> = input
                .iter()
                .map(|s| engine.process((*s, *s), &levels).0)
                .collect();

            let bin = bin_of(hz, RATE, length);
            let reading = db_ratio(amplitude(&output, bin), amplitude(&input, bin));
            assert!(reading.abs() < 0.1, "{hz} Hz came out {reading:.3} dB");
        }

        // And it is **not** the input sample for sample, which is what the
        // requirement warns about.
        let mut engine = Engine::new(RATE);
        let input = material(4_800);
        let moved = input
            .iter()
            .any(|s| engine.process((*s, *s), &levels).0 != *s);
        assert!(
            moved,
            "the split left the signal untouched, which it cannot"
        );
    }

    /// **The image does not move** (`REQ-SPK-011`).
    #[test]
    fn a_centred_signal_stays_centred() {
        let mut engine = Engine::new(RATE);
        let levels = working();
        for sample in material(4_800) {
            let (left, right) = engine.process((sample, sample), &levels);
            assert_eq!(left, right, "the two channels drifted apart");
        }
    }

    /// And a silent channel stays silent, which is what a linked **detector**
    /// with per-channel **gains** buys (`REQ-SPK-011`).
    #[test]
    fn a_silent_channel_stays_silent() {
        let mut engine = Engine::new(RATE);
        let levels = working();
        for sample in material(4_800) {
            let (_, right) = engine.process((sample, 0.0), &levels);
            assert_eq!(right, 0.0, "the silent side got something");
        }
    }

    #[test]
    fn soloing_a_band_drops_the_others() {
        let mut engine = Engine::new(RATE);
        let mut shape = Shape::default();
        shape.solo[0] = true;
        engine.set_shape(&shape);

        let levels = working();
        let length = RATE as usize;
        let input = tone(0.5, 12_000.0, RATE, length);
        for sample in &input {
            engine.process((*sample, *sample), &levels);
        }
        let output: Vec<f32> = input
            .iter()
            .map(|s| engine.process((*s, *s), &levels).0)
            .collect();

        let bin = bin_of(12_000.0, RATE, length);
        let reading = db_ratio(amplitude(&output, bin), amplitude(&input, bin));
        assert!(
            reading < -30.0,
            "soloing SUB still passed 12 kHz at {reading:.1} dB"
        );

        // And the gain for the band nobody is hearing is still being computed,
        // so the picture does not freeze.
        assert!(engine.gains_db().iter().all(|gain| gain.is_finite()));
    }

    #[test]
    fn the_tilts_lean_without_touching_the_other_bands() {
        let mut engine = Engine::new(RATE);
        let read = |engine: &mut Engine, levels: &Levels| {
            for sample in material(9_600) {
                engine.process((sample, sample), levels);
            }
            engine.gains_db()
        };

        let flat = read(&mut engine, &working());
        let mut engine = Engine::new(RATE);
        let leaned = read(
            &mut engine,
            &Levels {
                body: -1.0,
                ..working()
            },
        );

        assert_ne!(flat[BODY_BAND], leaned[BODY_BAND], "BODY did nothing");
        for band in 0..BAND_COUNT {
            if band == BODY_BAND {
                continue;
            }
            assert!(
                (flat[band] - leaned[band]).abs() < 1e-4,
                "BODY moved band {band}"
            );
        }
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut engine = Engine::new(RATE);
            engine.set_shape(&Shape {
                character: value,
                focus: value,
                speed: value,
                snap: value,
                lift: value,
                de_harsh: value,
                sub_protect: value,
                up: [value; BAND_COUNT],
                down: [value; BAND_COUNT],
                gain_db: [value; BAND_COUNT],
                ..Shape::default()
            });
            let levels = Levels {
                spark: value,
                body: value,
                air: value,
                mix: value,
            };
            for sample in material(4_800) {
                let (left, right) = engine.process((sample, sample), &levels);
                assert!(left.is_finite() && right.is_finite(), "{value} gave {left}");
            }
            assert!(engine.gains_db().iter().all(|gain| gain.is_finite()));
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut engine = Engine::new(RATE);
        let levels = working();
        for sample in material(9_600) {
            engine.process((sample, sample), &levels);
        }
        engine.reset();
        for _ in 0..64 {
            let (left, right) = engine.process((0.0, 0.0), &levels);
            assert_eq!((left, right), (0.0, 0.0), "state survived the reset");
        }
    }
}
