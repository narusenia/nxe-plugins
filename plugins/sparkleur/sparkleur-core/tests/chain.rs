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
        air: 0.5,
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
                wet + sparkle.process(bands[BAND_COUNT - 1])
            })
            .collect::<Vec<f32>>()
    };
    run();
    let output = run();
    20.0 * rms(&output).log10() + character.trim_db
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
