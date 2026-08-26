//! The NXE Velour nih-plug wrapper: parameter declarations, and the wiring
//! between them and `velour_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).

use analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use nih_plug::prelude::*;
use nxe_dsp::{Level, Spectrum};
use std::sync::Arc;
use velour_core::Engine;

mod analysis;
mod params;
mod ui;

use params::VelourParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct Velour {
    params: Arc<VelourParams>,
    /// The window's size and position, which the host saves with the project.
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    /// The rate the host committed to, for the editor to read. An atomic
    /// because `initialize` runs on the host's thread and the editor is built on
    /// another; `f32` bits because there is no `AtomicF32`.
    sample_rate_hint: Arc<std::sync::atomic::AtomicU32>,
    engine: Engine,
    /// What the editor reads. **The audio thread writes; nothing else touches
    /// the analysers below** (`plugins/velour/velour/src/analysis.rs`).
    analysis: Arc<Analysis>,
    dry_spectrum: Spectrum<BANDS>,
    wet_spectrum: Spectrum<BANDS>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    sample_rate: f32,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

impl Default for Velour {
    fn default() -> Self {
        Self {
            params: Arc::new(VelourParams::default()),
            editor_state: ui::default_state(),
            sample_rate_hint: Arc::new(std::sync::atomic::AtomicU32::new(
                FALLBACK_SAMPLE_RATE.to_bits(),
            )),
            engine: Engine::new(FALLBACK_SAMPLE_RATE),
            analysis: Arc::new(Analysis::default()),
            dry_spectrum: Spectrum::new(FALLBACK_SAMPLE_RATE, LOW_HZ, HIGH_HZ),
            wet_spectrum: Spectrum::new(FALLBACK_SAMPLE_RATE, LOW_HZ, HIGH_HZ),
            meters: std::array::from_fn(|_| Level::new(FALLBACK_SAMPLE_RATE)),
            sample_rate: FALLBACK_SAMPLE_RATE,
            input_channels: 2,
        }
    }
}

impl Plugin for Velour {
    const NAME: &'static str = "NXE Velour";
    const VENDOR: &'static str = "NXE";
    const URL: &'static str = "https://github.com/narusenia/nxe-plugins";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        // **Mono in, mono out** — unlike the Doubler, whose wet signal is
        // stereo by definition. Velour does not move the image
        // (`REQ-VEL-011`), so a mono track has no reason to leave as stereo.
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create(
            self.params.clone(),
            self.editor_state.clone(),
            self.sample_rate_hint.clone(),
            self.analysis.clone(),
        )
    }

    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to
    /// (`REQ-VEL-016`).
    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.input_channels = audio_io_layout
            .main_input_channels
            .map_or(0, |channels| channels.get() as usize);

        self.sample_rate_hint.store(
            buffer_config.sample_rate.to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );

        if buffer_config.sample_rate != self.sample_rate {
            self.sample_rate = buffer_config.sample_rate;
            // The filters hold coefficients derived from the rate, and AIR's
            // input ceiling is a fraction of it, so the engine is rebuilt rather
            // than corrected (`velour_core::bands::AIR_INPUT_CEILING`).
            self.engine = Engine::new(self.sample_rate);
            self.dry_spectrum = Spectrum::new(self.sample_rate, LOW_HZ, HIGH_HZ);
            self.wet_spectrum = Spectrum::new(self.sample_rate, LOW_HZ, HIGH_HZ);
            self.meters = std::array::from_fn(|_| Level::new(self.sample_rate));
        }
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.dry_spectrum.reset();
        self.wet_spectrum.reset();
        for meter in &mut self.meters {
            meter.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples();
        // Once per block: this is what rebuilds filter coefficients and the
        // curve's normalisation (`velour_core::Engine::set_shape`).
        self.engine.set_shape(&self.params.shape(samples as u32));

        let channels = buffer.as_slice();
        let stereo = channels.len() >= 2 && self.input_channels >= 2;

        // Split so a mono layout never indexes a second channel that is not
        // there. A wrong guess here would be a panic on the audio thread.
        if stereo {
            let [left, right, ..] = channels else {
                return ProcessStatus::Normal;
            };
            for sample in 0..samples {
                let levels = self.params.levels();
                let output = util::db_to_gain(self.params.output.smoothed.next());
                let (in_left, in_right) = (left[sample], right[sample]);
                let (out_left, out_right) = self.engine.process((in_left, in_right), &levels);
                let (out_left, out_right) = (out_left * output, out_right * output);
                left[sample] = out_left;
                right[sample] = out_right;

                // **After the output trim**, because the meters answer "is this
                // louder than what went in" and the trim is part of the answer
                // (`ui.md`).
                let layer = self.engine.wet();
                self.dry_spectrum.push((in_left + in_right) * 0.5);
                self.wet_spectrum.push((layer.0 + layer.1) * 0.5);
                self.meters[0].push(in_left);
                self.meters[1].push(in_right);
                self.meters[2].push(out_left);
                self.meters[3].push(out_right);
            }
        } else {
            let [mono, ..] = channels else {
                return ProcessStatus::Normal;
            };
            for sample in 0..samples {
                let levels = self.params.levels();
                let output = util::db_to_gain(self.params.output.smoothed.next());
                // The engine is stereo, so a mono host runs both channels and
                // one result is discarded. The two are bit identical by
                // construction (`REQ-VEL-011`), so which one is taken does not
                // matter. It does cost a channel's worth of work for nothing —
                // paid deliberately, because a mono path through the engine
                // would be a second version of the same arithmetic to keep
                // correct, and a mono instance is the cheap case anyway.
                let input = mono[sample];
                let (result, _) = self.engine.process((input, input), &levels);
                let result = result * output;
                mono[sample] = result;

                // A mono track drives both sides of every meter, which is why
                // the two bars sitting exactly on top of each other is itself
                // information (`ui.md`).
                let layer = self.engine.wet();
                self.dry_spectrum.push(input);
                self.wet_spectrum.push(layer.0);
                self.meters[0].push(input);
                self.meters[1].push(input);
                self.meters[2].push(result);
                self.meters[3].push(result);
            }
        }

        // One frame per block. The editor reads whatever is there — a frame it
        // misses is a frame nobody would have seen.
        self.analysis.dry.write(&self.dry_spectrum.levels());
        self.analysis.wet.write(&self.wet_spectrum.levels());
        self.analysis
            .peaks
            .write(&std::array::from_fn(|index| self.meters[index].peak()));
        self.analysis
            .holds
            .write(&std::array::from_fn(|index| self.meters[index].hold()));
        self.analysis.guards.write(&self.engine.guard_reductions());

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Velour {
    const CLAP_ID: &'static str = "com.nxe.velour";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Vocal presence saturator");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Distortion,
    ];
}

impl Vst3Plugin for Velour {
    // Sixteen bytes, and **never changeable once shipped**: a host stores it in
    // the project file (`AGENTS.md`).
    const VST3_CLASS_ID: [u8; 16] = *b"NXEVelour.......";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(Velour);
nih_export_vst3!(Velour);
