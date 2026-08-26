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
    /// A colour `t` of the way from this one to `other`, `t` in `0..=1`.
    ///
    /// Lets a set of related things be told apart along the accent instead of
    /// by adding hues the palette does not have.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self {
            red: lerp(self.red, other.red),
            green: lerp(self.green, other.green),
            blue: lerp(self.blue, other.blue),
            alpha: self.alpha + (other.alpha - self.alpha) * t,
        }
    }

    pub fn at(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }
}

// Surfaces. Fully neutral — every channel equal — so the accent is the only
// hue anywhere in the window. A slight blue cast in the greys reads as a
// colour scheme rather than as a background.
//
// **Two levels, not three.** Panels and controls sit at `BACKGROUND` with the
// rest of the window; what separates them is the one-pixel border, nothing
// else. `ELEVATED` is left for things that are only there for a moment — a
// hovered row, a bar's track, the field being typed into. A lighter resting
// surface (and the one-pixel top highlight that went with it) made a flat
// window look like a stack of cards.
pub const BACKGROUND: Token = Token::rgb(0x0A, 0x0A, 0x0A);
pub const ELEVATED: Token = Token::rgb(0x1F, 0x1F, 0x1F);
pub const BORDER: Token = Token::rgb(0x2A, 0x2A, 0x2A);

// Text. Neutral for the same reason as the surfaces.
pub const FOREGROUND: Token = Token::rgb(0xFA, 0xFA, 0xFA);
pub const MUTED: Token = Token::rgb(0xA3, 0xA3, 0xA3);
pub const SUBTLE: Token = Token::rgb(0x73, 0x73, 0x73);

// One accent, and no other hue anywhere.
pub const ACCENT: Token = Token::rgb(0x38, 0xBD, 0xF8);
pub const ACCENT_BRIGHT: Token = Token::rgb(0x7D, 0xD3, 0xFC);
/// The dark end of the accent ramp. Used with [`ACCENT_BRIGHT`] to tell groups
/// of the same kind of thing apart — four voice pairs, say — **without adding a
/// second hue**, which is the one thing this palette does not allow.
pub const ACCENT_DEEP: Token = Token::rgb(0x03, 0x69, 0xA1);
pub const ACCENT_DIM: Token = Token::rgba(0x38, 0xBD, 0xF8, 0.18);
/// The pale end of the accent — blue so light it reads as white.
///
/// **The far stop of every accent gradient.** Still the same hue, so the
/// "one accent and no other" rule holds: what changes along a filled bar is
/// lightness, not colour.
pub const ACCENT_WASH: Token = Token::rgb(0xE0, 0xF2, 0xFE);

/// A two-stop linear gradient, for `background-image`.
///
/// **`background-image`, not `background-color`** — vizia parses
/// `linear-gradient` only there, and a rule that sets both draws the colour
/// under the gradient rather than instead of it. Only linear gradients exist in
/// this revision; there is no radial.
pub fn gradient(direction: &str, from: Token, to: Token) -> String {
    format!(
        "linear-gradient(to {direction}, {}, {})",
        from.css(),
        to.css()
    )
}

/// Corner radii. **Zero — the corners are square**, and they stay that way.
/// They were 2 and 3 px, on the theory that a hair of radius keeps a one-pixel
/// border from looking chipped; on screen the hard corner simply looked better,
/// and the shapes are the design. The Swiss direction the interface took later
/// only made the case stronger — a grid is drawn with straight lines.
///
/// They stay as named constants rather than being deleted: every rounded shape
/// reads them, so a change of mind is one line rather than a sweep.
pub const RADIUS_CONTROL: f32 = 0.0;
pub const RADIUS_CARD: f32 = 0.0;

// Guards, not tests: someone reaching for a comfortable radius later should be
// stopped by the compiler rather than by a test they might not run.
const _: () = assert!(RADIUS_CONTROL <= 2.0, "controls are getting round");
const _: () = assert!(RADIUS_CARD <= 2.0, "surfaces are getting round");

/// The spacing scale. Five steps on a four-pixel grid; nothing between them.
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;

/// The eyebrow: the smallest thing on screen, naming a region rather than a
/// control.
///
/// **A region's name is not a control's name.** Set small, in [`SUBTLE`], over
/// a hairline, it reads as structure — the grid saying what this part of the
/// window is — instead of joining the row of labels underneath it.
pub const FONT_EYEBROW: f32 = 9.0;

