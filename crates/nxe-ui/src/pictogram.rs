//! The plugins' own symbols, drawn as paths (`UI-17`).
//!
//! **Not a font.** Lucide is a font and the trade it forced is written down in
//! [`crate::icon`]: the strokes are baked into filled glyphs, so an icon cannot
//! take a weight. These marks sit next to text at two different sizes and want
//! two different weights, which a font cannot give them (`UI-2`). Making one
//! would also add a generator to the build for eleven shapes that never change.
//!
//! **A mark never replaces its word.** Every one of these is drawn beside the
//! name it belongs to — [`heading`] and [`label`] are the two ways to do that,
//! and they exist so no call site has to decide the size again. The symbol is
//! what makes a column findable at a glance; the word is what makes it
//! understandable the first time (`.agents/rules/ui.md`).
//!
//! **Drawn for 12 px.** The smallest place one of these lands is a table's
//! column heading, which is [`crate::theme::LINE_EYEBROW`] tall. That is the
//! whole reason the drawings are as plain as they are: a glyph with eight
//! features has none at 12 px. Nothing here uses a curve, which also keeps them
//! in the same vocabulary as the rest of the window — rules, right angles and
//! flat fills.

use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// The design grid. Lucide's, so the two sets sit at the same optical size when
/// they are set at the same pixel size.
pub const GRID: f32 = 24.0;

/// The stroke width on the grid, before scaling. At a 12 px mark this is
/// three quarters of a pixel, which is the hairline the rest of the design is
/// drawn with ([`theme::RULE`]).
pub const WEIGHT: f32 = 1.5;

/// The heavier weight, for a mark set beside a control's name rather than a
/// column's. **This is the reason these are paths** — the same shape carries
/// more when it is nearer the eye.
pub const WEIGHT_STRONG: f32 = 2.0;

