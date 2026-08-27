//! The noise half of the generated layer, and the width model the product's
//! central promise rests on.
//!
//! **The only new DSP in v1** (`REQ-AIR-004`). Everything else Air needs
//! already exists in [`nxe_audio`], which is why this module is written as
//! something that does not know what Air is: the roadmap has two more callers
//! for a noise generator (Growl's `TEETH`, Vocal Glue's `AIR Cohesion`), and
//! the second one to ask moves this file into `nxe-audio` unchanged
//! (`REQ-AIR-015`).
//!
//! ```text
//! s, l, r ─→ mix(WIDTH) ─→ tilt ─→ HPF ─→ lid ─→ × grain ─→ × gain
//! ```
//!
//! ## Why three streams and no phase
//!
//! Widening something that is already correlated needs an all-pass, and an
//! all-pass is a comb filter waiting for a mono fold. Noise does not have that
//! problem: **generate the two channels from different draws and they are
//! decorrelated for free**, with the mono sum landing 3 dB down and no notch
//! anywhere (`REQ-AIR-008`). So the width control is a mixing ratio between one
//! shared stream and two independent ones, and the promise it makes —
//! "no comb in mono" — is structural rather than tuned.
//!
//! ## Why the sample rate is in the amplitude
//!
//! A fresh random number every sample carries the same **total** power at any
//! rate, and spreads it over `0..fs/2`. The audible band therefore gets less of
//! it the higher the rate runs — 3 dB less at 96 kHz — and because it is noise,
//! nobody would hear that as a bug (`REQ-AIR-017`). The amplitude carries
//! `√(fs / 48000)` to cancel it, and the lid above makes that correction
//! something a test can see.

use nxe_audio::Envelope;
use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};

/// Where the layer starts at `FOCUS` = 0.
///
/// **Ear** (`dsp.md`): this is where "the surface" begins, and what decides
/// whether the travel below is enough for a bass and the travel above enough
/// for a pad.
pub const BASE_CORNER_HZ: f32 = 3_000.0;

/// How far `FOCUS` moves the corner, in octaves either way.
///
/// Sparkleur's figure, for Sparkleur's reason: the material runs from a voice
/// to a bass (`REQ-AIR-006`).
pub const FOCUS_OCTAVES: f32 = 1.5;

/// The corner never goes below this, and never above [`CORNER_CEILING`] of the
/// rate. Neither bites at the shipped travel — they are here so that moving
/// [`BASE_CORNER_HZ`] later cannot walk the filters into Nyquist in silence.
const CORNER_MIN_HZ: f32 = 200.0;
const CORNER_CEILING: f32 = 0.30;

/// How far the tilt leans at the ends of `FOCUS`, in dB.
///
/// **A coefficient, not a slope you can read off a plot.** One pole transitions
/// over about two decades, so inside the audible band the tilt never reaches
/// its end value — which is fine, because what is wanted here is a continuous
/// lean rather than a named spectrum (`REQ-AIR-004`). **Ear.**
pub const TILT_MAX_DB: f32 = 12.0;

/// The lid on the layer: the top of hearing, or what the rate allows.
///
/// It is not decoration. Without it the rate correction above puts four times
/// the ultrasonic power into a 192 kHz session as into a 48 kHz one — inaudible
/// and wrong — and **with it the layer's plain RMS becomes the rate-independent
/// figure a test can assert on** (`dsp.md`).
const LID_HZ: f32 = 20_000.0;
const LID_CEILING: f32 = 0.45;

/// How fast the grain modulator wanders.
///
/// **Ear**: whether this reads as "grain" or as "roughness". The layer's power
/// does not move with it — the depth is normalised — so this is only texture.
const GRAIN_SECONDS: f32 = 0.006;

/// The modulator is clamped this many standard deviations out.
///
/// A bound on the peak, not on the power: clipping a Gaussian at 2.5σ takes
/// about 1% of the variance, which is 0.05 dB and below anything the tests
/// assert.
const GRAIN_CLAMP: f32 = 2.5;

/// Where the noise stops being kept alive, and over how many dB it fades in.
///
/// **Music does not go below −60 dBFS**, so inside a performance this is open
/// and still; what it exists for is the silence between them (`REQ-AIR-004`).
const KEEPALIVE_OFF_DB: f32 = -72.0;
const KEEPALIVE_RANGE_DB: f32 = 12.0;
const KEEPALIVE_ATTACK_SECONDS: f32 = 0.005;
const KEEPALIVE_RELEASE_SECONDS: f32 = 0.250;

/// The rate the amplitude is normalised against.
const REFERENCE_RATE: f32 = 48_000.0;

