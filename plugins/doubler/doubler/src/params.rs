//! Parameter declarations and the mapping to `doubler_core`'s plain values.
//!
//! This is the adapter the architecture calls for: `doubler-core` knows nothing
//! about truce, so the translation lives here
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
//!
//! **Ids are truce's hash of the field name**, so renaming a field is renaming
//! a parameter for every saved session. The display `name` is free to change.

use crate::analysis::Analysis;
use doubler_core::{MAX_VOICES, Macros, Source, VoiceShape, Voices};
use nxe_egui::text;
use truce::prelude::*;

/// How many voices are live.
///
/// A separate type from `doubler_core::Voices` on purpose: deriving truce's
/// `ParamEnum` on the core type would make the core depend on truce.
#[derive(ParamEnum, Debug)]
pub enum VoicesParam {
    #[name = "2"]
    Two,
    #[name = "4"]
    Four,
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

/// Where the voices take their input from. Separate from
/// `doubler_core::Source` for the same reason as `VoicesParam`.
#[derive(ParamEnum, Debug)]
pub enum SourceParam {
    #[name = "Mono Sum"]
    MonoSum,
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
    /// What the editor is shown about the sound. Not a parameter: the audio
    /// thread writes it and the editor reads it, and the params `Arc` is the
    /// one thing both are handed (`analysis.rs`).
    #[skip]
    pub analysis: Analysis,

    /// `Four` by default.
    #[param(name = "Voices", default = 1, parse = "voices")]
    pub voices: EnumParam<VoicesParam>,
    #[param(name = "Source", parse = "source")]
    pub source: EnumParam<SourceParam>,
    #[param(
        name = "Detune",
        parse = "number",
        range = "linear(0, 50)",
        default = 12.0,
        format = "cents",
        smooth = "linear(30)"
    )]
    pub detune: FloatParam,
    /// Slower than the rest: moving a read position *is* a pitch change, so a
    /// fast ramp is heard as a glide rather than as a delay edit.
    #[param(
        name = "Delay",
        parse = "number",
        range = "linear(0, 80)",
        default = 22.0,
        unit = "ms",
        smooth = "linear(100)"
    )]
    pub delay: FloatParam,
    #[param(
        name = "Spread",
        parse = "percent",
        range = "linear(0, 1)",
        default = 0.7,
        unit = "%",
        smooth = "linear(20)"
    )]
    pub spread: FloatParam,
    #[param(
        name = "Humanize",
        parse = "percent",
        range = "linear(0, 1)",
        default = 0.35,
        unit = "%",
        smooth = "linear(20)"
    )]
    pub humanize: FloatParam,
    #[param(
        name = "Tone Lo",
        parse = "number",
        range = "linear(-12, 12)",
        unit = "dB",
        smooth = "linear(20)"
    )]
    pub tone_lo: FloatParam,
    #[param(
        name = "Tone Hi",
        parse = "number",
        range = "linear(-12, 12)",
        unit = "dB",
        smooth = "linear(20)"
    )]
    pub tone_hi: FloatParam,
    #[param(
        name = "Tone Spread",
        parse = "percent",
        range = "linear(0, 1)",
        unit = "%",
        smooth = "linear(20)"
    )]
    pub tone_spread: FloatParam,
    #[param(
        name = "Mix",
        parse = "percent",
        range = "linear(0, 1)",
        default = 0.4,
        unit = "%",
        smooth = "linear(20)"
    )]
    pub mix: FloatParam,
    #[param(
        name = "Output",
        parse = "number",
        range = "linear(-12, 12)",
        unit = "dB",
        smooth = "linear(20)"
    )]
    pub output: FloatParam,

    /// The shape layer. All eight exist whatever `voices` says: a plugin
    /// declares its parameters once and cannot add them later (`REQ-DBL-001`).
    #[nested]
    pub voice1: Voice1,
    #[nested]
    pub voice2: Voice2,
    #[nested]
    pub voice3: Voice3,
    #[nested]
    pub voice4: Voice4,
    #[nested]
    pub voice5: Voice5,
    #[nested]
    pub voice6: Voice6,
    #[nested]
    pub voice7: Voice7,
    #[nested]
    pub voice8: Voice8,
}

impl DoublerParams {
    fn cents(&self, value: f64) -> String {
        format!("{value:.1} ct")
    }

    // **Every parameter reads its own text back** (`nxe_egui::text`): a typed
    // value goes through these, and so does a host's text entry.
    fn number(&self, text: &str) -> Option<f64> {
        text::number(text)
    }

    fn percent(&self, text: &str) -> Option<f64> {
        text::percent(text)
    }

