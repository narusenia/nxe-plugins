//! The Doubler's editor.
//!
//! Layout follows `plugins/doubler/docs/specifications/ui.md`.
//!
//! **The window never resizes.** Asking the host to resize on a disclosure
//! toggle left the editor wedged in Ableton — the same layout works when the
//! window is opened at either size, and the gallery, which has no host and no
//! resize, is fine. So the sections are tabs inside one fixed window instead.
//! That also removes a whole class of question: no host has to agree to
//! anything for a control to become reachable.

mod detail;
mod field;
mod param_bind;
mod tone;

use crate::analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, PAN_BINS};
use crate::params::DoublerParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, icon, theme};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// One size, tall enough for whichever tab needs the most room. Measured
/// against the built layout rather than estimated.
const WIDTH: u32 = 620;
const HEIGHT: u32 = 572;

/// How tall the swapped region is. Fixed, so switching tabs does not move
/// anything above it.
///
/// Sized to the taller tab with a little slack: `MAIN` needs about 212 px
/// (macros plus the Filter View) and `DETAIL` about 203 px. What is left over
/// goes to the figure.
const TAB_HEIGHT: f32 = 230.0;

/// The Voice Field's height, and the width of the column of global controls
/// beside it.
const FIELD_HEIGHT: f32 = 236.0;
const SIDE_WIDTH: f32 = 96.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

const TAB_MAIN: usize = 0;
const TAB_DETAIL: usize = 1;

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<DoublerParams>,
    /// Which tab is showing. The reactive copy of `params.detail_tab`: an
    /// `AtomicBool` cannot be observed by a lens, so the display binds to this
    /// and the atomic follows it for persistence.
    tab: usize,
    /// Which voice the pointer is over in the Voice Field, so the matching row
    /// can be highlighted. This is what stands in for numbering the dots
    /// (`plugins/doubler/docs/specifications/ui.md`).
    hovered: Option<usize>,
    /// Whether an edit mirrors onto the paired voice, per axis
    /// (`REQ-DBL-014`). The reactive copies of `params.mirror_*`, for the same
    /// reason `tab` is one.
    mirror_pan: bool,
    mirror_detune: bool,
    mirror_delay: bool,
    mirror_gain: bool,
    /// What the audio thread has published. Read on a timer rather than mapped
    /// from the `Arc`: the handoff's identity never changes, so nothing would
    /// tell the binding system to look again.
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    density: Vec<f32>,
    spectrum: Vec<(f32, f32)>,
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

/// The floor of the spectrum display. Below this a band is drawn as silence —
/// without a floor the curve sits on the noise of an idle track.
const SPECTRUM_FLOOR_DB: f32 = -72.0;

/// The floor of the stereo shading. Shallower than the spectrum's: this is one
/// flat colour rather than a curve, so a wide range would leave everything the
/// same shade of grey.
const DENSITY_FLOOR_DB: f32 = -48.0;

/// Which axis a mirror switch controls — one per shape axis.
///
/// `Gain` is here because the figure's radius *is* the gain: without it,
/// dragging a point would visibly lean the image, which is the thing mirroring
/// exists to prevent. Leaning it on purpose is what turning this one off is for
/// (`REQ-DBL-014`).
#[derive(Clone, Copy)]
pub(crate) enum MirrorAxis {
    Pan,
    Detune,
    Delay,
    Gain,
}

pub(crate) enum UiEvent {
    SelectTab(usize),
    Hover(Option<usize>),
    ToggleMirror(MirrorAxis),
    /// The timer asking the model to re-read what the audio thread published.
    Poll,
}

impl Model for Ui {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::SelectTab(tab) => {
                self.tab = *tab;
                self.params
                    .detail_tab
                    .store(*tab == TAB_DETAIL, Ordering::Relaxed);
            }
            UiEvent::Hover(index) => self.hovered = *index,
            UiEvent::Poll => {
                self.density = density_curve(&self.analysis.pan.read());
                self.spectrum = spectrum_curve(&self.analysis.spectrum.read());
            }
            UiEvent::ToggleMirror(axis) => match axis {
                MirrorAxis::Pan => {
                    self.mirror_pan = !self.mirror_pan;
                    self.params
                        .mirror_pan
                        .store(self.mirror_pan, Ordering::Relaxed);
                }
                MirrorAxis::Detune => {
                    self.mirror_detune = !self.mirror_detune;
                    self.params
                        .mirror_detune
                        .store(self.mirror_detune, Ordering::Relaxed);
                }
                MirrorAxis::Delay => {
                    self.mirror_delay = !self.mirror_delay;
                    self.params
                        .mirror_delay
                        .store(self.mirror_delay, Ordering::Relaxed);
                }
                MirrorAxis::Gain => {
                    self.mirror_gain = !self.mirror_gain;
                    self.params
                        .mirror_gain
                        .store(self.mirror_gain, Ordering::Relaxed);
                }
            },
        });
    }
}

