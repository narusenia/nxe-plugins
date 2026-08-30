//! The Band Field: the five bands as regions on a log frequency axis.
//!
//! **The gain is the subject** (`ui.md`). Velour's regions are how much is being
//! added to a band, growing from the floor; a split topology has no such thing —
//! every band is the signal, and what a multiband compressor is asked about is
//! **how far each band is being moved**. So a region's height is that band's
//! `GAIN`, unity is a line across the middle, and what is sounding sits either
//! side of it (`nxe_ui::band::Band::delta`, `SPK-10`).
//!
//! A region's horizontal span comes from `sparkleur_core::crossover::edges_for`,
//! **the function the filters are tuned from** — so `FOCUS` slides the picture
//! and the sound together, and the picture cannot drift.

use super::{Ui, UiEvent};
use crate::analysis::Analysis;
use crate::params::SparkleurParams;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::band::{Band, BandField, BandFieldModifiers, BandGesture};
use nxe_ui::theme;
use sparkleur_core::crossover::{BAND_COUNT, edges_for};

/// The axis the caller owns, because the widget cannot label its own gridlines
/// (`.agents/rules/vizia.md`).
const LOW_HZ: f32 = 20.0;
const HIGH_HZ: f32 = 20_000.0;

const MARKS: [(f32, &str); 6] = [
    (20.0, "20"),
    (100.0, "100"),
    (500.0, "500"),
    (2_000.0, "2k"),
    (5_000.0, "5k"),
    (20_000.0, "20k"),
];

/// The height of the figure. It is what the plugin *is*, so it gets the room.
///
/// **The same number in every plugin** (`SPK-19`). Velour tuned it to 236 for a
/// figure whose regions grow from the floor, Sparkleur to 176 for one whose
/// content gathers around a centre line — each right on its own, and together
/// they meant two windows on the same grid whose sections stopped lining up
/// where the figures ended. Opened side by side that is the first thing that
/// reads as sloppy. Two hundred is the shared number: Velour's regions still
/// have room to grow into, and Sparkleur is no longer mostly black.
pub const HEIGHT: f32 = 200.0;

/// Where "no change" sits: the middle, because `GAIN` is bipolar.
const UNITY: f32 = 0.5;

/// How many dB the full height of a region covers.
///
/// **The `GAIN` range**, so a decibel of trim and a decibel of compression are
/// the same distance on screen. A figure where the two were drawn at different
/// scales would be unreadable exactly when it matters — while both are moving.
const SPAN_DB: f32 = 24.0;

/// Hz onto `0..=1` across the view, logarithmically.
fn axis_x(hz: f32) -> f32 {
    (hz.max(LOW_HZ) / LOW_HZ).log10() / (HIGH_HZ / LOW_HZ).log10()
}

/// A gain in dB as a share of a region's height.
fn as_share(decibels: f32) -> f32 {
    (decibels / SPAN_DB).clamp(-1.0, 1.0)
}

/// The five regions, from the parameters and from what the engine is doing.
///
/// **The analysis is read here rather than copied into the model.** A region
/// carries what it is set to and what it is doing in one value, and a lens can
/// only map one field (`.agents/rules/vizia.md`) — so this map reads the handoff
/// directly. Any change to the model re-evaluates it, and the heartbeat is a
/// change to the model thirty times a second.
pub(crate) fn bands_of(params: &SparkleurParams, host_rate: f32, analysis: &Analysis) -> Vec<Band> {
    let gains = analysis.gains.read();
    let edges = edges_for(params.focus.value(), host_rate);
    let levels = [
        params.gain_sub.unmodulated_normalized_value(),
        params.gain_body.unmodulated_normalized_value(),
        params.gain_mid.unmodulated_normalized_value(),
        params.gain_pres.unmodulated_normalized_value(),
        params.gain_air.unmodulated_normalized_value(),
    ];
    let soloed = [
        params.solo_sub.value(),
        params.solo_body.value(),
        params.solo_mid.value(),
        params.solo_pres.value(),
        params.solo_air.value(),
    ];

    (0..BAND_COUNT)
        .map(|index| {
            // The outer bands run to the ends of the axis: the crossover has no
            // boundary below the first or above the last, and drawing one would
            // invent a band edge that does not exist.
            let low = if index == 0 { LOW_HZ } else { edges[index - 1] };
            let high = if index == BAND_COUNT - 1 {
                HIGH_HZ
            } else {
                edges[index]
            };

            Band {
                low: axis_x(low),
                high: axis_x(high),
                level: levels[index],
                // What is actually being applied, on the same scale as the
                // trim below it. De-Harsh is already in this number
                // (`sparkleur_core::Engine::gains_db`).
                delta: as_share(gains[index]),
                // A step along the accent per band, deep to bright, so the five
                // are distinguishable without adding a hue
                // (`crates/nxe-ui/README.md`).
                tint: index as f32 / (BAND_COUNT - 1) as f32,
                soloed: soloed[index],
            }
        })
        .collect()
}

