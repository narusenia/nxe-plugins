//! The CPU budget from `REQ-DBL-011`, measured rather than assumed.
//!
//! The worst case the requirement names: 48 kHz, a 512 sample block, eight
//! voices, `True Stereo` — every voice live, both delay lines running, every
//! shifter and both wobble sources advancing. **Under 5% of one core** means a
//! 512 sample block, which lasts 10.67 ms at this rate, has to be processed in
//! under 533 µs.
//!
//! criterion reports the time; the budget is stated here so a reader does not
//! have to do the arithmetic, and a regression shows up as criterion's own
//! comparison against the last run.

use criterion::{Criterion, criterion_group, criterion_main};
use doubler_core::{DEFAULT_SHAPE, Macros, Source, VoiceEngine, Voices};
use std::hint::black_box;

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;

/// 5% of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;

fn worst_case(criterion: &mut Criterion) {
    let macros = Macros {
        voices: Voices::Eight,
        source: Source::TrueStereo,
        ..Macros::default()
    };

    // A signal rather than silence: the delay line and the shifter cost the
    // same either way, but denormals would not show up against zeros.
    let input: Vec<f32> = (0..BLOCK).map(|i| (i as f32 * 0.07).sin() * 0.5).collect();

    let mut group = criterion.benchmark_group("engine");
    group.bench_function(
        format!("512 samples, 8 voices, true stereo (budget {BUDGET_US} us)"),
        |bencher| {
            bencher.iter_batched_ref(
                || VoiceEngine::new(SAMPLE_RATE),
                |engine| {
                    for &sample in &input {
                        let (left, right) =
                            engine.process(sample, -sample, &macros, &DEFAULT_SHAPE);
                        black_box((left, right));
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );
    group.finish();
}

criterion_group!(benches, worst_case);
criterion_main!(benches);
