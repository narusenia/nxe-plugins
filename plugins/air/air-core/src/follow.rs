//! What the layer is made to follow — **the part of Air that is not a
//! saturator** (`REQ-AIR-002`).
//!
//! Three detectors, none of them new: the input's loudness, how bright it is
//! against itself, and whether something just started. What is new is the
//! composition (`REQ-AIR-007`):
//!
//! ```text
//! 係数ᵢ = (1 − 深さᵢ) + 深さᵢ × 検出値ᵢ
//! gain  = 係数_env × 係数_bright × 係数_trans
//! ```
//!
//! ## Why not a plain product of three gates
//!
//! Three gates multiplied together only open when the input is loud **and**
//! bright **and** starting, which is almost never. Blending each detector
//! against one is what lets a depth of zero mean "this detector does not exist"
//! — exactly, with no rounding — and it is the shape Sparkleur's `SNAP` already
//! uses (`REQ-SPK-007`).
//!
//! **That is also what makes the boundary auditable.** Depths all zero is a
//! static generator, which is Velour; `TRANSIENT` alone at one is Sparkleur's
//! Sparkle. The two neighbours are coordinates in Air's own parameter space
//! rather than a paragraph in a document (`REQ-AIR-002`).
//!
//! ## Why the transient detector listens through a band-pass
//!
//! Sparkleur's gate reads a signal that is **already only the top band**, and
//! its 6 dB range is calibrated against that. Full-band, the same ratio is
//! diluted by whatever the bass is doing, and a hat inside a mix barely moves
//! it. Feeding it the same detection band `BRIGHTNESS` uses puts it back in
//! Sparkleur's situation — so the number can be borrowed instead of guessed —
//! and costs no extra filter (`dsp.md`).

use nxe_audio::Envelope;
use nxe_audio::biquad::BandPass;
use nxe_audio::envelope::{Power, coefficient};

use crate::noise::corner_of;

/// How many detectors there are. The display shows them separately, because
/// they multiply: one shut is the whole layer gone, and "which one" is the only
/// useful thing to say about it (`REQ-AIR-018`).
pub const DETECTORS: usize = 3;

pub const ENVELOPE: usize = 0;
pub const BRIGHTNESS: usize = 1;
pub const TRANSIENT: usize = 2;

/// Long enough to read a note rather than a cycle, short enough to let go
/// between phrases.
const ENVELOPE_ATTACK_SECONDS: f32 = 0.010;
const ENVELOPE_RELEASE_SECONDS: f32 = 0.200;

/// Where the loudness detector reads nothing, and over how many dB it opens.
///
/// **Ear** (`dsp.md`): whether a quiet phrase keeps its surface.
const ENVELOPE_FLOOR_DB: f32 = -48.0;
const ENVELOPE_RANGE_DB: f32 = 36.0;

/// Slower than the loudness detector, because a spectral balance that moves
/// inside a syllable is a consonant, not a brightness.
const BRIGHTNESS_ATTACK_SECONDS: f32 = 0.020;
const BRIGHTNESS_RELEASE_SECONDS: f32 = 0.200;

/// Where the brightness detector reads shut, and over how many dB it opens.
///
/// **A band-to-reference power ratio, not a level** — which is what makes it
/// independent of input gain, the same property `nxe_audio::guard` is built on
/// (`REQ-AIR-007`). Measured at ±12 dB of input gain, the reading moves by
/// **nothing at all**: −0.284 dB against −0.284 dB.
///
/// **Placed from the distribution, not from taste** (`AIR-5`, the lesson of
/// `SPK-18`). Median band-to-reference ratio, five materials at −18 dBFS:
///
/// | | dB |
/// |---|---|
/// | 220 Hz tone | −43.3 |
/// | low-passed pink (1 kHz) | −27.2 |
/// | **pink** | **−0.3** |
/// | white | +10.4 |
/// | high-passed pink (4 kHz) | +24.6 |
///
/// The floor sits below the ordinary material rather than on it, so pink reads
/// **0.65** — open, but with somewhere to go. Anything actually dark lands
/// under the floor and shuts.
pub const BRIGHTNESS_FLOOR_DB: f32 = -12.0;
pub const BRIGHTNESS_RANGE_DB: f32 = 18.0;

/// Inside one consonant, and what it is compared against.
const FAST_SECONDS: f32 = 0.001;
const SLOW_SECONDS: f32 = 0.100;
/// How long the gate stays open once opened. **The release belongs to the gate,
/// not to the follower** — both followers are symmetric so that their ratio is
/// one on anything steady (`SPK-6`).
const HOLD_SECONDS: f32 = 0.040;

