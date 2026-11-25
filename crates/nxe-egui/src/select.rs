//! One of several choices, as a word and the choice with a caret —
//! `Bands 20 ▾`. Corner marks close round the choice while the pointer is on
//! it or its list is open, and only then. A click opens the list under it. The box
//! is as wide as the longest choice, so it does not move as the choice
//! changes. As tall as a [`crate::segment`].

use crate::font;
use crate::segment::HEIGHT;
use crate::theme::{FONT_LABEL, Palette, SPACE_1, SPACE_2};
use egui::{
    Align2, Frame, Margin, Popup, PopupCloseBehavior, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2,
};

/// How long a corner mark's arms are.
const CORNER: f32 = 4.0;
/// The caret's width.
const CARET: f32 = 7.0;

/// Returns the index picked this frame, if any.
pub fn select(
    ui: &mut Ui,
    palette: &Palette,
    label: &str,
    choices: &[&str],
    selected: usize,
) -> Option<usize> {
    let painter = ui.painter();
    let word = painter.layout_no_wrap(
        label.to_owned(),
        font::sans(FONT_LABEL),
        palette.muted.color(),
    );
    let widest = choices
        .iter()
        .map(|choice| {
            painter
                .layout_no_wrap(
                    (*choice).to_owned(),
                    font::sans(FONT_LABEL),
                    palette.ink.color(),
                )
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let box_width = SPACE_2 + widest + SPACE_2 + CARET + SPACE_2;
    let gap = SPACE_1 * 1.5;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(word.size().x + gap + box_width, HEIGHT),
        Sense::click(),
    );
    let field = Rect::from_min_max(pos2(rect.right() - box_width, rect.top()), rect.max);
    let painter = ui.painter();
    painter.galley(
        pos2(rect.left(), rect.center().y - word.size().y * 0.5),
        word,
        palette.muted.color(),
    );
    let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
    let active = response.hovered() || open;
    let ink = if active { palette.ink } else { palette.accent };
    if active {
        corners(painter, field, ink.color());
    }
    painter.text(
        pos2(field.left() + SPACE_2, field.center().y),
        Align2::LEFT_CENTER,
        choices.get(selected).copied().unwrap_or(""),
        font::sans(FONT_LABEL),
        ink.color(),
    );
    let caret = pos2(field.right() - SPACE_2 - CARET * 0.5, field.center().y);
    painter.add(Shape::convex_polygon(
        vec![
            caret + Vec2::new(-CARET * 0.5, -CARET * 0.3),
            caret + Vec2::new(CARET * 0.5, -CARET * 0.3),
            caret + Vec2::new(0.0, CARET * 0.4),
        ],
        ink.color(),
        Stroke::NONE,
    ));

    let mut picked = None;
    Popup::menu(&response)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .frame(
            Frame::new()
                .fill(palette.ground.color())
                .stroke(Stroke::new(1.0, palette.line.color()))
                .inner_margin(Margin::ZERO),
        )
        .gap(SPACE_1)
        .width(box_width)
        .show(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            for (index, choice) in choices.iter().enumerate() {
                let (row, item) =
                    ui.allocate_exact_size(Vec2::new(box_width, HEIGHT), Sense::click());
                if item.hovered() {
                    ui.painter().rect_filled(row, 0.0, palette.track.color());
                }
                ui.painter().text(
                    pos2(row.left() + SPACE_2, row.center().y),
                    Align2::LEFT_CENTER,
                    choice,
                    font::sans(FONT_LABEL),
                    if index == selected {
                        palette.accent
                    } else {
                        palette.ink
                    }
                    .color(),
                );
                if item.clicked() && index != selected {
                    picked = Some(index);
                }
            }
        });
    picked
}

/// Corner marks round `rect`: what an open control looks like, here and on a
/// [`crate::number`].
pub(crate) fn corners(painter: &egui::Painter, rect: Rect, colour: egui::Color32) {
    let stroke = Stroke::new(1.0, colour);
    for (corner, (dx, dy)) in [
        (rect.left_top(), (1.0, 1.0)),
        (rect.right_top(), (-1.0, 1.0)),
        (rect.left_bottom(), (1.0, -1.0)),
        (rect.right_bottom(), (-1.0, -1.0)),
    ] {
        let corner = corner + Vec2::new(dx * 0.5, dy * 0.5);
        painter.line_segment([corner, corner + Vec2::new(dx * CORNER, 0.0)], stroke);
        painter.line_segment([corner, corner + Vec2::new(0.0, dy * CORNER)], stroke);
    }
}
