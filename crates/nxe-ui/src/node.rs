//! A log-frequency panel whose points can be **put anywhere, added and
//! removed**, over three signal curves.
//!
//! **Knows nothing about frequency or decibels.** A node's position arrives as
//! normalized `x` and `y`, its width as a fraction of the view's width, and the
//! curves behind it in the same coordinates. Mapping Hz onto a log axis is the
//! caller's business — which is also what lets the caller place the axis
//! labels, since vizia's `draw_text` only renders a view's own text (the same
//! limitation [`crate::polar::PolarField`], [`crate::curve::CurveView`] and
//! [`crate::band::BandField`] work around).
//!
//! ## Why not extend `CurveView` or `BandField`
//!
//! `CurveView`'s handles have **fixed `x`** — only their height moves — and
//! `BandField` moves the *edges of a region*, not a point. Free placement
//! needs a two-axis drag, a third value per point, and gestures that create and
//! destroy points; `band.rs` decided the same question the same way when it
//! declined to extend `CurveView`, and its existing callers would have paid for
//! all of it and used none.
//!
//! ## Three curves, and they must not read as three settings
//!
//! One is what is arriving, one is what the plugin is doing about it, and one is
//! what the user asked for. `dots.rs` notes that two curves of the same kind
//! read as two settings of the same thing, so they are drawn as different kinds
//! of object: **the signal is a fill from the floor, the reduction hangs from
//! the ceiling with a fill of its own, and only the weight is a stroked accent
//! line.**
//!
//! ## Nothing moves until the caller says so
//!
//! Straight from `polar.rs`: a drag reports where the pointer went, and the
//! points redraw when the bound value changes. A caller that clamps what it was
//! handed — a node pushed past the end of the range, or a seventh node when
//! only six exist — writes a value that does not change, so no update comes
//! back and a locally-moved point would stay where it was dragged rather than
//! where it landed.

use crate::curve::Curve;
use crate::input::FINE;
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How close the pointer has to be to a node to grab it, in logical pixels.
const GRAB: f32 = 12.0;

/// How close to a width grip, horizontally.
///
/// **Wider than it looks.** The grips are thin marks on purpose — a second
/// round thing beside a round thing reads as another node — so what makes them
/// catchable is the reach, not the drawing.
const GRIP_GRAB: f32 = 10.0;

/// How opaque the signal fill is. Light enough that the curves and the nodes
/// stay in front of it.
const ANALYSIS_ALPHA: f32 = 0.14;

/// How opaque the reduction fill is. **Heavier than the signal**: it is the
/// subject, and the signal is context.
const REDUCTION_ALPHA: f32 = 0.30;

const CURVE_WIDTH: f32 = 2.0;
const NODE_RADIUS: f32 = 5.0;
const GRIP_HEIGHT: f32 = 7.0;
/// The grips grow when the node is being worked on, so it is clear which one
/// the pointer has.
const GRIP_HEIGHT_ACTIVE: f32 = 11.0;
/// How opaque a resting node's width bar is. Visible enough to say "this is
/// how wide, and these ends move", faint enough that six of them are not a
/// second curve.
const SPAN_ALPHA: f32 = 0.45;

/// A node in the view's own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldNode {
    /// `0..=1` across the view.
    pub x: f32,
    /// `0..=1` bottom to top. **`0.5` is neutral** — the same resting line the
    /// weight curve is read against.
    pub y: f32,
    /// Half the node's width, as a fraction of the view's width. What the
    /// flanking grips move.
    pub half_width: f32,
}

