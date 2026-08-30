//! The transfer-curve window: one band's dynamics, read only.
//!
//! **Which band**: the soloed one, else the one being pointed at, else
//! PRESENCE (`ui.md`). PRESENCE is the fallback because it is where a mix turns
//! painful, and that is this plugin's main ground.
//!
//! **Velour's window and this one are different things.** Velour draws a
//! waveform's transfer curve — what the shaper does to a sample. This draws
//! **input level against output level**, which is what a compressor is read by:
//! under the diagonal is compression, over it is lift, and both knees are
//! visible at once so turning `CHARACTER` moves two of them together
//! (`REQ-SPK-006`).
//!
//! **Read only.** A grabbable curve would have to write back into four
//! parameters — two thresholds and two ratios — and from a dragged point there
//! is no way to say which of them moved.
//!
//! The curve comes from `sparkleur_core::engine::transfer_db`, the same gain
//! computer the audio path runs, so the window cannot show a curve the sound
//! does not have.

use super::Ui;
use crate::analysis::Analysis;
use crate::params::SparkleurParams;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::curve::{Curve, CurveView, CurveViewModifiers};
use nxe_ui::theme;
use sparkleur_core::crossover::BAND_COUNT;
use sparkleur_core::engine::transfer_db;

/// How many points the curve is drawn with. Enough that a knee reads as a curve
/// rather than a corner.
const RESOLUTION: usize = 96;

/// The window's range. `-60` is where the upward floor lives, and `0` is full
/// scale — the span a mix is actually read across (`ui.md`).
const LOW_DB: f32 = -60.0;
const HIGH_DB: f32 = 0.0;

/// **The plot is square**, and it has to be.
///
/// The line the curve is read against is the diagonal, and a diagonal is only
/// 45 degrees when the two axes are the same length. Stretched to a tall box it
/// still runs corner to corner, but "below the line is compression" stops being
/// something the eye reads at a glance and becomes something to work out
/// (looked at in a host, `SPK-15`).
///
/// The number is what is left of [`super::field::HEIGHT`] once the label under
/// the plot and the panel's own padding are taken out, so the frame sits
/// exactly as tall as the figure beside it.
///
/// **The padding is `.panel`'s, from the stylesheet** — `child-space: 16` and
/// `row-between: 12`, not the 8 this first assumed. Getting it wrong put the
/// plot through the right-hand border (`SPK-15`).
const PLOT: f32 = super::field::HEIGHT - theme::SPACE_4 * 2.0 - theme::SPACE_3 - LABEL;

/// The band's name under the plot. **Given a height rather than measured**: the
/// plot's size is worked out from what is left, and "what is left" cannot be
/// arithmetic on a number nobody has written down.
const LABEL: f32 = 16.0;

/// The panel around the plot: the plot plus `.panel`'s own padding.
pub const WIDTH: f32 = PLOT + theme::SPACE_4 * 2.0;

/// If the padding ever grows past the figure's height, the plot is a negative
/// number and the panel draws nothing. **Caught at compile time**, because a
/// test would only catch it after somebody ran one.
const _: () = assert!(PLOT > 0.0);

const NAMES: [&str; BAND_COUNT] = ["SUB", "BODY", "MID", "PRESENCE", "AIR"];

/// The fallback: where this plugin is mostly used.
const DEFAULT_BAND: usize = 3;

/// Which band the window is showing. **One value out of three inputs**, so it
/// is computed rather than mapped from a lens (`.agents/rules/vizia.md`).
fn shown(params: &SparkleurParams, hovered: Option<usize>) -> usize {
    let soloed = [
        params.solo_sub.value(),
        params.solo_body.value(),
        params.solo_mid.value(),
        params.solo_pres.value(),
        params.solo_air.value(),
    ];
    if let Some(index) = soloed.iter().position(|on| *on) {
        return index;
    }
    hovered
        .filter(|index| *index < BAND_COUNT)
        .unwrap_or(DEFAULT_BAND)
}

