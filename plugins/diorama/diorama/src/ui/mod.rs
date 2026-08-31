//! The NXE Diorama editor.
//!
//! **One screen, no tabs** (`REQ-DIO-013`). Eight parameters fit, and the
//! question this plugin is used to answer — where is the voice, and is anything
//! putting it back — cannot be asked of half a panel (`SPK-19`).
//!
//! **The Advanced deviations are not here yet** (`REQ-DIO-009`). They are seven
//! more parameters and an engine change, and `dsp.md` does not yet say what each
//! deviation does as a number — so they are their own unit rather than a row
//! quietly added to this window (`ui.md`).

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::heartbeat::Lifeline;
use nxe_ui::hint::Describe;
use nxe_ui::taps::Tap;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::{Analysis, METERS};
use crate::params::DioramaParams;

mod field;
mod meters;
mod readout;

/// The window.
///
/// **The height is arithmetic, not a number found by looking.** Every part in
/// the column below has a known height — `nxe_ui::theme::LINE_*` exists so that
/// the text lines do too — so adding a row moves the window instead of running
/// off the bottom of it (`.agents/rules/ui.md`).
pub(crate) const WIDTH: u32 = theme::WINDOW_WIDTH;
const HEIGHT: u32 = (theme::SPACE_3 * 2.0
    + nxe_ui::header::HEIGHT
    + theme::SPACE_3
    + FIGURE_HEIGHT
    + theme::SPACE_3
    + knob_block_height(MAIN_KNOB)
    + theme::SPACE_3
    + theme::RULE
    + theme::SPACE_3
    + TRIM_HEIGHT
    + nxe_ui::status::HEIGHT) as u32;

/// The figure's row, including the inverted surface's own padding
/// (`nxe_ui::surface`).
const FIGURE_HEIGHT: f32 = field::HEIGHT + theme::SPACE_4 * 2.0;

/// The row under the rule: two readings on the left, `OUTPUT` on the right.
/// The readings are the taller of the two.
const TRIM_HEIGHT: f32 = if nxe_ui::readout::HEIGHT > knob_block_height(OUTPUT_KNOB) {
    nxe_ui::readout::HEIGHT
} else {
    knob_block_height(OUTPUT_KNOB)
};

/// **All seven the same size.** Sparkleur separates "what shapes the sound" from
/// "how much of it arrives", but here `MIX` is part of the distance too: the wet
/// path *replaces* the voice rather than adding to it, so turning `MIX` down is
/// bringing the voice back rather than turning an effect off
/// (`diorama_core::engine`).
const MAIN_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<DioramaParams>,
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
    /// The direct sound's stem. **Fixed, because it is the scale** (`DIO-17`):
    /// every arrival is drawn as its level *against* this one, so it stands at
    /// its own 0 dB and the reflections are read against it. Past `DEPTH` 0.75
    /// they stand **above** it, which is what being a long way off means.
    ///
    /// Its absolute level is on the meters and on the strip's `DIR`, where a
    /// level belongs.
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
                    .map(|(&position, &decibels)| Tap {
                        // **Rescaled**: the core normalises against its own
                        // 120 ms and this axis spans 100 (`ui/field`).
                        position: position * field::SPAN_RATIO,
                        level: arrival_position(decibels),
                    })
                    .collect();

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

/// The figure's vertical scale, in dB against the direct sound (`DIO-17`).
///
/// **Measured, not chosen.** With `ROOM` at its top, an arrival sits between
/// **+5.8 dB** of the direct sound at `DEPTH` 1 and **−32.2 dB** at `DEPTH` 0:
///
/// | `DEPTH` | loudest | quietest |
/// |---|---|---|
/// | 0.00 | −14.0 | −32.2 |
/// | 0.50 | −3.6 | −7.8 |
/// | 1.00 | **+5.8** | −4.0 |
///
/// A floor of 48 puts the near end at a third of the plot and the far end at
/// the top, so **`DEPTH` moves the pattern up the figure** — the one thing this
/// window exists to show. The meters' −60 would squeeze it into the top half.
///
/// **And there is headroom above the voice**, because the reflections really do
/// get louder than it: without the +6 everything past `DEPTH` 0.75 clipped flat
/// against the top and the last quarter of the knob drew nothing.
pub(crate) const ARRIVAL_FLOOR_DB: f32 = -48.0;
pub(crate) const ARRIVAL_CEILING_DB: f32 = 6.0;

/// An arrival's level against the direct sound as a height in the figure.
fn arrival_position(decibels: f32) -> f32 {
    if !decibels.is_finite() {
        return 0.0;
    }
    ((decibels - ARRIVAL_FLOOR_DB) / (ARRIVAL_CEILING_DB - ARRIVAL_FLOOR_DB)).clamp(0.0, 1.0)
}

