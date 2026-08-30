//! Where the effect stops growing (`SPK-20`).
//!
//! **The unit that has to run before `MODE` can be designed.** `MODE` lifts the
//! mapping from the macros into the internals; if what is holding the effect
//! back is not the mapping, lifting it changes nothing. The complaint that
//! started this — "it does not do enough" — is an ear's, and the only road from
//! an ear to an implementation is a measurement (`VDP-14` is the same shape).
//!
//! Run it and read the numbers:
//!
//! ```text
//! cargo test -p sparkleur-core --test ceiling -- --nocapture
//! ```

use nxe_audio::harmonics::{at_dbfs, pink, rms};
use sparkleur_core::crossover::{BAND_COUNT, EDGES};
use sparkleur_core::dynamics;
use sparkleur_core::engine::{Engine, Levels, Shape};

const RATE: f32 = 48_000.0;
/// Where a mixed vocal sits, which is where the plugin is used.
const REFERENCE_DBFS: f32 = -18.0;
const SECONDS: f32 = 2.0;

/// What one setting of the plugin did to one signal.
#[derive(Debug, Clone, Copy)]
struct Reach {
    /// The most any band was pushed down, and the most any band was lifted.
    down_db: f32,
    up_db: f32,
    /// The most De-Harsh pulled back, in dB.
    de_harsh_db: f32,
    /// How far the Sparkle gate opened, averaged.
    opening: f32,
    /// **What came out, against what went in.** Not the sample-wise difference:
    /// this is a split-band design, so `wet - dry` is dominated by the
    /// crossover's phase rotation and reads the same whatever the dynamics do
    /// (it measured 1.65 dB for every setting, including the ones that were
    /// provably doing nothing).
    level_db: f32,
    /// Energy above the top edge, against the dry's — where Sparkle puts what
    /// it generates.
    top_db: f32,
}

fn run(shape: &Shape, levels: &Levels) -> Reach {
    run_at(shape, levels, REFERENCE_DBFS)
}

fn run_at(shape: &Shape, levels: &Levels, dbfs: f32) -> Reach {
    let length = (RATE * SECONDS) as usize;
    run_signal(shape, levels, &at_dbfs(pink(1.0, length), dbfs))
}

fn run_signal(shape: &Shape, levels: &Levels, dry: &[f32]) -> Reach {
    let length = dry.len();

    let mut engine = Engine::new(RATE);
    engine.set_shape(shape);

    let mut wet = Vec::with_capacity(length);
    let mut down_db = 0.0f32;
    let mut up_db = 0.0f32;
    let mut de_harsh_db = 0.0f32;
    let mut opening = 0.0f64;

    for (index, sample) in dry.iter().enumerate() {
        let (left, _) = engine.process((*sample, *sample), levels);
        wet.push(left);

        // The first tenth is the followers settling, not the plugin working.
        if index > length / 10 {
            for gain in engine.gains_db() {
                down_db = down_db.min(gain);
                up_db = up_db.max(gain);
            }
            de_harsh_db = de_harsh_db.min(engine.de_harsh_db());
            opening += f64::from(engine.sparkle_opening());
        }
    }

    let settled = (length - length / 10) as f64;
    Reach {
        down_db,
        up_db,
        de_harsh_db,
        opening: (opening / settled) as f32,
        level_db: 20.0 * (rms(&wet) / rms(dry)).log10(),
        top_db: 20.0 * (high(&wet) / high(dry)).log10(),
    }
}

/// The energy above the top crossover edge, as an RMS. A one-pole high-pass is
/// enough: what is being compared is two runs of the same signal, so the
/// filter's own shape cancels.
fn high(signal: &[f32]) -> f32 {
    let cutoff = EDGES[BAND_COUNT - 2];
    let coefficient = (-std::f32::consts::TAU * cutoff / RATE).exp();
    let mut low = 0.0;
    let filtered: Vec<f32> = signal
        .iter()
        .map(|sample| {
            low = low * coefficient + sample * (1.0 - coefficient);
            sample - low
        })
        .collect();
    rms(&filtered)
}

/// Everything at its maximum: this is the loudest the plugin can be asked to be.
fn flat_out(character: f32) -> Shape {
    Shape {
        character,
        lift: 1.0,
        ..Shape::default()
    }
}

fn flat_out_in(character: f32, mode: dynamics::Mode) -> Shape {
    Shape {
        mode,
        ..flat_out(character)
    }
}

fn all_in() -> Levels {
    Levels {
        spark: 1.0,
        mix: 1.0,
        ..Levels::default()
    }
}

