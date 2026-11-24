//! Afterimage: what was loud stays on as a fading trace
//! (`spectral-kit-plan.md`, decision 5).
//!
//! Each bin keeps the loudest it has been, falling away over `persistence`
//! seconds — the highs sooner, as `damping` asks. The trace that is left over
//! the present signal is added back, at the frequency it was caught at, with a
//! running phase so a held trace rings rather than buzzes. **The present is
//! never doubled**: only what the trace has above it is added, so `mix` 0 is
//! the input and a steady tone comes out as it went in.

use crate::{Analysis, BANDS, Bands, Shell, expected_advance, position, wrap};
use nxe_spectral::Complex;
use nxe_spectral::stft::MAX_CHANNELS;

pub const PERSISTENCE_MIN: f32 = 0.2;
pub const PERSISTENCE_MAX: f32 = 20.0;
/// How much shorter `damping` 1 makes the top's trace than the bottom's.
const DAMPING_RANGE: f32 = 0.9;

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Seconds for a trace to fall by `1/e` at the bottom of the spectrum.
    pub persistence: f32,
    /// `0..=1`: how much sooner the top fades.
    pub damping: f32,
    /// `0..=1`.
    pub mix: f32,
}

/// The trace's time constant at `hz`, in seconds. The figure draws this.
pub fn persistence_at(hz: f32, persistence: f32, damping: f32) -> f32 {
    persistence.max(PERSISTENCE_MIN) * (1.0 - damping * DAMPING_RANGE * position(hz))
}

struct Channel {
    analysis: Analysis,
    trace: Vec<f32>,
    frequency: Vec<f32>,
    phase: Vec<f32>,
    /// The last frame's input magnitudes, for the window.
    present: Vec<f32>,
}

pub struct Afterimage {
    shell: Shell,
    channels: [Channel; MAX_CHANNELS],
    hz: Vec<f32>,
    bands: Bands,
    /// The last frame's input and its trace, in bands, for the window.
    present: [f32; BANDS],
    trace: [f32; BANDS],
}

impl Afterimage {
    pub fn new(sample_rate: f32) -> Self {
        let shell = Shell::new(sample_rate);
        let bins = shell.bins;
        Self {
            channels: std::array::from_fn(|_| Channel {
                analysis: Analysis::new(bins),
                trace: vec![0.0; bins],
                frequency: vec![0.0; bins],
                phase: vec![0.0; bins],
                present: vec![0.0; bins],
            }),
            hz: (0..bins).map(|bin| bin as f32 * shell.bin_hz).collect(),
            bands: shell.bands(),
            present: [0.0; BANDS],
            trace: [0.0; BANDS],
            shell,
        }
    }

    /// In samples: one block.
    pub fn latency(&self) -> usize {
        self.shell.latency()
    }

    /// The input of the last frame, in [`BANDS`] log-spaced bands, as peak
    /// amplitudes.
    pub fn present(&self) -> &[f32; BANDS] {
        &self.present
    }

    /// The trace held over it, the same way.
    pub fn trace(&self) -> &[f32; BANDS] {
        &self.trace
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let Self {
            shell,
            channels,
            hz,
            bands,
            present,
            trace,
        } = self;
        let hop = shell.hop_seconds;
        let settings = *settings;
        shell.process(input, |frame| {
            present.fill(0.0);
            trace.fill(0.0);
            for (index, channel) in channels.iter_mut().enumerate().take(frame.channels()) {
                let bins = frame.channel(index);
                for (bin, value) in bins.iter_mut().enumerate() {
                    let (magnitude, frequency) = channel.analysis.read(bin, *value);
                    channel.present[bin] = magnitude;
                    let tau = persistence_at(hz[bin], settings.persistence, settings.damping);
                    let held = channel.trace[bin] * (-hop / tau).exp();
                    if magnitude >= held {
                        channel.trace[bin] = magnitude;
                        channel.frequency[bin] = frequency;
                        channel.phase[bin] = value.arg();
                        continue;
                    }
                    channel.trace[bin] = held;
                    channel.phase[bin] =
                        wrap(channel.phase[bin] + expected_advance(channel.frequency[bin]));
                    let left = Complex::from_polar(held - magnitude, channel.phase[bin]);
                    *value += left * settings.mix;
                }
                bands.fold(|bin| channel.present[bin], present);
                bands.fold(|bin| channel.trace[bin], trace);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::*;

    /// A second of tone, then a second of silence; the silence is returned.
    fn tail(settings: &Settings) -> Vec<f32> {
        let mut afterimage = Afterimage::new(RATE);
        let second = RATE as usize;
        (0..second * 2)
            .map(|index| {
                let x = if index < second {
                    sine(index, 1_000.0)
                } else {
                    0.0
                };
                afterimage.process([x, x], settings)[0]
            })
            .skip(second + 4_096)
            .collect()
    }

    #[test]
    fn a_tone_leaves_a_trace() {
        let out = tail(&Settings {
            persistence: 4.0,
            damping: 0.0,
            mix: 1.0,
        });
        let early = level_at(&out[..4_800], 1_000.0);
        assert!(early > 0.05, "{early}");
    }

    #[test]
    fn no_mix_leaves_no_trace() {
        let out = tail(&Settings {
            persistence: 4.0,
            damping: 0.0,
            mix: 0.0,
        });
        assert!(out.iter().all(|x| x.abs() < 1e-3));
    }

    #[test]
    fn damping_shortens_the_top() {
        assert!(persistence_at(10_000.0, 4.0, 1.0) < persistence_at(100.0, 4.0, 1.0));
        assert_eq!(persistence_at(10_000.0, 4.0, 0.0), 4.0);
    }
}
