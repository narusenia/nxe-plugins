//! A value as a word and a figure — `Reference 440 Hz` — with a knob's
//! gestures on the figure: drag it up or down to change it (Shift for fine),
//! double-click to reset, Cmd-click to type (`crate::entry::entry`, which
//! takes no click of its own, so every click reaches the figure). For a value
//! that wants no dial of its own (Dew's Pitch, Halo's Root). No box: the
//! figure in the accent is what says it can be moved, and corner marks close
//! round it while it is held or typed into, as on a [`crate::select`]. As
//! tall as a [`crate::segment`], and as wide as its words, so a row of these,
//! a [`crate::toggle`] and a [`crate::select`] is laid out by a right-to-left
//! `Ui`.

use crate::entry::entry;
use crate::font;
use crate::input;
use crate::knob::Control;
use crate::segment::HEIGHT;
use crate::select::corners;
use crate::theme::{FONT_LABEL, Palette, SPACE_1, SPACE_2};
use egui::{Align2, Rect, Sense, Ui, Vec2, pos2};

/// The narrowest the figure gets, so the field typed into has room.
const MIN_FIGURE: f32 = 40.0;

/// `value` is normalised `0..=1`; `text` is the figure.
pub fn number(ui: &mut Ui, palette: &Palette, label: &str, value: f32, text: &str) -> Control {
    let painter = ui.painter();
    let word = painter.layout_no_wrap(
        label.to_owned(),
        font::sans(FONT_LABEL),
        palette.muted.color(),
    );
    let figure_width = painter
        .layout_no_wrap(
            text.to_owned(),
            font::sans(FONT_LABEL),
            palette.accent.color(),
        )
        .size()
        .x
        .max(MIN_FIGURE)
        + 2.0 * SPACE_2;
    let gap = SPACE_1 * 1.5;
    let (rect, block) = ui.allocate_exact_size(
        Vec2::new(word.size().x + gap + figure_width, HEIGHT),
        Sense::hover(),
    );
    let figure = Rect::from_min_max(pos2(rect.right() - figure_width, rect.top()), rect.max);
    let id = block.id.with("number");
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let gestures = input::read(&response, value.clamp(0.0, 1.0));
    let active = response.hovered() || response.dragged();

    let painter = ui.painter().clone();
    painter.galley(
        pos2(rect.left(), rect.center().y - word.size().y * 0.5),
        word,
        palette.muted.color(),
    );
    let entry_id = id.with("entry");
    let open = ui.data(|data| data.get_temp::<String>(entry_id).is_some());
    let typed = if open || gestures.edit {
        // The field draws in the ink; the figure is the accent's.
        let accent = Palette {
            ink: palette.accent,
            ..*palette
        };
        entry(ui, &accent, entry_id, figure, text, gestures.edit)
    } else {
        painter.text(
            pos2(figure.left() + SPACE_2, figure.center().y),
            Align2::LEFT_CENTER,
            text,
            font::sans(FONT_LABEL),
            palette.accent.color(),
        );
        None
    };
    if active || open {
        corners(&painter, figure, palette.ink.color());
    }
    if active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    Control { gestures, typed }
}
