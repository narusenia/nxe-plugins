//! Dew on truce: water drops on every attack, the first of the Colour Kits
//! (`plugins/kit/docs/implementation/resonance-kit-plan.md`). The sound is
//! `resonance_core::dew`; this is the declarations and the window.

use resonance_core::dew::{DROPS, Dew as Engine, READING, Settings};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::{DewParams, Tuning};

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Dew;

pub struct DewState {
    engine: Engine,
    drops: [f32; DROPS * READING],
    count: u32,
}

impl DewState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            drops: [0.0; DROPS * READING],
            count: 0,
        }
    }
}

impl Default for DewState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Dew {
    type Params = DewParams;
    type DspState = DewState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut DewState, _params: &DewParams, config: &AudioConfig) {
        *state = DewState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut DewState,
        params: &DewParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let follow = params.tuning.index() as usize == Tuning::Follow as usize;
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                size: params.size.read(),
                splash: params.splash.read(),
                mix: params.mix.read(),
                follow,
            };
            *output = state.engine.process(*input, &settings);
        });
        // Reading, never writing: nothing here goes back into the engine.
        state.engine.drops(&mut state.drops);
        params.drops.write(&state.drops);
        state.count = state.count.wrapping_add(1);
        params.count.store(state.count, Ordering::Relaxed);
        ProcessStatus::Normal
    }

    fn editor(params: Arc<DewParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Dew,
    params: DewParams,
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
        let params = DewParams::default();
        kit_ui::check_factory(&params, &ui::knob_ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/dew_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/dew_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
