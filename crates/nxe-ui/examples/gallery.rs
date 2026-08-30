//! Every `nxe-ui` widget and token as a plain desktop app, so UI work can be
//! iterated without launching a DAW. A widget that is not in here cannot be
//! reviewed without one, so it will not be (`.agents/rules/vizia.md`).
//!
//! This opens a standalone **baseview** window, not a winit one: the two vizia
//! backends are mutually exclusive and the plugin needs baseview (see the
//! `vizia` entry in the workspace `Cargo.toml`). The upside is that the gallery
//! runs on the same backend the plugin does.
//!
//! Run it with `mise run gallery`.
//!
//! ## Measuring
//!
//! **An idle window should cost nothing**, and `ps -o %cpu` on this is the
//! whole test (`.agents/rules/vizia.md`). But idle is not the state a plugin is
//! in: a plugin's heartbeat rewrites the model while audio runs, and *that* is
//! what a host pays for — the idle case was fixed once and the moving one was
//! never measured. `NXE_GALLERY_HZ` rewrites the model at a given rate so the
//! moving window can be measured the same way:
//!
//! ```text
//! NXE_GALLERY_HZ=30 cargo run --release -p nxe-ui --example gallery
//! ```
//!
//! `NXE_GALLERY_SCROLL` does the same for scrolling, which is the case a
//! partial redraw cannot help with — the whole view moves, so the whole window
//! is drawn:
//!
//! ```text
//! NXE_GALLERY_SCROLL=60 cargo run --release -p nxe-ui --example gallery
//! ```
//!
//! **No plugin window scrolls**, so this measures the gallery rather than the
//! product. It is here because "scrolling feels heavy" is a thing that gets
//! reported, and guessing at it is how this investigation went wrong twice.
//!
//! Unset or `0` leaves the window idle, which stays the default.
//!
//! ## Palettes
//!
//! `NXE_GALLERY_PALETTE` picks which plugin's palette the **stylesheet** is
//! built from (`doubler` / `velour` / `sparkleur` / `air` / `parallax`;
//! `air` is the default). It has to be chosen at startup because vizia has no
//! way to replace a stylesheet once it is added.
//!
//! **The custom-drawn widgets do not need it.** They read the palette from the
//! nearest `Palette` model above them, so the PALETTES panel shows all five at
//! once — that is the half of `theme::install` the plugins never exercise.

use nxe_ui::band::{Band, BandField, BandFieldModifiers, BandGesture};
use nxe_ui::bar::Bar;
use nxe_ui::curve::{Curve, CurveView, CurveViewModifiers, Grip, Span};
use nxe_ui::dots::DotField;
use nxe_ui::entry::ValueEntry;
use nxe_ui::input::Gesture;
use nxe_ui::knob::Knob;
use nxe_ui::meter::Meter;
use nxe_ui::polar::{FieldGesture, FieldPoint, PolarField, PolarFieldModifiers};
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, icon, theme};
use std::time::Duration;
use vizia::prelude::*;

/// Somewhere for the demo controls to keep their values. A plugin has its
/// parameters instead; what matters here is that the widgets accept a lens, so
/// a value changed from outside reaches them.
#[derive(Lens)]
struct Demo {
    detune: f32,
    delay: f32,
    mix: f32,
    /// Four values per row, laid out like the Doubler's Detail table:
    /// delay, detune, pan, gain.
    rows: Vec<f32>,
    voices: usize,
    source: usize,
    /// The Doubler's default shape, read as pan and delay.
    field: Vec<FieldPoint>,
    /// How far the source markers sit from the origin. Dragging one moves them
    /// all, which is how the Doubler puts its dry level on the figure.
    anchor_radius: f32,
    /// Derived from `source` and `anchor_radius`, for the same reason `curves`
    /// is derived: a lens can only map one field.
    anchors: Vec<FieldPoint>,
    /// Stand-ins for what `nxe-dsp` measures. A gallery has no audio, so these
    /// are a fixed shape — enough to see that the layers stack in the right
    /// order and read as "the signal" rather than as another setting.
    density: Vec<f32>,
    analysis: Curve,
    /// Shelf gains and scatter, normalized with 0.5 as flat.
    tone_lo: f32,
    tone_hi: f32,
    tone_spread: f32,
    /// Derived from the three above. A lens can only map one field, so a curve
    /// that depends on two has to be computed when they change rather than when
    /// it is read — which is also cheaper.
    curves: Vec<Curve>,
    spans: Vec<Span>,
    grips: Vec<Grip>,
    last_gesture: String,
    /// What the typed-into value reads. Whatever comes back is kept verbatim:
    /// the gallery has no units to parse.
    typed: String,
    /// Mirrors the Doubler's Detail disclosure, so the show/hide of a tall
    /// table can be exercised without a host.
    detail_open: bool,
    /// Three parallel band regions, the way Velour reads them. Their edges are
    /// derived from `focus`, so a lens can only map one field — same reason
    /// `curves` is derived.
    bands: Vec<Band>,
    /// Moves all three regions together, `0.5` at rest.
    focus: f32,
    /// `0` for none, otherwise the band index plus one.
    solo: usize,
    /// Which region the pointer is over. Fed straight back into the widget's
    /// `highlight`, which is how a figure and a table point at the same thing.
    hovered: Option<usize>,
    /// What came in, and what is being added to it.
    dry: Curve,
    added: Curve,
    /// Stands in for a signal level, so the meters have something to do. The
    /// guards below bite when it gets loud.
    meter: f32,
    /// The grain field's two spectra and how coherent the grains are. The
    /// layer is a stand-in for something an additive plugin adds; `alignment`
    /// is what a `BLEND`-like control would move.
    grains: Curve,
    /// A stand-in for an early-reflection pattern: where each arrival lands and
    /// how loud it is, plus the direct sound that did not travel.
    taps: Vec<nxe_ui::taps::Tap>,
    direct: f32,
    distance: f32,
    alignment: f32,
    /// Keeps `NXE_GALLERY_HZ`'s heartbeat running; dropped with the window
    /// (`nxe_ui::heartbeat`). **Never read on purpose.**
    #[allow(dead_code)]
    motion: Option<nxe_ui::heartbeat::Lifeline>,
    /// Counts the motion steps, so the shapes above have a phase.
    step: u32,
    /// Keeps `NXE_GALLERY_SCROLL`'s heartbeat running. **Never read on
    /// purpose.**
    #[allow(dead_code)]
    scrolling: Option<nxe_ui::heartbeat::Lifeline>,
    /// How far down the scroll is, and which way it is going.
    scroll: f32,
    scroll_down: bool,
    /// What `nxe_ui::readout` prints and how far its bar is along. **Built on
    /// the motion step, not inside the lens** — which is the whole point of the
    /// widget's own note.
    readouts: Vec<String>,
    gauge: f32,
}

