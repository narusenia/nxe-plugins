//! A half-circle field of draggable points.
//!
//! Knows nothing about what the two axes mean. The caller supplies points in
//! normalized coordinates — angle `-1`..`1` across the arc, radius `0`..`1` from
//! the origin — and decides what they stand for. The Doubler reads them as pan
//! and delay (`plugins/doubler/docs/specifications/ui.md`).
//!
//! [`crate::input::Drag`] is not reused: that state machine turns vertical
//! travel into one value, and a point here moves in two at once. The fine-drag
//! factor is shared, so both behave the same under `Shift`.
//!
//! **Nothing moves until the caller says so.** A drag reports where the pointer
//! went; the points redraw when the bound value changes. Moving the local copy
//! first looked more responsive and lied: a caller that clamps what it was
//! handed — a pan beyond what the spread allows, say — writes a value that does
//! not change, so no update comes back and the dot stays where it was dragged
//! rather than where it landed. A caller with nothing to write simply has a
//! field that does not move.
//!
//! **The points carry no labels.** Vizia's `draw_text` renders the entity's own
//! text, so one view cannot put eight numbers at eight positions. Instead the
//! field reports which point the pointer is over, which lets the caller
//! highlight the matching row elsewhere — more use than eight small digits.

use crate::input::FINE;
use crate::theme;
use std::f32::consts::FRAC_PI_2;
use vizia::prelude::*;
use vizia::vg;

/// Room for a dot to sit on the outer arc without being clipped.
const MARGIN: f32 = 10.0;

/// Dot radius at `size` 0 and 1.
const DOT_MIN: f32 = 3.0;
const DOT_MAX: f32 = 7.0;

/// How opaque a wedge gets at full signal. Enough to read against the frame,
/// far enough from solid that the points stay the thing being looked at.
const ANALYSIS_ALPHA: f32 = 0.16;

/// Half the width of an anchor's triangle.
const ANCHOR_SIZE: f32 = 7.0;

/// How close the pointer has to be to grab a dot.
const GRAB: f32 = 14.0;

/// One point in the field.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FieldPoint {
    /// `-1` at the left end of the arc, `0` straight up, `1` at the right end.
    pub angle: f32,
    /// `0` at the origin, `1` on the arc.
    pub radius: f32,
    /// `0`..`1`, scaling the dot between [`DOT_MIN`] and [`DOT_MAX`].
    pub size: f32,
    /// Which anchor this point belongs to. Ignored when there is one anchor.
    pub anchor: usize,
    /// A disabled point is drawn dim but stays draggable — the caller may well
    /// want to set up something that is not in use yet.
    pub enabled: bool,
    /// Where along the accent ramp this point is drawn, `0` deepest and `1`
    /// brightest. **What a group is, is the caller's business** — the Doubler
    /// gives each pair of voices its own step, which is what makes a pair
    /// findable in a field of eight dots.
    pub tint: f32,
}

/// Vizia compares bound values to decide whether to rebuild. `PartialEq` is the
/// whole answer here — the struct is four floats and two small values.
impl Data for FieldPoint {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Default for FieldPoint {
    fn default() -> Self {
        Self {
            angle: 0.0,
            radius: 0.5,
            size: 0.5,
            anchor: 0,
            enabled: true,
            tint: 1.0,
        }
    }
}

/// What the pointer did. Two values move together, which is why this is not
/// [`crate::input::Gesture`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FieldGesture {
    Begin(usize),
    Change {
        index: usize,
        angle: f32,
        radius: f32,
    },
    End(usize),
    Reset(usize),
    /// Which point the pointer is over, if any.
    Hover(Option<usize>),
    /// The anchors were dragged. **They share one radius and do not move on the
    /// angle** — an anchor marks where a source is, and the caller decides what
    /// its distance from the origin means. A caller with nothing to write here
    /// can ignore these and the anchors simply will not move.
    AnchorBegin,
    AnchorChange(f32),
    AnchorEnd,
    AnchorReset,
}

type FieldCallback = Box<dyn Fn(&mut EventContext, FieldGesture)>;

enum FieldEvent {
    Points(Vec<FieldPoint>),
    Anchors(Vec<FieldPoint>),
    Highlight(Option<usize>),
    Density(Vec<f32>),
}

