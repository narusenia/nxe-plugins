//! Sparkleur's editor.
//!
//! Layout follows `plugins/sparkleur/docs/specifications/ui.md`.
//!
//! **One fixed window size, and tabs inside it.** The Doubler learned this the
//! expensive way: asking a host to resize the editor on a disclosure toggle
//! wedged it in Ableton. Tabs need nothing from the host for a control to
//! become reachable.
//!
mod advanced;
mod curve;
mod field;
mod meters;
mod readout;

use crate::analysis::{Analysis, BANDS, HIGH_HZ, LOW_HZ, METERS};
use crate::params::SparkleurParams;
use nih_plug::prelude::Editor;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::heartbeat::Lifeline;
use nxe_ui::hint::Describe;
use nxe_ui::{font, theme};
use sparkleur_core::crossover::BAND_COUNT;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The window.
///
/// **The height is arithmetic, not a number found by looking.** Every part in
/// the column below has a known height — `nxe_ui::theme::LINE_*` exists so that
/// the text lines do too — so adding a row to the table moves the window
/// instead of running off the bottom of it. It ran off the bottom three times
/// in one afternoon before this (`SPK-19`).
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

/// The knob sizes. The five that shape the sound are the large ones; the two
/// that decide how much of it arrives are smaller and sit apart, because they
/// are a different question (`ui.md`).
const SHAPE_KNOB: f32 = 52.0;
const OUTPUT_KNOB: f32 = 38.0;

/// The named points on the `CHARACTER` axis and where they sit
/// (`sparkleur_core::character`).
const ANCHORS: [(&str, f32); 3] = [("POLISH", 0.0), ("GLOSS", 0.5), ("CRUSH", 1.0)];

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    params: Arc<SparkleurParams>,
    /// Keeps the display's heartbeat running. Dropped with the window,
    /// which is what stops the thread (`nxe_ui::heartbeat`).
    ///
    /// **Never read on purpose**: what it does, it does by being dropped.
    #[allow(dead_code)]
    heartbeat: Lifeline,
    /// Which band the pointer is over, so the Advanced row and the region can
    /// mark each other. One value, both directions.
    hovered: Option<usize>,
    /// What the audio thread has published. Read on a heartbeat rather than
    /// mapped from the `Arc`: the handoff's identity never changes, so nothing
    /// would tell the binding system to look again.
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    dry: Curve,
    /// Peak and held peak per meter, normalized onto the meter's own scale.
    peaks: Vec<f32>,
    holds: Vec<f32>,
    /// **Always empty.** The widget takes two curves because Velour has two;
    /// a split topology has no separable added layer, and the per-band gains
    /// are what say what happened (`REQ-SPK-018`).
    wet: Curve,
    /// The five regions, and the readout strip's printed figures and bar.
    ///
    /// **All copied here rather than read inside a lens.** A lens that reads the
    /// handoff is re-evaluated once per *frame*, so the window redrew at the
    /// frame rate whenever a band gain moved — see `nxe_ui::readout`.
    bands: Vec<nxe_ui::band::Band>,
    readouts: Vec<String>,
    gauges: Vec<f32>,
    /// What each band is running at, for the Advanced table.
    applied_gains: Vec<String>,
    /// Where the shown band is sitting on its own transfer curve.
    curve_point: Option<(f32, f32)>,
    /// What `bands` needs besides the parameters, kept because the heartbeat
    /// rebuilds them and the rate is read once when the window opens.
    host_rate: f32,
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
                let peaks = self.analysis.peaks.read();
                let holds = self.analysis.holds.read();
                self.peaks = peaks.iter().copied().map(meter_position).collect();
                self.holds = holds.iter().copied().map(meter_position).collect();
                self.bands = field::bands_of(&self.params, self.host_rate, &self.analysis);
                readout::poll(&self.analysis, &mut self.readouts, &mut self.gauges);
                advanced::poll(&self.analysis, &mut self.applied_gains);
                curve::poll(&self.params, &self.analysis, &mut self.curve_point);
            }
        });
    }
}

/// How often the display re-reads the analysis. 30 Hz is as fast as a figure
/// needs to look alive, and half the work of matching the frame rate.
const ANALYSIS_INTERVAL: Duration = Duration::from_millis(33);

/// The floor of the spectrum curve. Below this a band is drawn as silence —
/// without a floor the curve sits on the noise of an idle track.
const SPECTRUM_FLOOR_DB: f32 = -72.0;

/// The floor of the meters. Shallower than the spectrum's: a meter is read for
/// "how close to clipping", and 60 dB of travel puts a working mix in the top
/// third where it can be read.
pub(crate) const METER_FLOOR_DB: f32 = -60.0;

