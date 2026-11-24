//! Each measurement with what it publishes, so a single-measurement plugin and
//! `Peek` hold the same pair and cannot drift apart.
//!
//! A `*Meter` is audio-thread state; its `*Readings` is the hand-off the
//! window reads (`nxe_dsp::Handoff`, latest wins). Where the window can ask
//! for something — clearing the maxima — it counts up an atomic and the meter
//! notices on its next publish (`peek-plan.md`, decision 6).

use crate::{ChannelLevel, Loudness, Scope, Stereo, Vu};
use nxe_dsp::{Correlation, Handoff, Spectrum};
use std::sync::atomic::{AtomicU32, Ordering};

pub const STEREO_POINTS: usize = 512;
pub const SCOPE_POINTS: usize = 256;
pub const SPECTRUM_BANDS: usize = 64;
pub const SPECTROGRAM_BANDS: usize = 96;
/// The range every spectrum covers.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// True when the window has asked for a clear since `seen`.
fn cleared(clears: &AtomicU32, seen: &mut u32) -> bool {
    let now = clears.load(Ordering::Relaxed);
    let asked = now != *seen;
    *seen = now;
    asked
}

// ---------------------------------------------------------------- Level

#[derive(Default)]
pub struct LevelReadings {
    /// Blocks published so far, wrapping: how the window tells that the
    /// host has stopped calling (bypass, a stopped transport).
    pub blocks: AtomicU32,
    pub peaks: Handoff<2>,
    pub holds: Handoff<2>,
    pub rms: Handoff<2>,
    /// Since the last clear, across both channels: sample peak, true peak,
    /// RMS.
    pub maxima: Handoff<3>,
    pub clears: AtomicU32,
}

pub struct LevelMeter {
    channels: [ChannelLevel; 2],
    seen: u32,
}

impl LevelMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            channels: std::array::from_fn(|_| ChannelLevel::new(sample_rate)),
            seen: 0,
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.channels[0].push(left);
        self.channels[1].push(right);
    }

    pub fn publish(&mut self, readings: &LevelReadings) {
        readings.blocks.fetch_add(1, Ordering::Relaxed);
        if cleared(&readings.clears, &mut self.seen) {
            self.channels.iter_mut().for_each(ChannelLevel::clear);
        }
        let [left, right] = &self.channels;
        readings
            .peaks
            .write(&[left.level().peak(), right.level().peak()]);
        readings
            .holds
            .write(&[left.level().hold(), right.level().hold()]);
        readings
            .rms
            .write(&[left.level().rms(), right.level().rms()]);
        readings.maxima.write(&[
            left.max_peak().max(right.max_peak()),
            left.max_true_peak().max(right.max_true_peak()),
            left.max_rms().max(right.max_rms()),
        ]);
    }

    /// The louder channel's peak, for a host's meter slot.
    pub fn peak(&self) -> f32 {
        self.channels[0]
            .level()
            .peak()
            .max(self.channels[1].level().peak())
    }
}

// ---------------------------------------------------------------- VU

#[derive(Default)]
pub struct VuReadings {
    /// Blocks published so far, wrapping: how the window tells that the
    /// host has stopped calling (bypass, a stopped transport).
    pub blocks: AtomicU32,
    /// Left and right, as amplitudes (a sine reads its RMS).
    pub vu: Handoff<2>,
    pub correlation: Handoff<1>,
}

pub struct VuMeter {
    channels: [Vu; 2],
    correlation: Correlation,
}

impl VuMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            channels: std::array::from_fn(|_| Vu::new(sample_rate)),
            correlation: Correlation::new(sample_rate),
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.channels[0].push(left);
        self.channels[1].push(right);
        self.correlation.push(left, right);
    }

    pub fn publish(&self, readings: &VuReadings) {
        readings.blocks.fetch_add(1, Ordering::Relaxed);
        readings
            .vu
            .write(&[self.channels[0].value(), self.channels[1].value()]);
        readings.correlation.write(&[self.correlation.value()]);
    }
}

// ---------------------------------------------------------------- Stereo

#[derive(Default)]
pub struct StereoReadings {
    /// Blocks published so far, wrapping: how the window tells that the
    /// host has stopped calling (bypass, a stopped transport).
    pub blocks: AtomicU32,
    pub lefts: Handoff<STEREO_POINTS>,
    pub rights: Handoff<STEREO_POINTS>,
    /// Correlation, then balance.
    pub figures: Handoff<2>,
}

