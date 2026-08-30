//! The Band Field: the three generators as regions on a log frequency axis.
//!
//! **The reason the parallel topology was chosen shows up here** (`ui.md`): the
//! dry spectrum and the spectrum of what is being added to it are two separate
//! layers, drawn at the same time. A crossover design has only "before" and
//! "after", and the difference between them has to be subtracted by eye.
//!
//! A region's horizontal span is `velour_core::Generator::input_range`, which is
//! the function the filters are tuned from — so `FOCUS` slides the picture and
//! the sound together, and the picture cannot drift.

use super::{Ui, UiEvent};
use crate::analysis::Analysis;
use crate::params::VelourParams;
use nih_plug::prelude::Param;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nxe_ui::band::{Band, BandField, BandFieldModifiers, BandGesture};
use nxe_ui::theme;
use velour_core::BAND_COUNT;
use velour_core::bands::{BANDS, Generator};
use velour_core::guard::MAX_REDUCTION_DB;

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

/// Hz onto `0..=1` across the view, logarithmically.
fn axis_x(hz: f32) -> f32 {
    (hz.max(LOW_HZ) / LOW_HZ).log10() / (HIGH_HZ / LOW_HZ).log10()
}

/// A guard's reduction in dB as the fraction of its allowed range. A region
/// sinks by this much, so **the amount it sinks by is the reduction itself**
/// (`REQ-VEL-006`).
fn reduction_fraction(decibels: f32) -> f32 {
    (-decibels / MAX_REDUCTION_DB).clamp(0.0, 1.0)
}

/// Which guard watches which band. **BODY has none** — a muddy low end is a
/// `FOCUS` and fader question, and a detector there would only add a way for it
/// to be unclear whether the plugin is working (`velour_core::engine`).
fn reduction_of(index: usize, guards: &[f32; 2]) -> f32 {
    match index {
        0 => 0.0,
        1 => reduction_fraction(guards[0]),
        _ => reduction_fraction(guards[1]),
    }
}

/// The three regions, from the parameters and from what the guards are doing.
///
/// **The guards are read here rather than copied into the model.** A region
/// carries its reduction in the same value as its level, and a lens can only map
/// one field (`.agents/rules/vizia.md`) — so this map reads the handoff
/// directly. Any change to the model re-evaluates it, and the heartbeat is a
/// change to the model thirty times a second.
pub(crate) fn bands_of(params: &VelourParams, host_rate: f32, analysis: &Analysis) -> Vec<Band> {
    let guards = analysis.guards.read();
    let focus = params.focus.value();
    let levels = [
        params.body.unmodulated_normalized_value(),
        params.presence.unmodulated_normalized_value(),
        params.air.unmodulated_normalized_value(),
    ];
    let soloed = [
        params.solo_body.value(),
        params.solo_presence.value(),
        params.solo_air.value(),
    ];

    (0..BAND_COUNT)
        .map(|index| {
            let (low, high) = Generator::input_range(BANDS[index], focus, host_rate);
            Band {
                low: axis_x(low),
                high: axis_x(high),
                level: levels[index],
                // **Negative, and scaled by the level.** `Band::delta` is a
                // share of the whole height and signed either way (`SPK-10`);
                // Velour only ever pulls down, and it pulls a fraction of what
                // the band was set to, so the picture is what it always was.
                delta: -reduction_of(index, &guards) * levels[index],
                // A step along the accent per band, deep to bright, so the three
                // are distinguishable without adding a hue
                // (`crates/nxe-ui/README.md`).
                tint: index as f32 / (BAND_COUNT - 1) as f32,
                soloed: soloed[index],
            }
        })
        .collect()
}

