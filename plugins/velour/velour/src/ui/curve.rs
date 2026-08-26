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

/// The floor under the curve's own peak, so a curve of all zeros — which
/// nothing produces, but a hostile parameter could — divides by something.
const PEAK_FLOOR: f32 = 1e-6;

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
///
/// **Scaled by its own peak, not by a fixed ceiling.** The shaper is normalised
/// for *level* at a quarter of full scale (`nxe_audio::shaper`), so a harder
/// curve returns *less* at full input than a soft one does — it is saturating.
/// Drawn against a fixed ceiling that meant the curve **shrank as `DRIVE` went
/// up**, collapsing to a tenth of the window at the top of the range: a squiggle
/// in the middle of an empty box, which is exactly backwards from what a
/// transfer curve is for.
///
/// Scaling by the peak makes the window show the **shape** — the asymmetry that
/// is `bias`, and the roundness of the ends that is the knee — which is what
/// this window was specified to show and nothing else (`ui.md`). How much is
/// being added is `DRIVE`'s own readout and the upper curve in the figure.
fn curve_of(params: &VelourParams, hovered: Option<usize>) -> Vec<Curve> {
    let shaper = curve_for(&params.display_shape(), shown(params, hovered), 0.0).shaper();

    // The horizontal axis is the input, −1 to +1, so the middle of the window is
    // silence and the curve's asymmetry about it is what `bias` looks like.
    let sampled: Vec<(f32, f32)> = (0..=RESOLUTION)
        .map(|step| {
            let x = step as f32 / RESOLUTION as f32;
            (x, shaper.shape(x * 2.0 - 1.0))
        })
        .collect();

    let peak = sampled
        .iter()
        .map(|(_, output)| output.abs())
        .fold(0.0f32, f32::max)
        .max(PEAK_FLOOR);

    vec![
        sampled
            .into_iter()
            .map(|(x, output)| (x, ((output / peak).clamp(-1.0, 1.0) + 1.0) * 0.5))
            .collect(),
    ]
}

/// The plot is **square**, and the panel is the figure's height.
///
/// `SPK-15` found both of these by looking at Sparkleur in a host, and Velour
/// was left as it was: a curve with no frame reads as something that escaped
/// rather than as a panel of its own, and **a diagonal is only 45° when the two
/// axes are the same length** — in a tall box "below the line is compressed"
/// cannot be read off the shape. Opened beside Sparkleur the difference is the
/// first thing that shows.
const LABEL: f32 = 16.0;
const PLOT: f32 = super::field::HEIGHT - theme::SPACE_4 * 2.0 - theme::SPACE_1 - LABEL;
pub const WIDTH: f32 = PLOT + theme::SPACE_4 * 2.0;
const _: () = assert!(PLOT > 0.0);

pub fn view(cx: &mut Context) {
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
        // **Both sides given, not stretched.** A stretching plot takes the
        // height the row hands it and the width the panel hands it, and those
        // are not the same number.
        .width(Pixels(PLOT))
        .height(Pixels(PLOT));

        // Centred on the label itself, **not with stretch on the column**:
        // `child-left: 1s` and `child-right: 1s` on the parent are two more
        // stretches for the curve's own `Stretch(1.0)` width to share, which
        // left it drawn a third of the width it was given
        // (`.agents/rules/vizia.md`).
        Label::new(
            cx,
            Ui::params.map(|params| NAMES[shown(params, None)].to_string()),
        )
        .class("subtle")
        .width(Stretch(1.0))
        .height(Pixels(LABEL))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));
    })
    // **A frame, like the meter strip's** (`SPK-15`).
    .class("panel")
    .width(Pixels(WIDTH))
    .height(Pixels(super::field::HEIGHT))
    .row_between(Pixels(theme::SPACE_1));
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
    /// (`nxe_audio::shaper`).
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

    /// **The bug this window shipped with**: drawn against a fixed ceiling, a
    /// harder curve came out *smaller*, because the shaper is normalised for
    /// level and a saturating curve returns less at full input. At the top of
    /// the drive range the curve collapsed to a tenth of the window — a squiggle
    /// in an empty box, exactly where there is most shape to look at.
    #[test]
    fn the_curve_fills_the_window_at_every_drive() {
        for drive in [0.0f32, 0.2, 0.4, 0.6, 0.8, 1.0] {
            let params = VelourParams::default();
            // The parameter cannot be moved without a host, so the shape is
            // built directly — the same one `curve_of` resolves.
            let mut shape = params.display_shape();
            shape.drive = drive;
            let shaper = curve_for(&shape, DEFAULT_BAND, 0.0).shaper();

            let sampled: Vec<f32> = (0..=RESOLUTION)
                .map(|step| shaper.shape(step as f32 / RESOLUTION as f32 * 2.0 - 1.0))
                .collect();
            let peak = sampled
                .iter()
                .map(|output| output.abs())
                .fold(0.0f32, f32::max)
                .max(PEAK_FLOOR);
            let drawn: Vec<f32> = sampled
                .iter()
                .map(|output| ((output / peak).clamp(-1.0, 1.0) + 1.0) * 0.5)
                .collect();

            let low = drawn.iter().copied().fold(1.0f32, f32::min);
            let high = drawn.iter().copied().fold(0.0f32, f32::max);

            // One edge is touched exactly and the other falls short by however
            // asymmetric the curve is — **that gap is the reading**, it is what
            // `bias` looks like. So the claim is that the curve spans nearly the
            // whole window, not that it touches both edges.
            assert!(
                low < 0.02 || high > 0.98,
                "drive {drive}: neither edge reached ({low:.3}..{high:.3})"
            );
            // Measured across the range: 0.99 at the bottom down to 0.75 at the
            // top, where the curve saturates one way well before the other. The
            // fixed-ceiling version this replaces reached 0.10.
            assert!(
                high - low > 0.7,
                "drive {drive}: the curve spans only {:.2} of the window",
                high - low
            );
        }
    }
}
