//! A log-frequency panel with draggable band regions and two signal curves.
//!
//! **Knows nothing about frequency or decibels.** A band's edges arrive as
//! normalized `x`, its height as `0..=1`, and the curves behind it in the same
//! coordinates. Mapping Hz onto a log axis is the caller's business — which is
//! also what lets the caller place the axis labels, since vizia's `draw_text`
//! only renders a view's own text (the same limitation
//! [`crate::polar::PolarField`] and [`crate::curve::CurveView`] work around).
//!
//! **Two signal curves, not one.** This is for a plugin that *adds* something
//! to an untouched dry path: one curve is what came in and the other is what is
//! being added, and being able to draw them as separate layers is the whole
//! reason a parallel topology is worth choosing. A widget that took one
//! analysis curve could only ever show the result.
//!
//! **Why not extend [`crate::curve::CurveView`].** Grabbable regions, a second
//! drag axis, a per-region reduction reading and two curves roughly double that
//! view's surface, and its existing caller — a filter response with read-only
//! bands — would pay for all of it and use none of it.

use crate::curve::Curve;
use crate::input::{Drag, Gesture};
use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How tall the bottom strip that drags `focus` is, in logical pixels.
///
/// **A rail rather than "the empty space".** Deciding between a vertical and a
/// horizontal drag by where the pointer happens to be is ambiguous the moment a
/// region is full height, and guessing from the direction of the first few
/// pixels of movement is worse. A strip that is always there is always
/// grabbable, and it reads as the frequency axis it sits under.
const RAIL: f32 = 12.0;

/// How opaque the dry signal's fill is. Light enough that the regions and the
/// added-harmonics curve stay in front of it.
const ANALYSIS_ALPHA: f32 = 0.14;

/// The region as it is set, behind the region as it is sounding.
const SET_ALPHA: f32 = 0.12;
/// And what is actually sounding.
const LIVE_ALPHA: f32 = 0.42;

const WET_WIDTH: f32 = 2.0;

/// One band region.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Band {
    /// The region's edges in normalized `x`. May overlap its neighbours —
    /// parallel generators have no reason to tile the axis.
    pub low: f32,
    pub high: f32,
    /// How much this band contributes, `0..=1`. What a vertical drag writes.
    pub level: f32,
    /// How far what is **sounding** sits from what was **set**, `-1..=1`.
    ///
    /// Negative sinks the solid part inside the set outline, so a protective
    /// detector doing its job is visible rather than silent. Positive lifts it
    /// above, which is what an upward compressor needs — half of Sparkleur's
    /// product is a gain going up, and a picture that can only draw a reduction
    /// cannot show it (`SPK-10`).
    ///
    /// **A share of the whole height, not of `level`.** A band trimmed all the
    /// way down is still being compressed, and a proportional reading would
    /// draw that as nothing.
    pub delta: f32,
    /// Where on the accent ramp this band sits, `0` deepest and `1` brightest.
    /// Steps of one hue rather than several hues (`README.md`).
    pub tint: f32,
    /// Drawn lit while every other band is dimmed.
    pub soloed: bool,
}

/// vizia compares every bound value each frame to decide what to redraw, and
/// `Band` is plain data, so equality is the right answer.
impl Data for Band {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl Band {
    /// What is sounding: `level` moved by `delta`.
    pub fn live(&self) -> f32 {
        (self.level.clamp(0.0, 1.0) + self.delta.clamp(-1.0, 1.0)).clamp(0.0, 1.0)
    }

