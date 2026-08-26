//! Velour's editor.
//!
//! Layout follows `plugins/velour/docs/specifications/ui.md`.
//!
//! **One fixed window size, and tabs inside it.** The Doubler learned this the
//! expensive way: asking a host to resize the editor on a disclosure toggle
//! wedged it in Ableton (`plugins/doubler/docs/implementation/doubler-plan.md`).
//! Tabs need nothing from the host for a control to become reachable.

mod advanced;
mod curve;
mod field;
mod meters;
mod param_bind;

use crate::analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use crate::params::VelourParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// `ui.md` started from the Doubler's 620 × 572 plus the meter strip, and this
/// is what looking at it in a host settled it to.
///
/// **The height is the sum of the parts, not a round number**: 12 of padding,
/// the wordmark, the figure, the tab strip, [`TAB_HEIGHT`], and 12 again. Set
/// too tall — it started at 580 — the window ends in a band of nothing, because
/// everything inside is a fixed height and piles at the top.
const WIDTH: u32 = 680;
const HEIGHT: u32 = 528;

/// How tall the swapped region is. Fixed, so switching tabs does not move
/// anything above it.
///
/// Sized to MAIN, which is the taller of the two: a 52 px knob with its label
/// and value is 88, the second row of smaller knobs is 74, and 12 between them.
/// ADVANCED needs about 130 and simply leaves the rest empty.
const TAB_HEIGHT: f32 = 180.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

const TAB_MAIN: usize = 0;
const TAB_ADVANCED: usize = 1;

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<VelourParams>,
    /// Which tab is showing.
    ///
    /// **Interface state, not a parameter.** Which tab was open does not change
    /// the sound, so it is not worth an id in the saved state — reopening on
    /// MAIN is the right default anyway.
    tab: usize,
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
}

/// How often the display re-reads the analysis. 30 Hz is as fast as a meter
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

/// The floor of the spectrum curves. Below this a band is drawn as silence —
/// without a floor the curve sits on the noise of an idle track.
const SPECTRUM_FLOOR_DB: f32 = -72.0;

/// The floor of the meters. Shallower than the spectrum's: a meter is read for
/// "how close to clipping", and 60 dB of travel puts a working vocal in the top
/// third where it can be read.
const METER_FLOOR_DB: f32 = -60.0;

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

pub(crate) enum UiEvent {
    SelectTab(usize),
    Hover(Option<usize>),
    /// The heartbeat asking the model to re-read what the audio thread
    /// published.
    Poll,
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::SelectTab(tab) => self.tab = *tab,
            UiEvent::Hover(index) => self.hovered = *index,
            UiEvent::Poll => {
                self.dry = spectrum_curve(&self.analysis.dry.read());
                self.wet = spectrum_curve(&self.analysis.wet.read());
                let peaks = self.analysis.peaks.read();
                let holds = self.analysis.holds.read();
                self.peaks = peaks.iter().copied().map(meter_position).collect();
                self.holds = holds.iter().copied().map(meter_position).collect();
                // The guards are **not** copied here. The figure reads them
                // inside its own lens, because a region carries its reduction
                // in the same value as its level and a lens can only map one
                // field (`.agents/rules/vizia.md`). Any change to this model —
                // this heartbeat included — re-evaluates that lens, which is
                // what makes the regions sink in time.
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
        theme::install(cx);

        // **Read once, when the window opens.** The rate decides where AIR's
        // upper edge is capped (`velour_core::bands::AIR_INPUT_CEILING`), and
        // that is the only thing on screen that depends on it. A host that
        // changes rate with the editor open leaves the figure an octave out at
        // the very top until it is reopened; polling for it every frame would be
        // work for a case that does not happen mid-session.
        let host_rate = f32::from_bits(sample_rate.load(Ordering::Relaxed));

        Ui {
            tab: TAB_MAIN,
            hovered: None,
            analysis: analysis.clone(),
            dry: Curve::new(),
            wet: Curve::new(),
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            params: params.clone(),
        }
        .build(cx);

        // Parameter changes wake the binding system on their own; an idle window
        // with audio running does not. The display needs its own heartbeat.
        start_heartbeat(cx);

        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                header(cx);
                // The figure stays put above the tabs. It is what the plugin
                // *is* — hiding it behind a tab would leave the window with
                // nothing to look at (`ui.md`).
                figure_row(cx, host_rate, analysis.clone());
                tab_strip(cx);

                // Both tabs are built and one is hidden. Rebuilding on a switch
                // would drop the widgets' own state — a drag in progress, a
                // hover — for nothing.
                VStack::new(cx, |cx| {
                    main_tab(cx);
                    advanced_tab(cx);
                })
                .height(Pixels(TAB_HEIGHT))
                .width(Stretch(1.0));
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(theme::SPACE_3));

