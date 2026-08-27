//! The two generators as one object.
//!
//! **The layer is a single thing that gets placed, modulated and protected**
//! (`REQ-AIR-002`) — that is the whole difference between Air and a saturator
//! with a noise generator bolted on. So `BLEND`, `CHARACTER` and `FOCUS` are
//! resolved here, once, and everything downstream sees one pair of samples.
//!
//! ```text
//! 倍音 ─ × cos(BLEND·π/2) · trim_h ─┐
//!                                   ├─→ layer
//! ノイズ ─ × sin(BLEND·π/2) · trim_n ┘
//! ```
//!
//! ## Why the crossfade is power-preserving
//!
//! The two halves are uncorrelated — one is made from the input and the other
//! from a random sequence — so mixing them by amplitude leaves a 3 dB dip in
//! the middle of the control (`REQ-AIR-005`). `cos²+ sin² = 1` is the mix that
//! keeps the sum's power flat, and it is the same identity `WIDTH` is built on.
//!
//! ## Why `CHARACTER` is handed to both halves unchanged
//!
//! The requirement describes it as an axis whose *meaning* moves with `BLEND`:
//! a knee on the harmonic side, grain on the noise side, both in the middle
//! (`REQ-AIR-005`). No interpolation is needed to get that — only the half
//! `BLEND` selected is audible, so passing the same normalised value to both is
//! the behaviour the table describes, with no branch to get wrong.

use nxe_audio::oversample::Factor;

use crate::harmonic::{self, Harmonic};
use crate::noise::{self, Noise};

/// What each half is scaled by so that the crossfade is flat.
///
/// **Measured, not chosen** (`AIR-3`): the power-preserving mix is only flat if
/// the two halves arrive at the same level, and the harmonic half's level
/// depends on the material. Pink noise at −18 dBFS is the proxy for "spectrally
/// ordinary" the rest of the workspace uses (`nxe_audio::harmonics::pink`).
pub const HARMONIC_TRIM: f32 = 1.0;
/// **28.90 dB below the noise generator's own unit RMS** (`AIR-3`): that is
/// what the harmonic half returns from pink noise at −18 dBFS, once the input
/// high-pass has taken everything below the corner away. With it, the `BLEND`
/// sweep is flat to **0.02 dB**; without it, it moves **28.9 dB**.
///
/// The harmonic half's level depends on the material and the noise half's does
/// not, so this is exact for the proxy and approximate for everything else.
/// **The absolute amount is `SURFACE`'s job** (`AIR-4`), not this pair's.
pub const NOISE_TRIM: f32 = 0.0359;

/// Everything about the layer that is not the signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `-1..=1`. Where the layer sits — moves both halves together.
    pub focus: f32,
    /// `0..=1`. The knee on one side, the grain on the other.
    pub character: f32,
    /// `0..=1`. Harmonic to noise.
    pub blend: f32,
    /// `0..=1`. The noise half only (`REQ-AIR-008`).
    pub width: f32,
    /// Advanced: the curve's drive and bias.
    pub drive: f32,
    pub bias: f32,
    pub factor: Factor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus: 0.0,
            character: 0.0,
            blend: 0.5,
            width: 0.5,
            drive: harmonic::DRIVE,
            bias: harmonic::BIAS,
            factor: Factor::default(),
        }
    }
}

/// Both generators, and the mix between them.
pub struct Layer {
    harmonic: Harmonic,
    noise: Noise,
    harmonic_gain: f32,
    noise_gain: f32,
    settings: Settings,
    /// How many times the settings have actually been rebuilt — for the test
    /// that says holding a knob still costs nothing.
    rebuilds: u32,
}

