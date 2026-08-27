//! The whole of Air: the layer, and what it is added to.
//!
//! **Additive, and the original never goes through anything** (`REQ-AIR-001`).
//! That is one decision with three consequences the rest of the design leans
//! on: `MIX` = 0 is the input, `SURFACE` = 0 is the input, and the layer can be
//! taken out on its own — which is what the picture draws and what the tests
//! measure instead of subtracting the dry back out.
//!
//! ```text
//! in ─┬──────────────────────────────── dry ──────────┐
//!     └─ layer ─ × SURFACE ─────────────── × MIX ─────┴─→ out
//! ```
//!
//! ## `MIX` does not remove the original
//!
//! It scales what was added. The requirement's first draft asked for a
//! crossfade — `MIX` = 1 leaving only the layer — and that cannot hold at the
//! same time as "`SURFACE` = 0 is bit-identical to the input", because with no
//! layer to fade to, a crossfade still turns the original down. The additive
//! reading is the one the topology supports, and `REQ-AIR-012` was corrected to
//! it (`dsp.md`).

use nxe_audio::oversample::Factor;

use crate::layer::{self, Layer};

/// Everything the engine needs once per block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub focus: f32,
    pub character: f32,
    pub blend: f32,
    pub width: f32,
    /// Advanced: the curve's drive and bias.
    pub drive: f32,
    pub bias: f32,
    pub factor: Factor,
}

impl Default for Shape {
    fn default() -> Self {
        let settings = layer::Settings::default();
        Self {
            focus: settings.focus,
            character: settings.character,
            blend: settings.blend,
            width: settings.width,
            drive: settings.drive,
            bias: settings.bias,
            factor: settings.factor,
        }
    }
}

/// The layer, and the addition.
pub struct Engine {
    layer: Layer,
    /// The last frame of layer, for the display and for the tests — **taken
    /// directly rather than by subtracting the dry back out**, which throws
    /// away most of the layer's precision when the source is louder
    /// (`docs/HANDOVER.md`).
    generated: (f32, f32),
}

