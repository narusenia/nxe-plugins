//! The embedded typefaces, and the three ways text is set.
//!
//! **Words are Inter, figures are Geist Mono** — both SIL OFL 1.1, licence text
//! beside the files in `assets/`, and it **has to travel with any release
//! bundle** because the faces are compiled in.
//!
//! **Two weights of Inter in v2, Regular and Medium** (`design-system.md`). A
//! weight still carries no meaning on its own — Medium is for the wordmark,
//! the subject figure and the eyebrows, where size and tracking already say
//! what the text is.
//!
//! **Figures stay in the mono face**: a value that changes width between a `1`
//! and an `8` jitters under a drag.

use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect};
use std::sync::Arc;

pub const INTER: &[u8] = include_bytes!("../assets/inter/Inter-Regular.ttf");
pub const INTER_MEDIUM: &[u8] = include_bytes!("../assets/inter/Inter-Medium.ttf");
pub const GEIST_MONO: &[u8] = include_bytes!("../assets/geist/GeistMono-Regular.ttf");

/// The family name Inter Medium is registered under.
pub const MEDIUM: &str = "nxe-medium";

/// The font definitions every window uses. egui's own faces stay behind ours
/// as a fallback for any glyph Inter does not have.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [
        ("inter", INTER),
        ("inter-medium", INTER_MEDIUM),
        ("geist-mono", GEIST_MONO),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(data)));
    }
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .expect("egui always defines the proportional family")
        .insert(0, "inter".into());
    fonts
        .families
        .get_mut(&FontFamily::Monospace)
        .expect("egui always defines the monospace family")
        .insert(0, "geist-mono".into());
    let mut medium = vec!["inter-medium".to_owned()];
    medium.extend(fallback);
    fonts
        .families
        .insert(FontFamily::Name(MEDIUM.into()), medium);
    fonts
}

/// Installs the faces, and says whether they are in force yet.
///
/// **egui applies new fonts from the next pass**, and a family that is not
/// bound yet panics the moment text asks for it. So the first pass only asks
/// for them and returns `false`; the caller draws nothing but the ground and
/// gets a second pass straight away. Cheap to call every frame.
pub fn install(ctx: &egui::Context) -> bool {
    let family = FontFamily::Name(MEDIUM.into());
    if ctx.fonts(|fonts| fonts.definitions().families.contains_key(&family)) {
        return true;
    }
    ctx.set_fonts(definitions());
    ctx.request_repaint();
    false
}

pub fn sans(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MEDIUM.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Tracked capitals: an eyebrow, or the `NXE` mark. Returns the rect drawn.
pub fn tracked(
    painter: &Painter,
    pos: Pos2,
    anchor: Align2,
    text: &str,
    size: f32,
    color: Color32,
) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        TextFormat {
            font_id: medium(size),
            color,
            extra_letter_spacing: crate::theme::TRACKING,
            ..Default::default()
        },
    );
    let galley = painter.layout_job(job);
    let rect = anchor.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}