/// A level in dB onto `0..=1` across the window.
///
/// **The output uses the same mapping as the input**, which is what makes the
/// diagonal mean unity. Velour drew its window against a scale of its own and
/// the curve shrank as the drive went up (`VEL-13`); here the two axes are one
/// scale by construction, so that cannot happen.
fn axis(decibels: f32) -> f32 {
    ((decibels - LOW_DB) / (HIGH_DB - LOW_DB)).clamp(0.0, 1.0)
}

/// The curve, as points across the window.
fn curve_of(params: &SparkleurParams, hovered: Option<usize>) -> Vec<Curve> {
    let shape = params.display_shape();
    let levels = params.display_levels();
    let band = shown(params, hovered);

    vec![
        (0..=RESOLUTION)
            .map(|step| {
                let input_db = LOW_DB + (HIGH_DB - LOW_DB) * step as f32 / RESOLUTION as f32;
                (
                    axis(input_db),
                    axis(transfer_db(&shape, &levels, band, input_db)),
                )
            })
            .collect(),
    ]
}

/// Unity: the line the curve is read against (`nxe_ui::curve` `.reference`).
fn diagonal() -> Curve {
    vec![(0.0, 0.0), (1.0, 1.0)]
}

/// Where the shown band is sitting on its own curve, right now.
///
/// **Both halves come from the audio thread** (`SPK-19`): the detector's reading
/// is the input, and the gain actually applied — De-Harsh included — is what
/// takes it to the output. Recomputing the output from the curve instead would
/// give a point that is always exactly on the line, which is a picture of the
/// arithmetic rather than of the sound.
fn point_of(level_db: f32, gain_db: f32) -> Option<(f32, f32)> {
    if !level_db.is_finite() || !gain_db.is_finite() || level_db <= LOW_DB {
        return None;
    }
    Some((axis(level_db), axis(level_db + gain_db)))
}

/// Where the shown band is sitting on its own curve. Called on the heartbeat —
/// **not read inside a lens**, which is re-evaluated once per frame
/// (`nxe_ui::readout`).
pub(crate) fn poll(params: &SparkleurParams, analysis: &Analysis, point: &mut Option<(f32, f32)>) {
    let band = shown(params, None);
    *point = point_of(analysis.levels.read()[band], analysis.gains.read()[band]);
}