    /// The midpoint of the region, in normalized `x`.
    fn centre(&self) -> f32 {
        (self.low + self.high) * 0.5
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BandGesture {
    Begin(usize),
    Change {
        index: usize,
        level: f32,
    },
    End(usize),
    Reset(usize),
    /// Which region the pointer is over, so a table elsewhere can light up the
    /// matching row.
    Hover(Option<usize>),
    /// The rail. Only sent when a caller has wired [`BandFieldModifiers::focus`]
    /// — there is nothing to write otherwise.
    FocusBegin,
    FocusChange(f32),
    FocusEnd,
    FocusReset,
}

/// What the pointer has hold of.
#[derive(Clone, Copy, PartialEq)]
enum Grabbed {
    Band(usize),
    Focus,
}

enum BandEvent {
    Bands(Vec<Band>),
    Dry(Curve),
    Wet(Curve),
    Highlight(Option<usize>),
    Focus(f32),
    Unity(f32),
}

type BandCallback = Box<dyn Fn(&mut EventContext, BandGesture)>;

pub struct BandField {
    bands: Vec<Band>,
    /// What came in. **The signal, behind the settings.**
    dry: Curve,
    /// What is being added to it.
    wet: Curve,
    /// Gridline positions in normalized `x`. Fixed for the life of the view.
    grid: Vec<f32>,
    highlighted: Option<usize>,
    hovered: Option<usize>,
    /// `None` until a caller wires `.focus(...)`, which is also what decides
    /// whether the rail does anything.
    focus: Option<f32>,
    /// Where "no change" sits, in normalized `y`. `None` for a field whose
    /// regions grow from the floor — Velour's, where an empty band is nothing
    /// added rather than unity.
    unity: Option<f32>,
    drag: Drag,
    grabbed: Option<Grabbed>,
    /// Pointer `x` at the last move of a rail drag.
    last_x: f32,
    on_gesture: BandCallback,
}

impl BandField {
    pub fn new<'a>(
        cx: &'a mut Context,
        bands: impl Res<Vec<Band>> + 'static,
        dry: impl Res<Curve> + 'static,
        wet: impl Res<Curve> + 'static,
        grid: Vec<f32>,
        on_gesture: impl Fn(&mut EventContext, BandGesture) + 'static,
    ) -> Handle<'a, Self> {
        let initial_bands = bands.get_val(cx);
        let initial_dry = dry.get_val(cx);
        let initial_wet = wet.get_val(cx);

        Self {
            bands: initial_bands,
            dry: initial_dry,
            wet: initial_wet,
            grid,
            highlighted: None,
            hovered: None,
            focus: None,
            unity: None,
            drag: Drag::default(),
            grabbed: None,
            last_x: 0.0,
            on_gesture: Box::new(on_gesture),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            bands.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, BandEvent::Bands(value));
            });
            dry.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, BandEvent::Dry(value));
            });
            wet.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, BandEvent::Wet(value));
            });
        })
    }

    /// The plot and the rail, split off the view's bounds.
    ///
    /// Pure: the split is the only geometry here that two different parts of the
    /// widget have to agree on.
    pub fn regions(bounds: BoundingBox, scale: f32) -> (BoundingBox, BoundingBox) {
        // A rail that ate the plot on a short view would be worse than a thin
        // one, so it gives way rather than the other way round.
        let height = (RAIL * scale).min(bounds.h * 0.25);
        let plot = BoundingBox {
            h: bounds.h - height,
            ..bounds
        };
        let rail = BoundingBox {
            y: bounds.y + bounds.h - height,
            h: height,
            ..bounds
        };
        (plot, rail)
    }

    /// Which region a normalized `x` falls in.
    ///
    /// **The nearest centre wins when regions overlap.** Parallel generators
    /// share edges on purpose, and "the first one in the list" would make the
    /// overlap belong to whichever band happened to be declared first.
    pub fn band_at(bands: &[Band], x: f32) -> Option<usize> {
        bands
            .iter()
            .enumerate()
            .filter(|(_, band)| x >= band.low.min(band.high) && x <= band.low.max(band.high))
            .min_by(|a, b| {
                (a.1.centre() - x)
                    .abs()
                    .total_cmp(&(b.1.centre() - x).abs())
            })
            .map(|(index, _)| index)
    }

    /// What the pointer is over, in the widget's own terms.
    fn target_at(&self, bounds: BoundingBox, scale: f32, x: f32, y: f32) -> Option<Grabbed> {
        let (plot, rail) = Self::regions(bounds, scale);

        if y >= rail.y {
            // The rail is inert without something to write to.
            return self.focus.map(|_| Grabbed::Focus);
        }
        if plot.w <= 0.0 {
            return None;
        }
        Self::band_at(&self.bands, (x - plot.x) / plot.w).map(Grabbed::Band)
    }

    fn notify(&self, cx: &mut EventContext, gesture: BandGesture) {
        (self.on_gesture)(cx, gesture);
    }
}

