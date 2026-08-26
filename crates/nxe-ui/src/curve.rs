//! A curve display with vertically draggable handles.
//!
//! **Knows nothing about frequency, decibels, or logarithms.** Everything comes
//! in normalized: `x` runs `0`..`1` across the view, `y` runs `0`..`1` bottom to
//! top with `0.5` as the resting line. Mapping Hz onto a log axis is the
//! caller's business, which is also what lets the caller place the axis labels —
//! vizia's `draw_text` only renders a view's own text, so a widget cannot label
//! its own gridlines (the same limitation the polar field works around).
//!
//! The vertical arithmetic is [`crate::input::Drag::value_after`], so a handle
//! and a knob move by the same amount for the same travel, `Shift` included.

use crate::input::{Drag, Gesture};
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// A polyline in normalized coordinates.
pub type Curve = Vec<(f32, f32)>;

/// A shaded `x` range, drawn behind the curves.
pub type Span = (f32, f32);

/// A draggable point: `(x, y)`. `x` is fixed; only `y` moves.
pub type Grip = (f32, f32);

/// How close the pointer has to be to a handle, horizontally, to grab it.
const GRAB: f32 = 16.0;

/// How opaque the signal fill is. Light enough that the curve and its handles
/// stay in front of it.
const ANALYSIS_ALPHA: f32 = 0.14;

const CURVE_WIDTH: f32 = 2.0;
const GRIP_RADIUS: f32 = 4.0;

/// The ring marking where the signal is on the curve.
///
/// Smaller than a grip: a grip is something to grab and this is something to
/// read, and the two must not look like the same offer.
const POINT_RADIUS: f32 = 3.0;

type CurveCallback = Box<dyn Fn(&mut EventContext, usize, Gesture)>;

enum CurveEvent {
    Curves(Vec<Curve>),
    Spans(Vec<Span>),
    Grips(Vec<Grip>),
    Analysis(Curve),
    Reference(Curve),
    Point(Option<(f32, f32)>),
}

pub struct CurveView {
    curves: Vec<Curve>,
    spans: Vec<Span>,
    grips: Vec<Grip>,
    /// A filled area behind everything: **what is going through**, as opposed to
    /// what the curves are set to. Empty unless a caller supplies one.
    analysis: Curve,
    /// The line the curves are read against. Empty means the horizontal one
    /// through the middle of the window.
    reference: Curve,
    /// Where the signal is sitting on the curve right now, in the same
    /// normalized coordinates as everything else. `None` draws nothing.
    point: Option<(f32, f32)>,
    /// Gridline positions in normalized `x`. Fixed for the life of the view —
    /// the caller knows where its own axis marks go.
    grid: Vec<f32>,
    drag: Drag,
    dragging: Option<usize>,
    on_gesture: CurveCallback,
}

impl CurveView {
    pub fn new<'a>(
        cx: &'a mut Context,
        curves: impl Res<Vec<Curve>> + 'static,
        spans: impl Res<Vec<Span>> + 'static,
        grips: impl Res<Vec<Grip>> + 'static,
        grid: Vec<f32>,
        on_gesture: impl Fn(&mut EventContext, usize, Gesture) + 'static,
    ) -> Handle<'a, Self> {
        let initial_curves = curves.get_val(cx);
        let initial_spans = spans.get_val(cx);
        let initial_grips = grips.get_val(cx);

        Self {
            curves: initial_curves,
            spans: initial_spans,
            grips: initial_grips,
            analysis: Curve::new(),
            reference: Curve::new(),
            point: None,
            grid,
            drag: Drag::default(),
            dragging: None,
            on_gesture: Box::new(on_gesture),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            curves.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, CurveEvent::Curves(value));
            });
            spans.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, CurveEvent::Spans(value));
            });
            grips.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, CurveEvent::Grips(value));
            });
        })
    }

    /// The handle nearest the pointer's `x`, if it is close enough to grab.
    ///
    /// Horizontal distance only: the handles sit at fixed `x` positions, so
    /// requiring the pointer to also be near the current `y` would make a
    /// handle that is already at the top hard to pull back down.
    fn nearest(&self, bounds: BoundingBox, x: f32, scale: f32) -> Option<usize> {
        let reach = GRAB * scale;
        self.grips
            .iter()
            .enumerate()
            .map(|(index, (grip_x, _))| {
                (
                    index,
                    (bounds.x + grip_x.clamp(0.0, 1.0) * bounds.w - x).abs(),
                )
            })
            .filter(|(_, distance)| *distance <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }
}

