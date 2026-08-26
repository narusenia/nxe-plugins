//! Crossover → detector → gain, run end to end.
//!
//! One property lives here rather than in a module's own tests because it is
//! about the three of them together: **a fast `SPEED` on a low tone must not
//! distort it** (`REQ-SPK-005`). `SPK-3` could not measure it — there was no
//! gain to apply yet — so it was sent here.

use nxe_audio::harmonics::{amplitude, bin_of, rms, tone};
use sparkleur_core::crossover::{BAND_COUNT, Crossover, EDGES};
use sparkleur_core::detector::Detector;
use sparkleur_core::dynamics::{self, Settings, linear};
use sparkleur_core::sparkle::{self, Sparkle};

const RATE: f32 = 48_000.0;
const HZ: f32 = 50.0;
/// Loud enough that the downward side is working, so there is a gain to
/// modulate in the first place.
const AMPLITUDE: f32 = 0.5;

/// Total harmonic distortion of the output, as a fraction of the fundamental.
///
/// `detector_edges` is what the detector is told the bands are. Handing it the
/// real ones floors the bottom band's attack at two wavelengths of 60 Hz;
/// handing it edges an octave decade higher takes that floor away and leaves
/// the 1 ms `SPEED` asked for, which is the comparison this file exists to
/// make.
fn distortion(detector_edges: [f32; BAND_COUNT - 1], spark: f32) -> f32 {
    let mut crossover = Crossover::new(RATE);
    let mut detector = Detector::new(RATE, detector_edges);
    detector.set_speed(1.0, detector_edges);
    let settings = Settings {
        spark,
        ..Settings::default()
    };

    let length = RATE as usize;
    let input = tone(AMPLITUDE, HZ, RATE, length);
    let mut run = || {
        input
            .iter()
            .map(|sample| {
                let bands = crossover.split(*sample);
                detector.push(bands);
                let mut levels = [0.0f32; BAND_COUNT];
                for (band, level) in levels.iter_mut().enumerate() {
                    *level = detector.decibels(band);
                }
                let gains = dynamics::gains_db(&settings, levels);
                bands
                    .iter()
                    .zip(gains)
                    .map(|(band, gain)| band * linear(gain))
                    .sum::<f32>()
            })
            .collect::<Vec<f32>>()
    };
    run();
    let output = run();

    let first = bin_of(HZ, RATE, length);
    let fundamental = amplitude(&output, first);
    let harmonics: f32 = (2..=6)
        .map(|order| amplitude(&output, first * order).powi(2))
        .sum();
    harmonics.sqrt() / fundamental
}

/// **The reason the time constants are floored** (`REQ-SPK-005`): at the
/// fastest `SPEED`, a detector that could follow a 50 Hz waveform would turn
/// the compressor into a distortion box.
///
/// Measured at 48 kHz, `SPARK` = 1, a 50 Hz tone at half scale:
///
/// | | THD |
/// |---|---|
/// | no dynamics (`SPARK` = 0) | 0.0005 % |
/// | floored — two wavelengths of 60 Hz | **0.26 %** (−52 dB) |
/// | unfloored — the 1 ms `SPEED` asked for | **2.0 %** (−34 dB) |
///
/// The floored figure is not zero and cannot be: a 33 ms attack still moves a
/// little inside a 20 ms period. It is the honest cost of compressing bass at
/// all, and it is 7.6× smaller than what the floor prevents.
#[test]
fn a_fast_speed_does_not_distort_a_low_tone() {
    // The chain with no dynamics at all — the crossover on its own, which the
    // gate already showed is transparent.
    let clean = distortion(EDGES, 0.0);
    assert!(clean < 1e-4, "the crossover alone distorted {clean:.5}");

    let floored = distortion(EDGES, 1.0);
    assert!(
        floored < 0.005,
        "compressing distorted {:.3} %",
        floored * 100.0
    );

    // **And the measurement can fail.** The same chain with the floor taken
    // away follows the waveform, and the gain modulates the tone it is
    // supposed to be levelling.
    let unfloored = distortion(EDGES.map(|edge| edge * 1_200.0), 1.0);
    assert!(
        unfloored > floored * 5.0,
        "an unfloored detector distorted {unfloored:.5}, no worse than {floored:.5}"
    );
}

