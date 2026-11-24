//! A figure that says what is happening right now: an eyebrow over a rule, and
//! one figure under it in the mono face.
//!
//! **Fixed width, right aligned, sign always printed, silence is a dash**
//! (`.agents/rules/ui.md`). A figure that changes length moves everything laid
//! out after it — the sign appearing at zero, a digit appearing at ten, a dash
//! replacing the number — and only a fixed box fixes all three.

use crate::font;
use crate::theme::{
    FONT_EYEBROW, FONT_READOUT, FONT_VALUE, LINE_EYEBROW, LINE_READOUT, Palette, RULE, SPACE_1,
};
use egui::{Align2, Sense, Stroke, Ui, pos2, vec2};

/// The height of a readout cell.
pub const HEIGHT: f32 = LINE_EYEBROW + RULE + SPACE_1 + LINE_READOUT;

/// The figure as text: the sign always, a fixed count of decimals, and a dash
/// for silence.
pub fn figure(value: Option<f32>, decimals: usize) -> String {
    match value {
        Some(value) if value.is_finite() => format!("{value:+.decimals$}"),
        _ => "—".to_owned(),
    }
}

/// `width` is the whole cell; the figure is right aligned against the unit.
pub fn readout(ui: &mut Ui, palette: &Palette, name: &str, figure: &str, unit: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::hover());
    let painter = ui.painter();
    font::tracked(
        painter,
        rect.left_top(),
        Align2::LEFT_TOP,
        name,
        FONT_EYEBROW,
        palette.subtle.color(),
    );
    let rule_y = rect.top() + LINE_EYEBROW + RULE / 2.0;
    painter.line_segment(
        [pos2(rect.left(), rule_y), pos2(rect.right(), rule_y)],
        Stroke::new(RULE, palette.line.color()),
    );
    let baseline = pos2(rect.right(), rect.bottom());
    let unit_rect = painter.text(
        baseline,
        Align2::RIGHT_BOTTOM,
        unit,
        font::sans(FONT_VALUE),
        palette.subtle.color(),
    );
    painter.text(
        pos2(unit_rect.left() - SPACE_1, rect.bottom()),
        Align2::RIGHT_BOTTOM,
        figure,
        font::mono(FONT_READOUT),
        palette.ink.color(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sign_is_always_printed() {
        assert_eq!(figure(Some(0.0), 1), "+0.0");
        assert_eq!(figure(Some(-3.25), 1), "-3.2");
        assert_eq!(figure(Some(12.0), 1), "+12.0");
    }

    #[test]
    fn silence_is_a_dash() {
        assert_eq!(figure(None, 1), "—");
        assert_eq!(figure(Some(f32::NEG_INFINITY), 1), "—");
    }
}
