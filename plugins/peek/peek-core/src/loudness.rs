//! Loudness after ITU-R BS.1770 and EBU R 128: momentary (400 ms),
//! short-term (3 s), integrated (gated) and the loudness range.
//!
//! **Allocation-free**, so the gated averages are kept as histograms rather
//! than as lists of blocks: a block's energy is added to the bin its loudness
//! falls in, and a gate is a sum over the bins above it. The bins are 0.1 LU
//! wide, finer than anything the readings print.
//!
//! The K-weighting is the usual pair of biquads designed for the running rate
//! (a shelf near 1.5 kHz and a high-pass at 38 Hz), not the 48 kHz
//! coefficients printed in the standard. The tests check the readings the
//! standard defines.

/// A loudness that has not been measured yet.
pub const SILENT: f32 = f32::NEG_INFINITY;

const HOP_SECONDS: f32 = 0.1;
/// Momentary is four hops, short-term thirty.
const MOMENTARY_HOPS: usize = 4;
const SHORT_TERM_HOPS: usize = 30;
const ABSOLUTE_GATE: f32 = -70.0;
const RELATIVE_GATE: f32 = -10.0;
const RANGE_GATE: f32 = -20.0;
const LOWEST: f32 = -70.0;
const BIN_LU: f32 = 0.1;
const BINS: usize = 800; // -70 to +10

pub struct Loudness {
    weighting: [[Biquad; 2]; 2],
    /// Weighted energy of the hop in progress, per channel.
    sums: [f64; 2],
    counted: usize,
    hop: usize,
    /// The last thirty hops' energy, both channels summed.
    hops: [f64; SHORT_TERM_HOPS],
    next: usize,
    seen: usize,
    momentary: f32,
    short_term: f32,
    max_momentary: f32,
    blocks: Histogram,
    short_terms: Histogram,
}

impl Loudness {
    pub fn new(sample_rate: f32) -> Self {
        let channel = || {
            [
                Biquad::shelf(f64::from(sample_rate)),
                Biquad::high_pass(f64::from(sample_rate)),
            ]
        };
        Self {
            weighting: [channel(), channel()],
            sums: [0.0; 2],
            counted: 0,
            hop: (HOP_SECONDS * sample_rate) as usize,
            hops: [0.0; SHORT_TERM_HOPS],
            next: 0,
            seen: 0,
            momentary: SILENT,
            short_term: SILENT,
            max_momentary: SILENT,
            blocks: Histogram::new(),
            short_terms: Histogram::new(),
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        for (channel, sample) in [left, right].into_iter().enumerate() {
            let sample = if sample.is_finite() {
                f64::from(sample)
            } else {
                0.0
            };
            let [shelf, high_pass] = &mut self.weighting[channel];
            let weighted = high_pass.process(shelf.process(sample));
            self.sums[channel] += weighted * weighted;
        }
        self.counted += 1;
        if self.counted >= self.hop {
            self.end_hop();
        }
    }

    fn end_hop(&mut self) {
        let energy = (self.sums[0] + self.sums[1]) / self.counted as f64;
        self.sums = [0.0; 2];
        self.counted = 0;
        self.hops[self.next] = energy;
        self.next = (self.next + 1) % SHORT_TERM_HOPS;
        self.seen += 1;

        if self.seen >= MOMENTARY_HOPS {
            let block = self.mean_of_last(MOMENTARY_HOPS);
            self.momentary = loudness(block);
            self.max_momentary = self.max_momentary.max(self.momentary);
            self.blocks.add(self.momentary, block);
        }
        if self.seen >= SHORT_TERM_HOPS {
            let block = self.mean_of_last(SHORT_TERM_HOPS);
            self.short_term = loudness(block);
            self.short_terms.add(self.short_term, block);
        }
    }

    fn mean_of_last(&self, count: usize) -> f64 {
        (1..=count)
            .map(|back| self.hops[(self.next + SHORT_TERM_HOPS - back) % SHORT_TERM_HOPS])
            .sum::<f64>()
            / count as f64
    }

    /// In LUFS; [`SILENT`] until there is a reading.
    pub fn momentary(&self) -> f32 {
        self.momentary
    }

    pub fn short_term(&self) -> f32 {
        self.short_term
    }

    pub fn max_momentary(&self) -> f32 {
        self.max_momentary
    }

    /// Gated at −70 LUFS, then at 10 LU under the result.
    pub fn integrated(&self) -> f32 {
        let ungated = self.blocks.mean_energy_above(ABSOLUTE_GATE);
        if ungated <= 0.0 {
            return SILENT;
        }
        let relative = loudness(ungated) + RELATIVE_GATE;
        let gated = self.blocks.mean_energy_above(relative.max(ABSOLUTE_GATE));
        loudness(gated)
    }

    /// The spread between the 10th and 95th percentile of the short-term
    /// readings, gated 20 LU under their mean (EBU Tech 3342). In LU.
    pub fn range(&self) -> f32 {
        let ungated = self.short_terms.mean_energy_above(ABSOLUTE_GATE);
        if ungated <= 0.0 {
            return 0.0;
        }
        let gate = (loudness(ungated) + RANGE_GATE).max(ABSOLUTE_GATE);
        match (
            self.short_terms.percentile_above(gate, 0.10),
            self.short_terms.percentile_above(gate, 0.95),
        ) {
            (Some(low), Some(high)) => high - low,
            _ => 0.0,
        }
    }

    /// Forgets the integrated reading, the range and the maximum. The moving
    /// readings keep going.
    pub fn clear(&mut self) {
        self.blocks = Histogram::new();
        self.short_terms = Histogram::new();
        self.max_momentary = SILENT;
    }
}

fn loudness(energy: f64) -> f32 {
    if energy <= 0.0 {
        SILENT
    } else {
        (-0.691 + 10.0 * energy.log10()) as f32
    }
}

struct Histogram {
    counts: [u32; BINS],
    energies: [f64; BINS],
}

impl Histogram {
    fn new() -> Self {
        Self {
            counts: [0; BINS],
            energies: [0.0; BINS],
        }
    }