/// One step of `NXE_GALLERY_SCROLL`'s motion.
///
/// Scrolling is the case a partial redraw cannot help with — the whole view
/// moves — so it is the one worth being able to measure without a hand on the
/// trackpad.
#[derive(Clone)]
struct Scroll;

/// One step of `NXE_GALLERY_HZ`'s motion. Its own type rather than a
/// `DemoEvent` variant, because the heartbeat needs `Clone` and `DemoEvent`
/// carries the typed string.
#[derive(Clone)]
struct Advance;

enum DemoEvent {
    Set(usize, f32),
    SetRow(usize, f32),
    SetVoices(usize),
    SetSource(usize),
    MovePoint {
        index: usize,
        angle: f32,
        radius: f32,
    },
    ResetPoint(usize),
    MoveAnchors(f32),
    SetTone(usize, f32),
    Gesture(&'static str),
    Typed(String),
    ToggleDetail,
    SetBand(usize, f32),
    ResetBand(usize),
    SetFocus(f32),
    SetAlignment(f32),
    SetDistance(f32),
    HoverBand(Option<usize>),
    SetSolo(usize),
    SetMeter(f32),
}

impl Demo {
    /// The band regions' resting edges in Hz. Velour's, near enough to see the
    /// widget work.
    const BAND_EDGES: [(f32, f32); 3] = [(90.0, 520.0), (480.0, 5_200.0), (4_800.0, 20_000.0)];

    /// Recomputes everything derived from `focus`, `solo` and `meter`.
    fn refresh_bands(&mut self) {
        // An octave either way, which is what a voice-range control has to
        // cover: a male fundamental near 110 Hz against a female one near 220.
        let shift = ((self.focus - 0.5) * 2.0).exp2();

        for (index, band) in self.bands.iter_mut().enumerate() {
            let (low, high) = Self::BAND_EDGES[index];
            band.low = log_x(low * shift);
            band.high = log_x(high * shift);
            band.soloed = self.solo == index + 1;
            // A stand-in for the dynamics, in **both** directions: the upper
            // two bands are pulled back once the signal gets loud, and the
            // bottom one is lifted while it is quiet. Watching the solid part
            // move either side of the set outline is the whole point of the
            // reading being signed (`SPK-10`).
            band.delta = if index == 0 {
                ((0.60 - self.meter) * 1.5).clamp(0.0, 0.30)
            } else {
                -((self.meter - 0.72) * 3.0).clamp(0.0, 0.8) * band.level
            };
        }

        self.added = added_curve(&self.bands);
    }

    /// Recomputes everything derived from the tone values.
    fn refresh(&mut self) {
        self.curves = vec![shelf_curve(self.tone_lo, self.tone_hi)];
        self.spans = spread_spans(self.tone_spread);
        self.grips = vec![
            (log_x(SHELF_LOW_HZ), self.tone_lo),
            (log_x(SHELF_HIGH_HZ), self.tone_hi),
        ];
    }
}

impl Model for Demo {
    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        // **What a plugin's heartbeat does**: rewrite the reactive copies from
        // something that moves. The shapes are arbitrary; what matters is that
        // the numbers a window draws keep changing, because that is the state
        // the CPU cost lives in.
        event.map(|_: &Advance, _| {
            self.step = self.step.wrapping_add(1);
            let phase = self.step as f32 / 64.0;
            self.meter = 0.5 + 0.45 * (phase * std::f32::consts::TAU).sin();
            self.detune = 0.5 + 0.4 * (phase * std::f32::consts::TAU * 0.37).sin();
            self.direct = 0.5 + 0.4 * (phase * std::f32::consts::TAU * 0.61).sin();
            for (index, tap) in self.taps.iter_mut().enumerate() {
                tap.level =
                    0.5 + 0.4 * ((phase + index as f32 * 0.1) * std::f32::consts::TAU).sin();
            }
            for (index, band) in self.bands.iter_mut().enumerate() {
                band.level =
                    0.5 + 0.4 * ((phase + index as f32 * 0.2) * std::f32::consts::TAU).cos();
            }
            for (index, text) in self.readouts.iter_mut().enumerate() {
                let db = -60.0 * (0.5 + 0.5 * ((phase + index as f32 * 0.3) * 6.0).sin());
                *text = format!("{db:+.1}");
            }
            self.gauge = 0.5 + 0.5 * (phase * 4.0).sin();
        });

        event.map(|_: &Scroll, _| {
            let step = 0.02;
            if self.scroll_down {
                self.scroll += step;
                if self.scroll >= 1.0 {
                    self.scroll = 1.0;
                    self.scroll_down = false;
                }
            } else {
                self.scroll -= step;
                if self.scroll <= 0.0 {
                    self.scroll = 0.0;
                    self.scroll_down = true;
                }
            }
            // **Down the tree, not up.** The model sits at the root and the
            // scroll view is below it, so the default upward propagation would
            // never reach it.
            cx.emit_custom(
                Event::new(ScrollEvent::SetY(self.scroll))
                    .target(Entity::root())
                    .propagate(Propagation::Subtree),
            );
        });

        event.map(|demo_event: &DemoEvent, _| match demo_event {
            DemoEvent::Set(0, value) => self.detune = *value,
            DemoEvent::Set(1, value) => self.delay = *value,
            DemoEvent::Set(2, value) => self.mix = *value,
            DemoEvent::Set(_, value) => {
                self.tone_spread = *value;
                self.refresh();
            }
            DemoEvent::SetRow(index, value) => self.rows[*index] = *value,
            DemoEvent::SetVoices(index) => {
                self.voices = *index;
                let live = [2, 4, 8][*index];
                for (i, point) in self.field.iter_mut().enumerate() {
                    point.enabled = i < live;
                }
            }
            DemoEvent::MovePoint {
                index,
                angle,
                radius,
            } => {
                self.field[*index].angle = *angle;
                self.field[*index].radius = *radius;
            }
            DemoEvent::ResetPoint(index) => {
                self.field[*index] = default_field()[*index];
            }
            DemoEvent::Typed(text) => self.typed = text.clone(),
            DemoEvent::MoveAnchors(radius) => {
                self.anchor_radius = *radius;
                self.anchors = anchors_of(self.source, self.anchor_radius);
            }
            DemoEvent::SetTone(0, value) => {
                self.tone_lo = *value;
                self.refresh();
            }
            DemoEvent::SetTone(_, value) => {
                self.tone_hi = *value;
                self.refresh();
            }
            DemoEvent::SetSource(index) => {
                self.source = *index;
                self.anchors = anchors_of(self.source, self.anchor_radius);
            }
            DemoEvent::Gesture(name) => self.last_gesture = (*name).to_owned(),
            DemoEvent::ToggleDetail => self.detail_open = !self.detail_open,
            DemoEvent::SetBand(index, level) => {
                self.bands[*index].level = *level;
                self.added = added_curve(&self.bands);
            }
            DemoEvent::ResetBand(index) => {
                self.bands[*index].level = [0.55, 0.70, 0.45][*index];
                self.added = added_curve(&self.bands);
            }
            DemoEvent::SetAlignment(value) => self.alignment = *value,
            // **Derived when the input changes, not in a lens** — a lens can
            // only map one field, and the pattern comes from the distance and
            // the tap table together (`.agents/rules/vizia.md`).
            DemoEvent::SetDistance(value) => {
                self.distance = *value;
                self.taps = sample_taps(*value);
            }
            DemoEvent::SetFocus(value) => {
                self.focus = *value;
                self.refresh_bands();
            }
            DemoEvent::HoverBand(over) => self.hovered = *over,
            DemoEvent::SetSolo(index) => {
                self.solo = *index;
                self.refresh_bands();
            }
            DemoEvent::SetMeter(value) => {
                self.meter = *value;
                self.refresh_bands();
            }
        });
    }
}

