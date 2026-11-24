//! Everything Pumice does to a signal, in one object the wrapper drives.
//!
//! **The ear-tuned constants live here** ([`Settings`]), in one block rather
//! than scattered through the modules that use them. `PUM-11` settles all of
//! them; the specification's "耳で詰める定数" table and this struct must agree
//! (`../docs/specifications/dsp.md`).
//!
//! **`STATIC` only, at this unit** (`PUM-3`). The long-term map that decides
//! *where* resonance lives is `PUM-4`; what runs here is the short-term
//! follower alone, which is what soothe does and what `MODE` will offer as the
//! way out when the adaptive path is wrong for a piece of material.

use crate::display::CURVE_POINTS;
use crate::gain::{Computer, Follower, smooth_into};
use crate::nodes::{NODES, Node, Range, weight_into};

use crate::quality::Quality;
use crate::reference::{Scratch, Widths, excess_db_into, power_into};
use crate::smoothing::Prefix;
use crate::stft::{MAX_BINS, MAX_CHANNELS, Stft};
use nxe_audio::DelayLine;

/// How many samples the dry path is carried through at a time.
///
/// **The buffer a host hands over is replaced in place by the transform**, so
/// the dry copy has to be taken first and read back afterwards at an offset
/// that depends on where in the buffer it was. Working a fixed chunk at a time
/// bounds that offset — and therefore the delay line — without asking the host
/// how big its buffers get. The transform is sample-exact whatever it is fed
/// (`REQ-PUM-017`), so chunking changes nothing about the output.
const CHUNK: usize = 256;

/// `DelayLine::at(1)` is the most recently written sample, so a delay of `L`
/// samples is read at `L + 1`.
const READ_OFFSET: usize = 1;

/// The product's tuning: everything an ear decides rather than a requirement.
///
/// A `const` rather than a set of arguments, so what Pumice is reads as one
/// block (`nxe_audio::guard::Settings` is the same shape for the same reason).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// A **band-to-reference ratio**, not a level.
    ///
    /// **Measured, not guessed** (`SPK-18` is the same procedure). It was zero,
    /// on the reasoning that ordinary material carries as much power as its
    /// neighbourhood and so reads no excess. Ordinary material does not: a
    /// single bin of a noise-like signal deviates from its own mean, and an
    /// asymmetric follower rectifies the positive half of that into a standing
    /// excess. What things actually read, at `SHARPNESS` and `SPEED` centred:
    ///
    /// | | worst | mean |
    /// |---|---|---|
    /// | white noise | 5.0 dB | 1.0 dB |
    /// | a sung sawtooth's partials | 5.3 dB | 0.1 dB |
    /// | **an 18 dB resonance, Q 4** | **13 dB** | |
    ///
    /// 4.5 puts ordinary material at exactly nothing and still takes 7.8 dB off
    /// the resonance. Zero took 0.71 dB out of plain noise — which is the
    /// defect `SPK-18` shipped and this number exists to prevent.
    pub threshold_db: f32,
    /// How many dB of reduction per dB of excess, at `DEPTH` fully up.
    ///
    /// **Three, so that the knob can go too far.** One flattens a bin onto its
    /// reference, which sounds like restraint rather than like a maximum — and
    /// a control whose top end is still tasteful gives nobody a way to find
    /// where the useful part ends.
    ///
    /// It went 0.7 → 1.0 → 2.0 → 3.0, each step because an ear said the top of
    /// the knob was still polite. **Four is where it stops being a knob**: with
    /// the ceiling raised to match, an 18 dB resonance loses 29.8 of a possible
    /// 30, so the last part of the travel does nothing.
    ///
    /// **Raising this does not touch ordinary material.** The threshold is what
    /// protects that, and the slope only scales what is already above it;
    /// measured, plain noise moves 0.01 dB at every slope from one to three.
    /// Three is too far for a different reason — two of three test resonances
    /// hit [`Settings::ceiling_db`], so the top of the knob stops doing
    /// anything.
    ///
    /// It was 0.7 before that, which put a second ceiling inside a control that
    /// already had one.
    pub slope: f32,
    /// The widest any bin may be pulled. Past this a vocal is not protected,
    /// it is missing (`REQ-PUM-023`).
    ///
    /// **24, up from 18** (`PUM-10b`). It has to stay ahead of what
    /// [`Settings::slope`] can ask for or the top of `DEPTH` dies against it —
    /// at slope 3 an 18 dB resonance asks for 23.8. This is looser protection
    /// than 18 was, and it is only reachable at `DEPTH` fully up on a bin the
    /// adaptive gate has opened, which takes a genuinely standing resonance.
    pub ceiling_db: f32,
    /// How much of a bin's neighbourhood is averaged before it is compared to
    /// anything.
    ///
    /// **Not a resolution setting — a variance one** (`reference`). One bin of
    /// a noise-like signal deviates by 5.6 dB from its own mean, and an
    /// asymmetric follower turns that into a standing excess out of nothing.
    pub detail_octaves: f32,
    /// What `SHARPNESS` interpolates between, in octaves: **how wide a thing
    /// counts as one feature**. Wide catches broad humps, narrow only spikes.
    pub feature_wide_octaves: f32,
    pub feature_narrow_octaves: f32,
    /// How thick the reference ring outside the feature is.
    ///
    /// **The hole in the middle is what makes the excess honest**
    /// (`reference::excess_db_into`); this is how much spectrum is left to
    /// judge against once the hole is cut.
    pub reference_margin_octaves: f32,
    /// What `SPEED` interpolates between. The fast end is bounded by the hop
    /// whatever is asked for (`REQ-PUM-020`).
    pub attack_slow_seconds: f32,
    pub attack_fast_seconds: f32,
    pub release_slow_seconds: f32,
    pub release_fast_seconds: f32,
    /// **Not reachable from any control** (`REQ-PUM-005`). The floor that
    /// keeps the reconstruction from warbling.
    pub gain_smoothing_octaves: f32,
    /// How wide each edge of the operating range fades, in octaves, centred on
    /// the edge. **Where the edges are is a control** (`nodes::Range`); how
    /// softly they fade is not.
    pub edge_octaves: f32,
}