/// The one figure that is the answer rather than a setting.
///
/// A panel whose numbers are all the same size has no subject. This is for the
/// number a region exists to show — the gain reduction, the output level —
/// and there should be **at most one per region**.
pub const FONT_READOUT: f32 = 15.0;

/// Two text sizes. Labels name things, values say what they are.
///
/// The value is the *smaller* of the two on purpose: it is set in mono at full
/// contrast, which already makes it the thing the eye lands on. At the label's
/// size it read as a headline under every knob.
///
/// **Written without a unit.** This vizia revision parses `font-size` as a
/// keyword or a bare number and nothing else, so `10px` fails to parse and the
/// declaration is dropped — silently, leaving every label at the 16 px default.
/// The test below is what keeps a `px` from creeping back in.
pub const FONT_LABEL: f32 = 12.0;
pub const FONT_VALUE: f32 = 10.0;

/// The wordmark, and nothing else. A third size exists only because a plugin's
/// name is not a label — it names the window, not a control in it.
pub const FONT_TITLE: f32 = 17.0;

/// Hover and selection only. Never a value: a knob that lags the mouse feels
/// broken.
pub const TRANSITION_MS: u32 = 150;

/// The stylesheet, built from the constants above.
pub fn stylesheet() -> String {
    let background = BACKGROUND.css();
    let elevated = ELEVATED.css();
    let border = BORDER.css();
    let foreground = FOREGROUND.css();
    let muted = MUTED.css();
    let subtle = SUBTLE.css();
    let accent = ACCENT.css();
    let accent_dim = ACCENT_DIM.css();
    let accent_fill = gradient("right", ACCENT, ACCENT_WASH);
    let accent_fill_up = gradient("top", ACCENT, ACCENT_WASH);
    let rule_accent = gradient("right", ACCENT, ACCENT.at(0.0));

    format!(
        "
/* An element selector, not a class: vizia's default text colour is black, so
   every label without a class would come out unreadable on a dark surface.
   The classes below then only have to say what is *different*. */
label {{
    color: {foreground};
    font-size: {FONT_VALUE};
}}

.root {{
    background-color: {background};
    child-space: {SPACE_5}px;
    row-between: {SPACE_4}px;
}}

.panel {{
    background-color: {background};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CARD}px;
    child-space: {SPACE_4}px;
    row-between: {SPACE_3}px;
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
    font-size: {FONT_LABEL};
}}

.value {{
    color: {foreground};
    font-size: {FONT_VALUE};
}}

/* The plugin's name. Set apart by size alone, like everything else here. */
.title {{
    color: {foreground};
    font-size: {FONT_TITLE};
}}

.subtle {{
    color: {subtle};
    font-size: {FONT_LABEL};
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

/* A filled part of a control. **A gradient along the fill, not a flat block**:
   the pale end marks where the value got to, so a bar reads as a quantity with
   a direction rather than as a coloured rectangle. Still one hue — what moves
   is lightness. */
.accent {{
    background-image: {accent_fill};
    border-radius: {RADIUS_CONTROL}px;
}}

/* The same fill for something that grows upward rather than rightward. */
.accent-up {{
    background-image: {accent_fill_up};
    border-radius: {RADIUS_CONTROL}px;
}}

/* Rules. **The structural device of this design** — the grid made visible, in
   place of the boxes and shadows a rounder interface would use. A rule sits
   under the thing it belongs to and runs the full width of the column, so the
   eye reads columns before it reads controls. */
.rule {{
    height: 1px;
    width: 1s;
    background-color: {border};
}}

/* A rule that marks the subject of a region. Fades out along its length, so it
   reads as a start rather than as one side of a box. */
.rule-accent {{
    height: 2px;
    width: 1s;
    background-image: {rule_accent};
}}

/* A region's name, over its rule. `.eyebrow` is the text, `.heading` is the
   pair — put `.heading` on the container and `.eyebrow` on the label. */
.eyebrow {{
    color: {subtle};
    font-size: {FONT_EYEBROW};
}}

.heading {{
    layout-type: column;
    height: auto;
    width: 1s;
    row-between: {SPACE_1}px;
    border-bottom-width: 1px;
    border-bottom-color: {border};
    child-bottom: {SPACE_1}px;
}}

/* The one number a region exists to show. At most one per region: a panel where
   every figure is this size has no subject. */
.readout {{
    color: {foreground};
    font-size: {FONT_READOUT};
}}

/* A row of choices. The container holds the groove; the segments sit inside it
   so the selected one reads as raised out of the track. */
.segmented {{
    layout-type: row;
    background-color: {background};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CONTROL}px;
    child-space: 1px;
    col-between: 1px;
    height: auto;
    width: auto;
}}

.segment {{
    color: {muted};
    font-size: {FONT_LABEL};
    child-space: 1s;
    child-left: {SPACE_2}px;
    child-right: {SPACE_2}px;
    height: 18px;
    border-radius: {RADIUS_CONTROL}px;
    transition: background-color {TRANSITION_MS}ms, color {TRANSITION_MS}ms;
}}

.segment:hover {{
    color: {foreground};
}}

.segment:checked {{
    color: {background};
    background-image: {accent_fill};
}}

/* A number that can be typed into. It looks like any other value until the
   pointer is over it, which is the whole hint — a box drawn around every figure
   would turn a panel into a form. */
.editable:hover {{
    background-color: {elevated};
}}

/* The field that replaces it. Same size and family as the number, so nothing
   jumps when it appears. */
textbox {{
    background-color: {elevated};
    border-width: 1px;
    border-color: {accent};
    border-radius: {RADIUS_CONTROL}px;
    color: {foreground};
    caret-color: {accent};
    selection-color: {accent_dim};
    child-left: {SPACE_1}px;
    child-right: {SPACE_1}px;
}}

/* Keyboard focus. Only ever from a keyboard: vizia sets `:focus-visible` when
   focus arrived by `Tab` rather than by a click, so a pointer user never sees a
   ring (`plugins/doubler/docs/specifications/ui.md`). Outline rather than
   border, so nothing moves when it appears. */
:focus-visible {{
    outline-width: 2px;
    outline-color: {accent_dim};
    outline-offset: 1px;
}}

/* Hover help. vizia's `.tooltip(…)` modifier builds the view and toggles `.vis`
   on it after a delay; all that is left is what it looks like. An element
   selector because the view has no class of its own.

   The delay lives in the transition: 500 ms before it fades in, so a pointer
   crossing a row of knobs does not leave a trail of boxes. */
tooltip {{
    pointer-events: none;
    background-color: {elevated};
    border-width: 1px;
    border-color: {border};
    border-radius: {RADIUS_CONTROL}px;
    child-space: {SPACE_1}px;
    child-left: {SPACE_2}px;
    child-right: {SPACE_2}px;
    color: {foreground};
    font-size: {FONT_LABEL};
    opacity: 0;
    transition: opacity 100ms 500ms;
}}

tooltip.vis {{
    opacity: 1;
    transition: opacity 100ms 500ms;
}}

/* A hint on a control near the right edge hangs to the *left* of it instead of
   the right, or it runs off the window. Put the class on the container: it
   matches every tooltip inside, so the decision is made once per region rather
   than once per control. */
.hint-left tooltip {{
    left: 1s;
    right: 0px;
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
/// The content of a hover hint.
///
/// **The label is `.decoration`.** vizia's tooltip is not hit-testable but its
/// children are, and the tooltip hangs *below* its anchor — right over whatever
/// sits under the control. An invisible label there swallows clicks meant for
/// the thing it is describing (`.agents/rules/vizia.md`).
pub fn hint(cx: &mut Context, text: &'static str) {
    Label::new(cx, text).class("decoration");
}

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
            ("elevated", ELEVATED),
            ("border", BORDER),
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
            ".section",
            ".row",
            ".divider",
            ".label",
            ".value",
            ".title",
            "tooltip",
            ":focus-visible",
            ".editable",
            "textbox",
            ".hint-left",
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

    /// `font-size` takes a keyword or a bare number in this vizia revision, and
    /// a value it cannot parse is dropped without a word — every label then
    /// renders at the 16 px default. That looked like "the design is just big"
    /// for as long as it took to notice the sizes never moved.
    #[test]
    fn font_sizes_carry_no_unit() {
        let css = stylesheet();
        for declaration in css.match_indices("font-size:") {
            let rest = &css[declaration.0 + "font-size:".len()..];
            let value = rest.split(';').next().unwrap_or("").trim();
            assert!(
                value.parse::<f32>().is_ok(),
                "`font-size: {value}` will not parse — write the number alone"
            );
        }
    }
}