/// The Doubler's default shape as field points: pan on the angle, delay on the
/// radius, alternating which source they hang off.
fn default_field() -> Vec<FieldPoint> {
    const SHAPE: [(f32, f32); 8] = [
        (-1.00, 1.00),
        (1.00, 0.62),
        (-0.45, 0.84),
        (0.45, 0.44),
        (-0.75, 0.92),
        (0.75, 0.30),
        (-0.20, 0.72),
        (0.20, 0.52),
    ];

    SHAPE
        .iter()
        .enumerate()
        .map(|(index, (angle, radius))| FieldPoint {
            angle: *angle,
            radius: *radius,
            size: 0.5,
            anchor: index % 2,
            // Four voices to start with, matching the VOICES control.
            enabled: index < 4,
            // A step along the accent per pair, the way the Doubler reads them.
            tint: (index / 2) as f32 / 3.0,
        })
        .collect()
}

fn name_of(gesture: Gesture) -> &'static str {
    match gesture {
        Gesture::Begin => "begin",
        Gesture::Change(_) => "change",
        Gesture::End => "end",
        Gesture::Reset => "reset (double click)",
        Gesture::Edit => "edit (cmd click)",
    }
}

/// A rate in Hz from an environment variable. Zero — the default — means off.
fn rate_from(name: &str) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// How fast to rewrite the model, from `NXE_GALLERY_HZ`. Zero — the default —
/// leaves the window idle.
fn motion_hz() -> u32 {
    rate_from("NXE_GALLERY_HZ")
}

/// Which palette the stylesheet is built from, out of `NXE_GALLERY_PALETTE`.
///
/// **Only the stylesheet.** Everything drawn by hand follows whichever
/// `Palette` model is nearest above it, which is why the PALETTES panel can
/// show five at once.
fn palette_from_env() -> theme::Palette {
    let name = std::env::var("NXE_GALLERY_PALETTE").unwrap_or_default();
    theme::Palette::ALL
        .into_iter()
        .find(|(plugin, _)| plugin.eq_ignore_ascii_case(&name))
        .map(|(_, palette)| palette)
        .unwrap_or(theme::Palette::AIR)
}

fn main() {
    Application::new(|cx| {
        theme::install(cx, palette_from_env());

        // **Started before the model, because the model holds what stops it**
        // (`nxe_ui::heartbeat`).
        let scrolling = match rate_from("NXE_GALLERY_SCROLL") {
            0 => None,
            hz => Some(nxe_ui::heartbeat::start(
                cx,
                Duration::from_nanos(1_000_000_000 / u64::from(hz)),
                Scroll,
            )),
        };

        let motion = match motion_hz() {
            0 => None,
            hz => Some(nxe_ui::heartbeat::start(
                cx,
                Duration::from_nanos(1_000_000_000 / u64::from(hz)),
                Advance,
            )),
        };

        let mut demo = Demo {
            motion,
            scrolling,
            scroll: 0.0,
            scroll_down: true,
            step: 0,
            readouts: vec![String::new(); 3],
            gauge: 0.0,
            detune: 0.24,
            delay: 0.62,
            mix: 0.4,
            rows: vec![
                1.00, 0.00, 0.00, 0.65, //
                0.62, 1.00, 1.00, 0.65, //
                0.84, 0.30, 0.28, 0.50,
            ],
            voices: 1,
            source: 0,
            field: default_field(),
            anchor_radius: 0.10,
            anchors: anchors_of(0, 0.10),
            density: sample_density(),
            analysis: sample_analysis(),
            tone_lo: 0.62,
            tone_hi: 0.44,
            tone_spread: 0.5,
            curves: Vec::new(),
            spans: Vec::new(),
            grips: Vec::new(),
            last_gesture: "—".to_owned(),
            typed: "22.0 ms".to_owned(),
            detail_open: false,
            bands: [0.55f32, 0.70, 0.45]
                .iter()
                .enumerate()
                .map(|(index, level)| Band {
                    level: *level,
                    // A step along the accent per band, so three regions of one
                    // hue stay distinguishable.
                    tint: index as f32 / 2.0,
                    ..Band::default()
                })
                .collect(),
            focus: 0.5,
            solo: 0,
            hovered: None,
            dry: sample_analysis(),
            added: Vec::new(),
            meter: 0.62,
            grains: sample_grains(),
            taps: sample_taps(0.5),
            direct: 0.85,
            distance: 0.5,
            alignment: 0.35,
        };
        demo.refresh();
        demo.refresh_bands();
        demo.build(cx);

        // The gallery grows every time a widget is added, so it scrolls from
        // the start rather than when someone notices it has stopped fitting.
        ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
            VStack::new(cx, |cx| {
                nxe_ui::header::header(cx, "nxe-ui", "tokens and widgets");

                grid(cx);
                readouts(cx);
                colours(cx);
                knobs(cx);
                bars(cx);
                segments(cx);
                field(cx);
                curves(cx);
                band_field(cx);
                dot_field(cx);
                tap_field(cx);
                meters(cx);
                detail(cx);
                icons(cx);
                shapes(cx);
                spacing(cx);
                text(cx);
                states(cx);
            })
            .class("root")
            .height(Auto);
        })
        .background_color(theme::BACKGROUND.vizia());
    })
    .title("nxe-ui gallery")
    .inner_size((760, 720))
    .run();
}

/// A titled surface. The title is an eyebrow over a rule, which is how a region
/// is named everywhere in this design (`crates/nxe-ui/README.md`).
fn panel(cx: &mut Context, title: &str, content: impl Fn(&mut Context)) {
    VStack::new(cx, |cx| {
        VStack::new(cx, |cx| {
            Label::new(cx, title).class("eyebrow");
        })
        .class("heading");
        content(cx);
    })
    .class("panel")
    .height(Auto);
}

fn swatch(cx: &mut Context, name: &str, token: theme::Token) {
    VStack::new(cx, |cx| {
        Element::new(cx)
            .width(Pixels(72.0))
            .height(Pixels(40.0))
            .background_color(token.vizia())
            .border_width(Pixels(1.0))
            .border_color(theme::BORDER.vizia())
            .border_radius(Pixels(theme::RADIUS_CONTROL));
        Label::new(cx, name).class("subtle");
    })
    .width(Auto)
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1));
}

