//! The embedded UI typeface.
//!
//! **Words are [Inter](https://rsms.me/inter/), figures are
//! [Geist Mono](https://vercel.com/font)** — both SIL OFL 1.1. The licence text
//! sits beside the files in `assets/inter/` and `assets/geist/`, and **has to
//! travel with any release bundle**, because the faces are compiled into the
//! binary.
//!
//! **Numbers stay in the mono face.** Fixing the number of decimals stops the
//! digit *count* from changing, but a proportional face still changes width
//! between a `1` and an `8` — and a value that jitters while a knob is dragged
//! is exactly what the fixed decimals were meant to prevent. Inter can do this
//! with its tabular figures, but `tnum` is an OpenType feature and **this vizia
//! revision has no way to turn one on**, so the mono face is still the answer.
//!
//! Three weights of Inter. Hierarchy comes mostly from size and colour;
//! [`title`] and [`display`] are the two places a weight carries meaning, and
//! both are deliberate. Nothing else reaches for a weight — if a third thing
//! needs one, the rule was wrong and should be rewritten rather than worked
//! around.
//!
//! **Inter Light's `name` table says family `Inter Light`, subfamily
//! `Regular`** — only its *typographic* family (name ID 16) says `Inter`.
//! `fontdb` prefers ID 16 and falls back to ID 1, so it lands in the `Inter`
//! family at weight 300 and the modifier reaches it. **Had it gone the other
//! way, `font_weight(Light)` would have silently rendered Regular** — which is
//! why the test below reads the name tables out of the files rather than
//! trusting the constants.
//!
//! The family has to be set through the modifier rather than through CSS; see
//! `.agents/rules/vizia.md`.

use vizia::prelude::*;

pub const SANS: &str = "Inter";
pub const MONO: &str = "Geist Mono";

const LIGHT_BYTES: &[u8] = include_bytes!("../assets/inter/Inter-Light.ttf");
const SANS_BYTES: &[u8] = include_bytes!("../assets/inter/Inter-Regular.ttf");
const BOLD_BYTES: &[u8] = include_bytes!("../assets/inter/Inter-Bold.ttf");
const MONO_BYTES: &[u8] = include_bytes!("../assets/geist/GeistMono-Regular.ttf");

/// Registers every face and makes Inter the default. `theme::install` already
/// does this.
///
/// All three Inter faces join one family, so [`title`] and [`display`] reach
/// theirs by weight rather than by name (see the module docs for why that is
/// not obvious for Light).
pub fn install(cx: &mut Context) {
    cx.add_font_mem(LIGHT_BYTES);
    cx.add_font_mem(SANS_BYTES);
    cx.add_font_mem(BOLD_BYTES);
    cx.add_font_mem(MONO_BYTES);
    cx.set_default_font(&[SANS]);
}

/// Large text set light — the fors register, where size does the work and the
/// stroke gets out of the way.
///
/// **For text that is big enough to carry it.** Light at label size is thin
/// rather than quiet, and on a dark ground it goes fragile before it goes
/// elegant. The window's large headings are what this is for.
pub fn display<T>(cx: &mut Context, text: impl Res<T> + Clone) -> Handle<'_, Label>
where
    T: ToStringLocalized,
{
    Label::new(cx, text).font_weight(FontWeightKeyword::Light)
}

/// The wordmark: the plugin's name, in the one bold face this design has.
///
/// **The weight is set by the modifier, not by CSS.** `font-weight` does parse
/// in this vizia revision, but keeping it here means the `.title` class stays
/// one thing — the size and the colour — and there is exactly one place that
/// knows the exception exists.
pub fn title<T>(cx: &mut Context, text: impl Res<T> + Clone) -> Handle<'_, Label>
where
    T: ToStringLocalized,
{
    Label::new(cx, text)
        .font_weight(FontWeightKeyword::Bold)
        .class("title")
}

