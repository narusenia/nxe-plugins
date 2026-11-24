//! The spectral engines behind the Spectral Kits: Invert, Shear and
//! Afterimage (`plugins/kit/docs/implementation/spectral-kit-plan.md`).
//!
//! Each runs inside `nxe_spectral::Stft` at `Quality::Normal` — about 23 Hz a
//! bin and a block of latency (43 ms at 48 kHz), which the plugin reports. The
//! mix is applied to the spectrum, so the dry signal comes out of the same
//! transform and is already aligned with the wet one.
//!
//! Host-agnostic and allocation-free once built.

pub mod afterimage;
pub mod invert;
pub mod shear;

pub use afterimage::Afterimage;
pub use invert::Invert;
pub use shear::Shear;

use nxe_spectral::quality::OVERLAP;
use nxe_spectral::{Complex, Frame, Quality, Stft};
use std::f32::consts::{PI, TAU};

/// The lowest and highest frequency the figures and the frequency-dependent
/// settings are laid out over.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// Where `hz` sits between [`LOW_HZ`] and [`HIGH_HZ`] on a log scale, `0..=1`.
pub fn position(hz: f32) -> f32 {
    ((hz / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

/// The STFT, fed a sample at a time.
struct Shell {
    stft: Stft,
    bins: usize,
    bin_hz: f32,
    hop_seconds: f32,
}

impl Shell {
    fn new(sample_rate: f32) -> Self {
        let stft = Stft::new(sample_rate, Quality::Normal);
        let block = stft.block();
        Self {
            stft,
            bins: block / 2 + 1,
            bin_hz: sample_rate / block as f32,
            hop_seconds: (block / OVERLAP) as f32 / sample_rate,
        }
    }

    fn latency(&self) -> usize {
        self.stft.latency()
    }

    fn process(&mut self, input: [f32; 2], on_frame: impl FnMut(&mut Frame<'_>)) -> [f32; 2] {
        let (mut left, mut right) = ([input[0]], [input[1]]);
        let mut channels: [&mut [f32]; 2] = [&mut left, &mut right];
        self.stft.process(&mut channels, on_frame);
        [left[0], right[0]]
    }
}

/// `phase` folded into `-π..=π`.
fn wrap(phase: f32) -> f32 {
    phase - TAU * ((phase + PI) / TAU).floor()
}

/// How far a bin's phase moves in one hop when its frequency is exactly the
/// bin's own.
fn expected_advance(bin: f32) -> f32 {
    TAU * bin / OVERLAP as f32
}

/// The phase-vocoder analysis: a bin's magnitude and its true frequency, in
/// bins, from how far its phase moved since the last frame.
struct Analysis {
    previous: Vec<f32>,
}

impl Analysis {
    fn new(bins: usize) -> Self {
        Self {
            previous: vec![0.0; bins],
        }
    }

    fn read(&mut self, bin: usize, value: Complex<f32>) -> (f32, f32) {
        let phase = value.arg();
        let deviation = wrap(phase - self.previous[bin] - expected_advance(bin as f32));
        self.previous[bin] = phase;
        (value.norm(), bin as f32 + deviation * OVERLAP as f32 / TAU)
    }
}

/// `dry` moved toward `wet` by `mix`.
fn mix(dry: Complex<f32>, wet: Complex<f32>, mix: f32) -> Complex<f32> {
    dry + (wet - dry) * mix
}

#[cfg(test)]
pub(crate) mod test_signal {
    use std::f32::consts::TAU;

    pub const RATE: f32 = 48_000.0;

    pub fn sine(index: usize, hz: f32) -> f32 {
        (TAU * hz * index as f32 / RATE).sin() * 0.5
    }

    /// The level of `hz` in `samples`, by Goertzel.
    pub fn level_at(samples: &[f32], hz: f32) -> f32 {
        let coefficient = 2.0 * (TAU * hz / RATE).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for sample in samples {
            let s0 = sample + coefficient * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coefficient * s1 * s2).sqrt() / samples.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::test_signal::*;
    use super::*;

    #[test]
    fn the_shell_gives_back_the_input_a_block_later() {
        let mut shell = Shell::new(RATE);
        let latency = shell.latency();
        let input: Vec<f32> = (0..RATE as usize / 2).map(|i| sine(i, 440.0)).collect();
        let output: Vec<f32> = input
            .iter()
            .map(|x| shell.process([*x, *x], |_| {})[0])
            .collect();
        for index in latency * 2..output.len() {
            assert!(
                (output[index] - input[index - latency]).abs() < 1e-3,
                "{index}"
            );
        }
    }

    #[test]
    fn the_analysis_finds_a_frequency_between_bins() {
        let shell = Shell::new(RATE);
        let hz = shell.bin_hz * 10.3;
        let mut analysis = Analysis::new(shell.bins);
        let mut found = 0.0;
        let mut stft = Shell::new(RATE);
        for index in 0..RATE as usize / 4 {
            let x = sine(index, hz);
            stft.process([x, x], |frame| {
                found = analysis.read(10, frame.read(0)[10]).1;
            });
        }
        assert!((found - 10.3).abs() < 0.05, "{found}");
    }

    #[test]
    fn wrap_stays_in_range() {
        for phase in [-10.0, -PI, 0.0, 3.0, 7.0, 100.0] {
            let wrapped = wrap(phase);
            assert!((-PI..=PI).contains(&wrapped), "{phase} → {wrapped}");
            assert!(((phase - wrapped) / TAU).fract().abs() < 1e-3);
        }
    }
}
