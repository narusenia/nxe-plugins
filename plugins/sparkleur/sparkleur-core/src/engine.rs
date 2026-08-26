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

        // **Sanitised once, here** (`REQ-SPK-016`). Every filter below is
        // recursive — forty biquads a channel, five power followers, the
        // guard — so one sample that is not a number would sit in their state
        // for the rest of the session. `SPK-9` measured that: a single NaN and
        // the engine never produced a finite sample again.
        //
        // One check at the mouth rather than one per filter: the same
        // arithmetic, forty times less of it. The **dry** path is deliberately
        // not sanitised — it is a pass-through, and a host that sends a NaN
        // gets it back rather than having it quietly turned into silence
        // (`velour_core::Engine` made the same call).
        let (left, right) = (finite(dry_left), finite(dry_right));

        let bands = [
            self.crossover[0].split(left),
            self.crossover[1].split(right),
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
        self.de_harsh.push((left + right) * 0.5, de_harsh);

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
    use nxe_audio::harmonics::{amplitude, bin_of, db_ratio, rms, tone};

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

    /// Renders the same material in blocks of `block` samples, calling
    /// `set_shape` once per block the way a host does.
    fn rendered(block: usize, shape: &Shape) -> Vec<f32> {
        let mut engine = Engine::new(RATE);
        let levels = working();
        let input = material(4_800);
        let mut output = Vec::with_capacity(input.len());
        for chunk in input.chunks(block) {
            engine.set_shape(shape);
            for sample in chunk {
                output.push(engine.process((*sample, *sample), &levels).0);
            }
        }
        output
    }

    /// **The block size is the host's business, not the sound's**
    /// (`REQ-SPK-017`). Everything that moves per block is idempotent, so
    /// calling it more often changes nothing.
    #[test]
    fn the_block_size_does_not_change_the_output() {
        let shape = Shape::default();
        let reference = rendered(512, &shape);
        for block in [1usize, 32, 2048] {
            assert_eq!(
                rendered(block, &shape),
                reference,
                "a block of {block} rendered differently"
            );
        }
    }

    /// A deterministic noise-like source. No `Math.random`, and — more to the
    /// point — **no stationary tone for an added layer to cancel against**.
    fn noise(length: usize) -> Vec<f32> {
        let mut state = 0x2545_F491u32;
        (0..length)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) as f32 / (1u32 << 23) as f32 * 0.6 - 0.3
            })
            .collect()
    }

    /// **The rate is the host's business too** (`REQ-SPK-017`): every time
    /// constant is in seconds and every corner is in hertz, so the same signal
    /// has to come out sounding the same.
    ///
    /// **Measured on noise, not on tones.** The Sparkle layer runs through an
    /// IIR oversampler whose delay is fixed in *samples*, so its phase against
    /// the band it rides on turns with the rate. A stationary tone in the top
    /// band therefore sums up to **1 dB** differently at 48 and 96 kHz — a
    /// phase difference on an added layer, not different processing, and
    /// nothing a real source holds still enough to show. Noise has no such
    /// coherence, and the level then agrees to **0.33 dB**.
    #[test]
    fn the_sample_rate_does_not_change_the_level() {
        let measure = |rate: f32| {
            let mut engine = Engine::new(rate);
            let levels = Levels {
                air: 1.0,
                ..working()
            };
            let input = noise(rate as usize);
            for sample in &input {
                engine.process((*sample, *sample), &levels);
            }
            let output: Vec<f32> = input
                .iter()
                .map(|s| engine.process((*s, *s), &levels).0)
                .collect();
            db_ratio(rms(&output), rms(&input))
        };

        let reference = measure(48_000.0);
        assert!(reference < -1.0, "the engine did nothing to compare");
        for rate in [44_100.0f32, 96_000.0] {
            let reading = measure(rate);
            assert!(
                (reading - reference).abs() < 0.5,
                "{rate} Hz came out {:.3} dB away",
                reading - reference
            );
        }
    }

    /// And it makes the same harmonics (`REQ-SPK-017`).
    ///
    /// **Referred to the input**, not to the output's own fundamental — that
    /// one is the sum of the dry band and the layer, and its phase is what the
    /// test above is written around. The second harmonic is the layer's alone,
    /// so nothing cancels.
    ///
    /// Measured spread: **9.1 %** against the 10 % the requirement allows. Most
    /// of it is the input lid, which is a fraction of the rate for aliasing
    /// safety and therefore genuinely lower at 44.1 kHz (`sparkle`).
    #[test]
    fn the_sample_rate_does_not_change_the_harmonics() {
        const HZ: f32 = 7_000.0;

        let measure = |rate: f32| {
            let mut engine = Engine::new(rate);
            let levels = Levels {
                air: 1.0,
                ..working()
            };
            let length = rate as usize;
            let input = tone(0.4, HZ, rate, length);
            for sample in &input {
                engine.process((*sample, *sample), &levels);
            }
            let output: Vec<f32> = input
                .iter()
                .map(|s| engine.process((*s, *s), &levels).0)
                .collect();
            amplitude(&output, bin_of(HZ * 2.0, rate, length))
                / amplitude(&input, bin_of(HZ, rate, length))
        };

        let reference = measure(48_000.0);
        assert!(reference > 0.01, "there were no harmonics to compare");
        for rate in [44_100.0f32, 96_000.0] {
            let reading = measure(rate);
            assert!(
                (reading - reference).abs() < reference * 0.1,
                "{rate} Hz made {reading:.5} against {reference:.5}"
            );
        }
    }

    /// **The trap `VEL-10` found, in a different place.** Every filter here is
    /// recursive, so one sample that is not a number would sit in its state for
    /// the rest of the session — and the failure is not silence, it is a plugin
    /// that looks like it is working and passes nothing (`REQ-SPK-016`).
    #[test]
    fn one_hostile_sample_does_not_latch_it() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut engine = Engine::new(RATE);
            let levels = working();

            engine.process((value, value), &levels);

            // A quarter of a second is far longer than any time constant here.
            let mut last = (0.0f32, 0.0f32);
            for sample in material(12_000) {
                last = engine.process((sample, sample), &levels);
            }
            assert!(
                last.0.is_finite() && last.1.is_finite(),
                "{value} latched it: {last:?}"
            );

            let output: Vec<f32> = material(4_800)
                .iter()
                .map(|s| engine.process((*s, *s), &levels).0)
                .collect();
            let level = rms(&output);
            assert!(level > 1e-3, "{value} left it silent: {level:e}");
        }
    }

    /// **No step when a control moves** (`REQ-SPK-016`). Measured the way
    /// `VEL-10` learned to: the largest sample-to-sample jump while a knob
    /// moves, against the largest one while everything is still.
    #[test]
    fn moving_a_control_does_not_step() {
        const BLOCK: usize = 512;

        fn worst_step(mut moved: impl FnMut(usize) -> Shape) -> f32 {
            let mut engine = Engine::new(RATE);
            let levels = working();
            let input = material(9_600);
            let mut previous = 0.0f32;
            let mut worst = 0.0f32;
            for (index, chunk) in input.chunks(BLOCK).enumerate() {
                engine.set_shape(&moved(index));
                for sample in chunk {
                    let output = engine.process((*sample, *sample), &levels).0;
                    // The first block settles the filters; a step there is the
                    // engine starting, not a control moving.
                    if index > 2 {
                        worst = worst.max((output - previous).abs());
                    }
                    previous = output;
                }
            }
            worst
        }

        let still = worst_step(|_| Shape::default());

        // A quarter of the axis in one block boundary — far faster than a
        // smoother would ever hand it over.
        let moving = worst_step(|block| Shape {
            character: if block < 8 { 0.25 } else { 0.50 },
            ..Shape::default()
        });
        assert!(
            moving < still * 1.3,
            "moving CHARACTER stepped {moving:.5} against {still:.5} at rest"
        );

        // **And the measurement can fail.** A per-band trim multiplies the
        // signal directly, so jumping one really does put a step in — which is
        // why the wrapper smooths it (`params.rs`).
        let trimmed = worst_step(|block| {
            let mut shape = Shape::default();
            if block >= 8 {
                shape.gain_db[2] = 12.0;
            }
            shape
        });
        assert!(
            trimmed > still * 1.3,
            "a 12 dB jump did not step either ({trimmed:.5}), so the test proves nothing"
        );
    }

    /// **The figure's subject has to point the way the dynamics do**
    /// (`REQ-SPK-018`): a band over the threshold reports a gain below unity,
    /// one under it reports a gain above. The picture draws this straight into
    /// `nxe_ui::band::Band::delta`, which is signed for exactly this reason
    /// (`SPK-10`).
    #[test]
    fn the_reported_gains_point_the_way_the_dynamics_do() {
        let settle = |scale: f32, lift: f32| {
            let mut engine = Engine::new(RATE);
            engine.set_shape(&Shape {
                lift,
                ..Shape::default()
            });
            let levels = working();
            for sample in material(RATE as usize / 2) {
                engine.process((sample * scale, sample * scale), &levels);
            }
            engine.gains_db()
        };

        // Loud: over the downward threshold, so the regions sink.
        let loud = settle(1.0, 0.0);
        assert!(
            loud.iter().any(|gain| *gain < -0.5),
            "nothing was compressed: {loud:?}"
        );
        assert!(
            loud.iter().all(|gain| *gain <= 0.001),
            "something was lifted while it was loud: {loud:?}"
        );

        // Quiet, with the floor left where it is: under the upward threshold
        // and above the fade, so the regions rise.
        let quiet = settle(0.01, 0.0);
        assert!(
            quiet.iter().any(|gain| *gain > 0.5),
            "nothing was lifted: {quiet:?}"
        );
        assert!(
            quiet.iter().all(|gain| *gain >= -0.001),
            "something was compressed while it was quiet: {quiet:?}"
        );
    }

    /// **And when the music stops the figure empties** (`REQ-SPK-018`). The
    /// floor is what does it: below the fade there is nothing to lift, so every
    /// region settles back on the unity line rather than drifting upward.
    #[test]
    fn silence_brings_every_reported_gain_back_to_unity() {
        let mut engine = Engine::new(RATE);
        let levels = working();
        for sample in material(RATE as usize / 2) {
            engine.process((sample, sample), &levels);
        }
        assert!(
            engine.gains_db().iter().any(|gain| *gain != 0.0),
            "nothing was happening to decay from"
        );

        // **How long "decays away" actually takes.** The release the axis
        // chooses for the bottom band is floored at 133 ms, and a one-pole
        // sheds 4.34 dB per time constant, so falling from working level to
        // under the floor's fade is a couple of seconds. That is the upward
        // side being honest rather than slow: until the level is under the
        // floor there really is something down there to lift, which is why the
        // floor exists at all (`REQ-SPK-003`).
        let at_rest = |engine: &Engine| {
            engine.gains_db().iter().all(|gain| *gain == 0.0)
                && engine.de_harsh_db() == 0.0
                && engine.sparkle_opening() == 0.0
        };

        let mut silent = 0usize;
        while !at_rest(&engine) {
            engine.process((0.0, 0.0), &levels);
            silent += 1;
            assert!(
                silent < RATE as usize * 5,
                "it never settled: gains {:?}, de-harsh {}, sparkle {}",
                engine.gains_db(),
                engine.de_harsh_db(),
                engine.sparkle_opening()
            );
        }
        let seconds = silent as f32 / RATE;
        assert!(seconds < 3.0, "it took {seconds:.2} s to empty");
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