impl Settings {
    /// `dsp.md`'s "耳で詰める定数". **None of these has been measured or
    /// listened to** — they are the design-time values, and `PUM-11` replaces
    /// them.
    pub const DEFAULT: Settings = Settings {
        threshold_db: 4.5,
        slope: 3.0,
        ceiling_db: 24.0,
        detail_octaves: 1.0 / 12.0,
        feature_wide_octaves: 1.5,
        feature_narrow_octaves: 0.15,
        reference_margin_octaves: 1.0,
        attack_slow_seconds: 0.040,
        attack_fast_seconds: 0.005,
        release_slow_seconds: 0.400,
        release_fast_seconds: 0.040,
        gain_smoothing_octaves: 1.0 / 12.0,
        edge_octaves: 0.5,
    };
}

/// What the host's controls are worth this block.
#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// `0..=1`. Zero is exactly nothing (`REQ-PUM-002`).
    pub depth: f32,
    /// `0..=1`. Zero is the wide reference, one the narrow.
    pub sharpness: f32,
    /// `0..=1`. Zero is the slow end.
    pub speed: f32,
    /// How far above its neighbourhood a bin has to sit before anything
    /// happens, in dB. **The measured default is
    /// [`Settings::threshold_db`]**; this is the control over it.
    pub threshold_db: f32,
    /// `0..=1`. Zero is the dry path alone (`REQ-PUM-012`).
    pub mix: f32,
    /// A **linear** gain, applied last.
    pub output: f32,
    /// Listen to what is being taken out, and nothing else (`REQ-PUM-019`).
    pub delta: bool,
    pub quality: Quality,
    /// Where the reduction is allowed to go (`REQ-PUM-004`).
    pub nodes: [Node; NODES],
    /// The band the plugin works in at all.
    pub range: Range,
}

/// The detection and the gain, with no transform of its own.
///
/// Separate from [`Engine`] so that the per-frame work can borrow it while
/// [`Stft`] is borrowed too — one `&mut self` closure over the whole engine
/// would not compile.
struct Detector {
    prefix: Prefix,
    power: Vec<f32>,
    /// [`Detector::power`] averaged over [`Settings::detail_octaves`].
    detail: Vec<f32>,
    reference: Vec<f32>,
    excess_db: Vec<f32>,
    drive_db: Vec<f32>,
    reduction_db: Vec<f32>,
    smoothed_db: Vec<f32>,
    gain: Vec<f32>,
    /// The nodes and the operating range as a per-bin weight. Rebuilt when one
    /// of them moves, never per frame.
    weight: Vec<f32>,
    follower: Follower,
    computer: Computer,
    settings: Settings,
    depth: f32,
    threshold_db: f32,
    feature_octaves: f32,
    bins: usize,
    /// Scratch for handing the channels' spectra to [`power_into`] without
    /// borrowing them mutably.
    channels: usize,
}

impl Detector {
    fn new(settings: Settings) -> Self {
        Self {
            prefix: Prefix::new(MAX_BINS),
            power: vec![0.0; MAX_BINS],
            detail: vec![0.0; MAX_BINS],
            reference: vec![0.0; MAX_BINS],
            excess_db: vec![0.0; MAX_BINS],
            drive_db: vec![0.0; MAX_BINS],
            reduction_db: vec![0.0; MAX_BINS],
            smoothed_db: vec![0.0; MAX_BINS],
            gain: vec![1.0; MAX_BINS],
            weight: vec![0.0; MAX_BINS],
            follower: Follower::new(MAX_BINS),
            computer: Computer {
                threshold_db: settings.threshold_db,
                slope: settings.slope,
                ceiling_db: settings.ceiling_db,
            },
            settings,
            depth: 0.0,
            threshold_db: settings.threshold_db,
            feature_octaves: settings.feature_wide_octaves,
            bins: 0,
            channels: 0,
        }
    }

    fn run(&mut self, frame: &mut crate::stft::Frame<'_>) {
        let bins = frame.bins();
        let channels = frame.channels();
        self.bins = bins;
        self.channels = channels;

        {
            // `Frame::read` hands out shared slices, which is what lets both
            // channels be measured before either is written.
            let left = frame.read(0);
            let right = frame.read(1);
            let spectra: [&[realfft::num_complex::Complex<f32>]; MAX_CHANNELS] = [left, right];
            power_into(&spectra[..channels], &mut self.power[..bins]);
        }

        excess_db_into(
            &self.power[..bins],
            Scratch {
                prefix: &mut self.prefix,
                detail: &mut self.detail[..bins],
                reference: &mut self.reference[..bins],
            },
            Widths {
                detail_octaves: self.settings.detail_octaves,
                feature_octaves: self.feature_octaves,
                margin_octaves: self.settings.reference_margin_octaves,
            },
            &mut self.excess_db[..bins],
        );

        self.follower
            .follow(&self.excess_db[..bins], &mut self.drive_db[..bins]);

        self.computer.reduction_db_into(
            &self.drive_db[..bins],
            &self.weight[..bins],
            self.depth,
            &mut self.reduction_db[..bins],
        );

        smooth_into(
            &self.reduction_db[..bins],
            &mut self.prefix,
            &mut self.smoothed_db[..bins],
            self.settings.gain_smoothing_octaves,
            &mut self.gain[..bins],
        );

        for channel in 0..channels {
            for (bin, value) in frame.channel(channel).iter_mut().enumerate() {
                *value *= self.gain[bin];
            }
        }
    }
}

