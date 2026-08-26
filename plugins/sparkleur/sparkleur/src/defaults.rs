#![cfg(test)]
//! What the shipping defaults actually do, measured (`SPK-18`).
//!
//! **The defaults are the product's face** — there are no presets
//! (`REQ-SPK-021`) — and `SPK-18` settles them by ear. Half of what `dsp.md`
//! files under "耳で詰める定数" is not a matter of taste at all, though: "does
//! not breathe on silence", "both sides move on ordinary material", "sparkles
//! on a hat and not on a pad", "the images stay below −60 dB" are all
//! measurable, and a number that fails them is wrong before anyone listens.
//! This file measures those, so a listening pass only has to judge what is
//! actually a judgement.
//!
//! It lives in the wrapper rather than in `sparkleur-core` because **the
//! shipping defaults are the parameters' defaults**. Reading them from
//! `SparkleurParams::default()` is what makes it impossible for the numbers
//! measured here to drift from the numbers shipped.

use crate::params::SparkleurParams;
use nxe_audio::harmonics::{amplitude, bin_of, db_ratio, rms, tone};
use sparkleur_core::crossover::BAND_COUNT;
use sparkleur_core::engine::{Engine, Shape};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 64;

/// How long the engine is given before anything is believed. **Switching a
/// signal on is itself a transient**, and a gate that is doing its job opens
/// fully on it — which would read as "the pad sparkled".
const SETTLE: f32 = 0.25;

/// A deterministic noise source. No `rand` dependency, and the same numbers
/// every run.
struct Noise(u32);

impl Noise {
    fn new() -> Self {
        Self(0x1234_5678)
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1 << 23) as f32 * 2.0 - 1.0
    }
}

fn scaled(mut signal: Vec<f32>, dbfs: f32) -> Vec<f32> {
    let scale = 10.0f32.powf(dbfs / 20.0) / rms(&signal);
    for sample in &mut signal {
        *sample *= scale;
    }
    signal
}

/// **Pink, not white, is the proxy for ordinary material.** White noise puts
/// four times as much energy in the presence band as in the sub band purely
/// because the band is two octaves wide, so anything judging one band against
/// another reads it as bright when nothing is. Three one-pole sections give
/// −3 dB/octave closely enough to judge a threshold against.
fn pink(dbfs: f32, length: usize) -> Vec<f32> {
    let mut source = Noise::new();
    let (mut b0, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32);
    let raw = (0..length)
        .map(|_| {
            let value = source.next();
            b0 = 0.99765 * b0 + value * 0.099_046;
            b1 = 0.96300 * b1 + value * 0.296_516_4;
            b2 = 0.57000 * b2 + value * 1.052_691_3;
            b0 + b1 + b2 + value * 0.1848
        })
        .collect();
    scaled(raw, dbfs)
}

/// Six steady partials: something with no transients in it at all.
fn pad(dbfs: f32, length: usize) -> Vec<f32> {
    let mut mixed = vec![0.0f32; length];
    for hz in [110.0f32, 220.0, 330.0, 550.0, 1_100.0, 3_300.0] {
        for (sample, value) in mixed.iter_mut().zip(tone(0.05, hz, RATE, length)) {
            *sample += value;
        }
    }
    scaled(mixed, dbfs)
}

/// Eight strikes a second, each a 6 ms burst: nothing but transients.
fn hats(dbfs: f32, length: usize) -> Vec<f32> {
    let mut source = Noise::new();
    let period = (RATE / 8.0) as usize;
    let raw = (0..length)
        .map(|index| {
            let since = (index % period) as f32 / RATE;
            source.next() * (-since / 0.006).exp()
        })
        .collect();
    scaled(raw, dbfs)
}

/// Everything one run of the engine says about itself.
///
/// **Per-sample extremes, not the value left at the end.** A snapshot taken
/// after a decaying transient reads the gate as shut, which is the opposite of
/// what it did.
struct Run {
    output: Vec<f32>,
    gain_low_db: [f32; BAND_COUNT],
    gain_high_db: [f32; BAND_COUNT],
    de_harsh_db: f32,
    gate_peak: f32,
    gate_mean: f32,
}

impl Run {
    fn change_db(&self, input: &[f32]) -> f32 {
        db_ratio(rms(&self.output), rms(input))
    }

    fn peak(&self) -> f32 {
        self.output
            .iter()
            .fold(0.0f32, |worst, sample| worst.max(sample.abs()))
    }
}

/// The engine at the shipping defaults, run the way a host runs it.
fn rendered(input: &[f32]) -> Run {
    rendered_with(input, |_| {})
}

