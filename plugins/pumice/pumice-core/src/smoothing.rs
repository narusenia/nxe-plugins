//! Averaging a spectrum over a **width in octaves**, at any width, for the
//! price of one pass.
//!
//! Three things in Pumice need this and they need different widths: the
//! reference a bin is judged against (`SHARPNESS`), the long-term map of where
//! resonance lives (`PUM-4`), and the final gain curve — which has to be
//! smoothed or the reconstruction warbles (`REQ-PUM-005`).
//!
//! ## Why a prefix sum
//!
//! The window is **wider at high bins than at low ones**, because a fixed span
//! in octaves is a fixed *ratio*. A direct implementation is therefore
//! `O(bins × width)` and gets slower as the width grows — which would make
//! `SHARPNESS` a CPU control. One prefix sum makes every interval two lookups,
//! so the cost is `O(bins)` whatever the width is.
//!
//! ## Why the bounds are not cached
//!
//! `lo` and `hi` are `k · 2^(∓w/2)`, and the two factors are computed once per
//! call. Per bin that is two multiplications and two roundings — cheaper than
//! reading a cached pair out of memory, and it means a swept `SHARPNESS` costs
//! nothing extra.

/// Below this an interval is treated as silence rather than divided by.
///
/// Without it, the ratio of two numbers made of noise floor decides whether the
/// plugin works (`nxe_audio::guard` states the same rule for the same reason).
pub const FLOOR: f32 = 1e-10;

/// Running sums of one spectrum, ready to be averaged at any width.
pub struct Prefix {
    /// `sums[i]` is the total of the first `i` values, so `sums[0]` is zero and
    /// the length is one more than the spectrum's.
    sums: Vec<f64>,
    bins: usize,
}

impl Prefix {
    pub fn new(max_bins: usize) -> Self {
        Self {
            sums: vec![0.0; max_bins + 1],
            bins: 0,
        }
    }

    /// **`f64`, and that is not caution.** A prefix sum over 8193 bins of power
    /// accumulates every value into every later entry, and an interval is the
    /// *difference* of two large numbers. In `f32` the difference of two sums
    /// near the total loses most of its significant digits, which shows up as a
    /// reference that is wrong exactly where the signal is loudest.
    pub fn build(&mut self, values: &[f32]) {
        self.bins = values.len();
        self.sums[0] = 0.0;
        let mut total = 0.0_f64;
        for (index, value) in values.iter().enumerate() {
            total += f64::from(*value);
            self.sums[index + 1] = total;
        }
    }

    /// The mean of each bin's neighbourhood, `width_octaves` wide and centred
    /// on the bin.
    ///
    /// **Bin zero has no neighbourhood** — a ratio around DC is meaningless and
    /// the operating range starts far above it — so it takes its own value and
    /// reads as no excess at all.
    pub fn average_into(&self, width_octaves: f32, out: &mut [f32]) {
        let bins = self.bins.min(out.len());
        if bins == 0 {
            return;
        }

        let lower = (-0.5 * width_octaves).exp2();
        let upper = (0.5 * width_octaves).exp2();

        let sums = &self.sums;
        out[0] = interval(sums, 0, 1);
        for (bin, value) in out.iter_mut().enumerate().take(bins).skip(1) {
            let low = ((bin as f32 * lower) as usize).min(bins - 1);
            let high = ((bin as f32 * upper).ceil() as usize).clamp(low + 1, bins);
            *value = interval(sums, low, high);
        }
    }