/// `Handle` belongs to vizia, so a modifier for it has to arrive as a trait.
pub trait BandFieldModifiers {
    /// Marks one region from outside — the row a pointer is over in a table.
    /// Optional: a field with nothing to mark simply does not call this.
    fn highlight(self, index: impl Res<Option<usize>> + 'static) -> Self;

    /// The value the rail drags, `0..=1`. **Wiring this is what turns the rail
    /// on**; without it a drag there does nothing, the way an unwired anchor in
    /// [`crate::polar::PolarField`] simply does not move.
    fn focus(self, value: impl Res<f32> + 'static) -> Self;

    /// Where "no change" sits, in normalized `y`. **Wiring this is what draws
    /// the line**; a field that does not call it has regions growing from the
    /// floor and nothing to mark.
    fn unity(self, y: impl Res<f32> + 'static) -> Self;
}

impl BandFieldModifiers for Handle<'_, BandField> {
    fn highlight(mut self, index: impl Res<Option<usize>> + 'static) -> Self {
        let entity = self.entity();
        index.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, BandEvent::Highlight(value));
        });
        self
    }

    fn focus(mut self, value: impl Res<f32> + 'static) -> Self {
        let entity = self.entity();
        value.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, BandEvent::Focus(value));
        });
        self
    }

    fn unity(mut self, y: impl Res<f32> + 'static) -> Self {
        let entity = self.entity();
        y.set_or_bind(self.context(), entity, move |cx, value| {
            cx.emit_to(entity, BandEvent::Unity(value));
        });
        self
    }
}

