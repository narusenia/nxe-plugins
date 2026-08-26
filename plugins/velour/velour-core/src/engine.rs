//! The whole generator bus: three curves, three band-limited paths per channel,
//! and one oversampler per channel.
//!
//! **The dry path is not in here.** It is one addition at the end of
//! [`Engine::process`], and that is the only thing that happens to it
//! (`REQ-VEL-001`).
//!
//! The API is split by rate on purpose. [`Engine::set_shape`] rebuilds
//! coefficients and is called once per block; [`Engine::process`] takes the
//! values that are smoothed and is called once per sample. Mixing the two would
//! mean either recomputing filter coefficients per sample or quantising the
//! smoothing to block boundaries.

use crate::bands::{BANDS, Band, Generator};
use crate::oversample::{Factor, Oversampler};
use crate::shaper::{DRIVE_MAX, DRIVE_MIN, Shaper};

pub const BAND_COUNT: usize = 3;

/// Where `TEXTURE`'s Clear anchor sits (`dsp.md`).
///
/// **Fixed for now.** `VEL-4` replaces these two with the morph between Warm,
/// Clear and Edge; until then the plugin sounds like its middle setting, which
/// is enough to hear whether the bus works at all.
const CLEAR_BIAS: f32 = 0.30;
const CLEAR_HARDNESS: f32 = 0.35;

/// What changes the coefficients. **Block rate.**
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shape {
    /// `0..=1`. Mapped onto the curve's drive geometrically, so the bottom of
    /// the knob has as much resolution as the top.
    pub drive: f32,
    /// `-1..=1`, sliding every band edge together.
    pub focus: f32,
    pub factor: Factor,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            drive: 0.0,
            focus: 0.0,
            factor: Factor::default(),
        }
    }
}

/// What is smoothed. **Per sample.**
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Levels {
    /// Linear amplitude per band, in the order of [`crate::bands::BANDS`].
    pub bands: [f32; BAND_COUNT],
    /// The generator bus's master, `0..=1`. At `0` the output is the input.
    pub mix: f32,
}

/// Maps a `0..=1` knob onto the curve's drive.
///
/// Geometric rather than linear: the audible difference between drive 0.1 and
/// 0.2 is about the same as between 4 and 8, and a linear map would put nearly
/// the whole knob in the range where it barely changes.
pub fn drive_of(knob: f32) -> f32 {
    let knob = if knob.is_finite() {
        knob.clamp(0.0, 1.0)
    } else {
        0.0
    };
    DRIVE_MIN * (DRIVE_MAX / DRIVE_MIN).powf(knob)
}

/// One channel's filters. The curves are shared across channels
/// (`crate::bands::Generator`).
struct Channel {
    oversampler: Oversampler,
    generators: [Generator; BAND_COUNT],
}

impl Channel {
    fn new(host_rate: f32, factor: Factor) -> Self {
        Self {
            oversampler: Oversampler::new(),
            generators: std::array::from_fn(|index| {
                Generator::new(BANDS[index], host_rate, factor)
            }),
        }
    }

    fn reset(&mut self) {
        self.oversampler.reset();
        for generator in &mut self.generators {
            generator.reset();
        }
    }
}

pub struct Engine {
    shapers: [Shaper; BAND_COUNT],
    channels: [Channel; 2],
    factor: Factor,
    focus: f32,
    drive: f32,
}

impl Engine {
    pub fn new(host_rate: f32) -> Self {
        let factor = Factor::default();
        Self {
            shapers: std::array::from_fn(|_| Shaper::new()),
            channels: [Channel::new(host_rate, factor), Channel::new(host_rate, factor)],
            factor,
            // Not the resting values: `set_shape` compares against these to
            // decide what to rebuild, and the first call has to rebuild
            // everything.
            focus: f32::NAN,
            drive: f32::NAN,
        }
    }

    /// **Block rate.** Rebuilds only what moved.
    pub fn set_shape(&mut self, shape: &Shape) {
        if shape.factor != self.factor {
            self.factor = shape.factor;
            for channel in &mut self.channels {
                channel.oversampler.set_factor(shape.factor);
                for generator in &mut channel.generators {
                    generator.set_factor(shape.factor);
                }
            }
        }

        if shape.focus != self.focus {
            self.focus = shape.focus;
            for channel in &mut self.channels {
                for generator in &mut channel.generators {
                    generator.set_focus(shape.focus);
                }
            }
        }

        if shape.drive != self.drive {
            self.drive = shape.drive;
            let drive = drive_of(shape.drive);
            for (index, shaper) in self.shapers.iter_mut().enumerate() {
                let (bias, curve_drive) = Band::curve_multipliers(BANDS[index]);
                shaper.set(drive * curve_drive, CLEAR_BIAS * bias, CLEAR_HARDNESS);
            }
        }
    }

