//! A rotary control.
//!
//! Takes a value and a gesture callback — never a parameter (`.agents/rules/vizia.md`).
//! The value may be a plain `f32` or a lens, because `Res` covers both; a
//! plugin binds a lens over its own model and gets automation-driven updates
//! for free.

use crate::input::{Drag, Gesture, GestureCallback};
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// Where the arc starts and ends, in degrees from straight up. The gap at the
/// bottom is what makes the position readable at a glance.
const ANGLE: f32 = 135.0;

/// Arc thickness, and the tick's share of the radius.
const ARC_WIDTH: f32 = 3.0;
const TICK_INNER: f32 = 0.42;

/// How far past the arc the two end marks sit, and how long they are.
///
/// **Where the sweep stops, said once.** A 270° arc has no natural end on
/// screen — the track is the same grey whether it has run out or not — so the
/// two extremes get a mark. It is the smallest thing that makes the control
/// read as an instrument with a range rather than as a ring.
const END_GAP: f32 = 3.0;
const END_LENGTH: f32 = 3.0;

/// Pushed into the view when the bound value changes from outside — an
/// automation move, a preset, the host.
enum KnobEvent {
    Set(f32),
}

pub struct Knob {
    drag: Drag,
    value: f32,
    on_gesture: GestureCallback,
}

impl Knob {
    pub fn new<'a>(
        cx: &'a mut Context,
        value: impl Res<f32> + 'static,
        on_gesture: impl Fn(&mut EventContext, Gesture) + 'static,
    ) -> Handle<'a, Self> {
        let initial = value.get_val(cx);

        Self {
            drag: Drag::default(),
            value: initial.clamp(0.0, 1.0),
            on_gesture: Box::new(on_gesture),
        }
        // The binding is set up inside `build`, where `cx.current` is this
        // view: doing it afterwards would need `cx` while the returned handle
        // still holds it.
        .build(cx, move |cx| {
            let entity = cx.current();
            value.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, KnobEvent::Set(value));
            });
        })
    }
}

impl View for Knob {
    fn element(&self) -> Option<&'static str> {
        Some("nxeknob")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|knob_event: &KnobEvent, _| match knob_event {
            KnobEvent::Set(value) => {
                self.value = value.clamp(0.0, 1.0);
                cx.needs_redraw();
            }
        });

        if let Some(gesture) = self.drag.handle(cx, event, self.value) {
            if let Gesture::Change(value) = gesture {
                self.value = value;
                cx.needs_redraw();
            }
            (self.on_gesture)(cx, gesture);
        }
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let palette = theme::palette(cx);
        let bounds = cx.bounds();
        let centre_x = bounds.x + bounds.w * 0.5;
        let centre_y = bounds.y + bounds.h * 0.5;
        let scale = cx.scale_factor();
        let width = ARC_WIDTH * scale;
        let radius = (bounds.w.min(bounds.h) * 0.5 - width).max(1.0);

        // The drawing convention: zero is straight up, angles grow clockwise.
        let start = (-ANGLE).to_radians() - std::f32::consts::FRAC_PI_2;
        let end = ANGLE.to_radians() - std::f32::consts::FRAC_PI_2;
        let angle = start + (end - start) * self.value;

        let mut track = vg::Path::new();
        track.arc(centre_x, centre_y, radius, start, end, vg::Solidity::Hole);
        let mut paint = vg::Paint::color(palette.track.vg());
        paint.set_line_width(width);
        paint.set_line_cap(vg::LineCap::Butt);
        canvas.stroke_path(&track, &paint);

        // The lit arc. The ramp runs across the widget from the resting end of
        // the sweep to the far one, so it agrees with a bar filled to the same
        // value (`Palette::paint`).
        if self.value > 0.0 {
            let mut filled = vg::Path::new();
            filled.arc(centre_x, centre_y, radius, start, angle, vg::Solidity::Hole);
            let mut paint = vg::Paint::color(palette.accent.vg());
            paint.set_line_width(width);
            paint.set_line_cap(vg::LineCap::Butt);
            canvas.stroke_path(&filled, &paint);
        }

        // The two end marks, outside the track.
        let mut ends = vg::Path::new();
        for edge in [start, end] {
            let (cos, sin) = (edge.cos(), edge.sin());
            let inner = radius + width * 0.5 + END_GAP * scale;
            ends.move_to(centre_x + cos * inner, centre_y + sin * inner);
            ends.line_to(
                centre_x + cos * (inner + END_LENGTH * scale),
                centre_y + sin * (inner + END_LENGTH * scale),
            );
        }
        let mut paint = vg::Paint::color(palette.line.vg());
        paint.set_line_width(scale.max(1.0));
        paint.set_line_cap(vg::LineCap::Butt);
        canvas.stroke_path(&ends, &paint);

        let mut tick = vg::Path::new();
        tick.move_to(
            centre_x + angle.cos() * radius * TICK_INNER,
            centre_y + angle.sin() * radius * TICK_INNER,
        );
        tick.line_to(
            centre_x + angle.cos() * (radius - width),
            centre_y + angle.sin() * (radius - width),
        );
        let mut paint = vg::Paint::color(cx.font_color().into());
        paint.set_line_width(width * 0.8);
        paint.set_line_cap(vg::LineCap::Butt);
        canvas.stroke_path(&tick, &paint);
    }
}
