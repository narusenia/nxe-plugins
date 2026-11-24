//! A thin horizontal value control, for rows and tables of them.
//!
//! The same gesture as the knob — **a vertical drag**, even on a horizontal
//! bar (`crate::input`). Unipolar fills from the left; bipolar fills from the
//! centre, for a value that rests at `0.5`.

use crate::input::{self, Gestures};
use crate::theme::Palette;
use egui::{Rect, Sense, Ui, Vec2, pos2};

/// Where the fill starts and ends, as fractions of the width.
pub fn span(value: f32, bipolar: bool) -> (f32, f32) {
    let value = value.clamp(0.0, 1.0);
    if bipolar {
        (value.min(0.5), value.max(0.5))
    } else {
        (0.0, value)
    }
}

/// `size` is the whole interactive area; the bar is drawn through its middle
/// at `thickness`.
pub fn bar(
    ui: &mut Ui,
    palette: &Palette,
    value: f32,
    bipolar: bool,
    size: Vec2,
    thickness: f32,
) -> Gestures {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let gestures = input::read(&response, value);
    let shown = gestures.change.unwrap_or(value);
    let track = Rect::from_center_size(rect.center(), egui::vec2(rect.width(), thickness));
    let painter = ui.painter();
    painter.rect_filled(track, 0.0, palette.track.color());
    let (from, to) = span(shown, bipolar);
    let fill = Rect::from_min_max(
        pos2(track.left() + track.width() * from, track.top()),
        pos2(track.left() + track.width() * to, track.bottom()),
    );
    painter.rect_filled(fill, 0.0, palette.accent.color());
    if bipolar {
        let x = track.center().x;
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            egui::Stroke::new(1.0, palette.line.color()),
        );
    }
    gestures
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bipolar_bar_at_rest_is_empty() {
        let (from, to) = span(0.5, true);
        assert_eq!(from, to);
    }

    #[test]
    fn a_bipolar_bar_fills_toward_the_value() {
        assert_eq!(span(0.2, true), (0.2, 0.5));
        assert_eq!(span(0.8, true), (0.5, 0.8));
        assert_eq!(span(0.3, false), (0.0, 0.3));
    }
}