            // **Outside the tabs**, because "is this louder or better" is a
            // question asked while looking at either of them (`ui.md`).
            meters::view(cx);
        })
        .class("root")
        .child_space(Pixels(theme::SPACE_3))
        .col_between(Pixels(theme::SPACE_3));
    })
}

/// The figure, with the transfer curve beside it. The curve is a fixed column:
/// two stretching children would split the row in half and leave the figure
/// drawn across half the width it is meant to span.
fn figure_row(cx: &mut Context, host_rate: f32, analysis: Arc<Analysis>) {
    HStack::new(cx, |cx| {
        field::view(cx, host_rate, analysis);
        curve::view(cx, CURVE_WIDTH);
    })
    .height(Pixels(field::HEIGHT))
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_3));
}

/// The transfer-curve window's width. Square-ish: it is a curve read for its
/// shape, and a wide one flattens the very thing being read.
const CURVE_WIDTH: f32 = 132.0;

fn header(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // The shipped name, in full — `NAME` in `lib.rs`, the bundle, and the
        // host's plugin list all say the same thing.
        font::title(cx, "NXE VELOUR");
    })
    .class("row")
    .height(Auto);
}

/// The tab strip is a segmented control: the same "one of these" choice as
/// everywhere else in this design, so it is the same widget.
fn tab_strip(cx: &mut Context) {
    HStack::new(cx, |cx| {
        SegmentedControl::new(cx, Ui::tab, &["MAIN", "ADVANCED"], |cx, tab| {
            cx.emit(UiEvent::SelectTab(tab));
        });
    })
    .class("row")
    .height(Auto);
}

/// The knob sizes. The six that shape the sound are the large ones; the two
/// that decide how much of it arrives are smaller and sit apart, because they
/// are a different question (`ui.md`).
const SHAPE_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
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
        })
        .class("row")
        .height(Auto);

        // `MIX` and `OUTPUT` are not part of the shape: one decides how much of
        // it is heard and the other how loud the result is. Centred and smaller,
        // so the row above reads as the instrument and this one as the tap.
        HStack::new(cx, |cx| {
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
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
            Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        })
        .class("row")
        .height(Auto);
    })
    .height(Auto)
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_3))
    .display(Ui::tab.map(|tab| *tab == TAB_MAIN));
}

fn advanced_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        advanced::view(cx);
    })
    .height(Auto)
    .width(Stretch(1.0))
    .display(Ui::tab.map(|tab| *tab == TAB_ADVANCED));
}

/// One labelled knob with its value underneath: the shape every macro control
/// takes.
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
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        param_bind::knob(cx, Ui::params, to_param, size).tooltip(move |cx| theme::hint(cx, hint));
        Label::new(cx, label).class("label");
        font::value(
            cx,
            ParamWidgetBase::make_lens(Ui::params, to_param, |param| param.to_string()),
        );
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
        param_bind::knob(cx, Ui::params, |params| &params.texture, SHAPE_KNOB)
            .tooltip(|cx| theme::hint(cx, "Warm through Clear to Edge"));
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
