//! Where the effect stops growing (`VEL-18`).
//!
//! **The unit that has to run before `MODE` can be designed.** `MODE` lifts the
//! mapping from the macros into the internals; if what is holding the effect
//! back is not the mapping, lifting it changes nothing. Sparkleur asked the
//! same question in `sparkleur-core/tests/ceiling.rs` and the answer was not
//! the one the plan expected — three suspects turned out to be innocent.
//!
//! **Velour's reach is not a gain.** Nothing here compresses the signal the
//! listener hears: three generators make harmonics out of the input and the
//! result is *added* to an untouched dry path (`REQ-VEL-001`). So the quantity
//! that stands for "how much it does" is the part of the added layer that was
//! not in the input — its harmonics, against the dry. A layer that is loud but
//! is only a band-limited copy of the input is a volume change wearing the
//! fader's label.
//!
//! Run it and read the numbers:
//!
//! ```text
//! cargo test -p velour-core --test ceiling -- --nocapture --test-threads=1
//! ```
//!
//! `--test-threads=1`, or the tables interleave and none of them can be read.

use nxe_audio::harmonics::{amplitude, at_dbfs, bin_of, db_ratio, pink, rms, tone};
use velour_core::bands::Mode;
use velour_core::bands::{AIR_INPUT_CEILING, Generator};
use velour_core::engine::{BAND_COUNT, Engine, Levels, Shape};
use velour_core::{BANDS, Band};

const RATE: f32 = 48_000.0;
/// Where a mixed vocal sits, which is where the plugin is used. The same number
/// as `velour_core::envelope::REFERENCE_DB`, and as Sparkleur's, on purpose.
const REFERENCE_DBFS: f32 = -18.0;
const SECONDS: f32 = 2.0;
/// What the wrapper hands `set_shape` between calls to `process`.
const BLOCK: usize = 64;

/// Where each generator is asked to work — the middle of its input band, so the
/// reading is of the band doing its job rather than of its skirt.
const TONES: [f32; BAND_COUNT] = [220.0, 1_500.0, 6_000.0];
const NAMES: [&str; BAND_COUNT] = ["BODY", "PRESENCE", "AIR"];

/// How many harmonics the reading sums. See [`Run::made_db`].
const HARMONICS: usize = 20;

/// One setting of the plugin, run against one signal.
struct Run {
    rate: f32,
    dry: Vec<f32>,
    /// The generator bus alone, before `MIX` — what is being added.
    ///
    /// Read from [`Engine::wet`] rather than reconstructed as `output − dry`:
    /// the dry is the larger of the two here, and subtracting it throws away
    /// most of the layer's precision. That trap has already cost this crate a
    /// unit (`velour-plan.md`, `VEL-9`).
    layer: Vec<f32>,
    output: Vec<f32>,
    /// The deepest each guard pulled, in dB, in the order of
    /// `velour_core::guard::GUARDS`. Zero is a guard that never fired.
    guard_db: [f32; 2],
}

impl Run {
    fn layer_db(&self) -> f32 {
        db_ratio(rms(&self.layer), rms(&self.dry))
    }

    fn output_db(&self) -> f32 {
        db_ratio(rms(&self.output), rms(&self.dry))
    }

