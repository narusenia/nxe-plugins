//! The Colour window (Colour 6, `design-system.md`): rounded neutral panels —
//! the controls on the left, and on the right a strip naming the notes over
//! the figure.
//!
//! ```text
//! ╭──────────╮ ╭──────────────────────────────────────╮
//! │ Wordmark │ │ Cmaj9  │C│C#│D│…│B│               strip │
//! │ NXE COL. │ ╰──────────────────────────────────────╯
//! │ ◠  ◠  ◠  │ ╭──────────────────────────────────────╮
//! │ ◠  ◠  ◠  │ │                                      │
//! │ ◠  ◠     │ │               the figure             │
//! │ [preset] │ │                                      │
//! ╰──────────╯ ╰──────────────────────────────────────╯
//! ```
//!
//! **The ground is neutral; colour rides only on the data** — each sounding
//! pitch class in the strip ([`classes`]) and its partials, by [`note`]. The panels' rounded corners are the one
//! exception to the line's square corners, and only the Colour series has them.
//! Meniscus set the sizes; Prisma and Diffract wear the same frame.

use crate::font;
use crate::knob::block_height;
use crate::theme::{FONT_EYEBROW, FONT_TITLE, Palette, SPACE_1, SPACE_2, SPACE_3, SPACE_4, Token};
use egui::{Align2, CornerRadius, Painter, Rect, Stroke, pos2, vec2};

pub const WIDTH: f32 = 880.0;
pub const HEIGHT: f32 = 430.0;
/// Round the window and between panels.
pub const GAP: f32 = 8.0;
/// The controls' panel.
pub const SIDE: f32 = 212.0;
/// The note strip.
pub const STRIP: f32 = 64.0;
/// A panel's corner.
pub const RADIUS: u8 = 8;
/// Inside a panel.
pub const PAD: f32 = 16.0;
/// A knob in the controls' grid, and the grid's columns.
pub const KNOB: f32 = 36.0;
pub const COLUMNS: usize = 3;
/// The NXE mark's height under the wordmark.
const MARK: f32 = 7.0;

/// A panel, a shade over the window's ground.
pub const PANEL: Token = Token::rgb(0x16, 0x16, 0x18);

/// The notes' colours, by pitch class from C: the hue wheel a semitone a
/// step, so a chord's notes spread across it as light through a prism does
/// (round the circle of fifths a chord of fifths came out one green). One
/// lightness and chroma, so no note shouts.
const NOTES: [Token; 12] = [
    Token::rgb(0xF0, 0x75, 0x75),
    Token::rgb(0xF0, 0xB2, 0x75),
    Token::rgb(0xF0, 0xF0, 0x75),
    Token::rgb(0xB3, 0xF0, 0x75),
    Token::rgb(0x75, 0xF0, 0x75),
    Token::rgb(0x75, 0xF0, 0xB3),
    Token::rgb(0x75, 0xF0, 0xF0),
    Token::rgb(0x75, 0xB2, 0xF0),
    Token::rgb(0x75, 0x75, 0xF0),
    Token::rgb(0xB2, 0x75, 0xF0),
    Token::rgb(0xF0, 0x75, 0xF0),
    Token::rgb(0xF0, 0x75, 0xB3),
];

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// The colour of MIDI note `note` (any octave).
pub fn note(note: i32) -> Token {
    NOTES[note.rem_euclid(12) as usize]
}

/// A note's name without its octave.
pub fn name(note: i32) -> &'static str {
    NAMES[note.rem_euclid(12) as usize]
}

/// The nearest MIDI note to `hz`, A4 = 440 Hz.
pub fn nearest(hz: f32) -> i32 {
    (69.0 + 12.0 * (hz.max(1.0) / 440.0).log2()).round() as i32
}

/// Where the caller draws.
pub struct Areas {
    /// Under the wordmark, for the knobs ([`cell`]).
    pub controls: Rect,
    /// The foot of the controls' panel, for the preset bar.
    pub presets: Rect,
    /// Inside the note strip.
    pub strip: Rect,
    /// Inside the figure's panel.
    pub figure: Rect,
}

/// Paints the frame into `ui`'s whole rect. `label` is the series, set after
/// the drawn NXE mark (`COLOUR`).
pub fn colour(
    painter: &Painter,
    rect: Rect,
    palette: &Palette,
    wordmark: &str,
    label: &str,
) -> Areas {
    painter.rect_filled(rect, 0.0, palette.ground.color());
    let inner = rect.shrink(GAP);
    let side = Rect::from_min_size(inner.min, vec2(SIDE, inner.height()));
    let right = Rect::from_min_max(pos2(side.right() + GAP, inner.top()), inner.max);
    let strip = Rect::from_min_size(right.min, vec2(right.width(), STRIP));
    let figure = Rect::from_min_max(pos2(right.left(), strip.bottom() + GAP), right.max);
    for panel in [side, strip, figure] {
        painter.rect_filled(panel, CornerRadius::same(RADIUS), PANEL.color());
    }

    let body = side.shrink(PAD);
    let name = painter.text(
        body.left_top(),
        Align2::LEFT_TOP,
        wordmark,
        font::medium(FONT_TITLE),
        palette.ink.color(),
    );
    let mark = Rect::from_min_size(
        pos2(body.left(), name.bottom() + SPACE_1 + 1.0),
        vec2(crate::logo::width_at(MARK), MARK),
    );
    crate::logo::mark(painter, mark, palette.muted.color());
    font::tracked(
        painter,
        pos2(mark.right() + SPACE_2, mark.center().y),
        Align2::LEFT_CENTER,
        label,
        FONT_EYEBROW,
        palette.muted.color(),
    );

    let presets = Rect::from_min_max(
        pos2(body.left(), body.bottom() - crate::preset::HEIGHT),
        body.max,
    );
    let controls = Rect::from_min_max(
        pos2(body.left(), mark.bottom() + SPACE_4 + SPACE_2),
        pos2(body.right(), presets.top() - SPACE_3),
    );
    Areas {
        controls,
        presets,
        strip: strip.shrink2(vec2(PAD + 2.0, 0.0)),
        figure: figure.shrink(PAD),
    }
}

