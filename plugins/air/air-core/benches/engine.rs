//! The CPU budget from `REQ-AIR-016`, measured rather than assumed.
//!
//! The worst case the requirement names: 48 kHz, a 512 sample block, **4x**
//! oversampling, everything working. Under 5% of one core means a 512 sample
//! block — 10.67 ms at this rate — has to be processed in under **533 µs**, and
//! the target is 250 µs because the analysis and the interface sit on top.
//!
//! **The two spectra are not in here.** They live in the wrapper
//! (`air::analysis`) and cost what Sparkleur measured for the same widths
//! (`SPK-17`: 15.2 µs for 32 bands), so the engine's own number plus twice that
//! is the figure to compare against the budget.
//!
//! `FOCUS` is measured separately because it is the one control whose movement
//! rebuilds filters — ten a channel across the two halves.

use air_core::engine::{Engine, Shape};
use criterion::{Criterion, criterion_group, criterion_main};
use nxe_audio::oversample::Factor;
use std::hint::black_box;

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;
const SEED: u32 = 0x4149_5238;

/// 5% of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;

/// A signal rather than silence: the filters cost the same either way, but the
/// detectors and the protection do nothing until something arrives, and
/// denormals would not show up against zeros.
fn signal() -> Vec<f32> {
    (0..BLOCK)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE;
            // Something above the corner and something well below it, so both
            // the detection band and the reference band have material.
            0.30 * (std::f32::consts::TAU * 220.0 * time).sin()
                + 0.20 * (std::f32::consts::TAU * 800.0 * time).sin()
                + 0.15 * (std::f32::consts::TAU * 4_000.0 * time).sin()
                + 0.10 * (std::f32::consts::TAU * 9_000.0 * time).sin()
        })
        .collect()
}

/// Both halves audible and every detector doing something — the expensive case,
/// not the default one.
fn working(factor: Factor) -> Shape {
    Shape {
        blend: 0.5,
        depths: [0.6; 3],
        factor,
        ..Shape::default()
    }
}

fn run(criterion: &mut Criterion) {
    let input = signal();
    let mut group = criterion.benchmark_group("engine");

    for (name, shape, moving) in [
        (
            format!("4x, 512 samples, settled (budget {BUDGET_US} us)"),
            working(Factor::Four),
            false,
        ),
        (
            "2x, 512 samples, settled".to_string(),
            working(Factor::Two),
            false,
        ),
        (
            "4x, 512 samples, noise only".to_string(),
            Shape {
                blend: 1.0,
                ..working(Factor::Four)
            },
            false,
        ),
        (
            "4x, 512 samples, focus moving".to_string(),
            working(Factor::Four),
            true,
        ),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut engine = Engine::new(SAMPLE_RATE, SEED);
                    // Built once here so the measured block is a steady-state
                    // one: the first `set_shape` of an engine's life rebuilds
                    // everything, and that is not what runs 94 times a second.
                    engine.set_shape(&shape);
                    (engine, 0u32)
                },
                |(engine, block)| {
                    *block = block.wrapping_add(1);
                    // A value that actually changes, or `set_shape` returns
                    // early and the rebuild being measured never happens.
                    let shape = if moving {
                        Shape {
                            focus: (*block % 16) as f32 / 16.0 - 0.5,
                            ..shape
                        }
                    } else {
                        shape
                    };
                    engine.set_shape(&shape);
                    for &sample in &input {
                        black_box(engine.process((sample, -sample), 0.8, 1.0));
                    }
                    black_box(engine.follow_coefficients());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    // **What `SURFACE` at zero saves.** The harmonic half returns before the
    // oversampled block when its gain is exactly zero (`REQ-AIR-001`), so the
    // plugin sitting at rest in a session should cost almost nothing.
    let shape = working(Factor::Four);
    group.bench_function("4x, 512 samples, surface at zero", |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut engine = Engine::new(SAMPLE_RATE, SEED);
                engine.set_shape(&shape);
                engine
            },
            |engine| {
                engine.set_shape(&shape);
                for &sample in &input {
                    black_box(engine.process((sample, -sample), 0.0, 1.0));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // **What silence costs.** Every follower here is a one-pole, so a tail of
    // silence walks their state down through denormals, and on some processors
    // denormal arithmetic is an order of magnitude slower.
    //
    // **A machine that is fast at denormals cannot answer for one that is
    // not.** Apple Silicon handles them at full speed; the number to watch on
    // an x86 host is the ratio between this and the working case, not this
    // alone.
    let quiet = vec![0.0f32; BLOCK];
    group.bench_function("4x, 512 samples, decaying into silence", |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut engine = Engine::new(SAMPLE_RATE, SEED);
                engine.set_shape(&shape);
                for &sample in &input {
                    engine.process((sample, -sample), 0.8, 1.0);
                }
                for _ in 0..(SAMPLE_RATE as usize * 2 / BLOCK) {
                    for &sample in &quiet {
                        engine.process((sample, sample), 0.8, 1.0);
                    }
                }
                engine
            },
            |engine| {
                engine.set_shape(&shape);
                for &sample in &quiet {
                    black_box(engine.process((sample, sample), 0.8, 1.0));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, run);
criterion_main!(benches);