    /// **Per sample.** Returns `(left, right)`.
    ///
    /// At `mix == 0` — or with every band at zero — the return value *is* the
    /// input: the dry path is one addition of zero (`REQ-VEL-001`).
    pub fn process(&mut self, input: (f32, f32), levels: &Levels) -> (f32, f32) {
        // Destructured so the shapers can be read while the channels are
        // written; borrowing through `self` inside the closure would not.
        let Self {
            shapers, channels, ..
        } = self;

        let dry = [input.0, input.1];
        let mut output = [0.0f32; 2];

        for (index, channel) in channels.iter_mut().enumerate() {
            let Channel {
                oversampler,
                generators,
            } = channel;
            let bands = levels.bands;

            // The band levels are applied **inside** the oversampled loop. They
            // are scalars, so it makes no arithmetic difference — but keeping
            // them outside would mean carrying three signals through three
            // downsamplers instead of one.
            let wet = oversampler.process(dry[index], |value| {
                let mut sum = 0.0;
                for band in 0..BAND_COUNT {
                    sum += generators[band].process(value, &shapers[band]) * bands[band];
                }
                sum
            });

            output[index] = dry[index] + levels.mix * wet;
        }

        (output[0], output[1])
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::sine;

    const RATE: f32 = 48_000.0;

    fn engine() -> Engine {
        let mut engine = Engine::new(RATE);
        engine.set_shape(&Shape {
            drive: 0.6,
            focus: 0.0,
            factor: Factor::Four,
        });
        engine
    }

    fn levels(bands: f32, mix: f32) -> Levels {
        Levels {
            bands: [bands; BAND_COUNT],
            mix,
        }
    }

    /// **The property the whole topology exists for** (`REQ-VEL-001`).
    #[test]
    fn a_closed_mix_returns_the_input_untouched() {
        let mut engine = engine();
        let input = sine(0.7, 37, 4_096);
        let quiet = levels(0.8, 0.0);

        for sample in &input {
            let (left, right) = engine.process((*sample, *sample * 0.5), &quiet);
            assert_eq!(left, *sample);
            assert_eq!(right, *sample * 0.5);
        }
    }

    #[test]
    fn silent_bands_return_the_input_untouched() {
        let mut engine = engine();
        let input = sine(0.7, 37, 4_096);
        let open = levels(0.0, 1.0);

        for sample in &input {
            let (left, _) = engine.process((*sample, 0.0), &open);
            assert_eq!(left, *sample);
        }
    }

    /// With the mix open something has to actually change, or the test above is
    /// only proving that the engine does nothing at all.
    #[test]
    fn an_open_mix_changes_the_signal() {
        let mut engine = engine();
        let input = sine(0.3, 37, 4_096);
        let open = levels(0.8, 1.0);

        let mut moved = 0;
        for sample in &input {
            let (left, _) = engine.process((*sample, *sample), &open);
            if (left - sample).abs() > 1e-4 {
                moved += 1;
            }
        }
        assert!(moved > input.len() / 2, "only {moved} samples moved");
    }

    /// **The image must not move** (`REQ-VEL-011`). Identical channels in,
    /// identical channels out — which is also what says the two channels'
    /// filter states are not sharing anything.
    #[test]
    fn a_mono_signal_stays_mono() {
        let mut engine = engine();
        let input = sine(0.4, 37, 4_096);
        let open = levels(0.7, 0.8);

        for sample in &input {
            let (left, right) = engine.process((*sample, *sample), &open);
            assert_eq!(left, right, "the channels drifted apart");
        }
    }

    /// The two rates the API is split across have to be independent: calling
    /// `set_shape` more often must not change the sound.
    #[test]
    fn the_block_size_does_not_change_the_output() {
        let shape = Shape {
            drive: 0.6,
            focus: 0.2,
            factor: Factor::Four,
        };
        let open = levels(0.7, 0.9);
        let input = sine(0.3, 37, 4_096);

        let mut reference = Vec::new();
        let mut engine = Engine::new(RATE);
        engine.set_shape(&shape);
        for sample in &input {
            reference.push(engine.process((*sample, *sample), &open).0);
        }

        for block in [1usize, 32, 512, 2_048] {
            let mut engine = Engine::new(RATE);
            let mut output = Vec::new();
            for chunk in input.chunks(block) {
                engine.set_shape(&shape);
                for sample in chunk {
                    output.push(engine.process((*sample, *sample), &open).0);
                }
            }
            assert_eq!(output, reference, "block size {block} disagreed");
        }
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];

        for value in wild {
            let mut engine = Engine::new(RATE);
            engine.set_shape(&Shape {
                drive: value,
                focus: value,
                factor: Factor::Four,
            });

            let open = Levels {
                bands: [value; BAND_COUNT],
                mix: value,
            };
            for _ in 0..256 {
                let (left, right) = engine.process((0.5, -0.5), &open);
                // A hostile *level* is allowed to produce a hostile sample —
                // it is a multiplication the host asked for. What must not
                // happen is a panic, or the filters latching.
                let _ = (left, right);
            }

            // With sane levels afterwards the engine has to still work.
            engine.set_shape(&Shape::default());
            let sane = levels(0.5, 0.5);
            for _ in 0..256 {
                let (left, _) = engine.process((0.5, -0.5), &sane);
                assert!(left.is_finite(), "{value} left the engine broken: {left}");
            }
        }
    }

    #[test]
    fn the_drive_map_covers_the_curve_and_nothing_else() {
        assert!((drive_of(0.0) - DRIVE_MIN).abs() < 1e-6);
        assert!((drive_of(1.0) - DRIVE_MAX).abs() < 1e-4);
        // Geometric, so the midpoint is the geometric mean rather than the
        // arithmetic one.
        let middle = drive_of(0.5);
        assert!((middle - (DRIVE_MIN * DRIVE_MAX).sqrt()).abs() < 1e-4, "{middle}");

        for knob in [f32::NAN, f32::INFINITY, -1.0, 2.0] {
            let drive = drive_of(knob);
            assert!(
                (DRIVE_MIN..=DRIVE_MAX).contains(&drive),
                "{knob} gave {drive}"
            );
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut engine = engine();
        let open = levels(0.8, 1.0);
        for _ in 0..1_024 {
            engine.process((0.5, 0.5), &open);
        }
        engine.reset();
        assert_eq!(engine.process((0.0, 0.0), &open), (0.0, 0.0));
    }
}
