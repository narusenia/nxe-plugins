//! Halo on truce: a ring of tuned resonance around the input, the second
//! Colour Kit (`plugins/kit/docs/implementation/resonance-kit-plan.md`). The
//! sound is `resonance_core::halo`; this is the declarations and the window.

use resonance_core::halo::{Chord as EngineChord, Halo as Engine, READING, Settings, VOICES};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::{Chord, HaloParams, SHOWN, Tuning};

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Halo;

pub struct HaloState {
    engine: Engine,
    rings: [f32; VOICES * READING],
    shown: [f32; SHOWN],
    count: u32,
}

impl HaloState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            rings: [0.0; VOICES * READING],
            shown: [0.0; SHOWN],
            count: 0,
        }
    }
}

/// The engine's chord for the parameter's index.
pub(crate) fn chord(index: usize) -> EngineChord {
    match index {
        index if index == Chord::Major as usize => EngineChord::Major,
        index if index == Chord::Minor as usize => EngineChord::Minor,
        _ => EngineChord::Fifth,
    }
}

impl Default for HaloState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Halo {
    type Params = HaloParams;
    type DspState = HaloState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut HaloState, _params: &HaloParams, config: &AudioConfig) {
        *state = HaloState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut HaloState,
        params: &HaloParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let follow = params.tuning.index() as usize == Tuning::Follow as usize;
        let chord = chord(params.chord.index() as usize);
        buffer.for_each_stereo_frame(|input, output| {
            let settings = Settings {
                root: params.root.read(),
                follow,
                chord,
                sensitivity: params.sensitivity.read(),
                decay: params.decay.read(),
                shimmer: params.shimmer.read(),
                tone: params.tone.read(),
                mix: params.mix.read(),
            };
            *output = state.engine.process(*input, &settings);
        });
        // Reading, never writing: nothing here goes back into the engine.
        state.engine.rings(&mut state.rings);
        state.shown[..VOICES * READING].copy_from_slice(&state.rings);
        state.shown[VOICES * READING] = state.engine.root();
        params.shown.write(&state.shown);
        state.count = state.count.wrapping_add(1);
        params.count.store(state.count, Ordering::Relaxed);
        ProcessStatus::Normal
    }

    fn editor(params: Arc<HaloParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Halo,
    params: HaloParams,
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
        let params = HaloParams::default();
        kit_ui::check_factory(&params, &ui::knob_ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/halo_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/halo_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
