//! Velour's editor.
//!
//! Layout follows `plugins/velour/docs/specifications/ui.md`.
//!
//! **One fixed window size, and everything inside it.** The Doubler learned the
//! first half the expensive way: asking a host to resize the editor on a
//! disclosure toggle wedged it in Ableton
//! (`plugins/doubler/docs/implementation/doubler-plan.md`). Tabs were the answer
//! to the second half and turned out to be a worse one — twenty-four controls
//! is few enough to show at once, and "which band is that harshness in" cannot
//! be asked of half a panel.

mod advanced;
mod curve;
mod field;
mod meters;
mod readout;

use crate::analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use crate::params::VelourParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::heartbeat::Lifeline;
use nxe_ui::hint::Describe;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The window.
///
/// **The height is arithmetic, not a number found by looking.** Every part in
/// the column below has a known height — `nxe_ui::theme::LINE_*` exists so that
/// the text lines do too — so adding a row moves the window instead of running
/// off the bottom of it (`SPK-19`).
const WIDTH: u32 = theme::WINDOW_WIDTH;
const HEIGHT: u32 = (theme::SPACE_3 * 2.0
    + nxe_ui::header::HEIGHT
    + theme::SPACE_3
    + FIGURE_HEIGHT
    + theme::SPACE_3
    + knob_block_height(SHAPE_KNOB)
    + theme::SPACE_3
    + theme::RULE
    + theme::SPACE_3
    + advanced::HEIGHT
    + nxe_ui::status::HEIGHT) as u32;

/// The figure's row, including the inverted surface's own padding
/// (`nxe_ui::surface`).
const FIGURE_HEIGHT: f32 = field::HEIGHT + theme::SPACE_4 * 2.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<VelourParams>,
    /// Keeps the display's heartbeat running. Dropped with the window,
    /// which is what stops the thread (`nxe_ui::heartbeat`).
    ///
    /// **Never read on purpose**: what it does, it does by being dropped.
    #[allow(dead_code)]
    heartbeat: Lifeline,
    /// Which band the pointer is over, so the Advanced row and the region can
    /// mark each other. One value, both directions — the Doubler's
    /// `Ui::hovered` (`plugins/doubler/docs/specifications/ui.md`).
    hovered: Option<usize>,
    /// What the audio thread has published. Read on a heartbeat rather than
    /// mapped from the `Arc`: the handoff's identity never changes, so nothing
    /// would tell the binding system to look again.
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    ///
    /// The two curves the figure draws behind the regions: what came in, and the
    /// harmonics being added to it (`REQ-VEL-018`).
    dry: Curve,
    wet: Curve,
    /// Peak and held peak per meter, normalized onto the meter's own scale.
    peaks: Vec<f32>,
    holds: Vec<f32>,
    /// The three regions, and the readout strip's printed figures.
    ///
    /// **Both copied here rather than read inside a lens.** A lens that reads
    /// the handoff is re-evaluated once per *frame*, so the window redrew at the
    /// frame rate whenever the guards moved — see `nxe_ui::readout`.
    bands: Vec<nxe_ui::band::Band>,
    readouts: Vec<String>,
    /// What `bands` needs besides the parameters, kept because the heartbeat
    /// rebuilds them and the rate is read once when the window opens.
    host_rate: f32,
}

/// How often the display re-reads the analysis. 30 Hz is as fast as a meter
/// needs to look alive, and half the work of matching the frame rate.
const ANALYSIS_INTERVAL: Duration = Duration::from_millis(33);

/// The floor of the spectrum curves. Below this a band is drawn as silence —
/// without a floor the curve sits on the noise of an idle track.
const SPECTRUM_FLOOR_DB: f32 = -72.0;

/// The floor of the meters. Shallower than the spectrum's: a meter is read for
/// "how close to clipping", and 60 dB of travel puts a working vocal in the top
/// third where it can be read.
pub(crate) const METER_FLOOR_DB: f32 = -60.0;