    fn voices(&self, text: &str) -> Option<f64> {
        text::choice(&["2", "4", "8"], text)
    }

    fn source(&self, text: &str) -> Option<f64> {
        text::choice(&["Mono Sum", "True Stereo"], text)
    }
}

/// One voice's shape. Normalized, except `gain` which has no macro.
///
/// **Eight types, not one.** truce fixes a parameter's default on its type, and
/// each voice's default is its row of `doubler_core::DEFAULT_SHAPE`. The test
/// `voice_defaults_are_the_core_table` keeps the literals here equal to that
/// table, so it is still the one place to change them.
macro_rules! voice {
    ($ty:ident, $group:tt, $delay_name:tt, $detune_name:tt, $pan_name:tt, $gain_name:tt,
     delay = $delay:tt, detune = [$($detune:tt)+], pan = [$($pan:tt)+]) => {
        #[derive(Params)]
        pub struct $ty {
            #[param(
                name = $delay_name,
                parse = "number",
                group = $group,
                range = "linear(0, 1)",
                default = $delay,
                smooth = "linear(100)"
            )]
            pub delay: FloatParam,
            #[param(
                name = $detune_name,
                parse = "number",
                group = $group,
                range = "linear(-1, 1)",
                default = $($detune)+,
                smooth = "linear(30)"
            )]
            pub detune: FloatParam,
            #[param(
                name = $pan_name,
                parse = "pan",
                group = $group,
                range = "linear(-1, 1)",
                default = $($pan)+,
                unit = "pan",
                smooth = "linear(20)"
            )]
            pub pan: FloatParam,
            #[param(
                name = $gain_name,
                parse = "number",
                group = $group,
                range = "linear(-24, 6)",
                unit = "dB",
                smooth = "linear(20)"
            )]
            pub gain: FloatParam,
        }

        impl $ty {
            fn number(&self, text: &str) -> Option<f64> {
                text::number(text)
            }

            fn pan(&self, text: &str) -> Option<f64> {
                text::pan(text)
            }

            fn shape(&self) -> VoiceShape {
                VoiceShape {
                    delay: self.delay.read(),
                    detune: self.detune.read(),
                    pan: self.pan.read(),
                    gain_db: self.gain.read(),
                }
            }
        }
    };
}

voice!(
    Voice1,
    "Voice 1",
    "V1 Delay",
    "V1 Detune",
    "V1 Pan",
    "V1 Gain",
    delay = 1.0,
    detune = [-1.0],
    pan = [-1.0]
);
voice!(
    Voice2,
    "Voice 2",
    "V2 Delay",
    "V2 Detune",
    "V2 Pan",
    "V2 Gain",
    delay = 0.72,
    detune = [1.0],
    pan = [1.0]
);
voice!(
    Voice3,
    "Voice 3",
    "V3 Delay",
    "V3 Detune",
    "V3 Pan",
    "V3 Gain",
    delay = 0.84,
    detune = [-0.40],
    pan = [-0.45]
);
voice!(
    Voice4,
    "Voice 4",
    "V4 Delay",
    "V4 Detune",
    "V4 Pan",
    "V4 Gain",
    delay = 0.44,
    detune = [0.40],
    pan = [0.45]
);
voice!(
    Voice5,
    "Voice 5",
    "V5 Delay",
    "V5 Detune",
    "V5 Pan",
    "V5 Gain",
    delay = 0.92,
    detune = [-0.70],
    pan = [-0.75]
);
voice!(
    Voice6,
    "Voice 6",
    "V6 Delay",
    "V6 Detune",
    "V6 Pan",
    "V6 Gain",
    delay = 0.30,
    detune = [0.70],
    pan = [0.75]
);
voice!(
    Voice7,
    "Voice 7",
    "V7 Delay",
    "V7 Detune",
    "V7 Pan",
    "V7 Gain",
    delay = 0.52,
    detune = [-0.25],
    pan = [-0.20]
);
voice!(
    Voice8,
    "Voice 8",
    "V8 Delay",
    "V8 Detune",
    "V8 Pan",
    "V8 Gain",
    delay = 0.62,
    detune = [0.25],
    pan = [0.20]
);

impl DoublerParams {
    /// The macro layer for this sample.
    pub fn macros(&self) -> Macros {
        Macros {
            voices: self.voices.value().into(),
            source: self.source.value().into(),
            detune: self.detune.read(),
            delay: self.delay.read(),
            spread: self.spread.read(),
            humanize: self.humanize.read(),
            tone_lo: self.tone_lo.read(),
            tone_hi: self.tone_hi.read(),
            tone_spread: self.tone_spread.read(),
        }
    }