/// A tone in every band, so moving `CHARACTER` moves all five at once.
fn material(length: usize) -> Vec<f32> {
    let mut mixed = vec![0.0f32; length];
    for hz in [60.0f32, 220.0, 800.0, 3_000.0, 12_000.0] {
        for (sample, value) in mixed.iter_mut().zip(tone(0.2, hz, RATE, length)) {
            *sample += value;
        }
    }
    mixed
}

/// The output level with the axis at `position`, in dB.
fn loudness_db(position: f32) -> f32 {
    let character = sparkleur_core::character::at(position);
    let mut crossover = Crossover::new(RATE);
    let mut detector = Detector::new(RATE, crossover.edges());
    detector.set_speed(character.speed_centre, crossover.edges());
    let mut sparkle = Sparkle::new(RATE);
    sparkle.set(sparkle::Settings {
        snap: 0.5,
        bias: character.bias,
        hardness: character.hardness,
        ..sparkle::Settings::default()
    });
    let settings = Settings {
        curve: character.curve,
        spark: 1.0,
        ..Settings::default()
    };

    let input = material(RATE as usize);
    let mut run = || {
        input
            .iter()
            .map(|sample| {
                let bands = crossover.split(*sample);
                detector.push(bands);
                let mut levels = [0.0f32; BAND_COUNT];
                for (band, level) in levels.iter_mut().enumerate() {
                    *level = detector.decibels(band);
                }
                let gains = dynamics::gains_db(&settings, levels);
                let wet: f32 = bands
                    .iter()
                    .zip(gains)
                    .map(|(band, gain)| band * linear(gain))
                    .sum();
                wet + sparkle.process(bands[BAND_COUNT - 1], 0.5)
            })
            .collect::<Vec<f32>>()
    };
    run();
    let output = run();
    20.0 * rms(&output).log10()
}

/// **The axis must not double as a volume knob** (`REQ-SPK-006`,
/// `REQ-SPK-010`). Turning it changes what the processing sounds like; how loud
/// the result is belongs to `OUTPUT`.
///
/// Measured end to end on a tone in every band: **0.98 dB** across the whole
/// axis, so the trim column is still zero. It exists anyway — Velour found the
/// same drift at 3.0 dB after `TEXTURE` was finished and had to fit nine trims
/// into it afterwards (`VEL-17`), and `SPK-18` will listen to material this
/// test does not have.
#[test]
fn moving_the_character_axis_keeps_the_loudness_within_one_and_a_half_db() {
    let readings: Vec<f32> = (0..=4).map(|step| loudness_db(step as f32 / 4.0)).collect();
    let highest = readings.iter().copied().fold(f32::MIN, f32::max);
    let lowest = readings.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        highest - lowest < 1.5,
        "the axis moved the level {:.2} dB: {readings:?}",
        highest - lowest
    );

    // And the chain is doing something, or the bound above is a bound on
    // silence.
    assert!(lowest > -40.0, "the chain produced nothing: {readings:?}");
}

/// The engine's output level with the axis at `position`, in dB.
fn axis_loudness_db(input: &[f32], position: f32, spark: f32) -> f32 {
    use sparkleur_core::engine::{Engine, Levels, Shape};

    let shape = Shape {
        character: position,
        ..Shape::default()
    };
    let levels = Levels {
        spark,
        ..Levels::default()
    };

    let mut engine = Engine::new(RATE);
    let mut output = Vec::with_capacity(input.len());
    for block in input.chunks(64) {
        engine.set_shape(&shape);
        for sample in block {
            output.push(engine.process((*sample, *sample), &levels).0);
        }
    }
    // A quarter of a second for the detectors to settle.
    20.0 * rms(&output[(RATE * 0.25) as usize..]).log10()
}

