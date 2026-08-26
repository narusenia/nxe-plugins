//! The CPU budget from `REQ-VEL-016`, measured rather than assumed.
//!
//! The worst case the requirement names: 48 kHz, a 512 sample block, **4x**
//! oversampling, all three generators running. **Under 5% of one core** means a
//! 512 sample block, which lasts 10.67 ms at this rate, has to be processed in
//! under 533 µs — and the target is 250 µs, because the analysis and the
//! interface are on top of this.
//!
//! `EMOTION` is on in the worst case, and that is the point of measuring it: a
//! moving envelope makes `set_shape` rebuild all three curves **every block**,
//! including the normalisation sweep over 64 probe points per band
//! (`nxe_audio::shaper`). With it off the curves are built once and the block
//! is filter arithmetic alone, so the pair of numbers says what the feature
//! costs.

use criterion::{Criterion, criterion_group, criterion_main};
use nxe_audio::oversample::Factor;
use std::hint::black_box;
use velour_core::engine::{BAND_COUNT, Engine, Levels, Shape};

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

/// 5% of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;

/// A signal rather than silence: the filters cost the same either way, but
/// denormals would not show up against zeros, and neither would the guards or
/// the compressor, which do nothing until something arrives.
fn signal() -> Vec<f32> {
    (0..BLOCK)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE;
            // A fundamental with something in every band, so all three
            // generators have material and both guards have a reason to look.
            0.4 * (std::f32::consts::TAU * 220.0 * time).sin()
                + 0.2 * (std::f32::consts::TAU * 3_000.0 * time).sin()
                + 0.1 * (std::f32::consts::TAU * 7_000.0 * time).sin()
        })
        .collect()
}

fn levels() -> Levels {
    Levels {
        bands: [0.8; BAND_COUNT],
        mix: 0.7,
    }
}

fn run(criterion: &mut Criterion) {
    let input = signal();
    let open = levels();

    let mut group = criterion.benchmark_group("engine");

    for (name, shape) in [
        (
            format!("4x, 512 samples, emotion on (budget {BUDGET_US} us)"),
            Shape {
                drive: 0.7,
                density: 0.6,
                emotion: 0.5,
                factor: Factor::Four,
                ..Shape::default()
            },
        ),
        (
            "4x, 512 samples, emotion off".to_string(),
            Shape {
                drive: 0.7,
                density: 0.6,
                emotion: 0.0,
                factor: Factor::Four,
                ..Shape::default()
            },
        ),
        (
            "2x, 512 samples, emotion on".to_string(),
            Shape {
                drive: 0.7,
                density: 0.6,
                emotion: 0.5,
                factor: Factor::Two,
                ..Shape::default()
            },
        ),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut engine = Engine::new(SAMPLE_RATE);
                    // Built once here so the measured block is a steady-state
                    // one: the first `set_shape` of an engine's life rebuilds
                    // everything, and that is not what runs 375 times a second.
                    engine.set_shape(&shape);
                    engine
                },
                |engine| {
                    engine.set_shape(&shape);
                    for &sample in &input {
                        black_box(engine.process((sample, -sample), &open));
                    }
                    black_box(engine.guard_reductions());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, run);
criterion_main!(benches);
