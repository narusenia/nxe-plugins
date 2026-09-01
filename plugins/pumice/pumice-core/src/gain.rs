//! From "how far above its neighbours" to "how much to take out".
//!
//! ## The follower is per bin, the coefficients are not
//!
//! `nxe_audio::envelope::Power` holds a state **and** its two coefficients.
//! There are up to 8193 bins and they all move at the same speed, so using it
//! here would store the same pair of numbers eight thousand times. What is
//! per-bin is the state; the arithmetic is the same one-pole.
//!
//! **The clock is the hop, not the sample.** A bin is only looked at once per
//! frame, so a time constant in seconds becomes a coefficient against
//! `sample_rate / hop`. Getting this wrong scales every time constant by four.
//!
//! ## Why the gain curve is smoothed, and why that is not a control
//!
//! An independent gain per bin makes the reconstruction warble — the artefact
//! that separates "sounds like an EQ" from "sounds like a phone". A fixed
//! floor of smoothing in the frequency direction is what stops it, so it is a
//! constant here rather than the wide end of `SHARPNESS` (`REQ-PUM-005`).
//! **Smoothed in dB**: averaging linear gains would make a deep narrow cut
//! shallower than a shallow wide one of the same area.

use crate::smoothing::Prefix;

/// A gain in dB is `2^(dB / (20·log10(2)))` — the constant
/// `nxe_audio::guard` keeps, for the same reason.
const DECIBELS_PER_OCTAVE_AMPLITUDE: f32 = 6.020_6;

/// The one-pole state of every bin, and the two coefficients they share.
pub struct Follower {
    state: Vec<f32>,
    attack: f32,
    release: f32,
}

impl Follower {
    pub fn new(max_bins: usize) -> Self {
        Self {
            state: vec![0.0; max_bins],
            attack: 1.0,
            release: 1.0,
        }
    }

    /// The same time constant both ways — what a long-term average wants.
    ///
    /// **Reusing this rather than writing a mean.** A symmetric one-pole *is*
    /// an exponential moving average, and a second type holding the same state
    /// for the same arithmetic is the thing `protect.rs` warns about.
    pub fn set_symmetric(&mut self, seconds: f32, frame_rate: f32) {
        self.set(seconds, seconds, frame_rate);
    }

    /// `frame_rate` is `sample_rate / hop`, not the sample rate.
    pub fn set(&mut self, attack_seconds: f32, release_seconds: f32, frame_rate: f32) {
        self.attack = nxe_audio::envelope::coefficient(attack_seconds, frame_rate);
        self.release = nxe_audio::envelope::coefficient(release_seconds, frame_rate);
    }

    pub fn reset(&mut self) {
        self.state.fill(0.0);
    }

    /// Rises at `attack`, falls at `release`, in place.
    pub fn follow(&mut self, input: &[f32], out: &mut [f32]) {
        for (bin, value) in input.iter().enumerate().take(out.len()) {
            let previous = self.state[bin];
            let coefficient = if *value > previous {
                self.attack
            } else {
                self.release
            };
            // A non-finite bin must not latch the follower for good
            // (`REQ-PUM-016`).
            let next = if value.is_finite() {
                previous + coefficient * (value - previous)
            } else {
                previous
            };
            self.state[bin] = next;
            out[bin] = next;
        }
    }
}

/// What the caller wants taken out, before the frequency-direction smoothing.
///
/// `weight` is `REQ-PUM-004`'s node curve times the operating range; at this
/// unit it is the range alone.
#[derive(Clone, Copy, Debug)]
pub struct Computer {
    pub threshold_db: f32,
    pub slope: f32,
    pub ceiling_db: f32,
}

impl Computer {
    /// **`DEPTH` multiplies last**, which is what makes `DEPTH` = 0 exactly
    /// nothing however the nodes are set (`REQ-PUM-002`).
    pub fn reduction_db_into(&self, drive_db: &[f32], weight: &[f32], depth: f32, out: &mut [f32]) {
        for (bin, value) in out.iter_mut().enumerate() {
            let excess = (drive_db[bin] - self.threshold_db).max(0.0);
            let reduction = (self.slope * excess).min(self.ceiling_db);
            *value = -reduction * weight[bin] * depth;
        }
    }
}

