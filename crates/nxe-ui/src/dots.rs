//! A spectrum drawn twice: what came in as a line, what was added as grains.
//!
//! **Only an additive plugin can draw this.** A split topology has no
//! "the layer" to separate out — every band is the signal — so the figure that
//! fits one is the per-band gain (`crate::band`). Where there genuinely is an
//! added signal, showing it beside the original is the most direct statement a
//! window can make about what the plugin did.
//!
//! ## Why grains rather than a second curve
//!
//! Two curves of the same kind read as two settings of the same thing. The
//! added layer is not the same kind of object as the source: it is made rather
//! than measured, and how *coherent* it is — a harmonic series against a noise
//! bed — is one of the things its plugin is set by. A field of dots can say
//! that in its own shape, and a curve cannot: [`DotField::new`]'s `alignment`
//! pulls the grains onto their columns or scatters them across the cell.
//!
//! ## The scatter is a function, never a random number
//!
//! A field re-scattered every frame shimmers, which is unreadable, and it never
//! draws the same picture twice — so it cannot be reviewed in the gallery
//! either. The offsets come from the cell's own coordinates
//! ([`offset_of`]), so the same spectrum is always the same picture.

use crate::curve::Curve;
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How many cells tall the field is.
///
/// **Sixteen, not the column count.** The rows are a quantisation of level and
/// the columns are the spectrum's own bands, so the two numbers answer to
/// different things.
pub const ROWS: usize = 16;

/// The radius of one grain, in logical pixels.
const DOT: f32 = 2.0;

/// How far a grain may wander from its cell's centre, as a share of the cell.
///
/// Under a half, so a scattered grain stays inside its own column: two columns
/// whose grains mixed would be reporting a spectrum neither of them has.
const SCATTER: f32 = 0.42;

/// The floor under which a cell is not drawn at all.
///
/// A one-pole analyser never reaches zero, so without this the bottom row is
/// permanently, faintly on — which reads as a layer that never stops
/// (`SPK-16`).
const FAINTEST: f32 = 0.05;

enum DotEvent {
    Source(Curve),
    Layer(Curve),
    Alignment(f32),
}

pub struct DotField {
    source: Curve,
    layer: Curve,
    alignment: f32,
}

impl DotField {
    /// `source` and `layer` are `(x, y)` in `0..=1`, left to right. `alignment`
    /// is `0..=1`: at one the grains sit on their columns, at zero they scatter.
    pub fn new<'a>(
        cx: &'a mut Context,
        source: impl Res<Curve> + 'static,
        layer: impl Res<Curve> + 'static,
        alignment: impl Res<f32> + 'static,
    ) -> Handle<'a, Self> {
        Self {
            source: source.get_val(cx),
            layer: layer.get_val(cx),
            alignment: alignment.get_val(cx).clamp(0.0, 1.0),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            source.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, DotEvent::Source(value));
            });
            layer.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, DotEvent::Layer(value));
            });
            alignment.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, DotEvent::Alignment(value));
            });
        })
    }
}

/// How full one cell is, `0..=1`.
///
/// **The top cell is partial**, which is what keeps the field's upper edge from
/// stepping a whole row at a time as a level drifts. Below [`FAINTEST`] a cell
/// is empty rather than nearly empty.
pub fn fill_of(level: f32, row: usize) -> f32 {
    if !level.is_finite() || row >= ROWS {
        return 0.0;
    }
    let bottom = row as f32 / ROWS as f32;
    let fill = (level - bottom) * ROWS as f32;
    if fill < FAINTEST { 0.0 } else { fill.min(1.0) }
}

/// Where the grain in one cell sits, relative to the cell's centre, in `-1..=1`.
///
/// **A hash of the cell, not a random number.** The same spectrum has to draw
/// the same picture every frame, or the field shimmers and no one can read it —
/// and a widget whose output changes on its own cannot be reviewed in the
/// gallery.
pub fn offset_of(column: usize, row: usize) -> (f32, f32) {
    // splitmix32 over the two coordinates packed into one word.
    let mut state =
        (column as u32).wrapping_mul(0x9E37_79B9) ^ (row as u32).wrapping_mul(0x85EB_CA6B);
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9);
        let mut z = state;
        z ^= z >> 16;
        z = z.wrapping_mul(0x21F0_AAAD);
        z ^= z >> 15;
        z = z.wrapping_mul(0x735A_2D97);
        z ^= z >> 15;
        (z >> 8) as f32 / (1 << 23) as f32 - 1.0
    };
    (next(), next())
}

/// One column's level, or zero where the curve says nothing.
fn level_at(curve: &Curve, column: usize) -> f32 {
    curve.get(column).map_or(0.0, |(_, y)| y.clamp(0.0, 1.0))
}

