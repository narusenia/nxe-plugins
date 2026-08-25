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

use nxe_ui::bar::Bar;
use nxe_ui::curve::{Curve, CurveView, Grip, Span};
use nxe_ui::input::Gesture;
use nxe_ui::knob::Knob;
use nxe_ui::polar::{FieldGesture, FieldPoint, PolarField};
use nxe_ui::segmented::SegmentedControl;
use nxe_ui::{font, icon, theme};
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
    /// Mirrors the Doubler's Detail disclosure, so the show/hide of a tall
    /// table can be exercised without a host.
    detail_open: bool,
}

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
    SetTone(usize, f32),
    Gesture(&'static str),
    ToggleDetail,
}

impl Demo {
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
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
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
            DemoEvent::SetTone(0, value) => {
                self.tone_lo = *value;
                self.refresh();
            }
            DemoEvent::SetTone(_, value) => {
                self.tone_hi = *value;
                self.refresh();
            }
            DemoEvent::SetSource(index) => self.source = *index,
            DemoEvent::Gesture(name) => self.last_gesture = (*name).to_owned(),
            DemoEvent::ToggleDetail => self.detail_open = !self.detail_open,
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

fn main() {
    Application::new(|cx| {
        theme::install(cx);

        let mut demo = Demo {
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
            tone_lo: 0.62,
            tone_hi: 0.44,
            tone_spread: 0.5,
            curves: Vec::new(),
            spans: Vec::new(),
            grips: Vec::new(),
            last_gesture: "—".to_owned(),
            detail_open: false,
        };
        demo.refresh();
        demo.build(cx);

        // The gallery grows every time a widget is added, so it scrolls from
        // the start rather than when someone notices it has stopped fitting.
        ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
            VStack::new(cx, |cx| {
                Label::new(cx, "nxe-ui").class("value");
                Label::new(cx, "tokens and widgets").class("subtle");

                colours(cx);
                knobs(cx);
                bars(cx);
                segments(cx);
                field(cx);
                curves(cx);
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

/// A titled surface, with the one-pixel top highlight the theme calls for.
fn panel(cx: &mut Context, title: &str, content: impl Fn(&mut Context)) {
    VStack::new(cx, |cx| {
        Element::new(cx).class("panel-highlight");
        Label::new(cx, title).class("label");
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

fn colours(cx: &mut Context) {
    panel(cx, "SURFACES", |cx| {
        HStack::new(cx, |cx| {
            swatch(cx, "background", theme::BACKGROUND);
            swatch(cx, "card", theme::CARD);
            swatch(cx, "elevated", theme::ELEVATED);
            swatch(cx, "border", theme::BORDER);
            swatch(cx, "highlight", theme::HIGHLIGHT);
        })
        .class("row")
        .height(Auto);
    });

    panel(cx, "TEXT AND ACCENT", |cx| {
        HStack::new(cx, |cx| {
            swatch(cx, "foreground", theme::FOREGROUND);
            swatch(cx, "muted", theme::MUTED);
            swatch(cx, "subtle", theme::SUBTLE);
            swatch(cx, "accent", theme::ACCENT);
            swatch(cx, "accent-bright", theme::ACCENT_BRIGHT);
            swatch(cx, "accent-dim", theme::ACCENT_DIM);
        })
        .class("row")
        .height(Auto);
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

fn field(cx: &mut Context) {
    panel(cx, "POLAR FIELD", |cx| {
        // The anchors come from the SOURCE control: one source in the middle,
        // or two sitting either side of it.
        PolarField::new(
            cx,
            Demo::field,
            Demo::source.map(|source| {
                if *source == 0 {
                    vec![FieldPoint {
                        angle: 0.0,
                        radius: 0.0,
                        ..FieldPoint::default()
                    }]
                } else {
                    vec![
                        FieldPoint {
                            angle: -0.30,
                            radius: 0.10,
                            ..FieldPoint::default()
                        },
                        FieldPoint {
                            angle: 0.30,
                            radius: 0.10,
                            ..FieldPoint::default()
                        },
                    ]
                }
            }),
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
            },
        )
        .height(Pixels(180.0))
        .width(Stretch(1.0));

        Label::new(
            cx,
            "drag a dot to move both axes · shift = fine · VOICES dims the unused              ones · SOURCE splits the anchor",
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
            );
            Label::new(cx, "DETAIL").class("label");
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
                .background_color(Demo::last_gesture.map(move |_| theme::CARD.vizia()));
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
            for size in [12.0, 16.0, 20.0, 28.0] {
                icon::label(cx, icon::WAVES).font_size(size);
            }
            icon::label(cx, icon::WAVES)
                .font_size(28.0)
                .color(theme::ACCENT.vizia());
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
                Label::new(cx, name).class("subtle").width(Pixels(24.0));
                for _ in 0..4 {
                    Element::new(cx)
                        .size(Pixels(16.0))
                        .background_color(theme::ACCENT.vizia())
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
        Label::new(cx, "LABEL — names a thing, 12 px, muted").class("label");
        Label::new(cx, "Value — says what it is, 13 px, Geist Sans").class("value");
        Label::new(cx, "Subtle — gridlines, units, disabled rows").class("subtle");
        Element::new(cx).class("divider");
        font::value(cx, "-12.0 ct    22.0 ms    L70    0.0 dB");
        Label::new(
            cx,
            "figures are Geist Mono: a digit changing does not shift the rest",
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