/// The two **measured** curves the figure draws.
///
/// **The weight is not here.** It is what the user set, it has an exact value
/// at every frequency, and a figure that read it back off the engine's bins
/// drew stairs below 300 Hz (`nodes::weight_at`). The window samples it
/// directly instead, which is also 128 fewer atomics a block.
///
/// Held by the caller rather than returned, so publishing a frame allocates
/// nothing (`REQ-PUM-016`).
#[derive(Clone, Copy, Debug)]
pub struct Curves {
    /// Input power, in dB, floored rather than allowed to reach negative
    /// infinity.
    pub spectrum_db: [f32; CURVE_POINTS],
    /// What is being taken out, in dB. Negative.
    pub reduction_db: [f32; CURVE_POINTS],
}

impl Default for Curves {
    fn default() -> Self {
        Self {
            spectrum_db: [0.0; CURVE_POINTS],
            reduction_db: [0.0; CURVE_POINTS],
        }
    }
}

/// The whole of Pumice, driven a buffer at a time.
pub struct Engine {
    stft: Stft,
    detector: Detector,
    /// The untouched input, held back to line up with the transform's output.
    ///
    /// **Not for transparency — for `MIX`** (`REQ-PUM-001`). Without it the dry
    /// and the wet are a whole window apart and the crossfade combs.
    dry: [DelayLine; MAX_CHANNELS],
    mix: f32,
    output: f32,
    delta: bool,
    sample_rate: f32,
    /// What [`Detector::weight`] was last built for, so a rebuild only happens
    /// when something actually moved.
    weight_bins: usize,
    weight_nodes: [Node; NODES],
    weight_range: Range,
}

impl Engine {
    /// **The only place that allocates** (`REQ-PUM-016`).
    pub fn new(sample_rate: f32, settings: Settings) -> Self {
        let longest = crate::quality::max_latency(sample_rate) + CHUNK + READ_OFFSET + 1;
        let mut engine = Self {
            stft: Stft::new(sample_rate, Quality::default()),
            detector: Detector::new(settings),
            dry: std::array::from_fn(|_| DelayLine::new(sample_rate, longest as f32 / sample_rate)),
            mix: 1.0,
            output: 1.0,
            delta: false,
            sample_rate,
            weight_bins: 0,
            weight_nodes: [Node::default(); NODES],
            weight_range: Range::default(),
        };
        engine.rebuild_weight();
        engine
    }

    pub fn latency(&self) -> usize {
        self.stft.latency()
    }

    pub fn reset(&mut self) {
        self.stft.reset();
        self.detector.follower.reset();
        for line in &mut self.dry {
            line.reset();
        }
    }

    /// What is being taken out, per bin, in dB — the figure's subject
    /// (`REQ-PUM-018`) and what makes a reduction measurable at one frequency
    /// rather than only as a change in level.
    pub fn reduction_curve(&self) -> &[f32] {
        &self.detector.smoothed_db[..self.detector.bins]
    }

    /// The measured curves the figure draws, on the figure's own logarithmic
    /// axis (`REQ-PUM-018`).
    ///
    /// **The reduction is the gain the audio actually got**, not a second
    /// calculation from the same parameters — a figure computed separately
    /// agrees with the sound only until one of them is changed.
    pub fn curves(&self, into: &mut Curves) {
        let bins = self.detector.bins;
        let bin_hz = self.sample_rate / self.stft.block() as f32;

        crate::display::resample_power_db_into(
            &self.detector.power[..bins],
            bin_hz,
            crate::display::full_scale(self.stft.block()),
            &mut into.spectrum_db,
        );
        crate::display::resample_into(
            &self.detector.smoothed_db[..bins],
            bin_hz,
            &mut into.reduction_db,
        );
    }

    /// Which bin a frequency lands in, for a caller reading
    /// [`Engine::reduction_curve`].
    pub fn bin_of(&self, hz: f32) -> usize {
        let block = self.stft.block();
        ((hz * block as f32 / self.sample_rate).round() as usize).min(block / 2)
    }

