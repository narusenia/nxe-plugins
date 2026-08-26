//! Splitting the input into five bands whose sum is flat.
//!
//! **This is the gate** (`SPK-2`, `REQ-SPK-001`). Sparkleur's promise is that
//! all five bands at unity is "not doing anything", and a tree of crossovers
//! does not give that for free: a band split off early misses the phase
//! rotation of every split below it, and the sum ripples by ±0.3–0.5 dB. Every
//! parameter above this reads as broken if the floor it stands on is not flat.
//!
//! ## The tree, and what each band missed
//!
//! Linkwitz-Riley 4th order — two Butterworth sections (`Q = 1/√2`) each way —
//! split four times, each time keeping the low half and passing the rest on:
//!
//! ```text
//! split(120):   band1, rest
//! split(400):   band2, rest
//! split(1500):  band3, rest
//! split(6000):  band4, band5
//! ```
//!
//! An LR4 pair sums to a second-order allpass, so `band4 + band5` is the last
//! split's allpass, `band3` has to be run through that same allpass before it
//! can join them, `band2` through two, and `band1` through three. Then the
//! whole sum telescopes down to four allpasses in series: **flat in amplitude,
//! and the phase rotation is the honest cost of splitting**
//! (`REQ-SPK-001` writes that down as a promise rather than hiding it).
//!
//! ## Why `LP + HP` rather than a dedicated allpass
//!
//! The correction is written as the sum of the same low-pass and high-pass the
//! split uses, because that is what it *is* — no second derivation to keep in
//! step with the first, and biquads are cheap (`dsp.md`). A dedicated
//! second-order allpass is the same filter with a quarter of the sections, so
//! this is the first thing to reach for if `SPK-17` finds the budget tight.
//!
//! ## One channel
//!
//! A `Crossover` is one channel's worth of filters. Stereo is two of them:
//! filters and gains are per channel, only detection is linked
//! (`REQ-SPK-011`).

use nxe_audio::biquad::{BUTTERWORTH_Q, Biquad, Coefficients};

pub const BAND_COUNT: usize = 5;

/// The four boundaries at `FOCUS` = 0, in Hz (`REQ-SPK-002`).
pub const EDGES: [f32; BAND_COUNT - 1] = [120.0, 400.0, 1_500.0, 6_000.0];

/// How far `FOCUS` slides every boundary, in octaves either way.
///
/// Wider than Velour's `FOCUS` because the material is wider — a voice to a
/// bass guitar, not a voice (`REQ-SPK-002`).
pub const FOCUS_OCTAVES: f32 = 1.5;

/// The highest a boundary may sit, as a fraction of the sample rate.
///
/// At 44.1 kHz nothing reaches it — the top edge fully open is 17 kHz against a
/// ceiling of 19.8 kHz — so this exists for the rates below that, where a
/// boundary would otherwise be asked to sit above Nyquist.
const EDGE_CEILING: f32 = 0.45;

/// One Linkwitz-Riley 4th-order split: two cascaded Butterworth sections each
/// way.
#[derive(Clone, Copy, Default)]
struct Split {
    low: [Biquad; 2],
    high: [Biquad; 2],
}

impl Split {
    fn set(&mut self, hz: f32, sample_rate: f32) {
        let low = Coefficients::lowpass(hz, BUTTERWORTH_Q, sample_rate);
        let high = Coefficients::highpass(hz, BUTTERWORTH_Q, sample_rate);
        for section in &mut self.low {
            section.set(low);
        }
        for section in &mut self.high {
            section.set(high);
        }
    }

    fn process(&mut self, input: f32) -> (f32, f32) {
        let low = self.low.iter_mut().fold(input, |x, s| s.process(x));
        let high = self.high.iter_mut().fold(input, |x, s| s.process(x));
        (low, high)
    }

    /// `LP + HP`, which for an LR4 pair **is** the second-order allpass the
    /// split leaves behind — see the module documentation.
    fn allpass(&mut self, input: f32) -> f32 {
        let (low, high) = self.process(input);
        low + high
    }

