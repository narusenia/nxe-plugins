//! Measuring what a curve did to a sine.
//!
//! **For tests and benches, not for the audio path.** It allocates, and it is
//! `O(n)` per harmonic. It lives in the crate rather than in a test module
//! because four different units need the same measurement
//! (`velour-plan.md`), and two of them are integration tests.
//!
//! A naive DFT at one frequency rather than an FFT: the frequency is known
//! exactly (a test generates the sine), so there is nothing to search for and
//! no window to choose. One bin is cheaper than a whole transform and has no
//! leakage as long as the buffer holds a whole number of cycles — which is the
//! caller's job, and [`sine`] makes it the easy thing to do.

use std::f32::consts::TAU;

/// A sine of `cycles` whole periods over `length` samples.
///
/// Whole cycles, so a DFT bin lands exactly on it and nothing leaks into the
/// neighbours — which is what makes a harmonic ratio measured off it mean
/// something.
pub fn sine(amplitude: f32, cycles: usize, length: usize) -> Vec<f32> {
    (0..length)
        .map(|index| amplitude * (TAU * cycles as f32 * index as f32 / length as f32).sin())
        .collect()
}

/// The amplitude of the component at `cycles` periods over the buffer.
pub fn amplitude(signal: &[f32], cycles: usize) -> f32 {
    if signal.is_empty() {
        return 0.0;
    }

    let length = signal.len() as f32;
    let (mut real, mut imaginary) = (0.0f32, 0.0f32);

    for (index, sample) in signal.iter().enumerate() {
        let phase = TAU * cycles as f32 * index as f32 / length;
        real += sample * phase.cos();
        imaginary -= sample * phase.sin();
    }

    // `2/N` turns the one-sided sum into the amplitude of a real sinusoid.
    2.0 * (real * real + imaginary * imaginary).sqrt() / length
}

/// The RMS of the whole buffer.
pub fn rms(signal: &[f32]) -> f32 {
    if signal.is_empty() {
        return 0.0;
    }
    (signal.iter().map(|sample| sample * sample).sum::<f32>() / signal.len() as f32).sqrt()
}

/// The mean of the whole buffer — the DC an asymmetric curve leaves behind.
pub fn mean(signal: &[f32]) -> f32 {
    if signal.is_empty() {
        return 0.0;
    }
    signal.iter().sum::<f32>() / signal.len() as f32
}

/// `20·log10(value / reference)`, or a large negative number if `value` is zero.
pub fn db_ratio(value: f32, reference: f32) -> f32 {
    if value <= 0.0 || reference <= 0.0 {
        return -200.0;
    }
    20.0 * (value / reference).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measurement has to be right before anything measured with it means
    /// anything.
    #[test]
    fn a_sine_reads_its_own_amplitude_in_its_own_bin() {
        let signal = sine(0.5, 4, 1024);
        assert!((amplitude(&signal, 4) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn a_sine_reads_nothing_in_another_bin() {
        let signal = sine(0.5, 4, 1024);
        for cycles in [1, 2, 3, 5, 8, 12] {
            assert!(amplitude(&signal, cycles) < 1e-4, "bin {cycles} leaked");
        }
    }

    #[test]
    fn rms_of_a_sine_is_the_amplitude_over_root_two() {
        let signal = sine(1.0, 4, 1024);
        assert!((rms(&signal) - 1.0 / 2.0f32.sqrt()).abs() < 1e-3);
    }

    #[test]
    fn a_sine_has_no_mean() {
        assert!(mean(&sine(1.0, 4, 1024)).abs() < 1e-5);
    }

    #[test]
    fn an_empty_buffer_measures_zero_rather_than_dividing_by_it() {
        assert_eq!(amplitude(&[], 1), 0.0);
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(mean(&[]), 0.0);
    }
}
