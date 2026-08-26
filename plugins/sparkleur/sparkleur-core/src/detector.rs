//! How loud each band is, and how fast that reading is allowed to move.
//!
//! ## Power, and no band-pass
//!
//! One follower per band, fed the band's own signal squared (`REQ-SPK-004`).
//! Velour's guard had to band-pass first because it detected from an unsplit
//! signal; here the crossover has already done that, so there is nothing left
//! to filter. What the up-and-down compressor wants to know is how much a band
//! is sounding, which is power — transients are v2's job
//! (`REQ-SPK-020`).
//!
//! ## The mono sum comes for free
//!
//! Detection is linked across channels so that a gain moving on one side cannot
//! throw the image sideways (`REQ-SPK-011`). **A crossover is linear**, so the
//! mono sum's bands are the mean of the two channels' bands — the caller
//! averages what it already has instead of running a third [`Crossover`].
//!
//! [`Crossover`]: crate::crossover::Crossover
//!
//! ## The time constants are physics, not taste
//!
//! There is no per-band Attack / Release. `SPEED` sets one pair, and each band
//! floors it at a number of periods of **its own centre frequency**
//! (`REQ-SPK-005`): one wavelength of 100 Hz is 10 ms, and an attack faster
//! than that modulates the waveform rather than its envelope — which is
//! distortion, not compression. A low band can only be slow, so "make LOW
//! faster" is an operation that must not exist.
//!
//! The floor bites on the bottom bands only. At the fastest `SPEED` the derived
//! floor wins for bands 1–3 on attack and bands 1–2 on release; above those,
//! `SPEED` is what decides.

use crate::crossover::BAND_COUNT;

/// What the lowest band's bottom edge is taken to be when its centre is worked
/// out. Not zero, because `√(0 · 120)` is zero and the floor would be infinite.
const LOWEST_HZ: f32 = 30.0;

/// And what the top band's upper edge is taken to be, held under Nyquist so a
/// high sample rate does not stretch the top band's centre past what exists.
const HIGHEST_HZ: f32 = 20_000.0;

/// What `SPEED` interpolates between, in seconds (`REQ-SPK-005`).
const ATTACK_FAST_SECONDS: f32 = 0.001;
const ATTACK_SLOW_SECONDS: f32 = 0.050;
const RELEASE_FAST_SECONDS: f32 = 0.020;
const RELEASE_SLOW_SECONDS: f32 = 0.400;

/// The floor, in periods of the band's centre frequency.
const ATTACK_FLOOR_PERIODS: f32 = 2.0;
const RELEASE_FLOOR_PERIODS: f32 = 8.0;

/// Below this a band reads as silence. `10·log10` of zero is `-inf`, and an
/// infinity multiplied by a ratio is a NaN in the gain computer.
const ENERGY_FLOOR: f32 = 1e-10;

/// One power follower per band, and the time constants they run at.
pub struct Detector {
    sample_rate: f32,
    energies: [f32; BAND_COUNT],
    /// Kept in seconds as well as in coefficients: the floors are the whole
    /// point of this unit, so what they resolved to has to be readable
    /// (`REQ-SPK-005`).
    attack_seconds: [f32; BAND_COUNT],
    release_seconds: [f32; BAND_COUNT],
    attack: [f32; BAND_COUNT],
    release: [f32; BAND_COUNT],
    speed: f32,
    edges: [f32; BAND_COUNT - 1],
    #[cfg(test)]
    rebuilds: usize,
}

impl Detector {
    pub fn new(sample_rate: f32, edges: [f32; BAND_COUNT - 1]) -> Self {
        let mut detector = Self {
            sample_rate,
            energies: [0.0; BAND_COUNT],
            attack_seconds: [0.0; BAND_COUNT],
            release_seconds: [0.0; BAND_COUNT],
            attack: [0.0; BAND_COUNT],
            release: [0.0; BAND_COUNT],
            // Not a value `SPEED` can take, so the first `set_speed` builds.
            speed: f32::NAN,
            edges,
            #[cfg(test)]
            rebuilds: 0,
        };
        detector.set_speed(0.5, edges);
        detector
    }

