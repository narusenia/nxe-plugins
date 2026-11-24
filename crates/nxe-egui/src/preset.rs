//! The preset bar (`v2-plan.md`, decision 17): the name, previous and next,
//! a list to load from, and saving under a new name.
//!
//! **This knows names, not presets.** Reading and writing the values is each
//! plugin's adapter, which hands them to truce's `PresetStore`; this crate
//! stays ignorant of truce.

use crate::font;
use crate::theme::{FONT_LABEL, LINE_LABEL, Palette, SPACE_1, SPACE_2};
use egui::{
    Align2, Area, Key, Order, Popup, Rect, Sense, Shape, Stroke, StrokeKind, TextEdit, Ui, Vec2,
    pos2, vec2,
};

pub const HEIGHT: f32 = LINE_LABEL + SPACE_1 * 2.0;
const ARROW: f32 = HEIGHT;
const SAVE: f32 = 44.0;

/// What the user asked of the preset bar this frame.
#[derive(Clone, PartialEq, Debug)]
pub enum PresetAction {
    Previous,
    Next,
    Load(usize),
    /// Save the current state under this name. Not empty.
    Save(String),
}

/// `current` is `None` when the state matches no preset; `modified` marks a
/// loaded preset that has been changed since.
pub fn preset_bar(
    ui: &mut Ui,
    palette: &Palette,
    names: &[String],
    current: Option<usize>,
    modified: bool,
    width: f32,
) -> Option<PresetAction> {
    let (rect, bar) = ui.allocate_exact_size(vec2(width, HEIGHT), Sense::hover());
    let id = bar.id;
    let hair = Stroke::new(1.0, palette.line.color());
    let mut action = None;

    let previous = Rect::from_min_size(rect.min, Vec2::splat(ARROW));
    let next = Rect::from_min_size(pos2(previous.right(), rect.top()), Vec2::splat(ARROW));
    let save = Rect::from_min_max(pos2(rect.right() - SAVE, rect.top()), rect.right_bottom());
    let name = Rect::from_min_max(
        pos2(next.right(), rect.top()),
        pos2(save.left(), rect.bottom()),
    );

    for (cell, left) in [(previous, true), (next, false)] {
        let response = ui.interact(cell, id.with(left), Sense::click());
        let colour = if response.hovered() {
            palette.ink
        } else {
            palette.muted
        };
        let c = cell.center();
        let (tip, back) = if left { (-3.0, 2.0) } else { (3.0, -2.0) };
        ui.painter().add(Shape::convex_polygon(
            vec![
                pos2(c.x + tip, c.y),
                pos2(c.x + back, c.y - 4.0),
                pos2(c.x + back, c.y + 4.0),
            ],
            colour.color(),
            Stroke::NONE,
        ));
        if response.clicked() && !names.is_empty() {
            action = Some(if left {
                PresetAction::Previous
            } else {
                PresetAction::Next
            });
        }
    }

    let saving_id = id.with("saving");
    let mut saving: Option<String> = ui.data(|data| data.get_temp(saving_id));
    if let Some(mut typed) = saving.take() {
        let field = ui.put(
            name.shrink2(vec2(SPACE_2, 0.0)),
            TextEdit::singleline(&mut typed)
                .font(font::sans(FONT_LABEL))
                .hint_text("Name")
                .frame(egui::Frame::NONE),
        );
        if !field.has_focus() && !field.lost_focus() {
            field.request_focus();
        }
        let enter = ui.input(|input| input.key_pressed(Key::Enter));
        if field.lost_focus() || enter {
            ui.data_mut(|data| data.remove::<String>(saving_id));
            let typed = typed.trim().to_owned();
            if enter && !typed.is_empty() {
                action = Some(PresetAction::Save(typed));
            }
        } else {
            ui.data_mut(|data| data.insert_temp(saving_id, typed));
        }
    } else {
        let response = ui.interact(name, id.with("name"), Sense::click());
        let label = match current {
            Some(index) => names.get(index).map_or("—", String::as_str),
            None => "—",
        };
        let colour = if response.hovered() {
            palette.ink
        } else {
            palette.muted
        };
        let shown = ui.painter().text(
            pos2(name.left() + SPACE_2, name.center().y),
            Align2::LEFT_CENTER,
            label,
            font::sans(FONT_LABEL),
            colour.color(),
        );
        if modified {
            ui.painter().text(
                pos2(shown.right() + SPACE_1, name.center().y),
                Align2::LEFT_CENTER,
                "*",
                font::sans(FONT_LABEL),
                palette.accent.color(),
            );
        }
        let list_id = id.with("list");
        if response.clicked() {
            Popup::toggle_id(ui.ctx(), list_id);
        }
        if Popup::is_id_open(ui.ctx(), list_id) {
            let area = Area::new(list_id)
                .order(Order::Foreground)
                .fixed_pos(name.left_bottom())
                .show(ui.ctx(), |ui| {
                    ui.set_width(name.width());
                    let (list, _) = ui.allocate_exact_size(
                        vec2(name.width(), HEIGHT * names.len().max(1) as f32),
                        Sense::hover(),
                    );
                    ui.painter().rect_filled(list, 0.0, palette.ground.color());
                    ui.painter()
                        .rect_stroke(list, 0.0, hair, StrokeKind::Inside);
                    for (index, preset) in names.iter().enumerate() {
                        let row = Rect::from_min_size(
                            pos2(list.left(), list.top() + HEIGHT * index as f32),
                            vec2(list.width(), HEIGHT),
                        );
                        let row_response = ui.interact(row, list_id.with(index), Sense::click());
                        if row_response.hovered() {
                            ui.painter()
                                .rect_filled(row.shrink(1.0), 0.0, palette.track.color());
                        }
                        let colour = if Some(index) == current {
                            palette.accent
                        } else {
                            palette.ink
                        };
                        ui.painter().text(
                            pos2(row.left() + SPACE_2, row.center().y),
                            Align2::LEFT_CENTER,
                            preset,
                            font::sans(FONT_LABEL),
                            colour.color(),
                        );
                        if row_response.clicked() {
                            action = Some(PresetAction::Load(index));
                        }
                    }
                });
            let clicked_elsewhere = ui.input(|input| input.pointer.any_click())
                && !area.response.contains_pointer()
                && !response.clicked();
            if action.is_some() || clicked_elsewhere {
                Popup::close_id(ui.ctx(), list_id);
            }
        }
    }

    let save_response = ui.interact(save, id.with("save"), Sense::click());
    let colour = if save_response.hovered() {
        palette.ink
    } else {
        palette.muted
    };
    font::tracked(
        ui.painter(),
        save.center(),
        Align2::CENTER_CENTER,
        "Save",
        crate::theme::FONT_EYEBROW,
        colour.color(),
    );
    if save_response.clicked() {
        ui.data_mut(|data| data.insert_temp(saving_id, String::new()));
    }

    let painter = ui.painter();
    painter.rect_stroke(rect, 0.0, hair, StrokeKind::Inside);
    for x in [previous.right(), next.right(), save.left()] {
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], hair);
    }
    action
}

/// Steps `current` by one through `count` presets, wrapping. With no preset
/// loaded, Next goes to the first and Previous to the last.
pub fn step(current: Option<usize>, count: usize, forward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(index), true) => (index + 1) % count,
        (Some(index), false) => (index + count - 1) % count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_wraps_both_ways() {
        assert_eq!(step(Some(2), 3, true), Some(0));
        assert_eq!(step(Some(0), 3, false), Some(2));
        assert_eq!(step(None, 3, true), Some(0));
        assert_eq!(step(None, 3, false), Some(2));
        assert_eq!(step(None, 0, true), None);
    }
}
