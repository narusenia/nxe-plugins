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
use crate::density::Density;
use crate::emotion;
use crate::envelope::Envelope;
use crate::guard::{GUARDS, Guarded, Guards};
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
    /// How much each guard is allowed to pull, `0..=1`, in the order of
    /// [`crate::guard::GUARDS`]. Zero is exactly off (`REQ-VEL-006`).
    pub guards: [f32; 2],
    /// How much the envelope is allowed to move the curves, `0..=1`
    /// (`crate::emotion`). **Zero is exactly static** (`REQ-VEL-008`).
    pub emotion: f32,
    /// How hard the generator bus's input is compressed, `0..=1`
    /// (`crate::density`). **The dry path never sees this** (`REQ-VEL-007`).
    pub density: f32,
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
            // On by default: the plugin's promise is that it does not get
            // painful, and a protection nobody switched on does not keep it.
            guards: [1.0; 2],
            // Not zero: this is the plugin's whole differentiation, and a
            // feature nobody switched on is a feature nobody heard
            // (`REQ-VEL-008`).
            emotion: 0.5,
            // Off: `DENSITY` decides how much the plugin ignores the
            // performance, and that is a choice rather than a default.
            density: 0.0,
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
    /// **One set for both channels**, fed the mono sum: a guard that fired on
    /// one side only would move the voice sideways (`REQ-VEL-011`).
    guards: Guards,
    guard_amounts: [f32; 2],
    /// **One detector, fed the mono sum, read before anything else touches the
    /// signal** — shared with `DENSITY` (`crate::envelope`).
    envelope: Envelope,
    /// **One for both channels**, driven by the shared detector — a compressor
    /// per channel would move the image (`REQ-VEL-011`).
    density: Density,
}

