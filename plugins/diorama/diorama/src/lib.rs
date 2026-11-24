//! NXE Diorama 2 on truce: parameter declarations, and the
//! wiring between them and `diorama_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! **The product name is settled** (`REQ-DIO-014`, `DIO-15`). It was
//! `NXE Vocal Depth` — a descriptive name in a line that otherwise uses
//! invented ones — and then `NXE Parallax`, which was dropped because five
//! others in this market already use it. `CLAP_ID` is written into a host's
//! project file, so this was the last moment it was free to move.

use analysis::METERS;
use diorama_core::Engine;
use diorama_core::reflections::TAPS;
use nxe_dsp::{Correlation, Level};
use std::sync::Arc;
use truce::prelude::*;

mod analysis;
mod params;
mod ui;

use params::DioramaParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `reset` replaces the engine with one at the real rate before any audio.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// The descriptor. Carries nothing; truce owns the state below.
pub struct Diorama;

/// Everything the audio thread owns. The editor never touches it — what it is
/// shown goes through `DioramaParams::analysis` (`analysis.rs`).
pub struct DioramaState {
    engine: Engine,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    direct_level: Level,
    reflected_level: Level,
    /// Of the reflections: whether the space survives a fold.
    correlation: Correlation,
}

impl DioramaState {
    /// The only place that allocates. Everything the audio thread touches is
    /// sized here, from the rate the host has just committed to.
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: Engine::new(sample_rate),
            meters: std::array::from_fn(|_| Level::new(sample_rate)),
            direct_level: Level::new(sample_rate),
            reflected_level: Level::new(sample_rate),
            correlation: Correlation::new(sample_rate),
        }
    }
}

impl Default for DioramaState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Diorama {
    type Params = DioramaParams;
    type DspState = DioramaState;

    /// Stereo, and **mono in, stereo out** (`REQ-DIO-011`): the reflections
    /// are a space, and a space is stereo.
    fn bus_layouts() -> Vec<BusLayout> {
        vec![
            BusLayout::stereo(),
            BusLayout::new()
                .with_input("Main", ChannelConfig::Mono)
                .with_output("Main", ChannelConfig::Stereo),
        ]
    }

    /// Off the audio thread, so rebuilding is allowed. The delay lines are
    /// sized in samples, so the engine is rebuilt rather than corrected.
    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut DioramaState, _params: &DioramaParams, config: &AudioConfig) {
        *state = DioramaState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut DioramaState,
        params: &DioramaParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        state.engine.set(params.macros(buffer.num_samples()));

        // A mono input arrives in both slots, which is the mirroring
        // `REQ-DIO-011` asks for.
        buffer.for_each_stereo_frame(|input, output| {
            let [dry_left, dry_right] = *input;
            let (out_left, out_right) = state.engine.process(dry_left, dry_right);
            *output = [out_left, out_right];

            let ((direct_left, direct_right), (room_left, room_right)) = state.engine.buses();
            state.direct_level.push((direct_left + direct_right) * 0.5);
            state.reflected_level.push((room_left + room_right) * 0.5);
            state.correlation.push(room_left, room_right);
            state.meters[0].push(dry_left);
            state.meters[1].push(dry_right);
            state.meters[2].push(out_left);
            state.meters[3].push(out_right);
        });

        // One frame per block. Reading, never writing.
        let analysis = &params.analysis;
        analysis
            .peaks
            .write(&std::array::from_fn(|index| state.meters[index].peak()));
        analysis
            .holds
            .write(&std::array::from_fn(|index| state.meters[index].hold()));
        analysis.buses.write(&[
            decibels(state.direct_level.rms()),
            decibels(state.reflected_level.rms()),
        ]);
        let pattern = state.engine.pattern();
        analysis
            .arrivals
            .write(&std::array::from_fn(|index| pattern[index].0));
        analysis
            .arrival_levels
            .write(&arrival_levels(&pattern, state.engine.direct_level()));
        analysis.clarity.write(&[state.engine.clarity_lift_db()]);
        analysis.correlation.write(&[state.correlation.value()]);
        let (corner, _) = state.engine.damping_corners_hz();
        analysis.damping.write(&[corner.unwrap_or(0.0)]);

        ProcessStatus::Normal
    }

    fn editor(params: Arc<DioramaParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

/// What an arrival reads as when there is no reflection bus at all — `ROOM` at
/// zero, where every weight is zero and the ratio is undefined rather than
/// small.
const SILENT_ARRIVAL_DB: f32 = -200.0;

/// Each arrival's level against the direct sound, in dB.
///
/// **Both sides are resolved from the macros, not measured.** A tap's weight
/// already carries the reflection bus's gain
/// (`diorama_core::Reflections::pattern`), and `direct_level` is what the
/// direct path is asked for — so this is a ratio of two things the knobs
/// decide. **It was the measured buses for one commit**, and the figure then
/// stopped moving unless something was playing: a window that cannot be set up
/// with the transport stopped is a window nobody sets up (`DIO-17`).
///
/// **One place, because the window and its test both need it.** A second copy
/// of this arithmetic would drift from the sound the first time a weight moved.
pub(crate) fn arrival_levels(pattern: &[(f32, f32); TAPS], direct_level: f32) -> [f32; TAPS] {
    std::array::from_fn(|index| {
        if direct_level <= 0.0 || pattern[index].1 <= 0.0 {
            return SILENT_ARRIVAL_DB;
        }
        decibels(pattern[index].1 / direct_level)
    })
}

/// An amplitude in dB, floored so a silent bus reads as silence rather than as
/// a very large negative number.
fn decibels(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-9).log10()
}