/// The readout strip as a widget, with figures that move.
///
/// **`grid` below is the design; this is the thing plugins actually build.**
/// It is here because it is the widget that changes most often on screen — a
/// window's readouts tick along with the audio — and because it is the one
/// whose cost was missed for that reason
/// (`docs/investigations/ui-frame-cost.md`).
fn readouts(cx: &mut Context) {
    panel(cx, "READOUT", |cx| {
        nxe_ui::readout::strip(cx, |cx| {
            nxe_ui::readout::cell(cx, "IN", Demo::readouts.index(0), "dB");
            nxe_ui::readout::cell(cx, "OUT", Demo::readouts.index(1), "dB");
            nxe_ui::readout::cell(cx, "REDUCTION", Demo::readouts.index(2), "dB");
            nxe_ui::readout::meter_cell(cx, "GATE", Demo::gauge);
        });
        Label::new(
            cx,
            "NXE_GALLERY_HZ moves these; the box is fixed so they cannot",
        )
        .class("subtle");
    });
}

/// The Swiss layer: eyebrows over rules, one readout per region, and the accent
/// as a gradient rather than a block.
///
/// **This panel is the design, not a widget.** Everything below it is a control
/// that happens to be styled; this is the grid those controls sit on, shown on
/// its own so it can be judged without a knob in the way.
fn grid(cx: &mut Context) {
    panel(cx, "SWISS LAYER", |cx| {
        HStack::new(cx, |cx| {
            for (name, readout, unit) in [
                ("DETECTION", "-18.4", "dB"),
                ("REDUCTION", "-6.2", "dB"),
                ("OUTPUT", "-0.3", "dB"),
            ] {
                VStack::new(cx, |cx| {
                    VStack::new(cx, |cx| {
                        Label::new(cx, name).class("eyebrow");
                    })
                    .class("heading");

                    HStack::new(cx, |cx| {
                        font::value(cx, readout).class("readout");
                        Label::new(cx, unit).class("subtle");
                    })
                    .height(Auto)
                    .width(Auto)
                    .col_between(Pixels(theme::SPACE_1))
                    .child_top(Stretch(1.0));
                })
                .width(Stretch(1.0))
                .height(Auto)
                .row_between(Pixels(theme::SPACE_2));
            }
        })
        .height(Auto)
        .col_between(Pixels(theme::SPACE_4));

        Element::new(cx).class("rule-accent");

        // The two rules, side by side, so the weight difference is visible.
        VStack::new(cx, |cx| {
            Label::new(cx, "rule").class("subtle");
            Element::new(cx).class("rule");
            Label::new(cx, "rule-accent").class("subtle");
            Element::new(cx).class("rule-accent");
        })
        .height(Auto)
        .row_between(Pixels(theme::SPACE_2));

        // The gradient fill, horizontal and vertical.
        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                Label::new(cx, "accent").class("subtle");
                Element::new(cx).class("accent").height(Pixels(10.0));
            })
            .width(Stretch(1.0))
            .height(Auto)
            .row_between(Pixels(theme::SPACE_1));

            VStack::new(cx, |cx| {
                Label::new(cx, "accent-up").class("subtle");
                Element::new(cx)
                    .class("accent-up")
                    .width(Pixels(10.0))
                    .height(Pixels(48.0));
            })
            .width(Auto)
            .height(Auto)
            .row_between(Pixels(theme::SPACE_1));
        })
        .height(Auto)
        .col_between(Pixels(theme::SPACE_4));
    });
}

fn colours(cx: &mut Context) {
    panel(cx, "SURFACES", |cx| {
        HStack::new(cx, |cx| {
            swatch(cx, "background", theme::BACKGROUND);
            swatch(cx, "elevated", theme::ELEVATED);
            swatch(cx, "border", theme::BORDER);
        })
        .class("row")
        .height(Auto);
    });

    panel(cx, "TEXT", |cx| {
        HStack::new(cx, |cx| {
            swatch(cx, "foreground", theme::FOREGROUND);
            swatch(cx, "muted", theme::MUTED);
            swatch(cx, "subtle", theme::SUBTLE);
        })
        .class("row")
        .height(Auto);
    });

    palettes(cx);
}

/// The five accent ramps, and the same drawn widget under each of them.
///
/// **The swatches prove the values; the bars prove the path.** A swatch is an
/// `Element` with its colour set directly, so it would look right even if
/// nothing could reach the palette at draw time. The `Bar` under each nested
/// `Palette` model is the actual mechanism the plugins use — if that path
/// breaks, this row goes uniform and the swatches above it do not.
fn palettes(cx: &mut Context) {
    panel(cx, "PALETTES", |cx| {
        HStack::new(cx, |cx| {
            for (name, palette) in theme::Palette::ALL {
                VStack::new(cx, move |cx| {
                    palette.build(cx);
                    HStack::new(cx, move |cx| {
                        swatch(cx, "wash", palette.wash);
                        swatch(cx, "bright", palette.bright);
                        swatch(cx, "accent", palette.accent);
                        swatch(cx, "deep", palette.deep);
                    })
                    .class("row")
                    .height(Auto);
                    Bar::new(cx, 0.62, |_, _| {});
                    Label::new(cx, name).class("label");
                })
                .width(Auto)
                .height(Auto)
                .row_between(Pixels(theme::SPACE_2));
            }
        })
        .class("row")
        .height(Auto)
        .col_between(Pixels(theme::SPACE_5));
    });
}

