//! The editor. **A placeholder until the H4 window** (`V2-10`); until then the
//! host's generic parameter view is the way to edit.
//!
//! The size is fixed and never asked of the host to change: a window that
//! resized itself froze Ableton in v0 (`docs/HANDOVER.md`).

use crate::params::VelourParams;
use nxe_egui::theme::WINDOW_WIDTH;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

const HEIGHT: u32 = 430;

pub fn create(params: Arc<VelourParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (WINDOW_WIDTH, HEIGHT), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut egui::Ui, _ctx: &PluginContext<VelourParams>) {
    if !nxe_egui::window(ui) {
        return;
    }
    // The bare name: the host list has already said `NXE` (`AGENTS.md`).
    ui.heading("Velour");
}
