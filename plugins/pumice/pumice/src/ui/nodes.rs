//! The row under the figure: one node, every one of its four parameters.
//!
//! **The figure is where nodes are placed; this is where they are read and
//! finished.** A drag in the figure is quick and coarse, and a node whose
//! width is narrower than its own dot cannot be widened there at all — so
//! every node parameter is also a bar here, which is what makes all 24 of
//! them reachable without the host's generic view (`every_parameter_is_on_screen`).
//!
//! One node at a time, picked by number or by touching it in the figure:
//! six columns of four would leave each bar 25 px wide.

use super::bind::{self, Ctx};
use egui::{Align2, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::bar::bar;
use nxe_egui::font;
use nxe_egui::segment;
use nxe_egui::theme::{FONT_VALUE, Palette, SPACE_1, SPACE_2, SPACE_3};
use pumice_core::NODES;

const ROW: f32 = 24.0;
const PICKER: f32 = 216.0;
const SWITCH: f32 = 64.0;
const NAME: f32 = 44.0;
const FIGURE: f32 = 64.0;
const NUMBERS: [&str; NODES] = ["1", "2", "3", "4", "5", "6"];
/// `[freq, width, depth]`, by their place in `node_ids`.
const FIELDS: [(&str, usize, bool); 3] =
    [("Freq", 1, false), ("Width", 2, false), ("Depth", 3, true)];

pub const HEIGHT: f32 = segment::HEIGHT + SPACE_2 + ROW;

pub fn nodes(
    ui: &mut Ui,
    ctx: &Ctx,
    palette: &Palette,
    rect: Rect,
    selected: usize,
) -> Option<usize> {
    let params = ctx.params();
    let painter = ui.painter_at(rect.expand(SPACE_1));
    let mut picked = None;

    let mut row = ui.new_child(
        UiBuilder::new().max_rect(Rect::from_min_size(rect.min, vec2(PICKER, segment::HEIGHT))),
    );
    if let Some(index) = segment::segment(&mut row, palette, &NUMBERS, selected, PICKER) {
        picked = Some(index);
    }
    // A tick under every node that is on, so the picker says which exist.
    let cell = PICKER / NODES as f32;
    for index in 0..NODES {
        if bind::plain(ctx, params.node_ids(index)[0]) > 0.5 {
            let x = rect.left() + cell * (index as f32 + 0.5);
            let y = rect.top() + segment::HEIGHT + SPACE_1;
            painter.line_segment(
                [pos2(x - 6.0, y), pos2(x + 6.0, y)],
                Stroke::new(2.0, palette.accent.color()),
            );
        }
    }

    let [on, ..] = params.node_ids(selected);
    let enabled = bind::plain(ctx, on) > 0.5;
    let button = Rect::from_min_size(
        pos2(rect.left() + PICKER + SPACE_3, rect.top()),
        vec2(SWITCH, segment::HEIGHT),
    );
    let response = ui.interact(button, ui.id().with("node-on"), Sense::click());
    if response.clicked() {
        bind::set_once(ctx, on, if enabled { 0.0 } else { 1.0 });
    }
    if enabled {
        painter.rect_filled(button, 0.0, palette.accent.color());
    } else {
        painter.rect_stroke(
            button,
            0.0,
            Stroke::new(
                1.0,
                if response.hovered() {
                    palette.muted.color()
                } else {
                    palette.line.color()
                },
            ),
            egui::StrokeKind::Inside,
        );
    }
    painter.text(
        button.center(),
        Align2::CENTER_CENTER,
        if enabled { "On" } else { "Off" },
        font::sans(FONT_VALUE),
        if enabled {
            palette.ground.color()
        } else {
            palette.muted.color()
        },
    );
    painter.text(
        pos2(button.right() + SPACE_3, button.center().y),
        Align2::LEFT_CENTER,
        "Double-click the figure to add a node, a node to remove it",
        font::sans(FONT_VALUE),
        palette.subtle.color(),
    );

    let y = rect.top() + segment::HEIGHT + SPACE_2;
    let column = rect.width() / FIELDS.len() as f32;
    let ids = params.node_ids(selected);
    for (place, (name, field, bipolar)) in FIELDS.iter().enumerate() {
        let x = rect.left() + column * place as f32;
        let id = ids[*field];
        painter.text(
            pos2(x, y + ROW / 2.0),
            Align2::LEFT_CENTER,
            name,
            font::sans(FONT_VALUE),
            palette.muted.color(),
        );
        let cell = Rect::from_min_size(
            pos2(x + NAME, y),
            vec2(column - NAME - FIGURE - SPACE_3, ROW),
        );
        let value = bind::normalized(ctx, id);
        let mut child = ui.new_child(UiBuilder::new().max_rect(cell));
        let gestures = bar(&mut child, palette, value, *bipolar, cell.size(), 4.0);
        bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(value)]);
        painter.text(
            pos2(cell.right() + FIGURE, y + ROW / 2.0),
            Align2::RIGHT_CENTER,
            ctx.format_param(id),
            font::mono(FONT_VALUE),
            if enabled {
                palette.ink.color()
            } else {
                palette.subtle.color()
            },
        );
    }

    picked
}
