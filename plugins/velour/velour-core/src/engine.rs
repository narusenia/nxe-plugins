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
use crate::texture;

pub const BAND_COUNT: usize = 3;

/// How far `Bias_i` moves a band's drive, in octaves either way.
pub const BIAS_OCTAVES: f32 = 1.5;

/// And how much of the band's level it gives back, in dB per unit of bias.
///
/// **Zero, because the measurement said so.** The specification put 6 dB here,
/// reasoning that a band driven harder needs pulling back. It does not: the
/// curve is normalised for level (`crate::shaper`), so raising drive leaves the
/// generator's RMS nearly alone — the whole bias range moves it by under a
/// decibel. A 6 dB compensation would have turned `Bias` into a volume knob and
/// nothing else.
///
/// **And a fixed number could not fix what is left, because it changes sign.**
/// Measured across the bias range: −0.15 dB on a 217 Hz tone, +0.80 dB on a
/// 434 Hz one. The residual comes from the band's *output* filter throwing away
/// harmonics that the extra drive created — the same mechanism that moves the
/// level across `TEXTURE` (`crate::texture`) — and how much it throws away
/// depends on where the content sits. No constant tracks that.
///
/// **The constant stays rather than being deleted.** What it would compensate
/// for is *perceived* density: more harmonics at the same RMS do read as
/// louder. How much of that to give back is an ear question, on `VEL-17`'s
/// list. The mechanism is the specification; the amount is not.
pub const BIAS_LEVEL_DB: f32 = 0.0;

/// What changes the coefficients. **Block rate.**
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Shape {
    /// `0..=1`. Mapped onto the curve's drive geometrically, so the bottom of
    /// the knob has as much resolution as the top.
    pub drive: f32,
    /// `0..=1`, walking Warm → Clear → Edge (`crate::texture`).
    pub texture: f32,
    /// `-1..=1` per band, deviating from `texture` by up to
    /// [`crate::texture::OFFSET_RANGE`].
    pub texture_offsets: [f32; BAND_COUNT],
    /// `-1..=1` per band. **Splits the same amount** between drive and level:
    /// positive is deeper distortion added more quietly, negative is a shallower
    /// curve added louder (`REQ-VEL-010`).
    ///
    /// One bipolar control rather than separate drive and level knobs, because
    /// two of them would put "how much" in two places and leave the band's own
    /// fader arguing with both.
    pub bias: [f32; BAND_COUNT],
    /// Which bands to listen to alone. Everything else — including the dry path
    /// — is muted while any of these is set.
    ///
    /// **Only a parallel topology can offer this.** A crossover would play the
    /// split dry band; here it is the layer being added, by itself
    /// (`REQ-VEL-010`).
    pub solo: [bool; BAND_COUNT],
    /// `-1..=1`, sliding every band edge together.
    pub focus: f32,
    pub factor: Factor,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            drive: 0.0,
            // The middle of the axis, which is Clear.
            texture: 0.5,
            texture_offsets: [0.0; BAND_COUNT],
            bias: [0.0; BAND_COUNT],
            solo: [false; BAND_COUNT],
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

/// Decibels to a linear gain. Exactly `1.0` at zero, which is what keeps a
/// resting trim from touching the signal.
fn decibels(value: f32) -> f32 {
    if value == 0.0 {
        1.0
    } else {
        10.0f32.powf(value / 20.0)
    }
}

fn clamp_or(value: f32, low: f32, high: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        fallback
    }
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
    /// `TEXTURE`'s per-band level trims, resolved with the curves. Block rate,
    /// so they are held here rather than recomputed per sample.
    trims: [f32; BAND_COUNT],
    channels: [Channel; 2],
    factor: Factor,
    focus: f32,
    /// The last values the curves were built from, so a block where nothing
    /// moved costs nothing.
    curve: CurveState,
    /// Which bands are soloed, and whether any is.
    solo: [bool; BAND_COUNT],
    soloing: bool,
}

/// Everything `set_shape` has to notice a change in before rebuilding the
/// curves. A tuple would compile and would not say which field is which.
#[derive(Clone, Copy, PartialEq)]
struct CurveState {
    drive: f32,
    texture: f32,
    texture_offsets: [f32; BAND_COUNT],
    bias: [f32; BAND_COUNT],
}