/// Four materials that have nothing in common but their level.
fn materials(length: usize) -> Vec<(&'static str, Vec<f32>)> {
    use nxe_audio::harmonics::{at_dbfs, noise, pink};

    let pad = {
        let mut mixed = vec![0.0f32; length];
        for hz in [110.0f32, 220.0, 330.0, 550.0, 1_100.0, 3_300.0] {
            for (sample, value) in mixed.iter_mut().zip(tone(0.05, hz, RATE, length)) {
                *sample += value;
            }
        }
        at_dbfs(mixed, -18.0)
    };
    let period = (RATE / 8.0) as usize;
    let hats = at_dbfs(
        noise(1.0, length)
            .iter()
            .enumerate()
            .map(|(index, value)| value * (-((index % period) as f32 / RATE) / 0.006).exp())
            .collect(),
        -18.0,
    );

    vec![
        ("pink -18", at_dbfs(pink(1.0, length), -18.0)),
        ("pink -30", at_dbfs(pink(1.0, length), -30.0)),
        ("pad", pad),
        ("hats", hats),
    ]
}

/// **The axis must not double as a volume knob — on sustained material**
/// (`REQ-SPK-006`, `SPK-18`).
///
/// The test above measures a tone in every band through a hand-built chain and
/// reads 0.98 dB. The standing note against the trim column was that **changing
/// the material would move it**, so this runs the real `Engine` over four
/// materials. Drift from the middle of the axis, at full `SPARK`:
///
/// | | POLISH | GLOSS | CRUSH |
/// |---|---|---|---|
/// | pink −18 dBFS | −0.07 | 0 | +0.08 |
/// | pink −30 dBFS | +0.01 | 0 | +0.09 |
/// | six steady partials | −0.13 | 0 | +0.08 |
/// | **hi-hats** | **+1.23** | 0 | **−1.18** |
///
/// **Three of the four do not move at all**, and the fourth moves 2.4 dB — so
/// the answer for the trim column is not a number, it is that **there is no
/// number**. A trim large enough to flatten the hats would tilt the other three
/// by the same amount in the opposite direction, turning a drift on one
/// material into a drift on all of them.
///
/// What the hats measure is the ratios working. CRUSH compresses 6:1 against
/// POLISH's 1.5:1, and a hi-hat is very nearly all transient, so a harder ratio
/// takes level off it — **that is the processing being asked for**, not the
/// axis leaking into the volume. The condition that means something is the one
/// on material that has something to sustain.
#[test]
fn the_character_axis_holds_its_level_on_sustained_material() {
    let length = RATE as usize;
    let drift = |material: &[f32], spark: f32| {
        let readings: Vec<f32> = (0..=8)
            .map(|step| axis_loudness_db(material, step as f32 / 8.0, spark))
            .collect();
        let span = readings.iter().copied().fold(f32::MIN, f32::max)
            - readings.iter().copied().fold(f32::MAX, f32::min);
        (span, readings)
    };

    for (name, material) in materials(length) {
        if name == "hats" {
            continue;
        }
        for spark in [0.35f32, 1.0] {
            let (span, readings) = drift(&material, spark);
            assert!(
                span < 0.5,
                "{name} at SPARK {spark}: the axis moved the level {span:.2} dB: {readings:?}"
            );
        }
    }

    // **The transient case, stated rather than asserted away.** It is allowed
    // to move, but only downward as the ratios harden — a drift that wandered
    // would be something else.
    let hats = &materials(length)[3].1;
    let (span, readings) = drift(hats, 1.0);
    assert!(span < 3.0, "the hats moved {span:.2} dB: {readings:?}");
    for pair in readings.windows(2) {
        assert!(
            pair[1] <= pair[0] + 0.01,
            "the level rose toward CRUSH: {readings:?}"
        );
    }

    // **And the measurement can fail.** With the amount at zero there is no
    // processing to differ, so every position must read the same — if this
    // moved, the spans above would be measuring the material rather than
    // the axis.
    let (silent, _) = drift(hats, 0.0);
    assert!(
        silent < 0.01,
        "the axis moved the level {silent:.2} dB at SPARK 0"
    );
}

