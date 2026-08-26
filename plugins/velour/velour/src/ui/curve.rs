//! The transfer-curve window: one band's curve, read only.
//!
//! **Which band**: the soloed one, else the one being pointed at, else PRESENCE
//! (`ui.md`). PRESENCE is the fallback because it is this plugin's main ground.
//!
//! **Read only.** A grabbable curve would have to write back into two
//! parameters — the bias and the drive — and from a dragged point there is no
//! way to say which of them moved.
//!
//! The curve comes from `velour_core::engine::curve_for`, the same function the
//! audio path is tuned from, so the window cannot show a curve the sound does
//! not have.

use super::Ui;
use crate::params::VelourParams;
use nih_plug_vizia::vizia::prelude::*;
use nxe_ui::curve::{Curve, CurveView};
use nxe_ui::theme;
use velour_core::BAND_COUNT;
use velour_core::engine::curve_for;

/// How many points the curve is drawn with. Enough that the knee reads as a
/// curve rather than a corner.
const RESOLUTION: usize = 96;

/// The output the window's top and bottom stand for.
///
/// **Not 1.0.** The curve is normalised for level at a quarter of full scale
/// (`velour_core::shaper::PROBE_AMPLITUDE`), so a hard setting drawn against ±1
/// would run off the top of the window and read as a flat clip.
const CEILING: f32 = 2.0;

const NAMES: [&str; BAND_COUNT] = ["BODY", "PRESENCE", "AIR"];

/// The fallback: the band this plugin is mostly about.
const DEFAULT_BAND: usize = 1;

/// Which band the window is showing. **One value out of three inputs**, so it is
/// computed rather than mapped from a lens (`.agents/rules/vizia.md`).
fn shown(params: &VelourParams, hovered: Option<usize>) -> usize {
    let soloed = [
        params.solo_body.value(),
        params.solo_presence.value(),
        params.solo_air.value(),
    ];
    if let Some(index) = soloed.iter().position(|on| *on) {
        return index;
    }
    hovered
        .filter(|index| *index < BAND_COUNT)
        .unwrap_or(DEFAULT_BAND)
}

/// The curve, as points across the window.
fn curve_of(params: &VelourParams, hovered: Option<usize>) -> Vec<Curve> {
    let shaper = curve_for(&params.display_shape(), shown(params, hovered), 0.0).shaper();

    vec![
        (0..=RESOLUTION)
            .map(|step| {
                let x = step as f32 / RESOLUTION as f32;
                // The horizontal axis is the input, −1 to +1, so the middle of
                // the window is silence and the asymmetry of the curve about it
                // is what `bias` looks like.
                let input = x * 2.0 - 1.0;
                let output = (shaper.shape(input) / CEILING).clamp(-1.0, 1.0);
                (x, (output + 1.0) * 0.5)
            })
            .collect(),
    ]
}

pub fn view(cx: &mut Context, width: f32) {
    VStack::new(cx, |cx| {
        CurveView::new(
            cx,
            // Recomputed whenever a parameter or the hover moves, which is what
            // makes the window follow `TEXTURE`.
            Ui::params.map(|params| curve_of(params, None)),
            // No bands, no handles, no analysis behind it: `CurveView` was not
            // changed to serve this, and it did not need to be (`ui.md`).
            Vec::<nxe_ui::curve::Span>::new(),
            Vec::<nxe_ui::curve::Grip>::new(),
            // One gridline, down the middle: the input's zero, which is what the
            // asymmetry is read against.
            vec![0.5],
            |_cx, _index, _gesture| {},
        )
        .height(Stretch(1.0))
        .width(Stretch(1.0));

        Label::new(
            cx,
            Ui::params.map(|params| NAMES[shown(params, None)].to_string()),
        )
        .class("subtle");
    })
    .width(Pixels(width))
    .height(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_solo_wins_over_a_hover_and_presence_is_the_fallback() {
        let params = VelourParams::default();
        assert_eq!(shown(&params, None), DEFAULT_BAND);
        assert_eq!(shown(&params, Some(0)), 0);
        // A stale index from a hover that outlived its region falls back rather
        // than indexing past the end.
        assert_eq!(shown(&params, Some(99)), DEFAULT_BAND);
    }

    /// The curve has to fit the window and pass through the middle: an input of
    /// zero is an output of zero, because the bias offset is subtracted
    /// (`velour_core::shaper`).
    #[test]
    fn the_curve_fits_the_window() {
        let params = VelourParams::default();
        let curves = curve_of(&params, None);
        let points = &curves[0];

        assert_eq!(points.len(), RESOLUTION + 1);
        for (x, y) in points {
            assert!((0.0..=1.0).contains(x), "x {x}");
            assert!((0.0..=1.0).contains(y), "y {y}");
        }
        let middle = points[RESOLUTION / 2];
        assert!((middle.0 - 0.5).abs() < 1e-6);
        assert!(
            (middle.1 - 0.5).abs() < 1e-3,
            "silence in is not silence out"
        );
        // And it rises left to right, or it is not a transfer curve.
        assert!(points[0].1 < middle.1);
        assert!(points[RESOLUTION].1 > middle.1);
    }
}