/// **The measurement `SPK-21` is designed from.** It asserts almost nothing on
/// purpose — what it produces is a table, and the table goes in the plan.
#[test]
fn where_the_ceiling_is() {
    println!("\n  CHARACTER   down     up   de-harsh  opening    level     top");
    for (name, character) in [("POLISH", 0.0), ("GLOSS", 0.5), ("CRUSH", 1.0)] {
        let reach = run(&flat_out(character), &all_in());
        println!(
            "  {name:9} {:6.2} {:6.2} {:9.2} {:8.3} {:8.2} {:7.2}",
            reach.down_db,
            reach.up_db,
            reach.de_harsh_db,
            reach.opening,
            reach.level_db,
            reach.top_db
        );
    }
}

/// **What the detector actually reads**, which is where a threshold has to be
/// placed. The thresholds are numbers on this scale, not on the input's: a
/// signal at −18 dBFS split five ways puts every band a long way below it.
#[test]
fn where_the_bands_sit() {
    println!("\n   input     SUB    BODY     MID    PRES     AIR");
    for dbfs in [-30.0, -24.0, -18.0, -12.0, -6.0] {
        let length = (RATE * SECONDS) as usize;
        let dry = at_dbfs(pink(1.0, length), dbfs);
        let mut engine = Engine::new(RATE);
        engine.set_shape(&flat_out(0.5));
        let levels = all_in();

        let mut sums = [0.0f64; BAND_COUNT];
        let settled = length - length / 10;
        for (index, sample) in dry.iter().enumerate() {
            engine.process((*sample, *sample), &levels);
            if index > length / 10 {
                for (sum, level) in sums.iter_mut().zip(engine.levels_db()) {
                    *sum += f64::from(level);
                }
            }
        }
        print!("  {dbfs:6.1}");
        for sum in sums {
            print!(" {:7.2}", sum / settled as f64);
        }
        println!();
    }
    println!(
        "  thresholds: down {:.1}, up {:.1}, floor {:.1}",
        dynamics::DOWN_THRESHOLD_DB,
        dynamics::UP_THRESHOLD_DB,
        dynamics::FLOOR_DB
    );
}

/// **Where the dynamics engage at all.** The thresholds are fixed points on the
/// detector's scale, so how hard the plugin works is a question about the
/// material's level as much as about the settings.
#[test]
fn across_the_input_level() {
    // **The defect, pinned.** At the level a mixed vocal sits at, everything at
    // maximum moves the output by less than half a decibel. `SPK-21` is
    // expected to break this assertion — that is what it is for. A number that
    // has to be edited is a decision; a number that quietly drifts is not.
    let at_reference = run_at(&flat_out(0.5), &all_in(), -18.0);
    assert!(
        at_reference.level_db.abs() < 0.5,
        "the reference case moved: {:.2} dB",
        at_reference.level_db
    );

    println!("\n   input   down     up   opening    level     top");
    for dbfs in [-42.0, -36.0, -30.0, -24.0, -18.0, -12.0, -6.0] {
        let reach = run_at(&flat_out(0.5), &all_in(), dbfs);
        println!(
            "  {dbfs:6.1} {:6.2} {:6.2} {:8.3} {:8.2} {:7.2}",
            reach.down_db, reach.up_db, reach.opening, reach.level_db, reach.top_db
        );
    }
}

/// **What `MODE` bought.** The same sweep as `across_the_input_level`, in both
/// modes — the pair is the acceptance condition for `SPK-21`.
#[test]
fn what_hard_reaches() {
    println!("\n   input      soft down/up/level        hard down/up/level");
    for dbfs in [-30.0, -24.0, -18.0, -12.0, -6.0] {
        let soft = run_at(&flat_out_in(0.5, dynamics::Mode::Soft), &all_in(), dbfs);
        let hard = run_at(&flat_out_in(0.5, dynamics::Mode::Hard), &all_in(), dbfs);
        println!(
            "  {dbfs:6.1}   {:6.2} {:6.2} {:6.2}      {:6.2} {:6.2} {:6.2}",
            soft.down_db, soft.up_db, soft.level_db, hard.down_db, hard.up_db, hard.level_db
        );
    }

    // **The acceptance condition for `SPK-21`.** At the level the plugin is
    // used at, `Hard` has to reach materially further than `Soft` on both
    // sides, and it has to do it without turning into a loudness change — a
    // plugin that mostly makes things quieter reads as worse, not as more.
    let soft = run_at(&flat_out_in(0.5, dynamics::Mode::Soft), &all_in(), -18.0);
    let hard = run_at(&flat_out_in(0.5, dynamics::Mode::Hard), &all_in(), -18.0);
    let reach = |r: &Reach| r.up_db - r.down_db;
    assert!(
        reach(&hard) - reach(&soft) > 5.0,
        "Hard reaches {:.2} dB against Soft's {:.2}",
        reach(&hard),
        reach(&soft)
    );
    assert!(
        hard.level_db.abs() < 1.0,
        "Hard moved the output by {:.2} dB",
        hard.level_db
    );
}