/// One piece of a drawing, in grid coordinates with `y` pointing down.
#[derive(Clone, Copy, Debug)]
pub enum Stroke {
    /// An open polyline.
    Line(&'static [(f32, f32)]),
    /// A filled rectangle: `(x, y, width, height)`.
    Fill(f32, f32, f32, f32),
    /// A stroked rectangle — a quantity that is absent, against a [`Fill`] that
    /// is present.
    ///
    /// [`Fill`]: Stroke::Fill
    Frame(f32, f32, f32, f32),
    /// A filled polygon, closed automatically. **An arrowhead is the only thing
    /// here that a rectangle cannot be**, and a stroked one is three hairlines
    /// meeting at a point — which at 12 px is a smudge.
    Solid(&'static [(f32, f32)]),
}

/// A whole symbol.
pub type Glyph = &'static [Stroke];

/// Upward, toward the line it is allowed to reach.
///
/// **An arrow, not the two-bar picture that was here first.** That drawing said
/// "compression" properly — a short bar with a hollow extension up to a tall
/// one — and at 12 px `UP` and `DOWN` were the same grey pair of blocks. A
/// column heading has one job, which is to be found; the word beside it is what
/// carries the meaning (`.agents/rules/ui.md`).
pub const UP: Glyph = &[
    Stroke::Line(&[(3.0, 3.0), (21.0, 3.0)]),
    Stroke::Solid(&[(12.0, 7.0), (19.0, 16.0), (5.0, 16.0)]),
    Stroke::Fill(9.5, 16.0, 5.0, 5.0),
];

/// Downward, toward the line it is held to.
pub const DOWN: Glyph = &[
    Stroke::Fill(9.5, 4.0, 5.0, 5.0),
    Stroke::Solid(&[(12.0, 18.0), (5.0, 9.0), (19.0, 9.0)]),
    Stroke::Line(&[(3.0, 22.0), (21.0, 22.0)]),
];

/// A static trim: a slider sitting off its centre.
///
/// **Two drawings before this one were read as something else.** A vertical
/// track with a handle across it was a plus sign at every size; a bipolar bar
/// filled from the centre was a toggle switch, which is worse than unclear —
/// it says the wrong thing about the control underneath it.
pub const GAIN: Glyph = &[
    Stroke::Line(&[(3.0, 12.0), (21.0, 12.0)]),
    Stroke::Line(&[(12.0, 7.0), (12.0, 17.0)]),
    Stroke::Fill(14.5, 7.0, 4.0, 10.0),
];

/// One of several, alone. The two that are not heard are lines rather than
/// empty boxes: a 5-unit box with a hairline round it is a smudge at 12 px.
pub const SOLO: Glyph = &[
    Stroke::Line(&[(4.0, 6.0), (4.0, 18.0)]),
    Stroke::Fill(9.5, 6.0, 5.0, 12.0),
    Stroke::Line(&[(20.0, 6.0), (20.0, 18.0)]),
];

/// Edges that slide.
pub const FOCUS: Glyph = &[
    Stroke::Line(&[(6.0, 4.0), (6.0, 20.0)]),
    Stroke::Line(&[(18.0, 4.0), (18.0, 20.0)]),
    Stroke::Line(&[(6.0, 12.0), (18.0, 12.0)]),
    Stroke::Line(&[(9.0, 9.0), (6.0, 12.0), (9.0, 15.0)]),
    Stroke::Line(&[(15.0, 9.0), (18.0, 12.0), (15.0, 15.0)]),
];

/// A peak held down: the mountain with its top taken off, and the line it was
/// taken off at.
pub const DE_HARSH: Glyph = &[
    Stroke::Line(&[(4.0, 20.0), (9.0, 9.0), (15.0, 9.0), (20.0, 20.0)]),
    Stroke::Line(&[(3.0, 9.0), (21.0, 9.0)]),
];

/// The bottom, and the lid that is only over it.
///
/// **A bracket over one block, not a bar chart.** Three bars of different
/// heights with a cap on the first was the same silhouette as `SOLO` at 12 px.
pub const SUB_PROTECT: Glyph = &[
    Stroke::Line(&[(3.0, 10.0), (3.0, 6.0), (21.0, 6.0), (21.0, 10.0)]),
    Stroke::Fill(5.0, 12.0, 14.0, 9.0),
];

/// A transient: one needle on a flat line.
pub const SNAP: Glyph = &[Stroke::Line(&[
    (3.0, 18.0),
    (10.0, 18.0),
    (12.0, 4.0),
    (14.0, 18.0),
    (21.0, 18.0),
])];

/// The same needle, hit — the two marks beside it are the only difference, and
/// they are there because a bolder needle would have been the same drawing at
/// another weight.
pub const PUNCH: Glyph = &[
    Stroke::Line(&[
        (3.0, 18.0),
        (10.0, 18.0),
        (12.0, 4.0),
        (14.0, 18.0),
        (21.0, 18.0),
    ]),
    Stroke::Line(&[(4.0, 6.0), (8.0, 10.0)]),
    Stroke::Line(&[(20.0, 6.0), (16.0, 10.0)]),
];

/// A floor that has moved up.
///
/// **A step, not an arrow.** `UP` is an arrow, and two arrows in one window
/// mean neither is a landmark.
pub const LIFT: Glyph = &[Stroke::Line(&[
    (3.0, 19.0),
    (11.0, 19.0),
    (11.0, 9.0),
    (21.0, 9.0),
])];

/// Twice as many samples.
pub const OVERSAMPLE: Glyph = &[
    Stroke::Line(&[(7.0, 4.0), (7.0, 10.0)]),
    Stroke::Line(&[(17.0, 4.0), (17.0, 10.0)]),
    Stroke::Line(&[(4.5, 14.0), (4.5, 20.0)]),
    Stroke::Line(&[(9.5, 14.0), (9.5, 20.0)]),
    Stroke::Line(&[(14.5, 14.0), (14.5, 20.0)]),
    Stroke::Line(&[(19.5, 14.0), (19.5, 20.0)]),
];

/// Every symbol and the word it is drawn for. **The gallery reads this**, so a
/// mark that is added without a row here is a mark nobody can look at.
pub const ALL: [(&str, Glyph); 11] = [
    ("UP", UP),
    ("DOWN", DOWN),
    ("GAIN", GAIN),
    ("SOLO", SOLO),
    ("FOCUS", FOCUS),
    ("DE-HARSH", DE_HARSH),
    ("SUB PROT", SUB_PROTECT),
    ("SNAP", SNAP),
    ("LIFT", LIFT),
    ("PUNCH", PUNCH),
    ("OVERSAMPLE", OVERSAMPLE),
];

/// A drawn mark, fitted to its bounds and painted with
/// [`theme::Palette::muted`] — the colour `.icon` gives a Lucide glyph, so the
/// two sets do not disagree where they sit in one row.
pub struct Pictogram {
    glyph: Glyph,
    weight: f32,
}

impl Pictogram {
    /// At the hairline weight, for a mark beside a column's name.
    pub fn new(cx: &mut Context, glyph: Glyph) -> Handle<'_, Self> {
        Self::weighted(cx, glyph, WEIGHT)
    }

    pub fn weighted(cx: &mut Context, glyph: Glyph, weight: f32) -> Handle<'_, Self> {
        Self { glyph, weight }.build(cx, |_| {})
    }
}

