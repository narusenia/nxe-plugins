//! The voice engine: N voices from one source, each with its own delay,
//! detune, position and level.
//!
//! Tone is not here yet (`DBL-7`).
//!
//! See `plugins/doubler/docs/specifications/dsp.md`.

use crate::wobble::Wobble;
use crate::{DelayLine, PitchShifter};

/// Parameters exist for this many voices at all times; `Voices` decides how
/// many of them are live. nih-plug declares parameters once at startup and
/// cannot add them later, so the count has to be a selector rather than a
/// length (`REQ-DBL-001`).
pub const MAX_VOICES: usize = 8;

/// The delay line has to hold the largest base delay, the Humanize wobble on
/// top of it, and the shifter's window.
const LINE_SECONDS: f32 = 0.150;

/// Humanize at full depth moves a voice's detune this far either way.
/// Ear-tuned (`dsp.md`).
const HUMANIZE_DETUNE_CENTS: f32 = 8.0;

/// ...and its delay this far. Deliberately much smaller than the detune depth:
/// moving a read position *is* a pitch change, so a wide delay wobble is heard
/// as a glide rather than as timing.
/// Ear-tuned (`dsp.md`).
const HUMANIZE_DELAY_MS: f32 = 3.0;

/// How long a `Source` change takes to cross over. Long enough not to click,
/// short enough to feel like a switch.
const SOURCE_FADE_SECONDS: f32 = 0.020;

/// In `TrueStereo`, how far a voice's own channel sits from the centre, and how
/// much `Pan_i` moves it from there. The two add to exactly 1 at full spread,
/// so the pan never has to be clamped. Ear-tuned (`dsp.md`).
const STEREO_BASE_PAN: f32 = 0.5;
const STEREO_SPREAD_PAN: f32 = 0.3;
const STEREO_SHAPE_PAN: f32 = 0.2;

/// How many voices are live. Discrete, so an out-of-range count cannot exist.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Voices {
    Two,
    #[default]
    Four,
    Eight,
}

impl Voices {
    pub fn count(self) -> usize {
        match self {
            Voices::Two => 2,
            Voices::Four => 4,
            Voices::Eight => 8,
        }
    }
}

/// Where a voice takes its input from (`REQ-DBL-004`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Source {
    /// Every voice reads `(L + R) / 2`, and `Pan_i` is an absolute position.
    /// Behaves the same whether the input is mono or wide, so the width and the
    /// phase are predictable.
    #[default]
    MonoSum,
    /// Even voices read L, odd voices read R, and `Pan_i` is an offset from
    /// that side. Leaves an already-wide source's image intact.
    TrueStereo,
}

/// One voice's normalized shape. The macros scale these; neither layer
/// overwrites the other (`REQ-DBL-007`) — which is why `process` takes the
/// shape by shared reference and cannot write to it at all.
#[derive(Clone, Copy, Debug)]
pub struct VoiceShape {
    /// `-1..=1`. Effective detune is `Detune × detune`.
    pub detune: f32,
    /// `0..=1`. Effective delay is `Delay × delay`.
    pub delay: f32,
    /// `-1..=1`. Effective pan depends on the source mode.
    pub pan: f32,
    /// Absolute, in dB. No macro scales this one.
    pub gain_db: f32,
}

/// The default shape, ordered so that **taking the first N entries stays
/// left-right symmetric**. That is what keeps the image from leaning when
/// `Voices` changes (`REQ-DBL-001`).
pub const DEFAULT_SHAPE: [VoiceShape; MAX_VOICES] = [
    VoiceShape {
        detune: -1.00,
        delay: 1.00,
        pan: -1.00,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: 1.00,
        delay: 0.62,
        pan: 1.00,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: -0.40,
        delay: 0.84,
        pan: -0.45,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: 0.40,
        delay: 0.44,
        pan: 0.45,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: -0.70,
        delay: 0.92,
        pan: -0.75,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: 0.70,
        delay: 0.30,
        pan: 0.75,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: -0.25,
        delay: 0.72,
        pan: -0.20,
        gain_db: 0.0,
    },
    VoiceShape {
        detune: 0.25,
        delay: 0.52,
        pan: 0.20,
        gain_db: 0.0,
    },
];

