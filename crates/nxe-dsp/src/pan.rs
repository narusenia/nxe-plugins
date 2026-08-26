//! Where a stereo signal's energy sits across the image.
//!
//! Answers "how much sound is coming from each direction", which is what makes
//! it something to draw *behind* a pan display: the same axis, the settings on
//! top and the result underneath.
//!
//! **Per short window, not per sample.** A single sample pair's balance jumps
//! between the extremes on almost any material — the instantaneous ratio of two
//! waveforms says nothing about where the sound is. Summing a couple of
//! milliseconds first gives a figure that holds still enough to read.

/// How many samples go into one balance reading. About 1.3 ms at 48 kHz: long
/// enough to be stable, short enough that a moving image still moves.
const WINDOW: usize = 64;

/// How long a bin takes to fall to `1/e` of its value with nothing feeding it.
/// Slow enough to read, fast enough to follow a phrase.
const DECAY_SECONDS: f32 = 0.250;

/// Below this a window is treated as silence and moves nothing. Otherwise the
/// balance of the noise floor draws a picture of nothing at all.
const FLOOR: f32 = 1e-9;

/// The energy across the image, in `BINS` directions from hard left to hard
/// right.
///
/// `BINS` is the caller's: it is a display resolution, not an acoustic fact.
pub struct PanScope<const BINS: usize> {
    bins: [f32; BINS],
    /// Sums for the window in progress.
    left_energy: f32,
    right_energy: f32,
    filled: usize,
    /// What every bin is multiplied by when a window completes.
    decay: f32,
    /// Turns an accumulated bin into a level a caller can read on an absolute
    /// scale — see [`PanScope::levels`].
    scale: f32,
}

impl<const BINS: usize> PanScope<BINS> {
    pub fn new(sample_rate: f32) -> Self {
        // Per window rather than per sample, because that is when the decay is
        // applied.
        let windows_per_second = sample_rate / WINDOW as f32;
        let decay = (-1.0 / (DECAY_SECONDS * windows_per_second)).exp();

        // A bin accumulates one window's energy per window and loses `decay` of
        // itself each time, so a steady signal settles at `energy / (1 - decay)`.
        // Dividing that out is what makes the reading mean something on its own
        // rather than only relative to the other bins.
        // "Full scale" is both channels at one, so a window's energy is twice
        // its length. A signal hard to one side therefore reads half — which is
        // right: half the pair is silent.
        let full_scale_window = 2.0 * WINDOW as f32;
        let steady_state = full_scale_window / (1.0 - decay);

        Self {
            bins: [0.0; BINS],
            left_energy: 0.0,
            right_energy: 0.0,
            filled: 0,
            decay,
            scale: 1.0 / steady_state,
        }
    }

    /// Feeds one sample pair. Call for every frame of output.
    pub fn push(&mut self, left: f32, right: f32) {
        self.left_energy += left * left;
        self.right_energy += right * right;
        self.filled += 1;

        if self.filled >= WINDOW {
            self.close_window();
        }
    }

    /// The energy in each direction, hard left first, on an **absolute scale**:
    /// a full-scale signal sitting in one direction settles near `1`, and
    /// silence really is `0`.
    ///
    /// **Not normalized against the largest bin.** That was the first version
    /// and it was wrong: every bin decays at the same rate, so the ratios
    /// between them never change and the picture of a sound that stopped stays
    /// on screen for ever. An absolute reading fades because the sound faded.
    pub fn levels(&self) -> [f32; BINS] {
        std::array::from_fn(|index| self.bins[index] * self.scale)
    }

    /// Forgets everything. For a transport stop, or reopening an editor onto a
    /// silent track.
    pub fn reset(&mut self) {
        self.bins = [0.0; BINS];
        self.left_energy = 0.0;
        self.right_energy = 0.0;
        self.filled = 0;
    }

    fn close_window(&mut self) {
        let energy = self.left_energy + self.right_energy;

        for bin in &mut self.bins {
            *bin *= self.decay;
        }

        if energy > FLOOR {
            // `-1` hard left, `+1` hard right. Energies rather than amplitudes,
            // so the reading is where the *power* is and a quiet channel does
            // not pull as hard as a loud one.
            let balance = (self.right_energy - self.left_energy) / energy;
            self.bins[Self::bin_of(balance)] += energy;
        }

        self.left_energy = 0.0;
        self.right_energy = 0.0;
        self.filled = 0;
    }

