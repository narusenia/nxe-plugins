//! A rotating-tap pitch shifter.
//!
//! One voice's constant detune, produced by reading a shared delay line at a
//! rate of `2^(cents / 1200)`. Reading at a rate other than 1 makes the read
//! position drift away from the write head forever, so the position is wrapped
//! by one window and crossfaded to hide the jump.
//!
//! **Only one tap is active most of the time.** The other exists only during a
//! short crossfade. That is the whole reason for this shape rather than a
//! continuous two-tap crossfade: two taps reading one signal at different
//! delays comb-filter each other, so keeping them both open all the time
//! colours the sound permanently. Here the comb lasts for the length of one
//! fade, a few milliseconds apart by seconds.
//!
//! It also makes `cents == 0` exactly transparent with no special case: the
//! read position does not drift, so it never wraps, so it never fades, and the
//! output is the delay line read at the requested delay.
//!
//! See `plugins/doubler/docs/specifications/dsp.md`.

/// How far the read position is allowed to drift before it wraps.
///
/// Longer means rarer wraps — at ±50 cent the position takes about 1.7 s to
/// cross a 50 ms window, and over 7 s at the default 12 cent. It also means
/// the voice's delay wanders further: the position sweeps the whole window, so
/// the delay a listener hears moves over `[base, base + window]`.
///
/// Ear-tuned (`dsp.md`).
const WINDOW_SECONDS: f32 = 0.050;

/// The crossfade at a wrap. Long enough not to click, short enough that the
/// comb between the two taps is an event rather than a colour.
///
/// Ear-tuned (`dsp.md`).
const FADE_SECONDS: f32 = 0.005;

pub struct PitchShifter {
    window: f32,
    fade: f32,
    /// The live tap's position above `base_delay`, kept in `[0, window]`.
    ///
    /// Stored relative to the base so the base can move — the `Delay` macro,
    /// Humanize — without disturbing where we are in the window.
    offset: f32,
    /// The outgoing tap's offset. Only meaningful while `fade_left > 0`.
    fade_from: f32,
    /// Samples of crossfade left to run. Zero means a single tap is live.
    fade_left: f32,
}