    fn bin(lufs: f32) -> Option<usize> {
        (lufs >= LOWEST).then(|| (((lufs - LOWEST) / BIN_LU) as usize).min(BINS - 1))
    }

    fn add(&mut self, lufs: f32, energy: f64) {
        if let Some(bin) = Self::bin(lufs) {
            self.counts[bin] += 1;
            self.energies[bin] += energy;
        }
    }

    /// The mean energy of the blocks louder than `gate`; 0 when there are
    /// none.
    fn mean_energy_above(&self, gate: f32) -> f64 {
        let from = Self::bin(gate).unwrap_or(0);
        let count: u64 = self.counts[from..]
            .iter()
            .map(|&count| u64::from(count))
            .sum();
        if count == 0 {
            return 0.0;
        }
        self.energies[from..].iter().sum::<f64>() / count as f64
    }

    /// The loudness `fraction` of the way up the blocks louder than `gate`.
    fn percentile_above(&self, gate: f32, fraction: f32) -> Option<f32> {
        let from = Self::bin(gate).unwrap_or(0);
        let count: u64 = self.counts[from..]
            .iter()
            .map(|&count| u64::from(count))
            .sum();
        if count == 0 {
            return None;
        }
        let wanted = (fraction * (count - 1) as f32).round() as u64;
        let mut passed = 0;
        for (offset, &bin_count) in self.counts[from..].iter().enumerate() {
            passed += u64::from(bin_count);
            if passed > wanted {
                return Some(LOWEST + (from + offset) as f32 * BIN_LU + BIN_LU * 0.5);
            }
        }
        None
    }
}

/// A direct-form-I biquad in `f64`: the high-pass sits at 38 Hz, where `f32`
/// state loses the low end.
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b: b.map(|value| value / a[0]),
            a: [a[1] / a[0], a[2] / a[0]],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    /// The K-weighting's first stage: +4 dB above about 1.5 kHz.
    fn shelf(sample_rate: f64) -> Self {
        let gain = 10f64.powf(4.0 / 40.0);
        let w0 = std::f64::consts::TAU * 1_500.0 / sample_rate;
        let alpha = w0.sin() / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
        let (cos, root) = (w0.cos(), 2.0 * gain.sqrt() * alpha);
        Self::new(
            [
                gain * ((gain + 1.0) + (gain - 1.0) * cos + root),
                -2.0 * gain * ((gain - 1.0) + (gain + 1.0) * cos),
                gain * ((gain + 1.0) + (gain - 1.0) * cos - root),
            ],
            [
                (gain + 1.0) - (gain - 1.0) * cos + root,
                2.0 * ((gain - 1.0) - (gain + 1.0) * cos),
                (gain + 1.0) - (gain - 1.0) * cos - root,
            ],
        )
    }