    /// **The measurement this unit is about.** What the curve made that was not
    /// in the tone that went in, against the dry.
    ///
    /// **Summed bin by bin rather than as `layer² − fundamental²`.** The
    /// subtraction is the obvious form and it is two nearly equal numbers: the
    /// first version of this file read −49.9 dB at drive 0 and −200 dB at drive
    /// 0.25, which is not a curve making fewer harmonics, it is `f32`
    /// cancellation. Twenty harmonics is past every band's output filter —
    /// BODY keeps nothing above 2 kHz, AIR nothing above 20.
    /// **The harmonic's bin is the fundamental's bin times `n`**, not the
    /// frequency times `n` rounded again. `tone` rounds the frequency it is
    /// given to a whole number of cycles, and rounding `n·hz` separately lands
    /// one bin beside that whenever the two roundings disagree — where the DFT
    /// reads nothing at all, because whole cycles leak into no neighbour. That
    /// is what put −95 dB against an AIR band that was working; the frequencies
    /// it happened at were the ones with a fractional cycle count.
    fn made_db(&self, hz: f32) -> f32 {
        let length = self.layer.len();
        let fundamental = bin_of(hz, self.rate, length);
        let mut power = 0.0f32;
        for harmonic in 2..=HARMONICS {
            let bin = fundamental * harmonic;
            if bin >= length / 2 {
                break;
            }
            let level = amplitude(&self.layer, bin);
            power += level * level * 0.5;
        }
        db_ratio(power.sqrt(), rms(&self.dry))
    }

    /// **How much of the layer is the effect.** The harmonics against the layer
    /// they arrive in; the rest of it is the band-limited copy.
    fn made_of_layer_db(&self, hz: f32) -> f32 {
        self.made_db(hz) - self.layer_db()
    }
}

fn measure(shape: &Shape, levels: &Levels, dry: Vec<f32>) -> Run {
    measure_at(RATE, shape, levels, dry)
}

fn measure_at(rate: f32, shape: &Shape, levels: &Levels, dry: Vec<f32>) -> Run {
    let mut engine = Engine::new(rate);
    let mut layer = Vec::with_capacity(dry.len());
    let mut output = Vec::with_capacity(dry.len());
    let mut guard_db = [0.0f32; 2];

    // **Twice through, recording the second.** Every detector here is
    // recursive, and a reading taken while they are still settling is a reading
    // of the settling. Dropping the first tenth instead — which is what
    // Sparkleur's does — would break the whole number of cycles the harmonic
    // measurement depends on; a second pass over a buffer holding whole cycles
    // continues the first without a seam.
    for pass in 0..2 {
        for block in dry.chunks(BLOCK) {
            // Once per block, which is where the wrapper calls it. `EMOTION` is
            // resolved here, so calling it once for the whole run would measure
            // a plugin whose envelope never moved.
            engine.set_shape(shape);
            for sample in block {
                let out = engine.process((*sample, *sample), levels);
                if pass == 1 {
                    layer.push(engine.wet().0);
                    output.push(out.0);
                    for (deepest, now) in guard_db.iter_mut().zip(engine.guard_reductions()) {
                        *deepest = deepest.min(now);
                    }
                }
            }
        }
    }

    Run {
        rate,
        dry,
        layer,
        output,
        guard_db,
    }
}

/// Everything the macros can be asked for: the drive at its top, every fader
/// open, `MIX` at 100. This is the loudest and the dirtiest the plugin can be.
fn flat_out(texture: f32) -> Shape {
    Shape {
        drive: 1.0,
        texture,
        ..Shape::default()
    }
}

fn flat_out_in(texture: f32, mode: Mode) -> Shape {
    Shape {
        mode,
        ..flat_out(texture)
    }
}

/// One band's fader open and the other two shut, so the layer that comes back
/// is that generator's alone.
///
/// Not `solo`, which also mutes the dry path — the output reading has to stay
/// comparable across bands.
fn only(band: usize) -> Levels {
    Levels {
        bands: std::array::from_fn(|index| if index == band { 1.0 } else { 0.0 }),
        mix: 1.0,
    }
}

fn all_in() -> Levels {
    Levels {
        bands: [1.0; BAND_COUNT],
        mix: 1.0,
    }
}

fn tone_at(band: usize, dbfs: f32) -> Vec<f32> {
    at_dbfs(tone(1.0, TONES[band], RATE, length_of(RATE)), dbfs)
}

fn pink_at(dbfs: f32) -> Vec<f32> {
    at_dbfs(pink(1.0, length_of(RATE)), dbfs)
}

fn length_of(rate: f32) -> usize {
    (rate * SECONDS) as usize
}

