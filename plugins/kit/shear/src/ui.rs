//! The Shear window: what comes out as a spectrogram, and beside it, on the
//! same frequency axis, how late each frequency comes out — drawn as a fan of
//! lines, each as long and as bright as its delay. A drum hit leaves the
//! spectrogram in the fan's shape.

use crate::params::{RING, ShearParams};
use egui::{Align2, Pos2, Rect, Shape, Stroke, Ui, pos2, vec2};
use kit_ui::{Factory, Kit};
use nxe_egui::font;
use nxe_egui::kit;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_2};
use peek_ui::spectrum::Axis;
use spectral_core::shear::delay;
use spectral_core::{BANDS, FRAMES_PER_SECOND, HIGH_HZ, LOW_HZ, position};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use truce::prelude::*;
use truce_core::editor::PluginContext;
use truce_egui::EguiEditor;

type Ctx = PluginContext<ShearParams>;

const PALETTE: Palette = Palette::DESIGN_KIT;

/// What the spectrogram covers: short, so a lean of tens of ms is wide
/// enough to see.
const SECONDS: f32 = 2.0;
/// Where the spectrogram starts to show and where it is full.
const FLOOR_DB: f32 = -72.0;
const CEILING_DB: f32 = -12.0;
/// The fan's lines, low to high.
const TEETH: usize = 28;

/// Shear, Length, Mix. Checked by ear on 2026-11-26.
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
    let params = ctx.params();
    let [shear, length, mix] = knob_ids(params);
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

    let split = figure.left() + figure.width() * 0.7;
    let picture = Rect::from_min_max(figure.min, pos2(split - SPACE_2, figure.bottom()));
    let lean = Rect::from_min_max(pos2(split, figure.top()), figure.max);
    let frames = frames(ui, params);
    peek_ui::spectrogram::figure_frames(
        ui,
        &PALETTE,
        picture,
        &frames,
        FLOOR_DB,
        CEILING_DB,
        Axis::Log,
        (SECONDS * FRAMES_PER_SECOND).round() as usize,
        SECONDS,
    );
    fan(ui, lean, s, seconds);
}

/// The frames that arrived since the last window frame, oldest first. When
/// none have for a while — bypassed, or the transport stopped — silent ones
/// at the frame rate, so the picture keeps moving and empties.
fn frames(ui: &Ui, params: &ShearParams) -> Vec<[f32; BANDS]> {
    let ring = params.ring.read();
    let (new, quiet) = kit_ui::arrivals(ui, "shear-frames", params.count.load(Ordering::Relaxed));
    let clock = ui.id().with("shear-silence");
    let now = ui.input(|input| input.time);
    if new > 0 || quiet < kit_ui::STALE_SECONDS {
        ui.data_mut(|data| data.insert_temp(clock, now));
        let new = (new as usize).min(RING);
        return ring
            .chunks_exact(BANDS)
            .skip(RING - new)
            .map(|chunk| std::array::from_fn(|band| chunk[band]))
            .collect();
    }
    let since: f64 = ui.data(|data| data.get_temp(clock)).unwrap_or(now);
    let due = ((now - since) * f64::from(FRAMES_PER_SECOND)) as usize;
    if due > 0 {
        ui.data_mut(|data| {
            data.insert_temp(clock, since + due as f64 / f64::from(FRAMES_PER_SECOND))
        });
    }
    vec![[0.0; BANDS]; due.min(RING * 4)]
}

/// How late each frequency is, frequency up as in the spectrogram beside it
/// and delay across: a line per frequency from the left edge out to its
/// delay, brighter the later, with the curve along their ends.
fn fan(ui: &Ui, rect: Rect, shear: f32, seconds: f32) {
    let painter = ui.painter_at(rect.expand(1.0));
    let top = seconds.max(f32::EPSILON);
    let x = |hz: f32| rect.left() + delay(hz, shear, seconds) / top * rect.width();
    let y = |hz: f32| rect.bottom() - position(hz) * rect.height();
    let hz_at = |at: f32| LOW_HZ * (HIGH_HZ / LOW_HZ).powf(at);

    // The frequency lines the spectrogram labels, and the time halves.
    let faint = Stroke::new(1.0, PALETTE.line.at(0.5).color());
    for hz in [100.0, 1_000.0, 10_000.0] {
        painter.line_segment([pos2(rect.left(), y(hz)), pos2(rect.right(), y(hz))], faint);
    }
    for half in [0.5, 1.0] {
        let at = rect.left() + half * rect.width();
        painter.line_segment([pos2(at, rect.top()), pos2(at, rect.bottom())], faint);
    }

    for tooth in 0..TEETH {
        let hz = hz_at((tooth as f32 + 0.5) / TEETH as f32);
        let reach = delay(hz, shear, seconds) / top;
        painter.line_segment(
            [pos2(rect.left(), y(hz)), pos2(x(hz), y(hz))],
            Stroke::new(2.0, PALETTE.accent.at(0.15 + 0.6 * reach).color()),
        );
    }

    const STEPS: usize = 120;
    let points: Vec<Pos2> = (0..=STEPS)
        .map(|step| hz_at(step as f32 / STEPS as f32))
        .map(|hz| pos2(x(hz), y(hz)))
        .collect();
    painter.add(Shape::line(
        points,
        Stroke::new(2.0, PALETTE.accent.color()),
    ));
    // The latest frequency, marked where it ends.
    let latest = if shear >= 0.0 { HIGH_HZ } else { LOW_HZ };
    painter.circle_filled(pos2(x(latest), y(latest)), 3.5, PALETTE.accent.color());

    let labels = font::mono(FONT_VALUE);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, PALETTE.line.color()),
    );
    painter.text(
        rect.left_bottom() + vec2(2.0, -2.0),
        Align2::LEFT_BOTTOM,
        "0",
        labels.clone(),
        PALETTE.subtle.color(),
    );
    painter.text(
        rect.right_bottom() - vec2(2.0, 2.0),
        Align2::RIGHT_BOTTOM,
        format!("{:.0} ms", seconds * 1_000.0),
        labels,
        PALETTE.muted.color(),
    );
}