    /// The second stage: a high-pass at 38 Hz.
    fn high_pass(sample_rate: f64) -> Self {
        let w0 = std::f64::consts::TAU * 38.0 / sample_rate;
        let alpha = w0.sin() / (2.0 * 0.5);
        let cos = w0.cos();
        Self::new(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    fn process(&mut self, input: f64) -> f64 {
        let output = self.b[0] * input + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [input, self.x[0]];
        self.y = [output, self.y[0]];
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    const SR: f32 = 48_000.0;

    fn run(meter: &mut Loudness, seconds: f32, left: f32, right: f32) {
        for index in 0..(seconds * SR) as usize {
            let sine = (TAU * 997.0 * index as f32 / SR).sin();
            meter.push(sine * left, sine * right);
        }
    }

    /// The definition: a 0 dBFS sine at 1 kHz on one channel is −3.01 LUFS.
    #[test]
    fn a_full_scale_sine_on_one_side_is_minus_three() {
        let mut meter = Loudness::new(SR);
        run(&mut meter, 4.0, 1.0, 0.0);
        for reading in [meter.momentary(), meter.short_term(), meter.integrated()] {
            assert!((reading + 3.01).abs() < 0.1, "{reading}");
        }
    }

    /// EBU Tech 3341 case 1: −23 dBFS on both channels reads −23 LUFS.
    #[test]
    fn minus_twenty_three_on_both_sides_is_minus_twenty_three() {
        let mut meter = Loudness::new(SR);
        let amplitude = 10f32.powf(-23.0 / 20.0);
        run(&mut meter, 20.0, amplitude, amplitude);
        assert!(
            (meter.integrated() + 23.0).abs() < 0.1,
            "{}",
            meter.integrated()
        );
    }

    /// The relative gate: quiet passages under it do not drag the reading
    /// down (Tech 3341 case 3 in spirit: −36 and −23 dBFS).
    #[test]
    fn the_gate_ignores_the_quiet_part() {
        let mut meter = Loudness::new(SR);
        let level = |db: f32| 10f32.powf(db / 20.0);
        run(&mut meter, 10.0, level(-36.0), level(-36.0));
        run(&mut meter, 60.0, level(-23.0), level(-23.0));
        run(&mut meter, 10.0, level(-36.0), level(-36.0));
        assert!(
            (meter.integrated() + 23.0).abs() < 0.2,
            "{}",
            meter.integrated()
        );
    }

    /// Two steady levels 10 dB apart give a range near 10 LU (Tech 3342
    /// case 1 is −20 then −30 dBFS: 10 ± 1 LU).
    #[test]
    fn two_levels_ten_apart_have_a_range_of_ten() {
        let mut meter = Loudness::new(SR);
        let level = |db: f32| 10f32.powf(db / 20.0);
        run(&mut meter, 20.0, level(-20.0), level(-20.0));
        run(&mut meter, 20.0, level(-30.0), level(-30.0));
        assert!((meter.range() - 10.0).abs() < 1.0, "{}", meter.range());
    }

    #[test]
    fn silence_is_silent() {
        let mut meter = Loudness::new(SR);
        run(&mut meter, 1.0, 0.0, 0.0);
        assert_eq!(meter.integrated(), SILENT);
        assert_eq!(meter.range(), 0.0);
    }
}
