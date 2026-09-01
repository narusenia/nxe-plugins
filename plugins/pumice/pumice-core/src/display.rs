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

/// How many bins a grid step has to span before the point is an average of
/// them, and below how many it is read between them.
///
/// **Continuous, and blended between the two** (`PUM-10e`). The first version
/// decided with `high - low >= 2` on the *rounded* bin indices, and those flip
/// between one and two as the grid moves across bin boundaries — so the two
/// behaviours alternated point by point and the curve grew **spikes**. The
/// span in bins is monotone in frequency, so it crosses over exactly once.
const NARROW_SPAN: f32 = 1.0;
const WIDE_SPAN: f32 = 3.0;

/// `10·log10(x)` through the `log2` the hardware has.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;

/// The frequency of one grid point.
pub fn point_hz(index: usize) -> f32 {
    let position = index as f32 / (CURVE_POINTS - 1) as f32;
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(position)
}

/// Puts a per-bin curve onto the grid.
///
/// **Two behaviours, and the axis needs both.** A logarithmic grid over a
/// linear transform is coarser than the bins at the top and finer than them at
/// the bottom:
///
/// - **Above** the crossover a grid step spans many bins, so the point is their
///   **average**. Reading one of them would alias — the drawn spectrum would
///   flicker between neighbouring partials as the pitch moved.
/// - **Below** it several grid points fall inside one bin, so the point is
///   **interpolated** between the two nearest. Averaging there returns the same
///   bin to each of them and the curve climbs in **visible stairs** — which is
///   what it did (`PUM-10e`, seen in a host; at 100 Hz the grid steps 5.6 Hz
///   against a bin every 23.4).
///
/// The weight curve had the same stairs for the same reason and was fixed a
/// different way — it is a formula, so the window samples it directly
/// (`nodes::weight_at`). A spectrum is a measurement and has nothing to sample.
///
/// `bin_hz` is the spacing. Points past either end take the nearest bin rather
/// than nothing, so the axis is drawn to its edges.
pub fn resample_into(values: &[f32], bin_hz: f32, out: &mut [f32; CURVE_POINTS]) {
    if values.is_empty() || bin_hz <= 0.0 {
        out.fill(0.0);
        return;
    }
    let last = values.len() - 1;
    // Half a step either side, so consecutive points tile the axis without
    // overlapping or leaving holes.
    let step = (HIGH_HZ / LOW_HZ).powf(0.5 / (CURVE_POINTS - 1) as f32);

    // How wide one grid step is, measured in bins. **Continuous in the
    // frequency**, which is what makes the crossover happen once.
    let span_of = |centre: f32| centre * (step - 1.0 / step) / bin_hz;

    for (index, slot) in out.iter_mut().enumerate() {
        let centre = point_hz(index);
        let span = span_of(centre);

        // **Never bin zero.** DC is not a frequency anybody reads off a
        // spectrum, and it is the one bin whose contents mean something else —
        // a block's offset rather than a level.
        let first = 1.min(last);

        let interpolated = {
            let position = (centre / bin_hz).clamp(first as f32, last as f32);
            let left = position.floor() as usize;
            let right = (left + 1).min(last);
            let fraction = position - left as f32;
            values[left] + (values[right] - values[left]) * fraction
        };

        *slot = if span <= NARROW_SPAN {
            interpolated
        } else {
            let low = (((centre / step) / bin_hz).floor().max(0.0) as usize).max(first);
            let high = (((centre * step) / bin_hz).ceil() as usize).clamp(low, last);
            let total: f32 = values[low..=high].iter().sum();
            let averaged = total / (high - low + 1) as f32;

            if span >= WIDE_SPAN {
                averaged
            } else {
                // **Blended across the crossover.** Switching outright puts a
                // step where the two disagree, and a step in a drawn spectrum
                // is a spike.
                let t = (span - NARROW_SPAN) / (WIDE_SPAN - NARROW_SPAN);
                interpolated + (averaged - interpolated) * t
            }
        };
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

    /// **No stairs at the bottom of the axis** (`PUM-10e`). Below about 300 Hz
    /// several grid points fall inside one bin, and averaging handed each of
    /// them the same value.
    #[test]
    fn the_bottom_of_the_axis_does_not_step() {
        // A ramp across the bins: whatever the grid does to it has to stay
        // strictly rising, with no point sitting still beside its neighbour.
        let values: Vec<f32> = (0..1025).map(|bin| bin as f32).collect();
        let mut out = [0.0; CURVE_POINTS];
        resample_into(&values, 23.437_5, &mut out);

        // **From the first point above bin one.** The axis starts at 20 Hz and
        // the first usable bin is at 23.4, so the handful of points below it
        // all read that bin — which is the axis running out of transform, not
        // the grid stepping.
        let first = (0..CURVE_POINTS)
            .find(|index| point_hz(*index) > 23.437_5)
            .expect("the axis reaches the first bin");
        for index in (first + 1)..CURVE_POINTS {
            assert!(
                out[index] > out[index - 1],
                "point {index} sits at {} beside {}",
                out[index],
                out[index - 1]
            );
        }
    }

    /// **No spikes anywhere on a smooth input** (`PUM-10e`). The first fix
    /// chose between interpolating and averaging on rounded bin indices, which
    /// flip back and forth across bin boundaries — so the curve grew a thorn
    /// wherever the choice changed.
    #[test]
    fn a_smooth_input_stays_smooth() {
        // A curve with a shape, so the test is not passed by a straight line:
        // a broad hump on a slope.
        let values: Vec<f32> = (0..1025)
            .map(|bin| {
                let hz = bin as f32 * 23.437_5;
                let hump = (-((hz / 2_500.0).log2()).powi(2)).exp2();
                20.0 + hz * 0.002 + 30.0 * hump
            })
            .collect();

        let mut out = [0.0; CURVE_POINTS];
        resample_into(&values, 23.437_5, &mut out);

        // Every point within a whisker of the line between its neighbours.
        // A thorn is exactly a point that is not.
        for index in 1..CURVE_POINTS - 1 {
            let between = (out[index - 1] + out[index + 1]) * 0.5;
            let span = (out[index + 1] - out[index - 1]).abs().max(0.5);
            assert!(
                (out[index] - between).abs() < span,
                "point {index} at {} spikes off the line through {} and {}",
                out[index],
                out[index - 1],
                out[index + 1]
            );
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
