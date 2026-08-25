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

use nxe_ui::theme;
use vizia::prelude::*;

fn main() {
    Application::new(|cx| {
        theme::install(cx);

        // The gallery grows every time a widget is added, so it scrolls from
        // the start rather than when someone notices it has stopped fitting.
        ScrollView::new(cx, 0.0, 0.0, false, true, |cx| {
            VStack::new(cx, |cx| {
                Label::new(cx, "nxe-ui").class("value");
                Label::new(cx, "tokens and widgets").class("subtle");

                colours(cx);
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
        Label::new(cx, "Value — says what it is, 13 px").class("value");
        Label::new(cx, "Subtle — gridlines, units, disabled rows").class("subtle");
        Element::new(cx).class("divider");
        Label::new(cx, "-12.0 ct    22.0 ms    L70    0.0 dB").class("value");
        Label::new(cx, "fixed decimals, right aligned: the numbers move").class("subtle");
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