/// The published pan energies as something to shade with: `0` nothing, `1` as
/// dark as the field goes.
///
/// **On a dB scale**, because a linear one shows almost nothing until a signal
/// is loud — and because it is what makes the picture fade out as the sound
/// does rather than only when it stops.
fn density_curve(levels: &[f32; PAN_BINS]) -> Vec<f32> {
    levels
        .iter()
        .map(|energy| {
            let db = 10.0 * energy.max(1e-9).log10();
            ((db - DENSITY_FLOOR_DB) / -DENSITY_FLOOR_DB).clamp(0.0, 1.0)
        })
        .collect()
}

/// The published band levels as a curve for the Filter View: `x` across the log
/// frequency axis, `y` on the same ±12 dB scale the Tone curve uses.
///
/// **Both live on the caller's side of the widget**, which is the same split as
/// everywhere else — `nxe-ui` knows nothing about hertz or decibels, and
/// `nxe-dsp` knows nothing about the view it ends up in.
fn spectrum_curve(levels: &[f32; BANDS]) -> Vec<(f32, f32)> {
    let span = (HIGH_HZ / LOW_HZ).log10();

    levels
        .iter()
        .enumerate()
        .map(|(index, level)| {
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(index as f32 / (BANDS - 1) as f32);
            let x = (hz / LOW_HZ).log10() / span;
            // `20 log10` of an amplitude, floored so silence is the bottom of
            // the view rather than minus infinity.
            let db = 20.0 * level.max(1e-9).log10();
            let y = ((db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB).clamp(0.0, 1.0);
            (x, y)
        })
        .collect()
}

pub fn create(
    params: Arc<DoublerParams>,
    state: Arc<ViziaState>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            tab: if params.detail_tab.load(Ordering::Relaxed) {
                TAB_DETAIL
            } else {
                TAB_MAIN
            },
            hovered: None,
            mirror_pan: params.mirror_pan.load(Ordering::Relaxed),
            mirror_detune: params.mirror_detune.load(Ordering::Relaxed),
            mirror_delay: params.mirror_delay.load(Ordering::Relaxed),
            mirror_gain: params.mirror_gain.load(Ordering::Relaxed),
            analysis: analysis.clone(),
            density: vec![0.0; PAN_BINS],
            spectrum: Vec::new(),
            params: params.clone(),
        }
        .build(cx);

        // Parameter changes wake the binding system on their own; an idle
        // window with audio running does not. The meter needs its own heartbeat.
        start_heartbeat(cx);

        VStack::new(cx, |cx| {
            header(cx);
            // The figure stays put. It is what the plugin *is* — hiding it
            // behind a tab would leave the window with nothing to look at.
            field_row(cx);
            tab_strip(cx);

            // Both tabs are built and one is hidden. Rebuilding on every switch
            // would drop the widgets' own state — a drag in progress, a hover —
            // for nothing.
            VStack::new(cx, |cx| {
                main_tab(cx);
                detail::view(cx);
            })
            .height(Pixels(TAB_HEIGHT))
            .width(Stretch(1.0));
        })
        .class("root")
        .child_space(Pixels(theme::SPACE_3))
        .row_between(Pixels(theme::SPACE_3));
    })
}

fn header(cx: &mut Context) {
    HStack::new(cx, |cx| {
        // The shipped name, in full — `NAME` in `lib.rs`, the bundle, and the
        // host's plugin list all say the same thing.
        font::title(cx, "NXE DOUBLER");
        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        param_bind::segmented(cx, Ui::params, |params| &params.voices, &["2", "4", "8"])
            .tooltip(|cx| theme::hint(cx, "How many voices run"));
        param_bind::segmented(
            cx,
            Ui::params,
            |params| &params.source,
            &["Mono Sum", "True Stereo"],
        )
        .class("hint-left")
        .tooltip(|cx| {
            theme::hint(
                cx,
                "Mono Sum: every voice doubles L+R. True Stereo: each doubles one side",
            )
        });
    })
    .class("row")
    .height(Auto);
}

/// The figure, with the two controls that apply to everything stacked beside
/// it. `Mix` and `Output` belong next to what they act on, not under a tab.
fn field_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        field::view(cx);

        // `MIX` has a knob as well as the figure's ▲ for the same reason the
        // shape layer has both a figure and a table — one is for reading, the
        // other for setting a number.
        VStack::new(cx, |cx| {
            macro_knob(cx, "MIX", "Dry against doubled", |params| &params.mix, 34.0);
            macro_knob(cx, "OUTPUT", "Level out", |params| &params.output, 34.0);
        })
        .class("hint-left")
        .width(Pixels(SIDE_WIDTH))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3))
        .child_top(Stretch(1.0))
        .child_bottom(Stretch(1.0));
    })
    .height(Pixels(FIELD_HEIGHT))
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_3));
}