impl View for Pictogram {
    fn element(&self) -> Option<&'static str> {
        Some("pictogram")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let palette = theme::palette(cx);
        let paint = vg::Paint::color(palette.muted.vg());

        // Fitted rather than stretched: a square drawing in a wide box stays
        // square and centres, the way the vendor's mark does (`crate::logo`).
        let scale = (bounds.w / GRID).min(bounds.h / GRID);
        let x0 = bounds.x + (bounds.w - GRID * scale) * 0.5;
        let y0 = bounds.y + (bounds.h - GRID * scale) * 0.5;
        let at = |x: f32, y: f32| (x0 + x * scale, y0 + y * scale);

        let mut stroke = vg::Paint::color(palette.muted.vg());
        // **Not rounded off at the minimum.** A hairline that is snapped to a
        // whole pixel at 12 px and left fractional at 16 stops being the same
        // drawing; femtovg antialiases a sub-pixel stroke and that is the
        // intended look here.
        stroke.set_line_width(self.weight * scale);
        stroke.set_line_join(vg::LineJoin::Miter);
        stroke.set_line_cap(vg::LineCap::Butt);

        for piece in self.glyph {
            match *piece {
                Stroke::Line(points) => {
                    let mut path = vg::Path::new();
                    for (index, (x, y)) in points.iter().enumerate() {
                        let (x, y) = at(*x, *y);
                        if index == 0 {
                            path.move_to(x, y);
                        } else {
                            path.line_to(x, y);
                        }
                    }
                    canvas.stroke_path(&path, &stroke);
                }
                Stroke::Fill(x, y, width, height) => {
                    let (x, y) = at(x, y);
                    let mut path = vg::Path::new();
                    path.rect(x, y, width * scale, height * scale);
                    canvas.fill_path(&path, &paint);
                }
                Stroke::Solid(points) => {
                    let mut path = vg::Path::new();
                    for (index, (x, y)) in points.iter().enumerate() {
                        let (x, y) = at(*x, *y);
                        if index == 0 {
                            path.move_to(x, y);
                        } else {
                            path.line_to(x, y);
                        }
                    }
                    path.close();
                    canvas.fill_path(&path, &paint);
                }
                Stroke::Frame(x, y, width, height) => {
                    // Inset by half the stroke, so a frame and a fill given the
                    // same rectangle cover the same area. Without it the
                    // hollow bar in `UP` sits a hairline proud of the solid one
                    // under it and the pair reads as misaligned.
                    let inset = self.weight * 0.5;
                    let (x, y) = at(x + inset, y + inset);
                    let mut path = vg::Path::new();
                    path.rect(
                        x,
                        y,
                        (width - self.weight) * scale,
                        (height - self.weight) * scale,
                    );
                    canvas.stroke_path(&path, &stroke);
                }
            }
        }
    }
}