    /// Once per block. Everything derived from a control is resolved here so
    /// that the per-frame path is arithmetic only.
    pub fn set(&mut self, controls: Controls) {
        let settings = self.detector.settings;

        self.stft.set_quality(controls.quality);
        if self.stft.block() / 2 + 1 != self.weight_bins
            || controls.nodes != self.weight_nodes
            || controls.range != self.weight_range
        {
            self.weight_nodes = controls.nodes;
            self.weight_range = controls.range;
            self.rebuild_weight();
        }

        self.detector.depth = controls.depth.clamp(0.0, 1.0);
        // **The threshold is a control now** (`PUM-10c`). It was a measured
        // constant and it still has a measured default, but "how easily does it
        // react" is the question a user asks about this plugin first and there
        // was nowhere to answer it.
        self.detector.threshold_db = controls.threshold_db;
        self.detector.computer.threshold_db = controls.threshold_db;
        self.mix = controls.mix.clamp(0.0, 1.0);
        self.output = controls.output;
        self.delta = controls.delta;

        // Interpolated **in the log domain**: the octave width is a ratio, so
        // half a turn should land on the geometric middle.
        let sharpness = controls.sharpness.clamp(0.0, 1.0);
        self.detector.feature_octaves = settings.feature_wide_octaves
            * (settings.feature_narrow_octaves / settings.feature_wide_octaves).powf(sharpness);

        let speed = controls.speed.clamp(0.0, 1.0);
        let attack = settings.attack_slow_seconds
            * (settings.attack_fast_seconds / settings.attack_slow_seconds).powf(speed);
        let release = settings.release_slow_seconds
            * (settings.release_fast_seconds / settings.release_slow_seconds).powf(speed);

        // **The hop is the clock** — a bin is looked at once per frame.
        let frame_rate = self.sample_rate / (self.stft.block() / crate::quality::OVERLAP) as f32;
        self.detector.follower.set(attack, release, frame_rate);
    }

    pub fn process(&mut self, channels: &mut [&mut [f32]]) {
        let used = channels.len().min(MAX_CHANNELS);
        let Some(samples) = channels[..used].iter().map(|c| c.len()).min() else {
            return;
        };
        let latency = self.stft.latency();

        let mut start = 0;
        while start < samples {
            let length = CHUNK.min(samples - start);

            // The dry copy has to be taken before the transform overwrites it.
            for (line, buffer) in self.dry.iter_mut().zip(channels[..used].iter()) {
                for sample in &buffer[start..start + length] {
                    line.write(*sample);
                }
            }

            {
                let Self { stft, detector, .. } = self;
                let mut chunk: [&mut [f32]; MAX_CHANNELS] = [&mut [], &mut []];
                for (slot, buffer) in chunk.iter_mut().zip(channels[..used].iter_mut()) {
                    *slot = &mut buffer[start..start + length];
                }
                stft.process(&mut chunk[..used], |frame| detector.run(frame));
            }

            let (mix, output, delta) = (self.mix, self.output, self.delta);
            for (line, buffer) in self.dry.iter().zip(channels[..used].iter_mut()) {
                for (offset, sample) in buffer[start..start + length].iter_mut().enumerate() {
                    // The input at this position is `length − offset` writes
                    // back from the head; the one `latency` before it is that
                    // much further.
                    let dry = line.read_whole(latency + length - offset);
                    let wet = *sample;

                    // **`DELTA` is before `MIX`** (`REQ-PUM-019`): the point is
                    // to hear what is being taken out, and `MIX` would dilute
                    // exactly that.
                    *sample = if delta {
                        wet - dry
                    } else {
                        dry + (wet - dry) * mix
                    } * output;
                }
            }

            start += length;
        }
    }

    /// The largest reduction any bin is taking, in dB, for the readout
    /// (`REQ-PUM-018`).
    pub fn reduction_db(&self) -> f32 {
        self.detector.smoothed_db[..self.detector.bins]
            .iter()
            .fold(0.0_f32, |worst, value| worst.min(*value))
    }