/// One labelled knob. Generic over the lens because every field's lens is its
/// own type, so they cannot share a list.
fn knob_column<L>(cx: &mut Context, index: usize, label: &'static str, lens: L, size: f32)
where
    L: Lens<Target = f32> + Copy,
{
    VStack::new(cx, |cx| {
        Knob::new(cx, lens, move |cx, gesture| {
            if let Gesture::Change(value) = gesture {
                cx.emit(DemoEvent::Set(index, value));
            }
            cx.emit(DemoEvent::Gesture(name_of(gesture)));
        })
        .size(Pixels(size));
        Label::new(cx, label).class("label");
        font::value(cx, lens.map(|value| format!("{value:.3}")));
    })
    .width(Auto)
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn knobs(cx: &mut Context) {
    panel(cx, "KNOBS", |cx| {
        HStack::new(cx, |cx| {
            knob_column(cx, 0, "DETUNE", Demo::detune, 56.0);
            knob_column(cx, 1, "DELAY", Demo::delay, 56.0);
            knob_column(cx, 2, "MIX", Demo::mix, 34.0);
            knob_column(cx, 3, "TONE SPREAD", Demo::tone_spread, 34.0);
        })
        .class("row")
        .height(Auto);

        Element::new(cx).class("divider");

        HStack::new(cx, |cx| {
            Label::new(
                cx,
                "drag · shift = fine · double click = reset · cmd click = type",
            )
            .class("subtle");
            Label::new(cx, Demo::last_gesture).class("value");
        })
        .class("row")
        .height(Auto);
    });
}

fn bars(cx: &mut Context) {
    // Column headings, then one row of bars per voice — the shape the Doubler's
    // Detail table takes.
    const COLUMNS: [(&str, bool); 4] = [
        ("DELAY", false),
        ("DETUNE", true),
        ("PAN", true),
        ("GAIN", false),
    ];

    panel(cx, "BARS", |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, "").class("subtle").width(Pixels(20.0));
            for (name, _) in COLUMNS {
                Label::new(cx, name).class("label").width(Stretch(1.0));
            }
        })
        .class("row")
        .height(Auto);

        for row in 0..3 {
            HStack::new(cx, |cx| {
                Label::new(cx, &format!("{}", row + 1))
                    .class("subtle")
                    .width(Pixels(20.0));

                for (column, (_, centred)) in COLUMNS.iter().enumerate() {
                    let index = row * COLUMNS.len() + column;
                    let lens = Demo::rows.index(index);
                    let gesture = move |cx: &mut EventContext, gesture: Gesture| {
                        if let Gesture::Change(value) = gesture {
                            cx.emit(DemoEvent::SetRow(index, value));
                        }
                        cx.emit(DemoEvent::Gesture(name_of(gesture)));
                    };

                    if *centred {
                        Bar::bipolar(cx, lens, gesture)
                    } else {
                        Bar::new(cx, lens, gesture)
                    }
                    .height(Pixels(10.0))
                    .width(Stretch(1.0));
                }
            })
            .class("row")
            .height(Auto);
        }

        Label::new(cx, "same gesture as a knob, including the vertical drag").class("subtle");
    });
}

fn segments(cx: &mut Context) {
    panel(cx, "SEGMENTED", |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, "VOICES").class("label");
            SegmentedControl::new(cx, Demo::voices, &["2", "4", "8"], |cx, index| {
                cx.emit(DemoEvent::SetVoices(index));
            });

            Label::new(cx, "SOURCE").class("label");
            SegmentedControl::new(
                cx,
                Demo::source,
                &["Mono Sum", "True Stereo"],
                |cx, index| cx.emit(DemoEvent::SetSource(index)),
            );
        })
        .class("row")
        .height(Auto);

        Label::new(cx, "click to select; 150 ms on hover and selection").class("subtle");
    });
}

/// A plausible stereo picture: energy either side of centre, less in between.
/// Absolute — `0` draws nothing, which is what silence looks like.
fn sample_density() -> Vec<f32> {
    (0..24)
        .map(|bin| {
            let x = bin as f32 / 23.0 * 2.0 - 1.0;
            (1.0 - (x.abs() - 0.6).abs() * 2.5).clamp(0.0, 1.0)
        })
        .collect()
}

/// A plausible spectrum: full low end, rolling off above the middle.
fn sample_analysis() -> Curve {
    (0..=48)
        .map(|step| {
            let x = step as f32 / 48.0;
            let level = (1.0 - x).powf(1.6) * 0.45 + 0.05;
            (x, level)
        })
        .collect()
}

/// One source marker in the middle, or two either side of it. All of them sit
/// at the same radius, which is what the anchor drag moves.
fn anchors_of(source: usize, radius: f32) -> Vec<FieldPoint> {
    let angles: &[f32] = if source == 0 { &[0.0] } else { &[-0.30, 0.30] };
    angles
        .iter()
        .map(|angle| FieldPoint {
            angle: *angle,
            radius,
            ..FieldPoint::default()
        })
        .collect()
}

fn field(cx: &mut Context) {
    panel(cx, "POLAR FIELD", |cx| {
        // The anchors come from the SOURCE control: one source in the middle,
        // or two sitting either side of it.
        PolarField::new(
            cx,
            Demo::field,
            Demo::anchors,
            |cx, gesture| match gesture {
                FieldGesture::Change {
                    index,
                    angle,
                    radius,
                } => {
                    cx.emit(DemoEvent::MovePoint {
                        index,
                        angle,
                        radius,
                    });
                    cx.emit(DemoEvent::Gesture("change"));
                }
                FieldGesture::Reset(index) => {
                    cx.emit(DemoEvent::ResetPoint(index));
                    cx.emit(DemoEvent::Gesture("reset (double click)"));
                }
                FieldGesture::Begin(_) => cx.emit(DemoEvent::Gesture("begin")),
                FieldGesture::End(_) => cx.emit(DemoEvent::Gesture("end")),
                FieldGesture::Hover(Some(_)) => cx.emit(DemoEvent::Gesture("hover")),
                FieldGesture::Hover(None) => {}
                FieldGesture::AnchorChange(radius) => {
                    cx.emit(DemoEvent::MoveAnchors(radius));
                    cx.emit(DemoEvent::Gesture("anchor change"));
                }
                FieldGesture::AnchorReset => {
                    cx.emit(DemoEvent::MoveAnchors(FieldPoint::default().radius));
                    cx.emit(DemoEvent::Gesture("anchor reset (double click)"));
                }
                FieldGesture::AnchorBegin => cx.emit(DemoEvent::Gesture("anchor begin")),
                FieldGesture::AnchorEnd => cx.emit(DemoEvent::Gesture("anchor end")),
            },
        )
        .density(Demo::density)
        .height(Pixels(180.0))
        .width(Stretch(1.0));

        Label::new(
            cx,
            "drag a dot to move both axes · shift = fine · SOURCE splits the anchor · the wedges behind are what is arriving",
        )
        .class("subtle");
    });
}

/// 20 Hz to 20 kHz on a log axis, normalized. The widget knows nothing about
/// this — mapping the axis is the caller's job, which is also what lets the
/// caller place the labels.
fn log_x(hz: f32) -> f32 {
    (hz / 20.0).log10() / (20_000.0f32 / 20.0).log10()
}

fn hz_at(x: f32) -> f32 {
    20.0 * 10.0f32.powf(x * (20_000.0f32 / 20.0).log10())
}

const SHELF_LOW_HZ: f32 = 200.0;
const SHELF_HIGH_HZ: f32 = 4_000.0;

/// A stand-in for two shelves, good enough to see the widget work. The plugin
/// computes its real response from the same filter coefficients the DSP uses
/// (`DBL-14`); this is not that.
fn shelf_curve(lo: f32, hi: f32) -> Curve {
    const LOW_HZ: f32 = SHELF_LOW_HZ;
    const HIGH_HZ: f32 = SHELF_HIGH_HZ;
    let lo_db = (lo - 0.5) * 24.0;
    let hi_db = (hi - 0.5) * 24.0;

    (0..=64)
        .map(|step| {
            let x = step as f32 / 64.0;
            let hz = hz_at(x);
            let low_weight = 1.0 / (1.0 + (hz / LOW_HZ).powi(2));
            let high_weight = 1.0 / (1.0 + (HIGH_HZ / hz).powi(2));
            let db = lo_db * low_weight + hi_db * high_weight;
            (x, 0.5 + db / 24.0)
        })
        .collect()
}

