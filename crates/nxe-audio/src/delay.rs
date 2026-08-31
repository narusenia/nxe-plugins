//! A fractional-delay ring buffer.
//!
//! One line holds one source channel. Doubler gives every voice its own read
//! position on a shared line (`plugins/doubler/docs/specifications/dsp.md`);
//! Diorama reads a fixed set of taps off one
//! (`plugins/diorama/docs/specifications/dsp.md`).
//!
//! **Written inside `doubler-core` and moved here when Diorama asked for
//! it** (`DIO-1`) — a shared module is created by the second caller, not in
//! anticipation of one (`docs/specifications/architecture.md`).
//!
//! Reads are interpolated with a 4-point Catmull-Rom (Hermite) kernel. Linear
//! interpolation is not an option here: a voice's read position is fractional
//! essentially all the time, so linear's high-frequency loss would be a
//! permanent part of the sound rather than an occasional artifact.

/// The number of extra slots the interpolator needs on top of the requested
/// maximum delay: one newer neighbour and two older ones, plus the slot the
/// write head is about to overwrite.
const INTERP_MARGIN: usize = 4;

/// The smallest delay a read can ask for. The interpolator needs one sample
/// *newer* than the read position, and the newest sample in the buffer sits at
/// a delay of one, so two is the floor.
const MIN_DELAY_SAMPLES: f32 = 2.0;

pub struct DelayLine {
    /// Always a power of two, so wrapping is a mask rather than a division.
    buffer: Vec<f32>,
    mask: usize,
    /// Index of the slot the next `write` will use.
    write: usize,
    max_delay_samples: f32,
}

impl DelayLine {
    /// Allocates a line that can hold `max_delay_seconds` at `sample_rate`.
    ///
    /// This is the only allocation. `write` and `read` never touch the heap,
    /// which is what lets them run on the audio thread
    /// (`.agents/rules/rust.md`).
    pub fn new(sample_rate: f32, max_delay_seconds: f32) -> Self {
        let max_delay_samples = (sample_rate * max_delay_seconds).max(MIN_DELAY_SAMPLES);
        let len = (max_delay_samples.ceil() as usize + INTERP_MARGIN).next_power_of_two();

        Self {
            buffer: vec![0.0; len],
            mask: len - 1,
            write: 0,
            max_delay_samples,
        }
    }

    /// The largest delay `read` will honour, in samples.
    pub fn max_delay_samples(&self) -> f32 {
        self.max_delay_samples
    }

