//! The NXE Air nih-plug wrapper: parameter declarations, and the wiring between
//! them and `air_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! The window is `ui`. **One screen, no tabs** (`REQ-AIR-013`).

use air_core::Engine;
use analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use nih_plug::prelude::*;
use nxe_dsp::{Correlation, Level, Spectrum};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

mod analysis;
#[cfg(test)]
mod defaults;
mod params;
mod ui;

use params::AirParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// How many instances have been created in this process.
///
/// **The noise seed comes from here, not from a clock** (`REQ-AIR-017`).
/// Two instances on the two halves of a stereo pair must not generate the same
/// noise, or the width they were bought for is undone one layer up. Counting
/// rather than reading the time keeps `air-core` free of a clock and makes a
/// bounce repeatable when the instances are created in the same order — which
/// is not promised, only accepted where it comes free.
static INSTANCES: AtomicU32 = AtomicU32::new(0);

struct Air {
    params: Arc<AirParams>,
    /// The window's size and position, which the host saves with the project.
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    /// What the editor reads. **The audio thread writes; nothing else touches
    /// the analysers below** (`analysis.rs`).
    analysis: Arc<Analysis>,
    dry_spectrum: Spectrum<BANDS>,
    layer_spectrum: Spectrum<BANDS>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    /// Of the **layer**, not the output: the promise is about what was added
    /// (`REQ-AIR-008`).
    correlation: Correlation,
    engine: Engine,
    seed: u32,
    sample_rate: f32,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

impl Default for Air {
    fn default() -> Self {
        let seed = INSTANCES
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(0x9E37_79B9)
            ^ 0x4149_5230;
        Self {
            params: Arc::new(AirParams::default()),
            editor_state: ui::default_state(),
            analysis: Arc::new(Analysis::default()),
            dry_spectrum: Spectrum::new(FALLBACK_SAMPLE_RATE, LOW_HZ, HIGH_HZ),
            layer_spectrum: Spectrum::new(FALLBACK_SAMPLE_RATE, LOW_HZ, HIGH_HZ),
            meters: std::array::from_fn(|_| Level::new(FALLBACK_SAMPLE_RATE)),
            correlation: Correlation::new(FALLBACK_SAMPLE_RATE),
            engine: Engine::new(FALLBACK_SAMPLE_RATE, seed),
            seed,
            sample_rate: FALLBACK_SAMPLE_RATE,
            input_channels: 2,
        }
    }
}

impl Plugin for Air {
    const NAME: &'static str = "NXE Air";
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
        // **Mono in, stereo out** (`REQ-AIR-011`). The layer is stereo by
        // definition — `WIDTH` is what the product is for — so a mono-out
        // layout would throw that away. This one lets a mono track host it.
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
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
            self.analysis.clone(),
        )
    }

    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to
    /// (`REQ-AIR-016`).
    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.input_channels = audio_io_layout
            .main_input_channels
            .map_or(0, |channels| channels.get() as usize);

        if buffer_config.sample_rate != self.sample_rate {
            self.sample_rate = buffer_config.sample_rate;
            // Every filter holds coefficients derived from the rate, the
            // harmonic half's lid is a fraction of it, and the noise half's
            // amplitude carries `√(fs / 48000)` — so the engine is rebuilt
            // rather than corrected (`air_core::noise`).
            self.engine = Engine::new(self.sample_rate, self.seed);
            self.dry_spectrum = Spectrum::new(self.sample_rate, LOW_HZ, HIGH_HZ);
            self.layer_spectrum = Spectrum::new(self.sample_rate, LOW_HZ, HIGH_HZ);
            self.meters = std::array::from_fn(|_| Level::new(self.sample_rate));
            self.correlation = Correlation::new(self.sample_rate);
        }
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.dry_spectrum.reset();
        self.layer_spectrum.reset();
        self.correlation.reset();
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
        // Once per block: this is what rebuilds filter coefficients and
        // resolves the curve (`air_core::Engine::set_shape`).
        self.engine.set_shape(&self.params.shape(samples as u32));

        // **The spectra are the one analyser worth switching off.** Every
        // handoff on this plugin is written whether or not anybody is looking,
        // which is right for the meters — they cost a follower per sample and
        // they have to be truthful the instant the window opens. A band bank is
        // a different size of thing: it is a biquad and a follower *per band*
        // per sample, and it is the only figure a closed window cannot want.
        //
        // One bool, read once per block, hoisted out of the sample loop
        // (`docs/investigations/ui-frame-cost.md`).        //
        // A closed window's bank goes stale, and the followers take their
        // release time (250 ms) to be right again after it opens. That is a
        // quarter of a second of a figure settling, on a figure nobody was
        // looking at.
        let watched = self.editor_state.is_open();

        let channels = buffer.as_slice();
        // Both layouts hand back two output channels, so this cannot fail.
        // Bail rather than index, because a wrong guess here would be a panic
        // on the audio thread.
        let [left, right, ..] = channels else {
            return ProcessStatus::Normal;
        };
        let stereo = self.input_channels >= 2;

        for sample in 0..samples {
            let surface = self.params.surface.smoothed.next();
            let mix = self.params.mix.smoothed.next();
            let output = util::db_to_gain(self.params.output.smoothed.next());

            let dry_left = left[sample];
            // A mono input leaves the second channel undefined, so mirror the
            // first instead of reading it (`REQ-AIR-011`).
            let dry_right = if stereo { right[sample] } else { dry_left };

            let (out_left, out_right) = self.engine.process((dry_left, dry_right), surface, mix);
            let (out_left, out_right) = (out_left * output, out_right * output);
            left[sample] = out_left;
            right[sample] = out_right;

            // **After the output trim**, because the meters answer "is this
            // louder than what went in" and the trim is part of the answer.
            let (layer_left, layer_right) = self.engine.layer();
            if watched {
                self.dry_spectrum.push((dry_left + dry_right) * 0.5);
                // **The layer taken directly**, never `out − dry`
                // (`analysis.rs`).
                self.layer_spectrum.push((layer_left + layer_right) * 0.5);
            }
            self.correlation.push(layer_left, layer_right);
            self.meters[0].push(dry_left);
            self.meters[1].push(dry_right);
            self.meters[2].push(out_left);
            self.meters[3].push(out_right);
        }

        // One frame per block. The editor reads whatever is there — a frame it
        // misses is a frame nobody would have seen.
        //
        // **Reading, never writing** (`REQ-AIR-018`): everything here comes out
        // of the engine, and nothing here goes back in. Stopping the analysis
        // would not change a sample.
        self.analysis.dry.write(&self.dry_spectrum.levels());
        self.analysis.layer.write(&self.layer_spectrum.levels());
        self.analysis
            .peaks
            .write(&std::array::from_fn(|index| self.meters[index].peak()));
        self.analysis
            .holds
            .write(&std::array::from_fn(|index| self.meters[index].hold()));
        self.analysis
            .follow
            .write(&self.engine.follow_coefficients());
        self.analysis
            .guard
            .write(&[self.engine.guard_reduction_db()]);
        self.analysis.correlation.write(&[self.correlation.value()]);

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Air {
    // **Never changeable once shipped**: a host stores it in the project file
    // (`AGENTS.md`).
    const CLAP_ID: &'static str = "com.nxe.air";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A signal-driven high-frequency texture generator");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Distortion,
    ];
}