/// Where Tone Spread puts each voice's band, as a stand-in for the real scatter.
fn spread_spans(spread: f32) -> Vec<Span> {
    const OFFSETS: [(f32, f32); 4] = [(0.85, 0.15), (-0.70, 0.80), (0.35, 0.55), (-0.95, 0.25)];
    if spread <= 0.0 {
        return Vec::new();
    }
    OFFSETS
        .iter()
        .map(|(high, low)| {
            let highpass = 20.0 * (high * spread * 3.5).exp2();
            let lowpass = 20_000.0 / (low.abs() * spread * 2.5).exp2();
            (log_x(highpass), log_x(lowpass))
        })
        .collect()
}

fn curves(cx: &mut Context) {
    const MARKS: [(f32, &str); 5] = [
        (20.0, "20"),
        (200.0, "200"),
        (1000.0, "1k"),
        (5000.0, "5k"),
        (20000.0, "20k"),
    ];

    panel(cx, "CURVE VIEW", |cx| {
        CurveView::new(
            cx,
            Demo::curves,
            Demo::spans,
            Demo::grips,
            MARKS.iter().map(|(hz, _)| log_x(*hz)).collect(),
            |cx, index, gesture| {
                if let Gesture::Change(value) = gesture {
                    cx.emit(DemoEvent::SetTone(index, value));
                }
                cx.emit(DemoEvent::Gesture(name_of(gesture)));
            },
        )
        .analysis(Demo::analysis)
        .height(Pixels(120.0))
        .width(Stretch(1.0));

        // The axis labels are the caller's, placed with the caller's own
        // mapping — the widget cannot draw text at arbitrary positions.
        HStack::new(cx, |cx| {
            for (hz, text) in MARKS {
                Label::new(cx, text)
                    .class("subtle")
                    .position_type(PositionType::SelfDirected)
                    .left(Percentage(log_x(hz) * 100.0));
            }
        })
        .height(Pixels(16.0))
        .width(Stretch(1.0));

        Label::new(
            cx,
            "drag a handle vertically · the shaded bands are Tone Spread",
        )
        .class("subtle");
    });

    transfer(cx);
}

/// The same view read against a line of its own.
///
/// A filter response is read against a level, which is the horizontal line the
/// view draws by default. **A transfer curve is read against the diagonal** —
/// below it is compression, above it is lift — and a diagonal cannot be a
/// gridline, because those are vertical. `.reference(…)` is the way in.
fn transfer(cx: &mut Context) {
    // A compressor with a soft knee either way: the shape `SPK-14` needs.
    const KNEE: f32 = 0.12;
    let curve: Curve = (0..=96)
        .map(|step| {
            let x = step as f32 / 96.0;
            // Squash above the upper threshold, lift below the lower one, and
            // leave the middle alone.
            let y = if x > 0.7 {
                0.7 + (x - 0.7) * 0.35
            } else if x < 0.3 {
                0.3 - (0.3 - x) * 0.55
            } else {
                x
            };
            // Round the two corners so both knees read as curves.
            (x, y * (1.0 - KNEE) + x * KNEE)
        })
        .collect();
    let diagonal: Curve = vec![(0.0, 0.0), (1.0, 1.0)];

    panel(cx, "CURVE VIEW · reference", move |cx| {
        CurveView::new(
            cx,
            vec![curve.clone()],
            Vec::<Span>::new(),
            Vec::<Grip>::new(),
            Vec::new(),
            |_cx, _index, _gesture| {},
        )
        .reference(diagonal.clone())
        .height(Pixels(120.0))
        .width(Stretch(1.0));

        Label::new(cx, "read only · in against out, against the diagonal").class("subtle");
    });
}

/// What the three generators are adding, as a bell per band scaled by whatever
/// is getting through it. Not a real spectrum — the point is that raising a
/// region visibly raises the curve above it, which is the reading the panel
/// exists to give.
fn added_curve(bands: &[Band]) -> Curve {
    (0..=96)
        .map(|step| {
            let x = step as f32 / 96.0;
            let level: f32 = bands
                .iter()
                .map(|band| {
                    let centre = (band.low + band.high) * 0.5;
                    let width = (band.high - band.low).max(0.02);
                    let bell = (-((x - centre) / (width * 0.6)).powi(2)).exp();
                    band.live() * bell * 0.55
                })
                .sum();
            (x, level.min(0.95))
        })
        .collect()
}

fn band_gesture_name(gesture: BandGesture) -> &'static str {
    match gesture {
        BandGesture::Begin(_) => "begin",
        BandGesture::Change { .. } => "change",
        BandGesture::End(_) => "end",
        BandGesture::Reset(_) => "reset (double click)",
        BandGesture::Hover(_) => "hover",
        BandGesture::FocusBegin => "focus begin",
        BandGesture::FocusChange(_) => "focus change",
        BandGesture::FocusEnd => "focus end",
        BandGesture::FocusReset => "focus reset (double click)",
    }
}

fn band_field(cx: &mut Context) {
    const MARKS: [(f32, &str); 5] = [
        (20.0, "20"),
        (200.0, "200"),
        (1000.0, "1k"),
        (5000.0, "5k"),
        (20000.0, "20k"),
    ];

    panel(cx, "BAND FIELD", |cx| {
        BandField::new(
            cx,
            Demo::bands,
            Demo::dry,
            Demo::added,
            MARKS.iter().map(|(hz, _)| log_x(*hz)).collect(),
            |cx, gesture| {
                match gesture {
                    BandGesture::Change { index, level } => {
                        cx.emit(DemoEvent::SetBand(index, level))
                    }
                    BandGesture::Reset(index) => cx.emit(DemoEvent::ResetBand(index)),
                    BandGesture::Hover(over) => cx.emit(DemoEvent::HoverBand(over)),
                    BandGesture::FocusChange(value) => cx.emit(DemoEvent::SetFocus(value)),
                    BandGesture::FocusReset => cx.emit(DemoEvent::SetFocus(0.5)),
                    _ => {}
                }
                cx.emit(DemoEvent::Gesture(band_gesture_name(gesture)));
            },
        )
        // Whatever the pointer is over comes straight back in, which is the
        // linkage a plugin uses to light up the matching table row.
        .highlight(Demo::hovered)
        // Wiring this is what makes the rail at the bottom live.
        .focus(Demo::focus)
        // And this is what draws the line a signed reading is read against. A
        // field whose regions grow from the floor simply does not call it.
        .unity(0.5f32)
        .height(Pixels(150.0))
        .width(Stretch(1.0));

        // The axis labels are the caller's, placed with the caller's own
        // mapping — the widget cannot draw text at arbitrary positions.
        HStack::new(cx, |cx| {
            for (hz, text) in MARKS {
                Label::new(cx, text)
                    .class("subtle")
                    .position_type(PositionType::SelfDirected)
                    .left(Percentage(log_x(hz) * 100.0));
            }
        })
        .height(Pixels(16.0))
        .width(Stretch(1.0));

        HStack::new(cx, |cx| {
            Label::new(cx, "SOLO").class("label");
            SegmentedControl::new(
                cx,
                Demo::solo,
                &["—", "BODY", "PRES", "AIR"],
                |cx, index| {
                    cx.emit(DemoEvent::SetSolo(index));
                },
            );

            Label::new(cx, "FOCUS").class("label");
            font::value(cx, Demo::focus.map(|value| format!("{value:.3}")));
        })
        .class("row")
        .height(Auto);

        Label::new(
            cx,
            "drag a region vertically · drag the rail at the bottom to move all three",
        )
        .class("subtle");
    });
}