pub fn view(cx: &mut Context) {
    // **The frame is painted from the palette, not from `.panel`.** This panel
    // sits on the window's inverted surface, and **a stylesheet cannot see a
    // nested palette**: styled by CSS the ground stayed black while the traces
    // inverted to black with the surface, and the curve disappeared entirely
    // (`SPK-23`, seen in a host). Read here, at build time, inside the surface.
    let palette = theme::palette(cx);
    VStack::new(cx, |cx| {
        CurveView::new(
            cx,
            // Recomputed whenever a parameter or the hover moves, which is what
            // makes the window follow `CHARACTER`.
            Ui::params.map(|params| curve_of(params, None)),
            // No bands, no handles, no analysis behind it: `CurveView` was not
            // changed to serve this beyond the reference line (`ui.md`).
            Vec::<nxe_ui::curve::Span>::new(),
            Vec::<nxe_ui::curve::Grip>::new(),
            Vec::new(),
            |_cx, _index, _gesture| {},
        )
        .reference(diagonal())
        .point(Ui::curve_point)
        // **Both sides given, not stretched.** A stretching plot would take the
        // height the row hands it and the width the panel hands it, and those
        // are not the same number.
        .width(Pixels(PLOT))
        .height(Pixels(PLOT));

        // Centred on the label itself, **not with stretch on the column**:
        // `child-left: 1s` and `child-right: 1s` on the parent are two more
        // stretches for the curve's own `Stretch(1.0)` width to share, which
        // left Velour's window drawn a third of the width it was given
        // (`.agents/rules/vizia.md`).
        Label::new(
            cx,
            Ui::params.map(|params| NAMES[shown(params, None)].to_string()),
        )
        .class("subtle")
        .class("ink-muted")
        .width(Stretch(1.0))
        .height(Pixels(LABEL))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));
    })
    // **A frame, like the meter strip's.** Without one the curve is a line
    // floating in the black beside the figure, and it reads as something that
    // escaped rather than as a panel of its own (looked at in a host, `SPK-15`).
    //
    // **Both sides given.** `Stretch(1.0)` here was a third of the row's height:
    // the row centres its children with `child-top: 1s` and `child-bottom: 1s`,
    // and those are two more stretches for the height to be divided among
    // (`.agents/rules/vizia.md`). The panel came out 58 px tall with a 140 px
    // plot hanging out of the bottom of it.
    .background_color(palette.ground.vizia())
    .border_width(Pixels(theme::RULE))
    .border_color(palette.line.vizia())
    .child_space(Pixels(theme::SPACE_4))
    .row_between(Pixels(theme::SPACE_3))
    .width(Pixels(WIDTH))
    .height(Pixels(super::field::HEIGHT));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solo_wins_over_a_hover_and_presence_is_the_fallback() {
        let params = SparkleurParams::default();
        assert_eq!(shown(&params, None), DEFAULT_BAND);
        assert_eq!(shown(&params, Some(0)), 0);
        // Out of range is the fallback rather than a panic.
        assert_eq!(shown(&params, Some(99)), DEFAULT_BAND);
    }

    /// **The plot is square, and the panel is as tall as the figure.** The
    /// diagonal is only 45 degrees at one aspect, and the frame only lines up
    /// with the figure at one height.
    #[test]
    fn the_plot_is_square_inside_a_panel_the_height_of_the_figure() {
        // The plot, the label under it and `.panel`'s own padding fill the
        // figure's height exactly — which is what makes the frame line up with
        // the figure beside it.
        assert_eq!(
            PLOT + LABEL + theme::SPACE_3 + theme::SPACE_4 * 2.0,
            super::super::field::HEIGHT
        );
        // And the plot is as wide as it is tall, which is what makes the
        // diagonal 45 degrees.
        assert_eq!(WIDTH - theme::SPACE_4 * 2.0, PLOT);
    }

    #[test]
    fn the_axis_covers_the_window_and_clamps_outside_it() {
        assert_eq!(axis(LOW_DB), 0.0);
        assert_eq!(axis(HIGH_DB), 1.0);
        assert_eq!(axis(-120.0), 0.0);
        assert_eq!(axis(12.0), 1.0);
        assert!((axis(-30.0) - 0.5).abs() < 1e-6);
    }

    /// **The window shows the plugin doing something out of the box.** `SPARK`
    /// rests at 0.35, not at zero — a dynamics processor that ships doing
    /// nothing reads as broken rather than as tasteful (`params.rs`), and the
    /// window is where that is visible.
    ///
    /// That the diagonal *is* unity is the gain computer's property and is
    /// tested there (`sparkleur_core::engine`); what has to hold here is that
    /// both axes go through one mapping, which is what makes the diagonal
    /// meaningful at all (`VEL-13`).
    #[test]
    fn the_default_curve_leaves_the_diagonal_in_both_directions() {
        let curves = curve_of(&SparkleurParams::default(), None);
        assert_eq!(curves.len(), 1);

        let quiet = curves[0].first().expect("the curve has points");
        let loud = curves[0].last().expect("the curve has points");
        assert!(quiet.1 > quiet.0 + 1e-3, "the quiet end was not lifted");
        assert!(loud.1 < loud.0 - 1e-3, "the loud end was not compressed");

        // And it rises all the way: a transfer curve that turned back would
        // mean a louder input coming out quieter.
        let mut previous = f32::MIN;
        for (_, y) in &curves[0] {
            assert!(*y >= previous - 1e-4, "the curve fell back");
            previous = *y;
        }
    }

    /// And it stays inside the window whatever it does.
    #[test]
    fn the_curve_stays_in_the_window() {
        let curves = curve_of(&SparkleurParams::default(), Some(0));
        for (x, y) in &curves[0] {
            assert!((0.0..=1.0).contains(x), "{x}");
            assert!((0.0..=1.0).contains(y), "{y}");
        }
        assert_eq!(curves[0].len(), RESOLUTION + 1);
    }
}
