//! The figure, on the accent ground: where every arrival lands, against time.
//!
//! **One axis, and it is both time and distance** (`ui.md`, `REQ-DIO-013`).
//! The direct sound stands at the left edge; each reflection stands where it
//! arrives, as tall as it is loud against the voice. Moving the voice away
//! moves the weight of the picture to the right.
//!
//! **Resolved from the macros, not measured** (`DIO-17`): a window that cannot
//! be set up with the transport stopped is a window nobody sets up. The window
//! keeps its own engine for that — `Engine::set` and `pattern` are the code the
//! audio path is tuned with, so the picture cannot show a room the sound does
//! not have (`v2-plan.md`, decision 18: every macro but Mix and Output moves
//! it).
//!
//! Dragging the figure up and down sets `DEPTH`.

use super::bind::{self, Ctx};
use crate::arrival_levels;
use diorama_core::Engine;
use diorama_core::reflections::{SPAN_MAX_MS, TAPS};
use egui::{Align2, Painter, Rect, Sense, Stroke, Ui, pos2, vec2};
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use nxe_egui::{font, input};
use std::cell::RefCell;

/// The figure's floor, in dB against the voice.
const FLOOR_DB: f32 = -36.0;
const SAMPLE_RATE: f32 = 48_000.0;
const LABEL: f32 = 16.0;

/// What the macros resolve to, for the picture.
pub struct Resolved {
    /// Arrival time as a fraction of `SPAN_MAX_MS`, and level against the
    /// voice in dB.
    pub arrivals: [(f32, f32); TAPS],
    /// Where the damping starts on the direct sound, if it is doing anything.
    pub damping_hz: Option<f32>,
}

thread_local! {
    /// The window's own engine. **Built once per thread**, because building one
    /// allocates its delay lines; `set` on it is cheap and returns early when
    /// nothing moved.
    static ENGINE: RefCell<Engine> = RefCell::new(Engine::new(SAMPLE_RATE));
}

pub fn resolve(params: &crate::params::DioramaParams) -> Resolved {
    ENGINE.with(|engine| {
        let mut engine = engine.borrow_mut();
        engine.set(params.display_macros());
        let pattern = engine.pattern();
        let levels = arrival_levels(&pattern, engine.direct_level());
        Resolved {
            arrivals: std::array::from_fn(|index| (pattern[index].0, levels[index])),
            damping_hz: engine.damping_corners_hz().0,
        }
    })
}

/// The direct-to-reflected ratio in dB, from the arrivals: how far away the
/// voice reads. `None` when there are no reflections at all.
pub fn direct_to_reflected(resolved: &Resolved) -> Option<f32> {
    let energy: f32 = resolved
        .arrivals
        .iter()
        .map(|(_, db)| 10f32.powf(db / 10.0))
        .sum();
    (energy > 1e-9).then(|| -10.0 * energy.log10())
}

pub fn field(
    ui: &mut Ui,
    ctx: &Ctx,
    painter: &Painter,
    rect: Rect,
    ink: &Palette,
    resolved: &Resolved,
) {
    let params = ctx.params();
    let plot = Rect::from_min_max(
        rect.min + vec2(SPACE_3, LABEL + SPACE_2),
        rect.max - vec2(SPACE_3, LABEL),
    );
    let y_of =
        |db: f32| plot.bottom() - plot.height() * ((db - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);

    // Time gridlines every 20 ms.
    let hair = Stroke::new(1.0, ink.line.color());
    for ms in (20..=SPAN_MAX_MS as i32).step_by(20) {
        let x = plot.left() + plot.width() * ms as f32 / SPAN_MAX_MS;
        painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], hair);
        painter.text(
            pos2(x, rect.bottom() - SPACE_1),
            Align2::CENTER_BOTTOM,
            format!("{ms} ms"),
            font::mono(FONT_EYEBROW),
            ink.subtle.color(),
        );
    }
    painter.line_segment([plot.left_bottom(), plot.right_bottom()], hair);

    // The voice, at the left edge.
    painter.line_segment(
        [pos2(plot.left(), y_of(0.0)), plot.left_bottom()],
        Stroke::new(4.0, ink.ink.color()),
    );
    painter.text(
        pos2(plot.left() + SPACE_2, y_of(0.0)),
        Align2::LEFT_TOP,
        "Voice",
        font::sans(FONT_VALUE),
        ink.ink.color(),
    );

    // The arrivals: a stem each, and the envelope across their tops.
    let mut tops = Vec::with_capacity(TAPS);
    for (position, db) in resolved.arrivals {
        if db <= FLOOR_DB {
            continue;
        }
        let x = plot.left() + plot.width() * position.clamp(0.0, 1.0);
        let top = pos2(x, y_of(db));
        painter.line_segment(
            [top, pos2(x, plot.bottom())],
            Stroke::new(2.0, ink.deep.color()),
        );
        painter.circle_filled(top, 2.5, ink.ink.color());
        tops.push(top);
    }
    tops.sort_by(|a, b| a.x.total_cmp(&b.x));
    if tops.len() > 1 {
        painter.add(egui::Shape::line(tops, Stroke::new(1.0, ink.muted.color())));
    }

    // Width: the bar across the top right, as wide as the space spreads.
    let width = bind::plain(ctx, params.width.id());
    let bar = Rect::from_min_size(
        pos2(plot.right() - 96.0, rect.top() + SPACE_1),
        vec2(96.0, 8.0),
    );
    painter.line_segment(
        [
            pos2(bar.center().x - bar.width() / 2.0 * width, bar.center().y),
            pos2(bar.center().x + bar.width() / 2.0 * width, bar.center().y),
        ],
        Stroke::new(3.0, ink.ink.color()),
    );
    painter.text(
        pos2(bar.left() - SPACE_2, bar.center().y),
        Align2::RIGHT_CENTER,
        "Width",
        font::sans(FONT_EYEBROW),
        ink.muted.color(),
    );
    if let Some(hz) = resolved.damping_hz {
        painter.text(
            pos2(plot.left() + 48.0, rect.top() + SPACE_1),
            Align2::LEFT_TOP,
            format!("Damping above {:.1} kHz", hz / 1_000.0),
            font::mono(FONT_VALUE),
            ink.ink.color(),
        );
    }

    let id = params.depth.id();
    let response = ui.interact(rect, ui.id().with("depth"), Sense::click_and_drag());
    let gestures = input::read(&response, bind::normalized(ctx, id));
    bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(0.0)]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::DioramaParams;
    use truce_params::Params;

    /// **Decision 18**: moving `DEPTH` moves the picture with nothing playing
    /// — the voice reads further away.
    #[test]
    fn depth_moves_the_arrivals() {
        let params = DioramaParams::default();
        params.set_plain(params.depth.id(), 0.1);
        let near = direct_to_reflected(&resolve(&params)).unwrap();
        params.set_plain(params.depth.id(), 0.9);
        let far = direct_to_reflected(&resolve(&params)).unwrap();
        assert!(far < near, "near {near} dB, far {far} dB");
    }

    #[test]
    fn no_room_has_no_ratio() {
        let params = DioramaParams::default();
        params.set_plain(params.room.id(), 0.0);
        assert_eq!(direct_to_reflected(&resolve(&params)), None);
    }
}