    /// The shape layer for this sample.
    ///
    /// **Every voice is read, live or not.** A smoother only advances when it
    /// is polled, so skipping the inactive ones would leave them holding a
    /// stale value to jump from when `Voices` goes back up.
    pub fn shape(&self) -> [VoiceShape; MAX_VOICES] {
        [
            self.voice1.shape(),
            self.voice2.shape(),
            self.voice3.shape(),
            self.voice4.shape(),
            self.voice5.shape(),
            self.voice6.shape(),
            self.voice7.shape(),
            self.voice8.shape(),
        ]
    }

    /// Voice `index`'s parameter ids, as `[delay, detune, pan, gain]`. For the
    /// editor, which addresses the eight voices by number.
    pub fn voice_ids(&self, index: usize) -> [u32; 4] {
        macro_rules! ids {
            ($voice:expr) => {
                [
                    $voice.delay.id(),
                    $voice.detune.id(),
                    $voice.pan.id(),
                    $voice.gain.id(),
                ]
            };
        }
        match index {
            0 => ids!(self.voice1),
            1 => ids!(self.voice2),
            2 => ids!(self.voice3),
            3 => ids!(self.voice4),
            4 => ids!(self.voice5),
            5 => ids!(self.voice6),
            6 => ids!(self.voice7),
            _ => ids!(self.voice8),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doubler_core::DEFAULT_SHAPE;

    /// The literals in `voice!` are a copy of the core's table, because a
    /// truce default has to be a literal. This is what keeps the copy honest.
    #[test]
    fn voice_defaults_are_the_core_table() {
        let params = DoublerParams::new();
        let shape = [
            (
                &params.voice1.delay,
                &params.voice1.detune,
                &params.voice1.pan,
                &params.voice1.gain,
            ),
            (
                &params.voice2.delay,
                &params.voice2.detune,
                &params.voice2.pan,
                &params.voice2.gain,
            ),
            (
                &params.voice3.delay,
                &params.voice3.detune,
                &params.voice3.pan,
                &params.voice3.gain,
            ),
            (
                &params.voice4.delay,
                &params.voice4.detune,
                &params.voice4.pan,
                &params.voice4.gain,
            ),
            (
                &params.voice5.delay,
                &params.voice5.detune,
                &params.voice5.pan,
                &params.voice5.gain,
            ),
            (
                &params.voice6.delay,
                &params.voice6.detune,
                &params.voice6.pan,
                &params.voice6.gain,
            ),
            (
                &params.voice7.delay,
                &params.voice7.detune,
                &params.voice7.pan,
                &params.voice7.gain,
            ),
            (
                &params.voice8.delay,
                &params.voice8.detune,
                &params.voice8.pan,
                &params.voice8.gain,
            ),
        ];
        for (index, (delay, detune, pan, gain)) in shape.into_iter().enumerate() {
            let core = DEFAULT_SHAPE[index];
            assert_eq!(delay.value(), core.delay, "voice {} delay", index + 1);
            assert_eq!(detune.value(), core.detune, "voice {} detune", index + 1);
            assert_eq!(pan.value(), core.pan, "voice {} pan", index + 1);
            assert_eq!(gain.value(), core.gain_db, "voice {} gain", index + 1);
        }
    }

    /// The same for the macros, whose defaults live in `Macros::default()`.
    #[test]
    fn macro_defaults_are_the_core_defaults() {
        let params = DoublerParams::new();
        let core = Macros::default();
        assert_eq!(Voices::from(params.voices.value()), core.voices);
        assert_eq!(Source::from(params.source.value()), core.source);
        assert_eq!(params.detune.value(), core.detune);
        assert_eq!(params.delay.value(), core.delay);
        assert_eq!(params.spread.value(), core.spread);
        assert_eq!(params.humanize.value(), core.humanize);
        assert_eq!(params.tone_lo.value(), core.tone_lo);
        assert_eq!(params.tone_hi.value(), core.tone_hi);
        assert_eq!(params.tone_spread.value(), core.tone_spread);
    }

    /// **Every parameter reads back its own text** — a typed value in the
    /// window, and clap-validator's `param-conversions`.
    #[test]
    fn every_parameter_reads_back_its_own_text() {
        use truce::params::Params;
        let params = DoublerParams::new();
        for info in params.param_infos() {
            for plain in [info.range.min(), info.range.max()] {
                let text = params.format_value(info.id, plain).unwrap();
                let back = params.parse_value(info.id, &text);
                assert!(
                    back.is_some_and(|back| (back - plain).abs() < 0.01),
                    "{} printed {plain} as {text:?} and read back {back:?}",
                    info.name
                );
            }
        }
    }
}
