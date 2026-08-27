//! What has to hold of the whole engine rather than of one block of it
//! (`AIR-7`): the same answer at any block size, no divergence at any rate,
//! nothing that latches, and no step a user would hear as a click.

use air_core::engine::{Engine, Shape};
use nxe_audio::harmonics::{at_dbfs, pink, rms, tone};

const RATE: f32 = 48_000.0;
const SEED: u32 = 0x4149_5237;
const SURFACE: f32 = 0.8;
const MIX: f32 = 1.0;

fn shape() -> Shape {
    Shape {
        depths: [0.5; 3],
        ..Shape::default()
    }
}

/// The harmonic half alone.
///
/// **A click can only be measured against something smooth.** The noise half's
/// neighbouring samples are unrelated by construction, so its own movement is
/// larger than any discontinuity a coefficient change could make — with it in
/// the mix, the worst jump in a run is the same number whether a parameter
/// stepped or not (measured: 0.036005 either way).
fn smooth_shape() -> Shape {
    Shape {
        blend: 0.0,
        // **Low, so the layer is smooth enough to read.** At the default corner
        // a tone that survives the input high-pass has its harmonics near
        // Nyquist, and a signal that jagged hides a discontinuity in its own
        // curvature. Down here the layer is a few kHz and a seam stands out.
        focus: -1.0,
        ..shape()
    }
}

fn material(length: usize) -> Vec<f32> {
    at_dbfs(pink(1.0, length), -18.0)
}

/// The output for one block size, with `set_shape` called once per block the
/// way a host would.
fn rendered(block: usize, input: &[f32]) -> Vec<f32> {
    let mut engine = Engine::new(RATE, SEED);
    let mut output = Vec::with_capacity(input.len());
    for chunk in input.chunks(block) {
        engine.set_shape(&shape());
        for sample in chunk {
            output.push(engine.process((*sample, *sample), SURFACE, MIX).0);
        }
    }
    output
}

/// **The host's buffer size must not be audible** (`REQ-AIR-017`).
///
/// It holds for a structural reason rather than a tuned one — `process` is a
/// plain per-sample loop and nothing here keeps per-block state — which is
/// exactly the kind of guarantee that breaks quietly when someone reaches for
/// a block-based API (`.agents/rules/rust.md`).
#[test]
fn the_block_size_does_not_change_the_output() {
    let input = material(48_000);
    let reference = rendered(512, &input);
    for block in [1usize, 64, 4_096] {
        assert_eq!(
            rendered(block, &input),
            reference,
            "block size {block} produced a different signal"
        );
    }
}

/// Every coefficient is derived from the rate, so every rate has to produce a
/// finite, non-silent signal (`REQ-AIR-017`).
#[test]
fn no_supported_rate_makes_the_coefficients_diverge() {
    for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
        for focus in [-1.0f32, 0.0, 1.0] {
            let mut engine = Engine::new(rate, SEED);
            engine.set_shape(&Shape { focus, ..shape() });
            let input = at_dbfs(pink(1.0, rate as usize), -18.0);
            let output: Vec<f32> = input
                .iter()
                .map(|sample| engine.process((*sample, *sample), SURFACE, MIX).0)
                .collect();
            assert!(
                output.iter().all(|value| value.is_finite()),
                "{rate} Hz at FOCUS {focus} produced a non-finite sample"
            );
            assert!(
                rms(&output) > 1e-6,
                "{rate} Hz at FOCUS {focus} went silent"
            );
        }
    }
}

/// **A parameter is host-controlled input** (`REQ-AIR-016`). Every one of them
/// at every hostile value, and the plugin still returns numbers.
#[test]
fn hostile_settings_produce_neither_a_panic_nor_a_non_finite_sample() {
    let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
    let input = material(4_800);
    for value in wild {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape {
            focus: value,
            character: value,
            blend: value,
            width: value,
            drive: value,
            bias: value,
            depths: [value; 3],
            guard: value,
            factor: Shape::default().factor,
        });
        for sample in &input {
            let (left, right) = engine.process((*sample, *sample), value, value);
            assert!(
                left.is_finite() && right.is_finite(),
                "{value} produced {left}, {right}"
            );
        }
    }
}

