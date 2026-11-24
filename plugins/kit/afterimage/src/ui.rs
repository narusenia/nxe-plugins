//! The Afterimage window: the spectra just played as lines, fading with age,
//! and the trace held over them as a line with a glow under it. Stop the
//! sound and the lines die away while the trace stays and fades — the
//! afterimage, seen. The present is not filled: the newest line already is it.

use crate::params::AfterimageParams;
use egui::{Align2, Stroke, Ui, pos2};
use kit_ui::{Factory, Frequency, Kit};
use nxe_egui::theme::{FONT_VALUE, Palette};
use nxe_egui::{font, kit};
use spectral_core::{BANDS, position};

/// The bottom of the figure, in dBFS.
const FLOOR_DB: f32 = -84.0;

/// Spectra kept behind the present one.
const GHOSTS: usize = 12;

/// The present spectrum every so often, newest last: a twelfth of the
/// persistence apart, but no closer than a window frame and no further than a
/// quarter second, so the trail moves at any setting.
fn ghosts(ui: &Ui, present: &[f32; BANDS], persistence: f32) -> Vec<[f32; BANDS]> {
    let id = ui.id().with("afterimage-ghosts");
    let now = ui.input(|input| input.time);
    let (mut last, mut kept): (f64, Vec<[f32; BANDS]>) = ui
        .data(|data| data.get_temp(id))
        .unwrap_or((0.0, Vec::new()));
    let every = f64::from(persistence / GHOSTS as f32).clamp(1.0 / 30.0, 0.25);
    if now - last >= every {
        last = now;
        kept.push(*present);
        if kept.len() > GHOSTS {
            kept.remove(0);
        }
    }
    ui.data_mut(|data| data.insert_temp(id, (last, kept.clone())));
    kept
}

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
    let ghosts = ghosts(ui, &present, p);
    plot.fill(ui, PALETTE.accent.at(0.12), |hz| height(&trace, hz));
    for (age, ghost) in ghosts.iter().rev().enumerate().rev() {
        // Oldest first, so the newer ones lie on top; fainter with age.
        let fade = 1.0 - age as f32 / GHOSTS as f32;
        let colour = PALETTE.accent.at(0.55 * fade * fade);
        plot.curve(ui, colour, 1.0, |hz| height(ghost, hz));
    }
    plot.curve(ui, PALETTE.accent, 2.0, |hz| height(&trace, hz));
}