impl View for DotField {
    fn element(&self) -> Option<&'static str> {
        Some("nxedots")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|dot_event: &DotEvent, _| {
            match dot_event {
                DotEvent::Source(curve) => self.source = curve.clone(),
                DotEvent::Layer(curve) => self.layer = curve.clone(),
                DotEvent::Alignment(value) => self.alignment = value.clamp(0.0, 1.0),
            }
            cx.needs_redraw();
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let palette = theme::palette(cx);
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let line = scale.max(1.0);
        let bottom = bounds.y + bounds.h;

        // **Depth from a one-pixel border, not from a fill.** This design has no
        // shadows and no rounding, so the plot is bounded by a rule
        // (`.agents/rules/ui.md`).
        let mut frame = vg::Path::new();
        frame.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.stroke_path(
            &frame,
            &vg::Paint::color(palette.line.vg()).with_line_width(line),
        );

        // What came in. **Neutral and quiet**: it is the ground the layer is
        // read against, not the subject (`REQ-AIR-013`).
        if self.source.len() > 1 {
            let mut path = vg::Path::new();
            for (index, (x, y)) in self.source.iter().enumerate() {
                let px = bounds.x + x.clamp(0.0, 1.0) * bounds.w;
                let py = bottom - y.clamp(0.0, 1.0) * bounds.h;
                if index == 0 {
                    path.move_to(px, py);
                } else {
                    path.line_to(px, py);
                }
            }
            canvas.stroke_path(
                &path,
                &vg::Paint::color(palette.subtle.vg()).with_line_width(line),
            );
        }

        let columns = self.layer.len();
        if columns == 0 {
            return;
        }
        let cell_w = bounds.w / columns as f32;
        let cell_h = bounds.h / ROWS as f32;
        let paint = vg::Paint::color(palette.accent.vg());
        let wander = 1.0 - self.alignment;

        // **Every grain in one path, filled once.** They all take the same
        // paint, and femtovg gives every `fill_path` its own draw call whatever
        // is in it. A grain each was up to `columns * ROWS` draw calls for one
        // picture (`docs/investigations/ui-frame-cost.md`).
        let mut grains = vg::Path::new();
        for column in 0..columns {
            let level = level_at(&self.layer, column);
            let centre_x = bounds.x + (column as f32 + 0.5) * cell_w;
            for row in 0..ROWS {
                let fill = fill_of(level, row);
                if fill <= 0.0 {
                    continue;
                }
                let (dx, dy) = offset_of(column, row);
                let x = centre_x + dx * cell_w * SCATTER * wander;
                let y = bottom - (row as f32 + 0.5) * cell_h + dy * cell_h * SCATTER * wander;

                // The partial top row is drawn smaller rather than dimmer: a
                // dimmer grain reads as a quieter band, and height is what the
                // figure already means.
                grains.circle(x, y, DOT * scale * fill.max(0.35));
            }
        }
        canvas.fill_path(&grains, &paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A full level fills every row; an empty one fills none.
    #[test]
    fn the_field_is_as_tall_as_the_level() {
        for row in 0..ROWS {
            assert_eq!(fill_of(1.0, row), 1.0, "row {row} at full level");
            assert_eq!(fill_of(0.0, row), 0.0, "row {row} at silence");
        }
    }

    /// **The top cell is partial.** Without it the field's upper edge steps a
    /// whole row at a time, which reads as a level that moves in jumps.
    #[test]
    fn the_top_row_is_partial() {
        // Half way up the third row of sixteen.
        let level = 2.5 / ROWS as f32;
        assert_eq!(fill_of(level, 0), 1.0);
        assert_eq!(fill_of(level, 1), 1.0);
        assert!((fill_of(level, 2) - 0.5).abs() < 1e-5);
        assert_eq!(fill_of(level, 3), 0.0);
    }

    /// A level that never quite reaches zero must not leave the bottom row
    /// permanently on (`SPK-16`).
    #[test]
    fn a_vanishing_level_empties_the_field() {
        assert_eq!(fill_of(1e-9, 0), 0.0);
        assert_eq!(fill_of(FAINTEST / ROWS as f32 * 0.5, 0), 0.0);
    }

    #[test]
    fn a_hostile_level_draws_nothing() {
        for level in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(fill_of(level, 0), 0.0, "{level}");
        }
        assert_eq!(fill_of(1.0, ROWS), 0.0);
    }

    /// **The same cell always scatters the same way.** A field that moves on
    /// its own cannot be read and cannot be reviewed.
    #[test]
    fn the_scatter_is_the_same_every_time() {
        for column in 0..8 {
            for row in 0..ROWS {
                assert_eq!(offset_of(column, row), offset_of(column, row));
            }
        }
    }

    /// And it is a scatter rather than a constant: neighbouring cells differ,
    /// and the offsets stay inside the cell.
    #[test]
    fn the_scatter_moves_and_stays_in_its_cell() {
        let mut seen = Vec::new();
        for column in 0..16 {
            for row in 0..ROWS {
                let (dx, dy) = offset_of(column, row);
                assert!((-1.0..=1.0).contains(&dx), "{dx}");
                assert!((-1.0..=1.0).contains(&dy), "{dy}");
                seen.push((dx * 1e6) as i32);
            }
        }
        seen.sort_unstable();
        seen.dedup();
        assert!(
            seen.len() > 16 * ROWS / 2,
            "only {} distinct offsets, so the field is a grid rather than a \
             scatter",
            seen.len()
        );
    }
}
