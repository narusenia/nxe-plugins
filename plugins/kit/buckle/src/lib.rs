//! Buckle on truce: one of the Design Kits
//! (`plugins/kit/docs/implementation/nonlinear-kit-plan.md`). The sound is
//! `nonlinear_core::buckle`; this is the declarations and the window.

use nonlinear_core::buckle::{Buckle as Engine, Settings};
use std::sync::Arc;
use truce::prelude::*;

mod params;
mod ui;

use params::BuckleParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;
/// How long the shaded reach takes to fall back.
const REACH_FALL_SECONDS: f32 = 0.3;

pub struct Buckle;

pub struct BuckleState {
    engine: Engine,
    reach: f32,
    sample_rate: f32,
}

impl BuckleState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            reach: 0.0,
            sample_rate,
        }
    }
}

impl Default for BuckleState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Buckle {
    type Params = BuckleParams;
    type DspState = BuckleState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut BuckleState, _params: &BuckleParams, config: &AudioConfig) {
        *state = BuckleState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut BuckleState,
        params: &BuckleParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        let mut peak = 0.0f32;
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                threshold: 10f32.powf(params.threshold.read() / 20.0),
                snap: params.snap.read(),
                mix: params.mix.read(),
            };
            *output = state.engine.process(*input, &settings);
            peak = peak.max(input[0].abs()).max(input[1].abs());
        });
        let fall =
            (-(buffer.num_samples() as f32) / (REACH_FALL_SECONDS * state.sample_rate)).exp();
        state.reach = (state.reach * fall).max(peak);
        context.set_meter(params.input.id(), state.reach.min(1.0));
        context.set_meter(params.buckled.id(), state.engine.buckled());
        ProcessStatus::Normal
    }

    fn editor(params: Arc<BuckleParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Buckle,
    params: BuckleParams,
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
        let params = BuckleParams::default();
        kit_ui::check_factory(&params, &ui::knob_ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/buckle_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/buckle_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