/// **The trim column is wired, and it was not** (`SPK-18`).
///
/// `Character::trim_db` was interpolated across the anchors and then read by
/// nobody: the engine never applied it, so the column that exists to correct
/// the axis's level could not have corrected anything. It now lives on
/// [`Curve`](sparkleur_core::dynamics::Curve) and is added **inside `SPARK`**,
/// because it pays back what the ratios took and those only act in proportion
/// to `SPARK`.
///
/// The shipping trims are all zero — the test above is why — so this drives the
/// mechanism directly rather than through the axis.
#[test]
fn the_character_trim_moves_the_level_and_zero_spark_still_does_nothing() {
    use sparkleur_core::dynamics::{Curve, Settings, gains_db};

    let levels = [-24.0f32; BAND_COUNT];
    let trimmed = |trim_db: f32, spark: f32| {
        gains_db(
            &Settings {
                curve: Curve {
                    trim_db,
                    ..Curve::GLOSS
                },
                spark,
                ..Settings::default()
            },
            levels,
        )[0]
    };

    let plain = trimmed(0.0, 1.0);
    assert!(
        (trimmed(3.0, 1.0) - plain - 3.0).abs() < 1e-4,
        "three decibels of trim moved the gain {:.3}",
        trimmed(3.0, 1.0) - plain
    );
    assert!(
        (trimmed(-3.0, 1.0) - plain + 3.0).abs() < 1e-4,
        "it is not symmetric"
    );

    // **Zero is still exactly nothing** (`REQ-SPK-009`). A trim applied outside
    // `SPARK` would make the axis a static gain at the one setting that must
    // not have one.
    for trim_db in [-6.0f32, 0.0, 6.0] {
        assert_eq!(
            trimmed(trim_db, 0.0),
            0.0,
            "a trim of {trim_db} leaked through SPARK = 0"
        );
    }
}

/// **The four boundaries divide ordinary material into comparable portions**
/// (`REQ-SPK-002`, `SPK-18`).
///
/// A boundary set that starved a band would leave a fader on the front panel
/// that never does anything, and no amount of listening tells you *which* band
/// unless you know what should have been in it. Pink noise carries equal energy
/// per octave, so it says what the boundaries themselves do rather than what
/// some particular sound does. Each band's share of the total:
///
/// | | SUB | BODY | MID | PRES | AIR |
/// |---|---|---|---|---|---|
/// | share, dB | −3.1 | −9.6 | −9.6 | −8.7 | −8.0 |
///
/// The four upper bands sit **within 1.6 dB of each other** — 120 / 400 / 1500
/// / 6000 Hz is close to four equal octave spans. SUB holds more because it
/// carries everything below 120 Hz including the part with no bottom to it.
#[test]
fn the_boundaries_divide_ordinary_material_evenly() {
    use nxe_audio::harmonics::{at_dbfs, db_ratio, pink};

    let length = RATE as usize;
    let material = at_dbfs(pink(1.0, length), -18.0);
    let mut crossover = Crossover::new(RATE);
    let mut energy = [0.0f64; BAND_COUNT];
    for sample in &material {
        for (slot, band) in energy.iter_mut().zip(crossover.split(*sample)) {
            *slot += (band * band) as f64;
        }
    }

    let total: f64 = energy.iter().sum();
    let shares: Vec<f32> = energy
        .iter()
        .map(|slot| db_ratio((slot / total).sqrt() as f32, 1.0))
        .collect();

    // No band is starved.
    for (band, share) in shares.iter().enumerate() {
        assert!(
            *share > -14.0,
            "band {band} holds only {share:.1} dB: {shares:?}"
        );
    }

    // And the four with a boundary on each side are comparable.
    let upper = &shares[1..];
    let span = upper.iter().copied().fold(f32::MIN, f32::max)
        - upper.iter().copied().fold(f32::MAX, f32::min);
    assert!(span < 2.5, "the upper four spread {span:.1} dB: {shares:?}");
}

