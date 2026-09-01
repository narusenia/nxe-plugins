//! The NXE Pumice editor.
//!
//! **One screen, no tabs** (`REQ-PUM-013`). `DEPTH` and `SHARPNESS` are asked
//! together — how much to take out, and how narrow a thing counts — so they
//! cannot live in different halves of a disclosure (`SPK-19`).
//!
//! ## The shape
//!
//! ```text
//! header
//! readout strip
//! ┌─ the figure, full width, on the accent ──────────┐
//! ┌─ DEPTH and MODE ─┬─ everything else ─────────────┐
//! status
//! ```
//!
//! **The figure is full width and the row under it is split.** The axis that
//! needs pixels is frequency (`field`), and the split gives the window the two
//! panels of the reference it was drawn from without moving the figure into a
//! narrow column.
//!
//! **The split is a rule, not a second inverted surface.** `.agents/rules/ui.md`
//! allows exactly two — the status bar and the figure — and the figure is
//! already one of them. An accent-coloured control panel beside it would take
//! the glance the figure is supposed to get.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::curve::Curve;
use nxe_ui::heartbeat::Lifeline;
use nxe_ui::hint::Describe;
use nxe_ui::node::FieldNode;
use nxe_ui::{font, theme};
use std::sync::Arc;
use std::time::Duration;

use crate::analysis::{Analysis, METERS};
use crate::params::{self, PumiceParams};

mod field;
mod readout;

pub(crate) const WIDTH: u32 = theme::WINDOW_WIDTH;

/// **The height is arithmetic, not a number found by looking**
/// (`.agents/rules/ui.md`). Adding a row moves the window instead of running
/// off the bottom of it.
const HEIGHT: u32 = (theme::SPACE_3 * 2.0
    + nxe_ui::header::HEIGHT
    + theme::SPACE_3
    + FIGURE_HEIGHT
    + theme::SPACE_3
    + SPLIT_HEIGHT
    + nxe_ui::status::HEIGHT) as u32;

/// The figure's row, including the inverted surface's own padding
/// (`nxe_ui::surface`).
const FIGURE_HEIGHT: f32 = field::HEIGHT + theme::SPACE_4 * 2.0;

/// The split row: a column of controls, the taller of the two sides.
const SPLIT_HEIGHT: f32 = knob_block_height(MAIN_KNOB)
    + theme::SPACE_3
    + theme::LINE_LABEL
    + theme::SPACE_2
    + theme::LINE_LABEL;

/// `DEPTH` is the one you reach for first, so it is the one that is bigger.
const MAIN_KNOB: f32 = 60.0;
const SIDE_KNOB: f32 = 44.0;

const ANALYSIS_INTERVAL: Duration = Duration::from_millis(33);

/// Below this the meters read as a floor rather than as a level.
pub(crate) const METER_FLOOR_DB: f32 = -60.0;

/// How deep the figure's reduction fill can go before it is drawn full height.
///
/// **The ceiling, plus nothing.** `pumice_core::Settings::ceiling_db` is what a
/// bin may lose at most, so a fill scaled to it uses the whole plot and never
/// clips — the mistake `DIO-17` records was a figure scaled to a number the
/// engine could not reach.
const REDUCTION_FLOOR_DB: f32 = -18.0;

/// The spectrum's window, in dB. Wide enough that a vocal's noise floor is not
/// drawn as silence and its peaks are not drawn flat against the top.
const SPECTRUM_FLOOR_DB: f32 = -84.0;
const SPECTRUM_CEILING_DB: f32 = 0.0;

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (WIDTH, HEIGHT))
}

#[derive(Lens)]
pub(crate) struct Ui {
    pub(crate) params: Arc<PumiceParams>,
    /// Keeps the display's heartbeat running. Dropped with the window, which is
    /// what stops the thread (`nxe_ui::heartbeat`).
    #[allow(dead_code)]
    heartbeat: Lifeline,
    analysis: Arc<Analysis>,
    /// The reactive copies. Updating these is what makes the display move.
    spectrum: Curve,
    reduction: Curve,
    weight: Curve,
    nodes: Vec<FieldNode>,
    /// Which node the pointer is on, for the strip to print.
    hovered: Option<usize>,
    peaks: Vec<f32>,
    holds: Vec<f32>,
    readouts: Vec<String>,
}

#[derive(Clone)]
pub(crate) enum UiEvent {
    /// The heartbeat asking the model to re-read what the audio thread
    /// published.
    Poll,
    /// A gesture asking the model to re-read the node parameters.
    ///
    /// **Separate from `Poll`, because a drag cannot wait for it.** The
    /// heartbeat runs at 30 Hz and a dragged node has to follow the pointer;
    /// re-reading on the gesture is what makes the figure show the *clamped*
    /// value immediately (`nxe_ui::node`).
    Sync,
    Hover(Option<usize>),
}