fn rendered_with(input: &[f32], adjust: impl FnOnce(&mut Shape)) -> Run {
    let params = SparkleurParams::default();
    let mut shape = params.display_shape();
    adjust(&mut shape);
    let levels = params.display_levels();

    let mut engine = Engine::new(RATE);
    let mut run = Run {
        output: Vec::with_capacity(input.len()),
        gain_low_db: [f32::MAX; BAND_COUNT],
        gain_high_db: [f32::MIN; BAND_COUNT],
        de_harsh_db: 0.0,
        gate_peak: 0.0,
        gate_mean: 0.0,
    };

    let settled = (RATE * SETTLE) as usize;
    for (index, block) in input.chunks(BLOCK).enumerate() {
        engine.set_shape(&shape);
        let onset = index * BLOCK < settled;
        for sample in block {
            let (left, _) = engine.process((*sample, *sample), &levels);
            run.output.push(left);
            if onset {
                continue;
            }

            for (band, gain) in engine.gains_db().iter().enumerate() {
                run.gain_low_db[band] = run.gain_low_db[band].min(*gain);
                run.gain_high_db[band] = run.gain_high_db[band].max(*gain);
            }
            run.de_harsh_db = run.de_harsh_db.min(engine.de_harsh_db());
            let opening = engine.sparkle_opening();
            run.gate_peak = run.gate_peak.max(opening);
            run.gate_mean += opening;
        }
    }
    run.gate_mean /= (input.len() - settled) as f32;
    run
}

/// **Silence stays silent, and nothing creeps** (`REQ-SPK-003`).
///
/// The upward side is what makes this worth measuring: a compressor that lifts
/// what is below its threshold will happily lift a noise floor into audibility,
/// and `FLOOR_DB` is the number that stops it. `LIFT` at zero means the floor
/// is fully in place, which is the shipping default.
#[test]
fn silence_does_not_breathe() {
    let run = rendered(&vec![0.0f32; RATE as usize]);
    assert_eq!(run.peak(), 0.0, "silence came out loud");
    assert_eq!(run.gate_peak, 0.0, "the gate opened on nothing");
    for band in 0..BAND_COUNT {
        assert_eq!(run.gain_high_db[band], 0.0, "band {band} lifted silence");
    }

    // **And the measurement can fail.** Silence itself cannot show this — a
    // level of exactly zero is below any floor there is — so the contrast is
    // drawn on a signal 80 dB down, which the shipping floor still holds at
    // nothing and an opened one reaches for. This is what `LIFT` is, and it is
    // why it ships closed (`REQ-SPK-003`).
    let whisper = pink(-80.0, RATE as usize);
    let held = rendered(&whisper);
    let lifted = rendered_with(&whisper, |shape| shape.lift = 1.0);
    let reach = |run: &Run| run.gain_high_db.iter().fold(f32::MIN, |a, b| a.max(*b));
    assert_eq!(reach(&held), 0.0, "the shipping floor let a whisper up");
    assert!(
        reach(&lifted) > 1.0,
        "opening the floor changed nothing: {:.2}",
        reach(&lifted)
    );
}

/// **Inserting it changes almost nothing, and both sides are working**
/// (`REQ-SPK-009`, `REQ-SPK-003`).
///
/// The two halves are one property: a level that barely moves at the level
/// mixes sit at would also be produced by a plugin that does nothing, so the
/// ends of the sweep have to show the two sides pulling in opposite directions.
#[test]
fn ordinary_material_passes_through_and_both_sides_move() {
    let length = RATE as usize;
    let nominal = pink(-18.0, length);
    let change = rendered(&nominal).change_db(&nominal);
    assert!(
        change.abs() < 0.5,
        "inserting it moved the level {change:+.2} dB"
    );

    let quiet = pink(-42.0, length);
    let up = rendered(&quiet).change_db(&quiet);
    let loud = pink(-6.0, length);
    let down = rendered(&loud).change_db(&loud);
    assert!(up > 0.2, "the upward side did nothing: {up:+.2} dB");
    assert!(down < -1.0, "the downward side did nothing: {down:+.2} dB");
}

/// **The protection stays out of the way of ordinary material**
/// (`REQ-SPK-008`, `SPK-18`).
///
/// De-Harsh is a relative guard, so it is level-independent by construction —
/// which means an ordinary spectrum that trips it trips it at *every* level,
/// on every insert, forever. It shipped doing exactly that: 1.3 dB out of plain
/// pink noise at any level, because the threshold sat below every broadband
/// material there is (`sparkleur_core::protect`).
#[test]
fn the_protection_leaves_ordinary_material_alone() {
    for dbfs in [-30.0f32, -18.0, -6.0] {
        let pull = rendered(&pink(dbfs, RATE as usize)).de_harsh_db;
        assert!(
            pull > -0.1,
            "it pulled {pull:.2} dB out of pink noise at {dbfs:.0} dBFS"
        );
    }

    // **And the measurement can fail.** Lift the painful band and the same
    // measurement finds it, so a reading of zero is the guard sitting still
    // rather than the guard being unreachable from here.
    let length = RATE as usize;
    let mut band = nxe_audio::biquad::BandPass::new(1_500.0, 5_000.0, RATE);
    let harsh: Vec<f32> = pink(-18.0, length)
        .iter()
        .map(|sample| sample + band.process(*sample) * (10.0f32.powf(10.0 / 20.0) - 1.0))
        .collect();
    let pull = rendered(&harsh).de_harsh_db;
    assert!(pull < -0.5, "harsh material did not move it: {pull:.2} dB");
}

