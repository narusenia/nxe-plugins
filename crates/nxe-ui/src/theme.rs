//! The design tokens, and the stylesheet generated from them.
//!
//! **The Rust constants here are the source of truth.** Custom-drawn widgets
//! need colours as values, so a stylesheet cannot be the authority without the
//! two drifting apart; the CSS is generated from these instead. It also means
//! this works whatever vizia's CSS does or does not support in the way of
//! custom properties.
//!
//! The palette is documented in
//! `plugins/doubler/docs/specifications/ui.md`: neutral surfaces in a few
//! steps, one accent, one-pixel borders, no shadows, depth from contrast.

use crate::font;
use crate::icon;
use vizia::prelude::*;
use vizia::vg;

/// A colour token. Carries its own alpha so the translucent tokens are the same
/// kind of thing as the opaque ones.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Token {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: f32,
}

impl Token {
    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 1.0,
        }
    }

    const fn rgba(red: u8, green: u8, blue: u8, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// For the generated stylesheet.
    pub fn css(self) -> String {
        format!(
            "rgba({}, {}, {}, {})",
            self.red, self.green, self.blue, self.alpha
        )
    }

    /// For `View::draw`.
    pub fn vg(self) -> vg::Color {
        vg::Color::rgba(
            self.red,
            self.green,
            self.blue,
            (self.alpha * 255.0).round() as u8,
        )
    }

    /// For the few places vizia wants its own colour type.
    pub fn vizia(self) -> Color {
        Color::rgba(
            self.red,
            self.green,
            self.blue,
            (self.alpha * 255.0).round() as u8,
        )
    }

    /// The same colour at a different opacity. Used for dimming a disabled
    /// control rather than desaturating it.
    pub fn at(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }
}

// Surfaces. Fully neutral — every channel equal — so the accent is the only
// hue anywhere in the window. A slight blue cast in the greys reads as a
// colour scheme rather than as a background.
pub const BACKGROUND: Token = Token::rgb(0x0A, 0x0A, 0x0A);
pub const CARD: Token = Token::rgb(0x14, 0x14, 0x14);
pub const ELEVATED: Token = Token::rgb(0x1F, 0x1F, 0x1F);
pub const BORDER: Token = Token::rgb(0x2A, 0x2A, 0x2A);
/// The one-pixel lift along the top of a surface. Most of the "shadcn feel" is
/// this line.
pub const HIGHLIGHT: Token = Token::rgba(0xFF, 0xFF, 0xFF, 0.04);

// Text. Neutral for the same reason as the surfaces.
pub const FOREGROUND: Token = Token::rgb(0xFA, 0xFA, 0xFA);
pub const MUTED: Token = Token::rgb(0xA3, 0xA3, 0xA3);
pub const SUBTLE: Token = Token::rgb(0x73, 0x73, 0x73);

// One accent, and no other hue anywhere.
pub const ACCENT: Token = Token::rgb(0x38, 0xBD, 0xF8);
pub const ACCENT_BRIGHT: Token = Token::rgb(0x7D, 0xD3, 0xFC);
pub const ACCENT_DIM: Token = Token::rgba(0x38, 0xBD, 0xF8, 0.18);

/// Corner radii. Deliberately small: the design is angular, and the corners are
/// there to stop a one-pixel border from looking chipped, not to soften
/// anything. Set both to zero for hard corners — nothing else has to change.
pub const RADIUS_CONTROL: f32 = 2.0;
pub const RADIUS_CARD: f32 = 3.0;

// Guards, not tests: someone reaching for a comfortable radius later should be
// stopped by the compiler rather than by a test they might not run.
const _: () = assert!(RADIUS_CONTROL <= 3.0, "controls are getting round");
const _: () = assert!(RADIUS_CARD <= 4.0, "surfaces are getting round");

/// The spacing scale. Five steps on a four-pixel grid; nothing between them.
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;

/// Two text sizes. Labels name things, values say what they are.
///
/// The value is the *smaller* of the two on purpose: it is set in mono at full
/// contrast, which already makes it the thing the eye lands on. At the label's
/// size it read as a headline under every knob.
pub const FONT_LABEL: f32 = 10.0;
pub const FONT_VALUE: f32 = 8.0;

/// Hover and selection only. Never a value: a knob that lags the mouse feels
/// broken.
pub const TRANSITION_MS: u32 = 150;

