//! Font loading for egui contexts.

use std::sync::Arc;

/// nxe-plugins patch: what an editor installs before its first pass. One face
/// for the proportional and monospace families (upstream's `with_font`), or a
/// whole `FontDefinitions` — several faces, named families — built by a
/// function so it stays `Copy` and `Send`.
#[derive(Clone, Copy)]
pub enum Font {
    Data(&'static [u8]),
    Definitions(fn() -> egui::FontDefinitions),
}

/// Load a TrueType font as the default proportional and monospace font.
///
/// Call on an `egui::Context` before rendering. Used by both the live
/// editor and the snapshot renderer.
///
/// # Panics
///
/// Cannot panic in practice - egui's `FontDefinitions::default()`
/// always populates the `Proportional` and `Monospace` families, so
/// the `unwrap()`s succeed. A future egui release that drops either
/// family would surface as a panic here rather than a silent miss.
pub fn apply_font(ctx: &egui::Context, font: Font) {
    // nxe-plugins patch: a whole set of definitions, not only one face.
    let font_data = match font {
        Font::Data(data) => data,
        Font::Definitions(definitions) => {
            ctx.set_fonts(definitions());
            return;
        }
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "truce_default".to_owned(),
        Arc::new(egui::FontData::from_static(font_data)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .get_mut(&family)
            .unwrap()
            .insert(0, "truce_default".to_owned());
    }
    ctx.set_fonts(fonts);
}