/// How much fast-over-slow counts as fully open, in dB.
///
/// Sparkleur's number, borrowed because the band-pass above puts this detector
/// in Sparkleur's situation (`sparkleur_core::sparkle::SNAP_RANGE_DB`) — and
/// then checked rather than assumed: a burst train peaks the gate at **1.000**
/// and a steady tone at **0.023** (`AIR-5`).
pub const TRANSIENT_RANGE_DB: f32 = 6.0;

/// The bottom of the reference band. Below this is rumble, which says nothing
/// about how bright a source is.
const REFERENCE_LOW_HZ: f32 = 200.0;
/// The top of the detection band, and how far it may reach at a given rate.
const DETECTION_TOP_HZ: f32 = 20_000.0;
const DETECTION_TOP_CEILING: f32 = 0.45;

const ENERGY_FLOOR: f32 = 1e-10;
/// Below this the gate counts as shut. A one-pole never reaches its target, so
/// without a floor it creeps down through numbers no picture can draw (`SPK-16`).
const CLOSED: f32 = 1e-6;
/// `10·log10(x)` is `10/log2(10)` times `log2(x)`.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// How much of each detector to use. **Zero is exactly "this detector does not
/// exist"** (`REQ-AIR-007`).
pub type Depths = [f32; DETECTORS];

/// Everything about the following that is not the signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// `-1..=1`. Moves the detection band with the layer, so a `FOCUS` that
    /// puts the layer above the detector cannot happen (`REQ-AIR-009`).
    pub focus: f32,
    pub depths: Depths,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus: 0.0,
            depths: [0.0; DETECTORS],
        }
    }
}

/// The three detectors and their composition.
pub struct Follow {
    sample_rate: f32,
    loudness: Envelope,
    /// **One band-pass for two detectors.** `BRIGHTNESS` and `TRANSIENT` ask
    /// about the same region — the one the layer is placed in — so they share
    /// the filter and differ only in their time constants.
    detection: BandPass,
    reference: BandPass,
    detection_power: Power,
    reference_power: Power,
    fast: Power,
    slow: Power,
    hold: f32,
    opening: f32,
    values: [f32; DETECTORS],
    coefficients: [f32; DETECTORS],
    settings: Settings,
}

impl Follow {
    pub fn new(sample_rate: f32) -> Self {
        let sample_rate = if sample_rate.is_finite() && sample_rate > 0.0 {
            sample_rate
        } else {
            48_000.0
        };
        let (detection, reference) = bands_of(0.0, sample_rate);
        let mut follow = Self {
            sample_rate,
            loudness: Envelope::new(
                ENVELOPE_ATTACK_SECONDS,
                ENVELOPE_RELEASE_SECONDS,
                sample_rate,
            ),
            detection: BandPass::new(detection.0, detection.1, sample_rate),
            reference: BandPass::new(reference.0, reference.1, sample_rate),
            detection_power: Power::new(
                BRIGHTNESS_ATTACK_SECONDS,
                BRIGHTNESS_RELEASE_SECONDS,
                sample_rate,
            ),
            reference_power: Power::new(
                BRIGHTNESS_ATTACK_SECONDS,
                BRIGHTNESS_RELEASE_SECONDS,
                sample_rate,
            ),
            fast: Power::new(FAST_SECONDS, FAST_SECONDS, sample_rate),
            slow: Power::new(SLOW_SECONDS, SLOW_SECONDS, sample_rate),
            hold: coefficient(HOLD_SECONDS, sample_rate),
            opening: 0.0,
            values: [0.0; DETECTORS],
            coefficients: [1.0; DETECTORS],
            // Every field differs, so the first `set` builds all of it
            // (`AIR-1`).
            settings: Settings {
                focus: f32::NAN,
                depths: [f32::NAN; DETECTORS],
            },
        };
        follow.set(Settings::default());
        follow
    }

    /// **Block rate.**
    pub fn set(&mut self, settings: Settings) {
        let settings = Settings {
            focus: finite(settings.focus, 0.0).clamp(-1.0, 1.0),
            depths: settings
                .depths
                .map(|depth| finite(depth, 0.0).clamp(0.0, 1.0)),
        };
        if settings == self.settings {
            return;
        }
        if settings.focus != self.settings.focus {
            let (detection, reference) = bands_of(settings.focus, self.sample_rate);
            self.detection
                .retune(detection.0, detection.1, self.sample_rate);
            self.reference
                .retune(reference.0, reference.1, self.sample_rate);
        }
        self.settings = settings;
    }

