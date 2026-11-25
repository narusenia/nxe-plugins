//! Shared egui widgets and theme tokens for the nxe plugins (v2).
//!
//! **Knows nothing about truce.** A widget takes a value and the text to show,
//! and hands back what the pointer did; binding that to a parameter is each
//! plugin's job. That boundary is what lets `examples/gallery` run as a plain
//! desktop app.
//!
//! Every widget is in `examples/gallery.rs` (`.agents/rules/ui.md`).

pub mod bar;
pub mod colour;
pub mod curve;
pub mod design;
pub mod entry;
pub mod font;
pub mod input;
pub mod kit;
pub mod knob;
pub mod logo;
pub mod meter;
pub mod number;
pub mod preset;
pub mod readout;
pub mod segment;
pub mod select;
pub mod sidebar;
pub mod smooth;
pub mod status;
pub mod surface;
pub mod text;
pub mod theme;
pub mod toggle;

/// Paints the whole window's ground and installs the faces. Call first, every
/// frame, and **draw nothing else when it returns `false`** — the faces are
/// not in force until the next pass ([`font::install`]).
pub fn window(ui: &mut egui::Ui) -> bool {
    let rect = ui.max_rect();
    ui.painter()
        .rect_filled(rect, 0.0, theme::BACKGROUND.color());
    font::install(ui.ctx())
}