/// An amplitude as a position on a meter.
fn meter_position(amplitude: f32) -> f32 {
    let db = 20.0 * amplitude.max(1e-9).log10();
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

/// One published band frame as a curve across the figure's axis.
///
/// **Both mappings live on this side of the widget**, which is the same split
/// as everywhere else: `nxe-ui` knows nothing about hertz or decibels, and
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

pub fn create(
    params: Arc<SparkleurParams>,
    state: Arc<ViziaState>,
    sample_rate: Arc<AtomicU32>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx, theme::Palette::SPARKLEUR);

        // **Read once, when the window opens.** The rate decides where the top
        // boundary is capped (`sparkleur_core::crossover`), and that is the
        // only thing on screen that depends on it. A host that changes rate
        // with the editor open leaves the figure a little out at the very top
        // until it is reopened; polling for it every frame would be work for a
        // case that does not happen mid-session.
        let host_rate = f32::from_bits(sample_rate.load(Ordering::Relaxed));

        // **Started before the model, because the model holds what stops
        // it.** The lifeline dies with the window's context, and the
        // thread ends within one interval (`nxe_ui::heartbeat`).
        let heartbeat = nxe_ui::heartbeat::start(cx, ANALYSIS_INTERVAL, UiEvent::Poll);

        Ui {
            params: params.clone(),
            hovered: None,
            analysis: analysis.clone(),
            dry: Curve::new(),
            wet: Curve::new(),
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            bands: field::bands_of(&params, host_rate, &analysis),
            readouts: vec![String::new(); readout::FIGURES],
            gauges: vec![0.0; readout::GAUGES],
            applied_gains: vec![String::new(); BAND_COUNT],
            curve_point: None,
            host_rate,
            heartbeat,
        }
        .build(cx);

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                VStack::new(cx, |cx| {
                    header(cx);

                    // The figure. It is what the plugin *is* (`ui.md`).
                    figure_row(cx);

                    // **No tabs.** They hid seventeen of the thirty-five
                    // controls behind a click, and the question a multiband
                    // compressor is used to answer — which band is doing what —
                    // cannot be asked of half a panel (`SPK-19`).
                    shape_row(cx);
                    Element::new(cx).class("rule");
                    advanced::view(cx);
                })
                .width(Stretch(1.0))
                .height(Stretch(1.0))
                .row_between(Pixels(theme::SPACE_3));

                // **Outside everything else**, because "is this louder or
                // better" is a question asked while looking at any of it
                // (`ui.md`).
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
        // **`.root` is what paints the window.** Without it the background is
        // whatever the host's window is — black — and every `.panel`, which
        // sits at `BACKGROUND`, reads as a lighter box on it. The theme's
        // "two levels, not three" only works when the window is one of them
        // (measured off a screenshot: panels 0x0A, window 0x00).
        //
        // **No child space here**: the padding belongs to the row above the
        // status bar, so the strip can reach the edges.
        .class("root")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .child_space(Pixels(0.0));
    })
}

/// The figure and the window that reads one band of it, side by side.
///
/// **On the accent ground — the window's exception** (`.agents/rules/ui.md`).
/// It is what the plugin *is*, and a glance should land on it.
///
/// **The transfer curve beside it had to stop being a `.panel` first.** For one
/// build it was one, and it went *completely black*: its traces followed the
/// nested palette and inverted, while its ground came from the stylesheet,
/// which cannot see one. Anything that carries its own ground onto this surface
/// has to read the palette for it (`curve::view`).
///
/// **The labels inside say `.ink-muted` for themselves**, for the same reason.
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

/// The inverted panel's height: the figure, plus the panel's own padding.
const FIGURE_HEIGHT: f32 = field::HEIGHT + theme::SPACE_4 * 2.0;

fn header(cx: &mut Context) {
    // The product's name — the vendor is its own mark to the left — with what
    // the window is for on the right, and the rule under both
    // (`nxe_ui::header`).
    //
    // **No wrapping row.** `.class("row")` centres its children vertically, and
    // the header wants the whole of the height it asks for
    // (`.agents/rules/vizia.md`).
    //
    // **`MODE` is in the band rather than in a panel** because it changes what
    // every other control does. A switch that re-scales the whole window and
    // sits in a row of ordinary controls gets missed (`.agents/rules/ui.md`).
    nxe_ui::header::header(cx, "Sparkleur", "five-band dynamics + sparkle", |cx| {
        nxe_plug_ui::segmented(cx, Ui::params, |params| &params.mode, &["Soft", "Hard"])
            .describe("How far every macro reaches");
    });
}