/// One published band frame as a curve across the figure's axis.
///
/// **Both mappings live on this side of the widget**, which is the same split as
/// everywhere else: `nxe-ui` knows nothing about hertz or decibels, and
/// `nxe-dsp` knows nothing about the view it ends up in.
fn spectrum_curve(levels: &[f32; BANDS]) -> Curve {
    let span = (HIGH_HZ / LOW_HZ).log10();

    levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(index as f32 / (BANDS - 1) as f32);
            let x = (hz / LOW_HZ).log10() / span;
            let db = 20.0 * level.max(1e-9).log10();
            let y = ((db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB).clamp(0.0, 1.0);
            (x, y)
        })
        .collect()
}

/// An amplitude as a position on a meter.
fn meter_position(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.max(1e-9).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

#[derive(Clone)]
pub(crate) enum UiEvent {
    Hover(Option<usize>),
    /// The heartbeat asking the model to re-read what the audio thread
    /// published.
    Poll,
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::Hover(index) => self.hovered = *index,
            UiEvent::Poll => {
                self.dry = spectrum_curve(&self.analysis.dry.read());
                self.wet = spectrum_curve(&self.analysis.wet.read());
                let peaks = self.analysis.peaks.read();
                let holds = self.analysis.holds.read();
                self.peaks = peaks.iter().copied().map(meter_position).collect();
                self.holds = holds.iter().copied().map(meter_position).collect();
                self.bands = field::bands_of(&self.params, self.host_rate, &self.analysis);
                readout::poll(&self.analysis, &mut self.readouts);
            }
        });
    }
}

pub fn create(
    params: Arc<VelourParams>,
    state: Arc<ViziaState>,
    sample_rate: Arc<AtomicU32>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx, theme::Palette::VELOUR);

        // **Read once, when the window opens.** The rate decides where AIR's
        // upper edge is capped (`velour_core::bands::AIR_INPUT_CEILING`), and
        // that is the only thing on screen that depends on it. A host that
        // changes rate with the editor open leaves the figure an octave out at
        // the very top until it is reopened; polling for it every frame would be
        // work for a case that does not happen mid-session.
        let host_rate = f32::from_bits(sample_rate.load(Ordering::Relaxed));

        // **Started before the model, because the model holds what stops
        // it.** The lifeline dies with the window's context, and the
        // thread ends within one interval (`nxe_ui::heartbeat`).
        let heartbeat = nxe_ui::heartbeat::start(cx, ANALYSIS_INTERVAL, UiEvent::Poll);

        Ui {
            hovered: None,
            analysis: analysis.clone(),
            dry: Curve::new(),
            wet: Curve::new(),
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            bands: field::bands_of(&params, host_rate, &analysis),
            readouts: vec![String::new(); readout::FIGURES],
            host_rate,
            params: params.clone(),
            heartbeat,
        }
        .build(cx);

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    header(cx);

                    // The figure. It is what the plugin *is* (`ui.md`).
                    figure_row(cx);

                    // **No tabs.** They hid the per-band layer behind a click,
                    // and "which band is that harshness in" cannot be asked of
                    // half a panel. Everything is on screen.
                    shape_row(cx);
                    Element::new(cx).class("rule");
                    advanced::view(cx);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .row_between(Pixels(theme::SPACE_3));

                // **Outside the column**, because "is this louder or better" is
                // a question asked while looking at any of it (`ui.md`).
                meters::view(cx);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .col_between(Pixels(theme::SPACE_3))
            .child_space(Pixels(theme::SPACE_3));

            // **Flush to the bottom edge, and the full width of the window.**
            // A strip that stopped at the meters would read as one more panel
            // rather than as the window's floor (`nxe_ui::status`).
            readout::status(cx);
        })
        // **No child space here**: the padding belongs to the row above the
        // status bar, so the strip can reach the edges.
        //
        // **And no row between, which `.root` sets to `SPACE_4`.** That 16 px
        // comes out of the row, and the row is what the window's height was
        // computed for — the column's fixed parts then overflow while the
        // meter strip, which stretches, comes out short and stops above the
        // table beside it (`SPK-23`, measured off a screenshot).
        .class("root")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .child_space(Pixels(0.0))
        .row_between(Pixels(0.0));
    })
}

