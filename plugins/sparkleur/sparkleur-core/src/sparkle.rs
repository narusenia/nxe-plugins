//! The layer added on top of the top band, and the transient that opens it.
//!
//! **Sparkle is the half of the product that is not compression**
//! (`REQ-SPK-007`). A static shelf cannot make the sheen this makes, and a
//! static *generator* — Velour's whole design — makes it all the time, which on
//! a full mix is a permanent hiss. So the layer is generated all the time and
//! **let through on attacks**.
//!
//! ```text
//! band 5 ─→ cap ─→ 4x ─→ shaper(β, h) ─→ HPF ─→ × gain ─→ added
//!    │                                            ↑
//!    └─ fast follower ─┐                          │
//!    └─ slow follower ─┴─ ratio → opening ────────┘
//! ```
//!
//! ## Why a ratio of two followers
//!
//! Spectral flux would put an FFT on the audio path (48 bands cost 22 µs in
//! Velour) and leave the threshold to the ear. A fast follower over a slow one,
//! **on a signal that is already only the top band**, is a transient detector
//! made of parts that exist (`REQ-SPK-007`). It reads a ratio, so it does not
//! depend on input gain.
//!
//! ## What moves is the output gain, never the curve
//!
//! The same judgement `nxe_audio::guard` is built on, upside down: a curve whose
//! drive is being modulated changes *character* as it moves, and then nobody
//! can hear what happened. The guard pulls a layer down when it hurts; Sparkle
//! lets one up when the music starts.
//!
//! ## The input needs a lid
//!
//! Velour's AIR band aliased at −44 dB **at every drive setting** until one was
//! added (`VEL-3`), and the lid was then written against the internal rate by
//! mistake, which opened it to 48 kHz at 4x. It is a fraction of the **host**
//! rate here for that reason (`INPUT_CEILING`).

use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
use nxe_audio::envelope::{Power, coefficient};
use nxe_audio::oversample::{Factor, Oversampler};
use nxe_audio::shaper::Shaper;

/// How much fast-over-slow counts as fully open, in dB.
///
/// **Ear-tuned** (`SPK-18`): consonants and hats should open it, a pad should
/// not.
pub const SNAP_RANGE_DB: f32 = 6.0;

/// How hard the curve is driven.
///
/// **`AIR` is the amount and it moves the output gain, not this**
/// (`REQ-SPK-007`), so the curve's drive is one number rather than a control.
/// The specification does not name it; this is where it lives until `SPK-18`
/// settles it by ear. The ceiling on it is aliasing, the same as Velour's
/// `DRIVE_MAX`.
pub const DRIVE: f32 = 4.0;

/// Inside one consonant.
const FAST_SECONDS: f32 = 0.001;
/// What the fast one is compared against — long enough to be "the level of this
/// passage" rather than "the level of this note".
const SLOW_SECONDS: f32 = 0.100;

/// How long the gate stays open after it has been opened.
///
/// **The release belongs to the gate, not to the follower** (`SPK-6`). The
/// specification put a 40 ms release on the fast follower so that two
/// consonants in a row would not make the gate chatter. That works, and it also
/// makes the fast follower sit about 3 dB above the slow one **on any steady
/// tone** — an asymmetric follower settles between the mean and the peak, and
/// how far between is set by its attack-to-release ratio (`SPK-3`). A ratio
/// detector fed two followers with different biases reads a sustain as a
/// half-open gate, which is the one thing `REQ-SPK-007` says it must not do.
///
/// So both followers are symmetric — their ratio is 1 on anything steady,
/// whatever it is — and the hold that stops the chatter sits after the ratio.
const HOLD_SECONDS: f32 = 0.040;

/// The lid on what reaches the curve, as a fraction of the **host** rate.
///
/// A fraction rather than a frequency, because what decides the aliasing is the
/// ratio of the input frequency to the internal rate, and at a fixed factor
/// those move together (`velour_core::bands::AIR_INPUT_CEILING`).
pub const INPUT_CEILING: f32 = 0.25;
/// And an absolute lid, which is what makes the product rate-independent.
///
/// **12 kHz, not the top of hearing** (`SPK-9`). With only the fraction, the
/// lid sits at 11 kHz at 44.1 kHz and 20 kHz at 96 kHz, and a 7 kHz tone comes
/// out **1.0 dB** apart at the two — `REQ-SPK-017` allows 0.5. With the
/// absolute lid the fraction only bites below 48 kHz, where there genuinely is
/// less room, and 48 against 96 becomes exact.
///
/// The cost is that content between 12 and 20 kHz is not excited at a high
/// rate even though there would be room. It is the same trade Velour made
/// (`velour_core::bands`), and the harmonics of anything above 12 kHz land
/// past hearing anyway.
const INPUT_CEILING_HZ: f32 = 12_000.0;