/// `Handle` belongs to vizia, so a modifier for it has to arrive as a trait.
pub trait CurveViewModifiers {
    /// A filled area drawn behind the curves — the signal going through, on the
    /// same axes as the curves. Optional; without it nothing is drawn there.
    fn analysis(self, curve: impl Res<Curve> + 'static) -> Self;

    /// **The line the curves are read against**, replacing the horizontal one
    /// through the middle.
    ///
    /// A transfer curve of input against output is read against the diagonal,
    /// not against a level (`plugins/sparkleur/docs/specifications/ui.md`), and
    /// a view cannot draw a diagonal as a gridline — those are vertical. Give
    /// it the line instead.
    fn reference(self, curve: impl Res<Curve> + 'static) -> Self;

    /// **Where the signal is on the curve right now.**
    ///
    /// A transfer plot says what *would* happen at every level. The one thing it
    /// cannot say on its own is which of those levels is arriving — and that is
    /// what a compressor is watched for: whether the material is sitting on the
    /// flat part, in the knee, or past it. Same normalized coordinates as the
    /// curves; `None` draws nothing.
    fn point(self, position: impl Res<Option<(f32, f32)>> + 'static) -> Self;
}

impl CurveViewModifiers for Handle<'_, CurveView> {
    fn analysis(mut self, curve: impl Res<Curve> + 'static) -> Self {
        let entity = self.entity();
        curve.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, CurveEvent::Analysis(value));
        });
        self
    }

    fn reference(mut self, curve: impl Res<Curve> + 'static) -> Self {
        let entity = self.entity();
        curve.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, CurveEvent::Reference(value));
        });
        self
    }

    fn point(mut self, position: impl Res<Option<(f32, f32)>> + 'static) -> Self {
        let entity = self.entity();
        position.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, CurveEvent::Point(value));
        });
        self
    }
}

