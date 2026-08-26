//! The NXE Sparkleur nih-plug wrapper: parameter declarations, and the wiring
//! between them and `sparkleur_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! **No editor yet** — `SPK-8` is the unit where sound first comes out, and the
//! interface arrives in `SPK-12` onward (`sparkleur-plan.md`). Until then a
//! host shows its own generic view, which is enough to turn a knob and listen.

use nih_plug::prelude::*;
use sparkleur_core::Engine;
use std::sync::Arc;

mod params;

use params::SparkleurParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct Sparkleur {
    params: Arc<SparkleurParams>,
    engine: Engine,
    sample_rate: f32,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

impl Default for Sparkleur {
    fn default() -> Self {
        Self {
            params: Arc::new(SparkleurParams::default()),
            engine: Engine::new(FALLBACK_SAMPLE_RATE),
            sample_rate: FALLBACK_SAMPLE_RATE,
            input_channels: 2,
        }
    }
}

impl Plugin for Sparkleur {
    const NAME: &'static str = "NXE Sparkleur";
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
        // **Mono in, mono out** (`REQ-SPK-011`). Nothing here widens, so a mono
        // track has no reason to leave as stereo.
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

    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to
    /// (`REQ-SPK-016`).
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
            // Every filter holds coefficients derived from the rate, and the
            // Sparkle bus's input lid is a fraction of it, so the engine is
            // rebuilt rather than corrected (`sparkleur_core::sparkle`).
            self.engine = Engine::new(self.sample_rate);
        }
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples();
        // Once per block: this is what rebuilds filter coefficients and
        // resolves the `CHARACTER` axis (`sparkleur_core::Engine::set_shape`).
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
                let processed = self.engine.process((left[sample], right[sample]), &levels);
                left[sample] = processed.0 * output;
                right[sample] = processed.1 * output;
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
                // construction (`REQ-SPK-011`), so which one is taken does not
                // matter. It does cost a channel's worth of work for nothing —
                // paid deliberately, because a mono path through the engine
                // would be a second version of the same arithmetic to keep
                // correct, and a mono instance is the cheap case anyway.
                let input = mono[sample];
                let (result, _) = self.engine.process((input, input), &levels);
                mono[sample] = result * output;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Sparkleur {
    const CLAP_ID: &'static str = "com.nxe.sparkleur";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Five-band up-and-down dynamics with a transient-gated harmonic generator");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Compressor,
    ];
}

impl Vst3Plugin for Sparkleur {
    // Sixteen bytes, and **never changeable once shipped**: a host stores it in
    // the project file (`AGENTS.md`).
    const VST3_CLASS_ID: [u8; 16] = *b"NXESparkleur....";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(Sparkleur);
nih_export_vst3!(Sparkleur);