/// A uniform draw over `-1..1` has this RMS, and dividing it out makes one
/// stream unit RMS — so the layer's absolute level is decided by the caller's
/// gain and by nothing in here.
const UNIFORM_RMS: f32 = 0.577_350_26;

/// Everything about the noise layer that is not the signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `-1..=1`. Moves the corner, and with it the tilt's pivot.
    pub focus: f32,
    /// `0..=1`. Smooth to grainy.
    pub character: f32,
    /// `0..=1`. Shared stream to independent streams.
    pub width: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus: 0.0,
            character: 0.0,
            width: 0.5,
        }
    }
}

/// splitmix32: a counter and three mixing rounds.
///
/// **Counter-based rather than recursive**, so two streams seeded next to each
/// other do not walk into one another — which is what a plain linear
/// congruential generator does, and there are three streams here whose whole
/// job is to be independent.
#[derive(Clone, Copy)]
struct Stream {
    state: u32,
}

impl Stream {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// One draw in `-1..1`, with no DC.
    ///
    /// **24 bits over 2^23, then minus one.** Scaling the range by two first —
    /// which is the obvious thing to write — leaves `-1..3`, a signal whose DC
    /// term is as large as the noise itself. Sparkleur shipped a measurement
    /// built on that mistake and read a constant as pink noise (`SPK-18`).
    fn next(&mut self) -> f32 {
        self.state = self.state.wrapping_add(0x9E37_79B9);
        let mut z = self.state;
        z ^= z >> 16;
        z = z.wrapping_mul(0x21F0_AAAD);
        z ^= z >> 15;
        z = z.wrapping_mul(0x735A_2D97);
        z ^= z >> 15;
        (z >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

/// A first-order tilt: one pole, and its complement, with opposite gains.
///
/// The pivot is the pole, so a tilt is always "the passband leans up from where
/// it starts" rather than "everything above 2 kHz is louder" — which is what a
/// fixed pivot degenerates into as soon as `FOCUS` walks the corner past it.
#[derive(Clone, Copy, Default)]
struct Tilt {
    low: f32,
    coefficient: f32,
    low_gain: f32,
    high_gain: f32,
}

impl Tilt {
    fn process(&mut self, input: f32) -> f32 {
        self.low += (input - self.low) * self.coefficient;
        self.low_gain * self.low + self.high_gain * (input - self.low)
    }

    fn reset(&mut self) {
        self.low = 0.0;
    }
}

/// One channel's filtering: the tilt, the high-pass and the lid.
#[derive(Clone, Copy, Default)]
struct Channel {
    tilt: Tilt,
    highpass: [Biquad; 2],
    lid: [Biquad; 2],
}

impl Channel {
    fn process(&mut self, input: f32) -> f32 {
        let tilted = self.tilt.process(input);
        let cut = self
            .highpass
            .iter_mut()
            .fold(tilted, |x, section| section.process(x));
        self.lid
            .iter_mut()
            .fold(cut, |x, section| section.process(x))
    }

    fn reset(&mut self) {
        self.tilt.reset();
        for section in self.highpass.iter_mut().chain(&mut self.lid) {
            section.reset();
        }
    }
}

/// The generated noise layer.
pub struct Noise {
    sample_rate: f32,
    /// Shared, left-only, right-only.
    streams: [Stream; 3],
    grain_stream: Stream,
    /// `√3 · √(fs / 48000)`: one stream at unit RMS, corrected for the rate.
    scale: f32,
    channels: [Channel; 2],
    /// `cos` and `sin` of `WIDTH · π/2`, so the two square to one.
    shared: f32,
    independent: f32,
    grain: f32,
    grain_coefficient: f32,
    /// What turns the smoothed draw into a unit-variance one.
    grain_scale: f32,
    depth: f32,
    depth_normalisation: f32,
    envelope: Envelope,
    keepalive: f32,
    corner_hz: f32,
    settings: Settings,
}

impl Noise {
    /// **The seed comes from the caller** (`REQ-AIR-017`). `air-core` reads no
    /// clock and no transport, so a test gets an exactly reproducible stream
    /// and the wrapper gets to decide that two instances differ.
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            REFERENCE_RATE
        };
        let mut noise = Self {
            sample_rate,
            streams: [
                Stream::new(seed),
                Stream::new(seed ^ 0x6C07_8965),
                Stream::new(seed ^ 0x2545_F491),
            ],
            grain_stream: Stream::new(seed ^ 0x1B87_3593),
            scale: (sample_rate / REFERENCE_RATE).sqrt() / UNIFORM_RMS,
            channels: [Channel::default(); 2],
            shared: 0.0,
            independent: 0.0,
            grain: 0.0,
            grain_coefficient: coefficient(GRAIN_SECONDS, sample_rate),
            grain_scale: 0.0,
            depth: 0.0,
            depth_normalisation: 1.0,
            envelope: Envelope::new(
                KEEPALIVE_ATTACK_SECONDS,
                KEEPALIVE_RELEASE_SECONDS,
                sample_rate,
            ),
            keepalive: 0.0,
            corner_hz: f32::NAN,
            // **Every field has to differ**, not just one: `set` skips the
            // branches whose value has not moved, so a sentinel that matches
            // the default in one field leaves that field's coefficients at
            // zero — and a `WIDTH` of zero and zero is silence.
            settings: Settings {
                focus: f32::NAN,
                character: f32::NAN,
                width: f32::NAN,
            },
        };
        // The variance of a one-pole fed unit-variance white noise is
        // `a / (2 - a)`, so this is what makes the modulator unit-variance
        // without measuring anything at run time.
        let a = noise.grain_coefficient;
        noise.grain_scale = ((2.0 - a) / a).sqrt();
        noise.tune_lid();
        noise.set(Settings::default());
        noise
    }

    /// **Block rate.** Everything expensive — five filters per channel — is
    /// resolved here, and only when something actually moved.
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            focus: finite(settings.focus, 0.0).clamp(-1.0, 1.0),
            character: finite(settings.character, 0.0).clamp(0.0, 1.0),
            width: finite(settings.width, 0.0).clamp(0.0, 1.0),
        };
        if settings == self.settings {
            return;
        }

