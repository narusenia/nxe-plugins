//! The NXE Vocal Depth nih-plug wrapper: parameter declarations, and the
//! wiring between them and `vocal_depth_core`.
//!
//! No DSP lives here (`.agents/rules/rust.md`).
//!
//! The window is `ui`. **One screen, no tabs** (`REQ-VDP-013`).
//!
//! **The product name is provisional** (`REQ-VDP-014`). `NXE Vocal Depth` is a
//! descriptive name in a line that otherwise uses invented ones, and
//! `CLAP_ID` cannot change once a host has stored it — so the name has to be
//! settled **before the first release**, not before this unit.

use analysis::{Analysis, METERS};
use nih_plug::prelude::*;
use nxe_dsp::{Correlation, Level};
use std::sync::Arc;
use vocal_depth_core::Engine;

mod analysis;
mod params;
mod ui;

use params::VocalDepthParams;

/// The sample rate the engine is built for before a host says otherwise.
/// `initialize` replaces the engine when the real rate differs.
const FALLBACK_SAMPLE_RATE: f32 = 48_000.0;

struct VocalDepth {
    params: Arc<VocalDepthParams>,
    /// The window's size and position, which the host saves with the project.
    editor_state: Arc<nih_plug_vizia::ViziaState>,
    /// What the editor reads. **The audio thread writes; nothing else touches
    /// the analysers below** (`analysis.rs`).
    analysis: Arc<Analysis>,
    /// IN L, IN R, OUT L, OUT R.
    meters: [Level; METERS],
    /// The two buses, so the readout can print the ratio that *is* the distance
    /// (`REQ-VDP-018`).
    direct_level: Level,
    reflected_level: Level,
    /// Of the **reflections**, not the output: the promise is about what the
    /// room does (`REQ-VDP-007`).
    correlation: Correlation,
    engine: Engine,
    sample_rate: f32,
    /// How many input channels the host actually negotiated. Under the mono
    /// layout there is one, and reading a second would read undefined data.
    input_channels: usize,
}

impl Default for VocalDepth {
    fn default() -> Self {
        Self {
            params: Arc::new(VocalDepthParams::default()),
            editor_state: ui::default_state(),
            analysis: Arc::new(Analysis::default()),
            meters: std::array::from_fn(|_| Level::new(FALLBACK_SAMPLE_RATE)),
            direct_level: Level::new(FALLBACK_SAMPLE_RATE),
            reflected_level: Level::new(FALLBACK_SAMPLE_RATE),
            correlation: Correlation::new(FALLBACK_SAMPLE_RATE),
            engine: Engine::new(FALLBACK_SAMPLE_RATE),
            sample_rate: FALLBACK_SAMPLE_RATE,
            input_channels: 2,
        }
    }
}

