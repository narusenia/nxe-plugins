//! The NXE Air nih-plug wrapper: parameter declarations, and the wiring between
//! them and `air_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! **There is no editor yet** (`AIR-4`). The host's generic parameter list is
//! enough to hear the layer, and hearing it is what the following units are
//! judged against — the same order `VEL-5` used, and for the same reason: the
//! Follow Engine is the part whose success is judged by ear (`air-plan.md`).

use air_core::Engine;
use nih_plug::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

mod params;

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
        // resolves the curve (`air_core::Engine::set_shape`).
        self.engine.set_shape(&self.params.shape(samples as u32));

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
            left[sample] = out_left * output;
            right[sample] = out_right * output;
        }

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
                "mix",
                "output",
                "drive",
                "bias",
                "os",
            ]
        );
    }
}