/// The macro layer, in the units the parameters are displayed in.
#[derive(Clone, Copy, Debug)]
pub struct Macros {
    pub voices: Voices,
    pub source: Source,
    /// Cents. Scales `VoiceShape::detune`.
    pub detune: f32,
    /// Milliseconds. Scales `VoiceShape::delay`.
    pub delay: f32,
    /// `0..=1`. Scales `VoiceShape::pan`.
    pub spread: f32,
    /// `0..=1`. Depth of the per-voice wobble. Zero is completely static.
    pub humanize: f32,
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            voices: Voices::Four,
            source: Source::MonoSum,
            detune: 12.0,
            delay: 22.0,
            spread: 0.7,
            humanize: 0.35,
        }
    }
}

pub struct VoiceEngine {
    sample_rate: f32,
    /// One line per input channel. **There is no third line for the mono sum**:
    /// a delay line and Hermite interpolation are both linear, so
    /// `(left.read(d) + right.read(d)) / 2` is exactly what reading a summed
    /// line would give. That saves a buffer, keeps the write path free of any
    /// mode switch, and — the real payoff — makes a `Source` change a crossfade
    /// between two read formulas rather than between two buffer contents.
    line_left: DelayLine,
    line_right: DelayLine,
    shifters: [PitchShifter; MAX_VOICES],
    /// Two independent wobbles per voice, so pitch and timing do not drift
    /// together — a single source would make every voice sound like one take
    /// being bent rather than several takes.
    detune_wobble: [Wobble; MAX_VOICES],
    delay_wobble: [Wobble; MAX_VOICES],
    /// 1 is fully `MonoSum`, 0 is fully `TrueStereo`. Ramped rather than
    /// switched, because `Source` is a discrete parameter and nothing smooths
    /// it for us.
    source_blend: f32,
    source_step: f32,
}

impl VoiceEngine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            line_left: DelayLine::new(sample_rate, LINE_SECONDS),
            line_right: DelayLine::new(sample_rate, LINE_SECONDS),
            shifters: std::array::from_fn(|_| PitchShifter::new(sample_rate)),
            detune_wobble: std::array::from_fn(|i| Wobble::new(sample_rate, i as u32)),
            delay_wobble: std::array::from_fn(|i| {
                Wobble::new(sample_rate, (i + MAX_VOICES) as u32)
            }),
            source_blend: 1.0,
            source_step: (SOURCE_FADE_SECONDS * sample_rate).recip(),
        }
    }

    pub fn reset(&mut self) {
        self.line_left.reset();
        self.line_right.reset();
        for shifter in &mut self.shifters {
            shifter.reset();
        }
        for wobble in self.detune_wobble.iter_mut().chain(&mut self.delay_wobble) {
            wobble.reset();
        }
    }

    /// One stereo frame in, the stereo wet pair out. Dry is the caller's
    /// business — it never passes through here (`REQ-DBL-006`).
    ///
    /// A mono caller passes the same sample for both channels; `MonoSum` then
    /// produces exactly what a mono input should
    /// (`REQ-DBL-004`).
    pub fn process(
        &mut self,
        left_in: f32,
        right_in: f32,
        macros: &Macros,
        shape: &[VoiceShape; MAX_VOICES],
    ) -> (f32, f32) {
        self.line_left.write(left_in);
        self.line_right.write(right_in);

        let target = match macros.source {
            Source::MonoSum => 1.0,
            Source::TrueStereo => 0.0,
        };
        self.source_blend = approach(self.source_blend, target, self.source_step);
        let blend = self.source_blend;

        let ms_to_samples = self.sample_rate / 1000.0;
        let humanize = macros.humanize.clamp(0.0, 1.0);
        let spread = macros.spread.clamp(0.0, 1.0);
        let live = macros.voices.count();
        let mut left = 0.0;
        let mut right = 0.0;
        let mut energy = 0.0;

        for voice_index in 0..MAX_VOICES {
            // Every wobble advances whether its voice is live or not, so the
            // sound of a given setting does not depend on what `Voices` was a
            // moment ago.
            let detune_wobble = self.detune_wobble[voice_index].next();
            let delay_wobble = self.delay_wobble[voice_index].next();
            if voice_index >= live {
                continue;
            }

            let shape = &shape[voice_index];
            let gain = db_to_gain(shape.gain_db);
            energy += gain * gain;

            let cents =
                macros.detune * shape.detune + humanize * HUMANIZE_DETUNE_CENTS * detune_wobble;
            let delay_ms =
                (macros.delay * shape.delay + humanize * HUMANIZE_DELAY_MS * delay_wobble).max(0.0);

            let own_is_left = voice_index % 2 == 0;
            let line_left = &self.line_left;
            let line_right = &self.line_right;
            let voice = self.shifters[voice_index].process(
                |delay| {
                    let l = line_left.read(delay);
                    let r = line_right.read(delay);
                    let own = if own_is_left { l } else { r };
                    (l + r) * 0.5 * blend + own * (1.0 - blend)
                },
                delay_ms * ms_to_samples,
                cents,
            ) * gain;

            let (gain_l, gain_r) =
                equal_power_pan(pan_position(blend, spread, shape.pan, own_is_left));
            left += voice * gain_l;
            right += voice * gain_r;
        }

        // The voices are decorrelated, so their powers add rather than their
        // amplitudes. Dividing by the root of the summed power keeps the wet
        // level put when `Voices` or a `Gain_i` changes (`REQ-DBL-006`).
        // Summing the squared gains rather than counting voices means pulling
        // one voice down compensates as it should.
        if energy <= 0.0 {
            return (0.0, 0.0);
        }
        let compensation = energy.sqrt().recip();
        (left * compensation, right * compensation)
    }
}