    fn reset(&mut self) {
        for section in self.low.iter_mut().chain(&mut self.high) {
            section.reset();
        }
    }
}

/// The allpasses each early band has to be run through to catch up with the
/// bands below it.
#[derive(Clone, Copy, Default)]
struct Corrections {
    /// Band 1 was split off first and missed all three splits above it.
    band1: [Split; 3],
    /// Band 2 missed two.
    band2: [Split; 2],
    /// Band 3 missed one.
    band3: Split,
}

/// Five bands whose sum is flat, and one `FOCUS` that slides every boundary.
pub struct Crossover {
    sample_rate: f32,
    splits: [Split; BAND_COUNT - 1],
    corrections: Corrections,
    edges: [f32; BAND_COUNT - 1],
    shift: f32,
    #[cfg(test)]
    rebuilds: usize,
}

impl Crossover {
    pub fn new(sample_rate: f32) -> Self {
        let mut crossover = Self {
            sample_rate,
            splits: [Split::default(); BAND_COUNT - 1],
            corrections: Corrections::default(),
            edges: EDGES,
            // Not a valid shift, so the first `set_focus` always builds.
            shift: 0.0,
            #[cfg(test)]
            rebuilds: 0,
        };
        crossover.set_focus(0.0);
        crossover
    }

    /// Slides all four boundaries together, `-1..=1`.
    ///
    /// **Block rate.** This is where every coefficient is built; [`split`] does
    /// no filter arithmetic at all (`SPK-2`). A `FOCUS` that has not moved
    /// costs a comparison.
    ///
    /// [`split`]: Self::split
    pub fn set_focus(&mut self, focus: f32) {
        let shift = shift_of(focus);
        if shift == self.shift {
            return;
        }
        self.shift = shift;
        self.edges = edges_for(focus, self.sample_rate);

        for (split, hz) in self.splits.iter_mut().zip(self.edges) {
            split.set(hz, self.sample_rate);
        }
        // Each correction stands in for one boundary **above** the band it
        // fixes, in the same order the tree passes them.
        let [_, second, third, fourth] = self.edges;
        for (section, hz) in self
            .corrections
            .band1
            .iter_mut()
            .zip([second, third, fourth])
        {
            section.set(hz, self.sample_rate);
        }
        for (section, hz) in self.corrections.band2.iter_mut().zip([third, fourth]) {
            section.set(hz, self.sample_rate);
        }
        self.corrections.band3.set(fourth, self.sample_rate);

        #[cfg(test)]
        {
            self.rebuilds += 1;
        }
    }

    /// One sample in, five bands out. **Audio rate**, allocation-free.
    pub fn split(&mut self, input: f32) -> [f32; BAND_COUNT] {
        let mut bands = [0.0f32; BAND_COUNT];

        let mut rest = input;
        for (band, split) in bands.iter_mut().zip(&mut self.splits) {
            let (low, high) = split.process(rest);
            *band = low;
            rest = high;
        }
        bands[BAND_COUNT - 1] = rest;

        // Put back the phase each band missed by being split off early.
        for section in &mut self.corrections.band1 {
            bands[0] = section.allpass(bands[0]);
        }
        for section in &mut self.corrections.band2 {
            bands[1] = section.allpass(bands[1]);
        }
        bands[2] = self.corrections.band3.allpass(bands[2]);

        bands
    }

    /// The four boundaries as they currently stand, in Hz.
    ///
    /// The detector reads these to derive its time constants from each band's
    /// centre (`SPK-3`), and the interface draws the regions from them.
    pub fn edges(&self) -> [f32; BAND_COUNT - 1] {
        self.edges
    }

    pub fn reset(&mut self) {
        for split in &mut self.splits {
            split.reset();
        }
        for section in self
            .corrections
            .band1
            .iter_mut()
            .chain(&mut self.corrections.band2)
        {
            section.reset();
        }
        self.corrections.band3.reset();
    }

    #[cfg(test)]
    fn rebuilds(&self) -> usize {
        self.rebuilds
    }
}