/// **The measurement `VEL-19` is designed from.** It asserts one thing and
/// prints the rest — what it produces is a table, and the table goes in the
/// plan.
#[test]
fn what_drive_reaches() {
    println!("\n  band       DRIVE    layer     made   of layer   output");
    for band in 0..BAND_COUNT {
        for drive in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let shape = Shape {
                drive,
                ..flat_out(0.5)
            };
            let run = measure(&shape, &only(band), tone_at(band, REFERENCE_DBFS));
            println!(
                "  {:9} {drive:5.2} {:8.2} {:8.2} {:10.2} {:8.2}",
                NAMES[band],
                run.layer_db(),
                run.made_db(TONES[band]),
                run.made_of_layer_db(TONES[band]),
                run.output_db()
            );
        }
    }

    // **The defect, pinned.** With one band's fader at its top, its drive at
    // its top and `MIX` at 100 — the most the plugin can be asked for — the
    // layer it adds is still more than nine parts copy to one part harmonics.
    // The rest of that layer is a band-limited duplicate of the input, so most
    // of what the fader adds is level, and level is what the listener takes
    // back out with `OUTPUT`.
    //
    // `VEL-19` is expected to break this assertion. A number that has to be
    // edited is a decision; a number that quietly drifts is not.
    for (band, hz) in TONES.iter().enumerate() {
        let run = measure(&flat_out(0.5), &only(band), tone_at(band, REFERENCE_DBFS));
        let fraction = run.made_of_layer_db(*hz);
        assert!(
            fraction < -12.0,
            "{}: the layer is {fraction:.2} dB harmonics",
            NAMES[band]
        );
    }
}

/// The whole plugin, on material rather than on a tone: what the layer is worth
/// against the dry at each end of `TEXTURE`, and what the output does.
#[test]
fn what_the_layer_is_worth() {
    println!("\n  TEXTURE      layer   output   harsh     sib");
    for (name, texture) in [("WARM", 0.0), ("CLEAR", 0.5), ("EDGE", 1.0)] {
        let run = measure(&flat_out(texture), &all_in(), pink_at(REFERENCE_DBFS));
        println!(
            "  {name:9} {:8.2} {:8.2} {:7.2} {:7.2}",
            run.layer_db(),
            run.output_db(),
            run.guard_db[0],
            run.guard_db[1]
        );
    }
}

/// **The first suspect** (`velour-plan.md`): `VEL-3` put a ceiling on what
/// reaches AIR's curve, and it is a fraction of the host's rate rather than a
/// frequency — so what it costs depends on the rate and on `FOCUS`.
///
/// The two runs are the same band asked for the same thing at two rates. At
/// 48 kHz the ceiling closes AIR's input at 12 kHz; at 96 kHz it opens to 24
/// and the band's own edge is what is left. Anything the ceiling costs shows up
/// as the difference.
#[test]
fn what_the_air_ceiling_costs() {
    println!("\n  the AIR generator's input band, in Hz");
    for rate in [44_100.0f32, 48_000.0, 96_000.0] {
        print!("  {rate:8.0}");
        for focus in [-1.0f32, 0.0, 1.0] {
            let (low, high) = Generator::input_range(Band::Air, focus, rate);
            print!("   FOCUS {focus:+.0}: {low:6.0}–{high:6.0}");
        }
        println!("   ceiling {:6.0}", rate * AIR_INPUT_CEILING);
    }

    println!("\n  AIR at FOCUS +1, where the ceiling is the edge that binds");
    let mut made = Vec::new();
    for rate in [48_000.0f32, 96_000.0] {
        let shape = Shape {
            focus: 1.0,
            ..flat_out(0.5)
        };
        // Inside the band at both rates, so what moves is the ceiling and not
        // whether the tone is being listened to at all.
        let hz = 10_000.0;
        let dry = at_dbfs(tone(1.0, hz, rate, length_of(rate)), REFERENCE_DBFS);
        let run = measure_at(rate, &shape, &only(2), dry);
        println!(
            "  {rate:8.0}: layer {:6.2} dB, made {:6.2} dB",
            run.layer_db(),
            run.made_db(hz)
        );
        made.push(run.made_db(hz));
    }
    println!("  the ceiling costs {:.2} dB there", made[1] - made[0]);

    println!("\n  AIR across FOCUS, at 48 kHz, tone following the band");
    for focus in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
        let shape = Shape {
            focus,
            ..flat_out(0.5)
        };
        let (low, high) = Generator::input_range(Band::Air, focus, RATE);
        let hz = (low * high).sqrt();
        let dry = at_dbfs(tone(1.0, hz, RATE, length_of(RATE)), REFERENCE_DBFS);
        let run = measure_at(RATE, &shape, &only(2), dry);
        println!(
            "  FOCUS {focus:+.1} ({hz:6.0} Hz): layer {:6.2} dB, made {:6.2} dB",
            run.layer_db(),
            run.made_db(hz)
        );
    }

    // **At the rate and the setting the plugin is used at, the ceiling removes
    // nothing.** `AIR_INPUT_CEILING` is `0.25`, and at 48 kHz that is 12 kHz —
    // exactly the band's own upper edge. It binds only above `FOCUS` 0, and
    // only because `FOCUS` is trying to move the band past it.
    let (_, high) = Generator::input_range(Band::Air, 0.0, RATE);
    assert_eq!(
        high,
        RATE * AIR_INPUT_CEILING,
        "the ceiling and the band's own edge have parted"
    );
}