/// Everything `set_shape` has to notice a change in before rebuilding the
/// curves. A tuple would compile and would not say which field is which.
#[derive(Clone, Copy, PartialEq)]
struct CurveState {
    drive: f32,
    texture: f32,
    texture_offsets: [f32; BAND_COUNT],
    bias: [f32; BAND_COUNT],
    /// `EMOTION`'s deflection, already multiplied by its amount — so at amount
    /// zero this is a constant zero and the envelope cannot cause a rebuild.
    /// That is what makes "completely static" exact rather than nearly
    /// (`REQ-VEL-008`).
    motion: f32,
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
                motion: f32::NAN,
            },
            solo: [false; BAND_COUNT],
            soloing: false,
            guards: Guards::new(host_rate),
            guard_amounts: [1.0; 2],
            envelope: Envelope::new(host_rate),
            density: Density::new(),
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
        self.guard_amounts = shape.guards;

        // Read once per block, from the envelope the audio path has been
        // feeding. `EMOTION` is a curve change, and the curves are block rate
        // (`crate::emotion`).
        let amount = clamp_or(shape.emotion, 0.0, 1.0, 0.0);
        let motion = amount * emotion::deflection(self.envelope.decibels());

        // Its own state, not part of `CurveState`: `DENSITY` moves a gain in
        // front of the bands and leaves the curves alone. That is the same
        // orthogonality the detector's placement buys — `DENSITY` levels how
        // *much* texture is made, `EMOTION` chooses *which*
        // (`REQ-VEL-007`, `REQ-VEL-008`).
        self.density.set(shape.density);

        let curve = CurveState {
            drive: shape.drive,
            texture: shape.texture,
            texture_offsets: shape.texture_offsets,
            bias: shape.bias,
            motion,
        };
        if curve != self.curve {
            self.curve = curve;
            let drive = drive_of(shape.drive);
            for (index, shaper) in self.shapers.iter_mut().enumerate() {
                let (_, curve_drive) = Band::curve_multipliers(BANDS[index]);
                let point =
                    texture::for_band(shape.texture, shape.texture_offsets[index], index);
                let bias = clamp_or(shape.bias[index], -1.0, 1.0, 0.0);

                let (curve_bias, hardness, band_drive) = emotion::modulate(
                    point.bias,
                    point.hardness,
                    drive * curve_drive * (bias * BIAS_OCTAVES).exp2(),
                    motion,
                );
                shaper.set(band_drive, curve_bias, hardness);
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
        // The guards run **before** the split, at the host rate, on the mono
        // sum — one detector for two channels and for both bands.
        let mono = (input.0 + input.1) * 0.5;
        self.guards.push(mono, self.guard_amounts);
        // **Before the generator bus**, which is where `DENSITY`'s compressor
        // lives: the detector has to see the signal the singer produced, not
        // the one that has already been levelled (`REQ-VEL-008`).
        self.envelope.push(mono);

        let Self {
            shapers,
            trims,
            channels,
            solo,
            soloing,
            guards,
            envelope,
            density,
            ..
        } = self;

        // One reading per sample, shared by both channels. Computed out here
        // rather than inside the oversampled closure: the level does not change
        // between a sample's phases, and computing it there would pay for the
        // two transcendentals two or four times over.
        let compression = density.gain(envelope.level());

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
                    return 0.0;
                }
                // A guard multiplies the generator's output gain and nothing
                // else, so it belongs here with the levels rather than in the
                // curve (`crate::guard`).
                levels.bands[band] * guard_gain(guards, band)
            });
            let dry_gain = if *soloing { 0.0 } else { 1.0 };

            // The band levels are applied **inside** the oversampled loop. They
            // are scalars, so it makes no arithmetic difference — but keeping
            // them outside would mean carrying three signals through three
            // downsamplers instead of one.
            let wet = oversampler.process(dry[index], |value| {
                // **`DENSITY` is applied here**: after the oversampling, before
                // the split, and on nothing else. The dry path below is a
                // separate addition and never sees it (`REQ-VEL-007`).
                let value = value * compression;
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

    /// How far each guard is pulling right now, in dB, in the order of
    /// [`crate::guard::GUARDS`] — for the display (`REQ-VEL-018`).
    pub fn guard_reductions(&self) -> [f32; 2] {
        std::array::from_fn(|index| self.guards.reduction_db(GUARDS[index]))
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        // Otherwise a guard that was pulling when the transport stopped is
        // still pulling when it starts again.
        self.guards.reset();
        self.envelope.reset();
    }
}

/// Which guard, if any, watches a band. **BODY has none**: a muddy low end is
/// solved by `FOCUS` and by that band's own fader, and a detector there would
/// only add a way for it to be unclear whether the plugin is working
/// (`dsp.md`).
fn guard_gain(guards: &Guards, band: usize) -> f32 {
    match BANDS[band] {
        Band::Body => 1.0,
        Band::Presence => guards.gain(Guarded::Presence),
        Band::Air => guards.gain(Guarded::Air),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::{sine, tone};

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
    ///
    /// **`EMOTION` off**, because with it on the block size is exactly what
    /// decides how often the envelope is sampled — that is the feature, and
    /// `emotion_moves_with_the_block_and_only_when_it_is_on` is where it is
    /// pinned.
    #[test]
    fn the_block_size_does_not_change_the_output() {
        let shape = Shape {
            drive: 0.6,
            focus: 0.2,
            emotion: 0.0,
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

    /// Renders the way a host does: `set_shape` once per block, then the
    /// samples. **Anything the envelope feeds has to be measured this way** —
    /// [`rendered`] resolves the curves before the detector has heard a single
    /// sample.
    fn blocked(shape: &Shape, levels: &Levels, input: &[f32], block: usize) -> Vec<f32> {
        let mut engine = Engine::new(RATE);
        let mut output = Vec::new();
        for chunk in input.chunks(block) {
            engine.set_shape(shape);
            for sample in chunk {
                output.push(engine.process((*sample, *sample), levels).0);
            }
        }
        output
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

    /// A signal that is mostly presence band — what a guard is there for.
    ///
    /// Frequencies rather than cycle counts, because writing 3 kHz as "1500
    /// cycles" is only true for one buffer length (`crate::harmonics::tone`).
    fn harsh(length: usize) -> Vec<f32> {
        let body = tone(0.10, 200.0, RATE, length);
        let bite = tone(0.45, 3_000.0, RATE, length);
        body.iter().zip(&bite).map(|(a, b)| a + b).collect()
    }

    /// **The promise being kept where it can be seen** (`REQ-VEL-006`): the
    /// guard reaches the generator, not just the detector.
    #[test]
    fn a_guard_pulls_its_band_back() {
        let input = harsh(24_000);
        let open = levels(0.8, 1.0);
        // Soloed, so the reading is the layer the guard acts on rather than the
        // dry it is buried in.
        let base = Shape {
            drive: 0.6,
            solo: [false, true, false],
            ..Shape::default()
        };

        let unguarded = rendered(
            &Shape {
                guards: [0.0; 2],
                ..base
            },
            &open,
            &input,
        );
        let guarded = rendered(&base, &open, &input);

        let settled = input.len() / 2;
        let loud = crate::harmonics::rms(&unguarded[settled..]);
        let held = crate::harmonics::rms(&guarded[settled..]);
        let difference = crate::harmonics::db_ratio(held, loud);

        assert!(difference < -2.0, "the guard changed nothing: {difference:+.1} dB");
    }

    /// And with the amount at zero it must be **exactly** absent, not nearly.
    #[test]
    fn a_guard_at_zero_reports_and_does_nothing() {
        let input = harsh(24_000);
        let open = levels(0.8, 1.0);
        let mut engine = Engine::new(RATE);
        engine.set_shape(&Shape {
            drive: 0.6,
            guards: [0.0; 2],
            ..Shape::default()
        });

        for sample in &input {
            engine.process((*sample, *sample), &open);
            assert_eq!(engine.guard_reductions(), [0.0, 0.0]);
        }
    }

    /// The guard is fed the mono sum, which is the whole reason it cannot move
    /// the image (`REQ-VEL-011`). Worth its own check, because a per-channel
    /// detector would pass every other test here.
    #[test]
    fn a_guard_does_not_move_the_image() {
        let input = harsh(24_000);
        let open = levels(0.8, 1.0);
        let mut engine = Engine::new(RATE);
        engine.set_shape(&Shape {
            drive: 0.6,
            ..Shape::default()
        });

        // Hard left: the guard still sees it through the sum, and both channels
        // get the same treatment.
        for sample in &input {
            let (left, right) = engine.process((*sample, 0.0), &open);
            assert_eq!(right, 0.0, "silence on the right stopped being silent");
            assert!(left.is_finite());
        }
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
                guards: [value; 2],
                emotion: value,
                density: value,
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

    /// **`amount = 0` is exactly static** (`REQ-VEL-008`), and the way to see
    /// it is a signal whose level is moving fast: if the envelope reached the
    /// curves at all, sampling it every block instead of every sample would
    /// change the output.
    ///
    /// The same test the other way round says the feature is actually wired —
    /// with the amount up, the block size *does* change the output, because the
    /// envelope is read once per block.
    #[test]
    fn emotion_moves_with_the_block_and_only_when_it_is_on() {
        // A tone that fades in over the buffer, so the envelope is somewhere
        // different in every block.
        let length = 24_000;
        let input: Vec<f32> = tone(1.0, 220.0, RATE, length)
            .iter()
            .enumerate()
            .map(|(index, sample)| sample * index as f32 / length as f32)
            .collect();
        let open = levels(0.8, 1.0);

        let render = |emotion: f32, block: usize| {
            blocked(
                &Shape {
                    drive: 0.6,
                    emotion,
                    ..Shape::default()
                },
                &open,
                &input,
                block,
            )
        };

        assert_eq!(
            render(0.0, 1),
            render(0.0, 512),
            "the envelope reached the curves with the amount at zero"
        );
        assert_ne!(
            render(1.0, 1),
            render(1.0, 512),
            "the envelope never reached the curves"
        );
    }

    /// **The direction the feature claims** (`REQ-VEL-008`, `dsp.md`): sung
    /// harder comes out less even and more odd.
    ///
    /// Compared against `EMOTION` off **at the same input level**, because the
    /// curve makes more harmonics from a louder signal whatever `EMOTION` does —
    /// comparing a loud render against a quiet one would measure that instead.
    #[test]
    fn a_loud_phrase_loses_even_harmonics_and_gains_odd() {
        // The analysed half has to hold a whole number of cycles for the
        // harmonic bins to land on the harmonics.
        const SETTLED: usize = 16_384;
        const CYCLES: usize = 100;
        let hertz = RATE * CYCLES as f32 / SETTLED as f32;
        // −3 dB, which is far past `emotion::REF_DB`, so the deflection is
        // pinned at +1.
        let input = tone(0.7, hertz, RATE, SETTLED * 2);

        // BODY alone: its output filter passes the second and third harmonics
        // of a 293 Hz tone, and soloing takes the layer rather than the sum it
        // is buried in.
        //
        // **Rendered block by block, not with `rendered`.** That helper calls
        // `set_shape` once before the first sample, when the envelope has still
        // only seen silence — so `EMOTION` would read the deflection as −1 and
        // every direction here would come out backwards.
        let ratios = |emotion: f32| {
            let wet = blocked(
                &Shape {
                    drive: 0.6,
                    emotion,
                    solo: [true, false, false],
                    ..Shape::default()
                },
                &levels(0.8, 1.0),
                &input,
                64,
            );
            let settled = &wet[SETTLED..];
            let first = crate::harmonics::amplitude(settled, CYCLES);
            (
                crate::harmonics::amplitude(settled, CYCLES * 2) / first,
                crate::harmonics::amplitude(settled, CYCLES * 3) / first,
            )
        };

        let (even_off, odd_off) = ratios(0.0);
        let (even_on, odd_on) = ratios(1.0);

        assert!(
            even_on < even_off * 0.9,
            "H2/H1 went {even_off:.4} -> {even_on:.4}"
        );
        assert!(
            odd_on > odd_off * 1.1,
            "H3/H1 went {odd_off:.4} -> {odd_on:.4}"
        );
    }

    /// The detector is fed the mono sum, so a line panned hard to one side
    /// still moves the character — and moves it the same for both channels
    /// (`REQ-VEL-011`).
    #[test]
    fn one_channel_still_drives_the_detector() {
        let input = tone(0.9, 293.0, RATE, 24_000);
        let open = levels(0.8, 1.0);

        // Hard left, and only the left channel's output is looked at: the right
        // has nothing to add to.
        let render = |emotion: f32| {
            let shape = Shape {
                drive: 0.6,
                emotion,
                solo: [true, false, false],
                ..Shape::default()
            };
            let mut engine = Engine::new(RATE);
            let mut output = Vec::new();
            for chunk in input.chunks(64) {
                engine.set_shape(&shape);
                for sample in chunk {
                    output.push(engine.process((*sample, 0.0), &open).0);
                }
            }
            output[12_000..].to_vec()
        };

        let off = render(0.0);
        let on = render(1.0);
        let difference = crate::harmonics::rms(
            &off.iter().zip(&on).map(|(a, b)| a - b).collect::<Vec<f32>>(),
        );
        assert!(
            difference > crate::harmonics::rms(&off) * 0.02,
            "a one-sided signal left the curves alone"
        );
    }

    /// **The structural promise** (`REQ-VEL-007`): `DENSITY` is on the
    /// generator bus, so the dry path cannot feel it however far it is pushed.
    #[test]
    fn density_cannot_reach_the_dry_path() {
        let input = tone(0.6, 220.0, RATE, 24_000);
        let shape = Shape {
            drive: 0.6,
            density: 1.0,
            ..Shape::default()
        };

        // Mix closed, and separately every fader down: two different ways for
        // the wet to be absent, and both have to leave the input exactly.
        for levels in [levels(0.8, 0.0), levels(0.0, 1.0)] {
            assert_eq!(
                blocked(&shape, &levels, &input, 64),
                input,
                "the dry path moved"
            );
        }
    }

    /// **What the control is for** (`REQ-VEL-007`): the difference in how much
    /// texture a quiet phrase and a loud one get, gets smaller.
    ///
    /// Measured on the soloed layer, so the reading is the texture rather than
    /// the dry it would otherwise be buried in.
    #[test]
    fn density_narrows_the_gap_between_a_quiet_phrase_and_a_loud_one() {
        let hertz = 293.0;
        let quiet = tone(0.05, hertz, RATE, 24_000);
        let loud = tone(0.8, hertz, RATE, 24_000);

        let gap = |density: f32| {
            let shape = Shape {
                drive: 0.6,
                density,
                // `EMOTION` off: it also changes with level, and this is a
                // measurement of `DENSITY`.
                emotion: 0.0,
                solo: [true, false, false],
                ..Shape::default()
            };
            let layer = |input: &[f32]| {
                let wet = blocked(&shape, &levels(0.8, 1.0), input, 64);
                crate::harmonics::rms(&wet[12_000..])
            };
            crate::harmonics::db_ratio(layer(&loud), layer(&quiet))
        };

        let open = gap(0.0);
        let compressed = gap(1.0);
        // Measured: **23.7 dB -> 6.0 dB** on a 24 dB span of input.
        assert!(
            compressed < open - 6.0,
            "the gap went {open:.1} dB -> {compressed:.1} dB"
        );
    }

    /// **The proof that the detector is pre-compression** (`REQ-VEL-008`).
    ///
    /// Not measured through the sound — `DENSITY` changes the level going into
    /// the curves, so the harmonics move whatever `EMOTION` does. What has to
    /// hold is that the curves themselves come out identical: the deflection is
    /// a function of the input, and `DENSITY` is downstream of where it is read.
    #[test]
    fn density_does_not_change_what_emotion_does() {
        let input = tone(0.7, 293.0, RATE, 24_000);
        let open = levels(0.8, 1.0);

        let curves = |density: f32| {
            let shape = Shape {
                drive: 0.6,
                emotion: 1.0,
                density,
                ..Shape::default()
            };
            let mut engine = Engine::new(RATE);
            for chunk in input.chunks(64) {
                engine.set_shape(&shape);
                for sample in chunk {
                    engine.process((*sample, *sample), &open);
                }
            }
            engine
                .shapers
                .iter()
                .map(|shaper| (shaper.drive(), shaper.bias(), shaper.hardness()))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            curves(0.0),
            curves(1.0),
            "`DENSITY` moved the detector `EMOTION` reads"
        );
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

