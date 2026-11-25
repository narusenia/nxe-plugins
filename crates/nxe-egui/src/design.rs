//! The Design window, drafting (Design 7, `design-system.md`): a square
//! figure on the left — the input and what became of it, laid over each
//! other — and on the right three columns, **input / processing / output**,
//! over a table of what drives what.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │ Wordmark  ▬NXE DESIGN                  [preset bar]  │
//! ├──────────────────┬──────────┬──────────┬─────────────┤
//! │ ┌ ─ ─ ─ ─ ─ ─ ┐  │ INPUT    │ PROCESS  │ OUTPUT      │
//! │   the figure     │ sliders  │ sliders  │ sliders     │
//! │   on a grid      ├──────────┴──────────┴─────────────┤
//! │ └ ─ ─ ─ ─ ─ ─ ┘  │ FEEDS ON  what drives what        │
//! └──────────────────┴───────────────────────────────────┘
//! ```
//!
//! It reads as a sheet, not a form: **no boxes**, only rules — the columns
//! split by hairlines, each title over a short accent bar, the figure on a
//! drafting grid. Words are Ioskeley Mono but the wordmark, which is Inter as on every window;
//! figures Geist Mono (2026-11-25, the user's ask). Parasite set the sizes;
//! Singularity and Topology wear the same frame.

use crate::entry::value_entry;
use crate::font;
use crate::input;
use crate::knob::Control;
use crate::theme::{
    FONT_EYEBROW, FONT_LABEL, FONT_VALUE, LINE_LABEL, Palette, SPACE_1, SPACE_2, SPACE_3, SPACE_4,
    TRACKING,
};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

pub const WIDTH: f32 = 880.0;
pub const HEIGHT: f32 = 430.0;
/// Round the edge.
pub const MARGIN: f32 = 12.0;
/// The wordmark's row, margin included.
pub const HEAD: f32 = 40.0;
/// The table under the columns.
pub const FEEDS: f32 = 132.0;
/// One slider: its name and figure, then the bar.
pub const SLIDER: f32 = LINE_LABEL + 12.0;
/// One row of the table.
pub const ROW: f32 = 21.0;
/// The wordmark.
const FONT_WORDMARK: f32 = 20.0;
/// The NXE mark's height in the head.
const MARK: f32 = 9.0;
const BAR_THICKNESS: f32 = 1.0;
const FILL_THICKNESS: f32 = 3.0;
/// The figure's box at the right of a slider: the entry centres in it, so it
/// is kept narrow to sit at the bar's end.
const FIGURE: f32 = 52.0;
/// The drafting grid: divisions across the figure.
const GRID: usize = 8;

/// Where the caller draws.
pub struct Areas {
    /// The square figure, its grid drawn.
    pub figure: Rect,
    /// The right of the head, for the preset bar.
    pub presets: Rect,
    /// Input, processing, output: the area under each column's title.
    pub columns: [Rect; 3],
    /// The table's rows, under its title.
    pub feeds: Rect,
}