/// vizia compares every bound value each frame to decide what to redraw, and
/// `FieldNode` is plain data, so equality is the right answer.
impl Data for FieldNode {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// What the pointer asked for. **None of it is applied here.**
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum NodeGesture {
    /// A drag on a node started. Nothing has changed yet.
    Begin(usize),
    /// A new position for a node, already clamped to the view.
    Change { index: usize, x: f32, y: f32 },
    /// A new half-width for a node.
    Width { index: usize, half_width: f32 },
    /// The drag finished.
    End(usize),
    /// A double click on empty space: put a node here.
    Add { x: f32, y: f32 },
    /// A double click on a node: take it away.
    Remove(usize),
    /// Which node the pointer is over, for the caller to print.
    Hover(Option<usize>),
}

type NodeCallback = Box<dyn Fn(&mut EventContext, NodeGesture)>;

enum NodeFieldEvent {
    Nodes(Vec<FieldNode>),
    Weight(Curve),
    Analysis(Curve),
    Reduction(Curve),
}

/// What is being dragged.
#[derive(Clone, Copy, PartialEq)]
enum Grabbed {
    Node(usize),
    /// A width grip, and which side of the node it is on.
    Width(usize),
}

pub struct NodeField {
    nodes: Vec<FieldNode>,
    /// What the user asked for, read against the resting line. The accent.
    weight: Curve,
    /// What is arriving. A fill from the floor.
    analysis: Curve,
    /// What is being taken out, `0` at the top for none. A fill from the
    /// ceiling.
    reduction: Curve,
    /// Gridline positions in normalized `x`. Fixed for the life of the view.
    grid: Vec<f32>,
    /// Gridline positions in normalized `y`, for a caller that labels the
    /// vertical axis.
    ///
    /// **A figure with a scale on one axis and none on the other cannot be
    /// read**: the spectrum behind the nodes is a level, and without lines to
    /// count nobody can say whether a peak is 6 dB or 20 above its
    /// neighbours. Empty draws none, which is what a field with no vertical
    /// quantity wants.
    rows: Vec<f32>,
    dragging: Option<Grabbed>,
    /// Where the pointer went down, and what the value was then — the same
    /// shape `polar.rs` uses, so `Shift` behaves the way it does everywhere.
    grab: (f32, f32),
    grab_value: (f32, f32),
    hovered: Option<usize>,
    on_gesture: NodeCallback,
}

impl NodeField {
    pub fn new<'a>(
        cx: &'a mut Context,
        nodes: impl Res<Vec<FieldNode>> + 'static,
        weight: impl Res<Curve> + 'static,
        grid: Vec<f32>,
        rows: Vec<f32>,
        on_gesture: impl Fn(&mut EventContext, NodeGesture) + 'static,
    ) -> Handle<'a, Self> {
        let initial_nodes = nodes.get_val(cx);
        let initial_weight = weight.get_val(cx);