impl Plugin for VocalDepth {
    const NAME: &'static str = "NXE Vocal Depth";
    const VENDOR: &'static str = "NXE";
    const URL: &'static str = "https://github.com/narusenia/nxe-plugins";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        // **Mono in, stereo out** (`REQ-VDP-011`). The reflections are stereo by
        // construction — the two channels read different tap sets — so a
        // mono-out layout would throw away what the product is for. **There is
        // no 1→1.**
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
    ];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        ui::create(
            self.params.clone(),
            self.editor_state.clone(),
            self.analysis.clone(),
        )
    }

    /// The only place that allocates. The reflection delay lines are sized from
    /// the rate the host has just committed to (`REQ-VDP-016`).
    ///
    /// **No latency is reported, and that is not an omission** — nih-plug's
    /// default is zero and the plugin has none: the reflections arrive *after*
    /// the direct sound, which is what a reflection is, and the direct path has
    /// no bulk delay in it (`REQ-VDP-012`).
    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.input_channels = audio_io_layout
            .main_input_channels
            .map_or(0, |channels| channels.get() as usize);

        if buffer_config.sample_rate != self.sample_rate {
            self.sample_rate = buffer_config.sample_rate;
            // The tap delays are milliseconds and the lines are samples, so a
            // new rate is a new set of lines rather than a correction
            // (`REQ-VDP-017`).
            self.engine = Engine::new(self.sample_rate);
            self.meters = std::array::from_fn(|_| Level::new(self.sample_rate));
            self.direct_level = Level::new(self.sample_rate);
            self.reflected_level = Level::new(self.sample_rate);
            self.correlation = Correlation::new(self.sample_rate);
        }
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.direct_level.reset();
        self.reflected_level.reset();
        self.correlation.reset();
        for meter in &mut self.meters {
            meter.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let samples = buffer.samples();
        // Once per block: this is what rebuilds the presence coefficients, the
        // tap weights and the loudness normalisation
        // (`vocal_depth_core::Engine::set`).
        self.engine.set(self.params.macros(samples as u32));

        let channels = buffer.as_slice();
        // Both layouts hand back two output channels, so this cannot fail. Bail
        // rather than index, because a wrong guess here would be a panic on the
        // audio thread.
        let [left, right, ..] = channels else {
            return ProcessStatus::Normal;
        };
        let stereo = self.input_channels >= 2;

        for sample in 0..samples {
            let dry_left = left[sample];
            // A mono input leaves the second channel undefined, so mirror the
            // first instead of reading it (`REQ-VDP-011`).
            let dry_right = if stereo { right[sample] } else { dry_left };

            let (out_left, out_right) = self.engine.process(dry_left, dry_right);
            left[sample] = out_left;
            right[sample] = out_right;

            // **The buses taken directly**, never `out − dry` (`analysis.rs`).
            let ((direct_left, direct_right), (room_left, room_right)) = self.engine.buses();
            self.direct_level.push((direct_left + direct_right) * 0.5);
            self.reflected_level.push((room_left + room_right) * 0.5);
            self.correlation.push(room_left, room_right);
            self.meters[0].push(dry_left);
            self.meters[1].push(dry_right);
            self.meters[2].push(out_left);
            self.meters[3].push(out_right);
        }

        // One frame per block. The editor reads whatever is there — a frame it
        // misses is a frame nobody would have seen.
        //
        // **Reading, never writing** (`REQ-VDP-018`): everything here comes out
        // of the engine, and nothing here goes back in. Stopping the analysis
        // would not change a sample.
        self.analysis
            .peaks
            .write(&std::array::from_fn(|index| self.meters[index].peak()));
        self.analysis
            .holds
            .write(&std::array::from_fn(|index| self.meters[index].hold()));
        self.analysis.buses.write(&[
            decibels(self.direct_level.rms()),
            decibels(self.reflected_level.rms()),
        ]);

        let pattern = self.engine.pattern();
        self.analysis
            .arrivals
            .write(&std::array::from_fn(|index| pattern[index].0));
        // **Against the loudest weight the design can produce**, so the figure
        // does not rescale itself as `ROOM` is turned down — a picture whose
        // ceiling moves cannot be read for level.
        self.analysis
            .arrival_levels
            .write(&std::array::from_fn(|index| {
                (pattern[index].1 / ARRIVAL_CEILING).clamp(0.0, 1.0)
            }));

        self.analysis
            .clarity
            .write(&[self.engine.clarity_lift_db()]);
        self.analysis.correlation.write(&[self.correlation.value()]);
        let (corner, _) = self.engine.damping_corners_hz();
        self.analysis.damping.write(&[corner.unwrap_or(0.0)]);

        ProcessStatus::Normal
    }
}

/// The tap weight the figure treats as full height.
///
/// **A fixed ceiling, not the loudest weight of the moment.** A figure that
/// rescales itself cannot be read for level — turning `ROOM` down would leave
/// the picture unchanged, which is exactly the thing it is there to show. The
/// number is the earliest tap at full amount and `ROOM` at its top
/// (`vocal_depth_core::reflections`).
const ARRIVAL_CEILING: f32 = 2.0;

/// An amplitude in dB, floored so a silent bus reads as silence rather than as
/// a very large negative number.
fn decibels(amplitude: f32) -> f32 {
    20.0 * amplitude.max(1e-9).log10()
}

impl ClapPlugin for VocalDepth {
    // **Never changeable once shipped**: a host stores it in the project file
    // (`AGENTS.md`). Provisional only because nothing has shipped
    // (`REQ-VDP-014`).
    const CLAP_ID: &'static str = "com.nxe.vocaldepth";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("A mix processor that moves a voice forward and back");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Reverb,
    ];
}

impl Vst3Plugin for VocalDepth {
    // Sixteen bytes, and **never changeable once shipped**, for the same reason
    // as `CLAP_ID`.
    const VST3_CLASS_ID: [u8; 16] = *b"NXEVocalDepth...";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}

