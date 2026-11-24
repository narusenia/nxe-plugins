//! NXE Doubler 2 on truce: parameter declarations, and the wiring between them
//! and `doubler_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).

use analysis::{BANDS, HIGH_HZ, LOW_HZ, METERS, PAN_BINS};
use doubler_core::VoiceEngine;
use nxe_dsp::{Correlation, Level, PanScope, Spectrum};
use std::sync::Arc;
use truce::prelude::*;

mod analysis;
mod params;
mod ui;

use params::DoublerParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `reset` replaces the engine with one at the real rate before any audio.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

/// The descriptor. Carries nothing; truce owns the state below.
pub struct Doubler;

/// Everything the audio thread owns. The editor never touches it — what it is
/// shown goes through `DoublerParams::analysis` (`analysis.rs`).
pub struct DoublerState {
    engine: VoiceEngine,
    pan_scope: PanScope<PAN_BINS>,
    spectrum: Spectrum<BANDS>,
    /// IN left, IN right, OUT left, OUT right.
    meters: [Level; METERS],
    correlation: Correlation,
}

impl DoublerState {
    /// The only place that allocates. Every buffer the audio thread touches is
    /// sized here, from the rate the host has just committed to
    /// (`REQ-DBL-011`).
    fn new(sample_rate: f32) -> Self {
        Self {
            engine: VoiceEngine::new(sample_rate),
            pan_scope: PanScope::new(sample_rate),
            spectrum: Spectrum::new(sample_rate, LOW_HZ, HIGH_HZ),
            meters: std::array::from_fn(|_| Level::new(sample_rate)),
            correlation: Correlation::new(sample_rate),
        }
    }
}

impl Default for DoublerState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Doubler {
    type Params = DoublerParams;
    type DspState = DoublerState;

    /// Stereo, and mono in / stereo out. The wet signal is stereo by
    /// definition — the voices are spread across the image — so a mono-out
    /// layout would throw away the point of the plugin.
    fn bus_layouts() -> Vec<BusLayout> {
        vec![
            BusLayout::stereo(),
            BusLayout::new()
                .with_input("Main", ChannelConfig::Mono)
                .with_output("Main", ChannelConfig::Stereo),
        ]
    }

    /// Off the audio thread, so rebuilding is allowed. Rebuilt rather than
    /// cleared because the analysers hold time constants and filter
    /// coefficients in samples — and it also clears the picture from before
    /// the transport stopped, which would otherwise decay on screen as if
    /// something were playing.
    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut DoublerState, _params: &DoublerParams, config: &AudioConfig) {
        *state = DoublerState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut DoublerState,
        params: &DoublerParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        _context: &mut ProcessContext,
    ) -> ProcessStatus {
        // A mono input arrives in both slots of the frame, which is exactly
        // what `MonoSum` should see from a mono source (`REQ-DBL-004`).
        buffer.for_each_stereo_frame(|input, output| {
            let macros = params.macros();
            let shape = params.shape();
            let [dry_left, dry_right] = *input;

            let (wet_left, wet_right) = state.engine.process(dry_left, dry_right, &macros, &shape);

            let mix = params.mix.read();
            let gain = db_to_linear(params.output.read());

            // Dry is never delayed or filtered, which is why the plugin
            // reports no latency (`REQ-DBL-006`). At `mix == 0` this is the
            // input multiplied by one, so a bypassed plugin is bit-transparent.
            let left = (dry_left * (1.0 - mix) + wet_left * mix) * gain;
            let right = (dry_right * (1.0 - mix) + wet_right * mix) * gain;
            *output = [left, right];

            // **The meters straddle the plugin**, so "is this louder or is it
            // better" can be asked of the two ends of the same comparison.
            state.meters[0].push(dry_left);
            state.meters[1].push(dry_right);
            state.meters[2].push(left);
            state.meters[3].push(right);
            // Of the output, because what a fold would do to it is what the
            // reading is for.
            state.correlation.push(left, right);
            // The picture is of the output — where the sound *is*.
            state.pan_scope.push(left, right);
            // The spectrum is of the wet bus, because `Tone` acts on it.
            // Summed to mono: one curve cannot say two things.
            state.spectrum.push((wet_left + wet_right) * 0.5);
        });

        // Once per block, not per sample: these are stores to shared atomics.
        let analysis = &params.analysis;
        analysis.pan.write(&state.pan_scope.levels());
        analysis.spectrum.write(&state.spectrum.levels());
        analysis
            .peaks
            .write(&std::array::from_fn(|index| state.meters[index].peak()));
        analysis
            .holds
            .write(&std::array::from_fn(|index| state.meters[index].hold()));
        analysis.correlation.write(&[state.correlation.value()]);

        ProcessStatus::Normal
    }

    fn editor(params: Arc<DoublerParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Doubler,
    params: DoublerParams,
}

// A no-op unless the `rt-paranoid` feature is on, which only the tests enable.
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

    /// Every parameter comes back from a save — the half of "a project reopens
    /// as it was left" that does not need a host.
    #[test]
    fn state_round_trips() {
        truce_test::assert_state_round_trip::<Plugin>();
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