/// The seven controls that shape the sound, on one line.
///
/// `MIX` and `OUTPUT` are not part of the shape — one decides how much of it is
/// heard and the other how loud the result is — so they are smaller and sit
/// apart, past a stretch (`ui.md`).
fn shape_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        macro_knob(cx, "SPARK", "How much of everything", |params| {
            &params.spark
        });
        character_knob(cx);
        macro_knob(cx, "BODY", "Lean on the low mids", |params| &params.body);
        macro_knob(cx, "AIR", "Lean on the top, and the sparkle", |params| {
            &params.air
        });
        macro_knob(
            cx,
            "SPEED",
            "Faster or slower than the character",
            |params| &params.speed,
        );

        Element::new(cx).width(Stretch(1.0)).height(Pixels(0.0));

        knob_block(
            cx,
            "MIX",
            "Dry against the processed signal",
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
    F: Fn(&Arc<SparkleurParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, |cx| {
        // The tooltip goes on the knob rather than the whole block, so it does
        // not follow the pointer around the label and the number.
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
    F: Fn(&Arc<SparkleurParams>) -> &P + Copy + 'static,
{
    knob_block(cx, label, hint, SHAPE_KNOB, to_param);
}

/// `CHARACTER` reads as **the nearest anchor and a percentage**, on one line.
///
/// Velour tried three names side by side and it failed three ways — the accent
/// was a fill rather than a weight, the selected one changed size, and the
/// column they needed clipped the knob beside it (`ui.md`). One line says the
/// same thing and cannot do any of that.
fn character_knob(cx: &mut Context) {
    VStack::new(cx, |cx| {
        nxe_plug_ui::knob(cx, Ui::params, |params| &params.character, SHAPE_KNOB)
            .describe("POLISH through GLOSS to CRUSH");
        Label::new(cx, "CHARACTER").class("label");
        font::value(
            cx,
            Ui::params.map(|params| {
                character_readout(params.character.value(), &params.character.to_string())
            }),
        );
    })
    .width(Stretch(1.0))
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

/// Which name a position belongs to: **an equal share of the axis each**, not
/// the anchor it happens to be nearest.
///
/// Nearest-anchor is the obvious rule and it is the wrong one here, because the
/// anchors are not spaced the way the names are. POLISH and CRUSH sit at the
/// two **ends** while GLOSS sits in the **middle**, so "nearest" hands GLOSS
/// half the axis and the other two a quarter each. The default at 0.27 then
/// read `GLOSS 27 %` — one hundredth past the midpoint and already named after
/// the anchor it is walking away from, while `REQ-SPK-006` had chosen 0.25–0.30
/// precisely to sit *toward POLISH*. Two documents, each self-consistent,
/// disagreeing because the readout rule was written without the anchor
/// positions in front of it.
///
/// A third of the axis each puts the boundaries at 0.33 and 0.67, which is what
/// "toward POLISH" means to someone looking at the knob.
fn named(position: f32) -> &'static str {
    let position = if position.is_finite() { position } else { 0.5 };
    match position {
        position if position < 1.0 / 3.0 => ANCHORS[0].0,
        position if position < 2.0 / 3.0 => ANCHORS[1].0,
        _ => ANCHORS[2].0,
    }
}

/// The one line under the knob.
///
/// **The percentage is the parameter's own**, so the plugin and the host never
/// show two different numbers for one control. At the very ends the number is
/// dropped: `POLISH 0 %` reads as "no polish" when it is the most polished the
/// plugin gets, and the same at the top.
///
/// | position | reads |
/// |---|---|
/// | 0.00 | `POLISH` |
/// | 0.27 (the default) | `POLISH 27 %` |
/// | 0.50 | `GLOSS 50 %` |
/// | 0.80 | `CRUSH 80 %` |
/// | 1.00 | `CRUSH` |
///
/// Eleven characters at the widest, which is what keeps it off the knob beside
/// it — the third of Velour's three failures (`ui.md`).
fn character_readout(position: f32, formatted: &str) -> String {
    match position {
        position if position <= 0.005 => ANCHORS[0].0.to_owned(),
        position if position >= 0.995 => ANCHORS[2].0.to_owned(),
        position => format!("{} {formatted}", named(position)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every parameter has somewhere to be touched.**
    ///
    /// A parameter with no control is one a user can only reach through the
    /// host's generic view, and **nothing else would notice** — it compiles, it
    /// saves, it automates, and the window simply never mentions it. Thirty-three
    /// is enough of them for one to go missing quietly.
    /// **The status bar cannot clip.** A sentence longer than the space the
    /// figures leave is drawn straight over them (`SPK-23`, seen in a host),
    /// and neither the text handling nor the `clip-path` binding in this vizia
    /// revision offers a way to cut it. So the limit is enforced here, on the
    /// source that writes the sentences.
    #[test]
    fn no_hint_is_longer_than_the_strip() {
        const SOURCES: [&str; 4] = [
            include_str!("mod.rs"),
            include_str!("advanced.rs"),
            include_str!("field.rs"),
            include_str!("curve.rs"),
        ];

        let mut checked = 0;
        for source in SOURCES {
            // **Whitespace flattened first.** The helpers' arguments are found
            // by the `", "` between a label and its hint, and `cargo fmt` puts
            // that pair on two lines as soon as the call gains an argument —
            // which is exactly what happened when the bars gained their marks
            // (`UI-17`). The scan went on passing and had quietly stopped
            // looking at five of them.
            let source: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
            for rest in source.split(".describe(\"").skip(1) {
                let hint = rest.split('"').next().unwrap_or_default();
                checked += 1;
                assert!(
                    hint.chars().count() <= nxe_ui::status::MAX_HINT,
                    "{hint:?} is {} characters",
                    hint.chars().count()
                );
            }
            // The knob and bar helpers take theirs as an argument, so they are
            // written at the call site rather than beside `.describe`.
            for rest in source.split("\", \"").skip(1) {
                let hint = rest.split('"').next().unwrap_or_default();
                if hint.starts_with(char::is_uppercase) && hint.contains(' ') {
                    checked += 1;
                    assert!(
                        hint.chars().count() <= nxe_ui::status::MAX_HINT,
                        "{hint:?} is {} characters",
                        hint.chars().count()
                    );
                }
            }
        }
        // **A floor, not a smoke test.** `> 10` went on passing while a quarter
        // of the hints had become invisible to the scan. Nineteen is all of
        // them: seven written beside `.describe`, twelve handed to a helper.
        assert!(checked >= 19, "the scan found only {checked} hints");
    }

    #[test]
    fn every_parameter_has_a_control() {
        const PARAMS: &str = include_str!("../params.rs");
        const COUNT: usize = 35;
        const SOURCES: [&str; 4] = [
            include_str!("mod.rs"),
            include_str!("advanced.rs"),
            include_str!("field.rs"),
            include_str!("curve.rs"),
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

    #[test]
    fn each_name_owns_an_equal_share_of_the_axis() {
        assert_eq!(named(0.0), "POLISH");
        assert_eq!(named(0.32), "POLISH");
        assert_eq!(named(0.34), "GLOSS");
        assert_eq!(named(0.5), "GLOSS");
        assert_eq!(named(0.66), "GLOSS");
        assert_eq!(named(0.68), "CRUSH");
        assert_eq!(named(1.0), "CRUSH");

        // Each anchor is inside the share that carries its name, which is the
        // one thing the boundaries must not break.
        for (name, position) in ANCHORS {
            assert_eq!(named(position), name, "{name} fell outside its own share");
        }

        // A hostile value lands in the middle rather than panicking.
        assert_eq!(named(f32::NAN), "GLOSS");
    }

    /// **The default reads "POLISH", which is what `REQ-SPK-006` asked for.**
    ///
    /// It used to read `GLOSS 27 %`: the requirement put the default at
    /// 0.25–0.30 to sit toward POLISH, and the readout named whichever anchor
    /// was nearest — and GLOSS at 0.5 is nearer to 0.27 than POLISH at 0.0 is.
    /// Both were as specified. **The readout moved, not the default**, because
    /// the sound at 0.27 is not what was wrong (`SPK-18`).
    #[test]
    fn the_default_reads_as_polish() {
        let default = sparkleur_core::character::DEFAULT_POSITION;
        assert_eq!(named(default), "POLISH");
        // And the old rule really would have said otherwise, so this is a
        // change rather than a restatement.
        assert!((default - 0.5).abs() < default);
    }

    /// The ends say the name alone. `POLISH 0 %` reads as "no polish" when it
    /// is the most polished the plugin gets.
    #[test]
    fn the_ends_drop_the_percentage() {
        assert_eq!(character_readout(0.0, "0 %"), "POLISH");
        assert_eq!(character_readout(1.0, "100 %"), "CRUSH");
        assert_eq!(character_readout(0.27, "27 %"), "POLISH 27 %");
        assert_eq!(character_readout(0.5, "50 %"), "GLOSS 50 %");
    }

    /// The anchors are the axis's, not a second copy of them.
    #[test]
    fn the_anchors_match_the_axis() {
        for (_, position) in ANCHORS {
            let character = sparkleur_core::character::at(position);
            assert!(character.curve.down_ratio.is_finite());
        }
        // POLISH is gentler than CRUSH, which is what the names mean.
        assert!(
            sparkleur_core::character::at(ANCHORS[0].1).curve.down_ratio
                < sparkleur_core::character::at(ANCHORS[2].1).curve.down_ratio
        );
    }
}