/// Paints the frame into `ui`'s whole rect. `label` is the series, set after
/// the drawn NXE mark (`DESIGN`); `titles` are the three columns'.
pub fn design(ui: &Ui, palette: &Palette, wordmark: &str, label: &str, titles: [&str; 3]) -> Areas {
    let rect = ui.max_rect();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.ground.color());
    let rule = Stroke::new(1.0, palette.line.color());

    let inner = rect.shrink(MARGIN);
    let split = inner.top() + HEAD - MARGIN + SPACE_3 * 0.5;
    // The head is the whole band, so what sits in it centres on the band.
    let head = Rect::from_min_max(pos2(inner.left(), rect.top()), pos2(inner.right(), split));
    // The head is a band on the accent, written dark (asked for on 2026-11-25);
    // the caller sets its presets on the same inverted palette.
    painter.rect_filled(
        Rect::from_min_max(rect.min, pos2(rect.right(), split)),
        0.0,
        palette.accent.color(),
    );
    let band = palette.inverted();
    let name = painter.text(
        pos2(head.left(), head.center().y),
        Align2::LEFT_CENTER,
        wordmark,
        font::medium(FONT_WORDMARK),
        band.ink.color(),
    );
    // The drawn mark, as on every other window, not three letters.
    let mark = Rect::from_min_size(
        pos2(name.right() + SPACE_4, head.center().y - MARK * 0.5),
        vec2(crate::logo::width_at(MARK), MARK),
    );
    crate::logo::mark(painter, mark, band.accent.color());
    caps(
        painter,
        pos2(mark.right() + SPACE_2, head.center().y),
        Align2::LEFT_CENTER,
        label,
        band.muted.color(),
    );

    let body = Rect::from_min_max(pos2(inner.left(), split + SPACE_3 * 0.5), inner.max);
    let side = body.height();
    let figure = Rect::from_min_size(body.min, vec2(side, side));
    sheet(painter, palette, figure);

    let right = Rect::from_min_max(pos2(figure.right() + SPACE_4, body.top()), body.max);
    let table = Rect::from_min_max(pos2(right.left(), right.bottom() - FEEDS), right.max);
    let upper = Rect::from_min_max(right.min, pos2(right.right(), table.top() - SPACE_3));
    let width = upper.width() / 3.0;
    let columns = std::array::from_fn(|index| {
        let left = upper.left() + index as f32 * width;
        if index > 0 {
            painter.line_segment([pos2(left, upper.top()), pos2(left, upper.bottom())], rule);
        }
        let pad = if index == 0 { 0.0 } else { SPACE_3 };
        let column = Rect::from_min_max(
            pos2(left + pad, upper.top()),
            pos2(left + width - SPACE_3, upper.bottom()),
        );
        titled(
            painter,
            column,
            titles[index],
            palette.muted.color(),
            palette.accent.color(),
        )
    });
    // The table sits on the accent (asked for on 2026-11-25); `feed` writes on it.
    painter.rect_filled(table, 0.0, palette.accent.color());
    let feeds = titled(
        painter,
        table
            .shrink2(vec2(SPACE_3, 0.0))
            .with_min_y(table.top() + SPACE_3),
        "Feeds on",
        palette.ground.at(0.7).color(),
        palette.ground.color(),
    );

    let presets = Rect::from_min_max(pos2(right.left(), head.top()), head.max);
    Areas {
        figure,
        presets,
        columns,
        feeds,
    }
}

/// The figure's ground: a drafting grid with a stronger cross through the
/// middle.
fn sheet(painter: &Painter, palette: &Palette, rect: Rect) {
    painter.rect_filled(rect, 0.0, palette.ink.at(0.025).color());
    let minor = Stroke::new(1.0, palette.line.at(0.45).color());
    let major = Stroke::new(1.0, palette.line.color());
    for step in 1..GRID {
        let at = step as f32 / GRID as f32;
        let stroke = if step == GRID / 2 { major } else { minor };
        let x = rect.left() + rect.width() * at;
        let y = rect.top() + rect.height() * at;
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke);
        painter.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], stroke);
    }
}

/// A title over a short accent bar; returns the area under it.
fn titled(painter: &Painter, rect: Rect, title: &str, colour: Color32, bar: Color32) -> Rect {
    let title = caps(painter, rect.left_top(), Align2::LEFT_TOP, title, colour);
    let mark = Rect::from_min_size(
        pos2(rect.left(), title.bottom() + SPACE_1 + 1.0),
        vec2(16.0, 2.0),
    );
    painter.rect_filled(mark, 0.0, bar);
    Rect::from_min_max(pos2(rect.left(), mark.bottom() + SPACE_3), rect.max)
}

/// Tracked capitals in the Design face: an eyebrow, on a drawing. Returns the
/// rect drawn.
pub fn caps(painter: &Painter, pos: Pos2, anchor: Align2, text: &str, colour: Color32) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: font::drafting_medium(FONT_EYEBROW),
            color: colour,
            extra_letter_spacing: TRACKING * 1.5,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, colour);
    rect
}