    /// `speed` is `0..=1` and **larger is faster**; `edges` comes from the
    /// crossover, because `FOCUS` moves what each band's centre is.
    ///
    /// **Block rate.** Every coefficient is built here; [`push`] does no
    /// arithmetic beyond the follower itself.
    ///
    /// [`push`]: Self::push
    pub fn set_speed(&mut self, speed: f32, edges: [f32; BAND_COUNT - 1]) {
        let speed = if speed.is_finite() {
            speed.clamp(0.0, 1.0)
        } else {
            0.5
        };
        if speed == self.speed && edges == self.edges {
            return;
        }
        self.speed = speed;
        self.edges = edges;

        let attack = geometric(ATTACK_FAST_SECONDS, ATTACK_SLOW_SECONDS, speed);
        let release = geometric(RELEASE_FAST_SECONDS, RELEASE_SLOW_SECONDS, speed);

        for band in 0..BAND_COUNT {
            let centre = self.centre_of(band);
            self.attack_seconds[band] = attack.max(ATTACK_FLOOR_PERIODS / centre);
            self.release_seconds[band] = release.max(RELEASE_FLOOR_PERIODS / centre);
            self.attack[band] = coefficient(self.attack_seconds[band], self.sample_rate);
            self.release[band] = coefficient(self.release_seconds[band], self.sample_rate);
        }

        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
    }

    /// Feeds one sample of every band. **Audio rate**, on the mono sum.
    ///
    /// A sample that is not a number is read as silence rather than let
    /// through: this is state, so one NaN would latch the band for the rest of
    /// the session (`REQ-SPK-004`, and the bug `VEL-10` found in Velour's
    /// guard).
    pub fn push(&mut self, bands: [f32; BAND_COUNT]) {
        for (index, sample) in bands.into_iter().enumerate() {
            let squared = if sample.is_finite() {
                sample * sample
            } else {
                0.0
            };
            let coefficient = if squared > self.energies[index] {
                self.attack[index]
            } else {
                self.release[index]
            };
            self.energies[index] += (squared - self.energies[index]) * coefficient;
        }
    }

    /// How loud a band is, in dB.
    ///
    /// **Power, so this is the band's RMS in dB** — a full-scale sine reads
    /// −3.01 dB, not 0. The gain computer's thresholds are on this scale
    /// (`dsp.md`).
    pub fn decibels(&self, band: usize) -> f32 {
        10.0 * self.energies[band].max(ENERGY_FLOOR).log10()
    }

    /// The attack actually in force, in seconds — `SPEED`'s value or the band's
    /// floor, whichever is slower.
    pub fn attack_seconds(&self, band: usize) -> f32 {
        self.attack_seconds[band]
    }

    pub fn release_seconds(&self, band: usize) -> f32 {
        self.release_seconds[band]
    }

    /// The geometric centre of a band, which is what its floor is derived from.
    pub fn centre_of(&self, band: usize) -> f32 {
        let low = if band == 0 {
            LOWEST_HZ
        } else {
            self.edges[band - 1]
        };
        let high = if band == BAND_COUNT - 1 {
            HIGHEST_HZ.min(self.sample_rate * 0.5)
        } else {
            self.edges[band]
        };
        (low * high).sqrt().max(LOWEST_HZ)
    }

    pub fn reset(&mut self) {
        self.energies = [0.0; BAND_COUNT];
    }

