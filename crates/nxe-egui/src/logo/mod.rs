//! The vendor's mark, drawn rather than set.
//!
//! **`NXE` in the UI face reads as one more label that happens to be short.**
//! The logotype is a shape, so it reads as a mark — v0 drew it
//! (`nxe_ui::logo`), and the first egui sidebar set three letters instead.
//!
//! **The SVG is the source, the Rust is generated** (`scripts/generate-logo.py`,
//! `mise run logo:generate`): one shape that never changes is not worth an SVG
//! dependency. The colour in the file is ignored; the mark is painted with a
//! token, so it inverts with the surface like everything else.
//!
//! **egui fills convex shapes only**, and neither letterform is convex, so the
//! outline is flattened and cut into triangles once (ear clipping), then
//! scaled into a mesh each frame.

mod generated;

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, pos2};
use std::sync::OnceLock;

pub use generated::VIEWBOX;

/// One step of the outline, in the SVG's own coordinates.
#[derive(Clone, Copy, Debug)]
pub enum Segment {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Steps per cubic. The mark is drawn at about ten pixels high; eight is
/// already finer than a pixel there.
const CUBIC_STEPS: usize = 8;

/// How wide the mark is at a given height.
pub fn width_at(height: f32) -> f32 {
    height * VIEWBOX.0 / VIEWBOX.1
}

/// Paints the mark fitted into `rect`, keeping its proportions, at the left
/// of it.
pub fn mark(painter: &Painter, rect: Rect, colour: Color32) {
    let scale = (rect.width() / VIEWBOX.0).min(rect.height() / VIEWBOX.1);
    let top = rect.top() + (rect.height() - VIEWBOX.1 * scale) * 0.5;
    let mut mesh = Mesh::default();
    for triangle in triangles() {
        let base = mesh.vertices.len() as u32;
        for point in triangle {
            mesh.colored_vertex(
                pos2(rect.left() + point.x * scale, top + point.y * scale),
                colour,
            );
        }
        mesh.add_triangle(base, base + 1, base + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// The mark as triangles in viewbox coordinates, cut once.
fn triangles() -> &'static [[Pos2; 3]] {
    static TRIANGLES: OnceLock<Vec<[Pos2; 3]>> = OnceLock::new();
    TRIANGLES.get_or_init(|| {
        outlines()
            .iter()
            .flat_map(|outline| clip(outline))
            .collect()
    })
}

/// Each subpath as a closed polygon, beziers flattened.
fn outlines() -> Vec<Vec<Pos2>> {
    let mut outlines = Vec::new();
    let mut current: Vec<Pos2> = Vec::new();
    let push = |current: &mut Vec<Pos2>, point: Pos2| {
        if current
            .last()
            .is_none_or(|last| last.distance(point) > 1e-3)
        {
            current.push(point);
        }
    };
    for segment in generated::SEGMENTS {
        match *segment {
            Segment::Move(x, y) => {
                if current.len() > 2 {
                    outlines.push(std::mem::take(&mut current));
                }
                current.clear();
                current.push(pos2(x, y));
            }
            Segment::Line(x, y) => push(&mut current, pos2(x, y)),
            Segment::Cubic(x1, y1, x2, y2, x, y) => {
                let start = *current.last().unwrap_or(&pos2(x, y));
                for step in 1..=CUBIC_STEPS {
                    let t = step as f32 / CUBIC_STEPS as f32;
                    let u = 1.0 - t;
                    let at = |a: f32, b: f32, c: f32, d: f32| {
                        u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                    };
                    push(
                        &mut current,
                        pos2(at(start.x, x1, x2, x), at(start.y, y1, y2, y)),
                    );
                }
            }
            Segment::Close => {
                if current.len() > 1 && current[0].distance(current[current.len() - 1]) < 1e-3 {
                    current.pop();
                }
                if current.len() > 2 {
                    outlines.push(std::mem::take(&mut current));
                }
            }
        }
    }
    outlines
}

fn signed_area(points: &[Pos2]) -> f32 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let (a, b) = (points[index], points[(index + 1) % points.len()]);
        area += a.x * b.y - b.x * a.y;
    }
    area * 0.5
}

fn cross(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Ear clipping for a simple polygon without holes — which both letterforms
/// are.
fn clip(polygon: &[Pos2]) -> Vec<[Pos2; 3]> {
    let mut points = polygon.to_vec();
    if signed_area(&points) < 0.0 {
        points.reverse();
    }
    let mut triangles = Vec::with_capacity(points.len());
    let mut guard = points.len() * points.len();
    while points.len() > 3 && guard > 0 {
        guard -= 1;
        let count = points.len();
        let ear = (0..count).find(|&index| {
            let (a, b, c) = (
                points[(index + count - 1) % count],
                points[index],
                points[(index + 1) % count],
            );
            if cross(a, b, c) <= 1e-6 {
                return false;
            }
            !points.iter().any(|&p| {
                p != a
                    && p != b
                    && p != c
                    && cross(a, b, p) >= 0.0
                    && cross(b, c, p) >= 0.0
                    && cross(c, a, p) >= 0.0
            })
        });
        match ear {
            Some(index) => {
                triangles.push([
                    points[(index + count - 1) % count],
                    points[index],
                    points[(index + 1) % count],
                ]);
                points.remove(index);
            }
            // Only collinear runs are left; dropping the middle of one
            // changes no area.
            None => {
                let flat = (0..count)
                    .find(|&index| {
                        cross(
                            points[(index + count - 1) % count],
                            points[index],
                            points[(index + 1) % count],
                        )
                        .abs()
                            <= 1e-6
                    })
                    .unwrap_or(0);
                points.remove(flat);
            }
        }
    }
    if points.len() == 3 {
        triangles.push([points[0], points[1], points[2]]);
    }
    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The logotype is two shapes: the `N`, and the `XE`. A parser that
    /// dropped one would still draw something.
    #[test]
    fn both_letterforms_are_there() {
        assert_eq!(outlines().len(), 2);
    }

    /// The triangles cover exactly the outline: no ear was skipped and none
    /// overlaps. A wrong cut would leave a notch or a smear in the mark.
    #[test]
    fn the_triangles_cover_the_outline() {
        let outline: f32 = outlines().iter().map(|o| signed_area(o).abs()).sum();
        let cut: f32 = triangles().iter().map(|t| signed_area(t).abs()).sum();
        assert!(
            (outline - cut).abs() / outline < 1e-3,
            "outline {outline}, triangles {cut}"
        );
    }

    #[test]
    fn the_mark_keeps_its_proportions() {
        assert!((width_at(VIEWBOX.1) - VIEWBOX.0).abs() < 1e-3);
    }
}
