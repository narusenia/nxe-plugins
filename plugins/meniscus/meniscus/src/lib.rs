//! Meniscus on truce: the input on a liquid surface that a stroke knocks
//! down and that wobbles back, the first Colour flagship
//! (`plugins/meniscus/docs/implementation/meniscus-plan.md`). The sound is
//! `resonance_core::meniscus`; this is the declarations and the window.

use resonance_core::dew::{DROPS, READING};
use resonance_core::meniscus::{HISTORY, Meniscus as Engine, Settings};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::{MeniscusParams, SHOWN, Tuning};

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Meniscus;

pub struct MeniscusState {
    engine: Engine,
    surface: [f32; HISTORY],
    drops: [f32; DROPS * READING],
    shown: [f32; SHOWN],
    count: u32,
}

impl MeniscusState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            surface: [0.0; HISTORY],
            drops: [0.0; DROPS * READING],
            shown: [0.0; SHOWN],
            count: 0,
        }
    }
}

impl Default for MeniscusState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Meniscus {
    type Params = MeniscusParams;
    type DspState = MeniscusState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut MeniscusState, _params: &MeniscusParams, config: &AudioConfig) {
        *state = MeniscusState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut MeniscusState,
        params: &MeniscusParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let follow = params.tuning.index() as usize == Tuning::Follow as usize;
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                sensitivity: params.sensitivity.read(),
                pitch: params.pitch.read(),
                follow,
                tension: params.tension.read(),
                viscosity: params.viscosity.read(),
                splash: params.splash.read(),
                resonance: params.resonance.read(),
                drops: params.drops.read(),
                mix: params.mix.read(),
            };
            let gain = 10f32.powf(params.output.read() / 20.0);
            let out = state.engine.process(*input, &settings);
            *output = [out[0] * gain, out[1] * gain];
        });
        // Reading, never writing: nothing here goes back into the engine.
        state.engine.surface(&mut state.surface);
        state.engine.drops(&mut state.drops);
        state.shown[..HISTORY].copy_from_slice(&state.surface);
        state.shown[HISTORY..SHOWN - 1].copy_from_slice(&state.drops);
        state.shown[SHOWN - 1] = state.engine.hz();
        params.shown.write(&state.shown);
        state.count = state.count.wrapping_add(1);
        params.count.store(state.count, Ordering::Relaxed);
        ProcessStatus::Normal
    }

    fn editor(params: Arc<MeniscusParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Meniscus,
    params: MeniscusParams,
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
        let params = MeniscusParams::default();
        kit_ui::check_factory(&params, &ui::ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/meniscus_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/meniscus_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
