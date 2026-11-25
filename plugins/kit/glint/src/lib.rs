//! Glint on truce: crystal sparks high above every attack, the first of the
//! Effloresce Kits (`plugins/kit/docs/implementation/particle-kit-plan.md`).
//! The sound is `particle_core::glint`; this is the declarations and the
//! window.

use particle_core::glint::{Glint as Engine, READING, SPARKS, Settings};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::GlintParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Glint;

pub struct GlintState {
    engine: Engine,
    sparks: [f32; SPARKS * READING],
    count: u32,
}

impl GlintState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            sparks: [0.0; SPARKS * READING],
            count: 0,
        }
    }
}

impl Default for GlintState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Glint {
    type Params = GlintParams;
    type DspState = GlintState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut GlintState, _params: &GlintParams, config: &AudioConfig) {
        *state = GlintState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut GlintState,
        params: &GlintParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                sensitivity: params.sensitivity.read(),
                height: params.height.read(),
                decay: params.decay.read(),
                mix: params.mix.read(),
            };
            *output = state.engine.process(*input, &settings);
        });
        // Reading, never writing: nothing here goes back into the engine.
        state.engine.sparks(&mut state.sparks);
        params.sparks.write(&state.sparks);
        state.count = state.count.wrapping_add(1);
        params.count.store(state.count, Ordering::Relaxed);
        ProcessStatus::Normal
    }

    fn editor(params: Arc<GlintParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Glint,
    params: GlintParams,
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
                .duration(Duration::from_millis(50))
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
        let params = GlintParams::default();
        kit_ui::check_factory(&params, &ui::knob_ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/glint_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/glint_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