    /// One frame of the mono sum. **Audio rate.**
    pub fn push(&mut self, mono: f32) {
        let mono = finite(mono, 0.0);

        self.loudness.push(mono);
        self.values[ENVELOPE] =
            ((self.loudness.decibels() - ENVELOPE_FLOOR_DB) / ENVELOPE_RANGE_DB).clamp(0.0, 1.0);

        let detected = self.detection.process(mono);
        let reference = self.reference.process(mono);
        let detected_energy = self.detection_power.push(detected * detected);
        let reference_energy = self.reference_power.push(reference * reference);
        self.values[BRIGHTNESS] = brightness_of(detected_energy, reference_energy);

        // The transient reads the same band, with its own pair of followers.
        let squared = detected * detected;
        let fast = self.fast.push(squared);
        let slow = self.slow.push(squared);
        let raw = opening_of(fast, slow);
        self.opening = if raw > self.opening {
            raw
        } else {
            let held = self.opening + (raw - self.opening) * self.hold;
            if held < CLOSED { 0.0 } else { held }
        };
        self.values[TRANSIENT] = self.opening;

        for index in 0..DETECTORS {
            let depth = self.settings.depths[index];
            // `depth == 0` has to be **exactly** one, which this is: `1 − 0`
            // plus `0 × anything` is `1` with no rounding to argue about.
            self.coefficients[index] = (1.0 - depth) + depth * self.values[index];
        }
    }

    /// What the layer's gain should be multiplied by.
    pub fn gain(&self) -> f32 {
        self.coefficients.iter().product()
    }

    /// The three coefficients, separately — which is what the display needs
    /// (`REQ-AIR-018`).
    pub fn coefficients(&self) -> [f32; DETECTORS] {
        self.coefficients
    }

    /// The raw detections, before the depths are applied.
    pub fn values(&self) -> [f32; DETECTORS] {
        self.values
    }

    pub fn reset(&mut self) {
        self.loudness.reset();
        self.detection.reset();
        self.reference.reset();
        self.detection_power.reset();
        self.reference_power.reset();
        self.fast.reset();
        self.slow.reset();
        self.opening = 0.0;
        self.values = [0.0; DETECTORS];
        self.coefficients = [1.0; DETECTORS];
    }
}

/// Where the two detectors listen, for a `FOCUS` position.
///
/// **The reference must not contain the detection band.** Sparkleur shipped
/// with one that did and pulled 1.3 dB out of ordinary material at the default
/// setting, because raising the detection band raised its own reference with it
/// (`SPK-18`). Here the reference stops at half the corner and the detection
/// starts at it.
pub fn bands_of(focus: f32, sample_rate: f32) -> ((f32, f32), (f32, f32)) {
    let corner = corner_of(focus, sample_rate);
    let top = DETECTION_TOP_HZ.min(sample_rate * DETECTION_TOP_CEILING);
    (
        (corner, top.max(corner * 1.5)),
        (REFERENCE_LOW_HZ, (corner * 0.5).max(REFERENCE_LOW_HZ * 1.5)),
    )
}

/// How bright the detection band is against the reference, `0..=1`.
fn brightness_of(detected: f32, reference: f32) -> f32 {
    if !(detected.is_finite() && reference.is_finite()) || reference < ENERGY_FLOOR {
        return 0.0;
    }
    let ratio_db = DECIBELS_PER_OCTAVE_POWER * (detected / reference).max(1e-20).log2();
    ((ratio_db - BRIGHTNESS_FLOOR_DB) / BRIGHTNESS_RANGE_DB).clamp(0.0, 1.0)
}