/// **`FOCUS` slides every boundary by an octave and a half either way, and the
/// order survives** (`REQ-SPK-002`).
///
/// The top edge is what breaks first: an octave and a half above 6 kHz is
/// 17 kHz, which is under Nyquist at 48 kHz but would not be at a lower rate.
#[test]
fn focus_slides_the_boundaries_without_crossing_them() {
    for rate in [44_100.0f32, 48_000.0, 96_000.0] {
        for focus in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            let edges = sparkleur_core::crossover::edges_for(focus, rate);
            for pair in edges.windows(2) {
                assert!(
                    pair[0] < pair[1],
                    "at {rate} Hz, focus {focus}: the edges crossed: {edges:?}"
                );
            }
            assert!(
                edges[BAND_COUNT - 2] < rate / 2.0,
                "at {rate} Hz, focus {focus}: the top edge reached {:.0} Hz",
                edges[BAND_COUNT - 2]
            );
        }
    }

    // The full sweep really is three octaves end to end.
    let low = sparkleur_core::crossover::edges_for(-1.0, 48_000.0)[0];
    let high = sparkleur_core::crossover::edges_for(1.0, 48_000.0)[0];
    let octaves = (high / low).log2();
    assert!(
        (octaves - 3.0).abs() < 0.01,
        "the sweep covered {octaves:.2} octaves"
    );
}

/// **The axis changes the layer's flavour, not its amount** (`REQ-SPK-007`,
/// `REQ-SPK-010`, `SPK-18`).
///
/// `SPK-6` measured this on a bare tone through the generator alone. This is
/// the same property on material that has a spectrum, through the real
/// crossover, so the number a listener is actually judging is on the record:
///
/// | | POLISH | default | GLOSS | CRUSH |
/// |---|---|---|---|---|
/// | pink noise | −20.5 | −20.0 | −19.6 | −18.9 |
/// | hi-hats | −19.1 | −19.1 | −19.1 | −19.1 |
///
/// At the shipping `SPARK` of 0.35 the layer sits about **twenty decibels under
/// the band it rides on**, and moving the whole axis moves that by at most
/// 1.6 dB. That is `nxe_audio::shaper`'s normalisation working: `(β, h)` change
/// what the harmonics are without changing how much is added, which is what
/// lets `CHARACTER` and `SPARK` be separate controls at all.
#[test]
fn the_sparkle_layer_keeps_its_level_across_the_axis() {
    use nxe_audio::harmonics::{at_dbfs, db_ratio, pink};
    use sparkleur_core::character;

    let length = RATE as usize;
    let material = at_dbfs(pink(1.0, length), -18.0);

    let layer_below_db = |position: f32| {
        let character = character::at(position);
        let mut crossover = Crossover::new(RATE);
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(sparkle::Settings {
            snap: 0.6,
            bias: character.bias,
            hardness: character.hardness,
            ..sparkle::Settings::default()
        });

        let mut band_energy = 0.0f64;
        let mut layer = Vec::with_capacity(length);
        for sample in &material {
            let top = crossover.split(*sample)[BAND_COUNT - 1];
            band_energy += (top * top) as f64;
            layer.push(sparkle.process(top, 0.35));
        }
        db_ratio(rms(&layer), (band_energy / length as f64).sqrt() as f32)
    };

    let readings: Vec<f32> = [0.0f32, 0.27, 0.5, 1.0]
        .iter()
        .map(|position| layer_below_db(*position))
        .collect();
    let span = readings.iter().copied().fold(f32::MIN, f32::max)
        - readings.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        span < 3.0,
        "the axis moved the amount {span:.2} dB: {readings:?}"
    );

    // And there is a layer to measure: a generator producing nothing would
    // satisfy the bound above perfectly.
    for reading in &readings {
        assert!(
            (-40.0..-6.0).contains(reading),
            "the layer sat at {reading:.1} dB, which is not a layer: {readings:?}"
        );
    }
}