    fn rebuild_weight(&mut self) {
        let block = self.stft.block();
        let bins = block / 2 + 1;
        let edge = self.detector.settings.edge_octaves;
        weight_into(
            bins,
            self.sample_rate / block as f32,
            &self.weight_nodes,
            self.weight_range,
            edge,
            &mut self.detector.weight[..bins],
        );
        self.weight_bins = bins;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(length: usize) -> Vec<f32> {
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        (0..length)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 40) as f32 / 8_388_608.0 - 1.0
            })
            .collect()
    }

    fn tone(length: usize, hz: f32, rate: f32, amplitude: f32) -> Vec<f32> {
        (0..length)
            .map(|n| amplitude * (std::f32::consts::TAU * hz * n as f32 / rate).sin())
            .collect()
    }

    fn run(engine: &mut Engine, input: &[f32]) -> Vec<f32> {
        let mut output = input.to_vec();
        for piece in output.chunks_mut(512) {
            let mut channels = [piece];
            engine.process(&mut channels);
        }
        output
    }

    /// A sawtooth on a **sung line**: the pitch steps to a new scale degree
    /// every `note_seconds`, over an octave.
    fn sung_saw(length: usize, rate: f32, low_hz: f32, note_seconds: f32) -> Vec<f32> {
        const STEPS: [f32; 5] = [0.0, 2.0, 4.0, 7.0, 9.0];
        let per_note = (rate * note_seconds) as usize;
        let mut phase = 0.0_f32;

        (0..length)
            .map(|n| {
                let note = n / per_note.max(1);
                let walk = (note * 7 + note * note * 3) % 15;
                let semitones = STEPS[walk % 5] + 12.0 * (walk / 5) as f32;
                let hz = low_hz * (semitones / 12.0).exp2();
                phase += hz / rate;
                phase -= phase.floor();
                (phase * 2.0 - 1.0) * 0.3
            })
            .collect()
    }

    fn controls(depth: f32) -> Controls {
        Controls {
            depth,
            sharpness: 0.5,
            speed: 0.5,
            quality: Quality::Normal,
            threshold_db: Settings::DEFAULT.threshold_db,
            mix: 1.0,
            output: 1.0,
            delta: false,
            nodes: [Node::default(); NODES],
            range: Range::default(),
        }
    }

    /// The largest jump between neighbouring samples.
    fn worst_step(samples: &[f32]) -> f32 {
        samples
            .windows(2)
            .fold(0.0_f32, |worst, pair| worst.max((pair[1] - pair[0]).abs()))
    }

    /// The average reduction across the band, in dB (negative).
    fn mean_reduction_db(engine: &Engine, low_hz: f32, high_hz: f32) -> f32 {
        let curve = engine.reduction_curve();
        let (low, high) = (engine.bin_of(low_hz), engine.bin_of(high_hz));
        curve[low..high].iter().sum::<f32>() / (high - low) as f32
    }

    /// The deepest reduction anywhere in the band, in dB (negative).
    fn deepest_reduction_db(engine: &Engine, low_hz: f32, high_hz: f32) -> f32 {
        let curve = engine.reduction_curve();
        let (low, high) = (engine.bin_of(low_hz), engine.bin_of(high_hz));
        curve[low..high].iter().fold(0.0_f32, |a, b| a.min(*b))
    }

    fn rms(samples: &[f32]) -> f32 {
        let total: f64 = samples.iter().map(|s| f64::from(*s) * f64::from(*s)).sum();
        (total / samples.len() as f64).sqrt() as f32
    }

    #[test]
    fn depth_zero_reconstructs_the_input() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(0.0));

        let input = noise(2048 * 6);
        let output = run(&mut engine, &input);

        let latency = engine.latency();
        let mut worst: f32 = 0.0;
        for index in (2048 * 2)..input.len() {
            worst = worst.max((output[index] - input[index - latency]).abs());
        }
        let error = 20.0 * worst.max(f32::MIN_POSITIVE).log10();
        assert!(error <= -120.0, "worst sample is {error:.1} dB");
    }

    /// **`REQ-PUM-003`'s acceptance condition, and the `SPK-18` regression.**
    /// Pink-ish broadband material must come out untouched at the shipped
    /// threshold — Sparkleur shipped pulling 1.3 dB out of exactly this.
    #[test]
    fn ordinary_material_is_left_alone() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));

        let input = noise(2048 * 10);
        let output = run(&mut engine, &input);

        let settled = 2048 * 4;
        let before = rms(&input[settled..input.len() - engine.latency()]);
        let after = rms(&output[settled + engine.latency()..]);
        let change_db = 20.0 * (after / before).log10();

        assert!(
            change_db.abs() < 0.1,
            "flat noise lost {change_db:.2} dB — the threshold is pulling on ordinary material"
        );
    }

    /// The product working: a tone standing well above its neighbourhood comes
    /// down, and it comes down further as `DEPTH` rises.
    #[test]
    fn a_resonance_comes_down_and_depth_moves_it() {
        let rate = 48_000.0;
        let length = 2048 * 12;
        let mut input = noise(length);
        for (sample, extra) in input.iter_mut().zip(tone(length, 2_500.0, rate, 0.9)) {
            *sample = *sample * 0.05 + extra;
        }

        let mut previous = f32::INFINITY;
        for depth in [0.0_f32, 0.25, 0.5, 1.0] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(depth));
            let output = run(&mut engine, &input);
            let level = rms(&output[2048 * 6..]);

            assert!(
                level < previous,
                "depth {depth} did not reduce further ({level} vs {previous})"
            );
            previous = level;
        }
    }

    /// `REQ-PUM-003`: the reduction must not move with input gain.
    #[test]
    fn input_gain_does_not_change_the_reduction() {
        let rate = 48_000.0;
        let length = 2048 * 12;
        let mut base = noise(length);
        for (sample, extra) in base.iter_mut().zip(tone(length, 2_500.0, rate, 0.9)) {
            *sample = *sample * 0.05 + extra;
        }

        let mut reductions = Vec::new();
        for gain_db in [-12.0_f32, 0.0, 12.0] {
            let gain = 10.0_f32.powf(gain_db / 20.0);
            let scaled: Vec<f32> = base.iter().map(|sample| sample * gain).collect();

            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(1.0));
            let output = run(&mut engine, &scaled);

            let settled = 2048 * 6;
            let before = rms(&scaled[settled..scaled.len() - engine.latency()]);
            let after = rms(&output[settled + engine.latency()..]);
            reductions.push(20.0 * (after / before).log10());
        }

        let span = reductions.iter().fold(f32::MIN, |a, b| a.max(*b))
            - reductions.iter().fold(f32::MAX, |a, b| a.min(*b));
        assert!(
            span < 0.2,
            "reduction moved {span:.3} dB across ±12 dB: {reductions:?}"
        );
    }

    /// `REQ-PUM-017`: the host's block size must not reach the output.
    #[test]
    fn the_host_block_size_does_not_change_the_output() {
        let rate = 48_000.0;
        let input = noise(2048 * 5);

        let mut reference = Vec::new();
        for chunk in [512, 1, 64, 4096] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(1.0));
            let mut output = input.clone();
            for piece in output.chunks_mut(chunk) {
                let mut channels = [piece];
                engine.process(&mut channels);
            }
            if reference.is_empty() {
                reference = output;
            } else {
                assert_eq!(output, reference, "block size {chunk}");
            }
        }
    }

    /// Extreme input must not produce a non-finite sample or a panic
    /// (`REQ-PUM-016`).
    #[test]
    fn extreme_input_stays_finite() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));

        let mut input = vec![0.0; 2048 * 4];
        input[100] = 1e6;
        input[200] = -1e6;
        let output = run(&mut engine, &input);
        assert!(output.iter().all(|sample| sample.is_finite()));
    }

    #[test]
    fn a_node_steers_the_reduction() {
        use nxe_audio::biquad::{Biquad, Coefficients};
        let rate = 48_000.0;
        let hz = 2_500.0;

        let mut input = noise(rate as usize * 12);
        let mut filter = Biquad::new(Coefficients::peaking(hz, 4.0, 18.0, rate));
        for sample in input.iter_mut() {
            *sample = filter.process(*sample) * 0.3;
        }

        let mut protecting = [Node::default(); NODES];
        protecting[0] = Node {
            enabled: true,
            freq_hz: hz,
            width_octaves: 1.0,
            depth: -1.0,
        };

        let plain = {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls(1.0));
            run(&mut engine, &input);
            engine.reduction_curve()[engine.bin_of(hz)]
        };
        let protected = {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(Controls {
                nodes: protecting,
                ..controls(1.0)
            });
            run(&mut engine, &input);
            engine.reduction_curve()[engine.bin_of(hz)]
        };

        assert!(plain < -6.0, "the resonance was not found: {plain:.2} dB");
        assert!(
            protected > -0.5,
            "a protecting node still let {protected:.2} dB through"
        );

        // And with `DEPTH` at zero the nodes cannot do anything at all.
        let mut deepening = protecting;
        deepening[0].depth = 1.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(Controls {
            nodes: deepening,
            ..controls(0.0)
        });
        let output = run(&mut engine, &input);
        let latency = engine.latency();
        let mut worst: f32 = 0.0;
        for index in (2048 * 4)..input.len() {
            worst = worst.max((output[index] - input[index - latency]).abs());
        }
        let error = 20.0 * worst.max(f32::MIN_POSITIVE).log10();
        assert!(error <= -120.0, "DEPTH zero with nodes moved {error:.1} dB");
    }

    fn run_with(engine: &mut Engine, controls: Controls, input: &[f32]) -> Vec<f32> {
        engine.set(controls);
        let mut output = input.to_vec();
        for piece in output.chunks_mut(512) {
            let mut channels = [piece];
            engine.process(&mut channels);
        }
        output
    }

    fn difference_db(output: &[f32], reference: &[f32], skip: usize) -> f32 {
        let mut worst: f32 = 0.0;
        for index in skip..reference.len() {
            worst = worst.max((output[index] - reference[index]).abs());
        }
        20.0 * worst.max(f32::MIN_POSITIVE).log10()
    }

    /// `REQ-PUM-012`: at `MIX` zero the output is the input, held back by the
    /// reported latency and nothing else.
    #[test]
    fn mix_zero_is_the_delayed_input() {
        let rate = 48_000.0;
        let input = noise(2048 * 8);
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        let output = run_with(
            &mut engine,
            Controls {
                mix: 0.0,
                ..controls(1.0)
            },
            &input,
        );

        let latency = engine.latency();
        let delayed: Vec<f32> = std::iter::repeat_n(0.0, latency)
            .chain(input.iter().copied())
            .take(input.len())
            .collect();
        let error = difference_db(&output, &delayed, latency);
        assert!(error <= -120.0, "MIX zero differs by {error:.1} dB");
    }

    /// The dry path is aligned to the sample, not approximately — an impulse
    /// must arrive exactly where the host was told it would (`REQ-PUM-001`).
    #[test]
    fn the_dry_path_is_delayed_by_the_reported_latency() {
        let rate = 48_000.0;
        for quality in Quality::ALL {
            let block = quality.block(rate);
            let mut input = vec![0.0; block * 4];
            input[block] = 1.0;

            let mut engine = Engine::new(rate, Settings::DEFAULT);
            let output = run_with(
                &mut engine,
                Controls {
                    mix: 0.0,
                    quality,
                    ..controls(1.0)
                },
                &input,
            );

            let peak = output
                .iter()
                .position(|sample| sample.abs() > 0.5)
                .expect("the impulse came out");
            assert_eq!(peak, block + quality.latency(rate), "{quality:?}");
        }
    }

    /// `REQ-PUM-019`: what `DELTA` plays plus what the plugin plays is the wet
    /// path, so nothing is hidden in either.
    #[test]
    fn delta_and_the_output_add_back_to_the_wet_path() {
        let rate = 48_000.0;
        let input = noise(2048 * 8);
        let base = controls(1.0);

        let wet = run_with(&mut Engine::new(rate, Settings::DEFAULT), base, &input);
        let dry = run_with(
            &mut Engine::new(rate, Settings::DEFAULT),
            Controls { mix: 0.0, ..base },
            &input,
        );
        let delta = run_with(
            &mut Engine::new(rate, Settings::DEFAULT),
            Controls {
                delta: true,
                ..base
            },
            &input,
        );

        let sum: Vec<f32> = dry.iter().zip(&delta).map(|(a, b)| a + b).collect();
        let error = difference_db(&sum, &wet, 2048 * 3);
        assert!(
            error <= -110.0,
            "dry + delta differs from wet by {error:.1} dB"
        );
    }

    /// `REQ-PUM-019`: nothing is being taken out, so there is nothing to hear.
    #[test]
    fn delta_is_silent_at_depth_zero() {
        let rate = 48_000.0;
        let input = noise(2048 * 8);
        let output = run_with(
            &mut Engine::new(rate, Settings::DEFAULT),
            Controls {
                delta: true,
                ..controls(0.0)
            },
            &input,
        );

        let worst = output[2048 * 3..]
            .iter()
            .fold(0.0_f32, |a, b| a.max(b.abs()));
        let level = 20.0 * worst.max(f32::MIN_POSITIVE).log10();
        assert!(level <= -120.0, "DELTA at DEPTH zero plays {level:.1} dB");
    }

    /// `OUTPUT` is the last thing in the chain and unity means unity.
    #[test]
    fn output_scales_everything_and_unity_is_exact() {
        let rate = 48_000.0;
        let input = noise(2048 * 6);
        let base = Controls {
            mix: 0.0,
            ..controls(1.0)
        };

        let unity = run_with(&mut Engine::new(rate, Settings::DEFAULT), base, &input);
        let halved = run_with(
            &mut Engine::new(rate, Settings::DEFAULT),
            Controls {
                output: 0.5,
                ..base
            },
            &input,
        );

        for (index, sample) in halved.iter().enumerate().skip(2048 * 2) {
            assert!((sample - unity[index] * 0.5).abs() < 1e-6, "sample {index}");
        }
    }

    /// `REQ-PUM-012`: sweeping `MIX` must not step.
    #[test]
    fn sweeping_mix_does_not_step() {
        let rate = 48_000.0;
        let input = noise(2048 * 8);

        let mut engine = Engine::new(rate, Settings::DEFAULT);
        let mut output = input.clone();
        let blocks = output.len() / 512;
        for (index, piece) in output.chunks_mut(512).enumerate() {
            engine.set(Controls {
                mix: index as f32 / blocks as f32,
                ..controls(1.0)
            });
            let mut channels = [piece];
            engine.process(&mut channels);
        }

        let steady = worst_step(&run_with(
            &mut Engine::new(rate, Settings::DEFAULT),
            controls(1.0),
            &input,
        ));
        let swept = worst_step(&output);
        assert!(
            swept <= steady * 1.2,
            "sweeping MIX raised the worst step from {steady} to {swept}"
        );
    }

    /// `REQ-PUM-017`: the host's buffer size must not reach the output, and the
    /// dry path is now chunked independently of it.
    #[test]
    fn the_block_size_does_not_change_the_output_with_the_dry_path() {
        let rate = 48_000.0;
        let input = noise(2048 * 5);
        let controls = Controls {
            mix: 0.6,
            output: 0.8,
            ..controls(1.0)
        };

        let mut reference = Vec::new();
        for chunk in [512, 1, 7, 64, 256, 257, 4096] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(controls);
            let mut output = input.clone();
            for piece in output.chunks_mut(chunk) {
                let mut channels = [piece];
                engine.process(&mut channels);
            }
            if reference.is_empty() {
                reference = output;
            } else {
                assert_eq!(output, reference, "block size {chunk}");
            }
        }
    }

    /// `REQ-PUM-017`: a time constant is in seconds, so the same `SPEED` has to
    /// take the same *time* at every rate.
    ///
    /// Measured as how long the reduction takes to reach half of where it
    /// settles, on a resonance that switches on.
    #[test]
    fn the_time_constants_are_the_same_seconds_at_every_rate() {
        use nxe_audio::biquad::{Biquad, Coefficients};

        let mut times = Vec::new();
        for rate in [44_100.0_f32, 48_000.0, 96_000.0] {
            let seconds = 12;
            let length = rate as usize * seconds;
            let mut input = noise(length);
            let mut filter = Biquad::new(Coefficients::peaking(2_500.0, 4.0, 18.0, rate));
            for sample in input.iter_mut() {
                *sample = filter.process(*sample) * 0.3;
            }

            let mut engine = Engine::new(rate, Settings::DEFAULT);
            // `STATIC`, so what is being timed is the short follower rather
            // than the map's warm-up.
            engine.set(controls(1.0));
            let bin = engine.bin_of(2_500.0);

            // Settle, then measure from silence into the tone.
            let mut warm = input[..length / 2].to_vec();
            for piece in warm.chunks_mut(512) {
                let mut channels = [piece];
                engine.process(&mut channels);
            }
            let settled = engine.reduction_curve()[bin];

            engine.reset();
            let mut half_at = None;
            let mut elapsed = 0usize;
            let mut second = input[length / 2..].to_vec();
            for piece in second.chunks_mut(64) {
                let mut channels = [piece];
                engine.process(&mut channels);
                elapsed += 64;
                if half_at.is_none() && engine.reduction_curve()[bin] <= settled * 0.5 {
                    half_at = Some(elapsed as f32 / rate);
                }
            }
            times.push(half_at.expect("the reduction never got half way"));
        }

        let span = times.iter().fold(f32::MIN, |a, b| a.max(*b))
            - times.iter().fold(f32::MAX, |a, b| a.min(*b));
        assert!(
            span < 0.020,
            "half-way times differ by {:.1} ms across rates: {times:?}",
            span * 1000.0
        );
    }

    /// `REQ-PUM-016`: one non-finite sample must not latch anything for good.
    #[test]
    fn a_non_finite_sample_does_not_latch_the_engine() {
        let rate = 48_000.0;
        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));

        let mut input = noise(2048 * 8);
        input[1000] = f32::NAN;
        input[1001] = f32::INFINITY;
        input[1002] = -f32::INFINITY;

        let output = run(&mut engine, &input);
        // The transform spreads a non-finite sample over one window, and it has
        // to be gone after that rather than for ever.
        let settled = 2048 * 4;
        assert!(
            output[settled..].iter().all(|sample| sample.is_finite()),
            "a non-finite sample survived a window"
        );
    }

    /// Every rate the requirement names has to build and run.
    #[test]
    fn every_rate_runs() {
        for rate in [44_100.0_f32, 48_000.0, 96_000.0, 192_000.0] {
            for quality in Quality::ALL {
                let mut engine = Engine::new(rate, Settings::DEFAULT);
                engine.set(Controls {
                    quality,
                    ..controls(1.0)
                });
                let output = run(&mut engine, &noise(quality.block(rate) * 4));
                assert!(
                    output.iter().all(|sample| sample.is_finite()),
                    "{quality:?} at {rate} Hz"
                );
            }
        }
    }

    /// `REQ-PUM-018`: the curve the figure draws is the gain the audio got, and
    /// it decays to nothing when the sound stops.
    #[test]
    fn the_published_curves_follow_the_sound() {
        use nxe_audio::biquad::{Biquad, Coefficients};
        let rate = 48_000.0;
        let hz = 2_500.0;

        let mut input = noise(rate as usize * 8);
        let mut filter = Biquad::new(Coefficients::peaking(hz, 4.0, 18.0, rate));
        for sample in input.iter_mut() {
            *sample = filter.process(*sample) * 0.3;
        }

        let mut engine = Engine::new(rate, Settings::DEFAULT);
        engine.set(controls(1.0));
        run(&mut engine, &input);

        let mut curves = Curves::default();
        engine.curves(&mut curves);

        // The resonance shows in both the spectrum and the reduction, at the
        // frequency it was put.
        let point = (0..CURVE_POINTS)
            .min_by(|a, b| {
                (crate::display::point_hz(*a) - hz)
                    .abs()
                    .total_cmp(&(crate::display::point_hz(*b) - hz).abs())
            })
            .unwrap();
        assert!(
            curves.reduction_db[point] < -6.0,
            "the figure shows {:.2} dB where the engine takes {:.2}",
            curves.reduction_db[point],
            engine.reduction_curve()[engine.bin_of(hz)]
        );
        assert!(curves.spectrum_db[point] > curves.spectrum_db[point / 2]);

        // **Three seconds of silence and the figure is empty** (`REQ-PUM-013`).
        run(&mut engine, &vec![0.0; rate as usize * 3]);
        engine.curves(&mut curves);
        assert!(
            curves.reduction_db.iter().all(|db| db.abs() < 0.01),
            "the reduction did not decay"
        );
        assert!(
            curves.spectrum_db.iter().all(|db| *db < -100.0),
            "the spectrum did not decay"
        );
    }

    /// **The threshold is what protects a sung line now** (`PUM-10c`).
    ///
    /// The adaptive map existed to leave a singer's partials alone and it did
    /// not work — three statistics were measured and none separates an
    /// intermittently-excited resonance from persistent partials
    /// (`REQ-PUM-003`). What is left is the threshold, and this pins the fact
    /// that raising it does the job: a sung sawtooth is chewed at the measured
    /// default and untouched a few decibels above it.
    ///
    /// **It is a worse answer than the map would have been**, because it is a
    /// number a person has to find rather than one the plugin works out. That
    /// is the cost of the map not working, and it is recorded rather than
    /// hidden.
    #[test]
    fn raising_the_threshold_protects_a_sung_line() {
        let rate = 48_000.0;
        let input = sung_saw(rate as usize * 12, rate, 110.0, 0.35);

        let mut losses = Vec::new();
        for threshold_db in [Settings::DEFAULT.threshold_db, 12.0] {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(Controls {
                threshold_db,
                ..controls(1.0)
            });
            let output = run(&mut engine, &input);
            let latency = engine.latency();
            let change =
                20.0 * (rms(&output[latency..]) / rms(&input[..input.len() - latency])).log10();
            losses.push((
                change,
                deepest_reduction_db(&engine, 200.0, 10_000.0),
                mean_reduction_db(&engine, 200.0, 10_000.0),
            ));
        }

        let (at_default, deepest_default, _) = losses[0];
        let (raised, deepest_raised, mean_raised) = losses[1];

        assert!(
            at_default < -1.0,
            "the default takes {at_default:.2} dB off a sung line — if this ever \
             reads near zero the product changed"
        );
        assert!(
            deepest_default < -2.0,
            "the deepest cut at the default is only {deepest_default:.2} dB"
        );
        assert!(
            raised > at_default + 1.0,
            "raising the threshold changed the level by {:.2} dB",
            raised - at_default
        );
        assert!(
            deepest_raised > deepest_default + 1.5 && mean_raised > -0.5,
            "raising the threshold left {deepest_raised:.2} dB deepest, \
             {mean_raised:.2} dB average"
        );
    }

    #[test]
    fn every_quality_runs() {
        let rate = 48_000.0;
        for quality in Quality::ALL {
            let mut engine = Engine::new(rate, Settings::DEFAULT);
            engine.set(Controls {
                quality,
                ..controls(1.0)
            });
            assert_eq!(engine.latency(), quality.block(rate));
            let output = run(&mut engine, &noise(quality.block(rate) * 5));
            assert!(output.iter().all(|sample| sample.is_finite()));
        }
    }
}