pub struct StereoMeter(Stereo<STEREO_POINTS>);

impl StereoMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self(Stereo::new(sample_rate))
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.0.push(left, right);
    }

    pub fn correlation(&self) -> f32 {
        self.0.correlation()
    }

    pub fn publish(&self, readings: &StereoReadings) {
        readings.blocks.fetch_add(1, Ordering::Relaxed);
        readings.lefts.write(self.0.lefts());
        readings.rights.write(self.0.rights());
        readings
            .figures
            .write(&[self.0.correlation(), self.0.balance()]);
    }
}

// ---------------------------------------------------------------- Spectrum

/// The mid signal's spectrum in `BANDS` log-spaced bands. Spectrum and
/// Spectrogram are the same measurement at two resolutions.
pub struct SpectrumMeter<const BANDS: usize>(Spectrum<BANDS>);

#[derive(Default)]
pub struct SpectrumReadings<const BANDS: usize> {
    /// Blocks published so far, wrapping (`LevelReadings::blocks`).
    pub blocks: AtomicU32,
    /// Amplitude per band, low to high.
    pub levels: Handoff<BANDS>,
}

impl<const BANDS: usize> SpectrumMeter<BANDS> {
    pub fn new(sample_rate: f32) -> Self {
        Self(Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ))
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.0.push((left + right) * 0.5);
    }

    pub fn publish(&self, readings: &SpectrumReadings<BANDS>) {
        readings.blocks.fetch_add(1, Ordering::Relaxed);
        readings.levels.write(&self.0.levels());
    }
}

// ---------------------------------------------------------------- Loudness

pub struct LoudnessReadings {
    /// Momentary, short-term, integrated, range, momentary maximum. LUFS
    /// (range in LU); `-inf` before there is a reading.
    pub values: Handoff<5>,
    pub clears: AtomicU32,
}

impl Default for LoudnessReadings {
    /// Silent until measured: a hand-off starts at zero, and 0 LUFS would
    /// read as the loudest thing possible.
    fn default() -> Self {
        let values = Handoff::new();
        values.write(&[
            crate::loudness::SILENT,
            crate::loudness::SILENT,
            crate::loudness::SILENT,
            0.0,
            crate::loudness::SILENT,
        ]);
        Self {
            values,
            clears: AtomicU32::new(0),
        }
    }
}

pub struct LoudnessMeter {
    loudness: Loudness,
    seen: u32,
}

impl LoudnessMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            loudness: Loudness::new(sample_rate),
            seen: 0,
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.loudness.push(left, right);
    }

    pub fn publish(&mut self, readings: &LoudnessReadings) {
        if cleared(&readings.clears, &mut self.seen) {
            self.loudness.clear();
        }
        let meter = &self.loudness;
        readings.values.write(&[
            meter.momentary(),
            meter.short_term(),
            meter.integrated(),
            meter.range(),
            meter.max_momentary(),
        ]);
    }
}

// ---------------------------------------------------------------- Scope

#[derive(Default)]
pub struct ScopeReadings {
    /// Blocks published so far, wrapping: how the window tells that the
    /// host has stopped calling (bypass, a stopped transport).
    pub blocks: AtomicU32,
    /// Oldest first.
    pub mins: Handoff<SCOPE_POINTS>,
    pub maxs: Handoff<SCOPE_POINTS>,
}

pub struct ScopeMeter(Scope<SCOPE_POINTS>);

impl ScopeMeter {
    pub fn new(sample_rate: f32) -> Self {
        Self(Scope::new(sample_rate))
    }

    pub fn set_window(&mut self, seconds: f32) {
        self.0.set_window(seconds);
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.0.push((left + right) * 0.5);
    }

    pub fn publish(&self, readings: &ScopeReadings) {
        readings.blocks.fetch_add(1, Ordering::Relaxed);
        readings.mins.write(&self.0.mins());
        readings.maxs.write(&self.0.maxs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loudness_starts_silent() {
        let values = LoudnessReadings::default().values.read();
        assert_eq!(values[2], crate::loudness::SILENT);
    }

    #[test]
    fn a_clear_is_noticed_once() {
        let clears = AtomicU32::new(0);
        let mut seen = 0;
        assert!(!cleared(&clears, &mut seen));
        clears.fetch_add(1, Ordering::Relaxed);
        assert!(cleared(&clears, &mut seen));
        assert!(!cleared(&clears, &mut seen));
    }
}
