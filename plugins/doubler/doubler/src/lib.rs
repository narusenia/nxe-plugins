//! The NXE Doubler nih-plug wrapper: parameter declarations, and the wiring
//! between them and `doubler_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`). The UI arrives in `DBL-9`; for
//! now a host draws its own generic controls, which is enough to judge the
//! sound (`docs/implementation/roadmap.md`, phase 1).

use doubler_core::VoiceEngine;
use nih_plug::prelude::*;
use std::sync::Arc;

mod params;

use params::DoublerParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct Doubler {
    params: Arc<DoublerParams>,
    engine: VoiceEngine,
    sample_rate: f32,
    /// How many input channels the host actually negotiated. With one, the
    /// buffer still has two channels (the output count), and only the first
    /// holds input — so the second has to be ignored rather than read.
    input_channels: usize,
}

impl Default for Doubler {
    fn default() -> Self {
        Self {
            params: Arc::new(DoublerParams::default()),
            engine: VoiceEngine::new(FALLBACK_SAMPLE_RATE),
            sample_rate: FALLBACK_SAMPLE_RATE,
            input_channels: 2,
        }
    }
}

impl Plugin for Doubler {
    const NAME: &'static str = "NXE Doubler";
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
        // Mono in, stereo out. The wet signal is stereo by definition — the
        // voices are spread across the image — so a mono-out layout would throw
        // away the point of the plugin. This one lets a mono track host it.
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

    /// The only place that allocates. Every buffer the audio thread touches is
    /// sized here, from the rate and block size the host has just committed to
    /// (`REQ-DBL-011`).
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
            self.engine = VoiceEngine::new(self.sample_rate);
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
        let channels = buffer.as_slice();
        // `AUDIO_IO_LAYOUTS` only offers stereo, so this cannot happen. Bail
        // rather than index, because a wrong guess here would be a panic on the
        // audio thread.
        let [left, right, ..] = channels else {
            return ProcessStatus::Normal;
        };

        for sample in 0..left.len() {
            let macros = self.params.macros();
            let shape = self.params.shape();

            let dry_left = left[sample];
            // A mono input leaves the second channel undefined, so mirror the
            // first instead of reading it. `MonoSum` then produces exactly what
            // a mono source should (`REQ-DBL-004`).
            let dry_right = if self.input_channels >= 2 {
                right[sample]
            } else {
                dry_left
            };

            let (wet_left, wet_right) = self.engine.process(dry_left, dry_right, &macros, &shape);

            let mix = self.params.mix.smoothed.next();
            let output = util::db_to_gain(self.params.output.smoothed.next());

            // Dry is never delayed or filtered, which is why the plugin reports
            // no latency (`REQ-DBL-006`). At `mix == 0` this is the input
            // multiplied by one, so a bypassed plugin is bit-transparent.
            left[sample] = (dry_left * (1.0 - mix) + wet_left * mix) * output;
            right[sample] = (dry_right * (1.0 - mix) + wet_right * mix) * output;
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Doubler {
    const CLAP_ID: &'static str = "com.nxe.doubler";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Multi-voice doubler");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Chorus,
    ];
}

impl Vst3Plugin for Doubler {
    const VST3_CLASS_ID: [u8; 16] = *b"NXEDoubler......";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nih_export_clap!(Doubler);
nih_export_vst3!(Doubler);
