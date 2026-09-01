//! What the picture needs, on the axis the picture is drawn on.
//!
//! **Not bins.** A transform has up to 8193 of them and the window is 880
//! pixels wide, so handing over bins would publish eight thousand atomics a
//! block for a figure that can show a fraction of them. The figure's axis is
//! **logarithmic in frequency** (`ui.md`), and a fixed log-spaced grid is both
//! what it wants and two orders of magnitude less to carry.
//!
//! **Averaged, not sampled.** At 20 kHz one grid step spans dozens of bins; a
//! point that read a single bin would alias — the drawn spectrum would flicker
//! between neighbouring partials as the pitch moved, which is exactly the
//! shimmer `nxe_ui::dots` refuses to draw.
//!
//! ## Why this is in the core
//!
//! The same reason the engine is: the figure's contents are a property of the
//! processing, and **the curve the window draws has to be the gain the audio
//! got** (`REQ-PUM-018`). Computing it again in the wrapper is how a figure
//! starts telling a different story from the sound.

/// How many points the figure is given. 880 px over ten octaves is 88 px per
/// octave; this is a point every eight pixels or so, which is finer than a
/// stroke.
pub const CURVE_POINTS: usize = 128;

/// The bottom and the top of the drawn axis.
pub const LOW_HZ: f32 = 20.0;
pub const HIGH_HZ: f32 = 20_000.0;

/// Below this a point is treated as silence rather than converted to dB.
const FLOOR_DB: f32 = -120.0;

/// `10·log10(x)` through the `log2` the hardware has.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// The frequency of one grid point.
pub fn point_hz(index: usize) -> f32 {
    let position = index as f32 / (CURVE_POINTS - 1) as f32;
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(position)
}

/// Averages a per-bin curve onto the grid.
///
/// `bin_hz` is the spacing. Points below the first bin or above the last take
/// the nearest bin rather than nothing, so the ends of the axis are drawn
/// rather than left as a gap.
pub fn resample_into(values: &[f32], bin_hz: f32, out: &mut [f32; CURVE_POINTS]) {
    if values.is_empty() || bin_hz <= 0.0 {
        out.fill(0.0);
        return;
    }
    let last = values.len() - 1;

    for (index, slot) in out.iter_mut().enumerate() {
        // Half a step either side, so consecutive points tile the axis without
        // overlapping or leaving holes.
        let centre = point_hz(index);
        let step = (HIGH_HZ / LOW_HZ).powf(0.5 / (CURVE_POINTS - 1) as f32);
        let low = ((centre / step) / bin_hz).floor().max(0.0) as usize;
        let high = (((centre * step) / bin_hz).ceil() as usize).min(last);

        let (low, high) = if low > high {
            (high, high)
        } else {
            (low, high)
        };
        let count = high - low + 1;
        let total: f32 = values[low..=high].iter().sum();
        *slot = total / count as f32;
    }
}

/// The same, converting power to dB on the way.
/// What a transform's power has to be divided by to read as dBFS.
///
/// **A transform's magnitude scales with its size**, and this was missed: a
/// full-scale sine through a 2048-point Hann-windowed FFT peaks at `A·N/4`, so
/// its power read **+34 dB** rather than 0 and every drawn spectrum sat flat
/// against the top of the figure whatever was playing.
///
/// The detection never noticed, because it works on the *ratio* of a bin to its
/// neighbourhood and a scale cancels out of a ratio. Only the picture was
/// wrong — which is the kind of bug a figure hides until somebody plays
/// something through it.
pub fn full_scale(block: usize) -> f32 {
    let peak = block as f32 * 0.25;
    peak * peak
}

