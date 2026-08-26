//! Parameter declarations and the mapping to `doubler_core`'s plain values.
//!
//! This is the adapter the architecture calls for: `doubler-core` knows nothing
//! about nih-plug, so the translation lives here
//! (`docs/specifications/architecture.md`).
//!
//! The layer split is the one `REQ-DBL-007` describes — macros in their natural
//! units, per-voice values as a normalized shape the macros scale. **Neither
//! writes to the other**, here or anywhere else.
//!
//! Smoothing times come from the table in
//! `plugins/doubler/docs/specifications/dsp.md`. They differ per parameter
//! because the path from the value to the sound differs: a gain can move in
//! 20 ms, but a read position moving that fast is an audible pitch transient.

use doubler_core::{DEFAULT_SHAPE, MAX_VOICES, Macros, Source, VoiceShape, Voices};
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// How many voices are live.
///
/// A separate type from `doubler_core::Voices` on purpose: deriving nih-plug's
/// `Enum` on the core type would make the core depend on nih-plug.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum VoicesParam {
    #[id = "2"]
    #[name = "2"]
    Two,
    #[id = "4"]
    #[name = "4"]
    Four,
    #[id = "8"]
    #[name = "8"]
    Eight,
}

impl From<VoicesParam> for Voices {
    fn from(value: VoicesParam) -> Self {
        match value {
            VoicesParam::Two => Voices::Two,
            VoicesParam::Four => Voices::Four,
            VoicesParam::Eight => Voices::Eight,
        }
    }
}

/// Where the voices take their input from.
///
/// A separate type from `doubler_core::Source` for the same reason as
/// `VoicesParam`.
#[derive(Enum, Debug, PartialEq, Eq, Clone, Copy)]
pub enum SourceParam {
    #[id = "mono"]
    #[name = "Mono Sum"]
    MonoSum,
    #[id = "stereo"]
    #[name = "True Stereo"]
    TrueStereo,
}

impl From<SourceParam> for Source {
    fn from(value: SourceParam) -> Self {
        match value {
            SourceParam::MonoSum => Source::MonoSum,
            SourceParam::TrueStereo => Source::TrueStereo,
        }
    }
}

#[derive(Params)]
pub struct DoublerParams {
    /// The editor's size. Persisted with the parameters so a project reopens
    /// looking the way it was left.
    #[persist = "editor-state"]
    pub editor_state: Arc<ViziaState>,

    /// Whether the Detail tab is the one showing. Not a parameter: it does not
    /// affect the sound and automating it would be meaningless
    /// (`REQ-DBL-008`). The editor's model is what the display binds to; this
    /// is the copy that survives closing the project.
    #[persist = "detail-tab"]
    pub detail_tab: Arc<AtomicBool>,

    /// Whether editing one of a voice's shape axes writes its partner too
    /// (`REQ-DBL-014`). One flag per axis: a single switch plus a per-axis
    /// exception would mean two pieces of state deciding one write.
    ///
    /// Not parameters, and deliberately so: they change what an edit does, not
    /// what the plugin sounds like, so automating them would automate the user
    /// interface. Persisted for the same reason `detail_tab` is.
    #[persist = "mirror-pan"]
    pub mirror_pan: Arc<AtomicBool>,
    #[persist = "mirror-detune"]
    pub mirror_detune: Arc<AtomicBool>,
    #[persist = "mirror-delay"]
    pub mirror_delay: Arc<AtomicBool>,
    #[persist = "mirror-gain"]
    pub mirror_gain: Arc<AtomicBool>,

    #[id = "voices"]
    pub voices: EnumParam<VoicesParam>,
    #[id = "source"]
    pub source: EnumParam<SourceParam>,
    #[id = "detune"]
    pub detune: FloatParam,
    #[id = "delay"]
    pub delay: FloatParam,
    #[id = "spread"]
    pub spread: FloatParam,
    #[id = "human"]
    pub humanize: FloatParam,
    #[id = "tonelo"]
    pub tone_lo: FloatParam,
    #[id = "tonehi"]
    pub tone_hi: FloatParam,
    #[id = "tonespr"]
    pub tone_spread: FloatParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "drygain"]
    pub dry_gain: FloatParam,
    #[id = "output"]
    pub output: FloatParam,

    /// The shape layer. Ids get a `_1`..`_8` suffix.
    ///
    /// All eight exist whatever `voices` says: nih-plug declares parameters
    /// once at startup and cannot add them later (`REQ-DBL-001`).
    #[nested(array, group = "Voice")]
    pub shape: [VoiceParams; MAX_VOICES],
}

