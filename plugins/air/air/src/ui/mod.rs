//! The NXE Air editor.
//!
//! **One screen, no tabs** (`REQ-AIR-013`). Fifteen parameters fit, and the
//! question this plugin is used to answer — is the layer where I want it, and
//! is anything holding it back — cannot be asked of half a panel (`SPK-19`).

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use crate::params::AirParams;

mod advanced;
mod field;
mod meters;
mod readout;

/// The window.
///
/// **The height is arithmetic, not a number found by looking.** Every part in
/// the column below has a known height — `nxe_ui::theme::LINE_*` exists so that
/// the text lines do too — so adding a row moves the window instead of running
/// off the bottom of it (`.agents/rules/ui.md`).
const WIDTH: u32 = 720;
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
    + advanced::HEIGHT) as u32;

/// **All seven the same size.** Sparkleur separates "what shapes the sound"
/// from "how much of it arrives", but every one of Air's seven decides how the
/// layer is made — `MIX` included, because it scales what was added rather than
/// turning the original down (`REQ-AIR-012`).
const MAIN_KNOB: f32 = 52.0;
pub(crate) const OUTPUT_KNOB: f32 = 38.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<AirParams>,
    /// What the audio thread has published. Read on a heartbeat rather than
    /// mapped from the `Arc`: the handoff's identity never changes, so nothing
    /// would tell the binding system to look again.
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    dry: Curve,
    layer: Curve,
    peaks: Vec<f32>,
    holds: Vec<f32>,
}

pub(crate) enum UiEvent {
    /// The heartbeat asking the model to re-read what the audio thread
    /// published.
    Poll,
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::Poll => {
                self.dry = spectrum_curve(&self.analysis.dry.read());
                self.layer = spectrum_curve(&self.analysis.layer.read());
                let peaks = self.analysis.peaks.read();
                let holds = self.analysis.holds.read();
                self.peaks = peaks.iter().copied().map(meter_position).collect();
                self.holds = holds.iter().copied().map(meter_position).collect();
                // The readout strip is **not** copied here. It reads inside its
                // own lenses, and any change to this model — this heartbeat
                // included — re-evaluates them.
            }
        });
    }
}

/// How often the display re-reads the analysis. 30 Hz is as fast as a figure
/// needs to look alive, and half the work of matching the frame rate.
const ANALYSIS_INTERVAL: Duration = Duration::from_millis(33);

/// Why this is a thread and not `cx.add_timer`.
///
/// **vizia's timers never fire here.** `process_timers` is called by
/// `vizia_winit` and by nothing else; the baseview backend — the one every
/// plugin and the gallery run on — does not call it (`.agents/rules/vizia.md`).
/// `cx.spawn` hands out a `ContextProxy`, which baseview *does* support, and
/// emitting through it fails once the window is gone, which is the thread's
/// signal to stop.
fn start_heartbeat(cx: &mut Context) {
    cx.spawn(|proxy| {
        while proxy.emit(UiEvent::Poll).is_ok() {
            std::thread::sleep(ANALYSIS_INTERVAL);
        }
    });
}

/// The floor of the spectra. Below this a band is drawn as silence — without a
/// floor the curve sits on the noise of an idle track, and the grain field
/// never empties (`nxe_ui::dots`).
const SPECTRUM_FLOOR_DB: f32 = -72.0;

/// The floor of the meters. Shallower than the spectra's: a meter is read for
/// "how close to clipping", and 60 dB of travel puts a working mix in the top
/// third where it can be read.
pub(crate) const METER_FLOOR_DB: f32 = -60.0;

/// An amplitude as a position on a meter.
pub(crate) fn meter_position(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.max(1e-9).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

/// One published band frame as a curve across the figure's axis.
///
/// **Both mappings live on this side of the widget**: `nxe-ui` knows nothing
/// about hertz or decibels, and `nxe-dsp` knows nothing about the view it ends
/// up in.
fn spectrum_curve(levels: &[f32; BANDS]) -> Curve {
    levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(index as f32 / (BANDS - 1) as f32);
            let db = 20.0 * level.max(1e-9).log10();
            let y = ((db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB).clamp(0.0, 1.0);
            (field::axis_x(hz), y)
        })
        .collect()
}

pub fn create(
    params: Arc<AirParams>,
    state: Arc<ViziaState>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            params: params.clone(),
            analysis: analysis.clone(),
            dry: Curve::new(),
            layer: Curve::new(),
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
        }
        .build(cx);

        // Parameter changes wake the binding system on their own; an idle
        // window with audio running does not. The display needs its own
        // heartbeat.
        start_heartbeat(cx);

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                nxe_ui::header::header(cx, "NXE AIR", "signal-driven texture");
                readout::view(cx, analysis.clone());
                // The figure. It is what the plugin *is* (`ui.md`).
                field::view(cx);
                main_row(cx);
                Element::new(cx).class("rule");
                advanced::view(cx);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(theme::SPACE_3));

            // **Outside everything else**, because "is this louder or better"
            // is a question asked while looking at any of it
            // (`.agents/rules/ui.md`).
            meters::view(cx);
        })
        // **`.root` is what paints the window.** Without it the ground is the
        // host's black while every `.panel` sits at `BACKGROUND`, so the panels
        // read as lighter boxes — the theme's "two levels, not three" needs the
        // window to be one of them. Sparkleur shipped one build without it
        // (`.agents/rules/vizia.md`).
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
        knob(cx, "SURFACE", "How much surface to make", |params| {
            &params.surface
        });
        knob(cx, "BLEND", "Harmonic against noise", |params| {
            &params.blend
        });
        knob(
            cx,
            "CHARACTER",
            "Knee on one side, grain on the other",
            |params| &params.character,
        );
        knob(cx, "FOCUS", "Where the layer sits", |params| &params.focus);
        knob(cx, "WIDTH", "How far the noise half spreads", |params| {
            &params.width
        });
        knob(
            cx,
            "FOLLOW",
            "How much the layer answers to the input",
            |params| &params.follow,
        );
        knob(cx, "MIX", "How much of the layer is added", |params| {
            &params.mix
        });
    })
    .class("row")
    .height(Auto)
    .width(Stretch(1.0));
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
    F: Fn(&Arc<AirParams>) -> &P + Copy + 'static,
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
    F: Fn(&Arc<AirParams>) -> &P + Copy + 'static,
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
        const COUNT: usize = 15;
        const SOURCES: [&str; 4] = [
            include_str!("mod.rs"),
            include_str!("advanced.rs"),
            include_str!("field.rs"),
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
}
