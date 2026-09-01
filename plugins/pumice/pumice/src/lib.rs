//! The NXE Pumice nih-plug wrapper.
//!
//! **This build does not process audio. It is a gate** (`PUM-1`,
//! `REQ-PUM-007`).
//!
//! Pumice is the first plugin in the line to report latency; the other five
//! each decided on zero for their own reasons, and none of them ever asked a
//! host for delay compensation. Whether four DAWs honour it — in CLAP and in
//! VST3, and across a change of the reported value while the transport is
//! running — cannot be answered by a test. It can only be answered by opening
//! them.
//!
//! So this build reports `3N/4` and then **delays by exactly that much**. If
//! compensation works, the track stays in time with an untouched one; if it
//! does not, it slides by a known amount. The plugin is its own instrument.
//!
//! **Nothing is built on top of this until it passes**
//! (`docs/implementation/roadmap.md`, criterion 0). If it fails, the fallback
//! is a subtractive band-pass bank at zero latency — the mechanism is already
//! specified as Vocal Glue's `Spectral Cohesion` (`REQ-GLU-003`).
//!
//! No DSP lives here (`.agents/rules/rust.md`); the transform size is
//! `pumice_core::quality`.

use nih_plug::prelude::*;
use nxe_audio::DelayLine;
use pumice_core::{Quality, quality};
use std::sync::Arc;

mod params;

use params::PumiceParams;

/// The sample rate the delay line is built for before a host says otherwise.
/// `initialize` replaces it when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// `DelayLine::at(1)` is the most recently written sample, so a delay of `L`
/// samples is read at `L + 1`.
const READ_OFFSET: usize = 1;

struct Pumice {
    params: Arc<PumiceParams>,
    /// One per channel. Sized for the **largest** latency any `QUALITY` can
    /// ask for at this rate, so switching steps never allocates
    /// (`REQ-PUM-008`).
    dry: [DelayLine; 2],
    sample_rate: f32,
    /// What was last reported to the host. Compared each block so that
    /// `set_latency_samples` is called on a change and not every block.
    reported_latency: usize,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

/// A line long enough for the deepest step, plus the read offset and a sample
/// of slack.
fn dry_line(sample_rate: f32) -> DelayLine {
    let samples = quality::max_latency(sample_rate) + READ_OFFSET + 1;
    DelayLine::new(sample_rate, samples as f32 / sample_rate)
}

impl Default for Pumice {
    fn default() -> Self {
        Self {
            params: Arc::new(PumiceParams::default()),
            dry: [
                dry_line(FALLBACK_SAMPLE_RATE),
                dry_line(FALLBACK_SAMPLE_RATE),
            ],
            sample_rate: FALLBACK_SAMPLE_RATE,
            reported_latency: usize::MAX,
            input_channels: 2,
        }
    }
}

impl Pumice {
    fn latency(&self) -> usize {
        Quality::from(self.params.quality.value()).latency(self.sample_rate)
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

    /// The only place that allocates.
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
            self.dry = [dry_line(self.sample_rate), dry_line(self.sample_rate)];
        }

        self.reported_latency = self.latency();
        context.set_latency_samples(self.reported_latency as u32);
        true
    }

    fn reset(&mut self) {
        for line in &mut self.dry {
            line.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // **Once per block, and only on a change.** Telling a host its delay
        // compensation is stale is expensive — most of them rebuild the graph —
        // so saying it every block would be a permanent dropout rather than a
        // one-off one (`REQ-PUM-008`).
        let latency = self.latency();
        if latency != self.reported_latency {
            self.reported_latency = latency;
            context.set_latency_samples(latency as u32);
            for line in &mut self.dry {
                line.reset();
            }
        }

        let read_at = latency + READ_OFFSET;
        let stereo = self.input_channels >= 2;

        for (index, channel) in buffer.as_slice().iter_mut().enumerate() {
            // Under the mono layout there is one line in use; a second output
            // channel does not exist, so this never runs past `dry`.
            let Some(line) = self.dry.get_mut(index) else {
                continue;
            };
            if index > 0 && !stereo {
                continue;
            }

            for sample in channel.iter_mut() {
                line.write(*sample);
                *sample = line.read_whole(read_at);
            }
        }

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

    /// The gate's own claim: the line can serve the deepest read any `QUALITY`
    /// asks for. If this were false the delay would silently clamp and the
    /// host's compensation would be right while the audio was not.
    #[test]
    fn the_line_reaches_the_deepest_read() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            let line = dry_line(rate);
            let deepest = quality::max_latency(rate) + READ_OFFSET;
            assert!(
                line.max_delay_samples() >= deepest as f32,
                "{rate} Hz: line reaches {} but needs {deepest}",
                line.max_delay_samples()
            );
        }
    }

    /// **What the four DAWs are being asked to agree with.** An impulse in must
    /// come out exactly `latency` samples later, or the reported number is a
    /// lie and the gate cannot distinguish a broken host from a broken plugin.
    #[test]
    fn the_delay_matches_what_is_reported() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for quality in Quality::ALL {
                let latency = quality.latency(rate);
                let mut line = dry_line(rate);
                let mut output = vec![0.0; latency * 2];

                for (index, out) in output.iter_mut().enumerate() {
                    line.write(if index == 0 { 1.0 } else { 0.0 });
                    *out = line.read_whole(latency + READ_OFFSET);
                }

                let found = output.iter().position(|sample| *sample != 0.0);
                assert_eq!(
                    found,
                    Some(latency),
                    "{quality:?} at {rate} Hz reports {latency}"
                );
            }
        }
    }
}
