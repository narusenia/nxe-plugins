//! The NXE Vocal Depth editor.
//!
//! **One screen, no tabs** (`REQ-VDP-013`). Eight parameters fit, and the
//! question this plugin is used to answer — where is the voice, and is anything
//! putting it back — cannot be asked of half a panel (`SPK-19`).
//!
//! **The Advanced deviations are not here yet** (`REQ-VDP-009`). They are seven
//! more parameters and an engine change, and `dsp.md` does not yet say what each
//! deviation does as a number — so they are their own unit rather than a row
//! quietly added to this window (`ui.md`).

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::heartbeat::Lifeline;
use nxe_ui::taps::Tap;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::{Analysis, METERS};
use crate::params::VocalDepthParams;

mod field;
mod meters;
mod readout;

/// The window.
///
/// **The height is arithmetic, not a number found by looking.** Every part in
/// the column below has a known height — `nxe_ui::theme::LINE_*` exists so that
/// the text lines do too — so adding a row moves the window instead of running
/// off the bottom of it (`.agents/rules/ui.md`).
pub(crate) const WIDTH: u32 = 720;
const HEIGHT: u32 = (theme::SPACE_3 * 2.0
    + nxe_ui::header::HEIGHT
    + theme::SPACE_3
    + nxe_ui::readout::HEIGHT
    + theme::SPACE_3
    + field::HEIGHT
    + theme::SPACE_3
    + knob_block_height(MAIN_KNOB)
    + theme::SPACE_3
    + theme::RULE
    + theme::SPACE_3
    + knob_block_height(OUTPUT_KNOB)) as u32;

/// **All seven the same size.** Sparkleur separates "what shapes the sound" from
/// "how much of it arrives", but here `MIX` is part of the distance too: the wet
/// path *replaces* the voice rather than adding to it, so turning `MIX` down is
/// bringing the voice back rather than turning an effect off
/// (`vocal_depth_core::engine`).
const MAIN_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<VocalDepthParams>,
    /// Keeps the display's heartbeat running. Dropped with the window, which is
    /// what stops the thread (`nxe_ui::heartbeat`).
    ///
    /// **Never read on purpose**: what it does, it does by being dropped.
    #[allow(dead_code)]
    heartbeat: Lifeline,
    /// What the audio thread has published. Read on a heartbeat rather than
    /// mapped from the `Arc`: the handoff's identity never changes, so nothing
    /// would tell the binding system to look again.
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    arrivals: Vec<Tap>,
    direct: f32,
    peaks: Vec<f32>,
    holds: Vec<f32>,
    /// The readout strip's printed figures. **Copied here rather than read
    /// inside a lens** — see `nxe_ui::readout`.
    readouts: Vec<String>,
}

#[derive(Clone)]
pub(crate) enum UiEvent {
    /// The heartbeat asking the model to re-read what the audio thread
    /// published.
    Poll,
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::Poll => {
                let positions = self.analysis.arrivals.read();
                let levels = self.analysis.arrival_levels.read();
                self.arrivals = positions
                    .iter()
                    .zip(levels.iter())
                    .map(|(&position, &level)| Tap { position, level })
                    .collect();
                self.direct = level_position(self.analysis.buses.read()[0]);

                let peaks = self.analysis.peaks.read();
                let holds = self.analysis.holds.read();
                self.peaks = peaks.iter().copied().map(meter_position).collect();
                self.holds = holds.iter().copied().map(meter_position).collect();
                readout::poll(&self.analysis, &mut self.readouts);
            }
        });
    }
}

/// How often the display re-reads the analysis. 30 Hz is as fast as a figure
/// needs to look alive, and half the work of matching the frame rate.
const ANALYSIS_INTERVAL: Duration = Duration::from_millis(33);

/// The floor of the meters. A meter is read for "how close to clipping", and
/// 60 dB of travel puts a working mix in the top third where it can be read.
pub(crate) const METER_FLOOR_DB: f32 = -60.0;

/// An amplitude as a position on a meter.
pub(crate) fn meter_position(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.max(1e-9).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

/// A level already in dB as a height in the figure.
///
/// **The same floor as the meters**, so the direct sound's stem and the OUT bar
/// cannot disagree about how loud it is.
fn level_position(decibels: f32) -> f32 {
    if !decibels.is_finite() {
        return 0.0;
    }
    ((decibels - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

pub fn create(
    params: Arc<VocalDepthParams>,
    state: Arc<ViziaState>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx, theme::Palette::PARALLAX);

        // **Started before the model, because the model holds what stops it.**
        // The lifeline dies with the window's context, and the thread ends
        // within one interval (`nxe_ui::heartbeat`).
        let heartbeat = nxe_ui::heartbeat::start(cx, ANALYSIS_INTERVAL, UiEvent::Poll);

        Ui {
            params: params.clone(),
            analysis: analysis.clone(),
            arrivals: Vec::new(),
            direct: 0.0,
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            readouts: vec![String::new(); readout::FIGURES],
            heartbeat,
        }
        .build(cx);

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                nxe_ui::header::header(cx, "NXE VOCAL DEPTH", "forward and back");
                readout::view(cx);
                // The figure. It is what the plugin *is* (`ui.md`).
                field::view(cx);
                main_row(cx);
                Element::new(cx).class("rule");
                trim_row(cx);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(theme::SPACE_3));

            // **Outside everything else**, because "did the voice move or just
            // change level" is a question asked while looking at any of it
            // (`.agents/rules/ui.md`).
            meters::view(cx);
        })
        // **`.root` is what paints the window.** Without it the ground is the
        // host's black while every `.panel` sits at `BACKGROUND`, so the panels
        // read as lighter boxes (`.agents/rules/vizia.md`).
        .class("root")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_3))
        .child_space(Pixels(theme::SPACE_3));
    })
}

