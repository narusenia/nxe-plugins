//! NXE Air 2 on truce: parameter declarations, and the wiring between them and
//! `air_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).

use air_core::Engine;
use analysis::{BANDS, HIGH_HZ, LOW_HZ, METERS};
use nxe_dsp::{Correlation, Level, Spectrum};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use truce::prelude::*;

mod analysis;
#[cfg(test)]
mod defaults;
mod params;
mod ui;

use params::AirParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `reset` replaces the engine with one at the real rate before any audio.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// How many instances have been made, for their seeds. **Two instances on two
/// tracks must not make the same noise**: summed, identical noise is 6 dB
/// louder and perfectly correlated, which is exactly the "two tracks phasing"
/// a user cannot find the cause of.
static INSTANCES: AtomicU32 = AtomicU32::new(0);

/// The descriptor. Carries nothing; truce owns the state below.
pub struct Air;

/// Everything the audio thread owns. The editor never touches it — what it is
/// shown goes through `AirParams::analysis` (`analysis.rs`).
pub struct AirState {
    engine: Engine,
    dry_spectrum: Spectrum<BANDS>,
    layer_spectrum: Spectrum<BANDS>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    /// Of the layer: whether the width survives a fold.
    correlation: Correlation,
    /// This instance's noise seed, kept across `reset` so the rate changing
    /// does not change the noise.
    seed: u32,
}

impl AirState {
    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to.
    fn new(sample_rate: f32, seed: u32) -> Self {
        Self {
            engine: Engine::new(sample_rate, seed),
            dry_spectrum: Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ),
            layer_spectrum: Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ),
            meters: std::array::from_fn(|_| Level::new(sample_rate)),
            correlation: Correlation::new(sample_rate),
            seed,
        }
    }
}

impl Default for AirState {
    fn default() -> Self {
        let seed = INSTANCES
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(0x9E37_79B9)
            ^ 0x4149_5230;
        Self::new(FALLBACK_SAMPLE_RATE, seed)
    }
}

impl PluginLogic for Air {
    type Params = AirParams;
    type DspState = AirState;

    /// Stereo, and **mono in, stereo out** (`REQ-AIR-011`). The layer is
    /// stereo by definition — `WIDTH` is what the product is for — so a
    /// mono-out layout would throw that away.
    fn bus_layouts() -> Vec<BusLayout> {
        vec![
            BusLayout::stereo(),
            BusLayout::new()
                .with_input("Main", ChannelConfig::Mono)
                .with_output("Main", ChannelConfig::Stereo),
        ]
    }

    /// Off the audio thread, so rebuilding is allowed. Rebuilt with the same
    /// seed, so a rate change does not change the noise.
    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut AirState, _params: &AirParams, config: &AudioConfig) {
        *state = AirState::new(config.sample_rate as f32, state.seed);
    }

    fn process(
        state: &mut AirState,
        params: &AirParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        state.engine.set_shape(&params.shape(buffer.num_samples()));

        // A mono input arrives in both slots, which is the mono-in /
        // stereo-out layout's dry signal on both sides.
        buffer.for_each_stereo_frame(|input, output| {
            let surface = params.surface.read();
            let mix = params.mix.read();
            let gain = db_to_linear(params.output.read());
            let [dry_left, dry_right] = *input;

            let (left, right) = state.engine.process((dry_left, dry_right), surface, mix);
            let (out_left, out_right) = (left * gain, right * gain);
            *output = [out_left, out_right];

            // The layer alone, for the picture and for the correlation: the
            // dry is what `WIDTH` does not touch.
            let (layer_left, layer_right) = state.engine.layer();
            state.dry_spectrum.push((dry_left + dry_right) * 0.5);
            state.layer_spectrum.push((layer_left + layer_right) * 0.5);
            state.correlation.push(layer_left, layer_right);
            state.meters[0].push(dry_left);
            state.meters[1].push(dry_right);
            state.meters[2].push(out_left);
            state.meters[3].push(out_right);
        });

        // One frame per block. Reading, never writing.
        let analysis = &params.analysis;
        analysis.dry.write(&state.dry_spectrum.levels());
        analysis.layer.write(&state.layer_spectrum.levels());
        analysis
            .peaks
            .write(&std::array::from_fn(|index| state.meters[index].peak()));
        analysis
            .holds
            .write(&std::array::from_fn(|index| state.meters[index].hold()));
        analysis.follow.write(&state.engine.follow_coefficients());
        analysis.guard.write(&[state.engine.guard_reduction_db()]);
        analysis.correlation.write(&[state.correlation.value()]);

        ProcessStatus::Normal
    }

    fn editor(params: Arc<AirParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Air,
    params: AirParams,
}

// A no-op unless the `rt-paranoid` feature is on, which only the tests enable.
truce::enable_rt_paranoid!();

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use truce_test::{InputSource, assertions, driver};

    #[test]
    fn two_instances_do_not_share_a_seed() {
        let first = AirState::default();
        let second = AirState::default();
        assert_ne!(first.seed, second.seed);
    }

    #[test]
    fn the_detector_order_matches_the_engine() {
        use air_core::follow::{BRIGHTNESS, ENVELOPE, TRANSIENT};
        use truce::params::Params;

        let params = AirParams::default();
        params.set_plain(params.follow.id(), 0.0);
        params.set_plain(params.follow_envelope.id(), 1.0);
        params.set_plain(params.follow_brightness.id(), 0.5);
        params.set_plain(params.follow_transient.id(), 0.25);
        params.snap_smoothers();

        let shape = params.shape(1);
        assert_eq!(shape.depths[ENVELOPE], 1.0);
        assert_eq!(shape.depths[BRIGHTNESS], 0.5);
        assert_eq!(shape.depths[TRANSIENT], 0.25);
    }

    #[test]
    fn info_is_valid() {
        truce_test::assert_valid_info::<Plugin>();
    }

    #[test]
    fn has_editor() {
        truce_test::assert_has_editor::<Plugin>();
    }

    /// Every parameter comes back from a save, **away from the defaults**.
    #[test]
    fn state_round_trips() {
        use truce::params::Params;
        use truce_core::export::PluginExport;
        use truce_core::state;

        let plugin = Plugin::create();
        let infos = plugin.params().param_infos();
        for info in &infos {
            let (min, max) = (info.range.min(), info.range.max());
            plugin.params().set_plain(info.id, min + (max - min) * 0.3);
        }
        let blob = state::snapshot_plugin(&plugin);
        let mut restored = Plugin::create();
        state::restore_plugin(&mut restored, &blob).expect("restore_plugin failed");

        for info in &infos {
            let saved = plugin.params().get_plain(info.id);
            let loaded = restored.params().get_plain(info.id);
            assert_eq!(saved, loaded, "{} did not come back", info.name);
        }
    }

    /// **The window against a committed picture** (`V2-4`). One baseline per
    /// OS, baked on CI, never on a laptop; a channel may be off by 2 for the
    /// anti-aliasing the runner and a laptop disagree on.
    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/default_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/default_windows.png")
            .pixel_threshold(2)
            .run();
    }

    #[test]
    fn makes_sound_without_allocating() {
        let result = truce_test::assert_no_audio_alloc(|| {
            driver!(Plugin)
                .duration(Duration::from_millis(50))
                .input(InputSource::Constant(0.5))
                .run()
        });
        assertions::assert_nonzero(&result);
        assertions::assert_no_nans(&result);
    }
}
