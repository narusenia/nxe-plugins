//! Peek on truce: every Peek measurement at once, in one window the host can resize.
//!
//! **The signal goes through untouched** (`peek-plan.md`, decision 1): the
//! output is the input, bit for bit, with no latency. The measurement is
//! `peek_core`'s and the figure `peek_ui`'s, shared with `Peek`.

use peek_core::meters::{
    LevelMeter, LoudnessMeter, SPECTROGRAM_BANDS, SPECTRUM_BANDS, ScopeMeter, SpectrumMeter,
    StereoMeter, VuMeter,
};
use std::sync::Arc;
use truce::prelude::*;

mod params;
mod ui;

use params::PeekParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct Peek;

pub struct PeekState {
    level: LevelMeter,
    vu: VuMeter,
    stereo: StereoMeter,
    spectrum: SpectrumMeter<SPECTRUM_BANDS>,
    spectrogram: SpectrumMeter<SPECTROGRAM_BANDS>,
    loudness: LoudnessMeter,
    scope: ScopeMeter,
}

impl PeekState {
    fn new(sample_rate: f32) -> Self {
        Self {
            level: LevelMeter::new(sample_rate),
            vu: VuMeter::new(sample_rate),
            stereo: StereoMeter::new(sample_rate),
            spectrum: SpectrumMeter::new(sample_rate),
            spectrogram: SpectrumMeter::new(sample_rate),
            loudness: LoudnessMeter::new(sample_rate),
            scope: ScopeMeter::new(sample_rate),
        }
    }
}

impl Default for PeekState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for Peek {
    type Params = PeekParams;
    type DspState = PeekState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut PeekState, _params: &PeekParams, config: &AudioConfig) {
        *state = PeekState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut PeekState,
        params: &PeekParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        state.scope.set_window(params.window.value() / 1_000.0);
        // The output is the input: written back exactly as it was read.
        buffer.for_each_stereo_frame(|input, output| {
            *output = *input;
            let [left, right] = *input;
            state.level.push(left, right);
            state.vu.push(left, right);
            state.stereo.push(left, right);
            state.spectrum.push(left, right);
            state.spectrogram.push(left, right);
            state.loudness.push(left, right);
            state.scope.push(left, right);
        });
        let readings = &params.readings;
        state.level.publish(&readings.level);
        state.vu.publish(&readings.vu);
        state.stereo.publish(&readings.stereo);
        state.spectrum.publish(&readings.spectrum);
        state.spectrogram.publish(&readings.spectrogram);
        state.loudness.publish(&readings.loudness);
        state.scope.publish(&readings.scope);
        context.set_meter(
            params.peak.id(),
            peek_ui::level::position(state.level.peak()),
        );
        let _ = context;
        ProcessStatus::Normal
    }

    fn editor(params: Arc<PeekParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: Peek,
    params: PeekParams,
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
    fn passes_the_signal_through_without_allocating() {
        let result = truce_test::assert_no_audio_alloc(|| {
            driver!(Plugin)
                .duration(Duration::from_millis(50))
                .input(InputSource::Constant(0.5))
                .run()
        });
        assertions::assert_nonzero(&result);
        assertions::assert_no_nans(&result);
    }

    /// **The promise, checked**: what comes out is what went in, bit for
    /// bit, with no delay (`peek-plan.md`, decision 1).
    #[test]
    fn the_output_is_the_input() {
        fn signal(index: usize) -> f32 {
            (index as f32 * 0.37).sin() * 0.9 + (index as f32 * 0.011).cos() * 0.05
        }
        let result = driver!(Plugin)
            .duration(Duration::from_millis(50))
            .input(InputSource::Generator(Box::new(|index, _| signal(index))))
            .run();
        assert!(!result.output.is_empty());
        for channel in &result.output {
            for (index, sample) in channel.iter().enumerate() {
                assert_eq!(sample.to_bits(), signal(index).to_bits(), "frame {index}");
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_matches_its_screenshot_macos() {
        truce_test::screenshot!(Plugin, "screenshots/peek_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/peek_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