/// One mirror switch: a pill that is checked while that axis mirrors.
///
/// Not a new widget. `SegmentedControl` is `.segment` labels inside a
/// `.segmented` row, and one label on its own is the same thing — including
/// `:checked` recolouring it, which only works because the label carries the
/// class itself (`plugins/doubler/docs/implementation/doubler-plan.md`).
/// `UI-9`'s `ToggleSwitch` would be a sliding switch, which is not what belongs
/// beside a figure.
fn mirror_switch(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    on: impl Lens<Target = bool>,
    axis: MirrorAxis,
) {
    HStack::new(cx, |cx| {
        Label::new(cx, label)
            .class("segment")
            .checked(on)
            .on_press(move |cx| cx.emit(UiEvent::ToggleMirror(axis)))
            .tooltip(move |cx| theme::hint(cx, hint));
    })
    .class("segmented");
}

/// The four mirror switches.
///
/// **One pill per axis, not a master switch with exceptions.** A master plus a
/// per-axis opt-out would mean two pieces of state deciding one write, and
/// "mirror is on but delay is not mirroring" is a state a reader has to
/// reconstruct. Separate pills say it outright.
///
/// They are separate `.segmented` groups rather than one, because one group is
/// how this interface says "pick exactly one of these" everywhere else.
///
/// **They live in the tab strip, not over the figure.** Four pills laid over the
/// top corner covered the outer dots, and the alternatives — icons without
/// tooltips, or a column that narrows the figure — both cost more than the row
/// of empty space that was already sitting here.
fn mirror_switches(cx: &mut Context) {
    HStack::new(cx, |cx| {
        icon::label(cx, icon::FLIP_HORIZONTAL_2);
        Label::new(cx, "MIRROR").class("label");
        mirror_switch(
            cx,
            "PAN",
            "Pan mirrors to the pair",
            Ui::mirror_pan,
            MirrorAxis::Pan,
        );
        mirror_switch(
            cx,
            "DETUNE",
            "Detune mirrors to the pair",
            Ui::mirror_detune,
            MirrorAxis::Detune,
        );
        mirror_switch(
            cx,
            "DELAY",
            "Delay copies to the pair",
            Ui::mirror_delay,
            MirrorAxis::Delay,
        );
        mirror_switch(
            cx,
            "GAIN",
            "Level copies to the pair",
            Ui::mirror_gain,
            MirrorAxis::Gain,
        );
    })
    .class("hint-left")
    // An unset size is `Stretch(1.0)`, not "size to content"
    // (`.agents/rules/vizia.md`).
    .width(Auto)
    .height(Auto)
    .col_between(Pixels(theme::SPACE_1))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0));
}

/// The tab strip is a segmented control: exactly the same "one of these" choice
/// as `Voices`, so it is the same widget rather than a new one.
fn tab_strip(cx: &mut Context) {
    HStack::new(cx, |cx| {
        SegmentedControl::new(cx, Ui::tab, &["MAIN", "DETAIL"], |cx, tab| {
            cx.emit(UiEvent::SelectTab(tab));
        });
        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));
        mirror_switches(cx);
    })
    .class("row")
    .height(Auto);
}

fn main_tab(cx: &mut Context) {
    VStack::new(cx, |cx| {
        macros(cx);
        tone::view(cx);
    })
    .height(Auto)
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_3))
    .display(Ui::tab.map(|tab| *tab == TAB_MAIN));
}

/// One labelled knob with its value underneath, which is the shape every macro
/// control takes.
pub(crate) fn macro_knob<P, F>(
    cx: &mut Context,
    label: &'static str,
    hint: &'static str,
    to_param: F,
    size: f32,
) where
    P: nih_plug::prelude::Param + 'static,
    F: Fn(&Arc<DoublerParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        param_bind::knob(cx, Ui::params, to_param, size).tooltip(move |cx| theme::hint(cx, hint));
        Label::new(cx, label).class("label");
        font::value(
            cx,
            nih_plug_vizia::widgets::param_base::ParamWidgetBase::make_lens(
                Ui::params,
                to_param,
                |param| param.to_string(),
            ),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn macros(cx: &mut Context) {
    HStack::new(cx, |cx| {
        macro_knob(cx, "DETUNE", "Pitch spread", |params| &params.detune, 56.0);
        macro_knob(
            cx,
            "DELAY",
            "Delay behind the dry",
            |params| &params.delay,
            56.0,
        );
        macro_knob(cx, "SPREAD", "Stereo width", |params| &params.spread, 56.0);
        macro_knob(
            cx,
            "HUMANIZE",
            "Drift, per voice",
            |params| &params.humanize,
            56.0,
        );
    })
    .class("row")
    .height(Auto);
}
