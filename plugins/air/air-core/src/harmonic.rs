//! The harmonic half of the generated layer.
//!
//! **No new DSP** (`REQ-AIR-003`). The curve is `nxe_audio::shaper`, the same
//! one Velour and Sparkleur drive; what makes this Air's is where the layer is
//! put and what is allowed to modulate it (`REQ-AIR-002`), and that lives in
//! `follow.rs` rather than here.
//!
//! ```text
//! input ─→ HPF(corner) ─→ lid ─→ 4x ─→ shaper(k, β, h) ─→ HPF(2·corner) ─→ × gain
//! ```
//!
//! ## Why the second high-pass sits an octave above the first
//!
//! A curve fed a band returns that band **plus** harmonics, so filtering the
//! result at the same corner adds a copy of the source back on top of itself.
//! Velour does exactly that on purpose — its job is to make a band feel present
//! (`velour_core::bands`) — and Air's is the opposite: put something where the
//! source is not (`REQ-AIR-001`). An octave up is where the second harmonic of
//! the lowest thing the curve sees lands, so what survives is mostly what was
//! not there before.
//!
//! ## Why this half is never widened
//!
//! It is made from the input, so the two channels are correlated, and widening
//! correlated material takes an all-pass — which is a comb in mono. The noise
//! half has no such problem and carries `WIDTH` alone (`REQ-AIR-008`). The two
//! channels here run through separate filter state and identical coefficients,
//! so a mono input comes out with `L == R` bit for bit.

use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
use nxe_audio::oversample::{Factor, Oversampler};
use nxe_audio::shaper::Shaper;

use crate::noise::corner_of;

/// The lid on what reaches the curve, as a fraction of the **host** rate.
///
/// A fraction rather than a frequency, because what decides the folding is the
/// ratio of the input frequency to the internal rate, and at a fixed factor
/// those move together. **Written against the host rate**: Velour wrote it
/// against the internal one by mistake and opened the lid to 48 kHz at 4x
/// (`VEL-3`).
pub const INPUT_CEILING: f32 = 0.25;

/// And an absolute lid, which is what makes the product rate-independent.
///
/// With only the fraction, the lid sits at 11 kHz at 44.1 kHz and 20 kHz at
/// 96 kHz, and the same tone comes out a decibel apart at the two (`SPK-9`).
/// The harmonics of anything above 12 kHz land past hearing anyway.
const INPUT_CEILING_HZ: f32 = 12_000.0;

/// How far above the input corner the generated layer starts.
pub const POST_RATIO: f32 = 2.0;

/// How hard the curve is driven, at the middle of the Advanced range.
///
/// **The ceiling on it is aliasing, not taste** — the same trade
/// `nxe_audio::shaper::DRIVE_MAX` documents. Whether it is *enough* is ear
/// (`dsp.md`).
pub const DRIVE: f32 = 4.0;

/// How far off centre the curve sits, at the middle of the Advanced range.
pub const BIAS: f32 = 0.30;

/// What `CHARACTER` moves the knee between (`REQ-AIR-005`).
pub const HARDNESS_SOFT: f32 = 0.10;
pub const HARDNESS_HARD: f32 = 0.90;

/// Everything about the harmonic layer that is not the signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `-1..=1`. Moves both corners, keeping their ratio.
    pub focus: f32,
    /// `0..=1`. Soft knee to hard knee.
    pub character: f32,
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
            drive: DRIVE,
            bias: BIAS,
            factor: Factor::default(),
        }
    }
}

/// One channel's filters and its own oversampler.
struct Channel {
    /// "Only what the layer is placed above feeds the curve."
    input: [Biquad; 2],
    /// The anti-aliasing lid.
    lid: [Biquad; 2],
    oversampler: Oversampler,
    /// "Only what was not already there is added."
    output: [Biquad; 2],
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            input: [Biquad::default(); 2],
            lid: [Biquad::default(); 2],
            oversampler: Oversampler::new(),
            output: [Biquad::default(); 2],
        }
    }
}

impl Channel {
    fn reset(&mut self) {
        for section in self
            .input
            .iter_mut()
            .chain(&mut self.lid)
            .chain(&mut self.output)
        {
            section.reset();
        }
        self.oversampler.reset();
    }
}

/// The generated harmonic layer.
pub struct Harmonic {
    sample_rate: f32,
    channels: [Channel; 2],
    /// **One curve for both channels.** `Shaper::shape` is a pure function of
    /// its settings, so a second copy would be a second set of the same
    /// numbers.
    shaper: Shaper,
    corner_hz: f32,
    settings: Settings,
}

