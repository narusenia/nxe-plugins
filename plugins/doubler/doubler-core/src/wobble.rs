//! The slow random signal behind Humanize.
//!
//! What makes a doubler sound like a second take rather than a chorus is that
//! the second take is *not* periodic. So this is not an LFO: it picks a random
//! target at a random-ish interval and slews toward it. The output is bounded
//! by construction — it only ever interpolates between targets that are
//! themselves in `[-1, 1]` — and it has no period to hear.
//!
//! Each voice owns two of these, one for its detune and one for its delay, with
//! different seeds so pitch and timing do not wobble together.
//!
//! See `plugins/doubler/docs/specifications/dsp.md`.

/// How long the slew takes to cover most of the distance to a new target.
/// Ear-tuned (`dsp.md`).
const SLEW_SECONDS: f32 = 0.300;

/// The interval between targets is drawn once per source from this range, and
/// then kept. Ear-tuned (`dsp.md`).
const INTERVAL_MIN_SECONDS: f32 = 0.400;
const INTERVAL_MAX_SECONDS: f32 = 1.200;

pub struct Wobble {
    rng: u32,
    /// Samples between targets. Fixed for the lifetime of this source, so two
    /// voices never fall into step.
    interval: f32,
    countdown: f32,
    target: f32,
    value: f32,
    /// One-pole coefficient for the slew.
    slew: f32,
}

impl Wobble {
    /// `seed` only has to differ per source; the same seed always produces the
    /// same sequence, which is what makes a rendered mix reproducible
    /// (`REQ-DBL-003`).
    pub fn new(sample_rate: f32, seed: u32) -> Self {
        // Spread the seeds apart before use: consecutive small integers put
        // xorshift in nearly the same state, so voice 0 and voice 1 would open
        // with almost the same sequence.
        let mut rng = seed.wrapping_mul(2_654_435_761) | 1;

        let span = INTERVAL_MAX_SECONDS - INTERVAL_MIN_SECONDS;
        let interval = (INTERVAL_MIN_SECONDS + span * unit(&mut rng)) * sample_rate;

        Self {
            rng,
            interval,
            countdown: 0.0,
            target: 0.0,
            value: 0.0,
            slew: (-1.0 / (SLEW_SECONDS * sample_rate)).exp(),
        }
    }

    pub fn reset(&mut self) {
        self.countdown = 0.0;
        self.target = 0.0;
        self.value = 0.0;
    }

    /// The next sample, in `[-1, 1]`.
    pub fn next(&mut self) -> f32 {
        self.countdown -= 1.0;
        if self.countdown <= 0.0 {
            self.countdown = self.interval;
            self.target = unit(&mut self.rng) * 2.0 - 1.0;
        }

        self.value = self.target + (self.value - self.target) * self.slew;
        self.value
    }
}

/// xorshift32, mapped to `[0, 1)`. A handful of lines rather than a dependency:
/// nothing here needs statistical quality, only "not periodic to the ear".
fn unit(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state as f32 / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn collect(seed: u32, len: usize) -> Vec<f32> {
        let mut wobble = Wobble::new(SR, seed);
        (0..len).map(|_| wobble.next()).collect()
    }

    /// The depth knob scales this, so anything outside `[-1, 1]` would make the
    /// documented depths wrong.
    #[test]
    fn the_output_stays_inside_the_unit_range() {
        for seed in 0..8 {
            for (i, value) in collect(seed, 480_000).iter().enumerate() {
                assert!(
                    (-1.0..=1.0).contains(value),
                    "seed {seed} left the range at sample {i}: {value}"
                );
            }
        }
    }

    /// `REQ-DBL-003`: the same settings and the same input give the same output.
    #[test]
    fn the_same_seed_repeats_exactly() {
        assert_eq!(collect(3, 100_000), collect(3, 100_000));
    }

    /// Two voices must not wobble together, including right at the start where
    /// a poorly spread seed would leave them nearly identical.
    #[test]
    fn different_seeds_diverge() {
        for a in 0..MAX_SEEDS {
            for b in (a + 1)..MAX_SEEDS {
                let (x, y) = (collect(a, 96_000), collect(b, 96_000));
                let difference: f32 =
                    x.iter().zip(&y).map(|(x, y)| (x - y).abs()).sum::<f32>() / x.len() as f32;
                assert!(
                    difference > 0.05,
                    "seeds {a} and {b} only differ by {difference} on average"
                );
            }
        }
    }
    const MAX_SEEDS: u32 = 8;

    /// It has to be slow. A fast random signal on a read position is a click,
    /// and on a detune it is a warble rather than a take.
    #[test]
    fn it_moves_slowly() {
        let values = collect(1, 480_000);
        let step = values
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);

        // A one-pole with a 300 ms time constant cannot cross more than the
        // full range in one sample by more than this.
        let bound = 2.0 / (SLEW_SECONDS * SR);
        assert!(step <= bound, "stepped by {step}, bound is {bound}");
    }

    /// Not an LFO: the interval between targets differs per source, so no two
    /// sources share a period.
    #[test]
    fn the_interval_differs_between_sources() {
        let intervals: Vec<f32> = (0..8).map(|s| Wobble::new(SR, s).interval).collect();
        for (i, a) in intervals.iter().enumerate() {
            assert!(
                (*a >= INTERVAL_MIN_SECONDS * SR) && (*a <= INTERVAL_MAX_SECONDS * SR),
                "source {i}: interval {a} is outside the documented range"
            );
            for (j, b) in intervals.iter().enumerate().skip(i + 1) {
                assert!(
                    (a - b).abs() > SR * 0.01,
                    "sources {i} and {j} share an interval"
                );
            }
        }
    }

    #[test]
    fn reset_returns_to_the_centre() {
        let mut wobble = Wobble::new(SR, 5);
        for _ in 0..50_000 {
            wobble.next();
        }
        wobble.reset();
        // The first sample after a reset is still at rest, not wherever the
        // wobble happened to be.
        assert_eq!(wobble.value, 0.0);
    }
}
