//! Invert on truce: one of the Spectral Kits
//! (`plugins/kit/docs/implementation/spectral-kit-plan.md`). The sound is
//! `spectral_core::invert`; this is the declarations and the window.
//!
//! **It reports latency**: one transform block, as Pumice does.

use spectral_core::invert::{Invert as Engine, Settings};
use std::sync::Arc;
use truce::prelude::*;

mod params;
mod ui;

use params::InvertParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Invert;

pub struct InvertState {
    engine: Engine,
}

impl Default for InvertState {
    fn default() -> Self {
        Self {
            engine: Engine::new(FALLBACK_SAMPLE_RATE),
        }
    }
}

impl PluginLogic for Invert {
    type Params = InvertParams;
    type DspState = InvertState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut InvertState, _params: &InvertParams, config: &AudioConfig) {
        state.engine = Engine::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut InvertState,
        params: &InvertParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                low: params.low.read(),
                high: params.high.read(),
                mix: params.mix.read(),
            };
            *output = state.engine.process(*input, &settings);
        });
        ProcessStatus::Normal
    }

    #[allow(clippy::cast_possible_truncation)]
    fn latency(state: &InvertState) -> u32 {
        state.engine.latency() as u32
    }

    fn editor(params: Arc<InvertParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Invert,
    params: InvertParams,
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
        let params = InvertParams::default();
        kit_ui::check_factory(&params, &ui::knob_ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/invert_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/invert_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
