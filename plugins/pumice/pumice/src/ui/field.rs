//! The figure, on the accent ground: what is arriving, what is being taken
//! out, and where the user said it may happen.
//!
//! **The figure's subject is the reduction** (`REQ-PUM-013`). Pumice works
//! automatically, and a process that works automatically and cannot be seen
//! reads as one that is not working (`REQ-SPK-008`). Three curves, drawn as
//! three kinds of object so they do not read as three settings of one thing:
//!
//! - **the spectrum** arriving, a wash from the floor
//! - **the reduction**, hanging from the ceiling — what the engine took out,
//!   per frequency, straight from `Engine::curves`
//! - **the weight**, the one stroked line: what the user asked for, sampled
//!   from `pumice_core::nodes::weight_at`, the function the engine is tuned
//!   from. It moves with nothing playing (`v2-plan.md`, decision 18)
//!
//! **The nodes are the only points in the line that are placed by hand.**
//! Drag one to move its frequency and depth, drag the marks at its ends to
//! widen it, double-click the empty figure to add one and a node to remove
//! it. The range's two edges are the lines at either side; drag them.

use super::bind::{self, Ctx};
use crate::params::{NARROWEST_OCTAVES, WIDEST_OCTAVES};
use egui::{Align2, Painter, Pos2, Rect, Sense, Shape, Stroke, Ui, pos2, vec2};
use nxe_egui::font;
use nxe_egui::smooth;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use pumice_core::display::{HIGH_HZ, LOW_HZ, point_hz};
use pumice_core::{CURVE_POINTS, NODES, Settings};

/// The spectrum's window, in dB.
pub const SPECTRUM_FLOOR_DB: f32 = -84.0;
/// How deep the reduction can hang: the engine's ceiling, so the fill uses
/// the whole plot and never clips (`DIO-17`).
pub const REDUCTION_FLOOR_DB: f32 = -24.0;

const LABEL: f32 = 16.0;
const LEVELS_WIDTH: f32 = 24.0;
const RADIUS: f32 = 7.0;
/// Half a width grip's size.
const GRIP_X: f32 = 5.0;
const GRIP_Y: f32 = 9.0;
const MARKS: [(f32, &str); 6] = [
    (50.0, "50"),
    (100.0, "100"),
    (500.0, "500"),
    (1_000.0, "1k"),
    (5_000.0, "5k"),
    (10_000.0, "10k"),
];
const LEVELS: [(f32, &str); 4] = [(0.0, "0"), (-24.0, "-24"), (-48.0, "-48"), (-72.0, "-72")];

/// Frequency → `0..=1` across the figure, log. **The same axis as every
/// frequency parameter's range** (`log(20, 20000)`), so a position here is
/// that parameter's normalized value.
pub fn x_of(hz: f32) -> f32 {
    ((hz.max(LOW_HZ) / LOW_HZ).ln() / (HIGH_HZ / LOW_HZ).ln()).clamp(0.0, 1.0)
}

pub fn hz_of(x: f32) -> f32 {
    LOW_HZ * (HIGH_HZ / LOW_HZ).powf(x.clamp(0.0, 1.0))
}

/// A node's width from how far the pointer is from its centre, either side.
pub fn width_from(centre_hz: f32, pointer_hz: f32) -> f32 {
    (2.0 * (pointer_hz / centre_hz).log2().abs()).clamp(NARROWEST_OCTAVES, WIDEST_OCTAVES)
}

/// The weight curve, `0..=1` both ways, `0.5` the resting line.
pub fn weight(params: &crate::params::PumiceParams) -> Vec<(f32, f32)> {
    let nodes = params.nodes();
    let range = params.range();
    let edge = Settings::DEFAULT.edge_octaves;
    (0..CURVE_POINTS)
        .map(|index| {
            let hz = point_hz(index);
            let weight = pumice_core::nodes::weight_at(hz, &nodes, range, edge);
            (x_of(hz), (weight * 0.5).clamp(0.0, 1.0))
        })
        .collect()
}