/// `FOCUS` as a frequency multiplier. Hostile values fall back to the centre
/// rather than to an edge, because the centre is the one setting that is
/// certainly meant (`REQ-SPK-002`).
fn shift_of(focus: f32) -> f32 {
    let focus = if focus.is_finite() {
        focus.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    (focus * FOCUS_OCTAVES).exp2()
}

/// The four boundaries at a `FOCUS` position, in Hz.
///
/// **Public because the picture needs the same numbers the filters are tuned
/// from** (`SPK-13`). An interface that mapped `FOCUS` to positions on its own
/// would be a second copy of this arithmetic, and the two would drift.
pub fn edges_for(focus: f32, sample_rate: f32) -> [f32; BAND_COUNT - 1] {
    edges_at(shift_of(focus), sample_rate)
}

/// The four boundaries after a shift, held under the ceiling.
///
/// **The ceiling does not have to preserve the order.** The allpass identity
/// `LP(f) + HP(f) = AP(f)` holds at any corner, so two boundaries landing on
/// the same frequency empties the band between them without costing the sum its
/// flatness. Guarding the order here would only be guarding a number the
/// interface shows.
fn edges_at(shift: f32, sample_rate: f32) -> [f32; BAND_COUNT - 1] {
    let ceiling = sample_rate * EDGE_CEILING;
    EDGES.map(|edge| (edge * shift).min(ceiling))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nxe_audio::harmonics::{amplitude, bin_of, db_ratio, tone};

    const RATE: f32 = 48_000.0;

    /// A whole second, so 20 Hz lands on a whole number of cycles at every rate
    /// tested and the filters are long settled.
    fn length_of(rate: f32) -> usize {
        rate as usize
    }

    /// The response of a signal taken from the crossover, in dB against the
    /// input.
    ///
    /// The buffer holds whole cycles, so feeding it twice is one continuous
    /// tone — the first pass settles the filters and the second is measured.
    fn measure(
        crossover: &mut Crossover,
        hz: f32,
        rate: f32,
        pick: impl Fn(&[f32; 5]) -> f32,
    ) -> f32 {
        let length = length_of(rate);
        let input = tone(1.0, hz, rate, length);
        for sample in &input {
            crossover.split(*sample);
        }
        let output: Vec<f32> = input.iter().map(|s| pick(&crossover.split(*s))).collect();
        let bin = bin_of(hz, rate, length);
        db_ratio(amplitude(&output, bin), amplitude(&input, bin))
    }

    fn sum_db(crossover: &mut Crossover, hz: f32, rate: f32) -> f32 {
        measure(crossover, hz, rate, |bands| bands.iter().sum())
    }

    fn band_db(crossover: &mut Crossover, band: usize, hz: f32, rate: f32) -> f32 {
        measure(crossover, hz, rate, |bands| bands[band])
    }

    /// **The gate** (`SPK-2`, `REQ-SPK-001`). Everything above this unit reads
    /// as broken if the floor is not flat.
    #[test]
    fn the_five_bands_sum_flat_across_the_audible_range() {
        let mut crossover = Crossover::new(RATE);
        for hz in [
            20.0f32, 30.0, 50.0, 80.0, 120.0, 200.0, 400.0, 800.0, 1_500.0, 3_000.0, 6_000.0,
            10_000.0, 15_000.0, 20_000.0,
        ] {
            let reading = sum_db(&mut crossover, hz, RATE);
            assert!(reading.abs() < 0.1, "{hz} Hz summed to {reading:.3} dB");
        }
    }

    /// **The flatness test above can pass without splitting anything** — five
    /// copies of a fifth of the input would sum flat too. This is what makes it
    /// mean something.
    #[test]
    fn each_band_keeps_its_own_range_and_rejects_the_rest() {
        let mut crossover = Crossover::new(RATE);
        // A frequency well inside each band, and the two bands that must not
        // have it.
        for (band, hz) in [
            (0, 40.0f32),
            (1, 220.0),
            (2, 800.0),
            (3, 3_000.0),
            (4, 12_000.0),
        ] {
            // Not 0 dB, and it should not be: a 1.7-octave band with 24 dB/oct
            // walls does not reach unity even at its geometric centre.
            let inside = band_db(&mut crossover, band, hz, RATE);
            assert!(inside > -2.0, "band {band} lost {hz} Hz: {inside:.1} dB");

            for other in 0..BAND_COUNT {
                if other.abs_diff(band) < 2 {
                    continue;
                }
                let outside = band_db(&mut crossover, other, hz, RATE);
                assert!(
                    outside < -30.0,
                    "band {other} passed {hz} Hz at {outside:.1} dB"
                );
            }
        }
    }

    /// LR4: both halves are 6 dB down where they cross, which is what makes the
    /// sum unity there.
    #[test]
    fn neighbouring_bands_cross_six_db_down() {
        let mut crossover = Crossover::new(RATE);
        for (lower, hz) in EDGES.iter().enumerate().map(|(i, hz)| (i, *hz)) {
            for band in [lower, lower + 1] {
                let reading = band_db(&mut crossover, band, hz, RATE);
                assert!(
                    (reading - -6.0).abs() < 0.6,
                    "band {band} is {reading:.2} dB at its {hz} Hz edge"
                );
            }
        }
    }

    /// 4th order is 24 dB per octave once it is past the knee.
    ///
    /// **Measured where exactly one boundary is acting.** A band in the middle
    /// of the tree carries the high-passes of every split above it as well —
    /// band 5 an octave below its 6 kHz edge falls at 30 dB/oct, not 24,
    /// because `HP(1500)` is still on it. That is the tree working, not a
    /// filter of the wrong order, so it is not what this measures.
    #[test]
    fn the_skirts_fall_at_twenty_four_db_per_octave() {
        let mut crossover = Crossover::new(RATE);

        // Band 1 above its 120 Hz edge: `LP(120)` and nothing else.
        let near = band_db(&mut crossover, 0, 480.0, RATE);
        let far = band_db(&mut crossover, 0, 960.0, RATE);
        assert!(
            (near - far - 24.0).abs() < 1.5,
            "band 1 fell {near:.1} → {far:.1} dB, which is not 24 dB/oct"
        );

        // Band 2 below its 120 Hz edge: `HP(120)`, with `LP(400)` far enough
        // above to be flat here.
        let near = band_db(&mut crossover, 1, 60.0, RATE);
        let far = band_db(&mut crossover, 1, 30.0, RATE);
        assert!(
            (near - far - 24.0).abs() < 1.5,
            "band 2 fell {near:.1} → {far:.1} dB, which is not 24 dB/oct"
        );
    }

    #[test]
    fn focus_moves_every_edge_together_and_keeps_the_ratios() {
        let mut crossover = Crossover::new(RATE);
        let centre = crossover.edges();
        assert_eq!(centre, EDGES);

        for focus in [-1.0f32, -0.5, 0.25, 1.0] {
            crossover.set_focus(focus);
            let moved = crossover.edges();
            let expected = (focus * FOCUS_OCTAVES).exp2();

            for (base, edge) in centre.iter().zip(&moved) {
                assert!(
                    (edge / base - expected).abs() < 1e-3,
                    "focus {focus}: {base} → {edge}, wanted ×{expected}"
                );
            }
            // Which is the same as saying the shape of the split is unchanged.
            for window in 0..centre.len() - 1 {
                let before = centre[window + 1] / centre[window];
                let after = moved[window + 1] / moved[window];
                assert!(
                    (before - after).abs() < 1e-3,
                    "focus {focus} bent the ratios"
                );
            }
        }
    }

    /// The top edge fully open is 17 kHz, so at 44.1 kHz and above the ceiling
    /// never bites — and below it, it has to.
    /// **The picture and the sound come from one function** (`SPK-13`). An
    /// interface that mapped `FOCUS` to positions of its own would drift from
    /// the filters the first time either changed.
    #[test]
    fn the_public_edges_are_the_ones_the_filters_use() {
        for rate in [44_100.0f32, 48_000.0, 96_000.0] {
            let mut crossover = Crossover::new(rate);
            for focus in [-1.0f32, -0.5, 0.0, 0.27, 1.0] {
                crossover.set_focus(focus);
                assert_eq!(
                    crossover.edges(),
                    edges_for(focus, rate),
                    "{rate} Hz at focus {focus}"
                );
            }
        }
    }

    #[test]
    fn focus_cannot_push_an_edge_past_nyquist() {
        for rate in [22_050.0f32, 44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            let mut crossover = Crossover::new(rate);
            for focus in [-1.0f32, 0.0, 1.0] {
                crossover.set_focus(focus);
                for edge in crossover.edges() {
                    assert!(
                        edge > 0.0 && edge < rate * 0.5,
                        "{rate} Hz, focus {focus}: an edge sat at {edge}"
                    );
                }
            }
        }
        // And the ceiling is not vacuous: at 22.05 kHz it does bite.
        let mut low = Crossover::new(22_050.0);
        low.set_focus(1.0);
        let top = low.edges()[BAND_COUNT - 2];
        assert!(
            top < 6_000.0 * FOCUS_OCTAVES.exp2(),
            "the ceiling never applied: {top}"
        );
    }

    #[test]
    fn the_sum_stays_flat_at_every_sample_rate() {
        for rate in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
            let mut crossover = Crossover::new(rate);
            for hz in [
                20.0f32, 60.0, 200.0, 700.0, 2_000.0, 5_000.0, 12_000.0, 18_000.0,
            ] {
                let reading = sum_db(&mut crossover, hz, rate);
                assert!(
                    reading.abs() < 0.1,
                    "{rate} Hz rate, {hz} Hz tone: {reading:.3} dB"
                );
            }
        }
    }

    /// And flat wherever `FOCUS` puts the boundaries, not only at the centre.
    #[test]
    fn the_sum_stays_flat_at_every_focus() {
        for focus in [-1.0f32, -0.5, 0.5, 1.0] {
            let mut crossover = Crossover::new(RATE);
            crossover.set_focus(focus);
            for hz in [
                20.0f32, 60.0, 200.0, 700.0, 2_000.0, 5_000.0, 12_000.0, 18_000.0,
            ] {
                let reading = sum_db(&mut crossover, hz, RATE);
                assert!(
                    reading.abs() < 0.1,
                    "focus {focus}, {hz} Hz: {reading:.3} dB"
                );
            }
        }
    }

    /// **The coefficients belong to the block, not to the sample** (`SPK-2`).
    #[test]
    fn splitting_never_rebuilds_the_coefficients() {
        let mut crossover = Crossover::new(RATE);
        let built = crossover.rebuilds();

        for index in 0..1_000 {
            crossover.split((index as f32 * 0.01).sin());
        }
        assert_eq!(crossover.rebuilds(), built, "a sample rebuilt the filters");

        crossover.set_focus(0.5);
        assert_eq!(crossover.rebuilds(), built + 1);
        // The same value is not a change.
        crossover.set_focus(0.5);
        assert_eq!(crossover.rebuilds(), built + 1);
        // And the count can go up, or the two assertions above are vacuous.
        crossover.set_focus(-0.5);
        assert_eq!(crossover.rebuilds(), built + 2);
    }

    #[test]
    fn hostile_focus_neither_panics_nor_produces_nonsense() {
        for focus in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1e9,
            1e9,
            -1.0,
            1.0,
        ] {
            let mut crossover = Crossover::new(RATE);
            crossover.set_focus(focus);

            for edge in crossover.edges() {
                assert!(edge.is_finite() && edge > 0.0, "focus {focus}: edge {edge}");
            }
            for sample in tone(1.0, 1_000.0, RATE, 4_800) {
                for band in crossover.split(sample) {
                    assert!(band.is_finite(), "focus {focus} produced {band}");
                }
            }
        }
    }

    #[test]
    fn silence_stays_silent_and_reset_clears_the_state() {
        let mut crossover = Crossover::new(RATE);
        for sample in tone(1.0, 100.0, RATE, 4_800) {
            crossover.split(sample);
        }
        crossover.reset();
        for _ in 0..64 {
            for band in crossover.split(0.0) {
                assert_eq!(band, 0.0, "state survived the reset");
            }
        }
    }
}