impl PitchShifter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            window: sample_rate * WINDOW_SECONDS,
            fade: sample_rate * FADE_SECONDS,
            offset: 0.0,
            fade_from: 0.0,
            fade_left: 0.0,
        }
    }

    /// How much delay the shifter can add on top of the base, in samples. The
    /// delay line has to hold the largest base plus this.
    pub fn window_samples(&self) -> f32 {
        self.window
    }

    pub fn reset(&mut self) {
        self.offset = 0.0;
        self.fade_from = 0.0;
        self.fade_left = 0.0;
    }

    /// Advances one sample and reads through `read`.
    ///
    /// The source is a closure rather than a `DelayLine` so the shifter does
    /// not have to know what a voice is reading from — one channel, the average
    /// of two, or a blend of both while the source mode crossfades
    /// (`REQ-DBL-004`).
    ///
    /// `base_delay` and the return value are in samples; `cents` is the shift.
    /// Nothing here allocates or locks, so it is safe on the audio thread.
    pub fn process(&mut self, read: impl Fn(f32) -> f32, base_delay: f32, cents: f32) -> f32 {
        // Reading at rate `r` moves the read position away from the write head
        // by `1 - r` samples per sample: faster than realtime (`r > 1`, a
        // higher pitch) eats into the delay, slower lets it grow.
        let drift = 1.0 - (cents / 1200.0).exp2();

        self.offset += drift;
        if self.fade_left > 0.0 {
            self.fade_from += drift;
            self.fade_left -= 1.0;
        }

        if self.offset > self.window {
            // ponytail: a wrap during a fade drops the older tap instead of
            // mixing three. Only reachable if the wrap interval is shorter than
            // the fade, which needs a shift far beyond the ±50 cent this
            // product exposes (`REQ-DBL-002`).
            self.fade_from = self.offset;
            self.offset -= self.window;
            self.fade_left = self.fade;
        } else if self.offset < 0.0 {
            self.fade_from = self.offset;
            self.offset += self.window;
            self.fade_left = self.fade;
        }

        let live = read(base_delay + self.offset);
        if self.fade_left <= 0.0 {
            return live;
        }

        // Equal power: a whole window apart, the two taps are uncorrelated for
        // anything but a steady tone, so their powers add rather than their
        // amplitudes.
        let progress = 1.0 - self.fade_left / self.fade;
        let (sin, cos) = (progress * std::f32::consts::FRAC_PI_2).sin_cos();
        sin * live + cos * read(base_delay + self.fade_from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DelayLine;

    const SR: f32 = 48_000.0;

    /// Runs a signal through a line and a shifter, returning the output.
    fn run(cents: f32, base_delay: f32, input: &[f32]) -> Vec<f32> {
        let mut line = DelayLine::new(SR, 0.15);
        let mut shifter = PitchShifter::new(SR);

        input
            .iter()
            .map(|&s| {
                line.write(s);
                shifter.process(|delay| line.read(delay), base_delay, cents)
            })
            .collect()
    }

    fn sine(freq: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (i as f32 * std::f32::consts::TAU * freq / SR).sin())
            .collect()
    }

    /// Frequency from the first and last positive-going zero crossing, with the
    /// crossing instants interpolated. Far more precise than counting them.
    fn measure_freq(signal: &[f32]) -> f32 {
        let mut first = None;
        let mut last = 0.0;
        let mut cycles = 0u32;

        for i in 1..signal.len() {
            if signal[i - 1] <= 0.0 && signal[i] > 0.0 {
                let frac = -signal[i - 1] / (signal[i] - signal[i - 1]);
                let t = (i - 1) as f32 + frac;
                match first {
                    None => first = Some(t),
                    Some(_) => {
                        cycles += 1;
                        last = t;
                    }
                }
            }
        }

        let first = first.expect("no zero crossing found");
        assert!(cycles > 10, "not enough cycles to measure");
        SR * cycles as f32 / (last - first)
    }

    fn cents_between(measured: f32, expected: f32) -> f32 {
        1200.0 * (measured / expected).log2()
    }

    /// The acceptance condition for `REQ-DBL-002`: the output's pitch is the
    /// input's times `2^(cents / 1200)`.
    ///
    /// Measured over a stretch shorter than the wrap interval (about 0.84 s at
    /// 100 cent), so the number is the algorithm's own accuracy rather than the
    /// crossfade's effect on it.
    #[test]
    fn the_shift_ratio_is_exact() {
        let len = (SR * 0.5) as usize;
        let input = sine(1000.0, len);

        for cents in [100.0f32, -100.0, 50.0, -50.0, 12.0] {
            let out = run(cents, 100.0, &input);
            let expected = 1000.0 * (cents / 1200.0).exp2();
            let measured = measure_freq(&out[1000..]);
            let error = cents_between(measured, expected);
            assert!(
                error.abs() < 0.5,
                "{cents} cent: expected {expected} Hz, measured {measured} Hz ({error:.3} cent off)"
            );
        }
    }

    /// No detune must mean no change: the read position never drifts, so it
    /// never wraps and never fades, and the output is the plain delayed signal.
    #[test]
    fn zero_detune_is_bit_for_bit_transparent() {
        let input = sine(1000.0, 4096);

        let mut line = DelayLine::new(SR, 0.15);
        let mut shifter = PitchShifter::new(SR);
        let base = 100.0;

        for &s in &input {
            line.write(s);
            let shifted = shifter.process(|delay| line.read(delay), base, 0.0);
            let plain = line.read(base);
            assert_eq!(shifted, plain);
        }
    }

    /// A wrap must not click. The output's largest sample-to-sample step stays
    /// in the region the input's own slope explains, over a stretch long enough
    /// to contain several wraps.
    #[test]
    fn wrapping_does_not_click() {
        let len = (SR * 4.0) as usize;
        let input = sine(220.0, len);
        let out = run(100.0, 200.0, &input);

        let step = |s: &[f32]| {
            s.windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max)
        };

        let input_step = step(&input);
        let out_step = step(&out[2000..]);
        assert!(
            out_step < input_step * 1.5,
            "output steps by {out_step} where the input steps by at most {input_step}"
        );
    }

    /// The window is what the delay line has to make room for on top of the
    /// largest base delay (`dsp.md`).
    #[test]
    fn the_window_is_reported_in_samples() {
        let shifter = PitchShifter::new(SR);
        assert!((shifter.window_samples() - SR * WINDOW_SECONDS).abs() < 0.5);
    }

    #[test]
    fn extreme_shifts_stay_finite() {
        let input = sine(1000.0, 8192);
        for cents in [4800.0f32, -4800.0, 0.0] {
            for s in run(cents, 100.0, &input) {
                assert!(s.is_finite(), "{cents} cent produced a non-finite sample");
            }
        }
    }
}
