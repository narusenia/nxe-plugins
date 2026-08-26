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

/// A sine at `hz`, rounded to the nearest whole number of cycles in `length`.
///
/// **This exists because [`sine`] has caused the same bug three times.** Its
/// `cycles` argument is periods *per buffer*, so a frequency written as a cycle
/// count silently changes when the buffer length or the sample rate does — and
/// the failure is not a crash, it is a harmonic ratio that leaks between bins
/// and comes out with the wrong sign. Say the frequency, and let the arithmetic
/// happen once here.
///
/// Pair it with [`bin_of`] so the measurement lands on the same place the tone
/// was put.
pub fn tone(amplitude: f32, hz: f32, sample_rate: f32, length: usize) -> Vec<f32> {
    sine(amplitude, cycles_of(hz, sample_rate, length), length)
}

/// Deterministic white noise in `-1..=1`.
///
/// **A test signal, not a source.** The generator is a plain linear
/// congruential one so that a measurement made from it is the same on every
/// machine and every run — a threshold tuned against a random signal that
/// changes underneath it is not tuned against anything.
pub fn noise(amplitude: f32, length: usize) -> Vec<f32> {
    let mut state = 0x1234_5678u32;
    (0..length)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            // **24 bits over 2^23, then minus one.** `>> 8` leaves 24 bits, so
            // dividing by 2^23 lands in `0..2` and subtracting one centres it.
            // Scaling that range by two first — which is the obvious thing to
            // write — leaves `-1..3`, a signal with a DC term as large as the
            // noise itself.
            amplitude * ((state >> 8) as f32 / (1 << 23) as f32 - 1.0)
        })
        .collect()
}

/// Deterministic pink noise in roughly `-1..=1`, falling 3 dB per octave.
///
/// **This is the proxy for "spectrally ordinary material"**, and the reason it
/// is here rather than in one crate's tests is that [`noise`] is not that
/// proxy. White noise puts four times as much energy in a two-octave band as in
/// a half-octave one purely because of the width, so anything that judges one
/// band against another reads it as bright when nothing is — which is exactly
/// how Sparkleur's De-Harsh threshold came to sit four decibels out of place
/// (`sparkleur_core::protect`).
///
/// Three one-pole sections, which is close enough to −3 dB/octave for a
/// threshold to be judged against.
pub fn pink(amplitude: f32, length: usize) -> Vec<f32> {
    let (mut b0, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32);
    noise(1.0, length)
        .iter()
        .map(|value| {
            b0 = 0.99765 * b0 + value * 0.099_046;
            b1 = 0.96300 * b1 + value * 0.296_516_4;
            b2 = 0.57000 * b2 + value * 1.052_691_3;
            amplitude * (b0 + b1 + b2 + value * 0.1848) * 0.2
        })
        .collect()
}

/// A signal rescaled to a chosen RMS in dBFS.
pub fn at_dbfs(mut signal: Vec<f32>, dbfs: f32) -> Vec<f32> {
    let scale = 10.0f32.powf(dbfs / 20.0) / rms(&signal);
    for sample in &mut signal {
        *sample *= scale;
    }
    signal
}

/// Which DFT bin `hz` lands in, for a buffer of `length` at `sample_rate`.
pub fn bin_of(hz: f32, sample_rate: f32, length: usize) -> usize {
    cycles_of(hz, sample_rate, length)
}

