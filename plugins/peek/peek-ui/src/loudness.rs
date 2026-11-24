//! Loudness: the integrated figure large, and the short-term reading's
//! history against the target.

use crate::WARN;
use egui::{Align2, Rect, Shape, Stroke, Ui, pos2};
use nxe_egui::font;
use nxe_egui::readout::figure as text_of;
use nxe_egui::theme::{FONT_EYEBROW, FONT_SUBJECT, FONT_VALUE, Palette, SPACE_2};

/// The history covers 60 s, a point every half second.
const HISTORY: usize = 120;
const EVERY: f64 = 0.5;
const LOW: f32 = -48.0;
const HIGH: f32 = 0.0;

/// `values` as `LoudnessReadings` publishes them.
pub fn figure(ui: &Ui, palette: &Palette, rect: Rect, values: [f32; 5], target: f32, big: bool) {
    let [_, short_term, integrated, _, _] = values;
    let painter = ui.painter_at(rect);
    let number = Rect::from_min_size(
        rect.min,
        egui::vec2(
            if big {
                rect.width() * 0.42
            } else {
                rect.width()
            },
            rect.height() * if big { 1.0 } else { 0.42 },
        ),
    );
    font::tracked(
        &painter,
        number.left_top(),
        Align2::LEFT_TOP,
        "Integrated",
        FONT_EYEBROW,
        palette.subtle.color(),
    );
    let shown = integrated.is_finite().then_some(integrated);
    let over = shown.is_some_and(|lufs| lufs > target + 1.0);
    let drawn = painter.text(
        pos2(number.left(), number.top() + 14.0),
        Align2::LEFT_TOP,
        text_of(shown, 1),
        font::mono(if big {
            FONT_SUBJECT
        } else {
            FONT_SUBJECT * 0.6
        }),
        if over { WARN } else { palette.ink }.color(),
    );
    painter.text(
        pos2(drawn.right() + 4.0, drawn.bottom() - 4.0),
        Align2::LEFT_BOTTOM,
        "LUFS",
        font::sans(FONT_VALUE),
        palette.subtle.color(),
    );
    painter.text(
        pos2(number.left(), drawn.bottom() + SPACE_2),
        Align2::LEFT_TOP,
        format!(
            "target {target:+.0}  ·  {}",
            text_of(shown.map(|lufs| lufs - target), 1)
        ),
        font::mono(FONT_VALUE),
        palette.muted.color(),
    );

    // The history, kept by the window.
    let id = ui.id().with("peek-loudness-history");
    let now = ui.input(|input| input.time);
    let (mut points, mut last): (Vec<f32>, f64) = ui
        .data(|data| data.get_temp(id))
        .unwrap_or((Vec::new(), 0.0));
    if now - last >= EVERY {
        last = now;
        points.push(short_term);
        if points.len() > HISTORY {
            points.remove(0);
        }
    }
    ui.data_mut(|data| data.insert_temp(id, (points.clone(), last)));

    let graph = if big {
        Rect::from_min_max(pos2(number.right() + SPACE_2, rect.top()), rect.max)
    } else {
        Rect::from_min_max(pos2(rect.left(), number.bottom() + SPACE_2), rect.max)
    };
    let y_of =
        |lufs: f32| graph.bottom() - ((lufs - LOW) / (HIGH - LOW)).clamp(0.0, 1.0) * graph.height();
    painter.rect_filled(graph, 0.0, palette.track.at(0.5).color());
    for lufs in [-12.0, -24.0, -36.0] {
        let y = y_of(lufs);
        painter.line_segment(
            [pos2(graph.left(), y), pos2(graph.right(), y)],
            Stroke::new(1.0, palette.line.color()),
        );
        painter.text(
            pos2(graph.left() + 2.0, y - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{lufs:.0}"),
            font::mono(FONT_VALUE),
            palette.subtle.color(),
        );
    }
    painter.line_segment(
        [
            pos2(graph.left(), y_of(target)),
            pos2(graph.right(), y_of(target)),
        ],
        Stroke::new(1.0, WARN.at(0.8).color()),
    );
    let step = graph.width() / (HISTORY - 1) as f32;
    let start = HISTORY - points.len();
    let line: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(_, lufs)| lufs.is_finite())
        .map(|(index, lufs)| pos2(graph.left() + (start + index) as f32 * step, y_of(*lufs)))
        .collect();
    if line.len() >= 2 {
        painter.add(Shape::line(line, Stroke::new(1.5, palette.accent.color())));
    }
}