impl View for CurveView {
    fn element(&self) -> Option<&'static str> {
        Some("nxecurveview")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|curve_event: &CurveEvent, _| {
            match curve_event {
                CurveEvent::Curves(curves) => self.curves = curves.clone(),
                CurveEvent::Spans(spans) => self.spans = spans.clone(),
                CurveEvent::Grips(grips) => self.grips = grips.clone(),
                CurveEvent::Analysis(analysis) => self.analysis = analysis.clone(),
                CurveEvent::Point(point) => self.point = *point,
                CurveEvent::Reference(reference) => self.reference = reference.clone(),
            }
            cx.needs_redraw();
        });

        // Grab a handle first; from then on the shared drag state machine runs
        // it, so the travel and the fine factor match every other control.
        let mut grabbed = None;
        if self.dragging.is_none() {
            event.map(|window_event, _meta| {
                if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                    grabbed = self.nearest(cx.bounds(), cx.mouse().cursorx, cx.scale_factor());
                }
            });
        }
        if let Some(index) = grabbed {
            self.dragging = Some(index);
        }

        let Some(index) = self.dragging else {
            return;
        };
        let value = self.grips[index].1;

        if let Some(gesture) = self.drag.handle(cx, event, value) {
            match gesture {
                Gesture::Change(new_value) => {
                    self.grips[index].1 = new_value;
                    cx.needs_redraw();
                }
                Gesture::End => self.dragging = None,
                _ => {}
            }
            (self.on_gesture)(cx, index, gesture);
        }
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let line = scale.max(1.0);
        let at = |x: f32, y: f32| {
            (
                bounds.x + x.clamp(0.0, 1.0) * bounds.w,
                bounds.y + (1.0 - y.clamp(0.0, 1.0)) * bounds.h,
            )
        };

        // Shaded ranges go behind everything: they are context, not data. Light
        // enough that eight of them overlapping still read as shading.
        for (start, end) in &self.spans {
            let (left, _) = at(start.min(*end), 0.0);
            let (right, _) = at(start.max(*end), 0.0);
            let mut path = vg::Path::new();
            path.rect(left, bounds.y, (right - left).max(0.0), bounds.h);
            canvas.fill_path(&path, &vg::Paint::color(theme::ACCENT_DIM.at(0.05).vg()));
        }

        // The signal, filled from the floor.
        //
        // **Translucent foreground, not a solid grey.** Grey over the tinted
        // bands underneath came out muddy; light at low opacity lifts what is
        // beneath it instead of covering it. Neutral either way, because the
        // accent belongs to what the plugin is *set* to — a reader has to be
        // able to tell the setting from the result at a glance.
        //
        // **Above the shaded ranges, below everything else.** Underneath them it
        // was buried: eight overlapping bands of translucent accent stack up to
        // near-opaque, and what the plugin is doing to the sound matters more
        // than which voice covers which octave.
        if self.analysis.len() > 1 {
            let mut path = vg::Path::new();
            let (first_x, _) = at(self.analysis[0].0, 0.0);
            path.move_to(first_x, bounds.y + bounds.h);
            for (x, y) in &self.analysis {
                let (px, py) = at(*x, *y);
                path.line_to(px, py);
            }
            let (last_x, _) = at(self.analysis[self.analysis.len() - 1].0, 0.0);
            path.line_to(last_x, bounds.y + bounds.h);
            path.close();
            canvas.fill_path(
                &path,
                &vg::Paint::color(theme::FOREGROUND.at(ANALYSIS_ALPHA).vg()),
            );
        }

        let mut grid = vg::Path::new();
        for x in &self.grid {
            let (gx, _) = at(*x, 0.0);
            grid.move_to(gx, bounds.y);
            grid.line_to(gx, bounds.y + bounds.h);
        }
        let mut paint = vg::Paint::color(theme::ELEVATED.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&grid, &paint);

        // The line a curve's distance from is the thing being read. The middle
        // of the window unless the caller gave one of its own — an input
        // against output plot is read against the diagonal, and a diagonal
        // cannot be a gridline here.
        let mut resting = vg::Path::new();
        if self.reference.len() > 1 {
            for (index, (x, y)) in self.reference.iter().enumerate() {
                let (px, py) = at(*x, *y);
                if index == 0 {
                    resting.move_to(px, py);
                } else {
                    resting.line_to(px, py);
                }
            }
        } else {
            let (_, centre_y) = at(0.0, 0.5);
            resting.move_to(bounds.x, centre_y);
            resting.line_to(bounds.x + bounds.w, centre_y);
        }
        let mut paint = vg::Paint::color(theme::BORDER.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&resting, &paint);

        for curve in &self.curves {
            let mut path = vg::Path::new();
            for (index, (x, y)) in curve.iter().enumerate() {
                let (px, py) = at(*x, *y);
                if index == 0 {
                    path.move_to(px, py);
                } else {
                    path.line_to(px, py);
                }
            }
            // The ramp runs left to right across the plot, so the curve says
            // which way it is read — the far end of the axis is the pale end,
            // the same as a bar filled to the far end
            // (`theme::accent_paint`).
            let mut paint = theme::accent_paint(bounds.x, bounds.y, bounds.x + bounds.w, bounds.y);
            paint.set_line_width(CURVE_WIDTH * scale);
            paint.set_line_cap(vg::LineCap::Butt);
            canvas.stroke_path(&path, &paint);
        }

        // **Where the signal is right now**, over the curve and under nothing.
        //
        // A ring rather than a disc: a filled dot on a two-pixel line hides the
        // very thing it is marking a place on, and what is being read is the
        // shape of the curve at that place.
        if let Some((x, y)) = self.point {
            let (px, py) = at(x, y);
            let radius = POINT_RADIUS * scale;

            // A dark disc under the ring, so the ring reads against the curve
            // rather than merging with it wherever the two cross.
            let mut back = vg::Path::new();
            back.circle(px, py, radius);
            canvas.fill_path(&back, &vg::Paint::color(theme::BACKGROUND.vg()));

            let mut ring = vg::Path::new();
            ring.circle(px, py, radius);
            let mut paint = vg::Paint::color(theme::FOREGROUND.vg());
            paint.set_line_width(line);
            canvas.stroke_path(&ring, &paint);
        }

        for (index, (x, y)) in self.grips.iter().enumerate() {
            let (px, py) = at(*x, *y);
            let radius = GRIP_RADIUS * scale;

            if self.dragging == Some(index) {
                let mut ring = vg::Path::new();
                ring.circle(px, py, radius + 3.0 * scale);
                canvas.fill_path(&ring, &vg::Paint::color(theme::ACCENT_DIM.vg()));
            }

            let mut path = vg::Path::new();
            path.circle(px, py, radius);
            canvas.fill_path(&path, &vg::Paint::color(theme::ACCENT_BRIGHT.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(grips: Vec<Grip>) -> CurveView {
        CurveView {
            curves: Vec::new(),
            spans: Vec::new(),
            grips,
            analysis: Curve::new(),
            reference: Curve::new(),
            point: None,
            grid: Vec::new(),
            drag: Drag::default(),
            dragging: None,
            on_gesture: Box::new(|_, _, _| {}),
        }
    }

    fn bounds() -> BoundingBox {
        BoundingBox {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        }
    }

    #[test]
    fn a_handle_is_grabbed_by_its_x_alone() {
        // Two handles, one at each quarter; the y values are far apart on
        // purpose — they must not affect the grab.
        let view = view(vec![(0.25, 0.0), (0.75, 1.0)]);

        assert_eq!(view.nearest(bounds(), 50.0, 1.0), Some(0));
        assert_eq!(view.nearest(bounds(), 150.0, 1.0), Some(1));
    }

    #[test]
    fn the_nearest_handle_wins() {
        let view = view(vec![(0.4, 0.5), (0.5, 0.5)]);
        assert_eq!(view.nearest(bounds(), 82.0, 1.0), Some(0));
        assert_eq!(view.nearest(bounds(), 98.0, 1.0), Some(1));
    }

    #[test]
    fn a_pointer_far_from_every_handle_grabs_nothing() {
        let view = view(vec![(0.1, 0.5)]);
        assert_eq!(view.nearest(bounds(), 180.0, 1.0), None);
    }

    #[test]
    fn no_handles_means_nothing_to_grab() {
        let view = view(Vec::new());
        assert_eq!(view.nearest(bounds(), 100.0, 1.0), None);
    }
}
