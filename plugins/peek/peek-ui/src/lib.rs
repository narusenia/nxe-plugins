//! The Peek figures. **One drawing per measurement**, used by the plugin that
//! shows only that measurement and by `Peek`, which shows them all
//! (`plugins/peek/docs/implementation/peek-plan.md`, decision 2).
//!
//! Every figure draws into the rect it is given, so the same code serves the
//! Kit window and a cell of Peek's grid.

pub mod bind;
pub mod level;
pub mod loudness;
pub mod scope;
pub mod spectrogram;
pub mod spectrum;
pub mod stereo;
pub mod vu;

use egui::{Align2, Rect, Stroke, Ui, UiBuilder, pos2, vec2};
use nxe_egui::readout::{HEIGHT as READOUT, figure, readout};
use nxe_egui::theme::{FONT_EYEBROW, Palette, SPACE_2, SPACE_4};
use nxe_egui::{font, smooth};

pub use nxe_egui::theme::WARN;

/// How often a silent window looks for sound.
const POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// truce-egui paints only on input or a parameter change, and the hand-offs
/// are neither: ask again at 30 Hz while there is sound, slower when not.
pub fn heartbeat(ui: &Ui, sounding: bool) {
    ui.ctx()
        .request_repaint_after(if sounding { smooth::FRAME } else { POLL });
}

/// An amplitude as decibels; `None` for silence.
pub fn decibels(amplitude: f32) -> Option<f32> {
    (amplitude > 1e-6).then(|| 20.0 * amplitude.log10())
}

/// One cell of a band of readings.
pub struct Reading<'a> {
    pub name: &'a str,
    pub value: Option<f32>,
    pub unit: &'a str,
    /// Drawn dark on the band: the reading that must not be missed.
    pub alarm: bool,
}

pub const CELL: f32 = 72.0;

/// Readings left to right across `rect`, on the band (`ink` is the band's
/// inverted palette, `palette` the window's).
pub fn readings(ui: &mut Ui, palette: &Palette, rect: Rect, cells: &[Reading]) {
    let ink = palette.inverted();
    let mut row = ui.new_child(
        UiBuilder::new()
            .max_rect(Rect::from_min_size(
                pos2(rect.left(), rect.center().y - READOUT * 0.5),
                vec2(rect.width(), READOUT),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    row.spacing_mut().item_spacing = vec2(SPACE_4, 0.0);
    for cell in cells {
        let text = figure(cell.value, 1);
        if cell.alarm {
            let at = row.cursor().min;
            row.painter().rect_filled(
                Rect::from_min_size(at, vec2(CELL, READOUT)).expand(SPACE_2 * 0.5),
                0.0,
                ink.ink.color(),
            );
            readout(&mut row, palette, cell.name, &text, cell.unit, CELL);
        } else {
            readout(&mut row, &ink, cell.name, &text, cell.unit, CELL);
        }
    }
}

/// A small eyebrow naming a figure, at its top left.
pub fn title(ui: &Ui, palette: &Palette, rect: Rect, text: &str) {
    font::tracked(
        ui.painter(),
        rect.left_top() + vec2(SPACE_2, SPACE_2),
        Align2::LEFT_TOP,
        text,
        FONT_EYEBROW,
        palette.muted.color(),
    );
}

/// −1 to +1, filled from the middle. For correlation and balance.
pub fn bipolar(ui: &Ui, palette: &Palette, rect: Rect, value: f32) {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, palette.track.color());
    let middle = rect.center().x;
    let at = middle + value.clamp(-1.0, 1.0) * rect.width() * 0.5;
    painter.rect_filled(
        Rect::from_min_max(
            pos2(middle.min(at), rect.top() + 2.0),
            pos2(middle.max(at), rect.bottom() - 2.0),
        ),
        0.0,
        palette.ink.color(),
    );
    // Quarters marked, the middle strongest.
    for quarter in -2..=2 {
        let x = middle + quarter as f32 * rect.width() * 0.25;
        let (top, colour) = if quarter == 0 {
            (rect.top(), palette.subtle)
        } else {
            (rect.bottom() - 3.0, palette.muted)
        };
        painter.line_segment(
            [pos2(x, top), pos2(x, rect.bottom())],
            Stroke::new(1.0, colour.color()),
        );
    }
}

/// A named −1…+1 bar in a band cell, `width` wide and a readout tall.
pub fn bipolar_cell(ui: &mut Ui, ink: &Palette, name: &str, value: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, READOUT), egui::Sense::hover());
    font::tracked(
        ui.painter(),
        rect.left_top(),
        Align2::LEFT_TOP,
        name,
        FONT_EYEBROW,
        ink.subtle.color(),
    );
    ui.painter().text(
        rect.right_top(),
        Align2::RIGHT_TOP,
        figure(Some(value), 2),
        font::mono(FONT_EYEBROW),
        ink.ink.color(),
    );
    bipolar(
        ui,
        ink,
        Rect::from_min_max(pos2(rect.left(), rect.bottom() - 10.0), rect.right_bottom()),
        value,
    );
}

/// A segmented choice bound to an enum parameter.
pub fn choice<P: truce_params::Params>(
    ui: &mut Ui,
    ctx: &truce_core::editor::PluginContext<P>,
    palette: &Palette,
    id: u32,
    names: &[&str],
    index: usize,
    width: f32,
) {
    if let Some(picked) = nxe_egui::segment::segment(ui, palette, names, index, width) {
        bind::set_plain(ctx, id, picked as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::decibels;

    #[test]
    fn silence_has_no_decibels() {
        assert_eq!(decibels(0.0), None);
        assert!((decibels(1.0).unwrap()).abs() < 1e-6);
    }
}
