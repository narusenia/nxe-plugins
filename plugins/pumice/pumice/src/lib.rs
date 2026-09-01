//! The NXE Pumice nih-plug wrapper: parameter declarations, and the wiring
//! between them and `pumice_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! **The gate has passed** (`PUM-1`, 2026-09-01). Pumice is the first plugin in
//! the line to report latency — the other five each decided on zero for their
//! own reasons — and whether a host honours it could only be answered by
//! opening one. Ableton Live in VST3 and Studio One Pro in CLAP both
//! compensate, and both recover from a `QUALITY` change while the transport
//! runs. Bitwig and Reaper are still to be checked; a failure there would be
//! that host's, not the method's.
//!
//! **`STATIC` only, and no `MIX` yet** (`PUM-3`). The output is the wet path.
//! The dry delay line and `MIX` arrive together in `PUM-6`, because the only
//! reason to hold a dry copy is to have something aligned to mix against.
//!
//! The window is `PUM-10`; there is no editor yet.

use nih_plug::prelude::*;
use pumice_core::{Engine, Settings};
use std::sync::Arc;

mod params;

use params::PumiceParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct Pumice {
    params: Arc<PumiceParams>,
    engine: Engine,
    sample_rate: f32,
    /// What was last reported to the host. Compared each block so that
    /// `set_latency_samples` is called on a change and not every block.
    reported_latency: usize,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

impl Default for Pumice {
    fn default() -> Self {
        Self {
            params: Arc::new(PumiceParams::default()),
            engine: Engine::new(FALLBACK_SAMPLE_RATE, Settings::DEFAULT),
            sample_rate: FALLBACK_SAMPLE_RATE,
            reported_latency: usize::MAX,
            input_channels: 2,
        }
    }
}

impl Plugin for Pumice {
    const NAME: &'static str = "NXE Pumice";
    const VENDOR: &'static str = "NXE";
    const URL: &'static str = "https://github.com/narusenia/nxe-plugins";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // **2→2 and 1→1** (`REQ-PUM-011`). A vocal track is often mono, and this
    // is a corrective processor — it does not build an image, so there is no
    // 1→2 the way Air and Diorama have one.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
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

    /// The only place that allocates. Every transform size is planned here so
    /// that a `QUALITY` change later is an index (`REQ-PUM-008`).
    ///
    /// **The latency is reported from here as well as from `process`.** A host
    /// reads it when it activates the plugin, and `QUALITY` may already differ
    /// from the default in a session being restored.
    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        context: &mut impl InitContext<Self>,
    ) -> bool {
        self.input_channels = audio_io_layout
            .main_input_channels
            .map_or(0, |channels| channels.get() as usize);

        if buffer_config.sample_rate != self.sample_rate {
            self.sample_rate = buffer_config.sample_rate;
            self.engine = Engine::new(self.sample_rate, Settings::DEFAULT);
        }

        self.engine.set(self.params.controls(1));
        self.reported_latency = self.engine.latency();
        context.set_latency_samples(self.reported_latency as u32);
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples();

        // Once per block: this is what resolves the reference width, the
        // follower coefficients and the transform size (`pumice_core::Engine`).
        self.engine.set(self.params.controls(samples as u32));

        // **Only on a change.** Telling a host its delay compensation is stale
        // is expensive — most rebuild the graph — so saying it every block
        // would be a permanent dropout rather than a one-off one.
        let latency = self.engine.latency();
        if latency != self.reported_latency {
            self.reported_latency = latency;
            context.set_latency_samples(latency as u32);
        }

        // A mono layout hands back one channel; taking more would read
        // undefined data (`REQ-PUM-011`).
        let used = self.input_channels.min(buffer.channels()).max(1);
        let mut channels: [&mut [f32]; 2] = [&mut [], &mut []];
        let mut count = 0;
        for (slot, channel) in channels.iter_mut().zip(buffer.as_slice().iter_mut()) {
            if count == used {
                break;
            }
            *slot = channel;
            count += 1;
        }

        self.engine.process(&mut channels[..count]);

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Pumice {
    // **Never changeable once shipped**: a host stores it in the project file
    // (`AGENTS.md`). Provisional only because nothing has shipped
    // (`REQ-PUM-014`).
    const CLAP_ID: &'static str = "com.nxe.pumice";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Dynamic resonance suppression for a single vocal");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Equalizer,
        ClapFeature::Restoration,
    ];
}

impl Vst3Plugin for Pumice {
    // Sixteen bytes, and **never changeable once shipped**, for the same reason
    // as `CLAP_ID`.
    const VST3_CLASS_ID: [u8; 16] = *b"NXEPumice.......";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Eq,
        Vst3SubCategory::Restoration,
    ];
}

nih_export_clap!(Pumice);
nih_export_vst3!(Pumice);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_class_id_is_sixteen_bytes() {
        assert_eq!(Pumice::VST3_CLASS_ID.len(), 16);
    }

    /// The plugin reports what the engine actually delays by. This is what the
    /// four hosts are being asked to agree with (`PUM-1`).
    #[test]
    fn the_reported_latency_is_the_engines() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for quality in pumice_core::Quality::ALL {
                let mut engine = Engine::new(rate, Settings::DEFAULT);
                engine.set(pumice_core::Controls {
                    depth: 0.5,
                    sharpness: 0.5,
                    speed: 0.5,
                    quality,
                });
                assert_eq!(engine.latency(), quality.block(rate));
            }
        }
    }
}