/// **A hat sparkles and a pad does not** (`REQ-SPK-007`).
///
/// This is what `SNAP_RANGE_DB` decides, and it is the property that separates
/// this from a static exciter — so it is measured on two signals chosen to have
/// nothing in common but their level.
#[test]
fn transients_open_the_gate_and_sustains_do_not() {
    let length = RATE as usize;
    let struck = rendered(&hats(-18.0, length));
    let sustained = rendered(&pad(-18.0, length));

    assert!(
        struck.gate_peak > 0.9,
        "a hat did not open it: {:.3}",
        struck.gate_peak
    );
    assert!(
        sustained.gate_peak < 0.25,
        "a pad opened it: {:.3}",
        sustained.gate_peak
    );
    assert!(
        struck.gate_mean > sustained.gate_mean * 5.0,
        "the two were not told apart: {:.3} against {:.3}",
        struck.gate_mean,
        sustained.gate_mean
    );
}

/// **The generator's images stay below −60 dB** (`REQ-SPK-014`).
///
/// Measured through the whole engine rather than through `nxe_audio::shaper`
/// alone, because what ships is the two together and `sparkle::DRIVE` is set
/// against the oversampling that follows it.
/// **What comes out at the shipping defaults is the signal and its harmonics**
/// (`REQ-SPK-007`).
///
/// `SPK-6` measured the generator's own worst-case folding against −60 dB with
/// the shaper on a bare tone. This is the other end: the whole engine at the
/// settings it ships with, where the crossover, the gain computer and the
/// generator all contribute. It is **not** an aliasing figure — a compressor
/// moving its gain puts sidebands around a tone whatever the oversampling — so
/// what it fixes is the total residue rather than one cause of it.
///
/// Measured: **−68.3 dB at the defaults, −52.3 dB at CRUSH with `SPARK` full.**
#[test]
fn the_defaults_leave_the_signal_clean() {
    let residue_below = |spark: f32, character: f32| {
        let length = RATE as usize;
        let hz = 12_000.0;
        let params = SparkleurParams::default();
        let mut shape = params.display_shape();
        shape.character = character;
        let mut levels = params.display_levels();
        levels.spark = spark;

        let mut engine = Engine::new(RATE);
        let input = tone(0.5, hz, RATE, length);
        let mut output = Vec::with_capacity(length);
        for block in input.chunks(BLOCK) {
            engine.set_shape(&shape);
            for sample in block {
                output.push(engine.process((*sample, *sample), &levels).0);
            }
        }

        let fundamental = amplitude(&output, bin_of(hz, RATE, length));
        // **Only what lands under 20 kHz counts.** The halfbands' transition
        // bands deliberately let content fold into 20..24 kHz, which is what
        // makes them cheap enough to be free (`nxe_audio::oversample`).
        let ceiling = (20_000.0 / (RATE / length as f32)) as usize;
        let worst = (1..ceiling)
            .filter(|bin| {
                let hz_of = *bin as f32 * RATE / length as f32;
                hz_of > 100.0 && (hz_of / hz - (hz_of / hz).round()).abs() > 0.02
            })
            .map(|bin| amplitude(&output, bin))
            .fold(0.0f32, f32::max);
        db_ratio(worst, fundamental)
    };

    let params = SparkleurParams::default();
    let shipped = residue_below(params.spark.value(), params.character.value());
    assert!(shipped < -60.0, "the defaults left {shipped:.1} dB");

    // **And the measurement can fail.** The hardest setting the plugin has
    // reaches sixteen decibels higher, so a figure this low is the defaults
    // being gentle rather than the search looking in the wrong bins.
    let hardest = residue_below(1.0, 1.0);
    assert!(
        hardest > shipped + 8.0,
        "CRUSH at full Spark measured {hardest:.1}, no worse than {shipped:.1}"
    );
}

/// The whole picture on one screen, for a listening pass to start from.
///
/// `cargo test -p sparkleur --lib defaults -- --nocapture --ignored`
#[test]
#[ignore = "a report, not a check"]
fn survey() {
    let length = RATE as usize;
    let report = |name: &str, input: &[f32]| {
        let run = rendered(input);
        let bands = (0..BAND_COUNT)
            .map(|band| {
                format!(
                    "{:+.1}/{:+.1}",
                    run.gain_low_db[band], run.gain_high_db[band]
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "{name:>12}  Δ {:>+6.2} dB  bands {bands}  de-harsh {:>+5.2}  gate {:.3} peak / {:.3} mean",
            run.change_db(input),
            run.de_harsh_db,
            run.gate_peak,
            run.gate_mean,
        );
    };

    println!("\n--- pink noise, the ordinary-material proxy ---");
    for dbfs in [-42.0f32, -36.0, -24.0, -18.0, -12.0, -6.0] {
        report(&format!("{dbfs:.0} dBFS"), &pink(dbfs, length));
    }
    println!("\n--- sustain against transients, both at -18 dBFS ---");
    report("pad", &pad(-18.0, length));
    report("hats", &hats(-18.0, length));
}