    /// Which bin a balance in `-1..=1` lands in. Pure, so the edges are
    /// testable without running a signal through.
    fn bin_of(balance: f32) -> usize {
        let position = (balance.clamp(-1.0, 1.0) + 1.0) * 0.5;
        ((position * BINS as f32) as usize).min(BINS - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run<const BINS: usize>(scope: &mut PanScope<BINS>, left: f32, right: f32, samples: usize) {
        for _ in 0..samples {
            scope.push(left, right);
        }
    }

    /// Which bin is loudest, which is the only thing a reader takes from the
    /// picture.
    fn peak<const BINS: usize>(scope: &PanScope<BINS>) -> usize {
        scope
            .levels()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap()
    }

    #[test]
    fn the_edges_land_in_the_end_bins() {
        assert_eq!(PanScope::<8>::bin_of(-1.0), 0);
        assert_eq!(PanScope::<8>::bin_of(1.0), 7);
        // Out of range is clamped, not wrapped.
        assert_eq!(PanScope::<8>::bin_of(-9.0), 0);
        assert_eq!(PanScope::<8>::bin_of(9.0), 7);
    }

    #[test]
    fn centred_sound_lands_in_the_middle() {
        let mut scope: PanScope<9> = PanScope::new(SR);
        run(&mut scope, 0.5, 0.5, WINDOW * 4);
        assert_eq!(peak(&scope), 4);
    }

    #[test]
    fn a_one_sided_signal_lands_at_that_side() {
        let mut left: PanScope<9> = PanScope::new(SR);
        run(&mut left, 0.5, 0.0, WINDOW * 4);
        assert_eq!(peak(&left), 0);

        let mut right: PanScope<9> = PanScope::new(SR);
        run(&mut right, 0.0, 0.5, WINDOW * 4);
        assert_eq!(peak(&right), 8);
    }

    /// Silence has no direction. Without the floor, the balance of whatever
    /// numerical dust is left would keep drawing.
    #[test]
    fn silence_moves_nothing() {
        let mut scope: PanScope<9> = PanScope::new(SR);
        run(&mut scope, 0.0, 0.0, WINDOW * 4);
        assert_eq!(scope.levels(), [0.0; 9]);
    }

    #[test]
    fn a_bin_falls_away_once_the_sound_stops() {
        let mut scope: PanScope<9> = PanScope::new(SR);
        run(&mut scope, 0.5, 0.5, WINDOW * 4);
        let loud = scope.levels()[4];

        // One time constant of silence.
        run(&mut scope, 0.0, 0.0, (DECAY_SECONDS * SR) as usize);
        let quiet = scope.levels()[4];

        assert!(quiet < loud * 0.5, "{loud} did not fall: {quiet}");
        assert!(quiet > 0.0, "it fell all the way to zero");
    }

    /// The reading has to mean something on its own, or a display can only ever
    /// show the shape and never the loudness — which is how a picture of a
    /// sound that stopped stays on screen.
    #[test]
    fn a_full_scale_signal_reads_about_one() {
        let mut scope: PanScope<9> = PanScope::new(SR);
        // Well past the 250 ms it takes to settle.
        run(&mut scope, 1.0, 1.0, (SR * 2.0) as usize);
        let level = scope.levels()[4];
        assert!((0.7..=1.3).contains(&level), "full scale read {level}");
    }

    #[test]
    fn a_quiet_signal_reads_quiet() {
        let mut loud: PanScope<9> = PanScope::new(SR);
        run(&mut loud, 1.0, 1.0, (SR * 2.0) as usize);

        let mut quiet: PanScope<9> = PanScope::new(SR);
        run(&mut quiet, 0.1, 0.1, (SR * 2.0) as usize);

        // A tenth of the amplitude is a hundredth of the energy.
        let ratio = quiet.levels()[4] / loud.levels()[4];
        assert!((0.005..=0.02).contains(&ratio), "the ratio was {ratio}");
    }

    #[test]
    fn reset_clears_it() {
        let mut scope: PanScope<9> = PanScope::new(SR);
        run(&mut scope, 0.5, 0.5, WINDOW * 4);
        scope.reset();
        assert_eq!(scope.levels(), [0.0; 9]);
    }

    /// The decay is defined in seconds, so a bin has to fall at the same rate in
    /// wall-clock terms whatever the host is running at.
    #[test]
    fn the_decay_is_the_same_at_every_sample_rate() {
        let mut levels = Vec::new();

        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut scope: PanScope<9> = PanScope::new(rate);
            run(&mut scope, 0.5, 0.5, WINDOW);
            let loud = scope.levels()[4];
            run(&mut scope, 0.0, 0.0, (DECAY_SECONDS * rate) as usize);
            levels.push(scope.levels()[4] / loud);
        }

        let first = levels[0];
        for level in &levels {
            assert!((level - first).abs() < 0.02, "{levels:?} disagree");
        }
    }
}
