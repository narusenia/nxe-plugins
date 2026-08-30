//! `PUNCH`, measured (`SPK-22`, `REQ-SPK-020`).
//!
//! **The receipt this feature was let back in on.** `REQ-SPK-020` put transient
//! enhancement in v2 because "whether it worked was the most ear-dependent
//! thing in the product" — there was no quantity to check.
//!
//! The quantity is **how far the attack of a note stands above its body**. The
//! plan proposed the crest factor for this and it turned out to be the wrong
//! instrument: over noise the peak is one random sample, so the number wandered
//! by a quarter of a decibel whichever way the feature was pointed. Measuring
//! the first 10 ms of a burst against the 40 ms after it asks the same question
//! of a hundred samples instead of one.
//!
//! ```text
//! cargo test -p sparkleur-core --test punch -- --nocapture
//! ```

use nxe_audio::harmonics::{at_dbfs, pink, rms};
use sparkleur_core::engine::{Engine, Levels, Shape};

const RATE: f32 = 48_000.0;
const SECONDS: f32 = 2.0;
const REFERENCE_DBFS: f32 = -18.0;

/// Pink noise in bursts — something for a transient detector to find. The same
/// material `ceiling.rs` uses for the gate.
fn bursts() -> Vec<f32> {
    let length = (RATE * SECONDS) as usize;
    let period = (RATE * 0.2) as usize;
    let mut signal = at_dbfs(pink(1.0, length), REFERENCE_DBFS);
    for (index, sample) in signal.iter_mut().enumerate() {
        if index % period > period / 2 {
            *sample = 0.0;
        }
    }
    signal
}

/// How far the attack of a burst stands above its body, in dB.
///
/// The first 10 ms of each burst against the 40 ms that follow it, averaged
/// over every burst in the signal.
fn attack_over_body_db(signal: &[f32]) -> f32 {
    const ATTACK_MS: f32 = 10.0;
    const BODY_MS: f32 = 40.0;
    let period = (RATE * 0.2) as usize;
    let attack = (RATE * ATTACK_MS / 1000.0) as usize;
    let body = (RATE * BODY_MS / 1000.0) as usize;

    let mut ratios = Vec::new();
    let mut start = 0;
    while start + attack + body < signal.len() {
        let head = rms(&signal[start..start + attack]);
        let rest = rms(&signal[start + attack..start + attack + body]);
        if head > 0.0 && rest > 0.0 {
            ratios.push(20.0 * (head / rest).log10());
        }
        start += period;
    }
    ratios.iter().sum::<f32>() / ratios.len() as f32
}

fn run(punch: f32) -> (f32, f32) {
    let dry = bursts();
    let mut engine = Engine::new(RATE);
    engine.set_shape(&Shape {
        punch,
        ..Shape::default()
    });
    let levels = Levels {
        spark: 1.0,
        mix: 1.0,
        ..Levels::default()
    };

    let wet: Vec<f32> = dry
        .iter()
        .map(|sample| engine.process((*sample, *sample), &levels).0)
        .collect();
    (
        attack_over_body_db(&wet),
        20.0 * (rms(&wet) / rms(&dry)).log10(),
    )
}

/// **The acceptance condition.** More `PUNCH`, more distance between the attack
/// and the body — every step of the way, not just at the ends.
#[test]
fn punch_raises_the_attack_over_the_body() {
    println!("\n  PUNCH  attack    level");
    let mut previous = f32::MIN;
    for punch in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let (attack, level) = run(punch);
        println!("  {punch:5.2} {attack:7.2} {level:8.2}");
        assert!(attack > previous, "PUNCH {punch} did not lift the attack");
        previous = attack;
    }

    let (flat, _) = run(0.0);
    let (hit, _) = run(1.0);
    assert!(
        hit - flat > 1.0,
        "PUNCH moved the attack by {:.2} dB",
        hit - flat
    );
}

/// **And it is not just a level.** Something that made everything louder would
/// move the body as much as the attack and leave the distance alone — this is
/// the assertion that tells the two apart.
#[test]
fn punch_is_not_a_gain() {
    let (flat_attack, flat_level) = run(0.0);
    let (hit_attack, hit_level) = run(1.0);
    println!(
        "\n  attack {:+.2} dB, level {:+.2} dB",
        hit_attack - flat_attack,
        hit_level - flat_level
    );
    assert!(
        hit_attack - flat_attack > (hit_level - flat_level).abs(),
        "the attack moved less than the level did"
    );
}

/// **`SPARK` = 0 is exactly nothing**, `PUNCH` included (`REQ-SPK-009`). It
/// rides on `SPARK` for the same reason the protections do: a macro that acted
/// with the dynamics turned off would make zero a setting rather than an off.
#[test]
fn spark_zero_is_still_nothing() {
    let dry = bursts();
    let mut engine = Engine::new(RATE);
    engine.set_shape(&Shape {
        punch: 1.0,
        ..Shape::default()
    });
    let levels = Levels {
        spark: 0.0,
        mix: 1.0,
        ..Levels::default()
    };

    for sample in &dry {
        let (left, _) = engine.process((*sample, *sample), &levels);
        assert!(left.is_finite());
    }

    let quiet = Levels {
        spark: 0.0,
        ..levels
    };
    let mut with_punch = Engine::new(RATE);
    with_punch.set_shape(&Shape {
        punch: 1.0,
        ..Shape::default()
    });
    let mut without = Engine::new(RATE);
    without.set_shape(&Shape::default());

    for sample in &dry {
        let hit = with_punch.process((*sample, *sample), &quiet).0;
        let flat = without.process((*sample, *sample), &quiet).0;
        assert_eq!(hit, flat, "PUNCH acted with SPARK at zero");
    }
}