impl Harmonic {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let mut harmonic = Self {
            sample_rate,
            channels: [Channel::default(), Channel::default()],
            shaper: Shaper::new(),
            corner_hz: f32::NAN,
            // Every field differs, so the first `set` builds all of it. A
            // sentinel that matches the default in one field leaves that
            // field's coefficients at zero (`AIR-1`).
            settings: Settings {
                focus: f32::NAN,
                character: f32::NAN,
                drive: f32::NAN,
                bias: f32::NAN,
                factor: Factor::Two,
            },
        };
        harmonic.tune_lid();
        harmonic.set(Settings::default());
        harmonic
    }

    /// **Block rate.** The curve's normalising integral and six filters per
    /// channel are resolved here, and only when something moved.
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            focus: finite(settings.focus, 0.0).clamp(-1.0, 1.0),
            character: finite(settings.character, 0.0).clamp(0.0, 1.0),
            drive: finite(settings.drive, DRIVE),
            bias: finite(settings.bias, BIAS),
            factor: settings.factor,
        };
        if settings == self.settings {
            return;
        }

        if settings.focus != self.settings.focus {
            self.corner_hz = corner_of(settings.focus, self.sample_rate);
            let input = Coefficients::highpass(self.corner_hz, BUTTERWORTH_Q, self.sample_rate);
            // The output corner is clamped too: at 192 kHz an octave above the
            // top of `FOCUS` is 17 kHz, but the constant it is built from is
            // free to move later.
            let output = Coefficients::highpass(
                (self.corner_hz * POST_RATIO).min(self.sample_rate * 0.45),
                BUTTERWORTH_Q,
                self.sample_rate,
            );
            for channel in &mut self.channels {
                for section in &mut channel.input {
                    section.set(input);
                }
                for section in &mut channel.output {
                    section.set(output);
                }
            }
        }

        self.shaper.set(
            settings.drive,
            settings.bias,
            HARDNESS_SOFT + settings.character * (HARDNESS_HARD - HARDNESS_SOFT),
        );
        for channel in &mut self.channels {
            channel.oversampler.set_factor(settings.factor);
        }
        self.settings = settings;
    }

    /// One frame in, the layer to add out. **Audio rate.**
    ///
    /// `gain` is an argument rather than a setting because it carries the
    /// smoothed macros — reading them once per block would step the layer on
    /// every automation ramp (`VEL-5`).
    pub fn process(&mut self, left: f32, right: f32, gain: f32) -> (f32, f32) {
        let gain = finite(gain, 0.0);
        // **Zero is exactly nothing**, not a multiplication by zero that still
        // costs an oversampled block (`REQ-AIR-001`).
        if gain == 0.0 {
            return (0.0, 0.0);
        }

        let shaper = &self.shaper;
        let through = |channel: &mut Channel, input: f32| {
            let input = finite(input, 0.0);
            let selected = channel
                .input
                .iter_mut()
                .fold(input, |x, section| section.process(x));
            let capped = channel
                .lid
                .iter_mut()
                .fold(selected, |x, section| section.process(x));
            let generated = channel
                .oversampler
                .process(capped, |sample| shaper.shape(sample));
            channel
                .output
                .iter_mut()
                .fold(generated, |x, section| section.process(x))
                * gain
        };

        let (first, second) = self.channels.split_at_mut(1);
        (through(&mut first[0], left), through(&mut second[0], right))
    }

    /// Where the layer starts, in Hz. The same number the noise half is placed
    /// by, which is what makes "the two move together" measurable
    /// (`REQ-AIR-006`).
    pub fn corner_hz(&self) -> f32 {
        self.corner_hz
    }

    /// The lid's corner, in Hz.
    pub fn lid_hz(&self) -> f32 {
        (self.sample_rate * INPUT_CEILING).min(INPUT_CEILING_HZ)
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
    }

    fn tune_lid(&mut self) {
        let coefficients = Coefficients::lowpass(self.lid_hz(), BUTTERWORTH_Q, self.sample_rate);
        for channel in &mut self.channels {
            for section in &mut channel.lid {
                section.set(coefficients);
            }
        }
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics::{amplitude, db_ratio, rms, sine};
    use nxe_audio::shaper::{BIAS_MAX, PROBE_AMPLITUDE};

    /// One second at 48 kHz, so a whole number of cycles is a whole number of
    /// hertz and a DFT bin index **is** a frequency.
    const RATE: f32 = 48_000.0;
    const LENGTH: usize = 48_000;
    const GAIN: f32 = 1.0;

    fn settings(character: f32) -> Settings {
        Settings {
            character,
            ..Settings::default()
        }
    }

    /// The layer for a steady tone, after one settling pass.
    fn layer(harmonic: &mut Harmonic, hz: usize, amplitude: f32) -> Vec<f32> {
        let input = sine(amplitude, hz, LENGTH);
        for sample in &input {
            harmonic.process(*sample, *sample, GAIN);
        }
        input
            .iter()
            .map(|sample| harmonic.process(*sample, *sample, GAIN).0)
            .collect()
    }

    /// Zero is **exactly** nothing (`REQ-AIR-001`).
    #[test]
    fn zero_gain_is_exactly_silent() {
        for character in [0.0f32, 0.5, 1.0] {
            let mut harmonic = Harmonic::new(RATE);
            harmonic.set(settings(character));
            for sample in sine(0.5, 5_000, 4_800) {
                assert_eq!(harmonic.process(sample, sample, 0.0), (0.0, 0.0));
            }
        }
    }

    /// **The harmonic half is never widened** (`REQ-AIR-008`), so a mono input
    /// leaves the two channels identical — bit for bit, not nearly.
    #[test]
    fn a_mono_input_stays_mono() {
        let mut harmonic = Harmonic::new(RATE);
        harmonic.set(settings(0.5));
        for sample in sine(0.4, 5_000, 9_600) {
            let (left, right) = harmonic.process(sample, sample, GAIN);
            assert_eq!(left, right);
        }
    }

    /// The layer starts where the noise half starts, so `FOCUS` moves both and
    /// keeps their ratio (`REQ-AIR-006`).
    #[test]
    fn both_halves_are_placed_by_one_number() {
        for focus in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
            let mut harmonic = Harmonic::new(RATE);
            harmonic.set(Settings {
                focus,
                ..Settings::default()
            });
            let mut noise = crate::Noise::new(RATE, 1);
            noise.set(crate::noise::Settings {
                focus,
                ..crate::noise::Settings::default()
            });
            assert_eq!(harmonic.corner_hz(), noise.corner_hz(), "FOCUS {focus}");
        }
    }

    /// **The lid is a fraction of the host rate** (`REQ-AIR-003`). Writing it
    /// against the internal rate is the mistake `VEL-3` made, and it opens the
    /// lid to 48 kHz at 4x.
    #[test]
    fn the_lid_is_a_fraction_of_the_host_rate() {
        for (rate, expected) in [
            (44_100.0f32, 11_025.0f32),
            (48_000.0, 12_000.0),
            // The absolute lid, which is what makes 48 and 96 the same plugin.
            (96_000.0, 12_000.0),
            (192_000.0, 12_000.0),
        ] {
            let harmonic = Harmonic::new(rate);
            assert!(
                (harmonic.lid_hz() - expected).abs() < 1.0,
                "{rate} Hz put the lid at {}",
                harmonic.lid_hz()
            );
        }

        // And the factor does not move it, which is the actual trap.
        let mut harmonic = Harmonic::new(RATE);
        let four = harmonic.lid_hz();
        harmonic.set(Settings {
            factor: Factor::Two,
            ..Settings::default()
        });
        assert_eq!(harmonic.lid_hz(), four, "the factor moved the lid");
    }

    /// The loudest fold that is not sitting on a real harmonic, in dB below the
    /// layer's own fundamental.
    fn worst_alias_db(hz: usize) -> f32 {
        let mut harmonic = Harmonic::new(RATE);
        // The worst curve the product can reach, and then some.
        harmonic.set(Settings {
            character: 1.0,
            bias: BIAS_MAX,
            ..Settings::default()
        });
        let output = layer(&mut harmonic, hz, PROBE_AMPLITUDE);

        let reference = amplitude(&output, hz);
        let mut worst = 0.0f32;
        for multiple in 2..200usize {
            let true_hz = multiple * hz;
            // Under Nyquist it is a real harmonic, not a fold.
            if true_hz < 24_000 {
                continue;
            }
            let folded = fold(true_hz);
            // A fold landing on a real harmonic cannot be told from one, and
            // the halfbands deliberately pass what lands above 20 kHz
            // (`nxe_audio::oversample`).
            if folded == 0 || folded >= 20_000 || folded.is_multiple_of(hz) {
                continue;
            }
            worst = worst.max(amplitude(&output, folded));
        }
        db_ratio(worst, reference)
    }

    fn fold(hz: usize) -> usize {
        let wrapped = hz % 48_000;
        if wrapped > 24_000 {
            48_000 - wrapped
        } else {
            wrapped
        }
    }

    /// The same bar Velour and Sparkleur are held to (`REQ-AIR-003`).
    #[test]
    fn the_worst_case_aliasing_stays_below_sixty_db() {
        for hz in [5_000usize, 7_000, 9_000, 11_000] {
            let worst = worst_alias_db(hz);
            assert!(worst < -60.0, "{hz} Hz folded back at {worst:.1} dB");
        }
    }

    /// **`CHARACTER` changes what the harmonics are, not how much is added**
    /// (`REQ-AIR-005`). The curve is normalised for RMS at a reference
    /// amplitude, and this is the property that rests on it.
    ///
    /// **The quantity is the third harmonic against the fundamental**, which is
    /// what a knee moves (`velour/docs/specifications/dsp.md`). The obvious
    /// alternative — third against second — reads as **1.7 %** across the whole
    /// axis, because the bias grows the second harmonic at the same time; a
    /// test written on it would say `CHARACTER` does nothing while the layer
    /// audibly changed.
    ///
    /// Measured at the shipped drive: **0.150 → 0.248** with the amount moving
    /// 0.55 dB.
    #[test]
    fn character_moves_the_harmonics_without_moving_the_amount() {
        let measure = |character: f32| {
            let mut harmonic = Harmonic::new(RATE);
            harmonic.set(settings(character));
            let output = layer(&mut harmonic, 5_000, PROBE_AMPLITUDE);
            let first = amplitude(&output, 5_000);
            let third = amplitude(&output, 15_000);
            (rms(&output), third / first)
        };

        let (soft_level, soft_ratio) = measure(0.0);
        let (hard_level, hard_ratio) = measure(1.0);

        assert!(
            hard_ratio > soft_ratio * 1.3,
            "soft {soft_ratio:.3} against hard {hard_ratio:.3} is not a \
             different balance"
        );
        let drift = db_ratio(hard_level, soft_level);
        assert!(
            drift.abs() < 3.0,
            "the axis moved the amount by {drift:.2} dB"
        );
    }

    /// **What is added is mostly what was not there.** The output corner sits
    /// an octave above the input one for exactly this (`dsp.md`): a layer that
    /// handed the source band back would be Velour's job, not Air's
    /// (`REQ-AIR-002`).
    ///
    /// Measured: a 3.5 kHz tone comes back **23.7 dB** below what it went in
    /// at, while its second harmonic — which is what the layer is for — is
    /// only 2.5 dB further down.
    #[test]
    fn the_source_band_is_not_handed_back() {
        let mut harmonic = Harmonic::new(RATE);
        harmonic.set(settings(0.5));
        let output = layer(&mut harmonic, 3_500, PROBE_AMPLITUDE);
        let returned = db_ratio(amplitude(&output, 3_500), PROBE_AMPLITUDE);
        assert!(returned < -18.0, "the source came back at {returned:.1} dB");
    }

    #[test]
    fn hostile_settings_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut harmonic = Harmonic::new(RATE);
            harmonic.set(Settings {
                focus: value,
                character: value,
                drive: value,
                bias: value,
                factor: Factor::Four,
            });
            for sample in sine(0.5, 5_000, 4_800) {
                let (left, right) = harmonic.process(sample, sample, value);
                assert!(
                    left.is_finite() && right.is_finite(),
                    "{value} produced {left}, {right}"
                );
            }
        }

        // And a hostile sample must not poison the recursive parts.
        let mut harmonic = Harmonic::new(RATE);
        harmonic.set(settings(0.5));
        for value in wild {
            harmonic.process(value, value, GAIN);
        }
        let recovered = layer(&mut harmonic, 5_000, PROBE_AMPLITUDE);
        assert!(
            rms(&recovered) > 1e-6,
            "the layer went silent and stayed so"
        );
    }

    #[test]
    fn reset_clears_it() {
        let mut harmonic = Harmonic::new(RATE);
        harmonic.set(settings(0.5));
        layer(&mut harmonic, 5_000, 0.5);
        harmonic.reset();
        for _ in 0..64 {
            assert_eq!(harmonic.process(0.0, 0.0, GAIN), (0.0, 0.0));
        }
    }
}