impl Model for Ui {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|ui_event: &UiEvent, _| match ui_event {
            UiEvent::Poll => {
                self.spectrum = curve(&self.analysis.spectrum.read(), |db| {
                    ((db - SPECTRUM_FLOOR_DB) / (SPECTRUM_CEILING_DB - SPECTRUM_FLOOR_DB))
                        .clamp(0.0, 1.0)
                });
                // **Depth, not height**: the widget hangs this from the ceiling
                // (`nxe_ui::node`), so zero is "nothing taken out".
                self.reduction = curve(&self.analysis.reduction.read(), |db| {
                    (db / REDUCTION_FLOOR_DB).clamp(0.0, 1.0)
                });
                // `0.5` is the resting line and the weight runs `0..=2`.
                self.peaks = self.analysis.peaks.read().to_vec();
                self.holds = self.analysis.holds.read().to_vec();
                self.readouts = readout::figures(&self.analysis, &self.peaks);
            }
            UiEvent::Sync => self.sync_nodes(),
            UiEvent::Hover(over) => {
                self.hovered = *over;
                let text = readout::hovered_text(self);
                cx.emit(nxe_ui::hint::HintEvent::Dynamic(
                    (!text.is_empty()).then_some(text),
                ));
            }
        });
    }
}

impl Ui {
    /// The nodes as the figure draws them, read back out of the parameters,
    /// **and the curve they add up to**.
    ///
    /// **The weight is sampled, not read back off the engine's bins.** It is
    /// what the user set, it has an exact value at every frequency, and going
    /// through the bins drew stairs below 300 Hz — a logarithmic display grid
    /// steps 5.6 Hz at 100 Hz against a bin every 23.4
    /// (`pumice_core::nodes::weight_at`).
    fn sync_nodes(&mut self) {
        let nodes: [pumice_core::Node; pumice_core::NODES] =
            std::array::from_fn(|index| self.params.nodes[index].resolve());
        let range = pumice_core::Range {
            low_hz: params::position_to_hz(self.params.low.value()),
            high_hz: params::position_to_hz(self.params.high.value()),
        };
        let edge = pumice_core::Settings::DEFAULT.edge_octaves;

        self.weight = (0..pumice_core::CURVE_POINTS)
            .map(|index| {
                let hz = pumice_core::display::point_hz(index);
                let weight = pumice_core::nodes::weight_at(hz, &nodes, range, edge);
                // `0.5` is the resting line and the weight runs `0..=2`.
                (params::hz_to_position(hz), (weight * 0.5).clamp(0.0, 1.0))
            })
            .collect();

        self.nodes = self
            .params
            .nodes
            .iter()
            .filter(|node| node.enabled.value())
            .map(|node| FieldNode {
                x: node.freq.value(),
                // `depth` is bipolar, so its normalized value is the height and
                // `0.5` is the resting line.
                y: (node.depth.value() + 1.0) * 0.5,
                half_width: params::position_to_octaves(node.width.value())
                    / (2.0 * field::axis_octaves()),
            })
            .collect();
    }
}

/// A published curve as the figure's polyline. The `x` grid is the engine's,
/// which is the figure's own axis (`pumice_core::display`).
fn curve(values: &[f32], map: impl Fn(f32) -> f32) -> Curve {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let hz = pumice_core::display::point_hz(index);
            (params::hz_to_position(hz), map(*value))
        })
        .collect()
}

pub fn create(
    params: Arc<PumiceParams>,
    state: Arc<ViziaState>,
    analysis: Arc<Analysis>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx, theme::Palette::PUMICE);

        // **Started before the model, because the model holds what stops it.**
        let heartbeat = nxe_ui::heartbeat::start(cx, ANALYSIS_INTERVAL, UiEvent::Poll);

        let mut ui = Ui {
            params: params.clone(),
            analysis: analysis.clone(),
            spectrum: Curve::new(),
            reduction: Curve::new(),
            weight: Curve::new(),
            nodes: Vec::new(),
            hovered: None,
            peaks: vec![0.0; METERS],
            holds: vec![0.0; METERS],
            readouts: vec![String::new(); readout::FIGURES],
            heartbeat,
        };
        ui.sync_nodes();
        ui.build(cx);

        VStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                nxe_ui::header::header(cx, "Pumice", "resonance, only when", |cx| {
                    nxe_plug_ui::segmented(cx, Ui::params, |p| &p.quality, &["L", "N", "H"])
                        .describe("How fine the transform is, and how much latency");
                });
                figure_row(cx);
                split_row(cx);
            })
            .width(Stretch(1.0))
            .height(Stretch(1.0))
            .row_between(Pixels(theme::SPACE_3))
            .child_space(Pixels(theme::SPACE_3));

            // **Flush to the bottom edge, and the full width of the window.**
            readout::status(cx);
        })
        // **`.root` is what paints the window** (`.agents/rules/vizia.md`), and
        // `row_between(0)` because the padding belongs to the column above so
        // the strip can reach the edges (`SPK-23`).
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

