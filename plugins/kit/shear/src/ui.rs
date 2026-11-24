//! The Shear window: what comes out as a spectrogram, and beside it how late
//! each frequency comes out.

use crate::params::ShearParams;
use egui::{Align2, Pos2, Rect, Shape, Stroke, Ui, pos2, vec2};
use kit_ui::{Factory, Kit};
use nxe_egui::font;
use nxe_egui::kit;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2};
use peek_ui::spectrum::Axis;
use spectral_core::shear::delay;
use spectral_core::{HIGH_HZ, LOW_HZ};
use std::sync::Arc;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<ShearParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// Shear, Length, Mix. A first draft; settling them needs an ear.
pub const FACTORY: [Factory; 4] = [
    Factory {
        name: "Default",
        values: &[0.5, 250.0, 1.0],
    },
    Factory {
        name: "Chirp",
        values: &[1.0, 60.0, 1.0],
    },
    Factory {
        name: "Low Lag",
        values: &[-0.8, 400.0, 0.8],
    },
    Factory {
        name: "Smear",
        values: &[1.0, 1000.0, 0.6],
    },
];

pub fn knob_ids(params: &ShearParams) -> [u32; 3] {
    [params.shear.id(), params.length.id(), params.mix.id()]
}

pub fn create(params: Arc<ShearParams>) -> Box<dyn Editor> {
    EguiEditor::new(params, (kit::WIDTH as u32, kit::HEIGHT as u32), view)
        .with_font_definitions(nxe_egui::font::definitions)
        .into_editor()
}

fn view(ui: &mut Ui, ctx: &Ctx) {
    if !nxe_egui::window(ui) {
        return;
    }
    let [shear, length, mix] = knob_ids(ctx.params());
    let figure = kit_ui::window(
        ui,
        ctx,
        &Kit {
            name: "Shear",
            label: "Design Kit",
            product: "Shear",
            plugin_id: "com.nxe.shear",
            palette: &PALETTE,
            knobs: &[(shear, "Shear"), (length, "Length"), (mix, "Mix")],
            factory: &FACTORY,
        },
    );
    let s = kit_ui::plain(ctx, shear) as f32;
    let seconds = kit_ui::plain(ctx, length) as f32 / 1_000.0;

    // Left, what comes out over the last seconds; right, on the same
    // frequency axis, how late each frequency is. A drum hit leaves the
    // spectrogram as the right-hand curve, drawn sideways.
    let split = figure.left() + figure.width() * 0.72;
    let (picture, lean) = (
        Rect::from_min_max(figure.min, pos2(split - SPACE_2, figure.bottom())),
        Rect::from_min_max(pos2(split, figure.top()), figure.max),
    );
    peek_ui::spectrogram::figure(
        ui,
        &PALETTE,
        picture,
        &ctx.params().levels.read(),
        FLOOR_DB,
        CEILING_DB,
        Axis::Log,
    );

    let painter = ui.painter_at(lean.expand(1.0));
    let line = Stroke::new(1.0, PALETTE.line.color());
    painter.rect_stroke(lean, 0.0, line, egui::StrokeKind::Inside);
    let top = seconds.max(f32::EPSILON);
    const STEPS: usize = 120;
    let points: Vec<Pos2> = (0..=STEPS)
        .map(|step| {
            let at = step as f32 / STEPS as f32;
            let hz = LOW_HZ * (HIGH_HZ / LOW_HZ).powf(at);
            pos2(
                lean.left() + delay(hz, s, seconds) / top * lean.width(),
                lean.bottom() - at * lean.height(),
            )
        })
        .collect();
    painter.add(Shape::line(
        points,
        Stroke::new(2.0, PALETTE.accent.color()),
    ));
    painter.text(
        lean.right_bottom() - vec2(3.0, 2.0),
        Align2::RIGHT_BOTTOM,
        format!("{:.0} ms", seconds * 1_000.0),
        font::mono(FONT_VALUE),
        PALETTE.muted.color(),
    );
}

/// Where the spectrogram starts to show and where it is full.
const FLOOR_DB: f32 = -72.0;
const CEILING_DB: f32 = -12.0;
