//! What frequencies are present, on a log axis.
//!
//! **A bank of band-pass filters, not an FFT.** The display this feeds has a
//! logarithmic frequency axis, and an FFT's bins are linear: resolving 20 Hz
//! would take a 4096-point transform — 85 ms of the display lagging the sound —
//! while still wasting most of its bins above 10 kHz where nothing needs them.
//! A log-spaced bank gives constant resolution *per octave*, which is the axis
//! it is drawn on, updates every sample instead of every block, needs no
//! windowing or block buffer, and adds no dependency.
//!
//! The cost is one biquad and one follower per band, per sample. Measured
//! against the CPU budget in `plugins/doubler/docs/implementation/doubler-plan.md`.

use std::f32::consts::TAU;

/// How fast a band rises to a new level, and how slowly it falls back. Meter
/// ballistics: quick enough to catch a transient, slow enough to read.
const ATTACK_SECONDS: f32 = 0.010;
const RELEASE_SECONDS: f32 = 0.250;

/// One band-pass. A biquad in direct form 1, with the constant-skirt-gain
/// coefficients from the RBJ cookbook.
#[derive(Clone, Copy, Default)]
struct BandPass {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BandPass {
    fn new(centre_hz: f32, q: f32, sample_rate: f32) -> Self {
        // Above Nyquist the coefficients stop meaning anything, so a band that
        // does not fit at this rate is built silent rather than unstable.
        if centre_hz >= sample_rate * 0.5 {
            return Self::default();
        }

        let w0 = TAU * centre_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha;

        Self {
            b0: alpha / a0,
            b1: 0.0,
            b2: -alpha / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            ..Self::default()
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// The level in each of `BANDS` log-spaced bands.
pub struct Spectrum<const BANDS: usize> {
    filters: [BandPass; BANDS],
    /// Smoothed squared level per band. Squared, so one `sqrt` happens on read
    /// rather than once per band per sample.
    energy: [f32; BANDS],
    low_hz: f32,
    high_hz: f32,
    attack: f32,
    release: f32,
}

impl<const BANDS: usize> Spectrum<BANDS> {
    /// Bands are spaced evenly in octaves between `low_hz` and `high_hz`, and
    /// each one's width is that spacing — so they meet rather than overlap or
    /// leave gaps.
    pub fn new(sample_rate: f32, low_hz: f32, high_hz: f32) -> Self {
        let octaves = (high_hz / low_hz).log2();
        let per_band = octaves / (BANDS - 1).max(1) as f32;
        // RBJ's bandwidth-to-Q, with the bandwidth being one band's spacing.
        let q = 1.0 / (2.0 * (std::f32::consts::LN_2 / 2.0 * per_band).sinh());

        Self {
            filters: std::array::from_fn(|index| {
                BandPass::new(Self::centre_at(index, low_hz, high_hz), q, sample_rate)
            }),
            energy: [0.0; BANDS],
            low_hz,
            high_hz,
            attack: Self::coefficient(ATTACK_SECONDS, sample_rate),
            release: Self::coefficient(RELEASE_SECONDS, sample_rate),
        }
    }

    /// Feeds one sample. Mono in: a spectrum of "what is going through" does not
    /// gain anything from being told twice, and the caller can sum first.
    pub fn push(&mut self, sample: f32) {
        for (filter, energy) in self.filters.iter_mut().zip(&mut self.energy) {
            let band = filter.process(sample);
            let squared = band * band;
            // Rising fast and falling slowly is what makes a meter readable;
            // one coefficient would either miss transients or smear them.
            let coefficient = if squared > *energy {
                self.attack
            } else {
                self.release
            };
            *energy += (squared - *energy) * coefficient;
        }
    }

    /// The amplitude in each band, low to high. **Not in dB** — the caller owns
    /// the axis it is drawing onto and the floor it wants to show.
    pub fn levels(&self) -> [f32; BANDS] {
        std::array::from_fn(|index| self.energy[index].max(0.0).sqrt())
    }

    /// Where a band sits, so a caller can place it on an axis of its own rather
    /// than assuming the two ranges match.
    pub fn centre_hz(&self, index: usize) -> f32 {
        Self::centre_at(index, self.low_hz, self.high_hz)
    }

    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
        self.energy = [0.0; BANDS];
    }

    fn centre_at(index: usize, low_hz: f32, high_hz: f32) -> f32 {
        let steps = (BANDS - 1).max(1) as f32;
        low_hz * (high_hz / low_hz).powf(index as f32 / steps)
    }

    /// A one-pole coefficient from a time constant, so the ballistics are the
    /// same wall-clock speed at every sample rate.
    fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
        1.0 - (-1.0 / (seconds * sample_rate)).exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn sine(spectrum: &mut Spectrum<16>, hz: f32, seconds: f32) {
        let samples = (SR * seconds) as usize;
        for n in 0..samples {
            spectrum.push((TAU * hz * n as f32 / SR).sin());
        }
    }

    fn loudest(spectrum: &Spectrum<16>) -> usize {
        spectrum
            .levels()
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap()
    }

    #[test]
    fn the_bands_span_the_range_in_order() {
        let spectrum: Spectrum<16> = Spectrum::new(SR, 20.0, 20_000.0);
        assert!((spectrum.centre_hz(0) - 20.0).abs() < 0.1);
        assert!((spectrum.centre_hz(15) - 20_000.0).abs() < 1.0);
        for index in 1..16 {
            assert!(spectrum.centre_hz(index) > spectrum.centre_hz(index - 1));
        }
    }

    /// The whole point: a tone shows up where it is, not somewhere else.
    #[test]
    fn a_tone_lights_its_own_band() {
        for band in [3usize, 7, 11] {
            let mut spectrum: Spectrum<16> = Spectrum::new(SR, 20.0, 20_000.0);
            let hz = spectrum.centre_hz(band);
            sine(&mut spectrum, hz, 0.4);
            assert_eq!(loudest(&spectrum), band, "a {hz} Hz tone landed wrong");
        }
    }

    #[test]
    fn a_tone_does_not_light_a_distant_band() {
        let mut spectrum: Spectrum<16> = Spectrum::new(SR, 20.0, 20_000.0);
        let hz = spectrum.centre_hz(7);
        sine(&mut spectrum, hz, 0.4);

        let levels = spectrum.levels();
        assert!(
            levels[1] < levels[7] * 0.1,
            "a {hz} Hz tone reached band 1: {levels:?}"
        );
    }

    /// The follower smooths *energy*, and the level is its square root, so the
    /// level falls at half the rate the time constant names. Two seconds is
    /// eight release constants — `e^-8` of the energy, which is under 2% of the
    /// amplitude.
    #[test]
    fn it_falls_back_to_nothing() {
        let mut spectrum: Spectrum<16> = Spectrum::new(SR, 20.0, 20_000.0);
        let hz = spectrum.centre_hz(7);
        sine(&mut spectrum, hz, 0.4);
        let loud = spectrum.levels()[7];

        for _ in 0..(SR as usize * 2) {
            spectrum.push(0.0);
        }
        let quiet = spectrum.levels()[7];
        assert!(quiet < loud * 0.05, "it kept ringing: {loud} -> {quiet}");
    }

    /// Above Nyquist there is nothing to measure, and the coefficients would be
    /// nonsense. The band has to be silent rather than unstable.
    #[test]
    fn bands_above_nyquist_are_silent_rather_than_wild() {
        let mut spectrum: Spectrum<16> = Spectrum::new(8_000.0, 20.0, 20_000.0);
        for n in 0..8_000 {
            spectrum.push((TAU * 1_000.0 * n as f32 / 8_000.0).sin());
        }
        for level in spectrum.levels() {
            assert!(level.is_finite(), "a band blew up");
        }
        assert_eq!(spectrum.levels()[15], 0.0, "a band above Nyquist responded");
    }

    #[test]
    fn reset_clears_it() {
        let mut spectrum: Spectrum<16> = Spectrum::new(SR, 20.0, 20_000.0);
        sine(&mut spectrum, 1_000.0, 0.2);
        spectrum.reset();
        assert_eq!(spectrum.levels(), [0.0; 16]);
    }
}
