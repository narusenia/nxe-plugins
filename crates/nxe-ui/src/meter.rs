//! A level bar with a peak-hold marker.
//!
//! **One bar per view.** A plugin that shows input and output shows four of
//! these (two channels each), and how far apart they sit is a layout decision —
//! the same reason a widget here never labels its own axis.
//!
//! No interaction: nothing here is a control. The values come in normalized,
//! so where the floor of the scale sits, and whether the scale is linear or in
//! decibels, stays with the caller (`crate::theme` has no opinion either).

use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How thick the hold marker is, in logical pixels.
const MARKER: f32 = 2.0;

enum MeterEvent {
    Level(f32),
    Hold(f32),
}

pub struct Meter {
    level: f32,
    hold: f32,
    vertical: bool,
    /// Tick positions in normalized value. Fixed for the life of the view — the
    /// caller knows where its own scale marks go, and draws their text itself.
    marks: Vec<f32>,
}

impl Meter {
    /// Fills upward from the bottom.
    pub fn new<'a>(
        cx: &'a mut Context,
        level: impl Res<f32> + 'static,
        hold: impl Res<f32> + 'static,
        marks: Vec<f32>,
    ) -> Handle<'a, Self> {
        Self::build_with(cx, level, hold, marks, true)
    }

    /// Fills rightward from the left.
    pub fn horizontal<'a>(
        cx: &'a mut Context,
        level: impl Res<f32> + 'static,
        hold: impl Res<f32> + 'static,
        marks: Vec<f32>,
    ) -> Handle<'a, Self> {
        Self::build_with(cx, level, hold, marks, false)
    }

    fn build_with<'a>(
        cx: &'a mut Context,
        level: impl Res<f32> + 'static,
        hold: impl Res<f32> + 'static,
        marks: Vec<f32>,
        vertical: bool,
    ) -> Handle<'a, Self> {
        let initial_level = level.get_val(cx);
        let initial_hold = hold.get_val(cx);

        Self {
            level: initial_level.clamp(0.0, 1.0),
            hold: initial_hold.clamp(0.0, 1.0),
            vertical,
            marks,
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            level.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, MeterEvent::Level(value));
            });
            hold.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, MeterEvent::Hold(value));
            });
        })
    }

    /// The filled rectangle for `value`, as `(x, y, w, h)`.
    ///
    /// Pure, because the one thing that goes wrong here is which end a bar
    /// grows from — and that is not visible in a unit test unless it is a
    /// function.
    pub fn fill(bounds: BoundingBox, value: f32, vertical: bool) -> (f32, f32, f32, f32) {
        let value = value.clamp(0.0, 1.0);
        if vertical {
            let height = bounds.h * value;
            (bounds.x, bounds.y + bounds.h - height, bounds.w, height)
        } else {
            (bounds.x, bounds.y, bounds.w * value, bounds.h)
        }
    }

    /// Where a normalized position sits along the bar's axis, as `(x, y)` of the
    /// marker's near corner.
    fn at(&self, bounds: BoundingBox, value: f32) -> (f32, f32) {
        let value = value.clamp(0.0, 1.0);
        if self.vertical {
            (bounds.x, bounds.y + (1.0 - value) * bounds.h)
        } else {
            (bounds.x + value * bounds.w, bounds.y)
        }
    }
}