impl View for BandField {
    fn element(&self) -> Option<&'static str> {
        Some("nxebandfield")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|band_event: &BandEvent, _| {
            match band_event {
                BandEvent::Bands(bands) => self.bands = bands.clone(),
                BandEvent::Dry(curve) => self.dry = curve.clone(),
                BandEvent::Wet(curve) => self.wet = curve.clone(),
                BandEvent::Highlight(index) => self.highlighted = *index,
                BandEvent::Focus(value) => self.focus = Some(value.clamp(0.0, 1.0)),
                BandEvent::Unity(value) => self.unity = Some(value.clamp(0.0, 1.0)),
            }
            cx.needs_redraw();
        });

        let bounds = cx.bounds();
        let scale = cx.scale_factor();

        // **Double-click is hit-tested here rather than taken from `Drag`.**
        // Whether a grab is still open by the time the double-click arrives
        // depends on the event order, and a reset that works most of the time is
        // worse than one that is decided by where the pointer is.
        let mut resolved = None;
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDoubleClick(MouseButton::Left) = window_event {
                match self.target_at(bounds, scale, cx.mouse().cursorx, cx.mouse().cursory) {
                    Some(Grabbed::Band(index)) => resolved = Some(BandGesture::Reset(index)),
                    Some(Grabbed::Focus) => resolved = Some(BandGesture::FocusReset),
                    None => return,
                }
                meta.consume();
            }
        });
        if let Some(gesture) = resolved {
            self.notify(cx, gesture);
            return;
        }

        // Decide what a fresh press has hold of before the drag machinery sees
        // the same event, the way `CurveView` picks up a handle.
        if self.grabbed.is_none() {
            let mut pressed = None;
            event.map(|window_event, _| {
                if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                    pressed = self.target_at(bounds, scale, cx.mouse().cursorx, cx.mouse().cursory);
                }
            });
            if let Some(target) = pressed {
                self.grabbed = Some(target);
                if target == Grabbed::Focus {
                    self.last_x = cx.mouse().cursorx;
                }
            }
        }

        match self.grabbed {
            Some(Grabbed::Band(index)) => {
                let Some(level) = self.bands.get(index).map(|band| band.level) else {
                    self.grabbed = None;
                    return;
                };

                if let Some(gesture) = self.drag.handle(cx, event, level) {
                    let translated = match gesture {
                        Gesture::Begin => BandGesture::Begin(index),
                        Gesture::Change(level) => {
                            // Kept locally so the drag runs at pointer speed
                            // rather than at the round-trip's. An incoming
                            // value always wins on the next update.
                            if let Some(band) = self.bands.get_mut(index) {
                                band.level = level;
                            }
                            cx.needs_redraw();
                            BandGesture::Change { index, level }
                        }
                        Gesture::End => {
                            self.grabbed = None;
                            BandGesture::End(index)
                        }
                        // Handled above, by hit test.
                        Gesture::Reset => return,
                        // There is nothing to type into here.
                        Gesture::Edit => return,
                    };
                    self.notify(cx, translated);
                }
            }

            Some(Grabbed::Focus) => {
                let mut gesture = None;
                event.map(|window_event, meta| match window_event {
                    WindowEvent::MouseDown(MouseButton::Left) => {
                        cx.capture();
                        cx.focus();
                        cx.set_active(true);
                        gesture = Some(BandGesture::FocusBegin);
                        meta.consume();
                    }
                    WindowEvent::MouseMove(x, _) => {
                        let delta = *x - self.last_x;
                        self.last_x = *x;
                        let fine = cx.modifiers().contains(Modifiers::SHIFT);
                        let value = self.focus.unwrap_or(0.5);
                        // `Drag::value_after` reads a *downward* delta, so the
                        // sign flips to make rightward increase. Going through
                        // it keeps the travel and the fine factor the same as
                        // every other control's.
                        let next = Drag::value_after(value, -delta, fine);
                        self.focus = Some(next);
                        cx.needs_redraw();
                        gesture = Some(BandGesture::FocusChange(next));
                    }
                    WindowEvent::MouseUp(MouseButton::Left) => {
                        cx.release();
                        cx.set_active(false);
                        gesture = Some(BandGesture::FocusEnd);
                        meta.consume();
                    }
                    _ => {}
                });
                if let Some(gesture) = gesture {
                    if gesture == BandGesture::FocusEnd {
                        self.grabbed = None;
                    }
                    self.notify(cx, gesture);
                }
            }

            None => {
                let mut over = None;
                let mut moved = false;
                event.map(|window_event, _| match window_event {
                    WindowEvent::MouseMove(x, y) => {
                        over = match self.target_at(bounds, scale, *x, *y) {
                            Some(Grabbed::Band(index)) => Some(index),
                            _ => None,
                        };
                        moved = true;
                    }
                    WindowEvent::MouseOut => {
                        over = None;
                        moved = true;
                    }
                    _ => {}
                });

                if moved && over != self.hovered {
                    self.hovered = over;
                    cx.needs_redraw();
                    self.notify(cx, BandGesture::Hover(over));
                }
            }
        }
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        let scale = cx.scale_factor();
        let line = scale.max(1.0);
        let (plot, rail) = Self::regions(bounds, scale);

        let at = |x: f32, y: f32| {
            (
                plot.x + x.clamp(0.0, 1.0) * plot.w,
                plot.y + (1.0 - y.clamp(0.0, 1.0)) * plot.h,
            )
        };

        // What came in, filled from the floor.
        //
        // **Translucent foreground, not a solid grey.** Grey over the tinted
        // regions came out muddy; light at low opacity lifts what is beneath it
        // instead of covering it. Neutral either way, because the accent belongs
        // to what the plugin is *set* to (`README.md`).
        if self.dry.len() > 1 {
            let mut path = vg::Path::new();
            let (first_x, _) = at(self.dry[0].0, 0.0);
            path.move_to(first_x, plot.y + plot.h);
            for (x, y) in &self.dry {
                let (px, py) = at(*x, *y);
                path.line_to(px, py);
            }
            let (last_x, _) = at(self.dry[self.dry.len() - 1].0, 0.0);
            path.line_to(last_x, plot.y + plot.h);
            path.close();
            canvas.fill_path(
                &path,
                &vg::Paint::color(theme::FOREGROUND.at(ANALYSIS_ALPHA).vg()),
            );
        }

        let soloing = self.bands.iter().any(|band| band.soloed);

        for band in &self.bands {
            let (left, _) = at(band.low.min(band.high), 0.0);
            let (right, _) = at(band.low.max(band.high), 0.0);
            let width = (right - left).max(0.0);
            if width <= 0.0 {
                continue;
            }

            // Soloing dims by hue as well as by alpha: a band that is off is
            // not a quieter version of the same thing.
            let tint = if soloing && !band.soloed {
                theme::SUBTLE
            } else {
                theme::ACCENT_DEEP.mix(theme::ACCENT_BRIGHT, band.tint.clamp(0.0, 1.0))
            };

            // What was asked for.
            let (_, set_top) = at(0.0, band.level);
            let mut set = vg::Path::new();
            set.rect(left, set_top, width, plot.y + plot.h - set_top);
            canvas.fill_path(&set, &vg::Paint::color(tint.at(SET_ALPHA).vg()));

            // And what is sounding. The gap between the two is what the
            // dynamics are doing, which is the only way they are visible.
            let live = band.live();
            let (_, live_top) = at(0.0, live);
            let mut path = vg::Path::new();
            path.rect(left, live_top, width, plot.y + plot.h - live_top);
            canvas.fill_path(&path, &vg::Paint::color(tint.at(LIVE_ALPHA).vg()));

            let mut edge = vg::Path::new();
            edge.rect(left, live_top - line * 0.5, width, line);
            canvas.fill_path(&edge, &vg::Paint::color(tint.vg()));
        }

        // The line everything is read against. Under the regions, because it
        // is the ground they stand on rather than something drawn over them.
        if let Some(unity) = self.unity {
            let (_, y) = at(0.0, unity);
            let mut path = vg::Path::new();
            path.move_to(plot.x, y);
            path.line_to(plot.x + plot.w, y);
            let mut paint = vg::Paint::color(theme::SUBTLE.vg());
            paint.set_line_width(line);
            canvas.stroke_path(&path, &paint);
        }

        let mut grid = vg::Path::new();
        for x in &self.grid {
            let (gx, _) = at(*x, 0.0);
            grid.move_to(gx, plot.y);
            grid.line_to(gx, plot.y + plot.h);
        }
        let mut paint = vg::Paint::color(theme::ELEVATED.vg());
        paint.set_line_width(line);
        canvas.stroke_path(&grid, &paint);

        // What is being added, on top of everything: it is the answer to the
        // question the panel exists to ask.
        if self.wet.len() > 1 {
            let mut path = vg::Path::new();
            for (index, (x, y)) in self.wet.iter().enumerate() {
                let (px, py) = at(*x, *y);
                if index == 0 {
                    path.move_to(px, py);
                } else {
                    path.line_to(px, py);
                }
            }
            let mut paint = vg::Paint::color(theme::ACCENT.vg());
            paint.set_line_width(WET_WIDTH * scale);
            paint.set_line_cap(vg::LineCap::Butt);
            canvas.stroke_path(&path, &paint);
        }

        // Whatever the caller is pointing at wins over the pointer's own
        // position, so a table row and this panel never disagree.
        if let Some(band) = self
            .highlighted
            .or(self.hovered)
            .and_then(|index| self.bands.get(index))
        {
            let (left, _) = at(band.low.min(band.high), 0.0);
            let (right, _) = at(band.low.max(band.high), 0.0);
            let mut ring = vg::Path::new();
            ring.rect(left, plot.y, (right - left).max(0.0), plot.h);
            let mut paint = vg::Paint::color(theme::ACCENT.vg());
            paint.set_line_width(line);
            canvas.stroke_path(&ring, &paint);
        }

        let dragging_rail = self.grabbed == Some(Grabbed::Focus);
        let mut strip = vg::Path::new();
        strip.rect(rail.x, rail.y, rail.w, rail.h);
        let colour = match (self.focus.is_some(), dragging_rail) {
            // Inert when there is nothing to write: it should not look grabbable.
            (false, _) => theme::BACKGROUND,
            (true, false) => theme::ELEVATED,
            (true, true) => theme::ACCENT_DIM,
        };
        canvas.fill_path(&strip, &vg::Paint::color(colour.vg()));

        // The axis marks continue through the rail, which is what says the rail
        // belongs to the axis rather than being a control that happens to sit
        // under it.
        if self.focus.is_some() {
            let mut ticks = vg::Path::new();
            for x in &self.grid {
                let gx = plot.x + x.clamp(0.0, 1.0) * plot.w;
                ticks.rect(gx - line * 0.5, rail.y, line, rail.h);
            }
            canvas.fill_path(&ticks, &vg::Paint::color(theme::BACKGROUND.vg()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> BoundingBox {
        BoundingBox {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        }
    }

    fn band(low: f32, high: f32) -> Band {
        Band {
            low,
            high,
            ..Band::default()
        }
    }

    #[test]
    fn the_rail_sits_under_the_plot_and_they_tile_the_view() {
        let (plot, rail) = BandField::regions(bounds(), 1.0);
        assert_eq!(plot.y, bounds().y);
        assert_eq!(plot.y + plot.h, rail.y);
        assert_eq!(rail.y + rail.h, bounds().y + bounds().h);
        assert_eq!(rail.h, RAIL);
    }

    /// A short view gives the plot priority; a rail that ate it would be worse
    /// than a thin rail.
    #[test]
    fn a_short_view_keeps_most_of_itself_for_the_plot() {
        let short = BoundingBox {
            h: 20.0,
            ..bounds()
        };
        let (plot, rail) = BandField::regions(short, 1.0);
        assert_eq!(rail.h, 5.0);
        assert_eq!(plot.h, 15.0);
    }

    #[test]
    fn a_position_inside_a_region_finds_it() {
        let bands = [band(0.0, 0.3), band(0.6, 1.0)];
        assert_eq!(BandField::band_at(&bands, 0.1), Some(0));
        assert_eq!(BandField::band_at(&bands, 0.8), Some(1));
    }

    #[test]
    fn a_position_in_no_region_finds_nothing() {
        let bands = [band(0.0, 0.3), band(0.6, 1.0)];
        assert_eq!(BandField::band_at(&bands, 0.45), None);
    }

    /// Parallel generators overlap on purpose, so the overlap has to belong to
    /// something better-defined than declaration order.
    #[test]
    fn an_overlap_belongs_to_the_nearer_centre() {
        let bands = [band(0.0, 0.5), band(0.4, 1.0)];
        assert_eq!(BandField::band_at(&bands, 0.42), Some(0));
        assert_eq!(BandField::band_at(&bands, 0.48), Some(1));
    }

    #[test]
    fn no_regions_means_nothing_to_find() {
        assert_eq!(BandField::band_at(&[], 0.5), None);
    }

    #[test]
    fn no_delta_means_what_was_set() {
        let full = Band {
            level: 0.8,
            ..Band::default()
        };
        assert_eq!(full.live(), 0.8);
    }

    /// **The half that could not be drawn before** (`SPK-10`): an upward
    /// compressor moves the gain the other way, and a picture that only sinks
    /// cannot say so.
    #[test]
    fn a_delta_moves_what_is_sounding_either_way() {
        let held = Band {
            level: 0.8,
            delta: -0.4,
            ..Band::default()
        };
        assert!((held.live() - 0.4).abs() < 1e-6);

        let lifted = Band {
            level: 0.5,
            delta: 0.3,
            ..Band::default()
        };
        assert!((lifted.live() - 0.8).abs() < 1e-6);
    }

    /// **A share of the whole height, not of `level`.** A band trimmed to
    /// nothing is still being compressed, and the picture has to say so.
    #[test]
    fn a_band_at_the_floor_can_still_be_lifted() {
        let lifted = Band {
            level: 0.0,
            delta: 0.6,
            ..Band::default()
        };
        assert!((lifted.live() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn what_is_sounding_stays_in_range() {
        for level in [-1.0f32, 0.0, 0.5, 1.0, 2.0] {
            for delta in [-2.0f32, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0] {
                let live = Band {
                    level,
                    delta,
                    ..Band::default()
                }
                .live();
                assert!((0.0..=1.0).contains(&live), "{level} {delta}: {live}");
            }
        }
    }
}