/// Where the field sits inside its bounds. The origin is the bottom centre, so
/// the half circle fills the view.
struct Geometry {
    origin_x: f32,
    origin_y: f32,
    radius: f32,
}

impl Geometry {
    fn of(bounds: BoundingBox, margin: f32) -> Self {
        Self {
            origin_x: bounds.x + bounds.w * 0.5,
            origin_y: bounds.y + bounds.h - margin,
            radius: ((bounds.w * 0.5).min(bounds.h) - margin).max(1.0),
        }
    }

    /// Screen position of a normalized point.
    fn position(&self, angle: f32, radius: f32) -> (f32, f32) {
        let theta = angle.clamp(-1.0, 1.0) * FRAC_PI_2;
        let distance = radius.clamp(0.0, 1.0) * self.radius;
        (
            self.origin_x + theta.sin() * distance,
            self.origin_y - theta.cos() * distance,
        )
    }

    /// The normalized point a screen position lands on. Below the baseline
    /// folds onto it rather than wrapping around.
    fn value_at(&self, x: f32, y: f32) -> (f32, f32) {
        let dx = x - self.origin_x;
        let dy = (self.origin_y - y).max(0.0);
        let angle = dx.atan2(dy) / FRAC_PI_2;
        let radius = (dx * dx + dy * dy).sqrt() / self.radius;
        (angle.clamp(-1.0, 1.0), radius.clamp(0.0, 1.0))
    }
}

/// What the pointer has hold of. Points come first when both are in reach:
/// they are what the field is for, and an anchor sits behind them.
#[derive(Clone, Copy, PartialEq)]
enum Grabbed {
    Point(usize),
    Anchor,
}

pub struct PolarField {
    points: Vec<FieldPoint>,
    anchors: Vec<FieldPoint>,
    dragging: Option<Grabbed>,
    hovered: Option<usize>,
    /// A point the caller wants marked, whatever the pointer is doing. What
    /// makes the field and a table beside it point at the same voice.
    highlighted: Option<usize>,
    /// How much there is in each direction across the arc, left to right.
    /// **The signal, behind the settings.** Empty unless a caller supplies it.
    density: Vec<f32>,
    /// Where the pointer was when the drag started, and what the point was, so
    /// a fine drag can scale the travel rather than jumping to the pointer.
    grab: (f32, f32),
    grab_value: (f32, f32),
    on_gesture: FieldCallback,
}