/// A thin slider: the name on the left, the figure on the right, the bar
/// under both. The same gestures as a knob — a vertical drag, double-click to
/// reset, the figure to type into (`crate::input`).
pub fn slider(
    ui: &mut Ui,
    palette: &Palette,
    name: &str,
    value: f32,
    text: &str,
    width: f32,
) -> Control {
    let value = value.clamp(0.0, 1.0);
    let (rect, block) = ui.allocate_exact_size(vec2(width, SLIDER), Sense::hover());
    let id = block.id.with("slider");
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let gestures = input::read(&response, value);
    let shown = gestures.change.unwrap_or(value);
    let active = response.hovered() || response.dragged();

    let painter = ui.painter();
    painter.text(
        pos2(rect.left(), rect.top()),
        Align2::LEFT_TOP,
        name,
        font::drafting(FONT_LABEL),
        if active { palette.ink } else { palette.muted }.color(),
    );
    let y = rect.top() + LINE_LABEL + 6.0;
    let track = Rect::from_center_size(pos2(rect.center().x, y), vec2(rect.width(), BAR_THICKNESS));
    painter.rect_filled(track, 0.0, palette.line.color());
    let end = track.left() + track.width() * shown;
    if shown > 0.0 {
        let fill = Rect::from_min_max(
            pos2(track.left(), y - FILL_THICKNESS * 0.5),
            pos2(end, y + FILL_THICKNESS * 0.5),
        );
        painter.rect_filled(fill, 0.0, palette.accent.color());
    }
    // A tick where the value sits, as on a scale.
    painter.line_segment(
        [pos2(end, y - 4.0), pos2(end, y + 4.0)],
        Stroke::new(1.0, palette.accent.color()),
    );

    let figure = Rect::from_min_max(
        pos2(rect.right() - FIGURE, rect.top()),
        pos2(rect.right(), rect.top() + LINE_LABEL),
    );
    let typed = value_entry(ui, palette, id.with("entry"), figure, text, gestures.edit);
    Control { gestures, typed }
}

/// One row of the table: what drives, what it drives, and the figure, written
/// dark on the accent the table sits on.
pub fn feed(ui: &Ui, palette: &Palette, rect: Rect, from: &str, to: &str, figure: &str) {
    let painter = ui.painter();
    painter.line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, palette.ground.at(0.15).color()),
    );
    let y = rect.center().y;
    let column = rect.width() * 0.36;
    caps(
        painter,
        pos2(rect.left(), y),
        Align2::LEFT_CENTER,
        from,
        palette.ground.at(0.6).color(),
    );
    painter.text(
        pos2(rect.left() + column - SPACE_4, y),
        Align2::LEFT_CENTER,
        "→",
        font::drafting(FONT_LABEL),
        palette.ground.at(0.6).color(),
    );
    painter.text(
        pos2(rect.left() + column, y),
        Align2::LEFT_CENTER,
        to,
        font::drafting(FONT_LABEL),
        palette.ground.color(),
    );
    painter.text(
        pos2(rect.right(), y),
        Align2::RIGHT_CENTER,
        figure,
        font::mono(FONT_VALUE),
        palette.ground.color(),
    );
}

/// How tall a row of [`glyphs`] is.
pub const GLYPHS: f32 = 26.0;

/// A row of mutually exclusive choices drawn as pictures rather than words:
/// `draw` paints choice `index` into the rect it is given, in the colour it
/// is given. The selected one sits on the accent. The names show on hover,
/// and are what a screen reader reads.
pub fn glyphs(
    ui: &mut Ui,
    palette: &Palette,
    names: &[&str],
    selected: usize,
    width: f32,
    draw: impl Fn(&Painter, Rect, Color32, usize),
) -> Option<usize> {
    let (rect, row) = ui.allocate_exact_size(vec2(width, GLYPHS), Sense::hover());
    let step = rect.width() / names.len().max(1) as f32;
    let mut picked = None;
    for (index, name) in names.iter().enumerate() {
        let cell = Rect::from_min_size(
            pos2(rect.left() + step * index as f32, rect.top()),
            vec2(step, GLYPHS),
        );
        let response = ui
            .interact(cell, row.id.with(index), Sense::click())
            .on_hover_text(*name);
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                true,
                index == selected,
                *name,
            )
        });
        let on = index == selected;
        let painter = ui.painter();
        if on {
            painter.rect_filled(cell, 0.0, palette.accent.color());
        } else if response.hovered() {
            painter.rect_filled(cell, 0.0, palette.track.color());
        }
        let colour = if on {
            palette.ground
        } else if response.hovered() {
            palette.ink
        } else {
            palette.muted
        };
        draw(
            painter,
            cell.shrink2(vec2(step * 0.22, GLYPHS * 0.3)),
            colour.color(),
            index,
        );
        if index > 0 {
            painter.line_segment(
                [cell.left_top(), cell.left_bottom()],
                Stroke::new(1.0, palette.line.color()),
            );
        }
        if response.clicked() && !on {
            picked = Some(index);
        }
    }
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, palette.line.color()),
        StrokeKind::Inside,
    );
    picked
}