    /// Clears the line without reallocating.
    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    /// Pushes one sample. It becomes readable at a delay of one sample.
    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write] = sample;
        self.write = (self.write + 1) & self.mask;
    }

    /// Reads at a fractional delay, in samples.
    ///
    /// `delay_samples` is clamped into the range the line can actually serve.
    /// A parameter-derived delay is host-controlled input, so clamping here is
    /// the guard that keeps `process()` from panicking (`REQ-DBL-011`).
    pub fn read(&self, delay_samples: f32) -> f32 {
        let delay = if delay_samples.is_nan() {
            MIN_DELAY_SAMPLES
        } else {
            delay_samples.clamp(MIN_DELAY_SAMPLES, self.max_delay_samples)
        };

        let whole = delay.floor();
        let t = delay - whole;
        let whole = whole as usize;

        // Ordered along increasing delay: `newer` is one sample toward the
        // present, `y0`..`y1` bracket the read position, `older` is past it.
        let newer = self.at(whole - 1);
        let y0 = self.at(whole);
        let y1 = self.at(whole + 1);
        let older = self.at(whole + 2);

        let a = -0.5 * newer + 1.5 * y0 - 1.5 * y1 + 0.5 * older;
        let b = newer - 2.5 * y0 + 2.0 * y1 - 0.5 * older;
        let c = -0.5 * newer + 0.5 * y1;

        ((a * t + b) * t + c) * t + y0
    }

    /// Reads at a whole-sample delay, skipping the interpolator.
    ///
    /// For a caller whose read positions never move, which is where the
    /// interpolation is pure cost: Diorama reads 13 fixed taps per channel
    /// per sample, and Catmull-Rom would be four times the arithmetic for an
    /// answer it already has exactly (`REQ-DIO-003`).
    ///
    /// `delay` is clamped the same way [`read`](Self::read) clamps, so a
    /// parameter-derived tap cannot index outside the line.
    pub fn read_whole(&self, delay: usize) -> f32 {
        let delay = delay.clamp(MIN_DELAY_SAMPLES as usize, self.max_delay_samples as usize);
        self.at(delay)
    }

    /// The sample `delay` whole samples in the past. `delay` of one is the most
    /// recently written sample.
    fn at(&self, delay: usize) -> f32 {
        self.buffer[(self.write + self.buffer.len() - delay) & self.mask]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `samples`, then reads. Keeps the tests from repeating the loop.
    fn feed(line: &mut DelayLine, samples: &[f32]) {
        for &s in samples {
            line.write(s);
        }
    }

    #[test]
    fn impulse_arrives_at_the_requested_delay() {
        let mut line = DelayLine::new(48_000.0, 0.1);

        let mut input = vec![0.0; 64];
        input[0] = 1.0;
        feed(&mut line, &input);

        // The impulse was written 64 samples ago, so it sits at a delay of 64.
        assert!((line.read(64.0) - 1.0).abs() < 1e-6, "{}", line.read(64.0));
        assert!(line.read(63.0).abs() < 1e-6);
        assert!(line.read(65.0).abs() < 1e-6);
    }

    /// Catmull-Rom reproduces a straight line exactly, which pins down both the
    /// kernel's coefficients and the direction of the buffer indexing — a sign
    /// error in either shows up here as a large miss rather than a subtle one.
    #[test]
    fn a_ramp_is_interpolated_exactly() {
        let mut line = DelayLine::new(48_000.0, 0.1);

        // value == index, so the sample at delay d is `1023 - (d - 1)`.
        let ramp: Vec<f32> = (0..1024).map(|i| i as f32).collect();
        feed(&mut line, &ramp);

        for delay in [8.0f32, 8.25, 8.5, 10.75, 100.0, 100.5] {
            let expected = 1024.0 - delay;
            let got = line.read(delay);
            assert!(
                (got - expected).abs() < 1e-3,
                "delay {delay}: expected {expected}, got {got}"
            );
        }
    }

    /// Wrapping must not shift the delay. A delay of a whole number of periods
    /// past the reference has to return the same value after the buffer has
    /// been written many times over.
    ///
    /// The reference is `MIN_DELAY_SAMPLES`, not 1: a shorter delay is clamped,
    /// so asking for 1 would quietly compare against a different sample.
    #[test]
    fn wrapping_does_not_move_the_read_position() {
        let mut line = DelayLine::new(48_000.0, 0.01); // 480 samples
        let period = 32;

        // Long enough to wrap the buffer many times over.
        let sine: Vec<f32> = (0..20_000)
            .map(|i| (i as f32 * std::f32::consts::TAU / period as f32).sin())
            .collect();
        feed(&mut line, &sine);

        let reference = line.read(MIN_DELAY_SAMPLES);
        for periods in 1..8 {
            let delay = MIN_DELAY_SAMPLES + (periods * period) as f32;
            let got = line.read(delay);
            assert!(
                (got - reference).abs() < 1e-3,
                "delay {delay} drifted: expected {reference}, got {got}"
            );
        }
    }

    /// The same check with a signal whose value states which sample it is, so a
    /// wrap that shifted the read position by even one slot fails by a whole
    /// unit rather than by a rounding error. Fractional delays are included so
    /// the interpolator is exercised across the wrap point too.
    #[test]
    fn wrapping_keeps_interpolation_exact() {
        let mut line = DelayLine::new(48_000.0, 0.01); // 480 samples, 512 slots
        let count = 20_000;

        // value == index, so the sample at delay d is `count - d`.
        let ramp: Vec<f32> = (0..count).map(|i| i as f32).collect();
        feed(&mut line, &ramp);

        for delay in [2.0f32, 2.5, 17.25, 200.0, 400.75, 479.0] {
            let expected = count as f32 - delay;
            let got = line.read(delay);
            assert!(
                (got - expected).abs() < 0.05,
                "delay {delay}: expected {expected}, got {got}"
            );
        }
    }

    /// The whole-sample read has to agree with the interpolating one at an
    /// integer position, or the two paths through the same buffer disagree
    /// about which sample a delay names. **This is the doubled-value case the
    /// rules warn about** (`.agents/rules/rust.md`): the same index arithmetic
    /// written twice.
    #[test]
    fn a_whole_read_matches_the_interpolated_one() {
        let mut line = DelayLine::new(48_000.0, 0.1);

        let ramp: Vec<f32> = (0..1024).map(|i| i as f32).collect();
        feed(&mut line, &ramp);

        for delay in [2usize, 8, 64, 100, 4_799] {
            let whole = line.read_whole(delay);
            let interpolated = line.read(delay as f32);
            assert!(
                (whole - interpolated).abs() < 1e-3,
                "delay {delay}: whole {whole}, interpolated {interpolated}"
            );
        }
    }

    #[test]
    fn out_of_range_delays_are_clamped_not_panics() {
        let mut line = DelayLine::new(48_000.0, 0.01);
        feed(&mut line, &[1.0; 1000]);

        let max = line.max_delay_samples();
        assert_eq!(line.read(-5.0), line.read(MIN_DELAY_SAMPLES));
        assert_eq!(line.read(0.0), line.read(MIN_DELAY_SAMPLES));
        assert_eq!(line.read(max + 1000.0), line.read(max));
        assert!(line.read(f32::NAN).is_finite());
        assert!(line.read(f32::INFINITY).is_finite());
    }

    #[test]
    fn the_line_holds_the_delay_it_was_asked_for() {
        let line = DelayLine::new(48_000.0, 0.15);
        assert!((line.max_delay_samples() - 7200.0).abs() < 0.5);
        // Power of two, with room for the interpolator on top of the maximum.
        assert!(line.buffer.len().is_power_of_two());
        assert!(line.buffer.len() >= 7200 + INTERP_MARGIN);
    }

    #[test]
    fn reset_clears_without_reallocating() {
        let mut line = DelayLine::new(48_000.0, 0.01);
        let len = line.buffer.len();
        feed(&mut line, &[1.0; 100]);
        assert!(line.read(10.0) > 0.5);

        line.reset();
        assert_eq!(line.read(10.0), 0.0);
        assert_eq!(line.buffer.len(), len);
    }
}