impl PolarField {
    pub fn new<'a>(
        cx: &'a mut Context,
        points: impl Res<Vec<FieldPoint>> + 'static,
        anchors: impl Res<Vec<FieldPoint>> + 'static,
        on_gesture: impl Fn(&mut EventContext, FieldGesture) + 'static,
    ) -> Handle<'a, Self> {
        let initial_points = points.get_val(cx);
        let initial_anchors = anchors.get_val(cx);

        Self {
            points: initial_points,
            anchors: initial_anchors,
            dragging: None,
            hovered: None,
            highlighted: None,
            density: Vec::new(),
            grab: (0.0, 0.0),
            grab_value: (0.0, 0.0),
            on_gesture: Box::new(on_gesture),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            points.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, FieldEvent::Points(value));
            });
            anchors.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, FieldEvent::Anchors(value));
            });
        })
    }

    /// The point nearest to a screen position, if it is close enough to grab.
    ///
    /// Pure, so the thing that decides whether the field feels responsive or
    /// fiddly is testable.
    fn nearest(&self, geometry: &Geometry, x: f32, y: f32, scale: f32) -> Option<usize> {
        let reach = GRAB * scale;
        self.points
            .iter()
            .enumerate()
            .map(|(index, point)| {
                let (px, py) = geometry.position(point.angle, point.radius);
                (index, (px - x).hypot(py - y))
            })
            .filter(|(_, distance)| *distance <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// Whether an anchor is close enough to grab. They share a radius, so
    /// which one it is does not matter.
    fn near_anchor(&self, geometry: &Geometry, x: f32, y: f32, scale: f32) -> bool {
        let reach = GRAB * scale;
        self.anchors.iter().any(|anchor| {
            let (ax, ay) = geometry.position(anchor.angle, anchor.radius);
            (ax - x).hypot(ay - y) <= reach
        })
    }

    /// What a drag from `grab` to the pointer lands on, scaled by `Shift`.
    ///
    /// Travel from the grab rather than the pointer's absolute position:
    /// following the pointer would make a fine drag impossible.
    fn drag_to(&self, geometry: &Geometry, cx: &EventContext, x: f32, y: f32) -> (f32, f32) {
        let factor = if cx.modifiers().contains(Modifiers::SHIFT) {
            FINE
        } else {
            1.0
        };
        let (grab_x, grab_y) = geometry.position(self.grab_value.0, self.grab_value.1);
        geometry.value_at(
            grab_x + (x - self.grab.0) * factor,
            grab_y + (y - self.grab.1) * factor,
        )
    }

    fn geometry(&self, cx: &EventContext) -> Geometry {
        Geometry::of(cx.bounds(), MARGIN * cx.scale_factor())
    }
}

/// `Handle` belongs to vizia, so a modifier for it has to arrive as a trait.
pub trait PolarFieldModifiers {
    /// Marks one point from outside — the row a pointer is over in a table, the
    /// voice a mirrored edit just moved. Optional: a field with nothing to mark
    /// simply does not call this.
    fn highlight(self, index: impl Res<Option<usize>> + 'static) -> Self;

    /// How much signal is arriving from each direction, left to right, drawn as
    /// wedges under the points. Any length; **`0` is nothing and `1` is as much
    /// as the view can show**.
    ///
    /// Absolute, not normalized against the largest value. Normalizing was the
    /// first version and it kept the picture of a sound that had stopped on
    /// screen for ever: every direction fades at the same rate, so the ratios
    /// between them never change.
    fn density(self, bins: impl Res<Vec<f32>> + 'static) -> Self;
}

impl PolarFieldModifiers for Handle<'_, PolarField> {
    fn highlight(mut self, index: impl Res<Option<usize>> + 'static) -> Self {
        let entity = self.entity();
        index.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, FieldEvent::Highlight(value));
        });
        self
    }

    fn density(mut self, bins: impl Res<Vec<f32>> + 'static) -> Self {
        let entity = self.entity();
        bins.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, FieldEvent::Density(value));
        });
        self
    }
}

