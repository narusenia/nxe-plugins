//! The VU, as a needle on an arc or as two bars. The scale is in volts, as a
//! VU's is: −20 sits close to the start and the last 6 dB take half the arc.
//!
//! **The arc is the exception to "curves are for signals"** (`design-system.md`,
//! PEEK-2): a VU is the needle's swing, and straightened it is a bar meter.

use crate::WARN;
use egui::{Align2, Pos2, Rect, Shape, Stroke, Ui, pos2, vec2};
use nxe_egui::theme::{FONT_EYEBROW, FONT_VALUE, Palette};
use nxe_egui::{font, smooth};

pub const TICKS: [f32; 11] = [
    -20.0, -10.0, -7.0, -5.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0,
];
const LABELLED: [f32; 10] = [-20.0, -10.0, -7.0, -5.0, -3.0, -1.0, 0.0, 1.0, 2.0, 3.0];
/// Where the arc starts and ends, in degrees from three o'clock.
const FROM: f32 = 147.0;
const TO: f32 = 33.0;

/// A VU reading as a place on the scale, `0..=1`, by voltage.
pub fn position(vu: f32) -> f32 {
    let (low, high) = (10f32.powf(-20.0 / 20.0), 10f32.powf(3.0 / 20.0));
    ((10f32.powf(vu / 20.0) - low) / (high - low)).clamp(0.0, 1.0)
}

/// An amplitude as VU, against `reference` dBFS for 0 VU.
pub fn vu_of(amplitude: f32, reference: f32) -> f32 {
    crate::decibels(amplitude).map_or(-60.0, |db| db - reference)
}

fn label(vu: f32) -> String {
    if vu > 0.0 {
        format!("+{vu:.0}")
    } else {
        format!("{:.0}", vu.abs())
    }
}

/// The needle points at the louder channel.
pub fn arc(ui: &Ui, palette: &Palette, rect: Rect, vu: [f32; 2]) {
    let shown = smooth::smoothed(
        ui,
        ui.id().with("peek-vu-arc"),
        &[vu[0].max(vu[1])],
        smooth::TAU,
    )[0];
    let painter = ui.painter_at(rect);
    // The arc's half-width is `r cos 33°`, and the labels sit 12 px outside it.
    let radius = (rect.height() - 22.0).min((rect.width() * 0.5 - 16.0) / TO.to_radians().cos());
    let centre = pos2(rect.center().x, rect.bottom() - 4.0);
    let at = |vu: f32, r: f32| -> Pos2 {
        let angle = (FROM + (TO - FROM) * position(vu)).to_radians();
        centre + vec2(angle.cos(), -angle.sin()) * r
    };
    let band = |from: f32, to: f32, r: f32| -> Vec<Pos2> {
        (0..=48)
            .map(|step| at(from + (to - from) * step as f32 / 48.0, r))
            .collect()
    };
    let width = (radius * 0.07).max(4.0);
    painter.add(Shape::line(
        band(-20.0, 0.0, radius - width),
        Stroke::new(width, palette.track.color()),
    ));
    painter.add(Shape::line(
        band(0.0, 3.0, radius - width),
        Stroke::new(width, WARN.at(0.35).color()),
    ));
    for vu in TICKS {
        let over = vu >= 0.0;
        let major = LABELLED.contains(&vu);
        let colour = if over { WARN } else { palette.muted };
        painter.line_segment(
            [
                at(vu, radius - width * 1.6),
                at(vu, radius + if major { 3.0 } else { -width * 0.4 }),
            ],
            Stroke::new(if major { 2.0 } else { 1.0 }, colour.color()),
        );
        if major {
            painter.text(
                at(vu, radius + 12.0),
                Align2::CENTER_CENTER,
                label(vu),
                font::mono(FONT_VALUE),
                if over {
                    WARN.color()
                } else {
                    palette.ink.color()
                },
            );
        }
    }
    let needle = at(shown.clamp(-22.0, 4.0), radius - 2.0);
    painter.line_segment([centre, needle], Stroke::new(2.0, palette.bright.color()));
    painter.circle(
        centre,
        4.0,
        palette.ground.color(),
        Stroke::new(1.0, palette.muted.color()),
    );
    font::tracked(
        &painter,
        centre - vec2(0.0, 12.0),
        Align2::CENTER_BOTTOM,
        "VU",
        FONT_EYEBROW,
        palette.subtle.color(),
    );
}

/// Two bars on the same volt scale; the part over 0 is a heavier rule.
pub fn line(ui: &Ui, palette: &Palette, rect: Rect, vu: [f32; 2]) {
    let shown = smooth::smoothed(ui, ui.id().with("peek-vu-line"), &vu, smooth::TAU);
    let painter = ui.painter_at(rect);
    let (left, right) = (rect.left() + 18.0, rect.right());
    let x_of = |vu: f32| left + (right - left) * position(vu);
    let scale = rect.bottom() - 24.0;
    let bar = (scale - rect.top()) * 0.32;
    for (index, name) in ["L", "R"].into_iter().enumerate() {
        let y = rect.top() + index as f32 * bar * 1.5;
        let cell = Rect::from_min_max(pos2(left, y), pos2(right, y + bar));
        painter.text(
            pos2(rect.left(), cell.center().y),
            Align2::LEFT_CENTER,
            name,
            font::medium(FONT_VALUE),
            palette.muted.color(),
        );
        painter.rect_filled(cell, 0.0, palette.track.color());
        let x = x_of(shown[index]);
        painter.rect_filled(
            Rect::from_min_max(cell.min, pos2(x, cell.bottom())),
            0.0,
            palette.accent.at(0.3).color(),
        );
        painter.line_segment(
            [pos2(x, cell.top() - 4.0), pos2(x, cell.bottom() + 4.0)],
            Stroke::new(
                2.0,
                if shown[index] > 0.0 {
                    WARN
                } else {
                    palette.bright
                }
                .color(),
            ),
        );
    }
    painter.line_segment(
        [pos2(left, scale), pos2(x_of(0.0), scale)],
        Stroke::new(1.0, palette.line.color()),
    );
    painter.line_segment(
        [pos2(x_of(0.0), scale), pos2(right, scale)],
        Stroke::new(3.0, WARN.color()),
    );
    for vu in TICKS {
        let x = x_of(vu);
        let major = LABELLED.contains(&vu);
        painter.line_segment(
            [
                pos2(x, scale),
                pos2(x, scale + if major { 8.0 } else { 5.0 }),
            ],
            Stroke::new(1.0, if vu >= 0.0 { WARN } else { palette.subtle }.color()),
        );
        if major {
            painter.text(
                pos2(x, scale + 10.0),
                Align2::CENTER_TOP,
                label(vu),
                font::mono(FONT_VALUE),
                if vu >= 0.0 {
                    WARN.color()
                } else {
                    palette.muted.color()
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_is_in_volts() {
        assert_eq!(position(-20.0), 0.0);
        assert_eq!(position(3.0), 1.0);
        // 0 VU sits well past the middle, as on a real meter.
        assert!(
            position(0.0) > 0.6 && position(0.0) < 0.75,
            "{}",
            position(0.0)
        );
    }
}