/// The same, converting power to dBFS on the way. `scale` is [`full_scale`].
pub fn resample_power_db_into(
    power: &[f32],
    bin_hz: f32,
    scale: f32,
    out: &mut [f32; CURVE_POINTS],
) {
    resample_into(power, bin_hz, out);
    let scale = scale.max(f32::MIN_POSITIVE);
    for value in out.iter_mut() {
        let level = DECIBELS_PER_OCTAVE_POWER * (*value / scale).max(f32::MIN_POSITIVE).log2();
        *value = level.max(FLOOR_DB);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_spans_the_drawn_axis() {
        assert!((point_hz(0) - LOW_HZ).abs() < 0.01);
        assert!((point_hz(CURVE_POINTS - 1) - HIGH_HZ).abs() < 1.0);

        // Evenly spaced in octaves, which is what makes it the figure's axis.
        let first = (point_hz(1) / point_hz(0)).log2();
        for index in 1..CURVE_POINTS {
            let step = (point_hz(index) / point_hz(index - 1)).log2();
            assert!((step - first).abs() < 1e-4, "step {index} is {step}");
        }
    }

    #[test]
    fn a_flat_curve_resamples_flat() {
        let values = vec![0.25_f32; 1025];
        let mut out = [0.0; CURVE_POINTS];
        resample_into(&values, 23.437_5, &mut out);
        for (index, value) in out.iter().enumerate() {
            assert!((value - 0.25).abs() < 1e-5, "point {index}: {value}");
        }
    }

    /// A peak has to land where it was put, on the axis the figure draws.
    #[test]
    fn a_peak_lands_at_its_own_frequency() {
        let mut values = vec![0.001_f32; 1025];
        let bin_hz = 23.437_5;
        let bin = (2_500.0_f32 / bin_hz).round() as usize;
        values[bin] = 1.0;

        let mut out = [0.0; CURVE_POINTS];
        resample_into(&values, bin_hz, &mut out);

        let loudest = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap();
        let hz = point_hz(loudest);
        assert!(
            (hz / 2_500.0).log2().abs() < 0.05,
            "the peak was drawn at {hz} Hz"
        );
    }

    /// **The high end must not alias.** One grid step at 20 kHz spans dozens of
    /// bins, and a point that read one of them would flicker.
    #[test]
    fn the_top_of_the_axis_averages_many_bins() {
        // Every other bin loud: a sampled point would read 0 or 1, an averaged
        // one reads the middle.
        let values: Vec<f32> = (0..1025)
            .map(|bin| if bin % 2 == 0 { 1.0 } else { 0.0 })
            .collect();
        let mut out = [0.0; CURVE_POINTS];
        resample_into(&values, 23.437_5, &mut out);

        let top = CURVE_POINTS - 8;
        for value in &out[top..] {
            assert!(
                (0.2..=0.8).contains(value),
                "a point near the top read {value}"
            );
        }
    }

    /// **The scale is the transform's, and it has to be divided out.**
    ///
    /// Without this the drawn spectrum read **+34 dB** at 2048 points and sat
    /// flat against the top of the figure whatever was playing — the detection
    /// never noticed, because it works on ratios and a scale cancels out of a
    /// ratio.
    ///
    /// Measured at [`full_scale`] rather than with a tone: the resampling
    /// *averages*, so a single loud bin is diluted by however many bins one
    /// grid step spans (12 dB at 12 kHz with a 1024-point transform). That
    /// dilution is the anti-aliasing working, and it would hide what this test
    /// is for.
    #[test]
    fn the_transforms_own_scale_is_divided_out() {
        for block in [1024_usize, 2048, 4096] {
            let bins = block / 2 + 1;
            let power = vec![full_scale(block); bins];

            let mut out = [0.0; CURVE_POINTS];
            resample_power_db_into(&power, 48_000.0 / block as f32, full_scale(block), &mut out);
            for (index, value) in out.iter().enumerate() {
                assert!(
                    value.abs() < 0.01,
                    "block {block}, point {index} reads {value:.2} dB"
                );
            }
        }
    }

    /// And a lone bin comes out *below* full scale, because the grid step it
    /// falls in is averaged. This is the behaviour, not a defect.
    #[test]
    fn a_lone_bin_is_diluted_by_its_grid_step() {
        let block = 2048_usize;
        let bins = block / 2 + 1;
        let mut power = vec![0.0_f32; bins];
        power[bins / 2] = full_scale(block);

        let mut out = [0.0; CURVE_POINTS];
        resample_power_db_into(&power, 48_000.0 / block as f32, full_scale(block), &mut out);
        let loudest = out.iter().fold(f32::MIN, |a, b| a.max(*b));
        assert!(
            (-30.0..-1.0).contains(&loudest),
            "a lone full-scale bin reads {loudest:.1} dB"
        );
    }

    #[test]
    fn silence_becomes_the_floor_rather_than_minus_infinity() {
        let mut out = [0.0; CURVE_POINTS];
        resample_power_db_into(&vec![0.0; 1025], 23.437_5, full_scale(2048), &mut out);
        for value in &out {
            assert_eq!(*value, FLOOR_DB);
        }
    }

    #[test]
    fn an_empty_curve_is_not_a_panic() {
        let mut out = [1.0; CURVE_POINTS];
        resample_into(&[], 23.437_5, &mut out);
        assert!(out.iter().all(|value| *value == 0.0));
    }
}