        Self {
            nodes: initial_nodes,
            weight: initial_weight,
            analysis: Curve::new(),
            reduction: Curve::new(),
            grid,
            rows,
            dragging: None,
            grab: (0.0, 0.0),
            grab_value: (0.0, 0.0),
            hovered: None,
            on_gesture: Box::new(on_gesture),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            nodes.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, NodeFieldEvent::Nodes(value));
            });
            weight.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, NodeFieldEvent::Weight(value));
            });
        })
    }

    fn position(&self, bounds: BoundingBox, node: &FieldNode) -> (f32, f32) {
        (
            bounds.x + node.x.clamp(0.0, 1.0) * bounds.w,
            bounds.y + (1.0 - node.y.clamp(0.0, 1.0)) * bounds.h,
        )
    }

    /// The node nearest the pointer, if it is close enough to grab.
    ///
    /// **Both axes, unlike `CurveView`.** A point that can be anywhere is not
    /// identified by its `x` alone, and two nodes an octave apart at different
    /// heights have to be tellable apart.
    fn nearest(&self, bounds: BoundingBox, x: f32, y: f32, scale: f32) -> Option<usize> {
        let reach = GRAB * scale;
        self.nodes
            .iter()
            .enumerate()
            .map(|(index, node)| {
                let (nx, ny) = self.position(bounds, node);
                (index, ((nx - x).powi(2) + (ny - y).powi(2)).sqrt())
            })
            .filter(|(_, distance)| *distance <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// A width grip, if the pointer is on one.
    ///
    /// **Every node's grips are live, because every node's are drawn.** They
    /// were shown only on the hovered node, and nobody could find out how to
    /// set a width — a handle that appears only once you are already on it is a
    /// handle nobody discovers.
    fn nearest_grip(&self, bounds: BoundingBox, x: f32, y: f32, scale: f32) -> Option<usize> {
        let reach = GRIP_GRAB * scale;
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                let (nx, ny) = self.position(bounds, node);
                if (ny - y).abs() > GRAB * scale {
                    return None;
                }
                let span = node.half_width.max(0.0) * bounds.w;
                let distance = ((nx - span) - x).abs().min(((nx + span) - x).abs());
                (distance <= reach).then_some((index, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
    }

    /// Where a drag has taken the grabbed thing, in the view's coordinates.
    fn drag_to(&self, bounds: BoundingBox, cx: &EventContext, x: f32, y: f32) -> (f32, f32) {
        // **The same fine factor as every other control**, applied to the
        // travel rather than to the position, so `Shift` slows a drag instead
        // of moving the point somewhere else.
        let factor = if cx.modifiers().contains(Modifiers::SHIFT) {
            FINE
        } else {
            1.0
        };
        let dx = (x - self.grab.0) * factor / bounds.w.max(1.0);
        let dy = (self.grab.1 - y) * factor / bounds.h.max(1.0);
        (
            (self.grab_value.0 + dx).clamp(0.0, 1.0),
            (self.grab_value.1 + dy).clamp(0.0, 1.0),
        )
    }

    fn normalized(&self, bounds: BoundingBox, x: f32, y: f32) -> (f32, f32) {
        (
            ((x - bounds.x) / bounds.w.max(1.0)).clamp(0.0, 1.0),
            (1.0 - (y - bounds.y) / bounds.h.max(1.0)).clamp(0.0, 1.0),
        )
    }
}

/// `Handle` belongs to vizia, so a modifier for it has to arrive as a trait.
pub trait NodeFieldModifiers {
    /// What is arriving — a fill from the floor, behind everything.
    fn analysis(self, curve: impl Res<Curve> + 'static) -> Self;

    /// **What the plugin is doing** — a fill hanging from the ceiling.
    ///
    /// `0` is no reduction. This is the figure's subject: a process that works
    /// automatically and cannot be seen reads as one that is not working.
    fn reduction(self, curve: impl Res<Curve> + 'static) -> Self;
}

impl NodeFieldModifiers for Handle<'_, NodeField> {
    fn analysis(mut self, curve: impl Res<Curve> + 'static) -> Self {
        let entity = self.entity();
        curve.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, NodeFieldEvent::Analysis(value));
        });
        self
    }

    fn reduction(mut self, curve: impl Res<Curve> + 'static) -> Self {
        let entity = self.entity();
        curve.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, NodeFieldEvent::Reduction(value));
        });
        self
    }
}

