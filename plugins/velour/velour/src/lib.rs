//! NXE Velour 2 on truce: parameter declarations, and the wiring between them
//! and `velour_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).

use analysis::{BANDS, HIGH_HZ, LOW_HZ, METERS};
use nxe_dsp::{Level, Spectrum};
use std::sync::Arc;
use truce::prelude::*;
use velour_core::Engine;

mod analysis;
mod params;
mod ui;

use params::VelourParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `reset` replaces the engine with one at the real rate before any audio.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// The descriptor. Carries nothing; truce owns the state below.
pub struct Velour;

/// Everything the audio thread owns. The editor never touches it — what it is
/// shown goes through `VelourParams::analysis` (`analysis.rs`).
pub struct VelourState {
    engine: Engine,
    dry_spectrum: Spectrum<BANDS>,
    wet_spectrum: Spectrum<BANDS>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
}

impl VelourState {
    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to.
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            dry_spectrum: Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ),
            wet_spectrum: Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ),
            meters: std::array::from_fn(|_| Level::new(sample_rate)),
        }
    }
}

impl Default for VelourState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Velour {
    type Params = VelourParams;
    type DspState = VelourState;

    /// Stereo, and **mono in, mono out**: nothing here widens, so a mono vocal
    /// has no reason to leave as stereo.
    fn bus_layouts() -> Vec<BusLayout> {
        vec![BusLayout::stereo(), BusLayout::mono()]
    }

    /// Off the audio thread, so rebuilding is allowed. The filters hold
    /// coefficients derived from the rate, so the engine is rebuilt rather than
    /// corrected.
    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut VelourState, _params: &VelourParams, config: &AudioConfig) {
        *state = VelourState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut VelourState,
        params: &VelourParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        // Once per block: this is what moves the band edges and resolves the
        // `TEXTURE` axis.
        state.engine.set_shape(&params.shape(buffer.num_samples()));

        // A mono input arrives in both slots and only the first output slot is
        // kept. The engine's channels are bit identical on equal input, so a
        // mono track costs a channel's work for nothing — paid deliberately,
        // rather than keeping a second copy of the arithmetic correct.
        buffer.for_each_stereo_frame(|input, output| {
            let levels = params.levels();
            let gain = db_to_linear(params.output.read());
            let [in_left, in_right] = *input;
            let (left, right) = state.engine.process((in_left, in_right), &levels);
            let (out_left, out_right) = (left * gain, right * gain);
            *output = [out_left, out_right];

            // The layer being added, separately from the result: the figure
            // draws what Velour adds over what came in.
            let layer = state.engine.wet();
            state.dry_spectrum.push((in_left + in_right) * 0.5);
            state.wet_spectrum.push((layer.0 + layer.1) * 0.5);
            // **After the output trim**, because the meters answer "is this
            // louder than what went in" and the trim is part of the answer.
            state.meters[0].push(in_left);
            state.meters[1].push(in_right);
            state.meters[2].push(out_left);
            state.meters[3].push(out_right);
        });

        // One frame per block. Reading, never writing: nothing here goes back
        // into the engine.
        let analysis = &params.analysis;
        analysis.dry.write(&state.dry_spectrum.levels());
        analysis.wet.write(&state.wet_spectrum.levels());
        analysis
            .peaks
            .write(&std::array::from_fn(|index| state.meters[index].peak()));
        analysis
            .holds
            .write(&std::array::from_fn(|index| state.meters[index].hold()));
        analysis.guards.write(&state.engine.guard_reductions());

        ProcessStatus::Normal
    }

    fn editor(params: Arc<VelourParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Velour,
    params: VelourParams,
}

// A no-op unless the `rt-paranoid` feature is on, which only the tests enable.
truce::enable_rt_paranoid!();

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use truce_test::{InputSource, assertions, driver};

    #[test]
    fn info_is_valid() {
        truce_test::assert_valid_info::<Plugin>();
    }

    #[test]
    fn has_editor() {
        truce_test::assert_has_editor::<Plugin>();
    }

    /// Every parameter comes back from a save, **away from the defaults** —
    /// a parameter that is never restored still passes a round trip of a
    /// fresh instance.
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
