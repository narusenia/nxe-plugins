//! The H4 layout's left column (`design-system.md`).
//!
//! From the top: the wordmark, **the subject figure — the largest text in the
//! window, and the only one that size** — its scale, the macros (the caller's),
//! and the `NXE` mark in the bottom corner. It is on the accent, so it is half
//! of the window's L-shaped inverted surface; the figure beside it is the
//! other half.

use crate::font;
use crate::theme::{
    FONT_EYEBROW, FONT_LABEL, FONT_SUBJECT, FONT_TITLE, LINE_EYEBROW, LINE_SUBJECT, LINE_TITLE,
    Palette, SIDEBAR_WIDTH, SPACE_2, SPACE_4,
};
use egui::{Align, Align2, Layout, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};

/// The subject: the one figure the window is about.
pub struct Subject<'a> {
    /// What it is, as an eyebrow.
    pub name: &'a str,
    pub figure: &'a str,
    pub unit: &'a str,
    /// Where it sits on its scale, `0..=1`. Drawn as a tick on a ruler.
    pub position: f32,
}

/// The height of everything above the macros.
pub const HEAD: f32 =
    SPACE_4 + LINE_TITLE + SPACE_4 + LINE_EYEBROW + LINE_SUBJECT + SPACE_2 + SCALE + SPACE_4;
const SCALE: f32 = 12.0;
/// The height under the macros: the `NXE` mark and its margins.
pub const FOOT: f32 = SPACE_4 + LINE_EYEBROW + SPACE_4;

/// Draws the column at the left of `ui`, `height` tall, and lays out `macros`
/// in the space under the subject with the inverted palette.
pub fn sidebar(
    ui: &mut Ui,
    palette: &Palette,
    wordmark: &str,
    subject: &Subject,
    height: f32,
    macros: impl FnOnce(&mut Ui, &Palette),
) {
    let (rect, _) = ui.allocate_exact_size(vec2(SIDEBAR_WIDTH, height), Sense::hover());
    let ink = palette.inverted();
    let inner = rect.shrink2(vec2(SPACE_4, 0.0));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ink.ground.color());

    let mut y = rect.top() + SPACE_4;
    painter.text(
        pos2(inner.left(), y),
        Align2::LEFT_TOP,
        wordmark,
        font::medium(FONT_TITLE),
        ink.ink.color(),
    );
    y += LINE_TITLE + SPACE_4;
    font::tracked(
        &painter,
        pos2(inner.left(), y),
        Align2::LEFT_TOP,
        subject.name,
        FONT_EYEBROW,
        ink.muted.color(),
    );
    y += LINE_EYEBROW;
    let unit = painter.text(
        pos2(inner.right(), y + LINE_SUBJECT - SPACE_2),
        Align2::RIGHT_BOTTOM,
        subject.unit,
        font::sans(FONT_LABEL),
        ink.muted.color(),
    );
    painter.text(
        pos2(unit.left() - SPACE_2, y + LINE_SUBJECT),
        Align2::RIGHT_BOTTOM,
        subject.figure,
        font::mono(FONT_SUBJECT),
        ink.ink.color(),
    );
    y += LINE_SUBJECT + SPACE_2;
    scale(
        &painter,
        Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), SCALE)),
        subject.position,
        &ink,
    );

    let corner = pos2(inner.left(), rect.bottom() - SPACE_4);
    font::tracked(
        &painter,
        corner,
        Align2::LEFT_BOTTOM,
        "NXE",
        FONT_EYEBROW,
        ink.muted.color(),
    );

    let body = Rect::from_min_max(
        pos2(inner.left(), rect.top() + HEAD),
        pos2(
            inner.right(),
            rect.bottom() - SPACE_4 - LINE_EYEBROW - SPACE_4,
        ),
    );
    // A child, not a scope: a scope allocates what its contents used in the
    // parent, and in a horizontal row that moved the cursor *back* to the
    // macros' right edge — the figure beside the sidebar was drawn over it.
    // Top-down whatever the caller's layout is; a builder inherits it otherwise.
    let mut body = ui.new_child(
        UiBuilder::new()
            .max_rect(body)
            .layout(Layout::top_down(Align::Min)),
    );
    macros(&mut body, &ink);
}

/// A ruler: ten ticks, every fifth taller, and the subject's position as a
/// solid mark across it.
fn scale(painter: &egui::Painter, rect: Rect, position: f32, ink: &Palette) {
    let hair = Stroke::new(1.0, ink.line.color());
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], hair);
    for step in 0..=10 {
        let x = rect.left() + rect.width() * step as f32 / 10.0;
        let top = if step % 5 == 0 {
            rect.top() + 2.0
        } else {
            rect.top() + rect.height() * 0.5
        };
        painter.line_segment([pos2(x, top), pos2(x, rect.bottom())], hair);
    }
    let x = rect.left() + rect.width() * position.clamp(0.0, 1.0);
    painter.line_segment(
        [pos2(x, rect.top() - 2.0), pos2(x, rect.bottom())],
        Stroke::new(2.0, ink.ink.color()),
    );
}
