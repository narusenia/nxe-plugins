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

// Surfaces.
pub const BACKGROUND: Token = Token::rgb(0x09, 0x09, 0x0B);
pub const CARD: Token = Token::rgb(0x13, 0x13, 0x16);
pub const ELEVATED: Token = Token::rgb(0x1B, 0x1B, 0x1F);
pub const BORDER: Token = Token::rgb(0x27, 0x27, 0x2A);
/// The one-pixel lift along the top of a surface. Most of the "shadcn feel" is
/// this line.
pub const HIGHLIGHT: Token = Token::rgba(0xFF, 0xFF, 0xFF, 0.04);

// Text.
pub const FOREGROUND: Token = Token::rgb(0xFA, 0xFA, 0xFA);
pub const MUTED: Token = Token::rgb(0xA1, 0xA1, 0xAA);
pub const SUBTLE: Token = Token::rgb(0x71, 0x71, 0x7A);

// One accent, and no other hue anywhere.
pub const ACCENT: Token = Token::rgb(0x38, 0xBD, 0xF8);
pub const ACCENT_BRIGHT: Token = Token::rgb(0x7D, 0xD3, 0xFC);
pub const ACCENT_DIM: Token = Token::rgba(0x38, 0xBD, 0xF8, 0.18);

/// Corner radii. Three steps and no more: controls, surfaces, and round.
pub const RADIUS_CONTROL: f32 = 6.0;
pub const RADIUS_CARD: f32 = 10.0;

/// The spacing scale. Five steps on a four-pixel grid; nothing between them.
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;

/// Two text sizes. Labels name things, values say what they are.
pub const FONT_LABEL: f32 = 12.0;
pub const FONT_VALUE: f32 = 13.0;

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

.hoverable {{
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

/// Installs the stylesheet. Call once when the window is built.
pub fn install(cx: &mut Context) {
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
        assert_eq!(BACKGROUND.css(), "rgba(9, 9, 11, 1)");
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
            ".track",
            ".accent",
            ".hoverable",
        ] {
            assert!(
                css.contains(class),
                "{class} is missing from the stylesheet"
            );
        }
    }
}