/// A stand-in for a generated layer: a broad rise with a couple of peaks in it,
/// so the field has something with shape to draw.
/// Ten arrivals with a window over them, the shape Vocal Depth's reflections
/// have. `distance` slides the window, which is what its `DEPTH` does.
fn sample_taps(distance: f32) -> Vec<nxe_ui::taps::Tap> {
    const MS: [f32; 10] = [11.0, 13.0, 17.0, 23.0, 31.0, 43.0, 53.0, 67.0, 79.0, 89.0];
    MS.iter()
        .map(|&ms| {
            let position = (ms - 10.0) / 110.0;
            let offset = position - distance;
            let window = (-(offset * offset) / (2.0 * 0.45 * 0.45)).exp();
            nxe_ui::taps::Tap {
                position,
                level: window * (11.0f32 / ms).sqrt() * 0.8,
            }
        })
        .collect()
}

fn sample_grains() -> Curve {
    const COLUMNS: usize = 32;
    (0..COLUMNS)
        .map(|index| {
            let x = index as f32 / (COLUMNS - 1) as f32;
            // A hump toward the top, with two harmonics standing out of it.
            let hump = (1.0 - (x - 0.72).abs() * 2.4).max(0.0);
            let peak = |at: f32| (1.0 - (x - at).abs() * 22.0).max(0.0) * 0.5;
            ((hump * 0.8 + peak(0.55) + peak(0.78)) * 0.9).clamp(0.0, 1.0)
        })
        .enumerate()
        .map(|(index, level)| (index as f32 / (COLUMNS - 1) as f32, level))
        .collect()
}

fn tap_field(cx: &mut Context) {
    panel(cx, "TAP FIELD", |cx| {
        nxe_ui::taps::TapField::new(cx, Demo::taps, Demo::direct)
            .height(Pixels(150.0))
            .width(Stretch(1.0));

        HStack::new(cx, |cx| {
            Label::new(cx, "DISTANCE").class("label");
            Bar::new(cx, Demo::distance, |cx, gesture| {
                if let Gesture::Change(value) = gesture {
                    cx.emit(DemoEvent::SetDistance(value));
                }
                if let Gesture::Reset = gesture {
                    cx.emit(DemoEvent::SetDistance(0.5));
                }
                cx.emit(DemoEvent::Gesture(name_of(gesture)));
            })
            .width(Pixels(160.0))
            .height(Pixels(10.0));
        })
        .class("row")
        .col_between(Pixels(theme::SPACE_2));
    });
}

fn dot_field(cx: &mut Context) {
    panel(cx, "DOT FIELD", |cx| {
        DotField::new(cx, Demo::analysis, Demo::grains, Demo::alignment)
            .height(Pixels(150.0))
            .width(Stretch(1.0));

        HStack::new(cx, |cx| {
            Label::new(cx, "ALIGNMENT").class("label");
            Bar::new(cx, Demo::alignment, |cx, gesture| {
                if let Gesture::Change(value) = gesture {
                    cx.emit(DemoEvent::SetAlignment(value));
                }
                if let Gesture::Reset = gesture {
                    cx.emit(DemoEvent::SetAlignment(0.35));
                }
                cx.emit(DemoEvent::Gesture(name_of(gesture)));
            })
            .width(Pixels(160.0))
            .height(Pixels(10.0));
            font::value(cx, Demo::alignment.map(|value| format!("{value:.2}")));
        })
        .class("row")
        .height(Auto);

        Label::new(
            cx,
            "the line is what came in · the grains are what was added · \
             alignment pulls them onto their columns",
        )
        .class("subtle");
    });
}

/// The scale the meters are drawn against, so the marks and the values agree.
const METER_FLOOR_DB: f32 = -60.0;

fn meter_mark(db: f32) -> f32 {
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
}

/// One labelled bar. The hold marker sits a little above the bar because that
/// is what a hold looks like — the DSP keeps the real one (`nxe_dsp::Level`).
fn meter_column<L>(cx: &mut Context, label: &'static str, level: L)
where
    L: Lens<Target = f32> + Copy,
{
    let marks = vec![meter_mark(-18.0), meter_mark(-6.0), meter_mark(0.0)];

    VStack::new(cx, |cx| {
        Meter::new(
            cx,
            level,
            level.map(|value| (value + 0.08).min(1.0)),
            marks.clone(),
        )
        .width(Pixels(10.0))
        .height(Pixels(120.0));
        Label::new(cx, label).class("label");
    })
    .width(Auto)
    .height(Auto)
    .row_between(Pixels(theme::SPACE_1))
    .child_left(Stretch(1.0))
    .child_right(Stretch(1.0));
}

fn meters(cx: &mut Context) {
    panel(cx, "METER", |cx| {
        HStack::new(cx, |cx| {
            VStack::new(cx, |cx| {
                Knob::new(cx, Demo::meter, |cx, gesture| {
                    if let Gesture::Change(value) = gesture {
                        cx.emit(DemoEvent::SetMeter(value));
                    }
                    cx.emit(DemoEvent::Gesture(name_of(gesture)));
                })
                .size(Pixels(44.0));
                Label::new(cx, "LEVEL").class("label");
            })
            .width(Auto)
            .height(Auto)
            .row_between(Pixels(theme::SPACE_1))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));

            // In and out side by side, because the question a saturator's meters
            // answer is "is this louder, or is it better".
            meter_column(cx, "IN L", Demo::meter);
            meter_column(cx, "IN R", Demo::meter);
            meter_column(cx, "OUT L", Demo::meter.map(|value| value * 0.85));
            meter_column(cx, "OUT R", Demo::meter.map(|value| value * 0.85));
        })
        .class("row")
        .col_between(Pixels(theme::SPACE_3))
        .height(Auto);

        Meter::horizontal(
            cx,
            Demo::meter,
            Demo::meter.map(|value| (value + 0.08).min(1.0)),
            vec![meter_mark(-18.0), meter_mark(-6.0), meter_mark(0.0)],
        )
        .width(Stretch(1.0))
        .height(Pixels(8.0));

        Label::new(
            cx,
            "turn LEVEL up past the marks · the hold marker turns white at full scale",
        )
        .class("subtle");
    });
}

