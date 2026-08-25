//! The Doubler nih-plug wrapper: parameter declarations, the Vizia UI, and the
//! wiring between them and `doubler_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`). Right now this is a
//! passthrough so the workspace bundles and loads in a host; the parameters
//! arrive in `DBL-4` and the UI in `DBL-9`
//! (`plugins/doubler/docs/implementation/doubler-plan.md`).

use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Default)]
struct Doubler {
    params: Arc<DoublerParams>,
}

#[derive(Params, Default)]
struct DoublerParams {}

impl Plugin for Doubler {
    const NAME: &'static str = "Doubler";
    const VENDOR: &'static str = "nxeu";
    const URL: &'static str = "https://github.com/narusenia/nxe-plugins";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Doubler {
    const CLAP_ID: &'static str = "com.nxeu.doubler";
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
    const VST3_CLASS_ID: [u8; 16] = *b"nxeuDoubler.....";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Modulation];
}

nih_export_clap!(Doubler);
nih_export_vst3!(Doubler);