    #[cfg(test)]
    fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

/// `fast` at `position` = 1 and `slow` at 0, geometrically — so equal steps of
/// the knob are equal ratios of time, which is how the ear reads speed.
fn geometric(fast: f32, slow: f32, position: f32) -> f32 {
    fast * (slow / fast).powf(1.0 - position)
}

/// A one-pole reaches `1 - 1/e` of a step in one time constant, the definition
/// `nxe_audio` uses everywhere.
fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
    1.0 - (-1.0 / (seconds * sample_rate)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crossover::{Crossover, EDGES};
    use nxe_audio::harmonics::{rms, tone};

    const RATE: f32 = 48_000.0;
    /// The fastest and the slowest `SPEED` can be asked for.
    const FASTEST: f32 = 1.0;
    const SLOWEST: f32 = 0.0;

    fn fresh(rate: f32) -> Detector {
        Detector::new(rate, EDGES)
    }

    /// Runs a tone through the crossover into the detector and returns the
    /// settled reading of one band.
    fn settle(speed: f32, hz: f32, amplitude: f32, band: usize, rate: f32) -> f32 {
        let mut crossover = Crossover::new(rate);
        let mut detector = fresh(rate);
        detector.set_speed(speed, crossover.edges());

        // Two seconds: the slowest release here is 400 ms.
        for sample in tone(amplitude, hz, rate, rate as usize * 2) {
            detector.push(crossover.split(sample));
        }
        detector.decibels(band)
    }

    /// The band's own RMS, measured straight off the crossover — the bound the
    /// follower has to sit above.
    fn band_rms_db(hz: f32, amplitude: f32, band: usize, rate: f32) -> f32 {
        let mut crossover = Crossover::new(rate);
        let input = tone(amplitude, hz, rate, rate as usize);
        for sample in &input {
            crossover.split(*sample);
        }
        let output: Vec<f32> = input.iter().map(|s| crossover.split(*s)[band]).collect();
        20.0 * rms(&output).log10()
    }

    /// **An asymmetric follower does not read the mean.** It settles between
    /// the band's RMS and its peak power, and where between is set by the
    /// attack/release ratio — 0.05 dB above the RMS where the ratio is 4,
    /// 2.5 dB where it is 20. That is ordinary compressor behaviour, and it is
    /// why the thresholds in `SPK-4` are settled by ear against **this**
    /// reading rather than calculated from a level.
    #[test]
    fn the_reading_sits_between_the_bands_rms_and_its_peak_power() {
        for speed in [SLOWEST, 0.5, FASTEST] {
            for (band, hz) in [
                (0, 60.0f32),
                (1, 220.0),
                (2, 800.0),
                (3, 3_000.0),
                (4, 12_000.0),
            ] {
                let floor = band_rms_db(hz, 1.0, band, RATE);
                // Peak power is 3.01 dB above the RMS of a sine.
                let ceiling = floor + 3.01;
                let reading = settle(speed, hz, 1.0, band, RATE);
                assert!(
                    reading > floor - 0.15 && reading < ceiling + 0.15,
                    "speed {speed}, band {band}: {reading:.2} dB is outside \
                     [{floor:.2}, {ceiling:.2}]"
                );
            }
        }
    }

    /// **What the gain computer actually needs**: the reading moves one dB for
    /// every dB the material moves. Where it sits is a constant offset; that it
    /// is a *constant* is the property.
    #[test]
    fn the_reading_tracks_the_amplitude_one_for_one() {
        for (band, hz) in [(0, 60.0f32), (1, 220.0), (2, 800.0), (3, 3_000.0)] {
            let reference = settle(SLOWEST, hz, 1.0, band, RATE);
            for amplitude in [0.5f32, 0.1, 0.01] {
                let expected = reference + 20.0 * amplitude.log10();
                let reading = settle(SLOWEST, hz, amplitude, band, RATE);
                assert!(
                    (reading - expected).abs() < 0.1,
                    "band {band} at {amplitude}: {reading:.2} dB, wanted {expected:.2}"
                );
            }
        }
    }

    /// **The property this unit exists for** (`REQ-SPK-005`): the bottom band
    /// cannot be made fast, however `SPEED` is set.
    #[test]
    fn the_lowest_band_cannot_go_faster_than_its_floor() {
        let mut detector = fresh(RATE);
        detector.set_speed(FASTEST, EDGES);

        let centre = detector.centre_of(0);
        assert!((centre - 60.0).abs() < 0.5, "centre moved to {centre}");

        let floor = ATTACK_FLOOR_PERIODS / centre;
        assert!(
            (detector.attack_seconds(0) - floor).abs() < 1e-6,
            "attack {} s is not the {floor} s floor",
            detector.attack_seconds(0)
        );
        assert!(
            detector.attack_seconds(0) > ATTACK_FAST_SECONDS * 30.0,
            "the floor did not bite at all"
        );
        assert!(
            (detector.release_seconds(0) - RELEASE_FLOOR_PERIODS / centre).abs() < 1e-6,
            "release {} s is not its floor",
            detector.release_seconds(0)
        );
    }

    /// Which bands the floor decides and which `SPEED` decides — the claim in
    /// `dsp.md`, measured.
    #[test]
    fn the_floor_rules_the_bottom_and_speed_rules_the_top() {
        let mut detector = fresh(RATE);
        detector.set_speed(FASTEST, EDGES);

        // Attack: the floor wins for the bottom three, `SPEED` for the top two.
        for band in 0..3 {
            assert!(
                detector.attack_seconds(band) > ATTACK_FAST_SECONDS,
                "band {band} attack is not floored"
            );
        }
        for band in 3..BAND_COUNT {
            assert!(
                (detector.attack_seconds(band) - ATTACK_FAST_SECONDS).abs() < 1e-6,
                "band {band} attack is floored when it should not be"
            );
        }

        // Release: the floor wins for the bottom two only.
        for band in 0..2 {
            assert!(
                detector.release_seconds(band) > RELEASE_FAST_SECONDS,
                "band {band} release is not floored"
            );
        }
        for band in 2..BAND_COUNT {
            assert!(
                (detector.release_seconds(band) - RELEASE_FAST_SECONDS).abs() < 1e-6,
                "band {band} release is floored when it should not be"
            );
        }

        // And at the slowest `SPEED` nothing is floored: 50 ms is above every
        // attack floor but the bottom band's 33 ms.
        detector.set_speed(SLOWEST, EDGES);
        for band in 1..BAND_COUNT {
            assert!(
                (detector.attack_seconds(band) - ATTACK_SLOW_SECONDS).abs() < 1e-6,
                "band {band} is floored at the slowest speed"
            );
        }
    }

    /// **What the floor buys, measured in the time domain.**
    ///
    /// A one-pole takes one time constant to cover `1 − 1/e` of a step, and the
    /// bottom band's is floored at two wavelengths of its own 60 Hz centre. So
    /// a single cycle of a bass note cannot move the reading — which is what
    /// "follow the envelope, not the waveform" means (`REQ-SPK-005`).
    ///
    /// The contrast is the same detector's top band, whose centre is 11 kHz and
    /// whose floor is therefore 0.2 ms: it follows the same step at the 1 ms
    /// `SPEED` asked for. **That is the speed the floor takes away from the
    /// bottom, and the reason it is not a per-band control.**
    ///
    /// A steady tone is a weaker test than it looks — at 50 Hz the 20 ms
    /// release holds the reading up on its own, so an unfloored attack ripples
    /// barely more than a floored one. The step is where the difference lives.
    #[test]
    fn the_floor_stops_the_lowest_band_following_the_waveform() {
        fn rise_seconds(detector: &mut Detector, band: usize) -> f32 {
            detector.reset();
            // `1 − 1/e` of a unit step, which is one time constant by
            // definition.
            let mark = 1.0 - 1.0 / std::f32::consts::E;
            for index in 0..RATE as usize {
                detector.push([1.0; BAND_COUNT]);
                if detector.energies[band] >= mark {
                    return index as f32 / RATE;
                }
            }
            panic!("band {band} never rose");
        }

        let mut detector = fresh(RATE);
        detector.set_speed(FASTEST, EDGES);

        let floored = rise_seconds(&mut detector, 0);
        let wavelengths = floored * detector.centre_of(0);
        assert!(
            wavelengths > ATTACK_FLOOR_PERIODS - 0.1,
            "the bottom band followed a step in {wavelengths:.2} wavelengths"
        );

        // And the measurement can fail: a band the floor does not reach follows
        // the same step at the speed that was asked for.
        let free = rise_seconds(&mut detector, BAND_COUNT - 1);
        assert!(
            (free - ATTACK_FAST_SECONDS).abs() < ATTACK_FAST_SECONDS * 0.2,
            "the top band took {free} s, not the {ATTACK_FAST_SECONDS} s asked for"
        );
        assert!(
            floored / free > 30.0,
            "the floor only bought {:.1}×",
            floored / free
        );
    }

    /// And the reading does hold still on a steady tone, which is the symptom
    /// the floor exists to prevent.
    #[test]
    fn a_steady_bass_tone_does_not_wobble_the_bottom_band() {
        let mut crossover = Crossover::new(RATE);
        let mut detector = fresh(RATE);
        detector.set_speed(FASTEST, crossover.edges());

        let input = tone(1.0, 50.0, RATE, RATE as usize);
        for sample in &input {
            detector.push(crossover.split(*sample));
        }
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        for sample in &input {
            detector.push(crossover.split(*sample));
            let reading = detector.decibels(0);
            low = low.min(reading);
            high = high.max(reading);
        }
        assert!(
            high - low < 1.0,
            "the bottom band swung {:.2} dB on a steady tone",
            high - low
        );
    }

    /// The time constants are seconds, so the same material has to read the
    /// same however the host is running (`REQ-SPK-005`).
    #[test]
    fn the_ballistics_are_the_same_at_every_sample_rate() {
        for speed in [SLOWEST, 0.5, FASTEST] {
            let mut readings = Vec::new();
            for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
                readings.push(settle(speed, 220.0, 0.5, 1, rate));
            }
            let first = readings[0];
            for reading in &readings {
                assert!(
                    (reading - first).abs() < 0.3,
                    "speed {speed}: {readings:?} disagree"
                );
            }
        }
    }