/// **The diagnostic.** If turning the protections down makes the effect
/// materially bigger, the ceiling is the guard and `MODE` has to reach it. If
/// it does not, the ceiling is the mapping and `MODE` can lift that alone.
#[test]
fn is_the_guard_the_ceiling() {
    let guarded = run(&flat_out(0.5), &all_in());
    let unguarded = run(
        &Shape {
            de_harsh: -1.0,
            sub_protect: -1.0,
            ..flat_out(0.5)
        },
        &all_in(),
    );

    println!(
        "\n  guard on:  level {:6.2} dB, top {:6.2} dB",
        guarded.level_db, guarded.top_db
    );
    println!(
        "  guard off: level {:6.2} dB, top {:6.2} dB",
        unguarded.level_db, unguarded.top_db
    );

    // **It is not.** Turning both protections all the way down changes nothing
    // at the level the plugin is used at, so `MODE` has no business reaching
    // them — which is what `REQ-SPK-022` promises.
    assert!(
        (unguarded.level_db - guarded.level_db).abs() < 0.1,
        "the guard is holding {:.2} dB back",
        unguarded.level_db - guarded.level_db
    );
}

/// And the other half: how much of the reach is `LIFT`, which is the one macro
/// whose whole job is to open the upward side further.
#[test]
fn how_much_of_it_is_lift() {
    println!("\n  LIFT sweep, at two levels");
    for dbfs in [-36.0, -18.0] {
        for lift in [0.0, 0.5, 1.0] {
            let reach = run_at(
                &Shape {
                    lift,
                    ..flat_out(0.5)
                },
                &all_in(),
                dbfs,
            );
            println!(
                "  {dbfs:6.1} dBFS  LIFT {lift:.1}: up {:6.2} dB, level {:6.2} dB",
                reach.up_db, reach.level_db
            );
        }
    }
}

/// **On stationary noise the gate has nothing to open for.** That is the
/// innocent explanation for a reading that never moves, and it has to be ruled
/// out before the reading is called broken: the material below has transients
/// in it, so a gate that works has to move here.
#[test]
fn does_the_sparkle_gate_move() {
    println!("\n  SNAP, on stationary noise and on bursts");
    for snap in [0.0, 0.5, 1.0] {
        let steady = run_at(
            &Shape {
                snap,
                ..flat_out(0.5)
            },
            &all_in(),
            -18.0,
        );
        let bursty = run_signal(
            &Shape {
                snap,
                ..flat_out(0.5)
            },
            &all_in(),
            &bursts(-18.0),
        );
        println!(
            "  SNAP {snap:.1}: steady opening {:8.3}   bursts opening {:8.3}",
            steady.opening, bursty.opening
        );

        // **The gate works, and `SNAP` is not what opens it.** `SNAP` decides
        // how much of the layer is handed to the transient; the detector is
        // what opens. A reading that never moves on stationary noise is the
        // detector being right, not the reading being broken.
        assert!(
            bursty.opening > steady.opening + 0.1,
            "the gate did not open on bursts"
        );
    }
}

/// **The floor only bites on material quiet enough to reach it.** `LIFT` moves
/// it, so a sweep at the level a vocal sits at says nothing about whether
/// `LIFT` is wired.
#[test]
fn does_lift_reach_the_floor() {
    println!("\n  LIFT, far below the reference");
    for dbfs in [-72.0, -60.0, -48.0] {
        for lift in [0.0, 1.0] {
            let reach = run_at(
                &Shape {
                    lift,
                    ..flat_out(0.5)
                },
                &all_in(),
                dbfs,
            );
            println!(
                "  {dbfs:6.1} dBFS  LIFT {lift:.1}: up {:6.2} dB, level {:6.2} dB",
                reach.up_db, reach.level_db
            );
        }
    }

    // **`LIFT` is wired.** It is simply below the material: the floor it opens
    // sits far under where a vocal lives, so a sweep at the reference level
    // says nothing about it.
    // `flat_out` already opens `LIFT` all the way, so the closed case has to
    // say so — comparing it against itself is how this assertion first read
    // "LIFT moved the floor by 0.00 dB" about a `LIFT` that works.
    let shut = run_at(
        &Shape {
            lift: 0.0,
            ..flat_out(0.5)
        },
        &all_in(),
        -72.0,
    );
    let opened = run_at(&flat_out(0.5), &all_in(), -72.0);
    assert!(
        opened.up_db - shut.up_db > 5.0,
        "LIFT moved the floor by {:.2} dB",
        opened.up_db - shut.up_db
    );
}

/// Pink noise in bursts: a tenth of a second on, a tenth off. Something for a
/// transient detector to find.
fn bursts(dbfs: f32) -> Vec<f32> {
    let length = (RATE * SECONDS) as usize;
    let period = (RATE * 0.2) as usize;
    let mut signal = at_dbfs(pink(1.0, length), dbfs);
    for (index, sample) in signal.iter_mut().enumerate() {
        if index % period > period / 2 {
            *sample = 0.0;
        }
    }
    signal
}