/// Knob `index`'s place in the controls' grid, [`COLUMNS`] across.
pub fn cell(controls: Rect, index: usize) -> Rect {
    let width = controls.width() / COLUMNS as f32;
    let height = block_height(KNOB) + SPACE_3;
    let (row, column) = (index / COLUMNS, index % COLUMNS);
    Rect::from_min_size(
        pos2(
            controls.left() + column as f32 * width,
            controls.top() + row as f32 * height,
        ),
        vec2(width, block_height(KNOB)),
    )
}

/// The most octaves of one pitch class [`classes`] marks.
pub const OCTAVES: usize = 4;

/// The twelve pitch classes, C to B, in columns split by hairlines: a class
/// that sounds is written in its colour with a bar under it per octave it
/// sounds in, the rest stay faint. A chord reads as a shape in the same place
/// every time, and F3, F4 and F5 are one column, not three names.
pub fn classes(painter: &Painter, palette: &Palette, rect: Rect, octaves: &[u8; 12]) {
    let width = rect.width() / 12.0;
    let hairline = Stroke::new(1.0, palette.line.color());
    let (top, bottom) = (rect.top() + SPACE_3, rect.bottom() - SPACE_3);
    for (class, count) in octaves.iter().enumerate() {
        let left = rect.left() + width * class as f32;
        painter.line_segment([pos2(left, top), pos2(left, bottom)], hairline);
        let centre = left + width * 0.5;
        let on = *count > 0;
        let colour = if on {
            NOTES[class]
        } else {
            palette.subtle.at(0.6)
        };
        painter.text(
            pos2(centre, top + SPACE_1),
            Align2::CENTER_TOP,
            NAMES[class],
            font::sans(FONT_EYEBROW + 1.0),
            colour.color(),
        );
        for octave in 0..usize::from(*count).min(OCTAVES) {
            let y = top + SPACE_4 + SPACE_2 + octave as f32 * (BAR + SPACE_1);
            painter.rect_filled(
                Rect::from_center_size(pos2(centre, y), vec2(width * 0.5, BAR)),
                0.0,
                NOTES[class].color(),
            );
        }
    }
    painter.line_segment(
        [pos2(rect.right(), top), pos2(rect.right(), bottom)],
        hairline,
    );
}

/// One octave's bar in [`classes`].
const BAR: f32 = 2.0;

/// Chord qualities by their intervals over the root, most common first, so an
/// ambiguous set (C6 is also Am7/C) gets the everyday name.
const QUALITIES: [(&str, &[i32]); 24] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("5", &[0, 7]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("mMaj7", &[0, 3, 7, 11]),
    ("7sus4", &[0, 5, 7, 10]),
    ("add9", &[0, 2, 4, 7]),
    ("madd9", &[0, 2, 3, 7]),
    ("9", &[0, 2, 4, 7, 10]),
    ("maj9", &[0, 2, 4, 7, 11]),
    ("m9", &[0, 2, 3, 7, 10]),
    // Voicings that leave out the fifth.
    ("7", &[0, 4, 10]),
    ("maj7", &[0, 4, 11]),
    ("m7", &[0, 3, 10]),
];

/// The chord `notes` (MIDI numbers, any order and octave) spell: `C`, `Fm`,
/// `A5`, `G7/B`. One pitch class is its name alone. `None` when the pitch
/// classes match no quality exactly — a cluster has no name worth guessing.
/// The lowest note is tried as the root first; another root over it is
/// written as a slash chord.
pub fn chord_name(notes: &[i32]) -> Option<String> {
    let bass = notes.iter().min()?.rem_euclid(12);
    let classes = notes
        .iter()
        .fold(0u16, |set, note| set | 1 << note.rem_euclid(12));
    if classes.count_ones() == 1 {
        return Some(NAMES[bass as usize].to_owned());
    }
    let roots = std::iter::once(bass).chain((0..12).filter(|root| *root != bass));
    for root in roots.filter(|root| classes & 1 << root != 0) {
        let over = (0..12)
            .filter(|class| classes & 1 << class != 0)
            .fold(0u16, |set, class: i32| {
                set | 1 << (class - root).rem_euclid(12)
            });
        if let Some((quality, _)) = QUALITIES
            .iter()
            .find(|(_, intervals)| intervals.iter().fold(0u16, |set, i| set | 1 << i) == over)
        {
            let name = format!("{}{quality}", NAMES[root as usize]);
            return Some(if root == bass {
                name
            } else {
                format!("{name}/{}", NAMES[bass as usize])
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::chord_name;

    #[test]
    fn chords_are_named() {
        let name = |notes: &[i32]| chord_name(notes);
        assert_eq!(name(&[60, 64, 67]).as_deref(), Some("C"));
        assert_eq!(name(&[53, 56, 60, 65]).as_deref(), Some("Fm"));
        assert_eq!(name(&[57, 64, 69]).as_deref(), Some("A5"));
        assert_eq!(name(&[55, 59, 62, 65]).as_deref(), Some("G7"));
        assert_eq!(name(&[64, 67, 72]).as_deref(), Some("C/E"));
        assert_eq!(name(&[48, 52, 55, 57]).as_deref(), Some("C6"));
        assert_eq!(name(&[57]).as_deref(), Some("A"));
        assert_eq!(name(&[60, 61, 62]), None);
        assert_eq!(name(&[]), None);
    }
}