/// Draws the figure and handles its gestures. Returns the node the pointer
/// picked, added or dragged, for the row under the figure to edit.
#[allow(clippy::too_many_lines)]
pub fn field(
    ui: &mut Ui,
    ctx: &Ctx,
    painter: &Painter,
    rect: Rect,
    ink: &Palette,
    selected: usize,
) -> Option<usize> {
    let params = ctx.params();
    let plot = Rect::from_min_max(
        rect.min + vec2(SPACE_3, SPACE_3),
        rect.max - vec2(SPACE_3 + LEVELS_WIDTH, LABEL),
    );
    let x_at = |x: f32| plot.left() + plot.width() * x;
    let y_at = |y: f32| plot.bottom() - plot.height() * y;
    let mut picked = None;

    // The grid, and its labels.
    let hair = Stroke::new(1.0, ink.line.color());
    for (hz, text) in MARKS {
        let x = x_at(x_of(hz));
        painter.line_segment([pos2(x, plot.top()), pos2(x, plot.bottom())], hair);
        painter.text(
            pos2(x, rect.bottom() - SPACE_1),
            Align2::CENTER_BOTTOM,
            text,
            font::mono(FONT_EYEBROW),
            ink.subtle.color(),
        );
    }
    for (db, text) in LEVELS {
        let y = y_at(level_of(db));
        painter.line_segment([pos2(plot.left(), y), pos2(plot.right(), y)], hair);
        painter.text(
            pos2(rect.right() - SPACE_2, y),
            Align2::RIGHT_CENTER,
            text,
            font::mono(FONT_EYEBROW),
            ink.subtle.color(),
        );
    }

    // What is arriving, as a wash from the floor.
    let spectrum: Vec<f32> = params.analysis.spectrum.read().map(level_of).to_vec();
    let spectrum = smooth::smoothed(ui, ui.id().with("spectrum"), &spectrum, smooth::TAU);
    // **Gated on the meters, not on the spectrum**: before the first block the
    // handoff holds zeros, which read as 0 dB everywhere.
    let sounding = params.analysis.peaks.read().iter().any(|peak| *peak > 1e-4);
    if sounding {
        nxe_egui::curve::curve(
            painter,
            plot,
            &on_grid(&spectrum),
            ink.subtle,
            Some(ink.dim),
            false,
        );
    }

    // What is being taken out, hanging from the ceiling.
    let reduction: Vec<f32> = params
        .analysis
        .reduction
        .read()
        .map(|db| (db / REDUCTION_FLOOR_DB).clamp(0.0, 1.0))
        .to_vec();
    let reduction = smooth::smoothed(ui, ui.id().with("reduction"), &reduction, smooth::TAU);
    let step = plot.width() / (CURVE_POINTS - 1) as f32;
    for (index, depth) in reduction.iter().enumerate() {
        if *depth > 0.002 {
            let x = x_at(x_of(point_hz(index)));
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(x - step / 2.0, plot.top()),
                    pos2(x + step / 2.0, plot.top() + plot.height() * depth),
                ),
                0.0,
                ink.deep.color(),
            );
        }
    }

    // Outside the range, shaded; the edges are handles.
    let ids = [params.low.id(), params.high.id()];
    let edges = [
        x_at(x_of(bind::plain(ctx, ids[0]))),
        x_at(x_of(bind::plain(ctx, ids[1]))),
    ];
    let shade = ink.track.color();
    painter.rect_filled(
        Rect::from_min_max(plot.left_top(), pos2(edges[0], plot.bottom())),
        0.0,
        shade,
    );
    painter.rect_filled(
        Rect::from_min_max(pos2(edges[1], plot.top()), plot.right_bottom()),
        0.0,
        shade,
    );

    // The background takes the double-click that adds a node. **Before the
    // edges and the nodes**, so they sit on top of it for the pointer.
    let background = ui.interact(plot, ui.id().with("plot"), Sense::click());
    if background.double_clicked()
        && let Some(pointer) = background.interact_pointer_pos()
    {
        let free = (0..NODES).find(|index| bind::plain(ctx, params.node_ids(*index)[0]) < 0.5);
        if let Some(index) = free {
            let [on, freq, width, depth] = params.node_ids(index);
            let x = ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0);
            let y = ((plot.bottom() - pointer.y) / plot.height()).clamp(0.0, 1.0);
            bind::set_once(ctx, freq, f64::from(x));
            bind::set_once(ctx, depth, f64::from(y));
            bind::set_once(ctx, width, bind::default_of(ctx, width));
            bind::set_once(ctx, on, 1.0);
            picked = Some(index);
        }
    }

    for (side, (id, x)) in ids.iter().zip(edges).enumerate() {
        let handle =
            Rect::from_center_size(pos2(x, plot.center().y), vec2(SPACE_2 * 2.0, plot.height()));
        let response = ui.interact(handle, ui.id().with(("edge", side)), Sense::drag());
        drag(ctx, &[*id], &response, |pointer| {
            [((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0)]
        });
        let strong = response.hovered() || response.dragged();
        painter.line_segment(
            [pos2(x, plot.top()), pos2(x, plot.bottom())],
            Stroke::new(if strong { 2.0 } else { 1.0 }, ink.ink.color()),
        );
        painter.text(
            pos2(x + if side == 0 { SPACE_1 } else { -SPACE_1 }, plot.top()),
            if side == 0 {
                Align2::LEFT_TOP
            } else {
                Align2::RIGHT_TOP
            },
            ctx.format_param(*id),
            font::mono(FONT_EYEBROW),
            ink.muted.color(),
        );
    }

    // What the user asked for: the resting line, and the weight through it.
    painter.line_segment(
        [pos2(plot.left(), y_at(0.5)), pos2(plot.right(), y_at(0.5))],
        Stroke::new(1.0, ink.muted.at(0.5).color()),
    );
    let line: Vec<Pos2> = weight(params)
        .into_iter()
        .map(|(x, y)| pos2(x_at(x), y_at(y)))
        .collect();
    painter.add(Shape::line(line, Stroke::new(2.0, ink.ink.color())));

    // The nodes.
    let mut hovered = None;
    for index in 0..NODES {
        let [on, freq_id, width_id, depth_id] = params.node_ids(index);
        if bind::plain(ctx, on) < 0.5 {
            continue;
        }
        let freq = bind::plain(ctx, freq_id);
        let width = bind::plain(ctx, width_id);
        let centre = pos2(
            x_at(bind::normalized(ctx, freq_id)),
            y_at(bind::normalized(ctx, depth_id)),
        );
        let half = width / 2.0;
        let ends = [
            x_at(x_of(freq * 2f32.powf(-half))),
            x_at(x_of(freq * 2f32.powf(half))),
        ];
        let current = index == selected;
        let span = Stroke::new(
            if current { 2.0 } else { 1.0 },
            ink.ink.at(if current { 1.0 } else { 0.6 }).color(),
        );
        painter.line_segment([pos2(ends[0], centre.y), pos2(ends[1], centre.y)], span);

        // The ends first, so the node wins where a narrow one overlaps them.
        for (end, x) in ends.iter().enumerate() {
            let grip = Rect::from_center_size(pos2(*x, centre.y), vec2(GRIP_X * 2.0, GRIP_Y * 2.0));
            painter.line_segment(
                [
                    pos2(*x, centre.y - GRIP_Y / 1.5),
                    pos2(*x, centre.y + GRIP_Y / 1.5),
                ],
                span,
            );
            let response = ui.interact(grip, ui.id().with(("grip", index, end)), Sense::drag());
            if response.drag_started() {
                picked = Some(index);
            }
            drag(ctx, &[width_id], &response, |pointer| {
                let octaves = width_from(freq, hz_of((pointer.x - plot.left()) / plot.width()));
                [bind::normalize(ctx, width_id, f64::from(octaves)) as f32]
            });
        }

        let response = ui.interact(
            Rect::from_center_size(centre, vec2(RADIUS * 3.0, RADIUS * 3.0)),
            ui.id().with(("node", index)),
            Sense::click_and_drag(),
        );
        if response.double_clicked() {
            bind::set_once(ctx, on, 0.0);
            continue;
        }
        if response.clicked() || response.drag_started() {
            picked = Some(index);
        }
        drag(ctx, &[freq_id, depth_id], &response, |pointer| {
            [
                ((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0),
                ((plot.bottom() - pointer.y) / plot.height()).clamp(0.0, 1.0),
            ]
        });
        if response.hovered() || response.dragged() {
            hovered = Some((index, centre));
        }

        if current {
            painter.circle_filled(centre, RADIUS, ink.ink.color());
        } else {
            painter.circle_filled(centre, RADIUS, ink.ground.color());
            painter.circle_stroke(centre, RADIUS, Stroke::new(1.5, ink.ink.color()));
        }
        painter.text(
            centre,
            Align2::CENTER_CENTER,
            (index + 1).to_string(),
            font::mono(FONT_EYEBROW),
            if current {
                ink.ground.color()
            } else {
                ink.ink.color()
            },
        );
    }

    // The hovered node's numbers, beside it: the one thing the figure cannot
    // draw.
    if let Some((index, centre)) = hovered {
        let [_, freq, width, depth] = params.node_ids(index);
        let text = format!(
            "{}  {}  {}",
            ctx.format_param(freq),
            ctx.format_param(width),
            ctx.format_param(depth)
        );
        let above = centre.y - plot.top() > 2.0 * RADIUS + FONT_VALUE;
        painter.text(
            pos2(
                centre.x.clamp(plot.left() + 80.0, plot.right() - 80.0),
                if above {
                    centre.y - RADIUS - SPACE_1
                } else {
                    centre.y + RADIUS + SPACE_1
                },
            ),
            if above {
                Align2::CENTER_BOTTOM
            } else {
                Align2::CENTER_TOP
            },
            text,
            font::mono(FONT_VALUE),
            ink.ink.color(),
        );
    }

    picked
}

/// A drag on `response` writing `ids` from where the pointer is, bracketed by
/// `begin_edit` / `end_edit` so a host records one move.
fn drag<const N: usize>(
    ctx: &Ctx,
    ids: &[u32; N],
    response: &egui::Response,
    to: impl Fn(Pos2) -> [f32; N],
) {
    if response.drag_started() {
        for id in ids {
            ctx.begin_edit(*id);
        }
    }
    if response.dragged()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        for (id, value) in ids.iter().zip(to(pointer)) {
            ctx.set_param(*id, f64::from(value));
        }
    }
    if response.drag_stopped() {
        for id in ids {
            ctx.end_edit(*id);
        }
    }
}