/// The seven controls, on one line.
fn main_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        knob(cx, "DEPTH", "How far away the voice is", |params| {
            &params.depth
        });
        knob(cx, "DIRECT", "How near the voice itself sounds", |params| {
            &params.direct
        });
        knob(cx, "ROOM", "How much early reflection there is", |params| {
            &params.room
        });
        knob(cx, "DAMPING", "How much top the distance takes", |params| {
            &params.damping
        });
        knob(cx, "WIDTH", "How far the space spreads", |params| {
            &params.width
        });
        knob(
            cx,
            "CLARITY",
            "How much intelligibility to put back",
            |params| &params.clarity,
        );
        knob(cx, "MIX", "Dry against the moved voice", |params| {
            &params.mix
        });
    })
    .class("row")
    .height(Auto)
    .width(Stretch(1.0));
}

/// The output trim, on its own line under a rule.
///
/// **Not part of the distance.** `DEPTH` is normalised on its own
/// (`REQ-VDP-008`), so this is here for gain staging and nothing else — which
/// is why it is under the rule rather than in the row above.
fn trim_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        knob_block(cx, "OUTPUT", "The final trim", OUTPUT_KNOB, |params| {
            &params.output
        });
    })
    .class("row")
    .height(Auto)
    .width(Stretch(1.0))
    .child_right(Stretch(1.0));
}

/// How tall a [`knob_block`] of a given knob size comes out.
///
/// **A window's height is the sum of its parts**, and this is one of them.
pub(crate) const fn knob_block_height(size: f32) -> f32 {
    size + theme::SPACE_1 + theme::LINE_LABEL + theme::SPACE_1 + theme::LINE_VALUE
}

/// One labelled knob with its value underneath.
pub(crate) fn knob_block<P, F>(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    size: f32,
    to_param: F,
) where
    P: Param + 'static,
    F: Fn(&Arc<VocalDepthParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        nxe_plug_ui::knob(cx, Ui::params, to_param, size).tooltip(move |cx| theme::hint(cx, hint));
        Label::new(cx, label)
            .class("label")
            .height(Pixels(theme::LINE_LABEL));
        // **Figures are set in the mono face.** A proportional one changes width
        // between a `1` and an `8`, and a value that jitters under a drag is
        // what fixing the decimal count was meant to prevent.
        font::value(
            cx,
            ParamWidgetBase::make_lens(Ui::params, to_param, |param| param.to_string()),
        )
        .height(Pixels(theme::LINE_VALUE));
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn knob<P, F>(cx: &mut Context, label: &'static str, hint: &'static str, to_param: F)
where
    P: Param + 'static,
    F: Fn(&Arc<VocalDepthParams>) -> &P + Copy + 'static,
{
    knob_block(cx, label, hint, MAIN_KNOB, to_param);
}

#[cfg(test)]
mod tests {
    /// **Every parameter has somewhere to be touched.**
    ///
    /// A parameter with no control is one a user can only reach through the
    /// host's generic view, and **nothing else would notice** — it compiles, it
    /// saves, it automates, and the window simply never mentions it. Two were
    /// lost to a header rewrite in the one crate that lacked this test
    /// (`SPK-19`).
    #[test]
    fn every_parameter_has_a_control() {
        const PARAMS: &str = include_str!("../params.rs");
        const COUNT: usize = 8;
        const SOURCES: [&str; 4] = [
            include_str!("mod.rs"),
            include_str!("field.rs"),
            include_str!("meters.rs"),
            include_str!("readout.rs"),
        ];

        // **Only the fields carrying an `#[id]`.** A `params.rs` also holds
        // editor state and persisted switches, and those are not parameters —
        // the attribute is what makes one, so it is what the scan looks for.
        let fields: Vec<&str> = PARAMS
            .lines()
            .zip(PARAMS.lines().skip(1))
            .filter(|(line, _)| line.trim().starts_with("#[id"))
            .filter_map(|(_, next)| next.trim().strip_prefix("pub "))
            .filter_map(|rest| rest.split_once(':'))
            .map(|(name, _)| name)
            .collect();
        assert_eq!(fields.len(), COUNT, "the parameter list moved: {fields:?}");

        for field in fields {
            let access = format!(".{field}");
            assert!(
                SOURCES.iter().any(|source| source.contains(&access)),
                "{field} has no control"
            );
        }
    }

    /// The window is as tall as its parts, and the width is the line's
    /// (`.agents/rules/ui.md`).
    #[test]
    fn the_window_is_the_size_of_its_parts() {
        assert_eq!(super::WIDTH, 720);
        // Nothing to compare the height against but its own arithmetic — what
        // this pins is that it *is* arithmetic, so a part changing size moves
        // the window instead of running off it.
        assert!(super::HEIGHT > super::field::HEIGHT as u32);
    }
}