pub fn view(cx: &mut Context) {
    let faders: Vec<ParamWidgetBase> = vec![
        ParamWidgetBase::new(cx, Ui::params, |params| &params.body),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.presence),
        ParamWidgetBase::new(cx, Ui::params, |params| &params.air),
    ];
    let focus = ParamWidgetBase::new(cx, Ui::params, |params| &params.focus);

    VStack::new(cx, |cx| {
        BandField::new(
            cx,
            Ui::bands,
            // What came in, and the harmonics being added to it. **Both at
            // once**, which is what the parallel topology bought (`ui.md`).
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

                let Some(fader) = faders.get(index) else {
                    return;
                };
                match gesture {
                    BandGesture::Begin(_) => fader.begin_set_parameter(cx),
                    BandGesture::End(_) => fader.end_set_parameter(cx),
                    BandGesture::Change { level, .. } => fader.set_normalized_value(cx, level),
                    BandGesture::Reset(_) => {
                        fader.begin_set_parameter(cx);
                        fader.set_normalized_value(cx, fader.default_normalized_value());
                        fader.end_set_parameter(cx);
                    }
                    _ => {}
                }
            },
        )
        // Whatever the model says is being pointed at — a region here, or a row
        // in the Advanced table (`VEL-14`) — is marked.
        .highlight(Ui::hovered)
        // Wiring this is what makes the rail live (`crates/nxe-ui/README.md`).
        .focus(nxe_plug_ui::value_of(Ui::params, |params| &params.focus))
        .height(Stretch(1.0))
        .width(Stretch(1.0));

        // The labels are placed with the same mapping the widget was given.
        HStack::new(cx, |cx| {
            for (index, (hz, text)) in MARKS.iter().enumerate() {
                let label = Label::new(cx, *text)
                    .class("subtle")
                    .position_type(PositionType::SelfDirected);

                // **An absolutely-positioned label at the far edge runs off
                // it.** `left: 100%` puts the label's *start* at the right-hand
                // edge, so `20k` grew out of the box — it read as `20` with the
                // curve panel beside it, and as `20` clipped without one. Hang
                // the last one off the other side instead (`SPK-15` fixed the
                // same three lines in Sparkleur; this is the copy the backlog
                // was holding).
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
        // Below the axis is the left edge rather than a negative position.
        assert_eq!(axis_x(1.0), 0.0);
        // And the decades are evenly spaced, which is what makes the marks
        // readable.
        let decade = axis_x(2_000.0) - axis_x(200.0);
        assert!((decade - (axis_x(200.0) - axis_x(20.0))).abs() < 1e-6);
    }

    /// **The picture did not change when `Band::reduction` became a signed
    /// `delta`** (`SPK-10`). The widget used to compute `level · (1 − r)`; it
    /// now computes `level + delta`, and this is the call site that has to make
    /// the two the same number.
    #[test]
    fn the_signed_delta_draws_what_the_reduction_used_to() {
        for level in [0.0f32, 0.3, 0.8, 1.0] {
            for reduction in [0.0f32, 0.25, 0.5, 1.0] {
                let band = Band {
                    level,
                    delta: -reduction * level,
                    ..Band::default()
                };
                let before = level * (1.0 - reduction);
                assert!(
                    (band.live() - before).abs() < 1e-6,
                    "{level} / {reduction}: {} against {before}",
                    band.live()
                );
            }
        }
    }

    /// The three regions have to come out in the engine's order, or a fader
    /// drags the wrong band.
    #[test]
    fn the_regions_are_in_the_engine_order() {
        let params = VelourParams::default();
        let bands = bands_of(&params, 48_000.0, &Analysis::default());
        assert_eq!(bands.len(), BAND_COUNT);
        for pair in bands.windows(2) {
            assert!(
                pair[0].low < pair[1].low,
                "the regions are not left to right"
            );
        }
        // Deep to bright across the three.
        assert_eq!(bands[0].tint, 0.0);
        assert_eq!(bands[BAND_COUNT - 1].tint, 1.0);
    }

    /// `FOCUS` slides them, which is the one thing a number cannot show
    /// (`REQ-VEL-002`).
    #[test]
    fn focus_slides_every_region() {
        let params = VelourParams::default();
        let resting = bands_of(&params, 48_000.0, &Analysis::default());

        // The parameter cannot be moved without a host, so the mapping is
        // checked at the source instead.
        for index in 0..BAND_COUNT {
            let (low, _) = Generator::input_range(BANDS[index], 1.0, 48_000.0);
            assert!(
                axis_x(low) > resting[index].low,
                "band {index} did not move up"
            );
        }
    }

    /// The figure sinks by exactly what the guard is pulling, and BODY never
    /// sinks because nothing guards it.
    #[test]
    fn a_region_sinks_by_the_reduction() {
        assert_eq!(reduction_fraction(0.0), 0.0);
        assert_eq!(reduction_fraction(-MAX_REDUCTION_DB), 1.0);
        assert!((reduction_fraction(-MAX_REDUCTION_DB / 2.0) - 0.5).abs() < 1e-6);
        // A positive number would be a guard pushing, which cannot happen.
        assert_eq!(reduction_fraction(3.0), 0.0);

        let pulling = [-9.0, -18.0];
        assert_eq!(reduction_of(0, &pulling), 0.0);
        assert_eq!(reduction_of(1, &pulling), reduction_fraction(-9.0));
        assert_eq!(reduction_of(2, &pulling), 1.0);
    }
}