impl Engine {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        Self {
            layer: Layer::new(sample_rate, seed),
            generated: (0.0, 0.0),
        }
    }

    /// **Block rate.**
    pub fn set_shape(&mut self, shape: &Shape) {
        self.layer.set(layer::Settings {
            focus: shape.focus,
            character: shape.character,
            blend: shape.blend,
            width: shape.width,
            drive: shape.drive,
            bias: shape.bias,
            factor: shape.factor,
        });
    }

    /// One frame. **Audio rate.**
    ///
    /// `surface` is how much layer to make and `mix` how much of it to add;
    /// both are read per sample because both are smoothed (`VEL-5`).
    ///
    /// **The input is not sanitised on its way through.** A host that sends a
    /// NaN gets one back — the dry path is a wire, and cleaning it would make
    /// `MIX` = 0 something other than the input. Everything that holds state
    /// sanitises what *it* reads (`SPK-9`).
    pub fn process(&mut self, input: (f32, f32), surface: f32, mix: f32) -> (f32, f32) {
        let (left, right) = input;
        let surface = if surface.is_finite() {
            surface.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mix = if mix.is_finite() {
            mix.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.generated = self.layer.process(left, right, surface);
        (
            left + mix * self.generated.0,
            right + mix * self.generated.1,
        )
    }

    /// The layer alone, as of the last frame.
    pub fn layer(&self) -> (f32, f32) {
        self.generated
    }

    /// Where the layer sits, in Hz.
    pub fn corner_hz(&self) -> f32 {
        self.layer.corner_hz()
    }

    /// How far the noise half's keep-alive gate stands open, `0..=1`.
    pub fn keepalive(&self) -> f32 {
        self.layer.keepalive()
    }

    pub fn reset(&mut self) {
        self.layer.reset();
        self.generated = (0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics::{at_dbfs, pink, rms};

    const RATE: f32 = 48_000.0;
    const SEED: u32 = 0x4149_5234;

    fn material(length: usize) -> Vec<f32> {
        at_dbfs(pink(1.0, length), -18.0)
    }

    /// **The promise the whole topology exists for** (`REQ-AIR-001`).
    #[test]
    fn mix_at_zero_is_the_input() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        for sample in material(48_000) {
            assert_eq!(engine.process((sample, sample), 1.0, 0.0), (sample, sample));
        }
    }

    /// **And the second way out** (`REQ-AIR-001`). Two exits rather than one,
    /// deliberately: neither is a branch in the code — both are a gain landing
    /// on zero.
    #[test]
    fn surface_at_zero_is_the_input() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        for sample in material(48_000) {
            assert_eq!(engine.process((sample, sample), 0.0, 1.0), (sample, sample));
        }
    }

    /// `MIX` = 1 adds the whole layer and **leaves the original alone**
    /// (`REQ-AIR-012`, corrected).
    #[test]
    fn mix_at_one_keeps_the_original() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        let mut dry = Vec::new();
        let mut wet = Vec::new();
        for sample in material(48_000) {
            let (out, _) = engine.process((sample, sample), 1.0, 1.0);
            dry.push(sample);
            wet.push(out - sample);
        }
        // The layer is there…
        assert!(rms(&wet) > 1e-5, "nothing was added");
        // …and it is far enough under the source that "the original is still
        // the record" is true. Measured −13.0 dB at the shipped trims.
        let against = 20.0 * (rms(&wet) / rms(&dry)).log10();
        assert!(
            (-20.0..-6.0).contains(&against),
            "the layer sat at {against:.1} dB against the source"
        );
    }

    /// The layer can be read out on its own, which is what the picture draws
    /// and what the width tests measure (`REQ-AIR-001`).
    ///
    /// **And why it is read out rather than subtracted back.** `out − dry` is
    /// the obvious way to get at what was added, and it is a different number:
    /// the source here is 13 dB louder than the layer, so the subtraction
    /// spends most of the mantissa cancelling it. The assertion below is that
    /// the two disagree — if they ever stop, this test is measuring nothing and
    /// the warning in `docs/HANDOVER.md` has lost its example.
    #[test]
    fn the_layer_can_be_taken_out_directly() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        let mut subtraction_disagreed = false;
        for sample in material(4_800) {
            let out = engine.process((sample, sample), 1.0, 1.0);
            let layer = engine.layer();
            assert_eq!(out.0, sample + layer.0);
            assert_eq!(out.1, sample + layer.1);
            subtraction_disagreed |= out.0 - sample != layer.0;
        }
        assert!(
            subtraction_disagreed,
            "subtracting the dry back out lost nothing, so it is not the trap \
             the layer read exists to avoid"
        );
    }

    /// **The dry path is a wire.** A host that sends nonsense gets it back
    /// rather than getting silence, and nothing latches (`SPK-9`).
    #[test]
    fn a_hostile_input_passes_through_without_latching_anything() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9] {
            engine.process((value, value), 1.0, 1.0);
        }
        let recovered: Vec<f32> = material(24_000)
            .into_iter()
            .map(|sample| engine.process((sample, sample), 1.0, 1.0).0)
            .collect();
        assert!(recovered.iter().all(|value| value.is_finite()));
        assert!(
            rms(&recovered) > 1e-6,
            "the engine went silent and stayed so"
        );
    }

    #[test]
    fn reset_clears_it() {
        let mut engine = Engine::new(RATE, SEED);
        engine.set_shape(&Shape::default());
        for sample in material(4_800) {
            engine.process((sample, sample), 1.0, 1.0);
        }
        engine.reset();
        assert_eq!(engine.layer(), (0.0, 0.0));
        assert_eq!(engine.process((0.0, 0.0), 1.0, 1.0), (0.0, 0.0));
    }
}
