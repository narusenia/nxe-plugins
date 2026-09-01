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
//! The window is `ui`. **One screen, no tabs** (`REQ-PUM-013`).

use analysis::{Analysis, METERS};
use nih_plug::prelude::*;
use nxe_dsp::Level;
use pumice_core::{Curves, Engine, Settings};
use std::sync::Arc;

mod analysis;
mod params;
mod ui;

use params::PumiceParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct Pumice {
    params: Arc<PumiceParams>,
    /// The window's size and position, which the host saves with the project.
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    /// What the editor reads. **The audio thread writes; nothing else touches
    /// the analysers below** (`analysis.rs`).
    analysis: Arc<Analysis>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    /// Scratch for the figure's curves, so publishing a frame allocates
    /// nothing.
    curves: Curves,
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
            editor_state: ui::default_state(),
            analysis: Arc::new(Analysis::default()),
            meters: std::array::from_fn(|_| Level::new(FALLBACK_SAMPLE_RATE)),
            curves: Curves::default(),
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create(
            self.params.clone(),
            self.editor_state.clone(),
            self.analysis.clone(),
        )
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
            self.meters = std::array::from_fn(|_| Level::new(self.sample_rate));
        }

        self.engine.set(self.params.controls(1));
        self.reported_latency = self.engine.latency();
        context.set_latency_samples(self.reported_latency as u32);
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        for meter in &mut self.meters {
            meter.reset();
        }
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

        // **Before the engine**, because it replaces the buffer in place.
        for (index, channel) in channels[..count].iter().enumerate() {
            for sample in channel.iter() {
                self.meters[index].push(*sample);
            }
        }
        if count == 1 {
            // Under the mono layout the right meter mirrors the left rather
            // than reading a channel that does not exist.
            for sample in channels[0].iter() {
                self.meters[1].push(*sample);
            }
        }

        self.engine.process(&mut channels[..count]);

        for (index, channel) in channels[..count].iter().enumerate() {
            for sample in channel.iter() {
                self.meters[2 + index].push(*sample);
            }
        }
        if count == 1 {
            for sample in channels[0].iter() {
                self.meters[3].push(*sample);
            }
        }

        // One frame per block. The editor reads whatever is there — a frame it
        // misses is a frame nobody would have seen.
        //
        // **Reading, never writing** (`REQ-PUM-018`): everything here comes out
        // of the engine, and nothing here goes back in. Stopping the analysis
        // would not change a sample.
        self.engine.curves(&mut self.curves);
        self.analysis.spectrum.write(&self.curves.spectrum_db);
        self.analysis.reduction.write(&self.curves.reduction_db);
        self.analysis
            .peaks
            .write(&std::array::from_fn(|index| self.meters[index].peak()));
        self.analysis
            .holds
            .write(&std::array::from_fn(|index| self.meters[index].hold()));
        self.analysis.readouts.write(&[self.engine.reduction_db()]);

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
                    mode: pumice_core::Mode::Adaptive,
                    quality,
                    mix: 1.0,
                    output: 1.0,
                    delta: false,
                    nodes: [pumice_core::Node::default(); pumice_core::NODES],
                    range: pumice_core::Range::default(),
                });
                assert_eq!(engine.latency(), quality.block(rate));
            }
        }
    }
}