nih_export_clap!(VocalDepth);
nih_export_vst3!(VocalDepth);

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything the audio path reads, woken to its default the way a host
    /// wakes it in `initialize`.
    ///
    /// **A fresh `FloatParam`'s smoother starts at zero, not at the default**
    /// (`AIR-13` measured a layer at -200 dB before this existed). A test that
    /// skips this measures a plugin with every macro at its lower bound.
    fn primed() -> VocalDepth {
        let plugin = VocalDepth::default();
        let params = &plugin.params;
        params.depth.smoothed.reset(params.depth.value());
        params.direct.smoothed.reset(params.direct.value());
        params.room.smoothed.reset(params.room.value());
        params.damping.smoothed.reset(params.damping.value());
        params.width.smoothed.reset(params.width.value());
        params.mix.smoothed.reset(params.mix.value());
        params.output.smoothed.reset(params.output.value());
        plugin
    }

    fn render(plugin: &mut VocalDepth, left_in: &[f32], right_in: &[f32]) -> (Vec<f32>, Vec<f32>) {
        plugin
            .engine
            .set(plugin.params.macros(left_in.len() as u32));
        let mut left = Vec::with_capacity(left_in.len());
        let mut right = Vec::with_capacity(left_in.len());
        for (&l, &r) in left_in.iter().zip(right_in) {
            let (out_left, out_right) = plugin.engine.process(l, r);
            left.push(out_left);
            right.push(out_right);
        }
        (left, right)
    }

    /// The smoothers really are awake, so every other test here is measuring
    /// the plugin a host would load rather than one pinned at its minimum
    /// (`AIR-13`).
    #[test]
    fn the_defaults_are_primed() {
        let plugin = primed();
        let macros = plugin.params.macros(1);
        assert_eq!(macros.depth, 0.5);
        assert_eq!(macros.direct, 0.5);
        assert_eq!(macros.room, 0.5);
        assert_eq!(macros.damping, 0.5);
        assert_eq!(macros.width, 0.6);
        assert_eq!(macros.mix, 1.0);
        assert_eq!(macros.output, 1.0);
    }

    /// **`MIX` = 0 is bit-identical**, through the parameters rather than
    /// through the core's own defaults — which is where a wrong output trim or
    /// an asleep smoother would show up (`REQ-VDP-001`).
    #[test]
    fn mix_zero_is_bit_identical() {
        let mut plugin = primed();
        plugin.params.mix.smoothed.reset(0.0);
        plugin.params.depth.smoothed.reset(0.9);
        plugin.params.room.smoothed.reset(1.0);
        plugin.params.damping.smoothed.reset(1.0);
        plugin.engine.reset();

        let left_in: Vec<f32> = (0..4_096)
            .map(|index| ((index as f32) * 0.017).sin() * 0.7)
            .collect();
        let right_in: Vec<f32> = left_in.iter().rev().copied().collect();

        let (left, right) = render(&mut plugin, &left_in, &right_in);
        for index in 0..left_in.len() {
            assert_eq!(left[index], left_in[index], "left at {index}");
            assert_eq!(right[index], right_in[index], "right at {index}");
        }
    }

    /// A mono input must not read the second channel, and the reflections still
    /// come out stereo (`REQ-VDP-011`).
    #[test]
    fn a_mono_input_is_mirrored_and_still_comes_out_stereo() {
        let mut plugin = primed();
        plugin.input_channels = 1;
        plugin.engine.reset();

        let mono: Vec<f32> = (0..8_192)
            .map(|index| ((index as f32) * 0.011).sin() * 0.5)
            .collect();
        // The wrapper mirrors before the engine sees it, so this is the same
        // call `process` makes for a mono layout.
        let (left, right) = render(&mut plugin, &mono, &mono);

        assert!(left.iter().all(|s| s.is_finite()));
        assert!(right.iter().all(|s| s.is_finite()));

        // The same input in both channels still leaves the two outputs
        // different, because the two tap sets share no time
        // (`REQ-VDP-007`).
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

    /// Hostile parameter values reach the core through this adapter, so the
    /// clamping has to survive the trip (`REQ-VDP-016`).
    #[test]
    fn hostile_parameters_stay_finite() {
        let mut plugin = primed();
        plugin.params.depth.smoothed.reset(f32::NAN);
        plugin.params.room.smoothed.reset(f32::INFINITY);
        plugin.params.mix.smoothed.reset(0.5);
        plugin.engine.reset();

        let signal: Vec<f32> = (0..2_048)
            .map(|index| ((index as f32) * 0.03).sin() * 0.5)
            .collect();
        let (left, right) = render(&mut plugin, &signal, &signal);
        assert!(left.iter().all(|s| s.is_finite()));
        assert!(right.iter().all(|s| s.is_finite()));
    }
}