/// The row under the figure: `DEPTH` on its own, everything else beside it.
///
/// **Split by a rule rather than by a colour** (see the module note). What the
/// division says is "this is the one you reach for, and these are the rest".
fn split_row(cx: &mut Context) {
    HStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            knob_block(
                cx,
                "DEPTH",
                "How much resonance to take out",
                MAIN_KNOB,
                |p| &p.depth,
            );
            nxe_plug_ui::segmented(cx, Ui::params, |p| &p.mode, &["ADAPTIVE", "STATIC"])
                .describe("Whether a long-term map decides where, or only the moment");
        })
        .class("panel")
        .width(Pixels(196.0))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));

        Element::new(cx)
            .class("rule-vertical")
            .width(Pixels(theme::RULE));

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                knob_block(
                    cx,
                    "SHARPNESS",
                    "How narrow a peak has to be",
                    SIDE_KNOB,
                    |p| &p.sharpness,
                );
                knob_block(
                    cx,
                    "SPEED",
                    "How fast the reduction follows",
                    SIDE_KNOB,
                    |p| &p.speed,
                );
                knob_block(
                    cx,
                    "LOW",
                    "The bottom of the working band",
                    SIDE_KNOB,
                    |p| &p.low,
                );
                knob_block(cx, "HIGH", "The top of the working band", SIDE_KNOB, |p| {
                    &p.high
                });
                knob_block(
                    cx,
                    "MIX",
                    "Dry against the treated signal",
                    SIDE_KNOB,
                    |p| &p.mix,
                );
                knob_block(cx, "OUTPUT", "The last gain in the chain", SIDE_KNOB, |p| {
                    &p.output
                });
            })
            .class("row")
            .height(Auto)
            .width(Stretch(1.0));

            HStack::new(cx, |cx| {
                nxe_plug_ui::toggle(cx, Ui::params, |p| &p.delta, "DELTA")
                    .describe("Hear only what is being taken out");
            })
            .height(Auto)
            .width(Stretch(1.0))
            .col_between(Pixels(theme::SPACE_2));
        })
        .class("panel")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3));
    })
    .height(Pixels(SPLIT_HEIGHT))
    .width(Stretch(1.0))
    .col_between(Pixels(theme::SPACE_3));
}

/// How tall a [`knob_block`] of a given knob size comes out.
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
    F: Fn(&Arc<PumiceParams>) -> &P + Copy + 'static,
{
    VStack::new(cx, move |cx| {
        nxe_plug_ui::knob(cx, Ui::params, to_param, size).describe(hint);
        Label::new(cx, label)
            .class("label")
            .height(Pixels(theme::LINE_LABEL));
        // **Figures are set in the mono face.** A proportional one changes
        // width between a `1` and an `8`, and a value that jitters under a drag
        // is what fixing the decimal count was meant to prevent.
        font::value(
            cx,
            ParamWidgetBase::make_lens(Ui::params, to_param, |param| param.to_string()),
        )
        .height(Pixels(theme::LINE_VALUE));
    })
    .height(Pixels(knob_block_height(size)))
    .width(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
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
    ///
    /// **The node fields are scanned through `NodeParams` too.** Twenty-four of
    /// this window's parameters live in an array of nested structs, and the
    /// controls that reach them are the figure's gestures rather than named
    /// widgets — so the scan looks for the field being read or written
    /// anywhere in the window's sources, which is what `field.rs` does.
    #[test]
    fn every_parameter_has_a_control() {
        const PARAMS: &str = include_str!("../params.rs");
        /// The main set plus the range, plus one node's four.
        ///
        /// **`depth` appears twice and that is not a mistake**: `NodeParams`
        /// has one and so does `PumiceParams`. Their ids are `d_1`..`d_6` and
        /// `depth`, so the parameter map is unambiguous even though the field
        /// names are not.
        const COUNT: usize = 14;
        const SOURCES: [&str; 3] = [
            include_str!("mod.rs"),
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

    /// The window is as tall as its parts, and the width is the line's
    /// (`.agents/rules/ui.md`).
    #[test]
    fn the_window_is_the_size_of_its_parts() {
        // **Against the shared constant, not against a number.** Writing the
        // number here is how one window quietly stops matching the other five.
        assert_eq!(super::WIDTH, nxe_ui::theme::WINDOW_WIDTH);
        assert_eq!(super::field::HEIGHT, 200.0, "the figure is the line's size");

        let parts = nxe_ui::theme::SPACE_3 * 2.0
            + nxe_ui::header::HEIGHT
            + nxe_ui::theme::SPACE_3
            + super::FIGURE_HEIGHT
            + nxe_ui::theme::SPACE_3
            + super::SPLIT_HEIGHT
            + nxe_ui::status::HEIGHT;
        assert_eq!(super::HEIGHT, parts as u32);
    }

    /// The figure's reduction fill is scaled to what the engine can actually
    /// reach, so the top of the plot is used and nothing clips (`DIO-17`).
    #[test]
    fn the_figure_is_scaled_to_the_engine() {
        assert_eq!(
            super::REDUCTION_FLOOR_DB,
            -pumice_core::Settings::DEFAULT.ceiling_db
        );
    }
}