const ENERGY_FLOOR: f32 = 1e-10;

/// Below this the gate counts as shut.
///
/// A one-pole never reaches its target, so without a floor the gate creeps down
/// through numbers no picture can draw and never arrives at nothing.
///
/// **Not about denormals.** nih-plug turns flush-to-zero on around `process`
/// on x86 and aarch64 alike, so inside a host the tail would be flushed anyway
/// (`SPK-17`). This is so that "shut" is a value that means something — and so
/// that it is the same value outside a host, where the tests run.
const CLOSED: f32 = 1e-6;
/// `10·log10(x)` is `10/log2(10)` times `log2(x)`.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// Everything the layer needs that is not the signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How much of that is handed to the transient. `0` is static — Velour's
    /// behaviour, and the way to hear what the gate is doing by turning it off
    /// (`REQ-SPK-007`).
    pub snap: f32,
    /// `(β, h)` — what `CHARACTER` decides (`SPK-5`).
    pub bias: f32,
    pub hardness: f32,
    /// The bottom of the top band, which `FOCUS` moves.
    ///
    /// **The specification says 6 kHz**, which is where the boundary sits at
    /// `FOCUS` = 0. It follows the boundary instead, because what the filter is
    /// for is "do not add below the band this came from" — and that moves
    /// (`SPK-6`).
    pub edge_hz: f32,
    pub factor: Factor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            snap: 0.5,
            bias: 0.30,
            hardness: 0.35,
            edge_hz: crate::crossover::EDGES[crate::crossover::BAND_COUNT - 2],
            factor: Factor::default(),
        }
    }
}

/// The generated layer, and the gate in front of it.
pub struct Sparkle {
    sample_rate: f32,
    /// The lid, as two cascaded Butterworth sections.
    lid: [Biquad; 2],
    oversampler: Oversampler,
    shaper: Shaper,
    /// "Only the top gets added", as two cascaded Butterworth sections.
    highpass: [Biquad; 2],
    fast: Power,
    slow: Power,
    /// The gate's own release (see [`HOLD_SECONDS`]).
    hold: f32,
    settings: Settings,
    opening: f32,
    transient: f32,
    gain: f32,
}

impl Sparkle {
    pub fn new(sample_rate: f32) -> Self {
        let mut sparkle = Self {
            sample_rate,
            lid: [Biquad::default(); 2],
            oversampler: Oversampler::new(),
            shaper: Shaper::new(),
            highpass: [Biquad::default(); 2],
            fast: Power::new(FAST_SECONDS, FAST_SECONDS, sample_rate),
            slow: Power::new(SLOW_SECONDS, SLOW_SECONDS, sample_rate),
            hold: coefficient(HOLD_SECONDS, sample_rate),
            // Not a reachable setting, so the first `set` builds everything.
            settings: Settings {
                edge_hz: f32::NAN,
                ..Settings::default()
            },
            opening: 0.0,
            transient: 0.0,
            gain: 0.0,
        };
        sparkle.set(Settings::default());
        sparkle.tune_lid();
        sparkle
    }

    /// **Block rate.** Everything expensive — the curve and both filters — is
    /// resolved here.
    pub fn set(&mut self, settings: Settings) {
        if settings == self.settings {
            return;
        }
        if settings.edge_hz != self.settings.edge_hz {
            let corner = finite(settings.edge_hz, 6_000.0).max(1.0);
            let coefficients = Coefficients::highpass(corner, BUTTERWORTH_Q, self.sample_rate);
            for section in &mut self.highpass {
                section.set(coefficients);
            }
        }
        self.settings = settings;
        self.shaper.set(DRIVE, settings.bias, settings.hardness);
        self.oversampler.set_factor(settings.factor);
    }