/// The figure, with the transfer curve beside it. The curve is a fixed column:
/// two stretching children would split the row in half and leave the figure
/// drawn across half the width it is meant to span.
fn figure_row(cx: &mut Context) {
    nxe_ui::surface::inverted(cx, |cx| {
        HStack::new(cx, |cx| {
            field::view(cx);
            curve::view(cx);
        })
        // **Not `.class("row")`.** That centres its children vertically, and
        // `child-top: 1s` / `child-bottom: 1s` are two more stretches for the
        // height to be divided among (`.agents/rules/vizia.md`). Both children
        // here are given an explicit height and want the whole of it.
        .height(Pixels(field::HEIGHT))
        .width(Stretch(1.0))
        .col_between(Pixels(theme::SPACE_3));
    })
    .height(Pixels(FIGURE_HEIGHT))
    .width(Stretch(1.0));
}

fn header(cx: &mut Context) {
    // The shipped name, in full — `NAME` in `lib.rs`, the bundle, and the
    // host's plugin list all say the same thing — with the one line that says
    // what the window is for, and the rule under both (`nxe_ui::header`).
    //
    // **No wrapping row.** `.class("row")` centres its children vertically, and
    // the header wants the whole of the height it asks for
    // (`.agents/rules/vizia.md`).
    nxe_ui::header::header(cx, "Velour", "vocal presence saturator", |cx| {
        nxe_plug_ui::segmented(cx, Ui::params, |params| &params.mode, &["Soft", "Hard"])
            .describe("How much of the layer is harmonics");
    });
}

/// The knob sizes. **The six that shape the sound are the large ones**; the
/// three beyond the break are smaller, because none of them is the shape —
/// `FOCUS` is set on the figure's rail and this is its number, `MIX` and
/// `OUTPUT` are how much arrives (`ui.md`).
const SHAPE_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

/// The eight controls that shape the sound, on one line.
///
/// `MIX` and `OUTPUT` are not part of the shape — one decides how much of it is
/// heard and the other how loud the result is — so they are smaller and sit
/// apart, past a stretch (`ui.md`).
fn shape_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        macro_knob(cx, "DRIVE", "How hard the curves are driven", |params| {
            &params.drive
        });
        macro_knob(cx, "BODY", "Weight, low harmonics", |params| &params.body);
        macro_knob(cx, "PRESENCE", "Forward, midrange harmonics", |params| {
            &params.presence
        });
        macro_knob(cx, "AIR", "Sheen, top harmonics", |params| &params.air);
        texture_knob(cx);
        macro_knob(
            cx,
            "DENSITY",
            "Levels the texture, not the voice",
            |params| &params.density,
        );

        // **A fixed gap, not a stretched one**, and `FOCUS` on the other side
        // of it. As `Stretch(1.0)` the gap was a whole knob's cell of nothing;
        // `FOCUS` stood alone beside the Advanced bars, leaving that column
        // short of the table next to it (`SPK-23`, the same two fixes).
        Element::new(cx)
            .width(Pixels(theme::SPACE_5))
            .height(Pixels(0.0));

        knob_block(
            cx,
            "FOCUS",
            "Slides every band edge",
            OUTPUT_KNOB,
            |params| &params.focus,
        );
        knob_block(
            cx,
            "MIX",
            "Dry against the added texture",
            OUTPUT_KNOB,
            |params| &params.mix,
        );
        knob_block(cx, "OUTPUT", "Level out", OUTPUT_KNOB, |params| {
            &params.output
        });
    })
    .class("row")
    .height(Auto)
    .width(Stretch(1.0));
}

/// One labelled knob with its value underneath: the shape every macro control
/// takes.
/// How tall a [`knob_block`] of a given knob size comes out.
///
/// **A window's height is the sum of its parts** (`theme::LINE_LABEL`), and
/// this is one of them.
pub(crate) const fn knob_block_height(size: f32) -> f32 {
    size + theme::SPACE_1 + theme::LINE_LABEL + theme::SPACE_1 + theme::LINE_VALUE
}