impl View for Meter {
    fn element(&self) -> Option<&'static str> {
        Some("nxemeter")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|meter_event: &MeterEvent, _| {
            match meter_event {
                MeterEvent::Level(value) => self.level = value.clamp(0.0, 1.0),
                MeterEvent::Hold(value) => self.hold = value.clamp(0.0, 1.0),
            }
            cx.needs_redraw();
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let palette = theme::palette(cx);
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let line = scale.max(1.0);

        let mut track = vg::Path::new();
        track.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.fill_path(&track, &vg::Paint::color(theme::ELEVATED.vg()));

        let (x, y, w, h) = Self::fill(bounds, self.level, self.vertical);
        if w > 0.0 && h > 0.0 {
            let mut fill = vg::Path::new();
            fill.rect(x, y, w, h);
            // The ramp spans the whole track, so a level reads against the
            // scale rather than against its own height
            // (`Palette::paint`).
            let paint = if self.vertical {
                palette.paint(bounds.x, bounds.y + bounds.h, bounds.x, bounds.y)
            } else {
                palette.paint(bounds.x, bounds.y, bounds.x + bounds.w, bounds.y)
            };
            canvas.fill_path(&fill, &paint);
        }

        // Over the fill, in the background colour, so the scale stays legible
        // when the bar is nearly full — the same trick `Bar` uses for its
        // centre mark.
        let mut ticks = vg::Path::new();
        for mark in &self.marks {
            let (mx, my) = self.at(bounds, *mark);
            if self.vertical {
                ticks.rect(bounds.x, my - line * 0.5, bounds.w, line);
            } else {
                ticks.rect(mx - line * 0.5, bounds.y, line, bounds.h);
            }
        }
        canvas.fill_path(&ticks, &vg::Paint::color(theme::BACKGROUND.vg()));

        // **The marker turns white at the top rather than red.** This design has
        // one hue and it belongs to what the plugin is set to (`README.md`), so
        // "at full scale" is said with brightness. It reads as louder than the
        // accent, which is the message.
        if self.hold > 0.0 {
            let colour = if self.hold >= 1.0 {
                theme::FOREGROUND
            } else {
                palette.bright
            };
            let thickness = MARKER * scale;
            let (mx, my) = self.at(bounds, self.hold);
            let mut marker = vg::Path::new();
            if self.vertical {
                marker.rect(bounds.x, my - thickness * 0.5, bounds.w, thickness);
            } else {
                marker.rect(mx - thickness * 0.5, bounds.y, thickness, bounds.h);
            }
            canvas.fill_path(&marker, &vg::Paint::color(colour.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> BoundingBox {
        BoundingBox {
            x: 10.0,
            y: 20.0,
            w: 8.0,
            h: 100.0,
        }
    }

    /// The one thing that is easy to get backwards.
    #[test]
    fn a_vertical_bar_grows_from_the_bottom() {
        let (_, y, _, h) = Meter::fill(bounds(), 0.25, true);
        assert_eq!(h, 25.0);
        // The bottom edge is y + h of the bounds; the fill has to reach it.
        assert_eq!(y + h, 120.0);
    }

    #[test]
    fn a_horizontal_bar_grows_from_the_left() {
        let (x, _, w, _) = Meter::fill(bounds(), 0.25, false);
        assert_eq!(x, 10.0);
        assert_eq!(w, 2.0);
    }

    #[test]
    fn an_empty_bar_has_no_extent() {
        assert_eq!(Meter::fill(bounds(), 0.0, true).3, 0.0);
        assert_eq!(Meter::fill(bounds(), 0.0, false).2, 0.0);
    }

    #[test]
    fn a_full_bar_fills_the_track() {
        assert_eq!(Meter::fill(bounds(), 1.0, true).3, 100.0);
        assert_eq!(Meter::fill(bounds(), 1.0, false).2, 8.0);
    }

    #[test]
    fn a_fill_never_leaves_the_track() {
        for value in [-5.0f32, -0.1, 0.0, 0.5, 1.0, 1.1, 5.0] {
            for vertical in [true, false] {
                let (x, y, w, h) = Meter::fill(bounds(), value, vertical);
                assert!(x >= bounds().x, "{value} {vertical}");
                assert!(y >= bounds().y, "{value} {vertical}");
                assert!(
                    x + w <= bounds().x + bounds().w + 1e-3,
                    "{value} {vertical}"
                );
                assert!(
                    y + h <= bounds().y + bounds().h + 1e-3,
                    "{value} {vertical}"
                );
            }
        }
    }
}