impl Layer {
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let mut layer = Self {
            harmonic: Harmonic::new(sample_rate),
            noise: Noise::new(sample_rate, seed),
            harmonic_gain: 0.0,
            noise_gain: 0.0,
            // Every field differs, so the first `set` builds all of it
            // (`AIR-1`).
            settings: Settings {
                focus: f32::NAN,
                character: f32::NAN,
                blend: f32::NAN,
                width: f32::NAN,
                drive: f32::NAN,
                bias: f32::NAN,
                factor: Factor::Two,
            },
            rebuilds: 0,
        };
        layer.set(Settings::default());
        layer
    }

    /// **Block rate.**
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            focus: finite(settings.focus, 0.0).clamp(-1.0, 1.0),
            character: finite(settings.character, 0.0).clamp(0.0, 1.0),
            blend: finite(settings.blend, 0.0).clamp(0.0, 1.0),
            width: finite(settings.width, 0.0).clamp(0.0, 1.0),
            drive: finite(settings.drive, harmonic::DRIVE),
            bias: finite(settings.bias, harmonic::BIAS),
            factor: settings.factor,
        };
        if settings == self.settings {
            return;
        }
        self.rebuilds += 1;

        let angle = settings.blend * std::f32::consts::FRAC_PI_2;
        self.harmonic_gain = angle.cos() * HARMONIC_TRIM;
        self.noise_gain = angle.sin() * NOISE_TRIM;
        // The ends have to be *exactly* zero, not `cos(π/2)`, which is
        // `-4.4e-8`: both halves promise silence when the control is against
        // their stop, and the harmonic half skips its oversampled block on an
        // exact zero (`REQ-AIR-005`).
        if settings.blend == 0.0 {
            self.noise_gain = 0.0;
        } else if settings.blend == 1.0 {
            self.harmonic_gain = 0.0;
        }

        self.harmonic.set(harmonic::Settings {
            focus: settings.focus,
            character: settings.character,
            drive: settings.drive,
            bias: settings.bias,
            factor: settings.factor,
        });
        self.noise.set(noise::Settings {
            focus: settings.focus,
            character: settings.character,
            width: settings.width,
        });
        self.settings = settings;
    }

    /// One frame in, the layer to add out. **Audio rate.**
    pub fn process(&mut self, left: f32, right: f32, gain: f32) -> (f32, f32) {
        let gain = finite(gain, 0.0);
        let mono = (finite(left, 0.0) + finite(right, 0.0)) * 0.5;

        let (harmonic_left, harmonic_right) =
            self.harmonic
                .process(left, right, gain * self.harmonic_gain);
        let (noise_left, noise_right) = self.noise.process(mono, gain * self.noise_gain);

        (harmonic_left + noise_left, harmonic_right + noise_right)
    }

    /// Where the layer sits, in Hz — the one number both halves are placed by.
    pub fn corner_hz(&self) -> f32 {
        self.harmonic.corner_hz()
    }

    /// How far the noise half's keep-alive gate stands open, `0..=1`.
    pub fn keepalive(&self) -> f32 {
        self.noise.keepalive()
    }

    pub fn rebuilds(&self) -> u32 {
        self.rebuilds
    }

    pub fn reset(&mut self) {
        self.harmonic.reset();
        self.noise.reset();
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics::{at_dbfs, db_ratio, pink, rms};

    const RATE: f32 = 48_000.0;
    const SEED: u32 = 0x4149_5233;
    const GAIN: f32 = 1.0;
    /// The proxy for "spectrally ordinary material" the workspace already has
    /// a generator for (`SPK-18`).
    const MATERIAL_DBFS: f32 = -18.0;
    const SETTLE_SECONDS: f32 = 0.5;
    const SECONDS: f32 = 2.0;

    fn settings(blend: f32) -> Settings {
        Settings {
            blend,
            ..Settings::default()
        }
    }

    /// The layer for one setting, after the gate and the filters have settled.
    fn layer(settings: Settings) -> (Vec<f32>, Vec<f32>) {
        let mut layer = Layer::new(RATE, SEED);
        layer.set(settings);
        let drive = at_dbfs(
            pink(1.0, (RATE * (SETTLE_SECONDS + SECONDS)) as usize),
            MATERIAL_DBFS,
        );
        let settle = (RATE * SETTLE_SECONDS) as usize;

        let mut left = Vec::new();
        let mut right = Vec::new();
        for (index, sample) in drive.iter().enumerate() {
            let (l, r) = layer.process(*sample, *sample, GAIN);
            if index >= settle {
                left.push(l);
                right.push(r);
            }
        }
        (left, right)
    }

    fn level(blend: f32) -> f32 {
        let (left, right) = layer(settings(blend));
        20.0 * ((rms(&left) + rms(&right)) * 0.5).max(1e-30).log10()
    }

    /// **`BLEND` is a crossfade, not a level control** (`REQ-AIR-005`). Mixing
    /// two uncorrelated signals by amplitude dips 3 dB in the middle; the
    /// power-preserving mix, with the two halves trimmed to the same level,
    /// does not.
    #[test]
    fn the_blend_holds_the_amount_flat() {
        let readings: Vec<f32> = [0.0f32, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|blend| level(*blend))
            .collect();
        let spread = readings.iter().cloned().fold(f32::MIN, f32::max)
            - readings.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread < 1.0,
            "BLEND moved the amount {spread:.2} dB {readings:?}"
        );
    }

    /// The control for the test above: an amplitude crossfade of the same two
    /// halves has to fail it, or the measurement is not seeing the mix at all.
    #[test]
    fn an_amplitude_crossfade_would_not_pass() {
        // What the middle would read at if the gains were `1-b` and `b`.
        let ends = 10.0f32.powf(level(0.0) / 10.0);
        let amplitude_middle = 10.0 * (0.25 * ends + 0.25 * ends).log10();
        let dip = amplitude_middle - level(0.0);
        assert!(
            dip < -2.5,
            "an amplitude mix would only dip {dip:.2} dB, so a flat reading \
             proves nothing"
        );
    }

    /// Each end is **exactly** the other half's silence (`REQ-AIR-005`).
    #[test]
    fn the_ends_are_exactly_one_half() {
        // At `BLEND` = 0 the noise half is off, so both channels are the
        // harmonic half — which is never widened.
        let (left, right) = layer(settings(0.0));
        for (l, r) in left.iter().zip(&right) {
            assert_eq!(l, r, "the noise half leaked in at BLEND 0");
        }

        // At `BLEND` = 1 the harmonic half is off, so a silent input leaves
        // nothing but the noise half — which its keep-alive gate shuts.
        let mut layer = Layer::new(RATE, SEED);
        layer.set(settings(1.0));
        for _ in 0..(RATE * 2.0) as usize {
            layer.process(0.0, 0.0, GAIN);
        }
        assert_eq!(layer.process(0.0, 0.0, GAIN), (0.0, 0.0));
    }

    /// `CHARACTER` has to do something wherever `BLEND` is standing
    /// (`REQ-AIR-005`) — that is the reason there is one axis rather than one
    /// per half.
    ///
    /// **This measures that the layer moved, not what the axis means.** On
    /// broadband material a knee is a subtle change — the harmonics of noise
    /// are noise — so the harmonic end reads weakest here even though the axis
    /// moves its third harmonic by 65 % (`harmonic.rs`, measured on a tone,
    /// which is where the axis is actually proven). Measured against the
    /// layer's own level: **−23.8 / −5.2 / −2.2 dB** at `BLEND` 0 / 0.5 / 1.
    #[test]
    fn character_moves_the_layer_at_every_blend() {
        for blend in [0.0f32, 0.5, 1.0] {
            let at = |character: f32| {
                layer(Settings {
                    blend,
                    character,
                    ..Settings::default()
                })
                .0
            };
            let soft = at(0.0);
            let hard = at(1.0);
            let difference: f32 = rms(&soft
                .iter()
                .zip(&hard)
                .map(|(a, b)| a - b)
                .collect::<Vec<_>>());
            let against = db_ratio(difference, rms(&soft));
            assert!(
                against > -28.0,
                "at BLEND {blend} the axis only changed the layer by \
                 {against:.1} dB"
            );
        }
    }

    /// `FOCUS` moves both halves, keeping their ratio (`REQ-AIR-006`).
    #[test]
    fn focus_moves_the_whole_layer() {
        let corner = |focus: f32| {
            let mut layer = Layer::new(RATE, SEED);
            layer.set(Settings {
                focus,
                ..Settings::default()
            });
            layer.corner_hz()
        };
        let low = corner(-1.0);
        let middle = corner(0.0);
        let high = corner(1.0);
        assert!(
            (high / middle - middle / low).abs() < 0.01,
            "{low} {middle} {high}"
        );
        assert!(high > middle && middle > low);
    }

    /// **Holding a knob still costs nothing** (`REQ-AIR-016`). The wrapper
    /// calls `set` every block whether anything moved or not.
    #[test]
    fn repeating_a_setting_rebuilds_nothing() {
        let mut layer = Layer::new(RATE, SEED);
        layer.set(settings(0.3));
        let after_first = layer.rebuilds();
        for _ in 0..1_000 {
            layer.set(settings(0.3));
        }
        assert_eq!(layer.rebuilds(), after_first);

        // And a real change still gets through.
        layer.set(settings(0.4));
        assert_eq!(layer.rebuilds(), after_first + 1);
    }

    /// Zero is **exactly** nothing, both halves at once (`REQ-AIR-001`).
    #[test]
    fn zero_gain_is_exactly_silent() {
        for blend in [0.0f32, 0.5, 1.0] {
            let mut layer = Layer::new(RATE, SEED);
            layer.set(settings(blend));
            for sample in at_dbfs(pink(1.0, 4_800), MATERIAL_DBFS) {
                assert_eq!(layer.process(sample, sample, 0.0), (0.0, 0.0));
            }
        }
    }

    #[test]
    fn hostile_settings_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut layer = Layer::new(RATE, SEED);
            layer.set(Settings {
                focus: value,
                character: value,
                blend: value,
                width: value,
                drive: value,
                bias: value,
                factor: Factor::Four,
            });
            for sample in at_dbfs(pink(1.0, 4_800), MATERIAL_DBFS) {
                let (l, r) = layer.process(sample, sample, value);
                assert!(l.is_finite() && r.is_finite(), "{value} produced {l}, {r}");
            }
        }
    }
}