/// Smooths a dB curve across `width_octaves` and converts it to linear gains.
pub fn smooth_into(
    reduction_db: &[f32],
    prefix: &mut Prefix,
    smoothed_db: &mut [f32],
    width_octaves: f32,
    out: &mut [f32],
) {
    prefix.build(reduction_db);
    prefix.average_into(width_octaves, smoothed_db);

    for (bin, value) in out.iter_mut().enumerate() {
        *value = (smoothed_db[bin] / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2();
    }
}

/// The band the plugin is allowed to work in, as a per-bin weight.
///
/// **Raised cosine, half an octave wide, centred on each edge.** A step would
/// switch the reduction on and off at the boundary, and that is audible as an
/// edge rather than as a limit.
///
/// **At the low edge the bins are what limit it.** Half an octave around
/// 100 Hz is 71–141 Hz, three bins wide at 48 kHz and 2048 points, so the ramp
/// climbs in three visible steps however smooth the cosine is. The final gain
/// curve is smoothed again over 1/12 octave before it reaches the audio
/// ([`smooth_into`]), which is what actually decides whether that is heard.
/// If it ever is, the fix is a wider edge down there, not a finer transform.
pub fn range_into(
    bins: usize,
    bin_hz: f32,
    low_hz: f32,
    high_hz: f32,
    edge_octaves: f32,
    out: &mut [f32],
) {
    let half = edge_octaves * 0.5;
    for (bin, value) in out.iter_mut().enumerate().take(bins) {
        let hz = bin as f32 * bin_hz;
        *value = if hz <= 0.0 {
            0.0
        } else {
            let octaves_above_low = (hz / low_hz).log2();
            let octaves_below_high = (high_hz / hz).log2();
            ramp(octaves_above_low, half) * ramp(octaves_below_high, half)
        };
    }
}

/// Zero at `-half` octaves, one at `+half`, a raised cosine between.
fn ramp(octaves: f32, half: f32) -> f32 {
    if octaves <= -half {
        return 0.0;
    }
    if octaves >= half {
        return 1.0;
    }
    let position = (octaves + half) / (2.0 * half);
    0.5 - 0.5 * (std::f32::consts::PI * position).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPUTER: Computer = Computer {
        threshold_db: 0.0,
        slope: 0.7,
        ceiling_db: 18.0,
    };

    /// `REQ-PUM-002`: zero is exactly nothing, whatever else is set.
    #[test]
    fn depth_zero_is_exactly_nothing() {
        let drive = vec![30.0; 64];
        let weight = vec![1.0; 64];
        let mut out = vec![9.9; 64];
        COMPUTER.reduction_db_into(&drive, &weight, 0.0, &mut out);
        assert!(out.iter().all(|value| *value == 0.0));
    }

    /// `REQ-PUM-023`: the ceiling is a mechanism, not a taste setting.
    #[test]
    fn nothing_reaches_past_the_ceiling() {
        let drive = vec![120.0; 64];
        let weight = vec![2.0; 64];
        let mut out = vec![0.0; 64];
        COMPUTER.reduction_db_into(&drive, &weight, 1.0, &mut out);
        for value in &out {
            // Weight may double it — that is `REQ-PUM-004`'s node range — but
            // the per-band reduction before weighting cannot pass the ceiling.
            assert!(*value >= -COMPUTER.ceiling_db * 2.0 - 1e-3, "{value}");
        }
    }

    /// The `SPK-18` promise, at this layer: nothing above the threshold means
    /// nothing taken out.
    #[test]
    fn no_excess_takes_nothing_out() {
        let drive = vec![-3.0; 64];
        let weight = vec![1.0; 64];
        let mut out = vec![9.9; 64];
        COMPUTER.reduction_db_into(&drive, &weight, 1.0, &mut out);
        assert!(out.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn a_negative_weight_gives_back_reduction() {
        let drive = vec![10.0; 4];
        let mut out = vec![0.0; 4];
        COMPUTER.reduction_db_into(&drive, &[1.0, 0.5, 0.0, -1.0], 1.0, &mut out);
        assert!(out[0] < out[1] && out[1] < out[2]);
        assert_eq!(out[2], 0.0);
        assert!(
            out[3] > 0.0,
            "a protecting node should lift, got {}",
            out[3]
        );
    }

    #[test]
    fn a_flat_curve_of_zero_smooths_to_unity() {
        let mut prefix = Prefix::new(256);
        let mut smoothed = vec![0.0; 256];
        let mut out = vec![0.0; 256];
        smooth_into(&vec![0.0; 256], &mut prefix, &mut smoothed, 0.083, &mut out);
        for value in &out {
            assert!((value - 1.0).abs() < 1e-6, "{value}");
        }
    }

    #[test]
    fn six_decibels_of_cut_is_half_the_amplitude() {
        let mut prefix = Prefix::new(64);
        let mut smoothed = vec![0.0; 64];
        let mut out = vec![0.0; 64];
        smooth_into(
            &vec![-6.0206; 64],
            &mut prefix,
            &mut smoothed,
            0.083,
            &mut out,
        );
        for value in &out {
            assert!((value - 0.5).abs() < 1e-4, "{value}");
        }
    }

    #[test]
    fn the_range_is_one_inside_and_zero_outside() {
        let mut out = vec![0.0; 1025];
        // 48 kHz, 2048-point transform.
        range_into(1025, 23.4375, 100.0, 18_000.0, 0.5, &mut out);

        let at = |hz: f32| out[(hz / 23.4375).round() as usize];
        assert!(at(1000.0) > 0.999, "mid band is {}", at(1000.0));
        assert!(at(60.0) < 0.001, "below the range is {}", at(60.0));
        assert_eq!(out[0], 0.0);
        // Half an octave centred on the edge: down at 100/√2, up at 100·√2.
        assert!(at(71.0) < 0.01 && at(142.0) > 0.99);
    }

    /// The range must rise once and fall once, with nothing jagged in between.
    ///
    /// **Not "every step is small".** Half an octave at the low edge is
    /// 71–141 Hz, which is three bins wide at 48 kHz and 2048 points — the
    /// ramp cannot be smoother than the bins that sample it, and the first
    /// version of this test asserted 0.2 where 0.23 is what one bin of that
    /// ramp is worth. Above the low edge the bins are dense and the curve is
    /// as smooth as the cosine.
    #[test]
    fn the_range_rises_once_and_falls_once() {
        let mut out = vec![0.0; 1025];
        range_into(1025, 23.4375, 100.0, 18_000.0, 0.5, &mut out);

        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(bin, _)| bin)
            .unwrap();

        for bin in 1..peak {
            assert!(out[bin] >= out[bin - 1] - 1e-6, "dip at bin {bin}");
        }
        for bin in (peak + 1)..1025 {
            assert!(out[bin] <= out[bin - 1] + 1e-6, "bump at bin {bin}");
        }

        // Where the bins resolve the ramp, it is smooth.
        for bin in 8..1025 {
            assert!(
                (out[bin] - out[bin - 1]).abs() < 0.05,
                "step at bin {bin}: {} to {}",
                out[bin - 1],
                out[bin]
            );
        }
    }

    #[test]
    fn a_non_finite_bin_does_not_latch_the_follower() {
        let mut follower = Follower::new(4);
        follower.set(0.01, 0.1, 100.0);
        let mut out = vec![0.0; 4];
        follower.follow(&[f32::NAN, f32::INFINITY, 1.0, 0.0], &mut out);
        assert!(out.iter().all(|value| value.is_finite()), "{out:?}");

        for _ in 0..500 {
            follower.follow(&[0.0; 4], &mut out);
        }
        assert!(out.iter().all(|value| value.abs() < 1e-3), "{out:?}");
    }
}