    /// The mean of a **ring** around each bin: everything between
    /// `inner_octaves` and `outer_octaves` away, on both sides, and nothing
    /// closer.
    ///
    /// **The hole is the point** (`PUM-4`). A reference that includes the bin
    /// it judges is inflated by whatever it is supposed to be measuring —
    /// `SPK-18` is the same mistake, where lifting a band by 10 dB lifted its
    /// own reference by 6, and ten decibels of harshness read as two. Measured
    /// here before the hole existed: an 18 dB resonance read as 8.2 dB of
    /// excess at the narrow end of `SHARPNESS`, and **narrowing made it
    /// worse** — a narrower window is more completely filled by the peak, so
    /// the control that should have made detection sharper made it blunter.
    ///
    /// A bin whose ring runs off the end of the spectrum keeps the side it
    /// has; a bin with no ring at all takes its own value, which reads as no
    /// excess rather than as a division by nothing.
    pub fn ring_average_into(&self, inner_octaves: f32, outer_octaves: f32, out: &mut [f32]) {
        let bins = self.bins.min(out.len());
        if bins == 0 {
            return;
        }

        let inner_low = (-0.5 * inner_octaves).exp2();
        let inner_high = (0.5 * inner_octaves).exp2();
        let outer_low = (-0.5 * outer_octaves).exp2();
        let outer_high = (0.5 * outer_octaves).exp2();

        let sums = &self.sums;
        out[0] = interval(sums, 0, 1);
        for (bin, value) in out.iter_mut().enumerate().take(bins).skip(1) {
            let position = bin as f32;
            let below_start = ((position * outer_low) as usize).min(bins);
            let below_end = ((position * inner_low) as usize).clamp(below_start, bins);
            let above_start = ((position * inner_high).ceil() as usize).min(bins);
            let above_end = ((position * outer_high).ceil() as usize).clamp(above_start, bins);

            let count = (below_end - below_start) + (above_end - above_start);
            *value = if count == 0 {
                interval(sums, bin, bin + 1)
            } else {
                let total =
                    (sums[below_end] - sums[below_start]) + (sums[above_end] - sums[above_start]);
                (total / count as f64) as f32
            };
        }
    }
}

/// **The subtraction stays in `f64`.** Narrowing either sum first is exactly
/// the precision this type exists to keep.
fn interval(sums: &[f64], low: usize, high: usize) -> f32 {
    let total = sums[high] - sums[low];
    (total / (high - low) as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flat_spectrum_averages_to_itself() {
        let values = vec![0.25; 512];
        let mut prefix = Prefix::new(512);
        prefix.build(&values);

        let mut out = vec![0.0; 512];
        for width in [0.1, 0.5, 1.0, 3.0] {
            prefix.average_into(width, &mut out);
            for (bin, value) in out.iter().enumerate() {
                assert!(
                    (value - 0.25).abs() < 1e-6,
                    "width {width}, bin {bin}: {value}"
                );
            }
        }
    }

    /// A lone spike must be diluted by roughly the width of the window, which
    /// is what makes it *look* like an excess rather than like the level.
    #[test]
    fn a_spike_is_diluted_by_the_window() {
        let mut values = vec![0.0; 1024];
        values[512] = 1.0;

        let mut prefix = Prefix::new(1024);
        prefix.build(&values);
        let mut narrow = vec![0.0; 1024];
        let mut wide = vec![0.0; 1024];
        prefix.average_into(0.2, &mut narrow);
        prefix.average_into(2.0, &mut wide);

        assert!(
            wide[512] < narrow[512],
            "wide {} should dilute more than narrow {}",
            wide[512],
            narrow[512]
        );
    }

    /// The interval must widen with the bin index — this is the whole reason
    /// the average is taken in octaves rather than in bins.
    #[test]
    fn the_window_widens_with_frequency() {
        let mut values = vec![0.0; 2048];
        values[100] = 1.0;
        values[1000] = 1.0;

        let mut prefix = Prefix::new(2048);
        prefix.build(&values);
        let mut out = vec![0.0; 2048];
        prefix.average_into(1.0, &mut out);

        assert!(
            out[1000] < out[100],
            "bin 1000 ({}) should sit in a wider window than bin 100 ({})",
            out[1000],
            out[100]
        );
    }

    /// **The `f64` accumulator earns its place.** At width zero every window is
    /// one bin, so each output must be its input *exactly* — and that is the
    /// difference of two running totals near four million. In `f32` those two
    /// are 0.25 apart at best, so a bin differing by a thousandth would come
    /// back as noise.
    ///
    /// This test replaced one that set a narrow-but-nonzero width and asserted
    /// the odd bin still stood out. It did not: a window 139 bins wide dilutes
    /// a thousandth to seven millionths, which is below `f32`'s resolution at
    /// 1000 whatever the accumulator is. **That was dilution working, not
    /// precision failing** — the first version of this test would have failed
    /// on a correct implementation.
    #[test]
    fn one_bin_intervals_are_exact_against_a_large_total() {
        let mut values = vec![1000.0_f32; 4096];
        values[4000] = 1000.001;
        values[7] = 1000.002;

        let mut prefix = Prefix::new(4096);
        prefix.build(&values);
        let mut out = vec![0.0; 4096];
        prefix.average_into(0.0, &mut out);

        for (bin, value) in out.iter().enumerate().skip(1) {
            assert_eq!(*value, values[bin], "bin {bin}");
        }
    }
}