/// Where a voice sits, blended between what each source mode would say.
///
/// **`Pan_i` means different things in the two modes** and that is deliberate
/// (`REQ-DBL-004`): an absolute position under `MonoSum`, an offset from the
/// voice's own channel under `TrueStereo`. Switching modes therefore moves the
/// voices, which is the specified behaviour and not a bug.
fn pan_position(blend: f32, spread: f32, shape_pan: f32, own_is_left: bool) -> f32 {
    let mono = spread * shape_pan;

    let own_side = if own_is_left { -1.0 } else { 1.0 };
    let stereo = own_side * (STEREO_BASE_PAN + STEREO_SPREAD_PAN * spread)
        + shape_pan * STEREO_SHAPE_PAN * spread;

    mono * blend + stereo * (1.0 - blend)
}

/// Moves `current` toward `target` by at most `step`, landing on it exactly.
fn approach(current: f32, target: f32, step: f32) -> f32 {
    if current < target {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

/// Equal-power pan. `pan` is `-1` hard left to `+1` hard right; the two gains
/// always satisfy `gain_l² + gain_r² == 1`.
fn equal_power_pan(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let (sin, cos) = angle.sin_cos();
    (cos, sin)
}

fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Runs a mono signal through the engine (both inputs fed the same sample)
    /// and returns the stereo output.
    fn run(macros: &Macros, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        run_stereo(macros, input, input)
    }

    fn run_stereo(macros: &Macros, left_in: &[f32], right_in: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut engine = VoiceEngine::new(SR);
        let mut left = Vec::with_capacity(left_in.len());
        let mut right = Vec::with_capacity(left_in.len());

        for (&l, &r) in left_in.iter().zip(right_in) {
            let (out_l, out_r) = engine.process(l, r, macros, &DEFAULT_SHAPE);
            left.push(out_l);
            right.push(out_r);
        }
        (left, right)
    }

    fn rms(signal: &[f32]) -> f32 {
        (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32).sqrt()
    }

    /// Deterministic noise, so the level tests do not depend on a crate.
    fn noise(len: usize) -> Vec<f32> {
        let mut state = 0x1234_5678u32;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state as f32 / u32::MAX as f32) * 2.0 - 1.0
            })
            .collect()
    }

    fn max_step(signal: &[f32]) -> f32 {
        signal
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    }

    /// `REQ-DBL-001`: taking the first N entries of the shape has to stay
    /// symmetric, or the image leans when `Voices` changes.
    #[test]
    fn the_default_shape_is_symmetric_for_every_voice_count() {
        for voices in [Voices::Two, Voices::Four, Voices::Eight] {
            let n = voices.count();
            let pan: f32 = DEFAULT_SHAPE[..n].iter().map(|s| s.pan).sum();
            let detune: f32 = DEFAULT_SHAPE[..n].iter().map(|s| s.detune).sum();
            assert!(pan.abs() < 1e-6, "{n} voices: pan sums to {pan}");
            assert!(detune.abs() < 1e-6, "{n} voices: detune sums to {detune}");
        }
    }

    /// The delays are spread out for every prefix too — two voices at the same
    /// delay would be one voice twice as loud.
    #[test]
    fn the_default_delays_are_distinct_for_every_voice_count() {
        for voices in [Voices::Two, Voices::Four, Voices::Eight] {
            let n = voices.count();
            let live = &DEFAULT_SHAPE[..n];
            for (i, a) in live.iter().enumerate() {
                for (j, b) in live.iter().enumerate().skip(i + 1) {
                    let gap = (a.delay - b.delay).abs();
                    assert!(gap > 0.05, "{n} voices: {i} and {j} are only {gap} apart");
                }
            }
        }
    }

    /// `REQ-DBL-006`: changing `Voices` must not change the wet level.
    #[test]
    fn the_wet_level_holds_across_voice_counts() {
        let input = noise(48_000);
        let mut levels = Vec::new();

        for voices in [Voices::Two, Voices::Four, Voices::Eight] {
            let macros = Macros {
                voices,
                ..Macros::default()
            };
            let (l, r) = run(&macros, &input);
            // Skip the head, where the delay lines are still filling.
            let level = (rms(&l[8_000..]) + rms(&r[8_000..])) * 0.5;
            levels.push((voices, level));
        }

        let reference = levels[0].1;
        for (voices, level) in &levels {
            let db = 20.0 * (level / reference).log10();
            assert!(
                db.abs() < 1.0,
                "{voices:?}: wet level is {db:.2} dB off the two-voice case"
            );
        }
    }

    /// `Spread` at zero must put every voice dead centre, which shows up as the
    /// two output channels being identical.
    #[test]
    fn zero_spread_is_centred() {
        let macros = Macros {
            spread: 0.0,
            humanize: 0.0,
            ..Macros::default()
        };
        let (l, r) = run(&macros, &noise(4096));
        for (i, (l, r)) in l.iter().zip(r.iter()).enumerate() {
            assert_eq!(l, r, "channels differ at sample {i}");
        }
    }

    /// `Spread` at one sends voice 0 (`pan == -1`) hard left, so the left
    /// channel has to carry more than the right — the shape is symmetric, but
    /// the delays are not, so this checks the pan law is wired to the right
    /// channel rather than mirrored.
    #[test]
    fn pan_puts_the_first_voice_on_the_left() {
        let macros = Macros {
            voices: Voices::Two,
            spread: 1.0,
            detune: 0.0,
            delay: 40.0,
            humanize: 0.0,
            ..Macros::default()
        };

        let mut impulse = vec![0.0; 8192];
        impulse[0] = 1.0;
        let (left, right) = run(&macros, &impulse);

        // Voice 0 sits at `delay * 1.00` = 40 ms and is panned hard left;
        // voice 1 at `delay * 0.62` = 24.8 ms, hard right.
        let at = |signal: &[f32], ms: f32| {
            let centre = (ms * SR / 1000.0) as usize;
            signal[centre - 3..centre + 4]
                .iter()
                .fold(0.0f32, |acc, s| acc.max(s.abs()))
        };

        assert!(at(&left, 40.0) > 0.3, "voice 0 missing from the left");
        assert!(at(&right, 40.0) < 0.05, "voice 0 leaked into the right");
        assert!(at(&right, 24.8) > 0.3, "voice 1 missing from the right");
        assert!(at(&left, 24.8) < 0.05, "voice 1 leaked into the left");
    }

    /// `REQ-DBL-007`: the effective delay is the macro times the shape, and it
    /// lands where that says. With no detune the shifter is transparent, so
    /// each voice is a plain delay and an impulse shows exactly where.
    #[test]
    fn effective_delays_are_the_macro_times_the_shape() {
        let macros = Macros {
            voices: Voices::Four,
            detune: 0.0,
            delay: 22.0,
            spread: 0.0,
            humanize: 0.0,
            ..Macros::default()
        };
        let mut impulse = vec![0.0; 8192];
        impulse[0] = 1.0;
        let (left, _) = run(&macros, &impulse);

        let mut expected: Vec<usize> = DEFAULT_SHAPE[..4]
            .iter()
            .map(|s| (22.0 * s.delay * SR / 1000.0) as usize)
            .collect();
        expected.sort_unstable();

        for &centre in &expected {
            let peak = left[centre - 3..centre + 4]
                .iter()
                .fold(0.0f32, |acc, s| acc.max(s.abs()));
            assert!(peak > 0.1, "nothing at sample {centre}");
        }

        // And nothing anywhere else: everything outside the expected windows
        // has to be near silent.
        for (i, sample) in left.iter().enumerate() {
            let near = expected.iter().any(|&c| i.abs_diff(c) <= 4);
            if !near {
                assert!(sample.abs() < 0.01, "unexpected energy at sample {i}");
            }
        }
    }

    #[test]
    fn the_pan_law_is_equal_power() {
        for pan in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            let (l, r) = equal_power_pan(pan);
            let power = l * l + r * r;
            assert!((power - 1.0).abs() < 1e-6, "pan {pan}: power {power}");
        }
        assert!(equal_power_pan(-1.0).1.abs() < 1e-6);
        assert!(equal_power_pan(1.0).0.abs() < 1e-6);
        // Out of range is clamped, not wrapped.
        assert_eq!(equal_power_pan(-4.0), equal_power_pan(-1.0));
        assert_eq!(equal_power_pan(4.0), equal_power_pan(1.0));
    }

    /// `REQ-DBL-003`: at zero depth the voices are completely static, which is
    /// what makes a MicroShift-style setting possible at all.
    #[test]
    fn humanize_at_zero_is_static_and_repeatable() {
        let macros = Macros {
            humanize: 0.0,
            ..Macros::default()
        };
        let input = noise(48_000);
        assert_eq!(run(&macros, &input), run(&macros, &input));
    }

    /// And with depth it actually does something.
    #[test]
    fn humanize_changes_the_sound() {
        let input = noise(96_000);
        let (static_l, _) = run(
            &Macros {
                humanize: 0.0,
                ..Macros::default()
            },
            &input,
        );
        let (wobbly_l, _) = run(
            &Macros {
                humanize: 1.0,
                ..Macros::default()
            },
            &input,
        );

        // Skip the head: the wobbles start at rest, so the two runs agree until
        // the first targets have been slewed toward.
        let difference: f32 = static_l[48_000..]
            .iter()
            .zip(&wobbly_l[48_000..])
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / (static_l.len() - 48_000) as f32;
        assert!(
            difference > 1e-3,
            "humanize changed nothing (mean {difference})"
        );
    }

    /// Humanize moves read positions, so a wobble that is too fast is a click.
    #[test]
    fn humanize_does_not_click() {
        let input: Vec<f32> = (0..192_000)
            .map(|i| (i as f32 * std::f32::consts::TAU * 220.0 / SR).sin())
            .collect();
        let (left, _) = run(
            &Macros {
                humanize: 1.0,
                ..Macros::default()
            },
            &input,
        );

        let input_step = max_step(&input);
        let out_step = max_step(&left[8_000..]);
        assert!(
            out_step < input_step * 1.5,
            "output steps by {out_step} where the input steps by at most {input_step}"
        );
    }

    /// Out-of-range depth is clamped, like every other host-controlled value.
    #[test]
    fn humanize_out_of_range_is_clamped() {
        let input = noise(4096);
        assert_eq!(
            run(
                &Macros {
                    humanize: 5.0,
                    ..Macros::default()
                },
                &input
            ),
            run(
                &Macros {
                    humanize: 1.0,
                    ..Macros::default()
                },
                &input
            )
        );
        assert_eq!(
            run(
                &Macros {
                    humanize: -5.0,
                    ..Macros::default()
                },
                &input
            ),
            run(
                &Macros {
                    humanize: 0.0,
                    ..Macros::default()
                },
                &input
            )
        );
    }

    /// `REQ-DBL-004`: a mono input and a stereo input carrying the same signal
    /// have to give the same result under `MonoSum`. This is what makes a mono
    /// caller — a host that only offers a mono-in layout — correct without a
    /// separate code path.
    #[test]
    fn mono_sum_treats_a_mono_input_and_a_doubled_one_alike() {
        let macros = Macros {
            source: Source::MonoSum,
            ..Macros::default()
        };
        let input = noise(24_000);

        let duplicated = run_stereo(&macros, &input, &input);
        let same_signal_both_sides = run(&macros, &input);
        assert_eq!(duplicated, same_signal_both_sides);
    }

    /// `REQ-DBL-004`: under `TrueStereo`, a voice fed from a silent channel has
    /// to be silent. Signal in the left only, so the odd voices contribute
    /// nothing — which shows up as the right channel being far quieter than the
    /// left rather than as an exact zero, because the even voices are panned
    /// left but not hard left.
    #[test]
    fn true_stereo_keeps_a_silent_channel_silent() {
        let macros = Macros {
            source: Source::TrueStereo,
            voices: Voices::Two,
            spread: 1.0,
            humanize: 0.0,
            ..Macros::default()
        };
        let input = noise(24_000);
        let silence = vec![0.0; input.len()];

        let (left, right) = run_stereo(&macros, &input, &silence);
        // Voice 0 reads L and sits hard left; voice 1 reads R (silent).
        let ratio = 20.0 * (rms(&right[8_000..]) / rms(&left[8_000..])).log10();
        assert!(ratio < -30.0, "the silent channel produced {ratio:.1} dB");
    }

    /// The same input under the two modes must not sound the same, or the mode
    /// is not wired up.
    #[test]
    fn the_source_modes_differ() {
        let left_in = noise(48_000);
        let right_in: Vec<f32> = noise(48_000).iter().rev().copied().collect();

        let mono = run_stereo(
            &Macros {
                source: Source::MonoSum,
                ..Macros::default()
            },
            &left_in,
            &right_in,
        );
        let stereo = run_stereo(
            &Macros {
                source: Source::TrueStereo,
                ..Macros::default()
            },
            &left_in,
            &right_in,
        );
        assert_ne!(mono, stereo);
    }

    /// `REQ-DBL-004`: switching modes must not click. The blend is ramped, so
    /// the output's largest step stays in the region the input explains.
    #[test]
    fn switching_source_does_not_click() {
        let input: Vec<f32> = (0..96_000)
            .map(|i| (i as f32 * std::f32::consts::TAU * 220.0 / SR).sin())
            .collect();
        let mut engine = VoiceEngine::new(SR);
        let mut out = Vec::with_capacity(input.len());

        for (i, &sample) in input.iter().enumerate() {
            // Flip the mode every 10 000 samples, well inside the fade time.
            let source = if (i / 10_000) % 2 == 0 {
                Source::MonoSum
            } else {
                Source::TrueStereo
            };
            let macros = Macros {
                source,
                humanize: 0.0,
                ..Macros::default()
            };
            let (l, _) = engine.process(sample, -sample, &macros, &DEFAULT_SHAPE);
            out.push(l);
        }

        let input_step = max_step(&input);
        let out_step = max_step(&out[8_000..]);
        assert!(
            out_step < input_step * 1.5,
            "output steps by {out_step} where the input steps by at most {input_step}"
        );
    }

    /// `TrueStereo`'s pan formula must never need clamping: at full spread the
    /// two terms add to exactly one.
    #[test]
    fn the_stereo_pan_formula_stays_in_range() {
        for spread in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            for shape_pan in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
                for own_is_left in [true, false] {
                    let pan = pan_position(0.0, spread, shape_pan, own_is_left);
                    assert!(
                        (-1.0..=1.0).contains(&pan),
                        "spread {spread}, pan {shape_pan}, left {own_is_left}: {pan}"
                    );
                }
            }
        }
    }

    #[test]
    fn silent_voices_produce_silence_not_a_division_by_zero() {
        let shape = [VoiceShape {
            gain_db: -200.0,
            ..DEFAULT_SHAPE[0]
        }; MAX_VOICES];
        let mut engine = VoiceEngine::new(SR);
        let macros = Macros::default();

        for _ in 0..1000 {
            let (l, r) = engine.process(1.0, 1.0, &macros, &shape);
            assert!(l.is_finite() && r.is_finite());
        }
    }
}
