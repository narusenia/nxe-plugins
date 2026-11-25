//! Prisma on truce: the input split into the notes you play, the second
//! Colour flagship (`plugins/prisma/docs/implementation/prisma-plan.md`). The
//! sound is `resonance_core::prisma`; this is the declarations, the MIDI and
//! the window. **An audio effect that takes MIDI**: `midi_input` in
//! `truce.toml`, so the AU is a music effect (`aumf`).

use resonance_core::halo::Chord as EngineChord;
use resonance_core::prisma::{Prisma as Engine, Settings, Source as EngineSource};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;

mod params;
mod ui;

use params::{Chord, PrismaParams, SHOWN, Source, Tuning};

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Prisma;

pub struct PrismaState {
    engine: Engine,
    shown: [f32; SHOWN],
    count: u32,
}

impl PrismaState {
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            shown: [0.0; SHOWN],
            count: 0,
        }
    }
}

impl Default for PrismaState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
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

impl PluginLogic for Prisma {
    type Params = PrismaParams;
    type DspState = PrismaState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut PrismaState, _params: &PrismaParams, config: &AudioConfig) {
        *state = PrismaState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut PrismaState,
        params: &PrismaParams,
        buffer: &mut AudioBuffer,
        events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        let source = if params.source.index() as usize == Source::Midi as usize {
            EngineSource::Midi
        } else {
            EngineSource::Chord
        };
        let follow = params.tuning.index() as usize == Tuning::Follow as usize;
        let chord = chord(params.chord.index() as usize);
        let freeze = params.freeze.value();
        let mut pending = events.iter().peekable();
        let mut frame = 0u32;
        buffer.for_each_stereo_frame(|input, output| {
            // Each note on the sample it was played on.
            while let Some(event) = pending.next_if(|event| event.sample_offset <= frame) {
                match event.body {
                    EventBody::NoteOn { note, .. } | EventBody::NoteOn2 { note, .. } => {
                        state.engine.note_on(note);
                    }
                    EventBody::NoteOff { note, .. } | EventBody::NoteOff2 { note, .. } => {
                        state.engine.note_off(note);
                    }
                    _ => {}
                }
            }
            frame += 1;
            let settings = Settings {
                source,
                chord,
                root: params.root.read(),
                follow,
                sensitivity: params.sensitivity.read(),
                decay: params.decay.read(),
                inharmonic: params.inharmonic.read(),
                coupling: params.coupling.read(),
                tone: params.tone.read(),
                shimmer: params.shimmer.read(),
                mix: params.mix.read(),
                freeze,
            };
            let gain = 10f32.powf(params.output.read() / 20.0);
            let out = state.engine.process(*input, &settings);
            *output = [out[0] * gain, out[1] * gain];
        });
        // Reading, never writing: nothing here goes back into the engine.
        state.engine.partials(&mut state.shown);
        params.shown.write(&state.shown);
        state.count = state.count.wrapping_add(1);
        params.count.store(state.count, Ordering::Relaxed);
        ProcessStatus::Normal
    }

    fn editor(params: Arc<PrismaParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Prisma,
    params: PrismaParams,
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
        let params = PrismaParams::default();
        kit_ui::check_factory(&params, &ui::ids(&params), &ui::FACTORY);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/prisma_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/prisma_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