    /// One sample of the top band in, the layer to add out. **Audio rate.**
    ///
    /// `air` is how much of the layer to add, `0..=1`. **It is an argument
    /// rather than a setting** because it carries `SPARK`, which is smoothed
    /// per sample — reading it once per block would step the layer on every
    /// automation ramp (`VEL-5`).
    pub fn process(&mut self, band: f32, air: f32) -> f32 {
        let band = finite(band, 0.0);

        // The gate reads the band **before** the curve: what opens it is the
        // music arriving, not what the curve did with it.
        let squared = band * band;
        let fast = self.fast.push(squared);
        let slow = self.slow.push(squared);
        let raw = opening_of(fast, slow);
        self.transient = raw;
        // Opens at once and closes over `HOLD_SECONDS`, so two consonants in a
        // row are one opening rather than two.
        self.opening = if raw > self.opening {
            raw
        } else {
            let held = self.opening + (raw - self.opening) * self.hold;
            if held < CLOSED { 0.0 } else { held }
        };

        let air = finite(air, 0.0).clamp(0.0, 1.0);
        let snap = finite(self.settings.snap, 0.0).clamp(0.0, 1.0);
        self.gain = air * ((1.0 - snap) + snap * self.opening);

        // `AIR` at zero is **exactly** nothing, not a multiplication by zero
        // that still costs an oversampled block (`REQ-SPK-007`).
        if self.gain == 0.0 {
            return 0.0;
        }

        let Self {
            lid,
            oversampler,
            shaper,
            highpass,
            ..
        } = self;

        let capped = lid.iter_mut().fold(band, |x, section| section.process(x));
        let generated = oversampler.process(capped, |sample| shaper.shape(sample));
        let filtered = highpass
            .iter_mut()
            .fold(generated, |x, section| section.process(x));

        filtered * self.gain
    }

    /// How far the gate is open, `0..=1` — for the display (`REQ-SPK-018`).
    pub fn opening(&self) -> f32 {
        self.opening
    }

    /// The same detector **before the hold**, `0..=1` (`REQ-SPK-020`).
    ///
    /// [`opening`](Self::opening) opens at once and closes over
    /// `HOLD_SECONDS`, so two consonants in a row read as one — which is what
    /// the layer wants and the opposite of what hitting a transient wants.
    /// **Held, it stays up through the body of a note**: `SPK-22` drove `PUNCH`
    /// from it first and measured the crest factor going *down*, because the
    /// boost was landing on the sustain as much as on the attack.
    ///
    /// This is the un-held tap of the same fast-over-slow ratio. **Not a second
    /// detector** — one detector, read at two points.
    pub fn transient(&self) -> f32 {
        self.transient
    }

    /// The lid's corner, in Hz.
    pub fn lid_hz(&self) -> f32 {
        (self.sample_rate * INPUT_CEILING).min(INPUT_CEILING_HZ)
    }

    pub fn reset(&mut self) {
        for section in self.lid.iter_mut().chain(&mut self.highpass) {
            section.reset();
        }
        self.oversampler.reset();
        self.fast.reset();
        self.slow.reset();
        self.opening = 0.0;
        self.transient = 0.0;
    }

    fn tune_lid(&mut self) {
        let coefficients = Coefficients::lowpass(self.lid_hz(), BUTTERWORTH_Q, self.sample_rate);
        for section in &mut self.lid {
            section.set(coefficients);
        }
    }
}