/// The stylesheet, built from the constants above.
pub fn stylesheet() -> String {
    let background = BACKGROUND.css();
    let card = CARD.css();
    let elevated = ELEVATED.css();
    let border = BORDER.css();
    let highlight = HIGHLIGHT.css();
    let foreground = FOREGROUND.css();
    let muted = MUTED.css();
    let subtle = SUBTLE.css();
    let accent = ACCENT.css();
    let accent_dim = ACCENT_DIM.css();

    format!(
        "
/* An element selector, not a class: vizia's default text colour is black, so
   every label without a class would come out unreadable on a dark surface.
   The classes below then only have to say what is *different*. */
label {{
    color: {foreground};
    font-size: {FONT_VALUE}px;
}}

.root {{
    background-color: {background};
    child-space: {SPACE_5}px;
    row-between: {SPACE_4}px;
}}

.panel {{
    background-color: {card};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CARD}px;
    child-space: {SPACE_4}px;
    row-between: {SPACE_3}px;
}}

/* One pixel along the top of a surface, which is where the sense of a raised
   card comes from without a shadow. Placed as the first child of a panel. */
.panel-highlight {{
    height: 1px;
    width: 1s;
    background-color: {highlight};
}}

.section {{
    layout-type: column;
    row-between: {SPACE_2}px;
}}

.row {{
    layout-type: row;
    col-between: {SPACE_3}px;
    child-top: 1s;
    child-bottom: 1s;
}}

.divider {{
    height: 1px;
    width: 1s;
    background-color: {border};
}}

.label {{
    color: {muted};
    font-size: {FONT_LABEL}px;
}}

.value {{
    color: {foreground};
    font-size: {FONT_VALUE}px;
}}

.subtle {{
    color: {subtle};
    font-size: {FONT_LABEL}px;
}}

/* Colour only. The family is set by `icon::label`, because `font-family` in a
   stylesheet does not select the embedded font on this vizia revision. */
.icon {{
    color: {muted};
}}

/* Disabled controls lose contrast rather than colour: the accent stays the
   only hue in the window. */
.disabled {{
    color: {subtle};
}}

.track {{
    background-color: {elevated};
    border-radius: {RADIUS_CONTROL}px;
}}

.accent {{
    background-color: {accent};
    border-radius: {RADIUS_CONTROL}px;
}}

/* A row of choices. The container holds the groove; the segments sit inside it
   so the selected one reads as raised out of the track. */
.segmented {{
    layout-type: row;
    background-color: {elevated};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CONTROL}px;
    child-space: 2px;
    col-between: 2px;
    height: auto;
    width: auto;
}}

.segment {{
    color: {muted};
    font-size: {FONT_LABEL}px;
    child-space: 1s;
    child-left: {SPACE_2}px;
    child-right: {SPACE_2}px;
    height: 22px;
    border-radius: {RADIUS_CONTROL}px;
    transition: background-color {TRANSITION_MS}ms, color {TRANSITION_MS}ms;
}}

.segment:hover {{
    color: {foreground};
}}

.segment:checked {{
    color: {background};
    background-color: {accent};
}}

/* Content inside something pressable. vizia only emits a press when the entity
   hovered on mouse-up is the one hovered on mouse-down, so a hit-testable child
   makes the container's press fire only when the pointer happens not to cross
   between children. Decoration must not be hit-testable. */
.decoration {{
    pointer-events: none;
}}

.hoverable {{
    color: {foreground};
    background-color: {elevated};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CONTROL}px;
    child-space: {SPACE_2}px;
    transition: background-color {TRANSITION_MS}ms, border-color {TRANSITION_MS}ms;
}}

.hoverable:hover {{
    background-color: {accent_dim};
    border-color: {accent};
}}
"
    )
}

/// Installs the typeface, the icon font and the stylesheet. Call once when the
/// window is built.
pub fn install(cx: &mut Context) {
    font::install(cx);
    icon::install(cx);
    cx.add_stylesheet(CSS::String(stylesheet()))
        .expect("the generated stylesheet is built from constants and cannot fail to parse");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated stylesheet must not contain a colour that did not come
    /// from a token — that is the whole point of generating it.
    #[test]
    fn the_stylesheet_has_no_colour_literals() {
        let css = stylesheet();
        assert!(!css.contains('#'), "a hex colour reached the stylesheet");
    }

    /// Every token has to survive the round trip into CSS, or a typo in `css`
    /// would silently produce a transparent or black surface.
    #[test]
    fn tokens_render_as_rgba() {
        assert_eq!(ACCENT.css(), "rgba(56, 189, 248, 1)");
        assert_eq!(ACCENT_DIM.css(), "rgba(56, 189, 248, 0.18)");
        assert_eq!(BACKGROUND.css(), "rgba(10, 10, 10, 1)");
    }

    /// The surfaces and the text have to be neutral: the accent is the only
    /// hue the design allows, and a grey with a cast stops reading as a
    /// background.
    #[test]
    fn the_neutrals_have_no_hue() {
        for (name, token) in [
            ("background", BACKGROUND),
            ("card", CARD),
            ("elevated", ELEVATED),
            ("border", BORDER),
            ("highlight", HIGHLIGHT),
            ("foreground", FOREGROUND),
            ("muted", MUTED),
            ("subtle", SUBTLE),
        ] {
            assert!(
                token.red == token.green && token.green == token.blue,
                "{name} is tinted: {token:?}"
            );
        }
    }

    /// Without a base rule on `label`, vizia paints text black and anything
    /// that forgot a class disappears into the background.
    #[test]
    fn labels_have_a_default_colour() {
        let css = stylesheet();
        let base = css
            .split_once("label {")
            .expect("no base rule for labels")
            .1;
        assert!(
            base.starts_with(&format!("\n    color: {}", FOREGROUND.css())),
            "the base label rule does not set a colour"
        );
    }

    #[test]
    fn the_stylesheet_mentions_every_class_it_documents() {
        let css = stylesheet();
        for class in [
            ".root",
            ".panel",
            ".panel-highlight",
            ".section",
            ".row",
            ".divider",
            ".label",
            ".value",
            ".subtle",
            ".disabled",
            ".icon",
            ".segmented",
            ".segment",
            ".track",
            ".accent",
            ".decoration",
            ".hoverable",
        ] {
            assert!(
                css.contains(class),
                "{class} is missing from the stylesheet"
            );
        }
    }
}