        if settings.focus != self.settings.focus {
            self.corner_hz = corner_of(settings.focus, self.sample_rate);
            let highpass = Coefficients::highpass(self.corner_hz, BUTTERWORTH_Q, self.sample_rate);
            let lean = settings.focus * TILT_MAX_DB;
            for channel in &mut self.channels {
                for section in &mut channel.highpass {
                    section.set(highpass);
                }
                channel.tilt.coefficient = coefficient_hz(self.corner_hz, self.sample_rate);
                channel.tilt.low_gain = amplitude_of(-lean);
                channel.tilt.high_gain = amplitude_of(lean);
            }
        }

        if settings.character != self.settings.character {
            self.depth = settings.character;
            // `E[(1 + d·v)²] = 1 + d²` for a unit-variance zero-mean `v`, so
            // the depth can be swept without the layer's level moving.
            self.depth_normalisation = 1.0 / (1.0 + self.depth * self.depth).sqrt();
        }

        if settings.width != self.settings.width {
            let angle = settings.width * std::f32::consts::FRAC_PI_2;
            // `cos² + sin² = 1`: the per-channel power does not move with
            // `WIDTH`, only how much of it the two channels have in common.
            self.shared = angle.cos();
            self.independent = angle.sin();
            // The ends have to be *exactly* one and zero. `cos(π/2)` is
            // `-4.4e-8` in f32, and "the layer is mono at `WIDTH` = 0" is an
            // assertion about equality.
            if settings.width == 0.0 {
                self.shared = 1.0;
                self.independent = 0.0;
            } else if settings.width == 1.0 {
                self.shared = 0.0;
                self.independent = 1.0;
            }
        }

