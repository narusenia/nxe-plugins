//! What a bin is judged against, and by how much it exceeds it.
//!
//! **Relative, never absolute** (`REQ-PUM-003`). A threshold in dBFS fires on
//! every loud moment, and a loud moment is a loud note rather than a resonant
//! one. Comparing a bin to its own neighbourhood moves the numerator and the
//! denominator together, so **the answer does not depend on input gain** —
//! which is also why `REQ-PUM-003` can require the reduction to hold within
//! 0.2 dB across ±12 dB of input.
//!
//! `nxe_audio::guard` reaches the same conclusion for one band; this is the
//! same judgement spread across every bin, with the reference made of the bins
//! around it rather than of a band chosen in advance.

use realfft::num_complex::Complex;

use crate::smoothing::{FLOOR, Prefix};

/// `10·log10(x)` is `10/log2(10)` times `log2(x)`.
///
/// The same constant `nxe_audio::guard` keeps, for the same reason: `log2` is
/// the one the hardware has.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// Mean power per bin across the channels present.
///
/// **Energy, and linked across channels** (`REQ-PUM-011`). One gain curve is
/// applied to both, so one detection feeds it; a per-channel curve would move
/// the image every time one side had a resonance the other did not.
pub fn power_into(channels: &[&[Complex<f32>]], out: &mut [f32]) {
    let bins = out.len();
    let count = channels.len().max(1) as f32;

    out.fill(0.0);
    for channel in channels {
        for (bin, value) in channel.iter().take(bins).enumerate() {
            out[bin] += value.norm_sqr();
        }
    }
    for value in out.iter_mut() {
        *value /= count;
    }
}

/// How far each bin sits above its own neighbourhood, in dB.
///
/// `width_octaves` is what `SHARPNESS` moves: wide, and a broad hump counts as
/// an excess; narrow, and only a spike does (`REQ-PUM-005`).
pub fn excess_db_into(
    power: &[f32],
    prefix: &mut Prefix,
    reference: &mut [f32],
    width_octaves: f32,
    out: &mut [f32],
) {
    prefix.build(power);
    prefix.average_into(width_octaves, reference);

    for (bin, value) in out.iter_mut().enumerate() {
        let level = power[bin].max(FLOOR);
        let against = reference[bin].max(FLOOR);
        *value = DECIBELS_PER_OCTAVE_POWER * (level / against).log2();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excess(power: &[f32], width: f32) -> Vec<f32> {
        let mut prefix = Prefix::new(power.len());
        let mut reference = vec![0.0; power.len()];
        let mut out = vec![0.0; power.len()];
        excess_db_into(power, &mut prefix, &mut reference, width, &mut out);
        out
    }

    /// `REQ-PUM-003`'s acceptance condition, and the `SPK-18` lesson: a
    /// spectrally ordinary signal must read as **no excess at all**, so a
    /// threshold of zero takes nothing out of it.
    #[test]
    fn a_flat_spectrum_shows_no_excess() {
        let out = excess(&vec![0.5; 1024], 1.0);
        for (bin, value) in out.iter().enumerate().skip(1) {
            assert!(value.abs() < 0.01, "bin {bin}: {value} dB");
        }
    }

    /// The property the whole design rests on: **the answer does not move with
    /// input gain** (`REQ-PUM-003`, ±12 dB within 0.2 dB).
    #[test]
    fn input_gain_does_not_change_the_excess() {
        let mut power = vec![0.01; 2048];
        power[600] = 0.2;

        let quiet = excess(&power, 1.0);
        for gain_db in [-12.0_f32, -6.0, 6.0, 12.0] {
            // Power scales by the square of an amplitude gain.
            let scale = 10.0_f32.powf(gain_db / 10.0);
            let scaled: Vec<f32> = power.iter().map(|value| value * scale).collect();
            let loud = excess(&scaled, 1.0);

            for bin in 1..power.len() {
                assert!(
                    (loud[bin] - quiet[bin]).abs() < 0.2,
                    "{gain_db} dB, bin {bin}: {} vs {}",
                    loud[bin],
                    quiet[bin]
                );
            }
        }
    }

    #[test]
    fn a_resonance_reads_as_positive_excess() {
        let mut power = vec![0.01; 2048];
        power[600] = 1.0;
        let out = excess(&power, 1.0);

        assert!(out[600] > 6.0, "the peak reads {} dB", out[600]);
        assert!(out[100] < 0.5, "a quiet bin reads {} dB", out[100]);
    }

    #[test]
    fn silence_does_not_produce_a_ratio_of_noise() {
        let out = excess(&vec![0.0; 512], 1.0);
        for value in &out {
            assert!(value.abs() < 0.01, "{value} dB out of silence");
            assert!(value.is_finite());
        }
    }

    #[test]
    fn power_is_the_mean_across_channels() {
        let left = vec![Complex::new(2.0_f32, 0.0); 8];
        let right = vec![Complex::new(0.0_f32, 0.0); 8];
        let mut out = vec![0.0; 8];
        power_into(&[&left, &right], &mut out);
        for value in &out {
            assert!((value - 2.0).abs() < 1e-6, "{value}");
        }
    }
}