/// The Doubler's Detail table, in the shape that broke in a host: eight rows of
/// bars behind a disclosure, with per-row opacity and background bound to
/// lenses.
fn detail(cx: &mut Context) {
    const ROW_HEIGHT: f32 = 22.0;

    panel(cx, "DETAIL (disclosure)", |cx| {
        HStack::new(cx, |cx| {
            icon::label(
                cx,
                Demo::detail_open.map(|open| {
                    if *open {
                        icon::CHEVRON_UP
                    } else {
                        icon::CHEVRON_DOWN
                    }
                }),
            )
            .class("decoration");
            Label::new(cx, "DETAIL").class("label").class("decoration");
        })
        .class("hoverable")
        .width(Pixels(96.0))
        .height(Pixels(22.0))
        .col_between(Pixels(theme::SPACE_1))
        .on_press(|cx| cx.emit(DemoEvent::ToggleDetail));

        VStack::new(cx, |cx| {
            for row in 0..8 {
                HStack::new(cx, |cx| {
                    font::value(cx, &format!("{}", row + 1))
                        .class("subtle")
                        .width(Pixels(18.0));
                    for column in 0..4 {
                        let index = (row * 4 + column) % 12;
                        HStack::new(cx, |cx| {
                            Bar::new(cx, Demo::rows.index(index), move |cx, gesture| {
                                if let Gesture::Change(value) = gesture {
                                    cx.emit(DemoEvent::SetRow(index, value));
                                }
                            })
                            .height(Pixels(10.0))
                            .width(Stretch(1.0));
                            font::value(cx, Demo::rows.index(index).map(|v| format!("{v:.2}")))
                                .width(Pixels(48.0));
                        })
                        .width(Stretch(1.0))
                        .height(Stretch(1.0))
                        .col_between(Pixels(theme::SPACE_2));
                    }
                })
                .class("row")
                .height(Pixels(ROW_HEIGHT))
                .opacity(Demo::voices.map(move |voices| {
                    if row < [2usize, 4, 8][*voices] {
                        1.0
                    } else {
                        0.42
                    }
                }))
                .background_color(Demo::last_gesture.map(move |_| theme::BACKGROUND.vizia()));
            }
        })
        .height(Auto)
        .row_between(Pixels(theme::SPACE_2))
        .display(Demo::detail_open);
    });
}

fn icons(cx: &mut Context) {
    panel(cx, "ICONS", |cx| {
        // The ones the Doubler UI actually uses. Anything added to a plugin
        // gets a row here in the same change (`.agents/rules/vizia.md`).
        HStack::new(cx, |cx| {
            for (glyph, name) in [
                (icon::CHEVRON_DOWN, "chevron-down"),
                (icon::CHEVRON_UP, "chevron-up"),
                (icon::SLIDERS_HORIZONTAL, "sliders-horizontal"),
                (icon::ROTATE_CCW, "rotate-ccw"),
            ] {
                VStack::new(cx, |cx| {
                    icon::label(cx, glyph).font_size(24.0);
                    Label::new(cx, name).class("subtle");
                })
                .width(Auto)
                .height(Auto)
                .row_between(Pixels(theme::SPACE_1));
            }
        })
        .class("row")
        .height(Auto);

        Element::new(cx).class("divider");

        // Size and colour are `font-size` and `color`, like any other text.
        HStack::new(cx, |cx| {
            let accent = theme::palette(cx).accent.vizia();
            for size in [12.0, 16.0, 20.0, 28.0] {
                icon::label(cx, icon::WAVES).font_size(size);
            }
            icon::label(cx, icon::WAVES).font_size(28.0).color(accent);
        })
        .class("row")
        .height(Auto);

        Label::new(cx, "2035 icons; stroke width is fixed by the font").class("subtle");
    });
}

fn shapes(cx: &mut Context) {
    panel(cx, "RADII", |cx| {
        HStack::new(cx, |cx| {
            for (name, radius) in [
                ("control 2", theme::RADIUS_CONTROL),
                ("card 3", theme::RADIUS_CARD),
                ("round (dots only)", 20.0),
            ] {
                VStack::new(cx, |cx| {
                    Element::new(cx)
                        .size(Pixels(40.0))
                        .background_color(theme::ELEVATED.vizia())
                        .border_width(Pixels(1.0))
                        .border_color(theme::BORDER.vizia())
                        .border_radius(Pixels(radius));
                    Label::new(cx, name).class("subtle");
                })
                .width(Auto)
                .height(Auto)
                .row_between(Pixels(theme::SPACE_1));
            }
        })
        .class("row")
        .height(Auto);
    });
}

fn spacing(cx: &mut Context) {
    panel(cx, "SPACING", |cx| {
        for (name, gap) in [
            ("4", theme::SPACE_1),
            ("8", theme::SPACE_2),
            ("12", theme::SPACE_3),
            ("16", theme::SPACE_4),
            ("24", theme::SPACE_5),
        ] {
            HStack::new(cx, |cx| {
                let accent = theme::palette(cx).accent.vizia();
                Label::new(cx, name).class("subtle").width(Pixels(24.0));
                for _ in 0..4 {
                    Element::new(cx)
                        .size(Pixels(16.0))
                        .background_color(accent)
                        .border_radius(Pixels(theme::RADIUS_CONTROL));
                }
            })
            .col_between(Pixels(gap))
            .height(Auto);
        }
    });
}

fn text(cx: &mut Context) {
    panel(cx, "TEXT", |cx| {
        // Both, side by side: the wordmark is the one place the bold face is
        // used, so the panel has to show what it looks like against the plain
        // one it is an exception to.
        font::title(cx, "TITLE — the wordmark, 17, bold");
        Label::new(cx, "TITLE — the same class, regular").class("title");
        Label::new(cx, "LABEL — names a thing, 12, muted").class("label");
        Label::new(cx, "Value — says what it is, 10, Geist Sans").class("value");
        Label::new(cx, "Subtle — gridlines, units, disabled rows").class("subtle");
        Element::new(cx).class("divider");
        font::value(cx, "-12.0 ct    22.0 ms    L70    0.0 dB");
        Label::new(
            cx,
            "figures are Geist Mono: a digit changing does not shift the rest",
        )
        .class("subtle");

        Element::new(cx).class("divider");
        ValueEntry::new(cx, Demo::typed, |cx, typed| {
            cx.emit(DemoEvent::Typed(typed.to_owned()));
        });
        Label::new(
            cx,
            "click a value to type into it; Enter commits, Escape cancels",
        )
        .class("subtle");
    });
}

fn states(cx: &mut Context) {
    panel(cx, "STATES", |cx| {
        HStack::new(cx, |cx| {
            Label::new(cx, "hover me").class("hoverable");
            Label::new(cx, "hover me too").class("hoverable");
            Label::new(cx, "disabled")
                .class("hoverable")
                .class("disabled");
        })
        .class("row")
        .height(Auto);
        Label::new(cx, "150 ms on hover and selection, never on a value").class("subtle");
    });
}
