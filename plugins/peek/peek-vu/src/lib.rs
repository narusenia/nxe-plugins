//! Peek VU on truce: a VU meter for both channels, with their correlation.
//!
//! **The signal goes through untouched** (`peek-plan.md`, decision 1): the
//! output is the input, bit for bit, with no latency. The measurement is
//! `peek_core`'s and the figure `peek_ui`'s, shared with `Peek`.

use std::sync::Arc;
use truce::prelude::*;

mod params;
mod ui;

use params::PeekVuParams;

const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

pub struct PeekVu;

pub struct PeekVuState {
    meter: peek_core::meters::VuMeter,
}

impl PeekVuState {
    fn new(sample_rate: f32) -> Self {
        Self {
            meter: peek_core::meters::VuMeter::new(sample_rate),
        }
    }
}

impl Default for PeekVuState {
    fn default() -> Self {
        Self::new(FALLBACK_SAMPLE_RATE)
    }
}

impl PluginLogic for PeekVu {
    type Params = PeekVuParams;
    type DspState = PeekVuState;

    fn bus_layouts() -> Vec<BusLayout> {
        BusLayout::stereo_and_mono()
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reset(state: &mut PeekVuState, _params: &PeekVuParams, config: &AudioConfig) {
        *state = PeekVuState::new(config.sample_rate as f32);
    }

    fn process(
        state: &mut PeekVuState,
        params: &PeekVuParams,
        buffer: &mut AudioBuffer,
        _events: &EventList,
        context: &mut ProcessContext,
    ) -> ProcessStatus {
        // The output is the input: written back exactly as it was read.
        buffer.for_each_stereo_frame(|input, output| {
            *output = *input;
            state.meter.push(input[0], input[1]);
        });
        state.meter.publish(&params.readings);
        let _ = context;
        ProcessStatus::Normal
    }

    fn editor(params: Arc<PeekVuParams>) -> Box<dyn Editor> {
        ui::create(params)
    }
}

truce::plugin! {
    logic: PeekVu,
    params: PeekVuParams,
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
        truce_test::screenshot!(Plugin, "screenshots/peek_vu_macos.png")
            .pixel_threshold(2)
            .run();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn window_matches_its_screenshot_windows() {
        truce_test::screenshot!(Plugin, "screenshots/peek_vu_windows.png")
            .pixel_threshold(2)
            .run();
    }
}
