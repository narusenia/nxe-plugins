//! What the analysis costs the audio thread.
//!
//! Measured against the same budget the Doubler's DSP is held to: a 512 sample
//! block at 48 kHz lasts 10.67 ms, and 5% of one core is **533 µs**
//! (`plugins/doubler/docs/requirements/REQ-DBL.md`, `REQ-DBL-011`). The
//! analysis is on top of the plugin's own work, so what matters is how much of
//! that budget it eats.
//!
//! The spectrum is the expensive half — one biquad and one follower per band,
//! per sample — which is exactly why it is measured rather than assumed.

use criterion::{Criterion, criterion_group, criterion_main};
use nxe_dsp::{PanScope, Spectrum};
use std::hint::black_box;

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

/// The sizes the Doubler asks for (`plugins/doubler/doubler/src/analysis.rs`).
const PAN_BINS: usize = 24;
const BANDS: usize = 32;

fn signal() -> Vec<f32> {
    (0..BLOCK).map(|i| (i as f32 * 0.07).sin() * 0.5).collect()
}

fn analysis(criterion: &mut Criterion) {
    let input = signal();
    let mut group = criterion.benchmark_group("analysis");

    group.bench_function("pan scope, 512 samples, 24 bins", |bencher| {
        bencher.iter_batched_ref(
            || PanScope::<PAN_BINS>::new(SAMPLE_RATE),
            |scope| {
                for &sample in &input {
                    scope.push(sample, -sample);
                }
                black_box(scope.bins());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("spectrum, 512 samples, 32 bands", |bencher| {
        bencher.iter_batched_ref(
            || Spectrum::<BANDS>::new(SAMPLE_RATE, 20.0, 20_000.0),
            |spectrum| {
                for &sample in &input {
                    spectrum.push(sample);
                }
                black_box(spectrum.levels());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, analysis);
criterion_main!(benches);