pub fn create(
    params: Arc<DioramaParams>,
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
            direct: arrival_position(0.0),
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            readouts: vec![String::new(); readout::FIGURES],
            heartbeat,
        }
        .build(cx);

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    nxe_ui::header::header(cx, "Diorama", "forward and back", |_| {});
                    // The figure. It is what the plugin *is* (`ui.md`), and it
                    // is the window's one inverted surface (`UI-18`).
                    figure_row(cx);
                    main_row(cx);
                    Element::new(cx).class("rule");
                    trim_row(cx);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .row_between(Pixels(theme::SPACE_3));

                // **Outside everything else**, because "did the voice move or
                // just change level" is a question asked while looking at any
                // of it (`.agents/rules/ui.md`).
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
        // **`.root` is what paints the window.** Without it the ground is the
        // host's black while every `.panel` sits at `BACKGROUND`, so the panels
        // read as lighter boxes (`.agents/rules/vizia.md`).
        //
        // **No child space, and no row between.** The padding belongs to the
        // row above the strip so the strip can reach the edges; `.root` sets
        // `row-between: SPACE_4`, and those 16 px come out of the row, leaving
        // anything `Stretch(1.0)` in it — the meter strip — short of the
        // fixed-height column beside it (`SPK-23`).
        .class("root")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .child_space(Pixels(0.0))
        .row_between(Pixels(0.0));
    })
}

/// The figure, on the accent — the window's one exception
/// (`.agents/rules/ui.md`).
fn figure_row(cx: &mut Context) {
    nxe_ui::surface::inverted(cx, |cx| {
        field::view(cx);
    })
    .height(Pixels(FIGURE_HEIGHT))
    .width(Stretch(1.0));
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
/// (`REQ-DIO-008`), so this is here for gain staging and nothing else — which
/// is why it is under the rule rather than in the row above.
/// The row under the rule: the two readings that are not about distance, and
/// the final trim.
///
/// **It held one small knob and 800 px of nothing.** The two readings that did
/// not fit on the status bar went here rather than off the window (`DIO-16`,
/// `ui/readout.rs`).
fn trim_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        readout::checks(cx);

        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));

        knob_block(cx, "OUTPUT", "The final trim", OUTPUT_KNOB, |params| {
            &params.output
        });
    })
    .height(Pixels(TRIM_HEIGHT))
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_4));
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
    F: Fn(&Arc<DioramaParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The description goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
        nxe_plug_ui::knob(cx, Ui::params, to_param, size).describe(hint);
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
    F: Fn(&Arc<DioramaParams>) -> &P + Copy + 'static,
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
        // **Against the shared constant, not against a number.** Writing the
        // number here is how one window quietly stops matching the other four.
        assert_eq!(super::WIDTH, nxe_ui::theme::WINDOW_WIDTH);
        // Nothing to compare the height against but its own arithmetic — what
        // this pins is that it *is* arithmetic, so a part changing size moves
        // the window instead of running off it.
        assert!(super::HEIGHT > super::field::HEIGHT as u32);
    }
}

#[cfg(test)]
mod arrivals {
    use super::*;
    use diorama_core::depth::Macros;

    /// The figure's heights at one `DEPTH`, loudest first.
    ///
    /// **Nothing is processed.** That is the point: the figure is resolved from
    /// the macros, so it answers with the transport stopped.
    fn heights(depth: f32) -> Vec<f32> {
        let mut engine = diorama_core::Engine::new(48_000.0);
        engine.set(Macros {
            depth,
            direct: 0.5,
            room: 1.0,
            damping: 0.5,
            width: 0.6,
            clarity: 0.0,
            mix: 1.0,
            output: 0.0,
        });

        let mut heights: Vec<f32> = crate::arrival_levels(&engine.pattern(), engine.direct_level())
            .iter()
            .map(|decibels| arrival_position(*decibels))
            .collect();
        heights.sort_by(|a, b| b.partial_cmp(a).unwrap());
        heights
    }

    /// **The figure has to use its height, and it has to move with the knob**
    /// (`DIO-17`).
    ///
    /// Two defects, both seen in a host. The arrivals were drawn as a linear
    /// ratio against a constant that was wrong by fourteen times, so the
    /// tallest stem reached 7 % of the plot. The first fix divided by the
    /// *measured* buses, and the figure then stopped moving unless something
    /// was playing — **this test processes no signal at all**, which is what
    /// makes that impossible to reintroduce.
    #[test]
    fn depth_moves_the_pattern_up_the_figure() {
        let near = heights(0.0);
        let far = heights(1.0);

        let voice = arrival_position(0.0);

        // Far: the reflections stand **above** the voice, which is what being a
        // long way off means — and they must not be clipped against the top, or
        // the last quarter of the knob draws nothing.
        assert!(
            far[0] > voice && far[0] < 1.0,
            "the loudest arrival is at {} with the voice at {voice}",
            far[0]
        );
        // Near: low, and still drawn. A floor that swallowed them would hide
        // the thing `DEPTH` is being turned down *from*.
        assert!(
            (0.15..voice).contains(&near[0]),
            "the near pattern sits at {}",
            near[0]
        );
        // And the two are not the same picture.
        assert!(
            far[0] - near[0] > 0.3,
            "DEPTH moved the pattern by {}",
            far[0] - near[0]
        );
    }
}
