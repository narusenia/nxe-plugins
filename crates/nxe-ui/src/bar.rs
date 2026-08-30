//! A thin horizontal value control, for rows of them.
//!
//! Same interaction as the knob — including the **vertical** drag. A horizontal
//! drag on a horizontal bar reads as the obvious gesture right up until there
//! are eight of them stacked, at which point sliding sideways past the end of
//! one and into the next is a misfire waiting to happen. One gesture for every
//! value control also means there is nothing to learn per widget
//! (`.agents/rules/vizia.md`).
//!
//! Two shapes: filling from the left for a value that runs `0..=1`, and filling
//! from the centre for one that runs `-1..=1` mapped onto `0..=1` with `0.5` at
//! rest. The Doubler's Detail table needs both — delay and gain are unipolar,
//! detune and pan are not.

use crate::input::{Drag, Gesture, GestureCallback};
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How thick the filled part is drawn, in logical pixels. The track fills the
/// view's height; the fill is inset so the track reads as a groove.
const INSET: f32 = 1.0;

enum BarEvent {
    Set(f32),
}

pub struct Bar {
    drag: Drag,
    value: f32,
    centred: bool,
    on_gesture: GestureCallback,
}

impl Bar {
    /// Fills from the left. For a value that means "how much".
    pub fn new<'a>(
        cx: &'a mut Context,
        value: impl Res<f32> + 'static,
        on_gesture: impl Fn(&mut EventContext, Gesture) + 'static,
    ) -> Handle<'a, Self> {
        Self::build_with(cx, value, on_gesture, false)
    }

    /// Fills from the centre. For a value that means "which way, and how far".
    pub fn bipolar<'a>(
        cx: &'a mut Context,
        value: impl Res<f32> + 'static,
        on_gesture: impl Fn(&mut EventContext, Gesture) + 'static,
    ) -> Handle<'a, Self> {
        Self::build_with(cx, value, on_gesture, true)
    }

    fn build_with<'a>(
        cx: &'a mut Context,
        value: impl Res<f32> + 'static,
        on_gesture: impl Fn(&mut EventContext, Gesture) + 'static,
        centred: bool,
    ) -> Handle<'a, Self> {
        let initial = value.get_val(cx);

        Self {
            drag: Drag::default(),
            value: initial.clamp(0.0, 1.0),
            centred,
            on_gesture: Box::new(on_gesture),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            value.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, BarEvent::Set(value));
            });
        })
    }

    /// The filled span in `0..=1` of the track's width, as `(start, end)`.
    ///
    /// Pure, so the one thing that can be subtly wrong — which way a bipolar
    /// bar grows — is testable without a window.
    pub fn span(value: f32, centred: bool) -> (f32, f32) {
        let value = value.clamp(0.0, 1.0);
        if centred {
            if value >= 0.5 {
                (0.5, value)
            } else {
                (value, 0.5)
            }
        } else {
            (0.0, value)
        }
    }
}

impl View for Bar {
    fn element(&self) -> Option<&'static str> {
        Some("nxebar")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|bar_event: &BarEvent, _| match bar_event {
            BarEvent::Set(value) => {
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
        let scale = cx.scale_factor();
        let radius = theme::RADIUS_CONTROL * scale;
        let inset = INSET * scale;

        let mut track = vg::Path::new();
        track.rounded_rect(bounds.x, bounds.y, bounds.w, bounds.h, radius);
        canvas.fill_path(&track, &vg::Paint::color(palette.track.vg()));

        let (start, end) = Self::span(self.value, self.centred);
        let inner_width = (bounds.w - inset * 2.0).max(0.0);
        let width = (end - start) * inner_width;
        if width > 0.0 {
            let mut fill = vg::Path::new();
            fill.rounded_rect(
                bounds.x + inset + start * inner_width,
                bounds.y + inset,
                width,
                (bounds.h - inset * 2.0).max(0.0),
                (radius - inset).max(0.0),
            );
            // **The ramp spans the whole track, not the filled part.** A bar
            // at a quarter then shows the first quarter of it, so two bars at
            // different values are the same colour where they overlap and the
            // pale end always means "further" (`Palette::paint`).
            let (from, to) = if self.centred {
                // Outward from the middle, because that is where the fill
                // starts: a bipolar bar reads as distance from rest.
                let middle = bounds.x + bounds.w * 0.5;
                if self.value >= 0.5 {
                    (middle, bounds.x + bounds.w)
                } else {
                    (middle, bounds.x)
                }
            } else {
                (bounds.x, bounds.x + bounds.w)
            };
            canvas.fill_path(&fill, &palette.paint(from, bounds.y, to, bounds.y));
        }

        // The centre mark stays visible under the fill, so "at rest" is legible
        // even when the bar is nearly full.
        if self.centred {
            let mut mark = vg::Path::new();
            mark.rect(
                bounds.x + bounds.w * 0.5 - scale * 0.5,
                bounds.y,
                scale.max(1.0),
                bounds.h,
            );
            canvas.fill_path(&mark, &vg::Paint::color(palette.ground.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_unipolar_bar_fills_from_the_left() {
        assert_eq!(Bar::span(0.0, false), (0.0, 0.0));
        assert_eq!(Bar::span(0.4, false), (0.0, 0.4));
        assert_eq!(Bar::span(1.0, false), (0.0, 1.0));
    }

    /// The one thing that is easy to get backwards.
    #[test]
    fn a_bipolar_bar_fills_from_the_centre() {
        assert_eq!(Bar::span(0.5, true), (0.5, 0.5));
        assert_eq!(Bar::span(0.75, true), (0.5, 0.75));
        assert_eq!(Bar::span(0.25, true), (0.25, 0.5));
        assert_eq!(Bar::span(0.0, true), (0.0, 0.5));
        assert_eq!(Bar::span(1.0, true), (0.5, 1.0));
    }

    #[test]
    fn a_span_never_leaves_the_track() {
        for centred in [false, true] {
            for value in [-5.0f32, -0.1, 0.0, 0.5, 1.0, 1.1, 5.0] {
                let (start, end) = Bar::span(value, centred);
                assert!((0.0..=1.0).contains(&start), "{value} {centred}: {start}");
                assert!((0.0..=1.0).contains(&end), "{value} {centred}: {end}");
                assert!(start <= end, "{value} {centred}: {start} > {end}");
            }
        }
    }
}