pub(crate) fn knob_block<P, F>(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    size: f32,
    to_param: F,
) where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<VelourParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The description goes on the knob rather than the whole block, so
        // pointing at the label or the number does not claim the strip.
        nxe_plug_ui::knob(cx, Ui::params, to_param, size).describe(hint);
        Label::new(cx, label)
            .class("label")
            .height(Pixels(theme::LINE_LABEL));
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

fn macro_knob<P, F>(cx: &mut Context, label: &'static str, hint: &'static str, to_param: F)
where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<VelourParams>) -> &P + Copy + 'static,
{
    knob_block(cx, label, hint, SHAPE_KNOB, to_param);
}

/// `TEXTURE`'s three anchors, and where they sit on the axis
/// (`velour_core::texture`).
const ANCHORS: [(&str, f32); 3] = [("WARM", 0.0), ("CLEAR", 0.5), ("EDGE", 1.0)];

/// `TEXTURE`, with the nearest anchor's name in front of the percentage.
///
/// **The name is what the discrete modes were traded for** (`REQ-VEL-004`). The
/// axis is continuous, so a number alone says nothing — 52% means nothing until
/// you know what is at either end — and the name alone would hide that there is
/// anything between the three.
///
/// **One line, not three labels.** The first attempt printed WARM / CLEAR / EDGE
/// side by side and lit the current one, and it was wrong twice over: `.accent`
/// is a *fill*, so the current one became a blue chip, and `.subtle` and
/// `.value` are different sizes, so it also changed size. Worse, three words are
/// wider than a knob column, and this row's columns are equal stretches — so the
/// overflow pushed into its neighbours. Every other knob here says label then
/// one value; this one now does too.
fn texture_knob(cx: &mut Context) {
    VStack::new(cx, |cx| {
        nxe_plug_ui::knob(cx, Ui::params, |params| &params.texture, SHAPE_KNOB)
            .describe("Warm through Clear to Edge");
        Label::new(cx, "TEXTURE").class("label");
        font::value(
            cx,
            Ui::params
                .map(|params| format!("{} {}", nearest(params.texture.value()), params.texture)),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

/// Which anchor a position is closest to. Ties cannot happen: the anchors are
/// evenly spaced, so the midpoints are the only ambiguous values and they fall
/// to the lower name.
fn nearest(position: f32) -> &'static str {
    ANCHORS
        .iter()
        .min_by(|(_, a), (_, b)| {
            (a - position)
                .abs()
                .partial_cmp(&(b - position).abs())
                .expect("the anchors and the position are finite")
        })
        .map(|(name, _)| *name)
        .unwrap_or("CLEAR")
}

#[cfg(test)]
mod tests {

    /// **Every parameter has somewhere to be touched.**
    ///
    /// A parameter with no control is one a user can only reach through the
    /// host's generic view, and **nothing else notices** — it compiles, it
    /// saves, it automates, and the window simply never mentions it.
    ///
    /// This crate did not have this test, and lost two controls to a header
    /// rewrite without a thing going red (`SPK-19`). Sparkleur had it and did
    /// not lose any.
    #[test]
    fn every_parameter_has_a_control() {
        const PARAMS: &str = include_str!("../params.rs");
        const COUNT: usize = 23;
        const SOURCES: [&str; 6] = [
            include_str!("mod.rs"),
            include_str!("advanced.rs"),
            include_str!("curve.rs"),
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
    use super::*;

    #[test]
    fn the_nearest_anchor_is_the_nearest_one() {
        assert_eq!(nearest(0.0), "WARM");
        assert_eq!(nearest(0.2), "WARM");
        assert_eq!(nearest(0.5), "CLEAR");
        assert_eq!(nearest(0.8), "EDGE");
        assert_eq!(nearest(1.0), "EDGE");
        // The midpoints fall to the lower name rather than flickering between
        // two of them while the knob sits still.
        assert_eq!(nearest(0.25), "WARM");
        assert_eq!(nearest(0.75), "CLEAR");
    }
}