pub fn view(cx: &mut Context) {
    let trims: Vec<ParamWidgetBase> = vec![
        ParamWidgetBase::new(cx, Ui::params, |params| &params.gain_sub),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.gain_body),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.gain_mid),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.gain_pres),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.gain_air),
    ];
    let focus = ParamWidgetBase::new(cx, Ui::params, |params| &params.focus);

    VStack::new(cx, |cx| {
        BandField::new(
            cx,
            Ui::bands,
            // What came in. **One curve, not Velour's two** — there is no
            // separable added layer here, and the gains are what says what the
            // plugin did (`REQ-SPK-018`).
            Ui::dry,
            Ui::wet,
            MARKS.iter().map(|(hz, _)| axis_x(*hz)).collect(),
            move |cx, gesture| {
                // **Rebuild the regions now, not on the next heartbeat.**
                // `Ui::bands` is written by the heartbeat, which is right for
                // what the audio thread publishes and wrong for a drag: the
                // fader would move under the pointer and the region behind it
                // would follow up to an interval later. The write below has
                // already happened by the time this is dispatched, so the
                // refresh sees the new value.
                cx.emit(UiEvent::Poll);

                let index = match gesture {
                    BandGesture::Begin(index)
                    | BandGesture::End(index)
                    | BandGesture::Reset(index)
                    | BandGesture::Change { index, .. } => index,
                    BandGesture::Hover(over) => {
                        cx.emit(UiEvent::Hover(over));
                        return;
                    }
                    // The rail is `FOCUS`. It writes a plain normalized value,
                    // which is what lets the widget move its own copy first —
                    // there is no other parameter to clamp it against
                    // (`.agents/rules/vizia.md`).
                    BandGesture::FocusBegin => {
                        focus.begin_set_parameter(cx);
                        return;
                    }
                    BandGesture::FocusChange(value) => {
                        focus.set_normalized_value(cx, value);
                        return;
                    }
                    BandGesture::FocusEnd => {
                        focus.end_set_parameter(cx);
                        return;
                    }
                    BandGesture::FocusReset => {
                        focus.begin_set_parameter(cx);
                        focus.set_normalized_value(cx, focus.default_normalized_value());
                        focus.end_set_parameter(cx);
                        return;
                    }
                };

                let Some(trim) = trims.get(index) else {
                    return;
                };
                match gesture {
                    BandGesture::Begin(_) => trim.begin_set_parameter(cx),
                    BandGesture::End(_) => trim.end_set_parameter(cx),
                    BandGesture::Change { level, .. } => trim.set_normalized_value(cx, level),
                    BandGesture::Reset(_) => {
                        trim.begin_set_parameter(cx);
                        trim.set_normalized_value(cx, trim.default_normalized_value());
                        trim.end_set_parameter(cx);
                    }
                    _ => {}
                }
            },
        )
        // Whatever the model says is being pointed at — a region here, or a row
        // in the Advanced table (`SPK-15`) — is marked.
        .highlight(Ui::hovered)
        // Wiring this is what makes the rail live (`crates/nxe-ui/README.md`).
        .focus(nxe_plug_ui::value_of(Ui::params, |params| &params.focus))
        // And this is what draws the line everything is read against.
        .unity(UNITY)
        .height(Stretch(1.0))
        .width(Stretch(1.0));

        // The labels are placed with the same mapping the widget was given.
        HStack::new(cx, |cx| {
            for (index, (hz, text)) in MARKS.iter().enumerate() {
                let label = Label::new(cx, *text)
                    .class("subtle")
                    .class("ink-muted")
                    .position_type(PositionType::SelfDirected);

                // **The last one is hung off the right edge**, not placed at
                // 100 % of the width. A label positioned at the far edge starts
                // there and runs past it, so "20k" was drawn as "20" with the
                // rest outside the box — the same absence of placement logic
                // tooltips have (`.agents/rules/vizia.md`).
                if index == MARKS.len() - 1 {
                    label.left(Stretch(1.0)).right(Pixels(0.0));
                } else {
                    label.left(Percentage(axis_x(*hz) * 100.0));
                }
            }
        })
        .height(Pixels(14.0))
        .width(Stretch(1.0));
    })
    .height(Pixels(HEIGHT))
    .width(Stretch(1.0))
    .row_between(Pixels(theme::SPACE_1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_axis_covers_hearing_and_clamps_below_it() {
        assert_eq!(axis_x(LOW_HZ), 0.0);
        assert!((axis_x(HIGH_HZ) - 1.0).abs() < 1e-6);
        assert_eq!(axis_x(1.0), 0.0);
        let decade = axis_x(2_000.0) - axis_x(200.0);
        assert!((decade - (axis_x(200.0) - axis_x(20.0))).abs() < 1e-6);
    }

    /// The five regions have to come out in the engine's order and tile the
    /// axis, or a drag moves the wrong band's trim.
    #[test]
    fn the_regions_are_in_the_engine_order_and_meet() {
        let params = SparkleurParams::default();
        let bands = bands_of(&params, 48_000.0, &Analysis::default());
        assert_eq!(bands.len(), BAND_COUNT);

        assert_eq!(
            bands[0].low, 0.0,
            "the first region does not start the axis"
        );
        assert!(
            (bands[BAND_COUNT - 1].high - 1.0).abs() < 1e-6,
            "the last region does not end the axis"
        );
        for pair in bands.windows(2) {
            assert!(
                (pair[0].high - pair[1].low).abs() < 1e-6,
                "the regions do not meet: {} against {}",
                pair[0].high,
                pair[1].low
            );
        }
        assert_eq!(bands[0].tint, 0.0);
        assert_eq!(bands[BAND_COUNT - 1].tint, 1.0);
    }

    /// **`FOCUS` moves the picture and the filters together** (`SPK-13`), which
    /// is what taking the edges from the crossover's own function buys.
    #[test]
    fn focus_slides_the_regions() {
        let params = SparkleurParams::default();
        let centred = bands_of(&params, 48_000.0, &Analysis::default());

        // The parameter cannot be moved without a host, so the mapping is
        // checked where it is: both come from `edges_for`.
        let opened = edges_for(1.0, 48_000.0);
        for (index, edge) in opened.iter().enumerate() {
            assert!(
                axis_x(*edge) > centred[index].high,
                "boundary {index} did not move right"
            );
        }
    }

    /// A trim at rest is the unity line, and the region's height is the trim —
    /// so the default picture is five regions meeting the line (`ui.md`).
    #[test]
    fn a_trim_at_rest_sits_on_the_unity_line() {
        let bands = bands_of(&SparkleurParams::default(), 48_000.0, &Analysis::default());
        for band in &bands {
            assert!(
                (band.level - UNITY).abs() < 1e-6,
                "a rested trim drew at {}",
                band.level
            );
            assert_eq!(band.delta, 0.0, "an idle band was moved");
            assert_eq!(band.live(), UNITY);
        }
    }

    /// **Up is up** (`SPK-13`). A decibel of compression and a decibel of trim
    /// are the same distance, and the sign is the direction.
    #[test]
    fn the_share_of_the_height_reads_the_way_the_gain_does() {
        assert_eq!(as_share(0.0), 0.0);
        assert!(as_share(6.0) > 0.0);
        assert!(as_share(-6.0) < 0.0);
        assert!((as_share(12.0) - 0.5).abs() < 1e-6);
        // And a gain past the region's own scale is held at the top rather than
        // drawn outside it.
        assert_eq!(as_share(48.0), 1.0);
        assert_eq!(as_share(-48.0), -1.0);
    }
}