/// A label for a number: Geist Mono, and the `value` class for colour and size.
///
/// Use this wherever a figure is displayed. A plain `Label` gets Inter, which is
/// right for words and wrong for anything that changes while you look at it.
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

    /// Every face has to be real TrueType, or the glyphs come from a fallback
    /// silently.
    #[test]
    fn the_faces_are_embedded() {
        for (name, bytes) in [
            ("light", LIGHT_BYTES),
            ("sans", SANS_BYTES),
            ("bold", BOLD_BYTES),
            ("mono", MONO_BYTES),
        ] {
            assert!(bytes.len() > 50_000, "{name} looks truncated");
            assert_eq!(
                &bytes[..4],
                &[0x00, 0x01, 0x00, 0x00],
                "{name} is not TrueType"
            );
        }
    }

    /// **Read out of the files, not asserted against the constants.** The old
    /// version of this test compared two string literals to each other and
    /// would have passed with any font at all in the folder.
    ///
    /// `fontdb` registers a face under its *typographic* family (`name` ID 16)
    /// when it has one, and under ID 1 otherwise. Inter Light only says `Inter`
    /// in ID 16 — ID 1 says `Inter Light` — so if that preference ever changed,
    /// `font_weight(Light)` would render Regular without a word.
    #[test]
    fn the_faces_land_in_the_family_the_modifier_asks_for() {
        for (bytes, weight) in [(LIGHT_BYTES, 300), (SANS_BYTES, 400), (BOLD_BYTES, 700)] {
            let (family, class) = family_and_weight(bytes);
            assert_eq!(family, SANS, "a face left the {SANS} family");
            assert_eq!(class, weight, "{family} at the wrong weight");
        }
        let (family, class) = family_and_weight(MONO_BYTES);
        assert_eq!(family, MONO);
        assert_eq!(class, 400);
    }

    /// The family `fontdb` would file this face under, and its `OS/2` weight
    /// class. A small `name` and `OS/2` reader — pulling in a font crate to
    /// check the fonts would only move the question.
    fn family_and_weight(bytes: &[u8]) -> (String, u16) {
        let u16_at = |at: usize| u16::from_be_bytes([bytes[at], bytes[at + 1]]);
        let u32_at = |at: usize| {
            u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
        };

        let mut name = None;
        let mut os2 = None;
        for table in 0..usize::from(u16_at(4)) {
            let record = 12 + 16 * table;
            let offset = u32_at(record + 8) as usize;
            match &bytes[record..record + 4] {
                b"name" => name = Some(offset),
                b"OS/2" => os2 = Some(offset),
                _ => {}
            }
        }
        let name = name.expect("no name table");
        let weight = u16_at(os2.expect("no OS/2 table") + 4);

        let count = usize::from(u16_at(name + 2));
        let strings = name + usize::from(u16_at(name + 4));
        // ID 16 wins over ID 1 when it is there, which is what `fontdb` does.
        let mut families: [Option<String>; 2] = [None, None];
        for entry in 0..count {
            let at = name + 6 + 12 * entry;
            let (platform, id) = (u16_at(at), u16_at(at + 6));
            let slot = match id {
                16 => 0,
                1 => 1,
                _ => continue,
            };
            // Windows/Unicode records are UTF-16BE; the Macintosh ones are not.
            if platform != 3 || families[slot].is_some() {
                continue;
            }
            let (length, offset) = (usize::from(u16_at(at + 8)), usize::from(u16_at(at + 10)));
            let raw = &bytes[strings + offset..strings + offset + length];
            let text: String = raw
                .chunks_exact(2)
                .filter_map(|pair| {
                    char::from_u32(u32::from(u16::from_be_bytes([pair[0], pair[1]])))
                })
                .collect();
            families[slot] = Some(text);
        }
        let family = families[0]
            .clone()
            .or_else(|| families[1].clone())
            .expect("no family name");
        (family, weight)
    }
}