impl View for NodeField {
    fn element(&self) -> Option<&'static str> {
        Some("nxenodefield")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|field_event: &NodeFieldEvent, _| {
            match field_event {
                NodeFieldEvent::Nodes(nodes) => self.nodes = nodes.clone(),
                NodeFieldEvent::Weight(weight) => self.weight = weight.clone(),
                NodeFieldEvent::Analysis(analysis) => self.analysis = analysis.clone(),
                NodeFieldEvent::Reduction(reduction) => self.reduction = reduction.clone(),
            }
            cx.needs_redraw();
        });

        let mut gesture = None;
        let bounds = cx.bounds();

        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let scale = cx.scale_factor();
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);

                // **A grip before a node.** The grips sit beside the node, so
                // asking for the node first would make a narrow one's grips
                // unreachable.
                if let Some(index) = self.nearest_grip(bounds, x, y, scale) {
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.dragging = Some(Grabbed::Width(index));
                    self.grab = (x, y);
                    self.grab_value = (self.nodes[index].half_width, 0.0);
                    gesture = Some(NodeGesture::Begin(index));
                    meta.consume();
                } else if let Some(index) = self.nearest(bounds, x, y, scale) {
                    cx.capture();
                    cx.focus();
                    cx.set_active(true);
                    self.dragging = Some(Grabbed::Node(index));
                    self.grab = (x, y);
                    self.grab_value = (self.nodes[index].x, self.nodes[index].y);
                    gesture = Some(NodeGesture::Begin(index));
                    meta.consume();
                }
            }

            WindowEvent::MouseMove(x, y) => match self.dragging {
                Some(Grabbed::Node(index)) => {
                    let (nx, ny) = self.drag_to(bounds, cx, *x, *y);
                    gesture = Some(NodeGesture::Change {
                        index,
                        x: nx,
                        y: ny,
                    });
                }
                Some(Grabbed::Width(index)) => {
                    // The distance from the node, whichever side the pointer
                    // went — a width has no sign.
                    let centre = bounds.x + self.nodes[index].x * bounds.w;
                    let factor = if cx.modifiers().contains(Modifiers::SHIFT) {
                        FINE
                    } else {
                        1.0
                    };
                    let travel = (*x - self.grab.0) * factor;
                    let from_grab = self.grab_value.0 * bounds.w;
                    let side = if self.grab.0 < centre { -1.0 } else { 1.0 };
                    let half_width =
                        ((from_grab + travel * side) / bounds.w.max(1.0)).clamp(0.0, 1.0);
                    gesture = Some(NodeGesture::Width { index, half_width });
                }
                None => {
                    let scale = cx.scale_factor();
                    let hovered = self.nearest(bounds, *x, *y, scale);
                    if hovered != self.hovered {
                        self.hovered = hovered;
                        cx.needs_redraw();
                        gesture = Some(NodeGesture::Hover(hovered));
                    }
                }
            },

            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(grabbed) = self.dragging.take() {
                    cx.release();
                    cx.set_active(false);
                    let index = match grabbed {
                        Grabbed::Node(index) | Grabbed::Width(index) => index,
                    };
                    gesture = Some(NodeGesture::End(index));
                    meta.consume();
                }
            }

            // **Double click, not right click and not scroll.** Neither of the
            // other two has any precedent here, and how a host's context menu
            // and baseview's scrolling behave cannot be known without opening a
            // DAW (`plugins/pumice/docs/specifications/ui.md`).
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                let scale = cx.scale_factor();
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                gesture = Some(match self.nearest(bounds, x, y, scale) {
                    Some(index) => NodeGesture::Remove(index),
                    None => {
                        let (nx, ny) = self.normalized(bounds, x, y);
                        NodeGesture::Add { x: nx, y: ny }
                    }
                });
                meta.consume();
            }

            WindowEvent::MouseOut if self.dragging.is_none() && self.hovered.is_some() => {
                self.hovered = None;
                cx.needs_redraw();
                gesture = Some(NodeGesture::Hover(None));
            }

            _ => {}
        });

        if let Some(gesture) = gesture {
            (self.on_gesture)(cx, gesture);
        }
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let palette = theme::palette(cx);
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let line = scale.max(1.0);
        let at = |x: f32, y: f32| {
            (
                bounds.x + x.clamp(0.0, 1.0) * bounds.w,
                bounds.y + (1.0 - y.clamp(0.0, 1.0)) * bounds.h,
            )
        };

        // What is arriving, filled from the floor. Neutral rather than accent:
        // the accent belongs to what the plugin is *set* to, so a reader can
        // tell the setting from the signal at a glance (`curve.rs`).
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
                &vg::Paint::color(palette.ink.at(ANALYSIS_ALPHA).vg()),
            );
        }

        let mut grid = vg::Path::new();
        for x in &self.grid {
            let (gx, _) = at(*x, 0.0);
            grid.move_to(gx, bounds.y);
            grid.line_to(gx, bounds.y + bounds.h);
        }
        for y in &self.rows {
            let (_, gy) = at(0.0, *y);
            grid.move_to(bounds.x, gy);
            grid.line_to(bounds.x + bounds.w, gy);
        }
        let mut paint = vg::Paint::color(palette.track.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&grid, &paint);

        // The resting line the weight is read against.
        let (_, centre_y) = at(0.0, 0.5);
        let mut resting = vg::Path::new();
        resting.move_to(bounds.x, centre_y);
        resting.line_to(bounds.x + bounds.w, centre_y);
        let mut paint = vg::Paint::color(palette.line.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&resting, &paint);

        // **What the plugin is doing**, hanging from the ceiling. A different
        // kind of object from the weight curve on purpose: same shape twice
        // would read as two settings of one thing (`dots.rs`).
        if self.reduction.len() > 1 {
            let mut path = vg::Path::new();
            let (first_x, _) = at(self.reduction[0].0, 0.0);
            path.move_to(first_x, bounds.y);
            for (x, depth) in &self.reduction {
                let (px, py) = at(*x, 1.0 - depth.clamp(0.0, 1.0));
                path.line_to(px, py);
            }
            let (last_x, _) = at(self.reduction[self.reduction.len() - 1].0, 0.0);
            path.line_to(last_x, bounds.y);
            path.close();
            canvas.fill_path(
                &path,
                &vg::Paint::color(palette.dim.at(REDUCTION_ALPHA).vg()),
            );
        }

        // What the user asked for.
        if self.weight.len() > 1 {
            let mut path = vg::Path::new();
            for (index, (x, y)) in self.weight.iter().enumerate() {
                let (px, py) = at(*x, *y);
                if index == 0 {
                    path.move_to(px, py);
                } else {
                    path.line_to(px, py);
                }
            }
            let mut paint = vg::Paint::color(palette.accent.vg());
            paint.set_line_width(CURVE_WIDTH * scale);
            paint.set_line_cap(vg::LineCap::Butt);
            canvas.stroke_path(&path, &paint);
        }

        let active = self
            .dragging
            .map(|grabbed| match grabbed {
                Grabbed::Node(index) | Grabbed::Width(index) => index,
            })
            .or(self.hovered);

        // **Every node's width is drawn, not just the one under the pointer.**
        // A bar through the node with a mark at each end says two things at
        // once: how wide this node is, and that its ends are what widen it.
        // Grips that appeared on hover alone were a handle nobody found.
        for (index, node) in self.nodes.iter().enumerate() {
            let (px, py) = self.position(bounds, node);
            let radius = NODE_RADIUS * scale;
            let span = node.half_width.max(0.0) * bounds.w;
            let working = active == Some(index);
            let height = if working {
                GRIP_HEIGHT_ACTIVE * scale
            } else {
                GRIP_HEIGHT * scale
            };

            let mut bar = vg::Path::new();
            bar.move_to(px - span, py);
            bar.line_to(px + span, py);
            let mut paint = vg::Paint::color(if working {
                palette.bright.vg()
            } else {
                palette.bright.at(SPAN_ALPHA).vg()
            });
            paint.set_line_width(line);
            canvas.stroke_path(&bar, &paint);

            // Marks rather than dots: they move one value along one axis, and a
            // second round thing beside a round thing reads as another node.
            let mut grips = vg::Path::new();
            for side in [-1.0_f32, 1.0] {
                let gx = px + side * span;
                grips.move_to(gx, py - height);
                grips.line_to(gx, py + height);
            }
            let mut paint = vg::Paint::color(if working {
                palette.bright.vg()
            } else {
                palette.bright.at(SPAN_ALPHA).vg()
            });
            paint.set_line_width(line * 2.0);
            canvas.stroke_path(&grips, &paint);

            if working {
                let mut halo = vg::Path::new();
                halo.circle(px, py, radius + 3.0 * scale);
                canvas.fill_path(&halo, &vg::Paint::color(palette.dim.vg()));
            }

            let mut path = vg::Path::new();
            path.circle(px, py, radius);
            canvas.fill_path(&path, &vg::Paint::color(palette.bright.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(nodes: Vec<FieldNode>) -> NodeField {
        NodeField {
            nodes,
            weight: Curve::new(),
            analysis: Curve::new(),
            reduction: Curve::new(),
            grid: Vec::new(),
            rows: Vec::new(),
            dragging: None,
            grab: (0.0, 0.0),
            grab_value: (0.0, 0.0),
            hovered: None,
            on_gesture: Box::new(|_, _| {}),
        }
    }

    fn bounds() -> BoundingBox {
        BoundingBox {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 200.0,
        }
    }

    fn node(x: f32, y: f32) -> FieldNode {
        FieldNode {
            x,
            y,
            half_width: 0.05,
        }
    }

    #[test]
    fn a_node_maps_to_the_view() {
        let view = field(vec![node(0.25, 0.75)]);
        let (x, y) = view.position(bounds(), &view.nodes[0]);
        assert!((x - 200.0).abs() < 0.01);
        // `y` runs bottom to top, so three quarters up is a quarter down.
        assert!((y - 50.0).abs() < 0.01);
    }

    /// **Both axes**, which is what `CurveView` cannot do: two nodes at the
    /// same `x` and different heights have to be tellable apart.
    #[test]
    fn grabbing_uses_both_axes() {
        let view = field(vec![node(0.5, 0.9), node(0.5, 0.1)]);
        assert_eq!(view.nearest(bounds(), 400.0, 20.0, 1.0), Some(0));
        assert_eq!(view.nearest(bounds(), 400.0, 180.0, 1.0), Some(1));
        // Neither is within reach in the middle.
        assert_eq!(view.nearest(bounds(), 400.0, 100.0, 1.0), None);
    }

    #[test]
    fn grabbing_needs_the_pointer_close() {
        let view = field(vec![node(0.5, 0.5)]);
        assert_eq!(view.nearest(bounds(), 400.0, 100.0, 1.0), Some(0));
        assert_eq!(view.nearest(bounds(), 500.0, 100.0, 1.0), None);
    }

    /// **Every node's grips are live**, because every node's are drawn. They
    /// used to appear on hover only, and a handle that appears once the pointer
    /// is already on it is a handle nobody discovers.
    #[test]
    fn every_node_offers_its_grips() {
        let view = field(vec![node(0.5, 0.5)]);
        // 0.05 of 800 px is 40 px either side of 400.
        assert_eq!(view.nearest_grip(bounds(), 360.0, 100.0, 1.0), Some(0));
        assert_eq!(view.nearest_grip(bounds(), 440.0, 100.0, 1.0), Some(0));
        // Not in the middle, and not far above.
        assert_eq!(view.nearest_grip(bounds(), 400.0, 100.0, 1.0), None);
        assert_eq!(view.nearest_grip(bounds(), 360.0, 10.0, 1.0), None);
    }

    /// With two nodes in reach, the nearer grip wins.
    #[test]
    fn the_nearer_grip_wins() {
        let view = field(vec![node(0.3, 0.5), node(0.7, 0.5)]);
        // 0.3 of 800 is 240, so its right grip is at 280.
        assert_eq!(view.nearest_grip(bounds(), 282.0, 100.0, 1.0), Some(0));
        // 0.7 of 800 is 560, so its left grip is at 520.
        assert_eq!(view.nearest_grip(bounds(), 518.0, 100.0, 1.0), Some(1));
    }

    #[test]
    fn a_click_maps_back_to_normalized_coordinates() {
        let view = field(Vec::new());
        let (x, y) = view.normalized(bounds(), 200.0, 50.0);
        assert!((x - 0.25).abs() < 0.01);
        assert!((y - 0.75).abs() < 0.01);
    }

    #[test]
    fn a_click_outside_the_view_is_clamped() {
        let view = field(Vec::new());
        assert_eq!(view.normalized(bounds(), -50.0, -50.0), (0.0, 1.0));
        assert_eq!(view.normalized(bounds(), 900.0, 300.0), (1.0, 0.0));
    }
}