/// How far the gate stands open, from the two followers.
///
/// A **ratio**, so it does not move with input gain — the same property
/// `nxe_audio::guard` is built on.
fn opening_of(fast: f32, slow: f32) -> f32 {
    if !(fast.is_finite() && slow.is_finite()) || slow < ENERGY_FLOOR {
        return 0.0;
    }
    let excess_db = DECIBELS_PER_OCTAVE_POWER * (fast / slow).max(1e-20).log2();
    (excess_db / SNAP_RANGE_DB).clamp(0.0, 1.0)
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

    /// The curve at one end of the `CHARACTER` axis, taken from the axis
    /// itself rather than copied — the numbers move in `SPK-18`.
    fn curve_at(position: f32) -> (f32, f32) {
        let character = crate::character::at(position);
        (character.bias, character.hardness)
    }

    fn polish() -> (f32, f32) {
        curve_at(0.0)
    }

    fn crush() -> (f32, f32) {
        curve_at(1.0)
    }

    /// The full amount, which is what every test but the one about zero wants.
    const AIR: f32 = 1.0;

    fn settings(snap: f32, curve: (f32, f32)) -> Settings {
        Settings {
            snap,
            bias: curve.0,
            hardness: curve.1,
            ..Settings::default()
        }
    }

    /// The layer produced for a steady tone, after one settling pass.
    fn layer(sparkle: &mut Sparkle, hz: usize, amplitude: f32) -> Vec<f32> {
        let input = sine(amplitude, hz, LENGTH);
        for sample in &input {
            sparkle.process(*sample, AIR);
        }
        input.iter().map(|s| sparkle.process(*s, AIR)).collect()
    }

    /// **Zero is exactly nothing**, not nearly (`REQ-SPK-007`).
    #[test]
    fn air_at_zero_is_exactly_silent() {
        for snap in [0.0f32, 0.5, 1.0] {
            let mut sparkle = Sparkle::new(RATE);
            sparkle.set(settings(snap, crush()));
            for sample in sine(0.5, 9_000, 4_800) {
                assert_eq!(
                    sparkle.process(sample, 0.0),
                    0.0,
                    "snap {snap} let something through"
                );
            }
        }
    }

    /// `SNAP` = 0 is Velour's behaviour: the layer is there all the time, and
    /// the gate is not consulted (`REQ-SPK-007`).
    #[test]
    fn snap_at_zero_is_static() {
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(0.0, crush()));

        // A burst: silence, then a tone. A static layer tracks the input and
        // nothing else.
        let tone = sine(0.5, 9_000, LENGTH);
        for sample in &tone {
            sparkle.process(*sample, AIR);
        }
        let early: f32 = rms(&(0..2_400)
            .map(|i| sparkle.process(tone[i], AIR))
            .collect::<Vec<_>>());
        let late: f32 = rms(&(2_400..4_800)
            .map(|i| sparkle.process(tone[i], AIR))
            .collect::<Vec<_>>());
        assert!(
            db_ratio(late, early).abs() < 0.5,
            "a static layer moved {:.2} dB",
            db_ratio(late, early)
        );
    }

    /// `SNAP` = 1 hands the layer entirely to the transient: it opens where the
    /// music starts and closes where it sustains.
    #[test]
    fn snap_at_one_opens_on_an_attack_and_closes_on_a_sustain() {
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(1.0, crush()));

        // Half a second of silence, so both followers are at rest.
        for _ in 0..(RATE * 0.5) as usize {
            sparkle.process(0.0, AIR);
        }
        assert_eq!(sparkle.opening(), 0.0, "silence opened the gate");

        // The first 5 ms of a tone.
        let tone = sine(0.5, 9_000, LENGTH);
        for sample in tone.iter().take((RATE * 0.005) as usize) {
            sparkle.process(*sample, AIR);
        }
        let attack = sparkle.opening();
        assert!(attack > 0.5, "the attack only opened it to {attack:.2}");

        // And a second of the same tone, which is no longer news.
        for sample in tone.iter().skip((RATE * 0.005) as usize) {
            sparkle.process(*sample, AIR);
        }
        // Measured 0.008 at 9 kHz, and 0.010 at 6.5 kHz where the fast
        // follower's own ripple is largest.
        let sustain = sparkle.opening();
        assert!(
            sustain < 0.05,
            "a sustained tone held the gate {sustain:.3} open"
        );
    }

    /// **The lid is a fraction of the host rate** (`REQ-SPK-007`). Writing it
    /// against the internal rate is the mistake `VEL-3` made, and it opens the
    /// lid to 48 kHz at 4x.
    #[test]
    fn the_lid_is_a_fraction_of_the_host_rate() {
        for (rate, expected) in [
            (44_100.0f32, 11_025.0f32),
            (48_000.0, 12_000.0),
            // The absolute lid, which is what makes 48 and 96 the same plugin
            // (`SPK-9`).
            (96_000.0, 12_000.0),
            (192_000.0, 12_000.0),
        ] {
            let sparkle = Sparkle::new(rate);
            assert!(
                (sparkle.lid_hz() - expected).abs() < 1.0,
                "{rate} Hz put the lid at {}",
                sparkle.lid_hz()
            );
        }

        // And the factor does not move it, which is the actual trap.
        let mut sparkle = Sparkle::new(RATE);
        let four = sparkle.lid_hz();
        sparkle.set(Settings {
            factor: Factor::Two,
            ..settings(0.0, crush())
        });
        assert_eq!(sparkle.lid_hz(), four, "the factor moved the lid");
    }

    /// The loudest fold that is not sitting on a real harmonic, in dB below the
    /// layer's own fundamental.
    fn worst_alias_db(hz: usize) -> f32 {
        let mut sparkle = Sparkle::new(RATE);
        // The worst curve the product can reach, and then some.
        sparkle.set(Settings {
            bias: BIAS_MAX,
            hardness: 1.0,
            ..settings(0.0, crush())
        });
        let output = layer(&mut sparkle, hz, PROBE_AMPLITUDE);

        let reference = amplitude(&output, hz);
        let mut worst = 0.0f32;
        for harmonic in 2..200usize {
            let true_hz = harmonic * hz;
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

    #[test]
    fn the_worst_case_aliasing_stays_below_sixty_db() {
        for hz in [7_000usize, 9_000, 11_000] {
            let worst = worst_alias_db(hz);
            assert!(worst < -60.0, "{hz} Hz folded back at {worst:.1} dB");
        }
    }

    /// **`CHARACTER` changes what the harmonics are, not how much is added**
    /// (`REQ-SPK-010`, `REQ-SPK-006`). The curve is normalised for RMS at a
    /// reference amplitude, and this is the property that rests on it. The two
    /// ends are read off the axis rather than written here, so this is a test
    /// of the axis and not of two numbers.
    #[test]
    fn character_moves_the_harmonics_without_moving_the_amount() {
        let measure = |curve| {
            let mut sparkle = Sparkle::new(RATE);
            sparkle.set(settings(0.0, curve));
            let output = layer(&mut sparkle, 7_000, PROBE_AMPLITUDE);
            let second = amplitude(&output, 14_000);
            let third = amplitude(&output, 21_000);
            (rms(&output), second / third)
        };

        let (polish_level, polish_ratio) = measure(polish());
        let (crush_level, crush_ratio) = measure(crush());

        assert!(
            polish_ratio > crush_ratio * 1.5,
            "POLISH {polish_ratio:.3} against CRUSH {crush_ratio:.3} is not a \
             different balance"
        );
        let drift = db_ratio(crush_level, polish_level);
        assert!(
            drift.abs() < 3.0,
            "the axis moved the amount by {drift:.2} dB"
        );
    }

    /// The generated layer is added on top, so what it makes below the band it
    /// came from has to be gone (`dsp.md`).
    #[test]
    fn nothing_is_added_below_the_edge() {
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(0.0, crush()));

        // Two tones in the band, so the curve's even term makes a difference
        // tone at 2 kHz — well below the 6 kHz edge.
        let input: Vec<f32> = sine(PROBE_AMPLITUDE, 7_000, LENGTH)
            .iter()
            .zip(sine(PROBE_AMPLITUDE, 9_000, LENGTH))
            .map(|(a, b)| a + b)
            .collect();
        for sample in &input {
            sparkle.process(*sample, AIR);
        }
        let output: Vec<f32> = input.iter().map(|s| sparkle.process(*s, AIR)).collect();

        let reference = amplitude(&output, 7_000);
        let difference = db_ratio(amplitude(&output, 2_000), reference);
        assert!(
            difference < -30.0,
            "the difference tone came through at {difference:.1} dB"
        );
    }

    #[test]
    fn hostile_settings_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut sparkle = Sparkle::new(RATE);
            sparkle.set(Settings {
                snap: value,
                bias: value,
                hardness: value,
                edge_hz: value,
                factor: Factor::Four,
            });
            for sample in sine(0.5, 9_000, 4_800) {
                let reading = sparkle.process(sample, value);
                assert!(reading.is_finite(), "{value} produced {reading}");
            }
            assert!((0.0..=1.0).contains(&sparkle.opening()));
        }

        // And a hostile sample must not poison the recursive parts.
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(0.0, crush()));
        for value in wild {
            sparkle.process(value, AIR);
        }
        let output = layer(&mut sparkle, 9_000, PROBE_AMPLITUDE);
        assert!(
            rms(&output) > 1e-6,
            "the layer went silent and stayed silent"
        );
    }

    /// **The gate has to actually shut** (`REQ-SPK-018`). A one-pole
    /// asymptotes, so without a floor it settles on a denormal that a picture
    /// draws as a permanently ajar gate.
    #[test]
    fn the_gate_shuts_completely_when_the_music_stops() {
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(1.0, crush()));
        for sample in sine(0.5, 9_000, 4_800) {
            sparkle.process(sample, AIR);
        }
        assert!(sparkle.opening() > 0.0, "it never opened");

        // A second of nothing, against a 40 ms hold.
        for _ in 0..RATE as usize {
            sparkle.process(0.0, AIR);
        }
        assert_eq!(sparkle.opening(), 0.0, "the gate stayed ajar");
    }

    #[test]
    fn reset_clears_it() {
        let mut sparkle = Sparkle::new(RATE);
        sparkle.set(settings(1.0, crush()));
        layer(&mut sparkle, 9_000, 0.5);
        sparkle.reset();
        assert_eq!(sparkle.opening(), 0.0);
        for _ in 0..64 {
            assert_eq!(sparkle.process(0.0, AIR), 0.0, "state survived the reset");
        }
    }
}
