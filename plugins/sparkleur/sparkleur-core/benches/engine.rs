//! The CPU budget from `REQ-SPK-016`, measured rather than assumed.
//!
//! The worst case the requirement names: 48 kHz, a 512 sample block, **4x**
//! oversampling, everything working. Under 5% of one core means a 512 sample
//! block — 10.67 ms at this rate — has to be processed in under **533 µs**, and
//! the target is 250 µs because the analysis and the interface sit on top.
//!
//! Two things here are expensive only when a knob moves, and both are measured
//! separately so the cost of moving one is a number rather than a worry:
//! `FOCUS` rebuilds **forty biquads a channel**, and `CHARACTER` renormalises
//! the Sparkle curve over its probe sweep (`nxe_audio::shaper`).

use criterion::{Criterion, criterion_group, criterion_main};
use nxe_audio::oversample::Factor;
use sparkleur_core::engine::{Engine, Levels, Shape};
use std::hint::black_box;

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

/// 5% of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;

/// A signal rather than silence: the filters cost the same either way, but the
/// compressors and the guard do nothing until something arrives, and denormals
/// would not show up against zeros.
fn signal() -> Vec<f32> {
    (0..BLOCK)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE;
            // Something in every band, so all five detectors have material, the
            // guard has a reason to look and the Sparkle bus has something to
            // shape.
            0.30 * (std::f32::consts::TAU * 60.0 * time).sin()
                + 0.25 * (std::f32::consts::TAU * 220.0 * time).sin()
                + 0.20 * (std::f32::consts::TAU * 800.0 * time).sin()
                + 0.15 * (std::f32::consts::TAU * 3_000.0 * time).sin()
                + 0.10 * (std::f32::consts::TAU * 9_000.0 * time).sin()
        })
        .collect()
}

fn levels() -> Levels {
    Levels {
        spark: 0.8,
        body: 0.2,
        air: 0.3,
        mix: 0.9,
    }
}

fn working(factor: Factor) -> Shape {
    Shape {
        factor,
        snap: 0.6,
        ..Shape::default()
    }
}

fn run(criterion: &mut Criterion) {
    let input = signal();
    let open = levels();

    let mut group = criterion.benchmark_group("engine");

    for (name, shape, moving) in [
        (
            format!("4x, 512 samples, settled (budget {BUDGET_US} us)"),
            working(Factor::Four),
            None,
        ),
        (
            "2x, 512 samples, settled".to_string(),
            working(Factor::Two),
            None,
        ),
        (
            "4x, 512 samples, focus moving".to_string(),
            working(Factor::Four),
            Some(true),
        ),
        (
            "4x, 512 samples, character moving".to_string(),
            working(Factor::Four),
            Some(false),
        ),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut engine = Engine::new(SAMPLE_RATE);
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
                    let shape = match moving {
                        Some(true) => Shape {
                            focus: (*block % 16) as f32 / 16.0 - 0.5,
                            ..shape
                        },
                        Some(false) => Shape {
                            character: (*block % 16) as f32 / 16.0,
                            ..shape
                        },
                        None => shape,
                    };
                    engine.set_shape(&shape);
                    for &sample in &input {
                        black_box(engine.process((sample, -sample), &open));
                    }
                    black_box(engine.gains_db());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    // **What silence costs** — the question `SPK-16` left open. Every follower
    // here is a one-pole, so a tail of silence walks their state down through
    // denormals, and on some processors denormal arithmetic is an order of
    // magnitude slower. Measured against the working case above, this says
    // whether that matters.
    //
    // **A machine that is fast at denormals cannot answer for one that is
    // not.** Apple Silicon handles them at full speed; the number to watch on
    // an x86 host is the ratio between these two, not this one alone.
    let quiet = vec![0.0f32; BLOCK];
    let shape = working(Factor::Four);
    group.bench_function("4x, 512 samples, decaying into silence", |bencher| {
        bencher.iter_batched_ref(
            || {
                let mut engine = Engine::new(SAMPLE_RATE);
                engine.set_shape(&shape);
                // Load every follower, then let it fall: two seconds of nothing
                // is far past the point where the state is denormal.
                for &sample in &input {
                    engine.process((sample, -sample), &open);
                }
                for _ in 0..(SAMPLE_RATE as usize * 2 / BLOCK) {
                    for &sample in &quiet {
                        engine.process((sample, sample), &open);
                    }
                }
                engine
            },
            |engine| {
                engine.set_shape(&shape);
                for &sample in &quiet {
                    black_box(engine.process((sample, sample), &open));
                }
                black_box(engine.gains_db());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, run);
criterion_main!(benches);
