//! The Kit window (Kit 7, `design-system.md`): a dark figure under the
//! wordmark, and the accent as an L — a narrow strip down the left and a band
//! along the bottom.
//!
//! ```text
//! ┌──┬──────────────────────────────┐
//! │K │ Wordmark                     │
//! │I │                              │
//! │T │ the figure (the caller's)    │
//! │N │                              │
//! ├──┴──────────────────────────────┤
//! │ the band (knobs, or readings)   │
//! └─────────────────────────────────┘
//! ```
//!
//! Every Kit and every Peek meter wears this; only the accent and the strip's
//! label change, so the label says which series it belongs to.

use crate::font;
use crate::theme::{FONT_EYEBROW, FONT_TITLE, LINE_TITLE, Palette, SPACE_2, SPACE_3};
use egui::epaint::TextShape;
use egui::text::LayoutJob;
use egui::{Align2, Painter, Rect, TextFormat, Ui, pos2};
use std::f32::consts::FRAC_PI_2;

pub const WIDTH: f32 = 420.0;
pub const HEIGHT: f32 = 280.0;
/// The left arm of the L.
pub const STRIP: f32 = 26.0;
/// The bottom arm of the L.
pub const BAND: f32 = 64.0;
/// What the wordmark takes at the top of the figure. A figure that draws
/// there draws under the name.
pub const HEAD: f32 = SPACE_2 + LINE_TITLE + SPACE_2;
/// The mark's height across the strip: the sidebar's.
const MARK_HEIGHT: f32 = 10.0;

/// Where the caller draws.
pub struct Areas {
    /// The dark area right of the strip and above the band, wordmark included.
    pub figure: Rect,
    /// The band, on the accent. Draw on it with [`Palette::inverted`].
    pub band: Rect,
}

/// Paints the L and the wordmark into `ui`'s whole rect. `label` is the
/// series, as the strip shows it (`PEEK KIT`).
pub fn kit(ui: &Ui, palette: &Palette, wordmark: &str, label: &str) -> Areas {
    let rect = ui.max_rect();
    let painter = ui.painter();
    let ink = palette.inverted();
    let band = Rect::from_min_max(pos2(rect.left(), rect.bottom() - BAND), rect.right_bottom());
    let strip = Rect::from_min_max(rect.left_top(), pos2(rect.left() + STRIP, band.top()));
    painter.rect_filled(strip, 0.0, ink.ground.color());
    painter.rect_filled(band, 0.0, ink.ground.color());

    // Read bottom to top, the series at the top and the vendor at the foot.
    upward(painter, strip, label, ink.ink.color());
    // The drawn mark, as in the sidebar (`logo`), not three letters.
    crate::logo::mark_upward(
        painter,
        strip.shrink2(egui::vec2(0.0, SPACE_2)),
        MARK_HEIGHT,
        ink.ink.color(),
    );

    let figure = Rect::from_min_max(
        pos2(strip.right(), rect.top()),
        pos2(rect.right(), band.top()),
    );
    painter.text(
        pos2(figure.left() + SPACE_3, figure.top() + SPACE_2),
        Align2::LEFT_TOP,
        wordmark,
        font::medium(FONT_TITLE),
        palette.ink.color(),
    );
    Areas { figure, band }
}

/// Tracked capitals turned a quarter to read upwards, centred across `strip`
/// and set against its top.
fn upward(painter: &Painter, strip: Rect, text: &str, colour: egui::Color32) {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: font::medium(FONT_EYEBROW),
            color: colour,
            extra_letter_spacing: crate::theme::TRACKING,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let (length, thickness) = (galley.size().x, galley.size().y);
    // Turned −90° about its top-left, the galley runs up from that point and
    // its thickness lies to the right of it.
    let x = strip.center().x - thickness * 0.5;
    let y = strip.top() + SPACE_2 + length;
    painter.add(TextShape::new(pos2(x, y), galley, colour).with_angle(-FRAC_PI_2));
}
