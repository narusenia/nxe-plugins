//! Parasite on truce: the first Design flagship
//! (`plugins/parasite/docs/implementation/parasite-plan.md`). The sound is
//! `parasite_core`; this is the declarations and the window.
//!
//! **It reports latency**: the fixed delay every species runs through, about
//! 10 ms (`parasite_core::engine`).

use parasite_core::{Parasite as Engine, Settings, Species as EngineSpecies};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::ParasiteParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Parasite;

pub struct ParasiteState {
    engine: Engine,
}

impl Default for ParasiteState {
    fn default() -> Self {
        Self {
            engine: Engine::new(FALLBACK_SAMPLE_RATE),
        }
    }
}

fn species(index: usize) -> EngineSpecies {
    match index {
        1 => EngineSpecies::Ring,
        2 => EngineSpecies::Sync,
        3 => EngineSpecies::Voice,
        _ => EngineSpecies::Fm,
    }
}

impl PluginLogic for Parasite {
    type Params = ParasiteParams;
    type DspState = ParasiteState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut ParasiteState, _params: &ParasiteParams, config: &AudioConfig) {
        state.engine = Engine::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut ParasiteState,
        params: &ParasiteParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let species = species(params.species.index() as usize);
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                species,
                infection: params.infection.read(),
                spread: params.spread.read(),
                hunger: params.hunger.read(),
                mutation: params.mutation.read(),
                resistance: params.resistance.read(),
                sensitivity: params.sensitivity.read(),
                mix: params.mix.read(),
                output: 10f32.powf(params.output.read() / 20.0),
            };
            *output = state.engine.process(*input, &settings);
        });
        // Reading, never writing: nothing here goes back into the engine.
        if let Some((host, output)) = state.engine.take_scope() {
            params.host.write(&host);
            params.output_wave.write(&output);
            params.count.fetch_add(1, Ordering::Relaxed);
        }
        let readings = state.engine.readings();
        params.readings.write(&[
            readings.hz,
            readings.presence,
            readings.infection,
            readings.hunger,
        ]);
        ProcessStatus::Normal
    }

    #[allow(clippy::cast_possible_truncation)]
    fn latency(state: &ParasiteState) -> u32 {
        state.engine.latency() as u32
    }

    fn editor(params: Arc<ParasiteParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Parasite,
    params: ParasiteParams,
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
    fn processes_without_allocating() {
        let result = truce_test::assert_no_audio_alloc(|| {
            driver!(Plugin)
                .duration(Duration::from_millis(200))
                .input(InputSource::Generator(Box::new(|index, _| {
                    (index as f32 * 0.05).sin() * 0.8
                })))
                .run()
        });
        assertions::assert_nonzero(&result);
        assertions::assert_no_nans(&result);
    }

    #[test]
    fn factory_presets_are_valid() {
        let params = ParasiteParams::default();
        kit_ui::check_factory(&params, &ui::ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/parasite_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/parasite_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