        self.settings = settings;
    }

    /// One frame: the mono sum of the input, and how much layer to make.
    ///
    /// **Audio rate.** `gain` is an argument rather than a setting because it
    /// carries the smoothed macros — reading them once per block would step the
    /// layer on every automation ramp (`VEL-5`).
    pub fn process(&mut self, mono: f32, gain: f32) -> (f32, f32) {
        self.envelope.push(mono);
        self.keepalive =
            ((self.envelope.decibels() - KEEPALIVE_OFF_DB) / KEEPALIVE_RANGE_DB).clamp(0.0, 1.0);

        let shared = self.streams[0].next();
        let left = self.streams[1].next();
        let right = self.streams[2].next();

        let mix =
            |independent: f32| (self.shared * shared + self.independent * independent) * self.scale;
        let left = self.channels[0].process(mix(left));
        let right = self.channels[1].process(mix(right));

        // **One modulator for both channels.** Two would decorrelate them, and
        // `WIDTH` = 0 promises a correlation of exactly one.
        let draw = self.grain_stream.next() / UNIFORM_RMS;
        self.grain += (draw - self.grain) * self.grain_coefficient;
        let wander = (self.grain * self.grain_scale).clamp(-GRAIN_CLAMP, GRAIN_CLAMP);
        let modulator = (1.0 + self.depth * wander) * self.depth_normalisation;

        let gain = finite(gain, 0.0) * self.keepalive * modulator;
        (left * gain, right * gain)
    }

    /// How far the keep-alive gate stands open, `0..=1` — for the display, and
    /// for the test that says silence is silent.
    pub fn keepalive(&self) -> f32 {
        self.keepalive
    }

    /// Where the layer starts, in Hz. Follows `FOCUS`.
    pub fn corner_hz(&self) -> f32 {
        self.corner_hz
    }

    /// The lid's corner, in Hz.
    pub fn lid_hz(&self) -> f32 {
        LID_HZ.min(self.sample_rate * LID_CEILING)
    }

    pub fn reset(&mut self) {
        for channel in &mut self.channels {
            channel.reset();
        }
        self.envelope.reset();
        self.keepalive = 0.0;
        self.grain = 0.0;
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

/// Where the layer starts, for a `FOCUS` position.
///
/// Shared with the harmonic half (`AIR-2`): both families are placed by this
/// one number, which is what makes "the two move together and keep their ratio"
/// something a test can read (`REQ-AIR-006`).
pub fn corner_of(focus: f32, sample_rate: f32) -> f32 {
    let shift = (finite(focus, 0.0).clamp(-1.0, 1.0) * FOCUS_OCTAVES).exp2();
    (BASE_CORNER_HZ * shift).clamp(CORNER_MIN_HZ, sample_rate * CORNER_CEILING)
}

/// A one-pole's coefficient for a time constant in seconds.
fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

/// A one-pole's coefficient for a corner in Hz.
fn coefficient_hz(hz: f32, sample_rate: f32) -> f32 {
    1.0 - (-std::f32::consts::TAU * hz / sample_rate).exp()
}

fn amplitude_of(decibels: f32) -> f32 {
    10.0f32.powf(decibels / 20.0)
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::biquad::BandPass;
    use nxe_audio::harmonics::{db_ratio, rms, tone};
    use nxe_dsp::Correlation;

    const RATE: f32 = 48_000.0;
    const RATES: [f32; 4] = [44_100.0, 48_000.0, 96_000.0, 192_000.0];
    const SEED: u32 = 0x4149_5231;
    /// Long enough that a third-octave band holds thousands of independent
    /// samples, so a power ratio is a measurement rather than a coin toss.
    const SECONDS: f32 = 2.0;
    /// The keep-alive gate has a 250 ms release, and the tilt and the lid have
    /// to fill up too.
    const SETTLE_SECONDS: f32 = 0.5;
    /// Any level well above the keep-alive threshold; nothing here depends on
    /// what it is.
    const INPUT: f32 = 0.2;
    const GAIN: f32 = 1.0;

    fn settings(width: f32) -> Settings {
        Settings {
            width,
            ..Settings::default()
        }
    }

    /// The layer for one setting, after the gate and the filters have settled.
    fn layer(settings: Settings, rate: f32, seconds: f32) -> (Vec<f32>, Vec<f32>) {
        let mut noise = Noise::new(rate, SEED);
        noise.set(settings);
        let drive = tone(
            INPUT,
            220.0,
            rate,
            (rate * (SETTLE_SECONDS + seconds)) as usize,
        );
        let settle = (rate * SETTLE_SECONDS) as usize;

        let mut left = Vec::with_capacity(drive.len() - settle);
        let mut right = Vec::with_capacity(drive.len() - settle);
        for (index, sample) in drive.iter().enumerate() {
            let (l, r) = noise.process(*sample, GAIN);
            if index >= settle {
                left.push(l);
                right.push(r);
            }
        }
        (left, right)
    }

    fn at(width: f32) -> (Vec<f32>, Vec<f32>) {
        layer(settings(width), RATE, SECONDS)
    }

    /// **The mono sum is `(L+R)/2`.** With `L+R` the fold would read +6 dB at
    /// `WIDTH` = 0, and the "−3 dB at full width" the requirement states would
    /// be measured against a different reference (`dsp.md`).
    fn mono(left: &[f32], right: &[f32]) -> Vec<f32> {
        left.iter().zip(right).map(|(l, r)| (l + r) * 0.5).collect()
    }

    /// The power in one band, with the filter's own fill-up discarded.
    fn band_power(signal: &[f32], low_hz: f32, high_hz: f32, rate: f32) -> f32 {
        let mut band = BandPass::new(low_hz, high_hz, rate);
        let skip = (rate * 0.05) as usize;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for (index, sample) in signal.iter().enumerate() {
            let filtered = band.process(*sample);
            if index >= skip {
                sum += (filtered as f64) * (filtered as f64);
                count += 1;
            }
        }
        (sum / count.max(1) as f64) as f32
    }

    /// Third-octave centres from below the corner to the top of the layer.
    const CENTRES: [f32; 10] = [
        2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0, 6_300.0, 8_000.0, 10_000.0, 12_500.0, 16_000.0,
    ];

    /// How much the fold-to-channel ratio varies across the spectrum, in dB.
    ///
    /// **This is the comb detector.** A notch anywhere shows up as one band
    /// sitting far from the others; a plain level change moves them all
    /// together and reads as nothing.
    fn fold_ratio_spread_db(left: &[f32], right: &[f32], rate: f32) -> f32 {
        let summed = mono(left, right);
        let edge = 2.0f32.powf(1.0 / 6.0);
        let ratios: Vec<f32> = CENTRES
            .iter()
            .map(|centre| {
                let (low, high) = (centre / edge, centre * edge);
                let channel = band_power(left, low, high, rate);
                let folded = band_power(&summed, low, high, rate);
                10.0 * (folded.max(1e-30) / channel.max(1e-30)).log10()
            })
            .collect();
        let highest = ratios.iter().cloned().fold(f32::MIN, f32::max);
        let lowest = ratios.iter().cloned().fold(f32::MAX, f32::min);
        highest - lowest
    }

    fn correlation_of(left: &[f32], right: &[f32], rate: f32) -> f32 {
        let mut correlation = Correlation::new(rate);
        for (l, r) in left.iter().zip(right) {
            correlation.push(*l, *r);
        }
        correlation.value()
    }

    /// **The gate** (`REQ-AIR-008`): no comb in the fold, at any width.
    ///
    /// Nothing in the path rotates phase, so this cannot break by accident —
    /// which is exactly why it is worth pinning. If it ever fails, something
    /// with a delay or an all-pass in it has been added.
    #[test]
    fn the_mono_fold_has_no_comb_at_any_width() {
        for width in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let (left, right) = at(width);
            let spread = fold_ratio_spread_db(&left, &right, RATE);
            assert!(
                spread < 1.0,
                "WIDTH {width} put {spread:.2} dB between the fold's bands"
            );
        }
    }

    /// **The control for the test above** (`VEL-10`): the same measurement has
    /// to be able to fail. A few samples of delay on one channel is the comb
    /// the design is avoiding, and it must not read as flat.
    ///
    /// **It has to be the correlated pair that is disturbed.** Delaying one of
    /// two independent streams produces no comb at all — the sum of two
    /// unrelated noises is flat however they are lined up — so a control built
    /// on `WIDTH` = 1 would pass while measuring nothing.
    ///
    /// **And it has to be an all-pass rather than a delay.** A twelve-sample
    /// delay combs every 4 kHz, which is finer than a third-octave band, and
    /// the band's own skirts let the neighbouring peak in: measured, that comb
    /// reads as **0.9 dB** of spread and slips under the gate. An all-pass —
    /// the technique `REQ-AIR-008` refuses, and the reason the harmonic half is
    /// not widened — puts one broad null in the fold instead, and reads as
    /// **6.6 dB**.
    #[test]
    fn the_comb_detector_can_fail() {
        let (left, right) = at(0.0);
        let mut low = [Biquad::new(Coefficients::lowpass(6_000.0, BUTTERWORTH_Q, RATE)); 2];
        let mut high = [Biquad::new(Coefficients::highpass(6_000.0, BUTTERWORTH_Q, RATE)); 2];
        let rotated: Vec<f32> = right
            .iter()
            .map(|sample| {
                let below = low.iter_mut().fold(*sample, |x, s| s.process(x));
                let above = high.iter_mut().fold(*sample, |x, s| s.process(x));
                below + above
            })
            .collect();
        let spread = fold_ratio_spread_db(&left, &rotated, RATE);
        assert!(
            spread > 3.0,
            "an all-pass on one channel only moved the bands {spread:.2} dB \
             apart, so the detector cannot see a comb"
        );
    }

    /// `WIDTH` = 0 is one stream in both channels — not nearly, exactly.
    #[test]
    fn width_at_zero_is_one_stream() {
        let (left, right) = at(0.0);
        for (l, r) in left.iter().zip(&right) {
            assert_eq!(l, r, "the channels differ at WIDTH 0");
        }
        let correlation = correlation_of(&left, &right, RATE);
        assert!((correlation - 1.0).abs() < 1e-3, "{correlation:.4}");
    }

    /// `WIDTH` = 1 is two draws: uncorrelated, and 3 dB down in the fold.
    #[test]
    fn width_at_one_folds_three_decibels_down() {
        let (left, right) = at(1.0);
        let correlation = correlation_of(&left, &right, RATE);
        assert!(correlation.abs() < 0.05, "{correlation:.4}");

        let fold = db_ratio(rms(&mono(&left, &right)), rms(&left));
        assert!(
            (fold - -3.01).abs() < 0.5,
            "the fold landed at {fold:.2} dB"
        );
    }

    /// And the way between them is monotonic (`REQ-AIR-008`).
    #[test]
    fn the_correlation_falls_monotonically() {
        let readings: Vec<f32> = [0.0f32, 0.25, 0.5, 0.75, 1.0]
            .iter()
            .map(|width| {
                let (left, right) = at(*width);
                correlation_of(&left, &right, RATE)
            })
            .collect();

        // The measurement has to be able to tell the ends apart at all.
        let travel = readings[0] - readings[readings.len() - 1];
        assert!(travel > 0.9, "the ends only differ by {travel:.3}");

        for pair in readings.windows(2) {
            assert!(
                pair[1] < pair[0],
                "the correlation went back up: {readings:?}"
            );
        }
    }

    /// **Per-channel power does not move with `WIDTH`** — `cos² + sin² = 1`.
    /// Without this the width control would read as a level control.
    #[test]
    fn width_does_not_move_the_level() {
        let levels: Vec<f32> = [0.0f32, 0.5, 1.0]
            .iter()
            .map(|width| rms(&at(*width).0))
            .collect();
        for level in &levels {
            let drift = db_ratio(*level, levels[0]);
            assert!(drift.abs() < 0.3, "{levels:?} drifted {drift:.2} dB");
        }
    }

    /// The bands the rate promise is made over.
    ///
    /// **Not the top octave** — see the test below for why it cannot be in
    /// here. These four hold the layer's energy and the ear's sensitivity.
    const CORE: [f32; 4] = [4_000.0, 6_300.0, 8_000.0, 10_000.0];

    fn level_at(centre: f32, rate: f32) -> f32 {
        let edge = 2.0f32.powf(1.0 / 6.0);
        let (left, _) = layer(settings(0.5), rate, 1.0);
        10.0 * band_power(&left, centre / edge, centre * edge, rate)
            .max(1e-30)
            .log10()
    }

    /// **The trap this module exists to avoid** (`REQ-AIR-017`): the same
    /// settings have to put the same amount of noise in the audible band at
    /// every rate. Without the `√(fs / 48000)` correction, 96 kHz is 3 dB down
    /// and it sounds like nothing at all.
    #[test]
    fn the_audible_level_does_not_move_with_the_rate() {
        for centre in CORE {
            let readings: Vec<f32> = RATES.iter().map(|rate| level_at(centre, *rate)).collect();
            let spread = readings.iter().cloned().fold(f32::MIN, f32::max)
                - readings.iter().cloned().fold(f32::MAX, f32::min);
            assert!(
                spread < 0.5,
                "{centre} Hz spread {spread:.2} dB {readings:?}"
            );
        }
    }

    /// **The top octave is rate-dependent, and cannot be made otherwise.**
    ///
    /// At 48 kHz a 20 kHz low-pass sits at 0.83 of Nyquist, where the bilinear
    /// transform squeezes its whole transition; at 192 kHz the same corner has
    /// room to be a filter. So "the same lid" takes 0.2 dB off 16 kHz at
    /// 48 kHz and 1.4 dB at 192 kHz — and nothing about the rate correction can
    /// see a difference in *shape*.
    ///
    /// **A scalar makeup was tried and made it worse** (`AIR-1`): normalising
    /// each rate's band power against 48 kHz brought the layer's total RMS from
    /// 0.72 dB of spread down to 0.30, and pushed the core bands from 0.39 dB
    /// out to **1.26** — a flat error traded for a tilt. Matching where the
    /// energy is beats matching the total.
    ///
    /// This test records what it is rather than fixing it. If it grows,
    /// something in the chain moved.
    #[test]
    fn the_top_octave_is_brighter_at_the_lower_rates() {
        let readings: Vec<f32> = RATES.iter().map(|rate| level_at(16_000.0, *rate)).collect();
        let spread = readings.iter().cloned().fold(f32::MIN, f32::max)
            - readings.iter().cloned().fold(f32::MAX, f32::min);
        // Measured 3.83 dB.
        assert!(spread < 4.5, "16 kHz spread {spread:.2} dB {readings:?}");
        assert!(
            readings[0] > readings[3],
            "the lower rates stopped being the brighter ones: {readings:?}"
        );
    }

    /// The control for the test above: the correction has to be doing work.
    /// Comparing the *whole* band at two rates without a correction is a 3 dB
    /// difference, so the measurement is not blind.
    #[test]
    fn the_rate_correction_can_be_seen() {
        let uncorrected = |rate: f32| {
            let mut stream = Stream::new(SEED);
            let samples: Vec<f32> = (0..(rate as usize)).map(|_| stream.next()).collect();
            let edge = 2.0f32.powf(1.0 / 6.0);
            10.0 * band_power(&samples, 8_000.0 / edge, 8_000.0 * edge, rate)
                .max(1e-30)
                .log10()
        };
        let difference = uncorrected(96_000.0) - uncorrected(48_000.0);
        assert!(
            difference < -2.0,
            "raw noise only lost {difference:.2} dB going to 96 kHz, so the \
             correction is measuring nothing"
        );
    }

    /// The sample rate is the only thing the noise reads besides its seed.
    #[test]
    fn the_block_size_does_not_change_it() {
        let one_pass = at(0.5).0;
        // The same run, driven a few samples at a time with `set` called
        // between blocks the way a host would.
        let mut noise = Noise::new(RATE, SEED);
        let drive = tone(
            INPUT,
            220.0,
            RATE,
            (RATE * (SETTLE_SECONDS + SECONDS)) as usize,
        );
        let settle = (RATE * SETTLE_SECONDS) as usize;
        let mut blocked = Vec::with_capacity(one_pass.len());
        for (index, chunk) in drive.chunks(37).enumerate() {
            noise.set(settings(0.5));
            for (offset, sample) in chunk.iter().enumerate() {
                let (l, _) = noise.process(*sample, GAIN);
                if index * 37 + offset >= settle {
                    blocked.push(l);
                }
            }
        }
        assert_eq!(one_pass, blocked);
    }

    /// `air-core` takes its seed from outside, so a test is exactly repeatable
    /// even though the plugin's stream is not (`REQ-AIR-017`).
    #[test]
    fn the_same_seed_is_the_same_noise() {
        assert_eq!(at(0.5).0, at(0.5).0);

        let other = {
            let mut noise = Noise::new(RATE, SEED ^ 0xFFFF);
            noise.set(settings(0.5));
            let drive = tone(INPUT, 220.0, RATE, 4_800);
            drive
                .iter()
                .map(|sample| noise.process(*sample, GAIN).0)
                .collect::<Vec<_>>()
        };
        let same = {
            let mut noise = Noise::new(RATE, SEED);
            noise.set(settings(0.5));
            let drive = tone(INPUT, 220.0, RATE, 4_800);
            drive
                .iter()
                .map(|sample| noise.process(*sample, GAIN).0)
                .collect::<Vec<_>>()
        };
        assert_ne!(other, same, "two seeds produced the same stream");
    }

    /// Zero is **exactly** nothing (`REQ-AIR-004`).
    #[test]
    fn zero_gain_is_exactly_silent() {
        let mut noise = Noise::new(RATE, SEED);
        noise.set(settings(1.0));
        for sample in tone(INPUT, 220.0, RATE, 4_800) {
            assert_eq!(noise.process(sample, 0.0), (0.0, 0.0));
        }
    }

    /// **The keep-alive gate** (`REQ-AIR-004`): noise made from nothing would
    /// otherwise be a permanent hiss between the takes. It has to reach zero,
    /// not a denormal a picture draws as an ajar gate (`SPK-16`).
    #[test]
    fn silence_shuts_the_layer_completely() {
        let mut noise = Noise::new(RATE, SEED);
        noise.set(settings(1.0));
        for sample in tone(INPUT, 220.0, RATE, (RATE * 0.5) as usize) {
            noise.process(sample, GAIN);
        }
        assert!(noise.keepalive() > 0.99, "music did not open the gate");

        // Two seconds of nothing, against a 250 ms release.
        for _ in 0..(RATE * 2.0) as usize {
            noise.process(0.0, GAIN);
        }
        assert_eq!(noise.keepalive(), 0.0, "the gate stayed ajar");
        for _ in 0..64 {
            assert_eq!(noise.process(0.0, GAIN), (0.0, 0.0));
        }
    }

    /// The power above the lid, against the layer's own.
    ///
    /// **Ten cascaded sections, and it has to be that many.** A single
    /// second-order high-pass at 24 kHz still passes 16 kHz at −7 dB, and the
    /// band below the lid holds thirty times the energy of the band above it —
    /// so a shallow measurement reads its own skirt. Measured at `FOCUS` = 0:
    /// **−18.3 dB** with one section, **−34.3 dB** with ten. Only the second
    /// number is about the layer.
    fn above_lid_db(signal: &[f32], rate: f32) -> f32 {
        let mut sections = [Biquad::new(Coefficients::highpass(24_000.0, BUTTERWORTH_Q, rate)); 10];
        let skip = (rate * 0.05) as usize;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for (index, sample) in signal.iter().enumerate() {
            let filtered = sections.iter_mut().fold(*sample, |x, s| s.process(x));
            if index >= skip {
                sum += (filtered as f64) * (filtered as f64);
                count += 1;
            }
        }
        let outside = (sum / count.max(1) as f64) as f32;
        10.0 * (outside / rms(signal).powi(2).max(1e-30))
            .max(1e-30)
            .log10()
    }

    /// The tilt leans the passband; it does not push energy past the lid
    /// (`REQ-AIR-004`).
    ///
    /// Measured: **−42.2 / −34.3 / −30.2 dB** at `FOCUS` −1 / 0 / +1. The lean
    /// costs 12 dB across the range, which is [`TILT_MAX_DB`] doing exactly
    /// what it says, and the lid still holds the result down.
    #[test]
    fn the_tilt_does_not_leak_past_the_lid() {
        // 96 kHz, so there is somewhere above 20 kHz for it to leak to.
        let rate = 96_000.0;
        for focus in [-1.0f32, 0.0, 1.0] {
            let (left, _) = layer(
                Settings {
                    focus,
                    ..settings(0.5)
                },
                rate,
                1.0,
            );
            let escape = above_lid_db(&left, rate);
            assert!(
                escape < -25.0,
                "FOCUS {focus} put {escape:.1} dB above the lid"
            );
        }
    }

    /// `CHARACTER` changes the texture, not the amount (`REQ-AIR-005`). The
    /// depth normalisation is what makes this hold in closed form.
    #[test]
    fn grain_does_not_move_the_level() {
        let levels: Vec<f32> = [0.0f32, 0.5, 1.0]
            .iter()
            .map(|character| {
                rms(&layer(
                    Settings {
                        character: *character,
                        ..settings(0.5)
                    },
                    RATE,
                    SECONDS,
                )
                .0)
            })
            .collect();
        for level in &levels {
            let drift = db_ratio(*level, levels[0]);
            assert!(drift.abs() < 0.5, "{levels:?} drifted {drift:.2} dB");
        }
    }

    /// …and it is doing something, which the level test cannot show.
    #[test]
    fn grain_modulates_the_layer() {
        let envelope_swing = |character: f32| {
            let (left, _) = layer(
                Settings {
                    character,
                    ..settings(0.5)
                },
                RATE,
                SECONDS,
            );
            // The layer's own level over 10 ms windows: a grainy layer's
            // windows differ from each other far more than a smooth one's.
            let windows: Vec<f32> = left.chunks(480).map(rms).collect();
            let mean = windows.iter().sum::<f32>() / windows.len() as f32;
            let variance =
                windows.iter().map(|w| (w - mean).powi(2)).sum::<f32>() / windows.len() as f32;
            variance.sqrt() / mean
        };
        let smooth = envelope_swing(0.0);
        let grainy = envelope_swing(1.0);
        assert!(
            grainy > smooth * 3.0,
            "grain {grainy:.4} against smooth {smooth:.4} is not a texture"
        );
    }

    /// `FOCUS` moves the corner both families are placed by (`REQ-AIR-006`),
    /// and never past what the rate allows.
    #[test]
    fn focus_moves_the_corner_and_stops_at_the_rate() {
        let low = corner_of(-1.0, RATE);
        let middle = corner_of(0.0, RATE);
        let high = corner_of(1.0, RATE);
        assert!((middle - BASE_CORNER_HZ).abs() < 1.0, "{middle}");
        assert!((high / middle - 2.828).abs() < 0.01, "{high} / {middle}");
        assert!((middle / low - 2.828).abs() < 0.01, "{middle} / {low}");

        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let corner = corner_of(1.0, rate);
            assert!(corner < rate * 0.5, "{rate} put the corner at {corner}");
        }
    }

    #[test]
    fn hostile_settings_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut noise = Noise::new(RATE, SEED);
            noise.set(Settings {
                focus: value,
                character: value,
                width: value,
            });
            for sample in tone(INPUT, 220.0, RATE, 4_800) {
                let (l, r) = noise.process(sample, value);
                assert!(l.is_finite() && r.is_finite(), "{value} produced {l}, {r}");
            }
        }

        // And a hostile sample must not poison the recursive parts.
        let mut noise = Noise::new(RATE, SEED);
        noise.set(settings(0.5));
        for value in wild {
            noise.process(value, GAIN);
        }
        let recovered: Vec<f32> = tone(INPUT, 220.0, RATE, (RATE * 0.5) as usize)
            .iter()
            .map(|sample| noise.process(*sample, GAIN).0)
            .collect();
        assert!(
            rms(&recovered) > 1e-6,
            "the layer went silent and stayed so"
        );

        // A rate that is not a rate must not build filters out of infinities.
        for rate in [0.0f32, -1.0, f32::NAN] {
            let mut noise = Noise::new(rate, SEED);
            noise.set(settings(0.5));
            assert!(noise.process(0.5, GAIN).0.is_finite(), "rate {rate}");
        }
    }

    #[test]
    fn reset_clears_it() {
        let mut noise = Noise::new(RATE, SEED);
        noise.set(settings(0.5));
        for sample in tone(INPUT, 220.0, RATE, 4_800) {
            noise.process(sample, GAIN);
        }
        noise.reset();
        assert_eq!(noise.keepalive(), 0.0);
        assert_eq!(noise.process(0.0, GAIN), (0.0, 0.0));
    }
}
