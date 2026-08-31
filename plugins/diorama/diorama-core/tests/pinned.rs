//! What `DIO-8` pins: block size, sample rate, hostile values and seams.
//!
//! These are the promises that hold the whole engine together rather than any
//! one stage, so they live outside the modules that make them
//! (`REQ-DIO-016`, `REQ-DIO-017`).

use diorama_core::{Engine, Macros};
use nxe_audio::harmonics;

const RATES: [f32; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];
const RATE: f32 = 48_000.0;

/// Everything turned on, so no stage is skipped by an early return.
fn working() -> Macros {
    Macros {
        depth: 0.7,
        direct: 0.6,
        room: 0.8,
        damping: 0.6,
        width: 0.8,
        clarity: 0.5,
        mix: 0.9,
        output: 1.0,
    }
}

/// Runs `input` through in blocks of `block`, the way a host would.
fn rendered(rate: f32, block: usize, input: &[f32]) -> Vec<f32> {
    let mut engine = Engine::new(rate);
    engine.set(working());
    engine.reset();

    let mut output = Vec::with_capacity(input.len());
    for chunk in input.chunks(block) {
        // **Set per block, the way a host drives it** (`VEL-6` measured
        // `EMOTION` backwards by setting once before the first sample).
        engine.set(working());
        for &sample in chunk {
            output.push(engine.process(sample, sample * 0.5).0);
        }
    }
    output
}

/// **The output does not depend on the host's buffer size** (`REQ-DIO-017`).
///
/// Bit-identical rather than close: with the parameters still, every smoother
/// is settled and the only thing a block boundary could change is a rebuild
/// that should not be happening.
#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let input = harmonics::pink(0.4, 24_000);
    let reference = rendered(RATE, 512, &input);

    for block in [1usize, 64, 4_096] {
        let measured = rendered(RATE, block, &input);
        for index in 0..input.len() {
            assert_eq!(
                measured[index], reference[index],
                "block {block} differs from 512 at sample {index}"
            );
        }
    }
}

/// Every rate produces a working, finite, comparable engine (`REQ-DIO-017`).
///
/// **Noise, not a tone.** The reflection delays are fixed in samples, so a
/// steady tone's phase relationships rotate with the rate and the measurement
/// drifts for a reason that is not a bug (`SPK-9`).
#[test]
fn no_rate_makes_the_coefficients_diverge() {
    let mut energies = Vec::new();
    for rate in RATES {
        let input = harmonics::pink(0.4, rate as usize);
        let output = rendered(rate, 512, &input);
        assert!(
            output.iter().all(|sample| sample.is_finite()),
            "rate {rate} produced a non-finite sample"
        );
        let energy: f32 = output.iter().map(|s| s * s).sum::<f32>() / output.len() as f32;
        energies.push((rate, energy));
    }

    let reference = energies[1].1;
    for (rate, energy) in &energies {
        let difference = 10.0 * (energy / reference).log10();
        assert!(
            difference.abs() < 1.0,
            "rate {rate}: {difference:.2} dB against 48 kHz"
        );
    }
}

/// **Every parameter at every extreme, and every sample hostile**
/// (`REQ-DIO-016`). Nothing non-finite comes out of the wet path and nothing
/// panics.
#[test]
fn hostile_parameters_and_samples_produce_no_panic_and_no_infinity() {
    let hostile = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        1e30,
        -1e30,
        0.0,
        1.0,
    ];

    for &value in &hostile {
        let mut engine = Engine::new(RATE);
        // Fully wet, so what is measured is the chain rather than the dry —
        // the dry is a wire and a host that sends a NaN gets one back
        // (`REQ-DIO-001`).
        engine.set(Macros {
            depth: value,
            direct: value,
            room: value,
            damping: value,
            width: value,
            clarity: value,
            mix: 1.0,
            output: 1.0,
        });
        engine.reset();

        for &sample in &hostile {
            let (left, right) = engine.process(sample, sample);
            assert!(
                left.is_finite() && right.is_finite(),
                "parameters at {value}, sample {sample}: {left} / {right}"
            );
        }
    }
}