truce::plugin! {
    logic: Diorama,
    params: DioramaParams,
}

// A no-op unless the `rt-paranoid` feature is on, which only the tests enable.
truce::enable_rt_paranoid!();

#[cfg(test)]
mod tests {
    use super::*;
    use diorama_core::Macros;
    use std::time::Duration;
    use truce::params::Params;
    use truce_test::{InputSource, assertions, driver};

    /// The defaults as a host would hand them to the engine, with the
    /// smoothers at their targets.
    fn primed() -> DioramaParams {
        let params = DioramaParams::default();
        params.snap_smoothers();
        params
    }

    fn render(macros: Macros, left_in: &[f32], right_in: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let mut engine = Engine::new(FALLBACK_SAMPLE_RATE);
        engine.set(macros);
        let mut left = Vec::with_capacity(left_in.len());
        let mut right = Vec::with_capacity(left_in.len());
        for (&l, &r) in left_in.iter().zip(right_in) {
            let (out_left, out_right) = engine.process(l, r);
            left.push(out_left);
            right.push(out_right);
        }
        (left, right)
    }

    #[test]
    fn the_defaults_reach_the_engine() {
        let macros = primed().macros(1);
        assert_eq!(macros.depth, 0.5);
        assert_eq!(macros.direct, 0.5);
        assert_eq!(macros.room, 0.5);
        assert_eq!(macros.damping, 0.5);
        assert_eq!(macros.width, 0.6);
        assert_eq!(macros.mix, 1.0);
        assert_eq!(macros.output, 1.0);
    }

    /// **`MIX` = 0 is bit-identical**, through the parameters rather than
    /// through the core's own defaults — which is where a wrong output trim
    /// would show up (`REQ-DIO-001`).
    #[test]
    fn mix_zero_is_bit_identical() {
        let params = primed();
        for (id, value) in [
            (params.mix.id(), 0.0),
            (params.depth.id(), 0.9),
            (params.room.id(), 1.0),
            (params.damping.id(), 1.0),
        ] {
            params.set_plain(id, value);
        }
        params.snap_smoothers();

        let left_in: Vec<f32> = (0..4_096)
            .map(|index| ((index as f32) * 0.017).sin() * 0.7)
            .collect();
        let right_in: Vec<f32> = left_in.iter().rev().copied().collect();

        let (left, right) = render(params.macros(left_in.len()), &left_in, &right_in);
        for index in 0..left_in.len() {
            assert_eq!(left[index], left_in[index], "left at {index}");
            assert_eq!(right[index], right_in[index], "right at {index}");
        }
    }

    /// The same input in both channels still leaves the two outputs
    /// different, because the two tap sets share no time (`REQ-DIO-007`).
    #[test]
    fn a_mono_input_still_comes_out_stereo() {
        let mono: Vec<f32> = (0..8_192)
            .map(|index| ((index as f32) * 0.011).sin() * 0.5)
            .collect();
        let (left, right) = render(primed().macros(1), &mono, &mono);
        assert!(left.iter().all(|s| s.is_finite()));
        let difference: f32 = left
            .iter()
            .zip(&right)
            .skip(4_000)
            .map(|(l, r)| (l - r).abs())
            .sum();
        assert!(
            difference > 0.0,
            "a mono input came out with identical channels"
        );
    }

    /// Hostile values reach the core through this adapter, so the clamping has
    /// to survive the trip (`REQ-DIO-016`).
    #[test]
    fn hostile_macros_stay_finite() {
        let macros = Macros {
            depth: f32::NAN,
            room: f32::INFINITY,
            mix: 0.5,
            ..primed().macros(1)
        };
        let signal: Vec<f32> = (0..2_048)
            .map(|index| ((index as f32) * 0.03).sin() * 0.5)
            .collect();
        let (left, right) = render(macros, &signal, &signal);
        assert!(left.iter().all(|s| s.is_finite()));
        assert!(right.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn info_is_valid() {
        truce_test::assert_valid_info::<Plugin>();
    }

    #[test]
    fn has_editor() {
        truce_test::assert_has_editor::<Plugin>();
    }

    /// Every parameter comes back from a save, **away from the defaults**.
    #[test]
    fn state_round_trips() {
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
    /// OS, baked on CI, never on a laptop.
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