/// One voice's shape. Normalized, except `gain` which has no macro.
#[derive(Params)]
pub struct VoiceParams {
    #[id = "vdly"]
    pub delay: FloatParam,
    #[id = "vdet"]
    pub detune: FloatParam,
    #[id = "vpan"]
    pub pan: FloatParam,
    #[id = "vgain"]
    pub gain: FloatParam,
}

impl Default for DoublerParams {
    fn default() -> Self {
        let defaults = Macros::default();

        Self {
            editor_state: crate::ui::default_state(),
            detail_tab: Arc::new(AtomicBool::new(false)),
            // On for the two axes the figure is read as symmetric on — angle
            // and radius — so dragging a dot cannot lean the image by accident.
            // Off for the two that are not in the picture: an asymmetric delay
            // and a detune that is not quite its partner's opposite are where a
            // doubler's thickness comes from, so those start free.
            mirror_pan: Arc::new(AtomicBool::new(true)),
            mirror_gain: Arc::new(AtomicBool::new(true)),
            mirror_detune: Arc::new(AtomicBool::new(false)),
            mirror_delay: Arc::new(AtomicBool::new(false)),

            voices: EnumParam::new("Voices", VoicesParam::Four),
            source: EnumParam::new("Source", SourceParam::MonoSum),

            detune: FloatParam::new(
                "Detune",
                defaults.detune,
                FloatRange::Linear {
                    min: 0.0,
                    max: 50.0,
                },
            )
            .with_unit(" ct")
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            delay: FloatParam::new(
                "Delay",
                defaults.delay,
                FloatRange::Linear {
                    min: 0.0,
                    max: 80.0,
                },
            )
            .with_unit(" ms")
            // Slower than the rest: moving a read position *is* a pitch change,
            // so a fast ramp is heard as a glide rather than as a delay edit.
            .with_smoother(SmoothingStyle::Linear(100.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            spread: FloatParam::new(
                "Spread",
                defaults.spread,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            humanize: FloatParam::new(
                "Humanize",
                defaults.humanize,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            tone_lo: FloatParam::new(
                "Tone Lo",
                defaults.tone_lo,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            tone_hi: FloatParam::new(
                "Tone Hi",
                defaults.tone_hi,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            tone_spread: FloatParam::new(
                "Tone Spread",
                defaults.tone_spread,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_percentage(0))
            .with_string_to_value(formatters::s2v_f32_percentage()),

            mix: FloatParam::new("Mix", 0.4, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit(" %")
                .with_smoother(SmoothingStyle::Linear(20.0))
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),

            // Same range as a voice's `Gain`, so the Voice Field's radius means
            // one thing whether it is under a dot or under the source marker
            // (`plugins/doubler/docs/specifications/ui.md`).
            dry_gain: FloatParam::new(
                "Dry Gain",
                0.0,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            output: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            shape: std::array::from_fn(VoiceParams::new),
        }
    }
}

impl VoiceParams {
    /// Defaults come from the core's shape table, so the sound out of the box
    /// is the one `dsp.md` describes and there is one place to change it.
    fn new(index: usize) -> Self {
        let default = DEFAULT_SHAPE[index];

        Self {
            delay: FloatParam::new(
                "Delay",
                default.delay,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(100.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            detune: FloatParam::new(
                "Detune",
                default.detune,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(30.0))
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            pan: FloatParam::new(
                "Pan",
                default.pan,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_panning())
            .with_string_to_value(formatters::s2v_f32_panning()),

            gain: FloatParam::new(
                "Gain",
                default.gain_db,
                FloatRange::Linear {
                    min: -24.0,
                    max: 6.0,
                },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(formatters::v2s_f32_rounded(1)),
        }
    }
}

impl DoublerParams {
    /// The macro layer for this sample.
    pub fn macros(&self) -> Macros {
        Macros {
            voices: self.voices.value().into(),
            source: self.source.value().into(),
            detune: self.detune.smoothed.next(),
            delay: self.delay.smoothed.next(),
            spread: self.spread.smoothed.next(),
            humanize: self.humanize.smoothed.next(),
            tone_lo: self.tone_lo.smoothed.next(),
            tone_hi: self.tone_hi.smoothed.next(),
            tone_spread: self.tone_spread.smoothed.next(),
        }
    }

    /// The shape layer for this sample.
    ///
    /// **Every voice is read, live or not.** A smoother only advances when it
    /// is polled, so skipping the inactive ones would leave them holding a
    /// stale value to jump from when `Voices` goes back up.
    pub fn shape(&self) -> [VoiceShape; MAX_VOICES] {
        std::array::from_fn(|i| {
            let params = &self.shape[i];
            VoiceShape {
                delay: params.delay.smoothed.next(),
                detune: params.detune.smoothed.next(),
                pan: params.pan.smoothed.next(),
                gain_db: params.gain.smoothed.next(),
            }
        })
    }
}