/// **The second suspect.** If turning the protections off makes the layer
/// materially bigger, the ceiling is the guard and `MODE` has to reach it. If
/// it does not, the ceiling is upstream and `MODE` can leave the guards alone —
/// which is what `REQ-VEL-006` would rather it did.
#[test]
fn are_the_guards_the_ceiling() {
    let guarded = measure(&flat_out(1.0), &all_in(), pink_at(REFERENCE_DBFS));
    let unguarded = measure(
        &Shape {
            guards: [0.0; 2],
            ..flat_out(1.0)
        },
        &all_in(),
        pink_at(REFERENCE_DBFS),
    );

    println!(
        "\n  guards on:  layer {:6.2} dB, output {:6.2} dB, pulls {:5.2} / {:5.2}",
        guarded.layer_db(),
        guarded.output_db(),
        guarded.guard_db[0],
        guarded.guard_db[1]
    );
    println!(
        "  guards off: layer {:6.2} dB, output {:6.2} dB",
        unguarded.layer_db(),
        unguarded.output_db()
    );
    println!(
        "  the guards are holding {:.2} dB of layer back",
        unguarded.layer_db() - guarded.layer_db()
    );
}

/// **The third suspect: the normalisation.** `nxe_audio::shaper` divides the
/// curve out by its RMS gain at a fixed amplitude, which is what keeps `DRIVE`
/// from being a volume knob — and it is exact only at that amplitude. How far
/// the effect reaches is therefore a question about the material's level as
/// much as about the settings.
#[test]
fn across_the_input_level() {
    println!("\n   input     BODY made   PRESENCE made    AIR made      layer");
    for dbfs in [-42.0f32, -30.0, -24.0, -18.0, -12.0, -6.0] {
        print!("  {dbfs:6.1}");
        for (band, hz) in TONES.iter().enumerate() {
            let run = measure(&flat_out(0.5), &only(band), tone_at(band, dbfs));
            print!("     {:9.2}", run.made_db(*hz));
        }
        let whole = measure(&flat_out(0.5), &all_in(), pink_at(dbfs));
        println!("   {:8.2}", whole.layer_db());
    }
}

