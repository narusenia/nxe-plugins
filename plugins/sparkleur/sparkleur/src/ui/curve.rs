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

pub fn view(cx: &mut Context, width: f32) {
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
        .height(Stretch(1.0))
        .width(Stretch(1.0));

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
        .width(Stretch(1.0))
        .child_left(Stretch(1.0))
        .child_right(Stretch(1.0));
    })
    .width(Pixels(width))
    .height(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1));
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