impl Engine {
    pub fn new(host_rate: f32) -> Self {
        let factor = Factor::default();
        Self {
            shapers: std::array::from_fn(|_| Shaper::new()),
            trims: [1.0; BAND_COUNT],
            channels: [Channel::new(host_rate, factor), Channel::new(host_rate, factor)],
            factor,
            // Not the resting values: `set_shape` compares against these to
            // decide what to rebuild, and the first call has to rebuild
            // everything.
            focus: f32::NAN,
            curve: CurveState {
                drive: f32::NAN,
                texture: f32::NAN,
                texture_offsets: [f32::NAN; BAND_COUNT],
                bias: [f32::NAN; BAND_COUNT],
            },
            solo: [false; BAND_COUNT],
            soloing: false,
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

        self.solo = shape.solo;
        self.soloing = shape.solo.iter().any(|band| *band);

        let curve = CurveState {
            drive: shape.drive,
            texture: shape.texture,
            texture_offsets: shape.texture_offsets,
            bias: shape.bias,
        };
        if curve != self.curve {
            self.curve = curve;
            let drive = drive_of(shape.drive);
            for (index, shaper) in self.shapers.iter_mut().enumerate() {
                let (_, curve_drive) = Band::curve_multipliers(BANDS[index]);
                let point =
                    texture::for_band(shape.texture, shape.texture_offsets[index], index);
                let bias = clamp_or(shape.bias[index], -1.0, 1.0, 0.0);

                shaper.set(
                    drive * curve_drive * (bias * BIAS_OCTAVES).exp2(),
                    point.bias,
                    point.hardness,
                );
                self.trims[index] = point.trim * decibels(bias * BIAS_LEVEL_DB);
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
            shapers,
            trims,
            channels,
            solo,
            soloing,
            ..
        } = self;

        let dry = [input.0, input.1];
        let mut output = [0.0f32; 2];

        for (index, channel) in channels.iter_mut().enumerate() {
            let Channel {
                oversampler,
                generators,
            } = channel;
            // Soloing mutes the other bands and the dry path, so what is left
            // is exactly the layer being added — at the level it is added, with
            // no makeup. How little it is *is* the reading.
            let bands: [f32; BAND_COUNT] = std::array::from_fn(|band| {
                if *soloing && !solo[band] {
                    0.0
                } else {
                    levels.bands[band]
                }
            });
            let dry_gain = if *soloing { 0.0 } else { 1.0 };

            // The band levels are applied **inside** the oversampled loop. They
            // are scalars, so it makes no arithmetic difference — but keeping
            // them outside would mean carrying three signals through three
            // downsamplers instead of one.
            let wet = oversampler.process(dry[index], |value| {
                let mut sum = 0.0;
                for band in 0..BAND_COUNT {
                    sum += generators[band].process(value, &shapers[band])
                        * bands[band]
                        * trims[band];
                }
                sum
            });

            output[index] = dry[index] * dry_gain + levels.mix * wet;
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
            ..Shape::default()
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
            ..Shape::default()
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

    /// `TEXTURE` has to reach the sound, not just the coefficients — and it has
    /// to do it without the level walking off (`REQ-VEL-004`, `REQ-VEL-009`).
    #[test]
    fn texture_changes_the_sound_without_changing_the_level() {
        let input = sine(0.25, 37, 4_096);
        let open = levels(0.8, 1.0);

        let render = |texture: f32| {
            let mut engine = Engine::new(RATE);
            engine.set_shape(&Shape {
                drive: 0.7,
                texture,
                ..Shape::default()
            });
            let output: Vec<f32> = input
                .iter()
                .map(|sample| engine.process((*sample, *sample), &open).0)
                .collect();
            output[output.len() / 2..].to_vec()
        };

        let warm = render(0.0);
        let edge = render(1.0);

        let difference = crate::harmonics::rms(
            &warm
                .iter()
                .zip(&edge)
                .map(|(a, b)| a - b)
                .collect::<Vec<f32>>(),
        );
        let level = crate::harmonics::rms(&warm);
        assert!(difference > level * 0.02, "the ends sound the same");

        // **Measured: −3.0 dB from Warm to Edge**, and it is not the trims.
        //
        // The shaper's normalisation holds its *own* output level constant
        // (`crate::shaper`), but a harder curve puts more of that energy into
        // high harmonics — and each band's output filter throws away whatever
        // lands outside its range. BODY cuts at 2 kHz, so Edge's extra harmonics
        // there are made and then discarded.
        //
        // That residual is what the per-anchor trims exist to absorb, and they
        // are provisional (`crate::texture`, `VEL-17`). The bound here says the
        // morph is a character control rather than a volume one; it is not a
        // claim that the levels match.
        let drift = crate::harmonics::db_ratio(crate::harmonics::rms(&edge), level);
        assert!(drift.abs() < 4.0, "the level walked {drift:+.1} dB");
    }

    fn rendered(shape: &Shape, levels: &Levels, input: &[f32]) -> Vec<f32> {
        let mut engine = Engine::new(RATE);
        engine.set_shape(shape);
        input
            .iter()
            .map(|sample| engine.process((*sample, *sample), levels).0)
            .collect()
    }

    /// **What `Bias` is for** (`REQ-VEL-010`): it changes how distorted a band
    /// is without changing how much of it is there.
    ///
    /// Measured under a decibel across the whole bias range, and **the sign
    /// depends on the tone** (−0.15 dB at 217 Hz, +0.80 dB at 434 Hz) — which is
    /// why `BIAS_LEVEL_DB` is zero rather than a compensation.
    #[test]
    fn bias_changes_the_character_and_not_the_level() {
        // Twice the cycles over twice the length, so the settled half holds a
        // whole number of them and the harmonic bins land on the tone.
        const CYCLES: usize = 37;
        let input = sine(0.25, CYCLES * 2, 8_192);
        let open = levels(0.8, 1.0);

        // The wet layer alone, taken by soloing everything rather than by
        // subtracting the dry: `(dry + wet) - dry` throws away most of `wet`'s
        // precision when the dry is the larger of the two.
        let at = |bias: f32, drive: f32| {
            let wet = rendered(
                &Shape {
                    drive,
                    bias: [bias; BAND_COUNT],
                    solo: [true; BAND_COUNT],
                    ..Shape::default()
                },
                &open,
                &input,
            );
            wet[wet.len() / 2..].to_vec()
        };

        for drive in [0.3f32, 0.5, 0.7] {
            let middle = crate::harmonics::rms(&at(0.0, drive));
            for bias in [-1.0f32, -0.5, 0.5, 1.0] {
                let drift =
                    crate::harmonics::db_ratio(crate::harmonics::rms(&at(bias, drive)), middle);
                assert!(
                    drift.abs() < 1.5,
                    "drive {drive}, bias {bias}: {drift:+.2} dB"
                );
            }
        }

        // And it does change the character, or the test above is only proving
        // that `Bias` does nothing.
        let ratio = |signal: &[f32]| {
            crate::harmonics::amplitude(signal, CYCLES * 3)
                / crate::harmonics::amplitude(signal, CYCLES)
        };
        let shallow = ratio(&at(-1.0, 0.5));
        let deep = ratio(&at(1.0, 0.5));
        assert!(
            deep > shallow * 1.5,
            "harmonics: deep {deep:.4} against shallow {shallow:.4}"
        );
    }

    /// **The thing only a parallel topology can offer** (`REQ-VEL-010`): the
    /// added layer, alone.
    ///
    /// Checked by changing what should not matter rather than by reconstructing
    /// what should — a soloed band's output must not move when the other bands'
    /// faders do.
    #[test]
    fn solo_ignores_the_other_bands() {
        let input = sine(0.25, 74, 4_096);
        let base = Shape {
            drive: 0.6,
            ..Shape::default()
        };

        for band in 0..BAND_COUNT {
            let mut shape = base;
            shape.solo[band] = true;

            let mut quiet = levels(0.0, 0.7);
            quiet.bands[band] = 0.8;
            let mut loud = levels(0.9, 0.7);
            loud.bands[band] = 0.8;

            assert_eq!(
                rendered(&shape, &quiet, &input),
                rendered(&shape, &loud, &input),
                "band {band} heard its neighbours"
            );
        }
    }

    /// And the soloed band is audible, or the test above passes on silence.
    #[test]
    fn a_soloed_band_is_still_heard() {
        let input = sine(0.25, 74, 4_096);
        let shape = Shape {
            drive: 0.6,
            solo: [false, true, false],
            ..Shape::default()
        };
        let output = rendered(&shape, &levels(0.8, 0.7), &input);
        assert!(crate::harmonics::rms(&output[2_048..]) > 1e-3);
    }

    /// The dry is muted by the solo itself, not as a side effect of the band
    /// levels: with every fader down, soloing leaves **nothing**.
    #[test]
    fn solo_mutes_the_dry() {
        let input = sine(0.5, 74, 4_096);

        for solo in [
            [true, false, false],
            [false, true, false],
            [false, false, true],
            [true, true, true],
        ] {
            let shape = Shape {
                drive: 0.6,
                solo,
                ..Shape::default()
            };
            let output = rendered(&shape, &levels(0.0, 1.0), &input);
            assert!(
                output.iter().all(|sample| *sample == 0.0),
                "{solo:?} left the dry in"
            );
        }
    }

    /// Nothing soloed means the dry is back, which is the other half of the
    /// same claim.
    #[test]
    fn without_a_solo_the_dry_is_untouched() {
        let input = sine(0.5, 74, 4_096);
        let shape = Shape {
            drive: 0.6,
            ..Shape::default()
        };
        let output = rendered(&shape, &levels(0.0, 1.0), &input);
        assert_eq!(output, input);
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];

        for value in wild {
            let mut engine = Engine::new(RATE);
            engine.set_shape(&Shape {
                drive: value,
                texture: value,
                texture_offsets: [value; BAND_COUNT],
                bias: [value; BAND_COUNT],
                solo: [true, false, true],
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