/// **One non-finite sample must not latch anything** (`SPK-9`): not the delay
/// lines, not the followers, not the filters.
#[test]
fn one_hostile_sample_does_not_latch_the_engine() {
    let mut engine = Engine::new(RATE);
    engine.set(Macros {
        mix: 1.0,
        ..working()
    });
    engine.reset();

    // One NaN, then ordinary signal.
    engine.process(f32::NAN, f32::NAN);

    let input = harmonics::pink(0.4, 2 * RATE as usize);
    let mut energy = 0.0;
    for &sample in &input {
        let (left, right) = engine.process(sample, sample);
        assert!(left.is_finite() && right.is_finite());
        energy += left * left;
    }
    assert!(energy > 0.0, "the engine went silent after one NaN");

    // And it recovers to the same place a clean run reaches, which is the part
    // "does not latch" actually means.
    let clean: f32 = {
        let mut engine = Engine::new(RATE);
        engine.set(Macros {
            mix: 1.0,
            ..working()
        });
        engine.reset();
        input
            .iter()
            .map(|&sample| {
                let (left, _) = engine.process(sample, sample);
                left * left
            })
            .sum()
    };
    let difference = 10.0 * (energy / clean).log10();
    assert!(
        difference.abs() < 0.1,
        "after one NaN the engine settled {difference:.3} dB away from a clean run"
    );
}

/// **A parameter step does not click** (`REQ-DIO-016`).
///
/// Measured as a second difference around the step against the largest
/// anywhere else — an absolute jump cannot tell a step apart from the signal
/// (`AIR-7`). The control jumps the output by the same amount for one sample,
/// which is what establishes that the measurement can see a discontinuity of
/// that size (`DIO-2` found that snapping a *retuned* filter produces no seam
/// at all, so it is not a usable control).
#[test]
fn a_step_in_every_macro_leaves_no_seam() {
    // Not a whole number of cycles into the tone: a step landing exactly on a
    // zero crossing has nothing to multiply (`DIO-2`).
    let step_at = 12_007;
    let tone = harmonics::tone(0.5, 700.0, RATE, 24_000);

    let roughness = |as_a_plain_gain: bool| {
        let mut engine = Engine::new(RATE);
        engine.set(Macros {
            depth: 0.0,
            direct: 0.0,
            room: 0.2,
            damping: 0.0,
            width: 0.2,
            clarity: 0.0,
            mix: 1.0,
            output: 1.0,
        });
        engine.reset();

        let mut rendered = Vec::with_capacity(tone.len());
        let mut extra = 1.0f32;
        for (index, &sample) in tone.iter().enumerate() {
            if index == step_at {
                // Every macro, end to end, in one sample.
                engine.set(Macros {
                    depth: 1.0,
                    direct: 1.0,
                    room: 1.0,
                    damping: 1.0,
                    width: 1.0,
                    clarity: 1.0,
                    mix: 1.0,
                    output: 1.0,
                });
                if as_a_plain_gain {
                    extra = 4.0;
                }
            }
            rendered.push(extra * engine.process(sample, sample).0);
            extra = 1.0;
        }

        let second = |i: usize| rendered[i + 2] - 2.0 * rendered[i + 1] + rendered[i];
        let near = (step_at - 32..step_at + 32)
            .map(|i| second(i).abs())
            .fold(0.0f32, f32::max);
        let far = (2_000..step_at - 64)
            .chain(step_at + 64..rendered.len() - 3)
            .map(|i| second(i).abs())
            .fold(0.0f32, f32::max);
        near / far
    };

    let control = roughness(true);
    assert!(
        control > 2.0,
        "the control did not produce a seam: {control:.2}"
    );

    let measured = roughness(false);
    assert!(
        measured < 1.5,
        "a step in every macro left a seam: {measured:.2} (control {control:.2})"
    );
}
