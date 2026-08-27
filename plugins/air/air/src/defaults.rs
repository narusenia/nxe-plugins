#![cfg(test)]
//! What the shipping defaults actually do, measured (`AIR-13`).
//!
//! **The defaults are the product's face** — there are no presets
//! (`REQ-AIR-022`) — and they were settled by ear. Not everything `dsp.md`
//! files under "耳で詰める定数" is a matter of taste, though: "ordinary
//! material is not held back", "silence is silent", "the layer is actually
//! there" are all measurable, and a number that fails them is wrong before
//! anyone listens. This file measures those, so a listening pass only has to
//! judge what is actually a judgement.
//!
//! It lives in the wrapper rather than in `air-core` because **the shipping
//! defaults are the parameters' defaults**. Reading them from
//! `AirParams::default()` is what makes it impossible for the numbers measured
//! here to drift from the numbers shipped.

use crate::params::AirParams;
use air_core::engine::Engine;
use nih_plug::prelude::Param;
use nxe_audio::harmonics::{at_dbfs, db_ratio, noise, pink, rms, tone};

const RATE: f32 = 48_000.0;
const BLOCK: usize = 64;
const SEED: u32 = 0x4149_5239;
const MATERIAL_DBFS: f32 = -18.0;

/// How long the engine is given before anything is believed. **Switching a
/// signal on is itself a transient**, and a detector doing its job opens fully
/// on it — which would read as "the pad sparkled" (`SPK-18`).
const SETTLE: f32 = 0.25;

fn ordinary(length: usize) -> Vec<f32> {
    at_dbfs(pink(1.0, length), MATERIAL_DBFS)
}

/// Six steady partials: something with no transients in it at all.
fn pad(length: usize) -> Vec<f32> {
    let mut mixed = vec![0.0f32; length];
    for hz in [110.0f32, 220.0, 330.0, 550.0, 1_100.0, 3_300.0] {
        for (sample, value) in mixed.iter_mut().zip(tone(0.05, hz, RATE, length)) {
            *sample += value;
        }
    }
    at_dbfs(mixed, MATERIAL_DBFS)
}

/// Eight strikes a second, each a 6 ms burst: nothing but transients.
fn hats(length: usize) -> Vec<f32> {
    let period = (RATE / 8.0) as usize;
    let raw = noise(1.0, length)
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let since = (index % period) as f32 / RATE;
            value * (-since / 0.006).exp()
        })
        .collect();
    at_dbfs(raw, MATERIAL_DBFS)
}

/// What the plugin does to one signal at the shipped settings.
struct Run {
    /// The layer alone, left channel.
    layer: Vec<f32>,
    /// The deepest the protection pulled, in dB, after settling.
    pull: f32,
    /// The three following coefficients at the end of the run.
    follow: [f32; 3],
}

/// **A freshly built parameter's smoother starts at zero**, not at the
/// parameter's value — a host primes them through `initialize`, and a test that
/// does not is measuring a plugin with every macro at the bottom of its range.
/// Measured before this: the layer came out at **−200 dB**, which reads as
/// "the defaults add nothing" and is entirely an artefact of the harness.
fn primed() -> AirParams {
    let params = AirParams::default();
    params.surface.smoothed.reset(params.surface.value());
    params.blend.smoothed.reset(params.blend.value());
    params.character.smoothed.reset(params.character.value());
    params.focus.smoothed.reset(params.focus.value());
    params.width.smoothed.reset(params.width.value());
    params.follow.smoothed.reset(params.follow.value());
    params.mix.smoothed.reset(params.mix.value());
    params.output.smoothed.reset(params.output.value());
    params.drive.smoothed.reset(params.drive.value());
    params.bias.smoothed.reset(params.bias.value());
    params
        .follow_envelope
        .smoothed
        .reset(params.follow_envelope.value());
    params
        .follow_brightness
        .smoothed
        .reset(params.follow_brightness.value());
    params
        .follow_transient
        .smoothed
        .reset(params.follow_transient.value());
    params.guard.smoothed.reset(params.guard.value());
    params
}

