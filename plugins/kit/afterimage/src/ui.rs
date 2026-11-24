//! The Afterimage window: the spectrum now, filled, and the trace held over
//! it as a line. Stop the sound and the fill drops while the line stays and
//! fades — the afterimage, seen.

use crate::params::AfterimageParams;
use egui::{Align2, Stroke, Ui, pos2};
use kit_ui::{Factory, Frequency, Kit};
use nxe_egui::theme::{FONT_VALUE, Palette};
use nxe_egui::{font, kit};
use spectral_core::{BANDS, position};

/// The bottom of the figure, in dBFS.
const FLOOR_DB: f32 = -84.0;

/// The level at `hz` from band levels, as `0..=1` of the figure's height.
fn height(levels: &[f32; BANDS], hz: f32) -> f32 {
    let band = ((position(hz) * BANDS as f32) as usize).min(BANDS - 1);
    let amplitude = levels[band];
    if amplitude <= 0.0 {
        return 0.0;
    }
    ((20.0 * amplitude.log10() - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0)
}
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<AfterimageParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Persistence, Damping, Mix. A first draft; settling them needs an ear.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[3.0, 0.5, 0.5],
    },
    Factory {
        name: "Glow",
        values: &[1.0, 0.7, 0.4],
    },
    Factory {
        name: "Haunt",
        values: &[8.0, 0.3, 0.6],
    },
    Factory {
        name: "Frozen",
        values: &[20.0, 0.0, 0.8],
    },
];

pub fn knob_ids(params: &AfterimageParams) -> [u32; 3] {
    [
        params.persistence.id(),
        params.damping.id(),
        params.mix.id(),
    ]
}

pub fn create(params: Arc<AfterimageParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let [persistence, damping, mix] = knob_ids(ctx.params());
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Afterimage",
            label: "Design Kit",
            product: "Afterimage",
            plugin_id: "com.nxe.afterimage",
            palette: &PALETTE,
            knobs: &[
                (persistence, "Persistence"),
                (damping, "Damping"),
                (mix, "Mix"),
            ],
            factory: &FACTORY,
        },
    );
    let _ = damping;
    let p = kit_ui::plain(ctx, persistence) as f32;
    let params = ctx.params();
    // Bypassed or stopped, the last frame would stay up: show nothing then.
    let (_, quiet) = kit_ui::arrivals(
        ui,
        "afterimage-frames",
        params.count.load(std::sync::atomic::Ordering::Relaxed),
    );
    let (present, trace) = if quiet < kit_ui::STALE_SECONDS {
        (params.present.read(), params.trace.read())
    } else {
        ([0.0; BANDS], [0.0; BANDS])
    };
    let plot = Frequency::amount(ui, &PALETTE, figure, 1.0, &format!("{p:.1} s"));
    let painter = ui.painter_at(figure);
    for db in [-24.0f32, -48.0, -72.0] {
        let y = figure.bottom() - (db - FLOOR_DB) / -FLOOR_DB * figure.height();
        painter.line_segment(
            [pos2(figure.left(), y), pos2(figure.right(), y)],
            Stroke::new(1.0, PALETTE.line.at(0.5).color()),
        );
        painter.text(
            pos2(figure.right() - 3.0, y - 1.0),
            Align2::RIGHT_BOTTOM,
            format!("{db:.0}"),
            font::mono(FONT_VALUE),
            PALETTE.subtle.color(),
        );
    }
    plot.fill(ui, PALETTE.line, |hz| height(&present, hz));
    plot.curve(ui, PALETTE.accent, 2.0, |hz| height(&trace, hz));
}
