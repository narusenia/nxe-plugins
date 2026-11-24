//! The voice table: the shape layer, one row per voice.
//!
//! **The bar is the shape (normalized), the figure is the effective value**
//! (`specifications/ui.md`). Turning the `Detune` macro leaves the bars where
//! they are and moves the figures. Voices that are not live are dimmed but
//! still editable — this is where a voice is set up before it is turned on.

use super::bind::{self, Ctx};
use doubler_core::MAX_VOICES;
use egui::{Align2, Rect, Ui, pos2, vec2};
use nxe_egui::bar::bar;
use nxe_egui::font;
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette, SPACE_2};

pub const ROW: f32 = 16.0;
const HEAD: f32 = 14.0;
const COLUMNS: [(&str, bool); 4] = [
    ("Delay", false),
    ("Detune", true),
    ("Pan", true),
    ("Gain", false),
];

pub const HEIGHT: f32 = HEAD + ROW * MAX_VOICES as f32;

pub fn table(ui: &mut Ui, ctx: &Ctx, palette: &Palette, rect: Rect) {
    let params = ctx.params();
    let live = doubler_core::Voices::count(params.voices.value().into());
    let number = 18.0;
    let column = (rect.width() - number) / COLUMNS.len() as f32;
    let figure = 58.0;
    let painter = ui.painter_at(rect);
    for (index, (name, _)) in COLUMNS.iter().enumerate() {
        let x = rect.left() + number + column * index as f32;
        font::tracked(
            &painter,
            pos2(x, rect.top()),
            Align2::LEFT_TOP,
            name,
            FONT_EYEBROW,
            palette.subtle.color(),
        );
    }
    for voice in 0..MAX_VOICES {
        let y = rect.top() + HEAD + ROW * voice as f32;
        let dim = if voice < live {
            *palette
        } else {
            dimmed(palette)
        };
        painter.text(
            pos2(rect.left(), y + ROW / 2.0),
            Align2::LEFT_CENTER,
            format!("{}", voice + 1),
            font::mono(FONT_VALUE),
            dim.muted.color(),
        );
        for (index, id) in params.voice_ids(voice).into_iter().enumerate() {
            let x = rect.left() + number + column * index as f32;
            let cell = Rect::from_min_size(pos2(x, y), vec2(column - figure - SPACE_2, ROW));
            let value = bind::normalized(ctx, id);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cell));
            let gestures = bar(&mut child, &dim, value, COLUMNS[index].1, cell.size(), 4.0);
            bind::gestures(ctx, &[id], &gestures, &[gestures.change.unwrap_or(value)]);
            painter.text(
                pos2(x + column - SPACE_2, y + ROW / 2.0),
                Align2::RIGHT_CENTER,
                effective(ctx, voice, index),
                font::mono(FONT_VALUE),
                dim.ink.color(),
            );
        }
    }
}

/// The value the voice actually runs at: the macro times the shape, in the
/// macro's unit. Gain is not scaled by a macro, so it is its own value.
fn effective(ctx: &Ctx, voice: usize, column: usize) -> String {
    let params = ctx.params();
    let [delay, detune, pan, gain] = params.voice_ids(voice);
    match column {
        0 => format!(
            "{:.1} ms",
            bind::plain(ctx, params.delay.id()) * bind::plain(ctx, delay)
        ),
        1 => format!(
            "{:+.1} ct",
            bind::plain(ctx, params.detune.id()) * bind::plain(ctx, detune)
        ),
        2 => {
            let source = params.source.value().into();
            let at = doubler_core::pan_for(
                source,
                bind::plain(ctx, params.spread.id()),
                bind::plain(ctx, pan),
                voice,
            );
            match (at * 100.0).round() as i32 {
                0 => "C".to_owned(),
                value if value < 0 => format!("L{}", -value),
                value => format!("R{value}"),
            }
        }
        _ => format!("{:+.1} dB", bind::plain(ctx, gain)),
    }
}

fn dimmed(palette: &Palette) -> Palette {
    Palette {
        accent: palette.accent.at(0.35),
        ink: palette.muted,
        muted: palette.subtle,
        ..*palette
    }
}