impl Vst3Plugin for Air {
    // Sixteen bytes, and **never changeable once shipped**, for the same
    // reason as `CLAP_ID`.
    const VST3_CLASS_ID: [u8; 16] = *b"NXEAir..........";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion];
}

nih_export_clap!(Air);
nih_export_vst3!(Air);

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every instance draws a different noise stream** (`REQ-AIR-017`). Two
    /// instances with the same seed on the two halves of a stereo pair would
    /// undo the width they were bought for.
    #[test]
    fn two_instances_do_not_share_a_seed() {
        let first = Air::default();
        let second = Air::default();
        assert_ne!(first.seed, second.seed);
    }

    /// **The engine reads the depths by position** (`params::depths`), so the
    /// three deviations have to land in the order `air_core::follow` names.
    #[test]
    fn the_detector_order_matches_the_engine() {
        use air_core::follow::{BRIGHTNESS, ENVELOPE, TRANSIENT};

        let params = AirParams::default();
        params.follow.smoothed.reset(0.0);
        params.follow_envelope.smoothed.reset(1.0);
        params.follow_brightness.smoothed.reset(0.5);
        params.follow_transient.smoothed.reset(0.25);

        let shape = params.shape(1);
        assert_eq!(shape.depths[ENVELOPE], 1.0);
        assert_eq!(shape.depths[BRIGHTNESS], 0.5);
        assert_eq!(shape.depths[TRANSIENT], 0.25);
    }

    /// A parameter id is as final as `CLAP_ID`: a host stores it in the project
    /// file. This is here so that renaming one is a failing test rather than a
    /// silent loss of every saved setting.
    #[test]
    fn the_parameter_ids_are_what_was_shipped() {
        let params = AirParams::default();
        let ids: Vec<String> = params
            .param_map()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(
            ids,
            [
                "surface",
                "blend",
                "character",
                "focus",
                "width",
                "follow",
                "mix",
                "output",
                "drive",
                "bias",
                "fol_env",
                "fol_brt",
                "fol_trn",
                "guard",
                "os",
            ]
        );
    }
}
