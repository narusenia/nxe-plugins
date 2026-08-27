//! The NXE Air editor.
//!
//! **One screen, no tabs** (`REQ-AIR-013`). Fifteen parameters fit, and the
//! question this plugin is used to answer — is the layer where I want it, and
//! is anything holding it back — cannot be asked of half a panel (`SPK-19`).

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::{ViziaState, ViziaTheming, create_vizia_editor};
use nxe_ui::{font, theme};
use std::sync::Arc;

use crate::params::AirParams;

mod advanced;

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
}

impl Model for Ui {}

pub fn create(params: Arc<AirParams>, state: Arc<ViziaState>) -> Option<Box<dyn Editor>> {
    // `ViziaTheming::None`: the plugin brings its own stylesheet and wants none
    // of vizia's defaults leaking into it.
    create_vizia_editor(state, ViziaTheming::None, move |cx, _| {
        theme::install(cx);

        Ui {
            params: params.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            nxe_ui::header::header(cx, "NXE AIR", "signal-driven texture");
            main_row(cx);
            Element::new(cx).class("rule");
            advanced::view(cx);
        })
        // **`.root` is what paints the window.** Without it the ground is the
        // host's black while every `.panel` sits at `BACKGROUND`, so the panels
        // read as lighter boxes — the theme's "two levels, not three" needs the
        // window to be one of them. Sparkleur shipped one build without it
        // (`.agents/rules/vizia.md`).
        .class("root")
        .width(Stretch(1.0))
        .height(Stretch(1.0))
        .row_between(Pixels(theme::SPACE_3))
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
        const SOURCES: [&str; 2] = [include_str!("mod.rs"), include_str!("advanced.rs")];

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
