//! The editor. **A placeholder until `V2-7`**, which puts Doubler 2 on the
//! `nxe-ui` egui widgets and the H4 layout; until then the host's generic
//! parameter view is the way to edit.
//!
//! The size is fixed and never asked of the host to change: a window that
//! resized itself froze Ableton in v0 (`docs/HANDOVER.md`).

use crate::params::DoublerParams;
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

/// The window width every NXE plugin shares (`design-system.md`). Moves into
/// `nxe-ui`'s egui tokens in `V2-5`.
const WIDTH: u32 = 880;
const HEIGHT: u32 = 480;

pub fn create(params: Arc<DoublerParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (WIDTH, HEIGHT), view).into_editor()
}

fn view(ui: &mut egui::Ui, _state: &PluginContext<DoublerParams>) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        // The bare name: the host list has already said `NXE` (`AGENTS.md`).
        ui.heading("Doubler");
    });
}