fn run(input: &[f32]) -> Run {
    let params = primed();
    let mut engine = Engine::new(RATE, SEED);
    let settle = (RATE * SETTLE) as usize;

    let mut layer = Vec::with_capacity(input.len());
    let mut pull = 0.0f32;
    for (block, chunk) in input.chunks(BLOCK).enumerate() {
        // **Set per block, the way a host drives it.** A shape read once before
        // the first sample leaves every detector looking at silence, which is
        // how Velour measured `EMOTION` backwards (`VEL-6`).
        engine.set_shape(&params.shape(chunk.len() as u32));
        for (offset, sample) in chunk.iter().enumerate() {
            let surface = params.surface.smoothed.next();
            let mix = params.mix.smoothed.next();
            engine.process((*sample, *sample), surface, mix);
            if block * BLOCK + offset >= settle {
                layer.push(engine.layer().0);
                pull = pull.min(engine.guard_reduction_db());
            }
        }
    }
    Run {
        layer,
        pull,
        follow: engine.follow_coefficients(),
    }
}

/// The layer's level against the source it was added to.
fn against_source(run: &Run, input: &[f32]) -> f32 {
    db_ratio(rms(&run.layer), rms(input))
}

/// **The defaults are what was listened to.** If one moves without a listening
/// pass behind it, this fails and says so.
#[test]
fn the_shipped_defaults_are_the_ones_that_were_chosen() {
    let params = AirParams::default();
    assert_eq!(params.surface.default_plain_value(), 0.50);
    assert_eq!(params.blend.default_plain_value(), 0.50);
    assert_eq!(params.character.default_plain_value(), 0.72);
    assert_eq!(params.focus.default_plain_value(), -0.50);
    assert_eq!(params.width.default_plain_value(), 0.60);
    assert_eq!(params.follow.default_plain_value(), 1.00);
    assert_eq!(params.mix.default_plain_value(), 1.00);
}

/// **Ordinary material is not held back** (`REQ-AIR-009`). The test Sparkleur
/// shipped without: its threshold sat below every broadband material's ratio
/// and pulled 1.3 dB out of ordinary music at the default setting (`SPK-18`).
#[test]
fn nothing_is_pulled_out_of_ordinary_material() {
    let input = ordinary((RATE * 2.0) as usize);
    assert_eq!(run(&input).pull, 0.0);
}

/// **The layer is actually there.** A default that adds nothing is a plugin
/// that does nothing until someone finds the right knob, and the acceptance
/// condition is "turn `SURFACE` once and know" (`REQ-AIR-022`).
///
/// Measured at the shipped settings, against the source:
///
/// | material | layer | ENV / BRT / TRN | guard |
/// |---|---|---|---|
/// | pink noise | **−32.4 dB** | 0.93 / 0.79 / 0.16 | 0.00 |
/// | a six-partial pad | −54.6 dB | 0.94 / 0.35 / 0.05 | 0.00 |
/// | a hat pattern | −24.1 dB | 0.92 / 1.00 / 0.06 | **−9.2 dB** |
#[test]
fn the_layer_is_audible_on_ordinary_material() {
    let input = ordinary((RATE * 2.0) as usize);
    let measured = run(&input);
    let level = against_source(&measured, &input);
    assert!(
        (-40.0..-20.0).contains(&level),
        "the layer sat at {level:.1} dB against the source"
    );
}

/// **`FOLLOW` at full is what the defaults chose**, so the layer follows the
/// material rather than sitting on top of it.
///
/// **The gap is 30 dB**: a hat pattern gets −24.1 dB of layer and a sustained
/// pad −54.6, because at full depth the transient detector hands a sustain
/// about a twentieth of the layer (0.05 against 0.16 on noise). That is the
/// mechanism doing exactly what `REQ-AIR-007` describes, and it is also why a
/// pad is a case for pulling `TRN` down in Advanced rather than for the
/// default.
#[test]
fn the_layer_follows_the_material() {
    let length = (RATE * 2.0) as usize;
    let steady = pad(length);
    let struck = hats(length);
    let sustained = against_source(&run(&steady), &steady);
    let transient = against_source(&run(&struck), &struck);
    assert!(
        transient > sustained,
        "a pad got {sustained:.1} dB and a hat pattern {transient:.1} dB"
    );
}

/// **Silence is silent** (`REQ-AIR-004`). The noise half would otherwise be a
/// permanent hiss between the takes.
#[test]
fn silence_is_exactly_silent() {
    let mut input = ordinary((RATE * 0.5) as usize);
    input.extend(std::iter::repeat_n(0.0, (RATE * 2.0) as usize));
    let measured = run(&input);
    let tail = &measured.layer[measured.layer.len() - 4_800..];
    assert!(
        tail.iter().all(|value| *value == 0.0),
        "the layer was still making {} at the end of two seconds of nothing",
        rms(tail)
    );
    // And the detectors are shut rather than merely small.
    assert_eq!(measured.follow[0], 0.0);
}