/// **One bad sample must not end the session** (`REQ-AIR-016`). Sparkleur's
/// crossover latched on exactly this, which is why every detector here reads
/// through a sanitiser (`SPK-9`).
///
/// The dry path is deliberately *not* sanitised — a NaN in is a NaN out,
/// because cleaning the wire would make `MIX` = 0 something other than the
/// input — so what is checked is that the plugin comes back.
#[test]
fn a_single_non_finite_sample_does_not_latch_anything() {
    let mut engine = Engine::new(RATE, SEED);
    engine.set_shape(&shape());
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        engine.process((value, value), SURFACE, MIX);
    }
    // Long enough for the slow follower to forget a 1e9-sized event
    // (`air_core::follow`).
    let recovered: Vec<f32> = material(48_000 * 4)
        .iter()
        .map(|sample| engine.process((*sample, *sample), SURFACE, MIX).0)
        .collect();
    assert!(recovered.iter().all(|value| value.is_finite()));
    assert!(engine.layer().0.abs() > 0.0, "the layer never came back");
}

/// The worst neighbour-to-neighbour movement in the layer, split into "right
/// where the parameter moved" and "everywhere else".
///
/// **A click cannot be measured as an absolute jump.** A 5 kHz tone moves most
/// of its amplitude between one sample and the next all by itself, so the only
/// thing that separates a discontinuity from a waveform is *where* it happens.
fn jumps_around(step_at: usize, next: Option<Shape>, surface_after: f32) -> (f32, f32) {
    const WINDOW: usize = 64;
    let length = 48_000;
    let mut engine = Engine::new(RATE, SEED);
    engine.set_shape(&smooth_shape());
    // Above the corner, or the input high-pass takes the tone away before the
    // curve sees it and there is no layer to measure.
    let input = tone(0.25, 1_500.0, RATE, length);

    let (mut previous, mut before) = (0.0f32, 0.0f32);
    let (mut near, mut elsewhere) = (0.0f32, 0.0f32);
    for (index, sample) in input.iter().enumerate() {
        if index == step_at
            && let Some(shape) = next
        {
            engine.set_shape(&shape);
        }
        let surface = if index >= step_at {
            surface_after
        } else {
            SURFACE
        };
        engine.process((*sample, *sample), surface, MIX);
        let layer = engine.layer().0;
        // **The second difference, not the first.** A 5 kHz tone moves most of
        // its amplitude between neighbours all by itself, but it moves *evenly*
        // — its curvature is a fifth of its amplitude. A discontinuity has the
        // curvature of the jump.
        let jump = (layer - 2.0 * previous + before).abs();
        before = previous;
        previous = layer;

        if index <= (RATE * 0.25) as usize {
            continue;
        }
        if (step_at..step_at + WINDOW).contains(&index) {
            near = near.max(jump);
        } else {
            elsewhere = elsewhere.max(jump);
        }
    }
    (near, elsewhere)
}

/// **A step in a parameter must not be audible** (`REQ-AIR-016`).
///
/// The engine does not smooth — that is the wrapper's job, and reading a
/// smoother once per block is the trap `VEL-5` hit — so a single `set_shape`
/// with a moved `FOCUS` rebuilds every filter in both halves at once, and that
/// does leave a seam. What this pins is how big it is.
///
/// Measured: a quarter of `FOCUS`'s travel inside one sample leaves an excess
/// curvature of **0.00127 against a 0.25 source**, which is **−46 dB** — under
/// the layer's own peak by 14 dB, and that is before the wrapper's 50 ms
/// smoother has spread the move over a hundred blocks.
#[test]
fn stepping_a_parameter_leaves_nothing_audible() {
    const AMPLITUDE: f32 = 0.25;
    let step_at = 24_000;
    let against_source = |excess: f32| 20.0 * (excess / AMPLITUDE).log10();

    let (near, elsewhere) = jumps_around(
        step_at,
        Some(Shape {
            focus: -0.5,
            ..smooth_shape()
        }),
        SURFACE,
    );
    let seam = (near - elsewhere).max(0.0);
    assert!(
        against_source(seam) < -40.0,
        "a quarter of FOCUS's travel left {:.1} dB below the source",
        against_source(seam)
    );

    // **And the measurement can fail** (`VEL-10`). Cutting the amount to
    // nothing at the same instant is a real discontinuity, and the same metric
    // has to see a bigger one — a test that cannot fail is not a test.
    let (clicked, baseline) = jumps_around(step_at, None, 0.0);
    let real = (clicked - baseline).max(0.0);
    assert!(
        real > seam * 2.0,
        "an unsmoothed amount step left {real:.6} against the coefficient \
         step's {seam:.6}, so the measurement cannot tell them apart"
    );
}