/// And what `DENSITY` buys, which is the documented answer to the drift above:
/// it brings the bus toward a consistent level before the curve sees it
/// (`REQ-VEL-007`). Its makeup is referenced to a vocal, so the reference level
/// is the pivot and everything else moves toward it.
#[test]
fn what_density_buys() {
    println!("\n   input   DENSITY 0 made   DENSITY 1 made");
    for dbfs in [-30.0f32, -18.0, -6.0] {
        print!("  {dbfs:6.1}");
        for density in [0.0, 1.0] {
            let shape = Shape {
                density,
                ..flat_out(0.5)
            };
            let run = measure(&shape, &only(1), tone_at(1, dbfs));
            print!("        {:9.2}", run.made_db(TONES[1]));
        }
        println!();
    }
}

/// The bands are a list in two places, and a reading labelled with the wrong
/// one would be worse than no reading.
#[test]
fn the_tones_sit_in_the_bands_they_name() {
    for (index, band) in BANDS.iter().enumerate() {
        let (low, high) = Generator::input_range(*band, 0.0, RATE);
        assert!(
            (low..=high).contains(&TONES[index]),
            "{}: {} is not inside {low}–{high}",
            NAMES[index],
            TONES[index]
        );
    }
}

/// **What `MODE` bought** (`VEL-19`). The pair is the acceptance condition.
///
/// `Hard` swaps `shape` for `residual` in every generator: the same curve with
/// its pass-through taken out and the rest normalised back up
/// (`nxe_audio::shaper`). What has to come out of that is more harmonics at the
/// same level — more at a *higher* level would be a volume knob, which is the
/// thing `VEL-18` found the plugin already had.
#[test]
fn what_hard_reaches() {
    println!("\n  band       soft layer / made / of layer      hard layer / made / of layer");
    for (band, hz) in TONES.iter().enumerate() {
        let soft = measure(&flat_out(0.5), &only(band), tone_at(band, REFERENCE_DBFS));
        let hard = measure(
            &flat_out_in(0.5, Mode::Hard),
            &only(band),
            tone_at(band, REFERENCE_DBFS),
        );
        println!(
            "  {:9} {:8.2} {:8.2} {:9.2}      {:8.2} {:8.2} {:9.2}",
            NAMES[band],
            soft.layer_db(),
            soft.made_db(*hz),
            soft.made_of_layer_db(*hz),
            hard.layer_db(),
            hard.made_db(*hz),
            hard.made_of_layer_db(*hz),
        );

        // **More harmonics, against the signal they are added to.** Eight
        // decibels is the floor; the shaper's own measurement says nine is what
        // the pass-through was worth at the top of the drive range.
        let gained = hard.made_db(*hz) - soft.made_db(*hz);
        assert!(
            gained > 8.0,
            "{}: Hard only gained {gained:.2} dB of harmonics",
            NAMES[band]
        );

        // **And not by being louder.** A mode that reached further by pushing
        // the output up would be undone by `OUTPUT`, which is exactly how the
        // effect came to feel weak in the first place (`VEL-18`: one band at
        // its top raised the output 5.7 dB and almost all of it was a copy of
        // the input). `Hard` has to be **at most** as loud as `Soft`.
        //
        // The layer itself comes out *quieter* — the pass-through it dropped
        // was most of it, and the harmonics that replace it lose whatever the
        // band's output filter does not keep. That is the mode working, not a
        // shortfall: what is left is all texture.
        let louder = hard.output_db() - soft.output_db();
        assert!(
            louder < 0.5,
            "{}: Hard raised the output by {louder:.2} dB",
            NAMES[band]
        );
    }
}

/// **`Soft` is the arithmetic that shipped.** Not close to it — the same
/// samples, because it is the same call on the same shaper (`REQ-VEL-021`).
#[test]
fn soft_is_what_v0_1_4_did() {
    for (band, name) in NAMES.iter().enumerate() {
        let dry = tone_at(band, REFERENCE_DBFS);
        let explicit = measure(&flat_out_in(0.5, Mode::Soft), &only(band), dry.clone());
        let default = measure(&flat_out(0.5), &only(band), dry);
        assert_eq!(explicit.layer, default.layer, "{name}");
        assert_eq!(explicit.output, default.output, "{name}");
    }
}
