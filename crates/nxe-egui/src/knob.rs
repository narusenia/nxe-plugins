//! A rotary control with its name above and its figure below.
//!
//! Takes a normalized value and the text to print; hands back what the pointer
//! did ([`Control`]). It never holds the value itself.

use crate::entry::value_entry;
use crate::font;
use crate::input::{self, Gestures};
use crate::theme::{FONT_LABEL, LINE_LABEL, LINE_VALUE, Palette};
use egui::{Align2, Pos2, Sense, Shape, Stroke, Ui, pos2, vec2};
use std::f32::consts::FRAC_PI_2;

/// Half the sweep, in degrees from straight up.
const ANGLE: f32 = 135.0;
const ARC_WIDTH: f32 = 3.0;

/// What a value control reports in one frame.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Control {
    pub gestures: Gestures,
    /// A typed value, still a string — parsing is the caller's.
    pub typed: Option<String>,
}

/// The height a knob of `size` takes with its name and figure.
pub fn block_height(size: f32) -> f32 {
    LINE_LABEL + size + LINE_VALUE
}

/// The points of an arc, zero straight up and angles clockwise.
fn arc(centre: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    let steps = ((to - from).abs() * radius / 2.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|step| {
            let angle = from + (to - from) * step as f32 / steps as f32;
            centre + radius * vec2(angle.cos(), angle.sin())
        })
        .collect()
}

pub fn knob(
    ui: &mut Ui,
    palette: &Palette,
    name: &str,
    value: f32,
    text: &str,
    size: f32,
) -> Control {
    let value = value.clamp(0.0, 1.0);
    // The id comes from the allocation, so two knobs with one name (a column of
    // "Gain"s) are still two knobs.
    let (rect, block) =
        ui.allocate_exact_size(vec2(size.max(56.0), block_height(size)), Sense::hover());
    let id = block.id.with("dial");
    let dial = egui::Rect::from_center_size(
        pos2(rect.center().x, rect.top() + LINE_LABEL + size / 2.0),
        vec2(size, size),
    );
    let response = ui.interact(dial, id, Sense::click_and_drag());
    let gestures = input::read(&response, value);
    let shown = gestures.change.unwrap_or(value);

    let painter = ui.painter();
    painter.text(
        pos2(rect.center().x, rect.top()),
        Align2::CENTER_TOP,
        name,
        font::sans(FONT_LABEL),
        palette.muted.color(),
    );

    let centre = dial.center();
    let radius = size / 2.0 - ARC_WIDTH;
    let start = (-ANGLE).to_radians() - FRAC_PI_2;
    let end = ANGLE.to_radians() - FRAC_PI_2;
    let angle = start + (end - start) * shown;
    painter.add(Shape::line(
        arc(centre, radius, start, end),
        Stroke::new(ARC_WIDTH, palette.track.color()),
    ));
    if shown > 0.0 {
        painter.add(Shape::line(
            arc(centre, radius, start, angle),
            Stroke::new(ARC_WIDTH, palette.accent.color()),
        ));
    }
    let direction = vec2(angle.cos(), angle.sin());
    painter.line_segment(
        [
            centre + direction * radius * 0.35,
            centre + direction * (radius - ARC_WIDTH),
        ],
        Stroke::new(ARC_WIDTH * 0.8, palette.ink.color()),
    );

    let figure = egui::Rect::from_min_max(pos2(rect.left(), dial.bottom()), rect.right_bottom());
    let typed = value_entry(ui, palette, id.with("entry"), figure, text, gestures.edit);
    Control { gestures, typed }
}