/// How far the transient gate stands open, from the two followers.
fn opening_of(fast: f32, slow: f32) -> f32 {
    if !(fast.is_finite() && slow.is_finite()) || slow < ENERGY_FLOOR {
        return 0.0;
    }
    let excess_db = DECIBELS_PER_OCTAVE_POWER * (fast / slow).max(1e-20).log2();
    (excess_db / TRANSIENT_RANGE_DB).clamp(0.0, 1.0)
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};
    use nxe_audio::harmonics::{at_dbfs, noise, pink, tone};

    const RATE: f32 = 48_000.0;
    const SECONDS: f32 = 3.0;
    const MATERIAL_DBFS: f32 = -18.0;

    fn length() -> usize {
        (RATE * SECONDS) as usize
    }

    fn settings(depths: Depths) -> Settings {
        Settings { focus: 0.0, depths }
    }

    /// A signal shaped by two cascaded Butterworth sections.
    fn shaped(signal: Vec<f32>, hz: f32, high: bool) -> Vec<f32> {
        let coefficients = if high {
            Coefficients::highpass(hz, BUTTERWORTH_Q, RATE)
        } else {
            Coefficients::lowpass(hz, BUTTERWORTH_Q, RATE)
        };
        let mut sections = [Biquad::new(coefficients); 2];
        signal
            .into_iter()
            .map(|sample| sections.iter_mut().fold(sample, |x, f| f.process(x)))
            .collect()
    }

    /// Eight bursts a second, ten milliseconds each — a hat pattern, near
    /// enough for a transient detector.
    fn bursts() -> Vec<f32> {
        let period = RATE as usize / 8;
        let open = RATE as usize / 100;
        at_dbfs(noise(1.0, length()), -12.0)
            .iter()
            .enumerate()
            .map(|(index, sample)| if index % period < open { *sample } else { 0.0 })
            .collect()
    }

    /// The detector readings after the followers have settled.
    ///
    /// **A quarter of a second is discarded** — a relative detector reads
    /// infinitely bright until its reference has filled up, which is how a pad
    /// came to record a fully open gate in Sparkleur (`SPK-18`).
    fn settled(depths: Depths, signal: &[f32]) -> ([f32; DETECTORS], [f32; DETECTORS]) {
        let mut follow = Follow::new(RATE);
        follow.set(settings(depths));
        let skip = (RATE * 0.25) as usize;
        let mut values = [0.0f32; DETECTORS];
        let mut count = 0.0f32;
        for (index, sample) in signal.iter().enumerate() {
            follow.push(*sample);
            if index >= skip {
                for (slot, value) in values.iter_mut().zip(follow.values()) {
                    *slot += value;
                }
                count += 1.0;
            }
        }
        (values.map(|total| total / count), follow.coefficients())
    }

    fn material() -> Vec<f32> {
        at_dbfs(pink(1.0, length()), MATERIAL_DBFS)
    }

    /// **Depth zero is exactly one** (`REQ-AIR-007`), whatever the input is
    /// doing — which is what makes "all depths zero is Velour" a statement
    /// about arithmetic rather than about tuning (`REQ-AIR-002`).
    #[test]
    fn every_depth_at_zero_leaves_the_gain_exactly_one() {
        let mut follow = Follow::new(RATE);
        follow.set(settings([0.0; DETECTORS]));
        for sample in material() {
            follow.push(sample);
            assert_eq!(follow.coefficients(), [1.0; DETECTORS]);
            assert_eq!(follow.gain(), 1.0);
        }
    }

    /// `ENVELOPE` at full depth shuts the layer when the music stops
    /// (`REQ-AIR-007`).
    #[test]
    fn the_envelope_detector_closes_on_silence() {
        let mut follow = Follow::new(RATE);
        follow.set(settings([1.0, 0.0, 0.0]));
        for sample in material() {
            follow.push(sample);
        }
        let playing = follow.gain();
        assert!(
            playing > 0.5,
            "ordinary material only opened it to {playing:.2}"
        );

        for _ in 0..(RATE * 2.0) as usize {
            follow.push(0.0);
        }
        assert_eq!(follow.gain(), 0.0, "silence left it open");
    }

    /// `BRIGHTNESS` at full depth shuts on dark material and opens on bright
    /// (`REQ-AIR-007`). Measured: **0.00 / 0.65 / 1.00** for a low-passed pink,
    /// pink, and a high-passed pink.
    #[test]
    fn the_brightness_detector_reads_the_material() {
        let dark = settled(
            [0.0, 1.0, 0.0],
            &at_dbfs(shaped(pink(1.0, length()), 1_000.0, false), MATERIAL_DBFS),
        )
        .0;
        let ordinary = settled([0.0, 1.0, 0.0], &material()).0;
        let bright = settled(
            [0.0, 1.0, 0.0],
            &at_dbfs(shaped(pink(1.0, length()), 4_000.0, true), MATERIAL_DBFS),
        )
        .0;

        assert!(dark[BRIGHTNESS] < 0.05, "dark read {:.3}", dark[BRIGHTNESS]);
        assert!(
            (0.4..0.9).contains(&ordinary[BRIGHTNESS]),
            "ordinary material read {:.3}, which leaves the detector nowhere to go",
            ordinary[BRIGHTNESS]
        );
        assert!(
            bright[BRIGHTNESS] > 0.95,
            "bright read {:.3}",
            bright[BRIGHTNESS]
        );
    }

    /// **And it does not move with input gain** (`REQ-AIR-007`), because it
    /// reads a ratio. Measured: the same number to three decimals across 24 dB.
    #[test]
    fn the_brightness_detector_ignores_input_gain() {
        let loud = settled([0.0, 1.0, 0.0], &at_dbfs(pink(1.0, length()), -6.0)).0;
        let quiet = settled([0.0, 1.0, 0.0], &at_dbfs(pink(1.0, length()), -30.0)).0;
        let difference = (loud[BRIGHTNESS] - quiet[BRIGHTNESS]).abs();
        // 0.2 dB of the detector's own range, the bar `REQ-SPK-008` set.
        assert!(
            difference < 0.2 / BRIGHTNESS_RANGE_DB,
            "24 dB of input moved it {difference:.4}"
        );
    }

    /// `TRANSIENT` at full depth opens where something starts and closes where
    /// it sustains — **the shape of Sparkleur's Sparkle**, which is the second
    /// half of the boundary audit (`REQ-AIR-002`).
    ///
    /// Measured: a burst train peaks at **1.000**, a steady tone at **0.023**.
    #[test]
    fn the_transient_detector_is_sparkleurs_gate() {
        let peak_of = |signal: &[f32]| {
            let mut follow = Follow::new(RATE);
            follow.set(settings([0.0, 0.0, 1.0]));
            let mut peak = 0.0f32;
            for (index, sample) in signal.iter().enumerate() {
                follow.push(*sample);
                if index > (RATE * 0.5) as usize {
                    peak = peak.max(follow.values()[TRANSIENT]);
                }
            }
            peak
        };

        let pad = peak_of(&at_dbfs(tone(1.0, 3_000.0, RATE, length()), MATERIAL_DBFS));
        let hats = peak_of(&bursts());
        assert!(pad < 0.1, "a steady tone held the gate {pad:.3} open");
        assert!(hats > 0.9, "the bursts only opened it to {hats:.3}");
    }

    /// The three are reported separately because they multiply: one shut is the
    /// whole layer gone, and the display is the only place that can say which
    /// (`REQ-AIR-018`).
    #[test]
    fn the_gain_is_the_product_of_the_three() {
        let (_, coefficients) = settled([0.4, 0.7, 1.0], &material());
        let mut follow = Follow::new(RATE);
        follow.set(settings([0.4, 0.7, 1.0]));
        for sample in material() {
            follow.push(sample);
        }
        assert_eq!(follow.coefficients(), coefficients);
        assert_eq!(follow.gain(), coefficients.iter().product::<f32>());
    }

    /// The reference band must never contain the detection band, at any
    /// `FOCUS` — the mistake that pulled 1.3 dB out of ordinary material in
    /// Sparkleur (`SPK-18`).
    #[test]
    fn the_reference_band_never_reaches_the_detection_band() {
        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            for focus in [-1.0f32, -0.5, 0.0, 0.5, 1.0] {
                let (detection, reference) = bands_of(focus, rate);
                assert!(
                    reference.1 <= detection.0,
                    "{rate} Hz, FOCUS {focus}: reference {reference:?} reaches into {detection:?}"
                );
                assert!(detection.1 > detection.0 && reference.1 > reference.0);
            }
        }
    }

    #[test]
    fn hostile_input_neither_panics_nor_latches_it() {
        let mut follow = Follow::new(RATE);
        follow.set(Settings {
            focus: f32::NAN,
            depths: [f32::INFINITY, -1e9, f32::NAN],
        });
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9] {
            follow.push(value);
            assert!(
                follow.gain().is_finite(),
                "{value} produced {}",
                follow.gain()
            );
        }
        // **Recovery takes seconds, and that is correct.** A sample of 1e9 is
        // a genuine event, and the slow follower it lands in has a 100 ms
        // release — so it takes about fifty time constants before ordinary
        // material can look like a transient again. What must not happen is
        // that it never does.
        follow.set(settings([1.0; DETECTORS]));
        for _ in 0..4 {
            for sample in material() {
                follow.push(sample);
            }
        }
        assert!(
            follow.gain() > 0.0,
            "the detectors went shut and stayed shut"
        );
    }

    #[test]
    fn reset_clears_it() {
        let mut follow = Follow::new(RATE);
        follow.set(settings([1.0; DETECTORS]));
        for sample in material() {
            follow.push(sample);
        }
        follow.reset();
        assert_eq!(follow.coefficients(), [1.0; DETECTORS]);
        assert_eq!(follow.values(), [0.0; DETECTORS]);
    }
}