    #[test]
    fn silence_reads_the_floor_rather_than_an_infinity() {
        let detector = fresh(RATE);
        for band in 0..BAND_COUNT {
            let reading = detector.decibels(band);
            assert_eq!(reading, 10.0 * ENERGY_FLOOR.log10());
            assert!(reading.is_finite());
        }
    }

    /// The trap `VEL-10` found in Velour: one hostile sample must not poison a
    /// recursive detector for the rest of the session.
    #[test]
    fn hostile_samples_do_not_latch_it() {
        let mut detector = fresh(RATE);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            detector.push([value; BAND_COUNT]);
            for band in 0..BAND_COUNT {
                assert!(detector.decibels(band).is_finite(), "{value} latched it");
            }
        }
        for _ in 0..48_000 {
            detector.push([0.5; BAND_COUNT]);
        }
        assert!(detector.decibels(2) > -12.0, "it never recovered");
    }

    #[test]
    fn hostile_speed_and_edges_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 1e9];
        for value in wild {
            let mut detector = fresh(RATE);
            detector.set_speed(value, EDGES);
            for band in 0..BAND_COUNT {
                assert!(
                    detector.attack_seconds(band) > 0.0
                        && detector.attack_seconds(band).is_finite(),
                    "speed {value}: attack {}",
                    detector.attack_seconds(band)
                );
                assert!(
                    detector.release_seconds(band) > 0.0
                        && detector.release_seconds(band).is_finite()
                );
            }

            let mut detector = fresh(RATE);
            detector.set_speed(0.5, [value; BAND_COUNT - 1]);
            for band in 0..BAND_COUNT {
                assert!(detector.centre_of(band) >= LOWEST_HZ, "edges {value}");
                assert!(detector.attack_seconds(band).is_finite());
            }
            for _ in 0..64 {
                detector.push([0.5; BAND_COUNT]);
            }
            for band in 0..BAND_COUNT {
                assert!(detector.decibels(band).is_finite());
            }
        }
    }

    /// **The coefficients belong to the block, not to the sample.**
    #[test]
    fn pushing_never_rebuilds_the_coefficients() {
        let mut detector = fresh(RATE);
        let built = detector.rebuilds();

        for _ in 0..1_000 {
            detector.push([0.5; BAND_COUNT]);
        }
        assert_eq!(detector.rebuilds(), built, "a sample rebuilt the detector");

        detector.set_speed(0.25, EDGES);
        assert_eq!(detector.rebuilds(), built + 1);
        detector.set_speed(0.25, EDGES);
        assert_eq!(
            detector.rebuilds(),
            built + 1,
            "the same setting rebuilt it"
        );
        // `FOCUS` moves the edges, and that has to rebuild — the floors are
        // derived from them.
        detector.set_speed(0.25, EDGES.map(|edge| edge * 2.0));
        assert_eq!(detector.rebuilds(), built + 2);
    }

    #[test]
    fn reset_clears_it() {
        let mut detector = fresh(RATE);
        for _ in 0..48_000 {
            detector.push([0.5; BAND_COUNT]);
        }
        assert!(detector.decibels(0) > -20.0);
        detector.reset();
        assert_eq!(detector.decibels(0), 10.0 * ENERGY_FLOOR.log10());
    }
}
