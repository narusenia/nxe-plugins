//! The vendor's mark, drawn rather than set.
//!
//! **`NXE` was three letters of the UI face**, which made the vendor look like
//! one more label that happened to be short. The logotype is a shape, so it
//! reads as a mark — and a mark is what belongs beside a product's name
//! (`crate::header`).
//!
//! **The SVG is the source, the Rust is generated** (`scripts/generate-logo.py`,
//! `mise run logo:generate`). femtovg has no SVG reader, and pulling one in to
//! draw one shape that never changes would cost a dependency and a licence.
//!
//! **The colour in the file is ignored.** The mark is painted with a token, so
//! it inverts with the surface like everything else — which is why one file
//! covers both the black and the white version of the artwork.

mod generated;

use crate::theme;
use vizia::prelude::*;
use vizia::vg;

pub use generated::VIEWBOX;

/// One step of the outline, in the SVG's own coordinates.
#[derive(Clone, Copy, Debug)]
pub enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// How wide the mark is at a given height. **The header's arithmetic needs
/// this** — a window's height is a sum of parts, and so is a row's width.
pub fn width_at(height: f32) -> f32 {
    height * VIEWBOX.0 / VIEWBOX.1
}

/// The mark, filled with [`theme::Palette::subtle`] and fitted to its bounds.
pub struct Mark;

impl Mark {
    pub fn new(cx: &mut Context) -> Handle<'_, Self> {
        Self.build(cx, |_| {})
    }
}

impl View for Mark {
    fn element(&self) -> Option<&'static str> {
        Some("logo")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let palette = theme::palette(cx);

        // Fitted rather than stretched: the artwork's proportions are the mark.
        let scale = (bounds.w / VIEWBOX.0).min(bounds.h / VIEWBOX.1);
        let x0 = bounds.x + (bounds.w - VIEWBOX.0 * scale) * 0.5;
        let y0 = bounds.y + (bounds.h - VIEWBOX.1 * scale) * 0.5;
        let at = |x: f32, y: f32| (x0 + x * scale, y0 + y * scale);

        let mut path = vg::Path::new();
        for segment in generated::SEGMENTS {
            match *segment {
                Segment::Move(x, y) => {
                    let (x, y) = at(x, y);
                    path.move_to(x, y);
                }
                Segment::Line(x, y) => {
                    let (x, y) = at(x, y);
                    path.line_to(x, y);
                }
                Segment::Cubic(x1, y1, x2, y2, x, y) => {
                    let (x1, y1) = at(x1, y1);
                    let (x2, y2) = at(x2, y2);
                    let (x, y) = at(x, y);
                    path.bezier_to(x1, y1, x2, y2, x, y);
                }
                Segment::Close => path.close(),
            }
        }
        canvas.fill_path(&path, &vg::Paint::color(palette.subtle.vg()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated file has to describe a shape, not an empty path — a
    /// regeneration that silently produced nothing would draw a blank space
    /// where the vendor is and nothing would fail.
    #[test]
    fn the_mark_has_an_outline() {
        assert!(generated::SEGMENTS.len() > 20);
        assert!(matches!(generated::SEGMENTS[0], Segment::Move(..)));
        assert!(matches!(
            generated::SEGMENTS[generated::SEGMENTS.len() - 1],
            Segment::Close
        ));
        assert!(VIEWBOX.0 > 0.0 && VIEWBOX.1 > 0.0);
    }

    /// Both letterforms are in there. One `Move` is the `N`, the other is the
    /// `XE` — a parser that dropped a subpath would still pass the test above.
    #[test]
    fn both_subpaths_survived() {
        let moves = generated::SEGMENTS
            .iter()
            .filter(|segment| matches!(segment, Segment::Move(..)))
            .count();
        assert_eq!(moves, 2, "the logotype is two shapes");
    }
}
