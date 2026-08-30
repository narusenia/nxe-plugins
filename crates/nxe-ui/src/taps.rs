//! The pattern of an early-reflection set, drawn against time.
//!
//! **One axis, and it is both time and distance.** The direct sound stands at
//! the left edge; every reflection stands where it arrives. Moving a voice away
//! moves the weight of the picture to the right, which is the same gesture the
//! ear makes sense of — so the figure is the mechanism rather than a
//! visualisation of it (`plugins/vocal-depth/docs/specifications/ui.md`).
//!
//! ## Why not a spectrum
//!
//! A spectrum is the right figure when a plugin's subject is *where in the
//! frequency range* something happens — Velour's bands, Air's layer. Distance
//! is not a spectral quantity: what says "far away" is the direct-to-reflected
//! ratio and the spread of arrival times, and neither of those is visible in a
//! spectrum at all.
//!
//! ## What it does not draw
//!
//! **No decay curve, no tail.** The plugin has neither (`REQ-VDP-020`), and a
//! drawn envelope would promise one. **No frequency information**: the damping
//! is a readout, not a shape here, because two quantities in one picture with no
//! axis for the second is how a figure stops being readable.

use crate::theme;
use vizia::prelude::*;
use vizia::vg;

/// How wide a reflection's stem is, in logical pixels.
const STEM: f32 = 2.0;

/// How wide the direct sound's stem is. **Wider than a reflection** — it is a
/// different kind of thing, not a louder one of the same kind.
const DIRECT_STEM: f32 = 4.0;

/// Under this a reflection is not drawn.
///
/// A one-pole analyser never reaches zero, so without a floor the picture keeps
/// a permanent haze of stems at the bottom, which reads as a room that never
/// stops (`SPK-16`).
const FAINTEST: f32 = 0.02;

/// **Asserted at compile time, not in a test** (`.agents/rules/rust.md`): a
/// floor of zero would let a decaying analyser leave a permanent haze, and one
/// too high would swallow a reflection worth seeing. The direct sound is a
/// different *kind* of thing, so it is drawn differently rather than louder.
const _: () = assert!(FAINTEST > 0.0 && FAINTEST < 0.1);
const _: () = assert!(DIRECT_STEM > STEM);

/// One arrival.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tap {
    /// Where it arrives, `0..=1` across the figure's time span.
    pub position: f32,
    /// How loud it is, `0..=1`.
    pub level: f32,
}

impl Data for Tap {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

enum TapEvent {
    Taps(Vec<Tap>),
    Direct(f32),
}

pub struct TapField {
    taps: Vec<Tap>,
    direct: f32,
}

impl TapField {
    /// `taps` are the arrivals and `direct` is the level of the sound that did
    /// not travel, both normalised.
    pub fn new<'a>(
        cx: &'a mut Context,
        taps: impl Res<Vec<Tap>> + 'static,
        direct: impl Res<f32> + 'static,
    ) -> Handle<'a, Self> {
        Self {
            taps: taps.get_val(cx),
            direct: direct.get_val(cx).clamp(0.0, 1.0),
        }
        .build(cx, move |cx| {
            let entity = cx.current();
            taps.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, TapEvent::Taps(value));
            });
            direct.set_or_bind(cx, entity, move |cx, value| {
                cx.emit_to(entity, TapEvent::Direct(value));
            });
        })
    }
}

impl View for TapField {
    fn element(&self) -> Option<&'static str> {
        Some("nxetaps")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|tap_event: &TapEvent, _| {
            match tap_event {
                TapEvent::Taps(taps) => self.taps = taps.clone(),
                TapEvent::Direct(level) => self.direct = level.clamp(0.0, 1.0),
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

        // **Depth from a one-pixel border, not a fill** (`.agents/rules/ui.md`).
        let mut frame = vg::Path::new();
        frame.rect(bounds.x, bounds.y, bounds.w, bounds.h);
        canvas.stroke_path(
            &frame,
            &vg::Paint::color(palette.line.vg()).with_line_width(line),
        );

        // The floor the stems stand on, which is what makes the picture read as
        // arrivals over time rather than as a bar chart.
        let mut floor = vg::Path::new();
        floor.move_to(bounds.x, bottom - line);
        floor.line_to(bounds.x + bounds.w, bottom - line);
        canvas.stroke_path(
            &floor,
            &vg::Paint::color(palette.line.vg()).with_line_width(line),
        );

        // **The ramp spans the whole plot**, so two stems of the same height are
        // the same colour whatever else is on screen (`Palette::paint`).
        let paint = palette.paint(bounds.x, bottom, bounds.x, bounds.y);

        // **Every stem in one path, filled once.** They all take the same
        // paint, and femtovg gives every `fill_path` its own draw call whatever
        // is in it (`docs/investigations/ui-frame-cost.md`).
        let mut stems = vg::Path::new();
        for tap in &self.taps {
            if !tap.level.is_finite() || tap.level < FAINTEST {
                continue;
            }
            let x = bounds.x + tap.position.clamp(0.0, 1.0) * bounds.w;
            let height = tap.level.clamp(0.0, 1.0) * bounds.h;
            stems.rect(
                x - STEM * scale * 0.5,
                bottom - height,
                STEM * scale,
                height,
            );
        }

        // The direct sound is in the same path. It was drawn last so an early
        // reflection could not hide it; with one fill there is nothing to hide
        // it behind, and it is wider than a reflection's stem anyway.
        if self.direct.is_finite() && self.direct >= FAINTEST {
            let height = self.direct.clamp(0.0, 1.0) * bounds.h;
            stems.rect(bounds.x, bottom - height, DIRECT_STEM * scale, height);
        }

        canvas.fill_path(&stems, &paint);
    }
}