fn cycles_of(hz: f32, sample_rate: f32, length: usize) -> usize {
    ((hz * length as f32 / sample_rate).round() as usize).max(1)
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

    /// The whole point of [`tone`]: the same frequency at every rate and length.
    #[test]
    fn a_tone_is_the_frequency_it_says_it_is() {
        for (rate, length) in [
            (48_000.0f32, 4_800usize),
            (48_000.0, 9_600),
            (44_100.0, 4_410),
            (96_000.0, 9_600),
        ] {
            let signal = tone(1.0, 3_000.0, rate, length);
            let bin = bin_of(3_000.0, rate, length);
            assert!(
                (amplitude(&signal, bin) - 1.0).abs() < 1e-3,
                "{rate}/{length} put it somewhere else"
            );
            // And nowhere near a neighbouring bin, which is what leaks when the
            // cycle count is not whole.
            assert!(amplitude(&signal, bin + 1) < 1e-3);
        }
    }

    #[test]
    fn an_empty_buffer_measures_zero_rather_than_dividing_by_it() {
        assert_eq!(amplitude(&[], 1), 0.0);
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(mean(&[]), 0.0);
    }
}

#[cfg(test)]
mod noise_tests {
    use super::*;

    /// **Pink is pink, and white is not.** The whole point of [`pink`] is that
    /// a band an octave up holds the same power as the one below it; if that
    /// ever stopped being true, every threshold measured against it would move
    /// without anything saying so.
    #[test]
    fn pink_falls_three_decibels_an_octave_and_white_does_not() {
        // A naive DFT per bin is O(n) each, so the buffer stays small enough
        // that summing whole octaves of them is not O(n²) at a noticeable size.
        let length = 1 << 14;
        let rate = 48_000.0;

        // Power in an octave, by summing the bins inside it.
        let octave_db = |signal: &[f32], low_hz: f32| {
            let bin = |hz: f32| (hz / (rate / length as f32)) as usize;
            let power: f32 = (bin(low_hz)..bin(low_hz * 2.0))
                .map(|index| amplitude(signal, index).powi(2))
                .sum();
            10.0 * power.max(1e-30).log10()
        };

        let pink = pink(1.0, length);
        let white = noise(1.0, length);
        // From 250 Hz up: an octave below that holds too few bins at this
        // length for the measurement to say anything.
        for low_hz in [250.0f32, 500.0, 1_000.0, 2_000.0, 4_000.0] {
            let step = octave_db(&pink, low_hz * 2.0) - octave_db(&pink, low_hz);
            assert!(
                step.abs() < 1.5,
                "pink moved {step:.2} dB across the octave above {low_hz:.0} Hz"
            );

            // **And the measurement can fail**: white rises about 3 dB an
            // octave through the same instrument.
            let white_step = octave_db(&white, low_hz * 2.0) - octave_db(&white, low_hz);
            assert!(
                white_step > 2.0,
                "white only moved {white_step:.2} dB above {low_hz:.0} Hz"
            );
        }
    }

    /// **Zero mean, or the pink filter turns a DC term into the whole signal.**
    ///
    /// The first of the three sections has a pole at 0.99765, which is a gain
    /// of 425 at DC. A white source centred on 1.0 rather than 0.0 therefore
    /// comes out of [`pink`] as a large constant with a little noise on it, and
    /// every measurement made against it reads as "all the energy is in the
    /// bottom band".
    #[test]
    fn the_noise_is_centred() {
        let white = noise(1.0, 1 << 16);
        assert!(
            mean(&white).abs() < 0.01,
            "white sat at {:.4}",
            mean(&white)
        );
        let pink = pink(1.0, 1 << 16);
        assert!(
            mean(&pink).abs() < 0.05 * rms(&pink),
            "pink sat at {:.4} against an RMS of {:.4}",
            mean(&pink),
            rms(&pink)
        );
    }

    #[test]
    fn the_generators_are_deterministic_and_scaled() {
        assert_eq!(noise(1.0, 64), noise(1.0, 64));
        assert_eq!(pink(1.0, 64), pink(1.0, 64));
        for dbfs in [-30.0f32, -18.0, -6.0] {
            let level = rms(&at_dbfs(pink(1.0, 4_096), dbfs));
            assert!(
                (20.0 * level.log10() - dbfs).abs() < 0.01,
                "asked for {dbfs} dBFS and got {:.2}",
                20.0 * level.log10()
            );
        }
    }
}