/// A level in dB as a height in the figure.
fn level_of(db: f32) -> f32 {
    ((db - SPECTRUM_FLOOR_DB) / -SPECTRUM_FLOOR_DB).clamp(0.0, 1.0)
}

/// A published curve on the figure's own axis.
fn on_grid(levels: &[f32]) -> Vec<(f32, f32)> {
    levels
        .iter()
        .enumerate()
        .map(|(index, level)| (x_of(point_hz(index)), *level))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::PumiceParams;
    use truce_params::Params;

    /// The figure's axis is the frequency parameters' own normalization, so
    /// a drag can write the pointer's position straight through.
    #[test]
    fn the_axis_is_the_parameters() {
        let params = PumiceParams::default();
        let id = params.node1.freq.id();
        for hz in [20.0_f32, 200.0, 2_500.0, 20_000.0] {
            params.set_plain(id, f64::from(hz));
            let normalized = params.get_normalized(id).unwrap() as f32;
            assert!((normalized - x_of(hz)).abs() < 1e-4, "{hz} Hz");
            assert!((hz_of(x_of(hz)) / hz).log2().abs() < 1e-4);
        }
    }

    #[test]
    fn a_width_has_no_side_and_stays_in_range() {
        assert!((width_from(1_000.0, 2_000.0) - 2.0).abs() < 1e-4);
        assert!((width_from(1_000.0, 500.0) - 2.0).abs() < 1e-4);
        assert_eq!(width_from(1_000.0, 1_000.0), NARROWEST_OCTAVES);
        assert_eq!(width_from(1_000.0, 20_000.0), WIDEST_OCTAVES);
    }

    /// **Decision 18**: placing a node moves the picture with nothing
    /// playing.
    #[test]
    fn a_node_bends_the_weight() {
        let params = PumiceParams::default();
        let flat = weight(&params);
        params.set_plain(params.node3.enabled.id(), 1.0);
        params.set_plain(params.node3.depth.id(), 1.0);
        let bent = weight(&params);
        let moved = flat
            .iter()
            .zip(&bent)
            .map(|(a, b)| (a.1 - b.1).abs())
            .fold(0.0f32, f32::max);
        assert!(moved > 0.2, "the weight moved by {moved}");
    }
}