/// A mark and a column's name, on one line. The mark is the eyebrow's own
/// height, so a heading row does not grow when one gains a symbol.
pub fn heading<'a>(cx: &'a mut Context, glyph: Glyph, word: &'static str) -> Handle<'a, HStack> {
    HStack::new(cx, |cx| {
        Pictogram::new(cx, glyph)
            .width(Pixels(theme::LINE_EYEBROW))
            .height(Pixels(theme::LINE_EYEBROW));
        Label::new(cx, word)
            .class("eyebrow")
            .height(Pixels(theme::LINE_EYEBROW));
    })
    .height(Pixels(theme::LINE_EYEBROW))
    .col_between(Pixels(theme::SPACE_1))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0))
}

/// A mark and a control's name, on one line. Heavier and larger than
/// [`heading`], because it is a thing to be operated rather than a column to be
/// found.
pub fn label<'a>(cx: &'a mut Context, glyph: Glyph, word: &'static str) -> Handle<'a, HStack> {
    HStack::new(cx, |cx| {
        Pictogram::weighted(cx, glyph, WEIGHT_STRONG)
            .width(Pixels(theme::LINE_LABEL))
            .height(Pixels(theme::LINE_LABEL));
        Label::new(cx, word)
            .class("subtle")
            .height(Pixels(theme::LINE_LABEL));
    })
    .height(Pixels(theme::LINE_LABEL))
    .col_between(Pixels(theme::SPACE_1))
    .child_top(Stretch(1.0))
    .child_bottom(Stretch(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(glyph: Glyph) -> Vec<(f32, f32)> {
        let mut all = Vec::new();
        for piece in glyph {
            match *piece {
                Stroke::Line(points) | Stroke::Solid(points) => all.extend_from_slice(points),
                Stroke::Fill(x, y, width, height) | Stroke::Frame(x, y, width, height) => {
                    all.push((x, y));
                    all.push((x + width, y + height));
                }
            }
        }
        all
    }

    /// A drawing that runs off the grid is not clipped — it is scaled with
    /// everything else and lands smaller than its neighbours, which reads as a
    /// drawing that is simply worse rather than as a bug.
    #[test]
    fn every_drawing_stays_on_the_grid() {
        for (name, glyph) in ALL {
            for (x, y) in points(glyph) {
                assert!((0.0..=GRID).contains(&x), "{name}: x = {x}");
                assert!((0.0..=GRID).contains(&y), "{name}: y = {y}");
            }
        }
    }

    /// **The 12 px rule.** The smallest feature in a drawing is what decides
    /// whether it survives a column heading, and 3 grid units is a pixel and a
    /// half there. Anything finer was measured to be mush.
    #[test]
    fn nothing_is_finer_than_the_smallest_place_it_lands() {
        const FINEST: f32 = 3.0;
        for (name, glyph) in ALL {
            for piece in glyph {
                match *piece {
                    Stroke::Fill(_, _, width, height) | Stroke::Frame(_, _, width, height) => {
                        assert!(width >= FINEST, "{name}: a {width}-wide box");
                        assert!(height >= FINEST, "{name}: a {height}-tall box");
                    }
                    Stroke::Line(points) | Stroke::Solid(points) => {
                        for pair in points.windows(2) {
                            let (x0, y0) = pair[0];
                            let (x1, y1) = pair[1];
                            let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
                            assert!(length >= FINEST, "{name}: a {length:.1}-long segment");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_drawing_has_something_in_it() {
        for (name, glyph) in ALL {
            assert!(!glyph.is_empty(), "{name} is empty");
            assert!(!points(glyph).is_empty(), "{name} has no points");
        }
    }

    /// The list is what the gallery shows and what a plugin picks from. Two
    /// rows with one name would hide one of them from both.
    #[test]
    fn the_names_are_distinct() {
        let mut names: Vec<&str> = ALL.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count, "two symbols share a name");
    }

    /// `UI-17` asked for between ten and sixteen. Fewer and the set is not
    /// worth its own vocabulary; more and it is a font by another name.
    #[test]
    fn the_set_is_the_size_it_was_asked_for() {
        assert!((10..=16).contains(&ALL.len()), "{} symbols", ALL.len());
    }
}
