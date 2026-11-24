//! The NXE Pumice truce wrapper: parameter declarations, the egui editor, and
//! the wiring between them and `pumice_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! **Pumice is the one plugin in the line that reports latency** (`PUM-1`):
//! the transform's block. truce polls `latency` after each block and tells
//! the host when it moves, which is what a `QUALITY` change does.
//!
//! **Ships as `NXE Pumice` 1.0.0, not as a "2"**: it was never released
//! (`v2-plan.md`, decision 16).

use analysis::METERS;
use nxe_dsp::Level;
use pumice_core::{Curves, Engine, Settings};
use std::sync::Arc;
use truce::prelude::*;

mod analysis;
mod params;
mod ui;

use params::PumiceParams;

/// The rate and block the state is built for before a host says otherwise.
/// `reset` replaces both.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;
const FALLBACK_BLOCK: usize = 4_096;

pub struct Pumice;

pub struct PumiceState {
    engine: Engine,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    /// Scratch for the figure's curves, so publishing a frame allocates
    /// nothing.
    curves: Curves,
    /// The engine works in place on both channels at once, and truce hands
    /// out one output slice at a time, so the block goes through here.
    /// Sized in `reset` to the host's largest block.
    scratch: [Vec<f32>; 2],
}

impl PumiceState {
    fn new(sample_rate: f32, block: usize, params: &PumiceParams) -> Self {
        let mut engine = Engine::new(sample_rate, Settings::DEFAULT);
        // **Before the first block**, so the latency a host reads when it
        // activates is the one a restored `QUALITY` asks for.
        engine.set(params.controls(0));
        Self {
            engine,
            meters: std::array::from_fn(|_| Level::new(sample_rate)),
            curves: Curves::default(),
            scratch: [vec![0.0; block], vec![0.0; block]],
        }
    }
}

impl Default for PumiceState {
    fn default() -> Self {
        Self::new(
            FALLBACK_SAMPLE_RATE,
            FALLBACK_BLOCK,
            &PumiceParams::default(),
        )
    }
}

impl PluginLogic for Pumice {
    type Params = PumiceParams;
    type DspState = PumiceState;

    /// **2→2 and 1→1** (`REQ-PUM-011`). A corrective processor builds no
    /// image, so there is no 1→2 the way Air and Diorama have one.
    fn bus_layouts() -> Vec<BusLayout> {
        vec![BusLayout::stereo(), BusLayout::mono()]
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut PumiceState, params: &PumiceParams, config: &AudioConfig) {
        *state = PumiceState::new(
            config.sample_rate as f32,
            config.max_block_size.max(1),
            params,
        );
    }

    fn process(
        state: &mut PumiceState,
        params: &PumiceParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let samples = buffer.num_samples();
        // Once per block: this is what resolves the reference width, the
        // follower coefficients and the transform size (`pumice_core::Engine`).
        state.engine.set(params.controls(samples));

        let count = buffer.channels().clamp(1, 2);
        let [left, right] = &mut state.scratch;
        // A host block longer than it promised is cut rather than read past.
        let samples = samples.min(left.len());
        for (channel, scratch) in [&mut *left, &mut *right]
            .into_iter()
            .take(count)
            .enumerate()
        {
            let input = if buffer.is_in_place(channel) {
                &*buffer.in_out_mut(channel)
            } else {
                buffer.input(channel)
            };
            scratch[..samples].copy_from_slice(&input[..samples]);
        }

        // **Before the engine**, which replaces the block in place. Under the
        // mono layout the right meters mirror the left.
        for (index, scratch) in [&*left, &*right].into_iter().enumerate() {
            let source = if index < count { scratch } else { &*left };
            for sample in &source[..samples] {
                state.meters[index].push(*sample);
            }
        }

        let mut channels: [&mut [f32]; 2] = [&mut left[..samples], &mut right[..samples]];
        state.engine.process(&mut channels[..count]);

        for (index, scratch) in [&*left, &*right].into_iter().enumerate() {
            let source = if index < count { scratch } else { &*left };
            for sample in &source[..samples] {
                state.meters[2 + index].push(*sample);
            }
        }
        for (channel, scratch) in [&*left, &*right].into_iter().take(count).enumerate() {
            let output = if buffer.is_in_place(channel) {
                buffer.in_out_mut(channel)
            } else {
                buffer.output(channel)
            };
            output[..samples].copy_from_slice(&scratch[..samples]);
        }

        // One frame per block. **Reading, never writing** (`REQ-PUM-018`):
        // everything here comes out of the engine, and nothing goes back in.
        state.engine.curves(&mut state.curves);
        let analysis = &params.analysis;
        analysis.spectrum.write(&state.curves.spectrum_db);
        analysis.reduction.write(&state.curves.reduction_db);
        analysis
            .peaks
            .write(&std::array::from_fn(|index| state.meters[index].peak()));
        analysis
            .holds
            .write(&std::array::from_fn(|index| state.meters[index].hold()));
        analysis.readouts.write(&[state.engine.reduction_db()]);

        ProcessStatus::Normal
    }

    /// What the engine actually delays by. truce compares it after every
    /// block and tells the host only on a change.
    #[allow(clippy::cast_possible_truncation)]
    fn latency(state: &PumiceState) -> u32 {
        state.engine.latency() as u32
    }

    fn editor(params: Arc<PumiceParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Pumice,
    params: PumiceParams,
}

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

    /// The plugin reports what the engine delays by, at every quality and
    /// rate — what the hosts were asked to agree with (`PUM-1`).
    #[test]
    fn the_reported_latency_is_the_engines() {
        use truce::params::Params;
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for (index, quality) in pumice_core::Quality::ALL.into_iter().enumerate() {
                let params = PumiceParams::default();
                params.set_plain(params.quality.id(), index as f64);
                let state = PumiceState::new(rate, 512, &params);
                assert_eq!(
                    Pumice::latency(&state) as usize,
                    quality.block(rate),
                    "{quality:?} at {rate}"
                );
            }
        }
    }

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
                .duration(Duration::from_millis(200))
                .input(InputSource::Constant(0.5))
                .run()
        });
        assertions::assert_no_nans(&result);
    }
}
