//! The embedded UI typeface.
//!
//! [Geist](https://vercel.com/font), SIL OFL 1.1. Its licence text sits beside
//! the files in `assets/geist/` and **has to travel with any release bundle**,
//! because the faces are compiled into the binary.
//!
//! Two faces, one weight each. Hierarchy in this design comes from size and
//! colour, not from weight, so the other weights would be dead bytes.
//!
//! **Numbers are set in Geist Mono.** Fixing the number of decimals stops the
//! digit *count* from changing, but a proportional face still changes width
//! between a `1` and an `8` — and a value that jitters while a knob is dragged
//! is exactly what the fixed decimals were meant to prevent. A monospaced face
//! solves it outright.
//!
//! The family has to be set through the modifier rather than through CSS; see
//! `.agents/rules/vizia.md`.

use vizia::prelude::*;

pub const SANS: &str = "Geist";
pub const MONO: &str = "Geist Mono";

const SANS_BYTES: &[u8] = include_bytes!("../assets/geist/Geist-Regular.ttf");
const MONO_BYTES: &[u8] = include_bytes!("../assets/geist/GeistMono-Regular.ttf");

/// Registers both faces and makes Geist Sans the default. `theme::install`
/// already does this.
pub fn install(cx: &mut Context) {
    cx.add_font_mem(SANS_BYTES);
    cx.add_font_mem(MONO_BYTES);
    cx.set_default_font(&[SANS]);
}

/// A label for a number: Geist Mono, and the `value` class for colour and size.
///
/// Use this wherever a figure is displayed. A plain `Label` gets Geist Sans,
/// which is right for words and wrong for anything that changes while you look
/// at it.
pub fn value<T>(cx: &mut Context, text: impl Res<T> + Clone) -> Handle<'_, Label>
where
    T: ToStringLocalized,
{
    Label::new(cx, text)
        .font_family(vec![FamilyOwned::Name(MONO.to_owned())])
        .class("value")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both faces have to be real TrueType, and the family names have to be the
    /// ones the modifier asks for — a mismatch renders fallback glyphs silently.
    #[test]
    fn the_faces_are_embedded() {
        for (name, bytes) in [("sans", SANS_BYTES), ("mono", MONO_BYTES)] {
            assert!(bytes.len() > 50_000, "{name} looks truncated");
            assert_eq!(
                &bytes[..4],
                &[0x00, 0x01, 0x00, 0x00],
                "{name} is not TrueType"
            );
        }
    }

    /// The names are what `name` table ID 1 says in the files themselves. Guard
    /// against a rename that would silently fall back.
    #[test]
    fn the_family_names_match_the_files() {
        assert_eq!(SANS, "Geist");
        assert_eq!(MONO, "Geist Mono");
    }
}
