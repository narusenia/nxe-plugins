//! A signal drawn as a curve — a spectrum, a transfer curve.
//!
//! **Only signals are curved** (`design-system.md`). Marks and layout stay
//! straight. The curve is a monotone cubic through the points
//! (Fritsch–Carlson), so it never overshoots between two of them: a spectrum
//! that bulges above its own peak shows energy that is not there.

use crate::surface::glow;
use crate::theme::Token;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2};

/// Samples a monotone cubic through `points` (x strictly increasing) at
/// `steps` evenly spaced x positions.
pub fn monotone(points: &[(f32, f32)], steps: usize) -> Vec<(f32, f32)> {
    if points.len() < 2 || steps < 2 {
        return points.to_vec();
    }
    let n = points.len();
    let slopes: Vec<f32> = points
        .windows(2)
        .map(|pair| (pair[1].1 - pair[0].1) / (pair[1].0 - pair[0].0))
        .collect();
    let mut tangents = vec![0.0f32; n];
    tangents[0] = slopes[0];
    tangents[n - 1] = slopes[n - 2];
    for i in 1..n - 1 {
        tangents[i] = if slopes[i - 1] * slopes[i] <= 0.0 {
            0.0
        } else {
            (slopes[i - 1] + slopes[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if slopes[i] == 0.0 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
            continue;
        }
        let a = tangents[i] / slopes[i];
        let b = tangents[i + 1] / slopes[i];
        let length = a.hypot(b);
        if length > 3.0 {
            let t = 3.0 / length;
            tangents[i] = t * a * slopes[i];
            tangents[i + 1] = t * b * slopes[i];
        }
    }

    let (first, last) = (points[0].0, points[n - 1].0);
    let mut segment = 0;
    (0..steps)
        .map(|step| {
            let x = first + (last - first) * step as f32 / (steps - 1) as f32;
            while segment < n - 2 && x > points[segment + 1].0 {
                segment += 1;
            }
            let (x0, y0) = points[segment];
            let (x1, y1) = points[segment + 1];
            let h = x1 - x0;
            let t = ((x - x0) / h).clamp(0.0, 1.0);
            let (t2, t3) = (t * t, t * t * t);
            let y = (2.0 * t3 - 3.0 * t2 + 1.0) * y0
                + (t3 - 2.0 * t2 + t) * h * tangents[segment]
                + (-2.0 * t3 + 3.0 * t2) * y1
                + (t3 - t2) * h * tangents[segment + 1];
            (x, y)
        })
        .collect()
}

/// Draws `points` (normalized: x and y in `0..=1`, y up) into `rect`.
///
/// `fill` paints the area under the curve — the dark wash on the figure's
/// accent ground. `glow` adds the bloom a moving signal is allowed.
pub fn curve(
    painter: &Painter,
    rect: Rect,
    points: &[(f32, f32)],
    line: Token,
    fill: Option<Token>,
    glows: bool,
) {
    let steps = (rect.width() / 2.0).max(2.0) as usize;
    let screen: Vec<Pos2> = monotone(points, steps)
        .into_iter()
        .map(|(x, y)| {
            pos2(
                rect.left() + x * rect.width(),
                rect.bottom() - y.clamp(0.0, 1.0) * rect.height(),
            )
        })
        .collect();
    if screen.len() < 2 {
        return;
    }
    if let Some(fill) = fill {
        // One quad per column: egui fills convex shapes only, and the area
        // under a curve is not one.
        let colour: Color32 = fill.color();
        for pair in screen.windows(2) {
            painter.add(Shape::convex_polygon(
                vec![
                    pair[0],
                    pair[1],
                    pos2(pair[1].x, rect.bottom()),
                    pos2(pair[0].x, rect.bottom()),
                ],
                colour,
                Stroke::NONE,
            ));
        }
    }
    if glows {
        glow(painter, &screen, line);
    }
    painter.add(Shape::line(screen, Stroke::new(1.5, line.color())));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Between two points the curve stays inside their range.
    #[test]
    fn it_never_overshoots() {
        let points = [(0.0, 0.1), (0.2, 0.9), (0.4, 0.9), (0.6, 0.2), (1.0, 0.25)];
        let sampled = monotone(&points, 400);
        for (x, y) in sampled {
            let segment = points
                .windows(2)
                .find(|pair| x >= pair[0].0 && x <= pair[1].0)
                .unwrap();
            let (low, high) = (
                segment[0].1.min(segment[1].1),
                segment[0].1.max(segment[1].1),
            );
            assert!(
                y >= low - 1e-4 && y <= high + 1e-4,
                "{y} at {x} left {low}..{high}"
            );
        }
    }

    #[test]
    fn it_passes_through_the_points() {
        let points = [(0.0, 0.0), (0.5, 1.0), (1.0, 0.5)];
        let sampled = monotone(&points, 3);
        for (got, want) in sampled.iter().zip(points) {
            assert!((got.1 - want.1).abs() < 1e-5);
        }
    }
}