impl View for PolarField {
    fn element(&self) -> Option<&'static str> {
        Some("nxepolarfield")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|field_event: &FieldEvent, _| match field_event {
            FieldEvent::Points(points) => {
                self.points = points.clone();
                cx.needs_redraw();
            }
            FieldEvent::Anchors(anchors) => {
                self.anchors = anchors.clone();
                cx.needs_redraw();
            }
            FieldEvent::Highlight(index) => {
                self.highlighted = *index;
                cx.needs_redraw();
            }
            FieldEvent::Density(bins) => {
                self.density = bins.clone();
                cx.needs_redraw();
            }
        });

        let mut gesture = None;

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let geometry = self.geometry(cx);
                let scale = cx.scale_factor();
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                if let Some(index) = self.nearest(&geometry, x, y, scale) {
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.dragging = Some(Grabbed::Point(index));
                    self.grab = (x, y);
                    self.grab_value = (self.points[index].angle, self.points[index].radius);
                    gesture = Some(FieldGesture::Begin(index));
                    meta.consume();
                } else if self.near_anchor(&geometry, x, y, scale) {
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.dragging = Some(Grabbed::Anchor);
                    self.grab = (x, y);
                    let anchor = self.anchors[0];
                    self.grab_value = (anchor.angle, anchor.radius);
                    gesture = Some(FieldGesture::AnchorBegin);
                    meta.consume();
                }
            }

            WindowEvent::MouseMove(x, y) => {
                let geometry = self.geometry(cx);
                if let Some(grabbed) = self.dragging {
                    let (angle, radius) = self.drag_to(&geometry, cx, *x, *y);
                    gesture = Some(match grabbed {
                        Grabbed::Point(index) => FieldGesture::Change {
                            index,
                            angle,
                            radius,
                        },
                        Grabbed::Anchor => FieldGesture::AnchorChange(radius),
                    });
                } else {
                    let scale = cx.scale_factor();
                    let hovered = self.nearest(&geometry, *x, *y, scale);
                    if hovered != self.hovered {
                        self.hovered = hovered;
                        cx.needs_redraw();
                        gesture = Some(FieldGesture::Hover(hovered));
                    }
                }
            }

            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(grabbed) = self.dragging.take() {
                    cx.release();
                    cx.set_active(false);
                    gesture = Some(match grabbed {
                        Grabbed::Point(index) => FieldGesture::End(index),
                        Grabbed::Anchor => FieldGesture::AnchorEnd,
                    });
                    meta.consume();
                }
            }

            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let geometry = self.geometry(cx);
                let scale = cx.scale_factor();
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                if let Some(index) = self.nearest(&geometry, x, y, scale) {
                    gesture = Some(FieldGesture::Reset(index));
                    meta.consume();
                } else if self.near_anchor(&geometry, x, y, scale) {
                    gesture = Some(FieldGesture::AnchorReset);
                    meta.consume();
                }
            }

            WindowEvent::MouseOut if self.dragging.is_none() && self.hovered.is_some() => {
                self.hovered = None;
                cx.needs_redraw();
                gesture = Some(FieldGesture::Hover(None));
            }

            _ => {}
        });

        if let Some(gesture) = gesture {
            (self.on_gesture)(cx, gesture);
        }
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let palette = theme::palette(cx);
        let scale = cx.scale_factor();
        let geometry = Geometry::of(cx.bounds(), MARGIN * scale);
        let line = (1.0 * scale).max(1.0);

        // The signal, as a wedge per direction, under everything else.
        //
        // **Translucent foreground, not a solid grey.** A flat mid-grey over a
        // tinted background comes out muddy; light at low opacity only ever
        // lifts what is under it, which is what a layer meaning "there is sound
        // here" should do. Neutral either way: the accent says what the plugin
        // is *set* to, and a reader has to be able to tell that from the result.
        //
        // The values arrive on an absolute scale, so a wedge fades as the sound
        // does and silence draws nothing at all.
        for (index, value) in self.density.iter().enumerate() {
            let share = value.clamp(0.0, 1.0);
            if share <= 0.0 {
                continue;
            }

            // Bin `i` covers the slice of the arc from `i` to `i + 1`.
            let bins = self.density.len() as f32;
            let from = index as f32 / bins * 2.0 - 1.0;
            let to = (index + 1) as f32 / bins * 2.0 - 1.0;

            let mut wedge = vg::Path::new();
            wedge.move_to(geometry.origin_x, geometry.origin_y);
            let (x1, y1) = geometry.position(from, 1.0);
            let (x2, y2) = geometry.position(to, 1.0);
            wedge.line_to(x1, y1);
            wedge.line_to(x2, y2);
            wedge.close();
            canvas.fill_path(
                &wedge,
                &vg::Paint::color(theme::FOREGROUND.at(share * ANALYSIS_ALPHA).vg()),
            );
        }

        // The outer arc and the baseline: the frame the values are read against.
        let mut frame = vg::Path::new();
        frame.arc(
            geometry.origin_x,
            geometry.origin_y,
            geometry.radius,
            std::f32::consts::PI,
            std::f32::consts::TAU,
            vg::Solidity::Hole,
        );
        frame.move_to(geometry.origin_x - geometry.radius, geometry.origin_y);
        frame.line_to(geometry.origin_x + geometry.radius, geometry.origin_y);
        let mut paint = vg::Paint::color(theme::BORDER.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&frame, &paint);

        // One mid arc, so a radius can be judged rather than guessed.
        let mut mid = vg::Path::new();
        mid.arc(
            geometry.origin_x,
            geometry.origin_y,
            geometry.radius * 0.5,
            std::f32::consts::PI,
            std::f32::consts::TAU,
            vg::Solidity::Hole,
        );
        let mut paint = vg::Paint::color(theme::ELEVATED.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&mid, &paint);

        // Links from each point to its anchor, but only when there is more than
        // one anchor to tell apart.
        if self.anchors.len() > 1 {
            let mut links = vg::Path::new();
            for point in &self.points {
                if let Some(anchor) = self.anchors.get(point.anchor) {
                    let (ax, ay) = geometry.position(anchor.angle, anchor.radius);
                    let (px, py) = geometry.position(point.angle, point.radius);
                    links.move_to(ax, ay);
                    links.line_to(px, py);
                }
            }
            let mut paint = vg::Paint::color(theme::ELEVATED.vg());
            paint.set_line_width(line);
            canvas.stroke_path(&links, &paint);
        }

        // Anchors: an upward triangle, which reads as a source rather than as
        // another draggable dot. In the accent because it *is* draggable — the
        // shape is what tells it apart from a voice, not the colour.
        for anchor in &self.anchors {
            let (x, y) = geometry.position(anchor.angle, anchor.radius);
            let size = ANCHOR_SIZE * scale;
            let mut path = vg::Path::new();
            path.move_to(x, y - size);
            path.line_to(x + size, y + size * 0.7);
            path.line_to(x - size, y + size * 0.7);
            path.close();
            canvas.fill_path(&path, &vg::Paint::color(palette.accent.vg()));
        }

        for (index, point) in self.points.iter().enumerate() {
            let (x, y) = geometry.position(point.angle, point.radius);
            let dot = (DOT_MIN + (DOT_MAX - DOT_MIN) * point.size.clamp(0.0, 1.0)) * scale;

            let marked = self.hovered == Some(index)
                || self.highlighted == Some(index)
                || self.dragging == Some(Grabbed::Point(index));
            if marked {
                let mut ring = vg::Path::new();
                ring.circle(x, y, dot + 3.0 * scale);
                canvas.fill_path(&ring, &vg::Paint::color(palette.dim.vg()));
            }

            let colour = if point.enabled {
                palette.deep.mix(palette.bright, point.tint)
            } else {
                theme::SUBTLE
            };
            let mut path = vg::Path::new();
            path.circle(x, y, dot);
            canvas.fill_path(&path, &vg::Paint::color(colour.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> Geometry {
        Geometry {
            origin_x: 100.0,
            origin_y: 100.0,
            radius: 100.0,
        }
    }

    #[test]
    fn the_origin_is_the_bottom_centre() {
        let (x, y) = geometry().position(0.0, 0.0);
        assert!((x - 100.0).abs() < 1e-4);
        assert!((y - 100.0).abs() < 1e-4);
    }

    #[test]
    fn the_arc_runs_left_to_right_through_the_top() {
        let g = geometry();
        let (left_x, left_y) = g.position(-1.0, 1.0);
        let (top_x, top_y) = g.position(0.0, 1.0);
        let (right_x, right_y) = g.position(1.0, 1.0);

        assert!((left_x - 0.0).abs() < 1e-3, "{left_x}");
        assert!((left_y - 100.0).abs() < 1e-3, "{left_y}");
        assert!((top_x - 100.0).abs() < 1e-3, "{top_x}");
        assert!((top_y - 0.0).abs() < 1e-3, "{top_y}");
        assert!((right_x - 200.0).abs() < 1e-3, "{right_x}");
        assert!((right_y - 100.0).abs() < 1e-3, "{right_y}");
    }

    /// A position converted to a value and back has to land where it started,
    /// or dragging a point would make it creep.
    #[test]
    fn position_and_value_round_trip() {
        let g = geometry();
        for angle in [-1.0f32, -0.5, 0.0, 0.25, 1.0] {
            for radius in [0.1f32, 0.5, 1.0] {
                let (x, y) = g.position(angle, radius);
                let (back_angle, back_radius) = g.value_at(x, y);
                assert!(
                    (back_angle - angle).abs() < 1e-3,
                    "angle {angle} came back as {back_angle}"
                );
                assert!(
                    (back_radius - radius).abs() < 1e-3,
                    "radius {radius} came back as {back_radius}"
                );
            }
        }
    }

    /// Dragging below the baseline or past the arc must fold onto the edge, not
    /// wrap around to the other side.
    #[test]
    fn out_of_bounds_positions_fold_onto_the_edge() {
        let g = geometry();
        let (angle, radius) = g.value_at(100.0, 400.0);
        assert!((0.0..=1.0).contains(&radius));
        assert!((-1.0..=1.0).contains(&angle));

        let (_, far) = g.value_at(100.0, -500.0);
        assert_eq!(far, 1.0);

        let (left, _) = g.value_at(-500.0, 100.0);
        assert_eq!(left, -1.0);
        let (right, _) = g.value_at(500.0, 100.0);
        assert_eq!(right, 1.0);
    }
}
