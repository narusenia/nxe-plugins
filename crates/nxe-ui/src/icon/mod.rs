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
//! Reference an icon through its constant, never as a raw escape:
//!
//! ```ignore
//! Label::new(cx, nxe_ui::icon::CHEVRON_DOWN).class("icon");
//! ```

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

    #[test]
    fn the_font_is_embedded() {
        assert!(FONT.len() > 100_000, "the font looks truncated");
        // TrueType's magic number.
        assert_eq!(&FONT[..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
