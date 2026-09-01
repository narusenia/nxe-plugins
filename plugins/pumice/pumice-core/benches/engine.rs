//! The CPU budget from `REQ-PUM-016`, measured rather than assumed.
//!
//! The case the requirement names: 48 kHz, a 512 sample block, stereo,
//! everything working. Under 5 % of one core means a 512 sample block —
//! 10.67 ms at this rate — has to be processed in under **533 µs**, and the
//! target is 250 µs because the interface sits on top.
//!
//! **`HIGH` is the case that has to fit**, not `NORMAL`: it is a setting a user
//! can choose, so the budget applies to it.
//!
//! ## Why the measured unit is eight host blocks and not one
//!
//! **A bench that measures one 512 sample block measures nothing at `HIGH`.**
//! Criterion builds a fresh engine for every iteration, and at `HIGH` the hop
//! is 1024 samples — so 512 samples never reach a frame boundary and no
//! transform runs at all. The first version of this file reported **5.25 µs for
//! `HIGH` against 32.7 for `NORMAL`**, which is backwards and was the giveaway.
//!
//! 4096 samples is a whole number of hops at every step (16 at `LOW`, 8 at
//! `NORMAL`, 4 at `HIGH`), so every frame that should run does. **Divide the
//! reported time by eight for the per-block figure** the budget is written
//! against.
//!
//! ## What the estimate in `dsp.md` said
//!
//! 50–80 µs, on the grounds that the followers run at **hop rate** rather than
//! sample rate — which is the whole reason an FFT was allowed onto the audio
//! path here at all (`docs/specifications/architecture.md`). Velour pays 45 µs
//! for a 48-band filter bank whose followers run every sample; this runs 1025
//! bins every 512.
//!
//! **The analysis costs nothing extra** (`REQ-PUM-018`): the spectrum the
//! window draws is the spectrum the engine is already working on, so there is
//! no second `nxe_dsp::Spectrum` to pay for.

use criterion::{Criterion, criterion_group, criterion_main};
use pumice_core::{Controls, Engine, NODES, Node, Quality, Range, Settings};
use std::hint::black_box;

const SAMPLE_RATE: f32 = 48_000.0;
const BLOCK: usize = 512;
/// Eight host blocks: a whole number of hops at every `QUALITY`.
const BLOCKS: usize = 8;
const SAMPLES: usize = BLOCK * BLOCKS;

/// 5 % of the 10.67 ms a 512 sample block occupies at 48 kHz.
const BUDGET_US: f64 = 533.0;
/// What the reported figure has to stay under, since it covers [`BLOCKS`].
const WINDOW_BUDGET_US: f64 = BUDGET_US * BLOCKS as f64;

/// A signal rather than silence.
///
/// **Silence would measure the wrong thing twice over**: the map's gate holds
/// when nothing is sounding, and denormals do not show up against zeros. This
/// has something in every region the detector looks at, plus a resonance for it
/// to find.
fn signal() -> Vec<Vec<f32>> {
    let channel = |offset: f32| -> Vec<f32> {
        (0..SAMPLES)
            .map(|index| {
                let time = (index as f32 + offset) / SAMPLE_RATE;
                0.30 * (std::f32::consts::TAU * 220.0 * time).sin()
                    + 0.20 * (std::f32::consts::TAU * 800.0 * time).sin()
                    + 0.25 * (std::f32::consts::TAU * 2_500.0 * time).sin()
                    + 0.10 * (std::f32::consts::TAU * 9_000.0 * time).sin()
            })
            .collect()
    };
    vec![channel(0.0), channel(17.0)]
}

/// Everything doing something — the expensive case, not the default one.
fn working(quality: Quality) -> Controls {
    Controls {
        depth: 0.7,
        sharpness: 0.5,
        speed: 0.6,
        threshold_db: Settings::DEFAULT.threshold_db,
        mix: 0.9,
        output: 1.0,
        delta: false,
        quality,
        // **All six on**, which is what the weight curve costs to rebuild.
        nodes: std::array::from_fn(|index| Node {
            enabled: true,
            freq_hz: 200.0 * 2.0_f32.powi(index as i32),
            width_octaves: 0.5,
            depth: 0.5,
        }),
        range: Range::default(),
    }
}

fn run(criterion: &mut Criterion) {
    let input = signal();
    let mut group = criterion.benchmark_group("engine");

    for (name, quality, moving) in [
        (
            format!("8 blocks stereo, HIGH, settled (budget {WINDOW_BUDGET_US} us)"),
            Quality::High,
            false,
        ),
        (
            "8 blocks stereo, NORMAL, settled".to_string(),
            Quality::Normal,
            false,
        ),
        (
            "8 blocks stereo, LOW, settled".to_string(),
            Quality::Low,
            false,
        ),
        // The path a user is on whenever they drag a node: the weight curve is
        // rebuilt across every bin.
        (
            "8 blocks stereo, HIGH, a node moving".to_string(),
            Quality::High,
            true,
        ),
    ] {
        group.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let mut engine = Engine::new(SAMPLE_RATE, Settings::DEFAULT);
                    // Built once here so the measured block is a steady-state
                    // one: the first `set` of an engine's life rebuilds
                    // everything, and that is not what runs 94 times a second.
                    engine.set(working(quality));
                    let buffers: Vec<Vec<f32>> = input.clone();
                    (engine, buffers, 0u32)
                },
                |(engine, buffers, block)| {
                    for (index, buffer) in buffers.iter_mut().enumerate() {
                        buffer.copy_from_slice(&input[index]);
                    }

                    // Fed the way a host feeds it: `set` once per block, then
                    // the block.
                    for start in (0..SAMPLES).step_by(BLOCK) {
                        *block = block.wrapping_add(1);
                        let mut controls = working(quality);
                        if moving {
                            // A value that actually changes, or `set` returns
                            // early and the rebuild being measured never
                            // happens.
                            controls.nodes[0].freq_hz = 500.0 + (*block % 16) as f32 * 10.0;
                        }
                        engine.set(controls);

                        let (left, right) = buffers.split_at_mut(1);
                        let mut channels: [&mut [f32]; 2] = [
                            &mut left[0][start..start + BLOCK],
                            &mut right[0][start..start + BLOCK],
                        ];
                        engine.process(&mut channels);
                        black_box(&channels);
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
    let _ = NODES;
}

criterion_group!(benches, run);
criterion_main!(benches);
