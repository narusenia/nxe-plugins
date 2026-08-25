//! The embedded Lucide icon font.
//!
//! Vizia cannot read an SVG, and the vizia revision `nih_plug_vizia` pins is
//! from 2024, so waiting for one is not a plan. Lucide publishes its icons as a
//! font, which sidesteps the whole question: an icon is a glyph, its colour is
//! `color` and its size is `font-size`.
//!
//! The trade is that **stroke width is not adjustable** — the strokes were
//! baked into filled glyphs — so an icon that needs a variable stroke has to be
//! drawn as a path in `View::draw`. That is a documented exception, not the
//! default (`.agents/rules/vizia.md`).
//!
//! Reference an icon through its constant and build it with [`label`], never
//! as a raw escape and never by setting the family in CSS:
//!
//! ```ignore
//! icon::label(cx, icon::CHEVRON_DOWN).font_size(20.0);
//! ```
//!
//! **`font-family` in a stylesheet does not select this font** on the vizia
//! revision `nih_plug_vizia` pins. The declaration parses and the conversion to
//! cosmic-text's family list looks correct on inspection, but the glyphs come
//! out of a fallback face — which for private use codepoints means whatever CJK
//! font the system reaches for, so the failure looks like garbage rather than
//! like a missing font. Setting the family through the modifier works, so
//! [`label`] does that and the `.icon` class carries only the colour.

use vizia::prelude::*;

mod generated;

pub use generated::*;

/// The font itself, ISC licensed. Its licence text sits beside it in
/// `assets/lucide/` and **has to travel with any release bundle**, because the
/// face is compiled into the binary.
///
/// All 2035 icons are here, which is 859 KB in every plugin. Subsetting to the
/// handful actually used would need `fonttools` in the loop and a regeneration
/// step every time a new icon is wanted; the size is not worth that.
pub const FONT: &[u8] = include_bytes!("../../assets/lucide/lucide.ttf");

/// The family name to select the font in CSS. The `.icon` class does this.
pub const FAMILY: &str = "lucide";

/// Registers the font. Call once when the window is built; `theme::install`
/// already does.
pub fn install(cx: &mut Context) {
    cx.add_font_mem(FONT);
}

/// A label in the icon font.
///
/// Size and colour are still `font-size` and `color` — only the family has to
/// come from here (see the module docs). The `.icon` class gives the default
/// muted colour; override it per call where an icon should be brighter.
pub fn label<'a>(cx: &'a mut Context, glyph: &str) -> Handle<'a, Label> {
    Label::new(cx, glyph)
        .font_family(vec![FamilyOwned::Name(FAMILY.to_owned())])
        .class("icon")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated constants have to be single characters in the private use
    /// area. Anything else means the generator picked up the wrong field.
    #[test]
    fn the_constants_are_private_use_characters() {
        for (name, glyph) in [
            ("CHEVRON_DOWN", CHEVRON_DOWN),
            ("CHEVRON_UP", CHEVRON_UP),
            ("SLIDERS_HORIZONTAL", SLIDERS_HORIZONTAL),
            ("ROTATE_CCW", ROTATE_CCW),
        ] {
            let mut characters = glyph.chars();
            let character = characters
                .next()
                .unwrap_or_else(|| panic!("{name} is empty"));
            assert!(
                characters.next().is_none(),
                "{name} is more than one character"
            );
            assert!(
                ('\u{e000}'..='\u{f8ff}').contains(&character),
                "{name} is {character:?}, outside the private use area"
            );
        }
    }

    /// The `.icon` class must not try to set the family: that path silently
    /// renders fallback glyphs, and `label` is the reason it does not have to.
    #[test]
    fn the_icon_class_does_not_claim_to_set_the_family() {
        let css = crate::theme::stylesheet();
        let rule = css
            .split_once(".icon {")
            .expect("no .icon rule")
            .1
            .split_once('}')
            .expect("unterminated .icon rule")
            .0;
        assert!(
            !rule.contains("font-family"),
            "the .icon rule sets font-family, which does not work here"
        );
    }

    #[test]
    fn the_font_is_embedded() {
        assert!(FONT.len() > 100_000, "the font looks truncated");
        // TrueType's magic number.
        assert_eq!(&FONT[..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
