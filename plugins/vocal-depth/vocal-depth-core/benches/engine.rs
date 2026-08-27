//! The CPU budget from `REQ-VDP-016`, measured rather than assumed.
//!
//! The case the requirement names: 48 kHz, a 512 sample block, everything
//! working. Under 5% of one core means a 512 sample block — 10.67 ms at this
//! rate — has to be processed in under **533 µs**, and the target is 250 µs
//! because the analysis and the interface sit on top.
//!
//! **No oversampling and no FFT anywhere in here** (`REQ-VDP-016`), which is why
//! the estimate in `dsp.md` was under 60 µs: ten integer tap reads, three
//! allpasses, a handful of one-poles and two followers.
//!
//! **Every macro moving is measured separately**, because that is the path that
//! rebuilds coefficients and tap weights — and it is a path a user is on
//! whenever they turn `DEPTH`.

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use vocal_depth_core::{Engine, Macros};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

/// 5% of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;

/// A signal rather than silence: the filters cost the same either way, but the
/// detectors and `CLARITY` do nothing until something arrives, and denormals
/// would not show up against zeros.
fn signal() -> Vec<f32> {
    (0..BLOCK)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE;
            // Material in the body, in the presence band and above it, so every
            // detector has something to read.
            0.30 * (std::f32::consts::TAU * 220.0 * time).sin()
                + 0.20 * (std::f32::consts::TAU * 800.0 * time).sin()
                + 0.15 * (std::f32::consts::TAU * 3_000.0 * time).sin()
                + 0.10 * (std::f32::consts::TAU * 9_000.0 * time).sin()
        })
        .collect()
}

/// Everything doing something — the expensive case, not the default one.
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

fn run(criterion: &mut Criterion) {
    let input = signal();
    let mut group = criterion.benchmark_group("engine");

    for (name, macros, moving) in [
        (
            format!("512 samples, settled (budget {BUDGET_US} us)"),
            working(),
            false,
        ),
        ("512 samples, depth moving".to_string(), working(), true),
        (
            "512 samples, room at zero".to_string(),
            Macros {
                room: 0.0,
                ..working()
            },
            false,
        ),
        (
            "512 samples, damping and clarity off".to_string(),
            Macros {
                damping: 0.0,
                clarity: 0.0,
                ..working()
            },
            false,
        ),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut engine = Engine::new(SAMPLE_RATE);
                    // Built once here so the measured block is a steady-state
                    // one: the first `set` of an engine's life rebuilds
                    // everything, and that is not what runs 94 times a second.
                    engine.set(macros);
                    engine.reset();
                    (engine, 0u32)
                },
                |(engine, block)| {
                    *block = block.wrapping_add(1);
                    // A value that actually changes, or `set` returns early and
                    // the rebuild being measured never happens.
                    let macros = if moving {
                        Macros {
                            depth: (*block % 16) as f32 / 16.0,
                            ..macros
                        }
                    } else {
                        macros
                    };
                    engine.set(macros);
                    for &sample in &input {
                        black_box(engine.process(sample, -sample));
                    }
                    black_box(engine.pattern());
                    black_box(engine.clarity_lift_db());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, run);
criterion_main!(benches);
