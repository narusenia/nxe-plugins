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

/// One accent, and no other hue anywhere **inside a window**.
///
/// **The hue is the only thing that changes between plugins.** Every stop is
/// built at the same OKLCH lightness and chroma and differs only in hue, so a
/// bar at half fill has the same weight in all five windows. Told apart at a
/// glance, one product群 when opened side by side — the same reason the
/// windows share a width (`.agents/rules/ui.md`).
///
/// **Reached at draw time through the tree, not through a global.** The
/// palette is a vizia `Model` built by [`install`], and `DrawContext`
/// implements `DataContext`, so a custom-drawn widget calls [`palette`] and
/// gets whichever palette is nearest above it. That is what lets
/// `examples/gallery` put all five side by side.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Palette {
    /// The accent itself. A state that is simply on is this, flat.
    pub accent: Token,
    /// The light end of the ramp, for telling things of the same kind apart.
    pub bright: Token,
    /// The dark end of the ramp. Used with [`Palette::bright`] to tell groups of
    /// the same kind of thing apart — four voice pairs, say — **without adding
    /// a second hue**, which is the one thing this palette does not allow.
    pub deep: Token,
    /// The accent at 18 % — a wash behind something, not a colour of its own.
    pub dim: Token,

    // The surface roles. Identical in all five palettes — and **not** identical
    // on an inverted surface, which is the whole reason they live here rather
    // than staying constants a widget could reach for directly
    // ([`Palette::inverted`]).
    /// What the surface under this widget is painted.
    pub ground: Token,
    /// The unfilled part of a control, and anything there for a moment.
    pub track: Token,
    /// Hairlines, grids, axis marks — the structural device of this design.
    pub line: Token,
    /// Text, and a mark that is text-like.
    pub ink: Token,
    /// Text that names something rather than saying it.
    pub muted: Token,
    /// Gridlines, units, inert rows.
    pub subtle: Token,
}

impl Palette {
    const fn ramp(bright: Token, accent: Token, deep: Token) -> Self {
        Self {
            accent,
            bright,
            deep,
            dim: Token::rgba(accent.red, accent.green, accent.blue, 0.18),
            ground: BACKGROUND,
            track: ELEVATED,
            line: BORDER,
            ink: FOREGROUND,
            muted: MUTED,
            subtle: SUBTLE,
        }
    }

    /// The same palette for the one panel in a window whose ground **is** the
    /// accent (`.agents/rules/ui.md`).
    ///
    /// **Inversion is a palette, not a second concept.** A widget already asks
    /// the tree what colours are in force; the inverted surface builds this as
    /// a nested model and everything under it comes out right without knowing
    /// the surface exists.
    ///
    /// The hue roles collapse to ink, because **on a coloured ground the accent
    /// has nowhere to go** — the ground is already the accent. What is left to
    /// say with is darkness, so a mark is near-black and the ramp that tells
    /// kinds apart runs in alpha rather than in lightness.
    ///
    /// **One way.** Inverting an inverted palette does not give the first one
    /// back, and a window that wanted that has two subjects rather than one.
    pub fn inverted(self) -> Self {
        let ink = BACKGROUND;
        Self {
            accent: ink,
            bright: ink,
            // Groups are still told apart along a ramp; here it runs in alpha
            // rather than in lightness.
            deep: ink.at(0.55),
            dim: ink.at(0.18),
            ground: self.accent,
            track: ink.at(0.22),
            line: ink.at(0.35),
            ink,
            muted: ink.at(0.72),
            subtle: ink.at(0.5),
        }
    }

    /// Jade. Hue 158.
    pub const DOUBLER: Self = Self::ramp(
        Token::rgb(0x8C, 0xDB, 0xAD),
        Token::rgb(0x53, 0xC9, 0x8D),
        Token::rgb(0x00, 0x77, 0x49),
    );

    /// Violet. Hue 300.
    pub const VELOUR: Self = Self::ramp(
        Token::rgb(0xD0, 0xB8, 0xFF),
        Token::rgb(0xBD, 0x9A, 0xFA),
        Token::rgb(0x6E, 0x51, 0x9C),
    );

    /// Coral. Hue 35.
    ///
    /// **It was 50, and 50 made mud.** At the `deep` stop's lightness every
    /// warm hue lands in the brown band, and 50 landed square in it — the one
    /// ramp of the five that read as dirt rather than as a colour. Lifting that
    /// stop does not help (it goes from brown to tan); only the hue does.
    /// 35 puts `deep` at brick red and leaves the accent a coral, still 50°
    /// clear of Parallax's rose.
    pub const SPARKLEUR: Self = Self::ramp(
        Token::rgb(0xFF, 0xB0, 0x9C),
        Token::rgb(0xFA, 0x8C, 0x71),
        Token::rgb(0x9B, 0x46, 0x31),
    );

    /// Sky. Hue 232.7 — **the accent every plugin shipped with**, kept for the
    /// one whose subject is air. The other three stops moved a little: the old
    /// ramp's hue wandered from 230 to 243 across its four stops, and the
    /// family test below needs one hue per palette.
    pub const AIR: Self = Self::ramp(
        Token::rgb(0x7F, 0xD2, 0xFE),
        Token::rgb(0x38, 0xBD, 0xF8),
        Token::rgb(0x00, 0x6C, 0x94),
    );

    /// Rose. Hue 345.
    pub const PARALLAX: Self = Self::ramp(
        Token::rgb(0xF7, 0xAC, 0xD6),
        Token::rgb(0xED, 0x89, 0xC3),
        Token::rgb(0x91, 0x44, 0x73),
    );

    /// Every palette with the name of the plugin that wears it. For the gallery
    /// and for the test that keeps them one family.
    pub const ALL: [(&'static str, Self); 5] = [
        ("Doubler", Self::DOUBLER),
        ("Velour", Self::VELOUR),
        ("Sparkleur", Self::SPARKLEUR),
        ("Air", Self::AIR),
        ("Parallax", Self::PARALLAX),
    ];
}

/// The palette is model data, so `DrawContext` can reach it.
impl Model for Palette {}

/// The palette in force where this view sits.
///
/// Takes any `DataContext`, so the same call works while building the tree
/// (`Context`), while handling an event (`EventContext`) and while drawing
/// (`DrawContext`) — the lookup walks up the tree in all three.
///
/// **Falls back to [`Palette::AIR`] rather than to something obviously wrong.**
/// A widget can only miss the model by being built outside the subtree
/// [`install`] was called in — and in that case there is no stylesheet either,
/// so the window is already visibly broken. A debug colour here would only add
/// noise to a failure that is impossible to miss.
pub fn palette(cx: &impl DataContext) -> Palette {
    cx.data::<Palette>().copied().unwrap_or(Palette::AIR)
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

/// **The one width every plugin window is.**
///
/// Opened side by side the plugins are one product; different widths make them
/// look like several (`.agents/rules/ui.md`). It lives here because five copies
/// of the same number are identical by accident rather than on purpose — the
/// same reason the header is one function.
///
/// **It was 720**, set when there were three windows and no coloured surface in
/// any of them. The inverted panel wants room beside it, and a row of marks
/// with words under them wants more than a row of knobs did (`UI-20`).
///
/// Heights are not here: they differ because the amount inside each window
/// differs, and each is the sum of its own parts.
pub const WINDOW_WIDTH: u32 = 880;

const _: () = assert!(WINDOW_WIDTH.is_multiple_of(4), "the window is off the grid");

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
///
/// **Thirteen, not fifteen.** Fifteen was a headline: three of them across the
/// top of a window read as the loudest thing on screen, over the figure that is
/// what the plugin *is*. It only has to be larger than [`FONT_VALUE`], and set
/// in mono at full contrast it already wins without the extra size (looked at
/// in a host, `SPK-19`).
pub const FONT_READOUT: f32 = 13.0;

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
///
/// **It was 17, set bold.** At that size one weight of a grotesque reads as
/// another label with the volume turned up, so the wordmark had to shout to be
/// a wordmark. Set in the light face it does not have to: **size carries it,
/// the way size carries everything else here** (`UI-19`).
///
/// **26 was too much.** It read as a title page rather than as the name of the
/// window you are already looking at — the wordmark does not need to make a
/// case for itself. 20 is where it stops asserting and still is not a label.
pub const FONT_TITLE: f32 = 20.0;

/// The height a one-line label occupies at each size.
///
/// **A window's height is the sum of its parts, and an `Auto` label is not a
/// part anyone can add up** — vizia takes its height from the font's own
/// metrics, which the caller does not have. Every line a window's height
/// depends on is given one of these instead, so the total is arithmetic rather
/// than a number found by opening the plugin and looking (which is how it was
/// found five times in one afternoon, `SPK-19`).
///
/// Each is its font size rounded up onto the four-pixel grid, with room for
/// descenders.
pub const LINE_EYEBROW: f32 = 12.0;
pub const LINE_VALUE: f32 = 14.0;
/// The readout's line box. **Fixed for the same reason its width is**
/// (`nxe_ui::readout`): a box that measures its own contents makes every
/// change to the figure a relayout of the whole window, and this figure changes
/// with the audio (`docs/investigations/ui-frame-cost.md`).
pub const LINE_READOUT: f32 = 16.0;
pub const LINE_LABEL: f32 = 16.0;
pub const LINE_TITLE: f32 = 26.0;

const _: () = assert!(LINE_EYEBROW > FONT_EYEBROW, "the eyebrow will clip");
const _: () = assert!(LINE_VALUE > FONT_VALUE, "the value will clip");
const _: () = assert!(LINE_READOUT > FONT_READOUT, "the readout will clip");
const _: () = assert!(LINE_LABEL > FONT_LABEL, "the label will clip");
const _: () = assert!(LINE_TITLE > FONT_TITLE, "the wordmark will clip");

/// The rule, as a height. **One kind, one pixel** — the 2 px accent bar under
/// the wordmark was the only other one, and it went with the wordmark's
/// redesign (`UI-19`).
pub const RULE: f32 = 1.0;

/// How tall one segment of a segmented control is. In the stylesheet below and
/// here, because a window that holds one has to add it up.
pub const SEGMENT: f32 = 18.0;

/// Hover and selection only. Never a value: a knob that lags the mouse feels
/// broken.
pub const TRANSITION_MS: u32 = 150;

/// The stylesheet, built from the constants above.
pub fn stylesheet(palette: Palette) -> String {
    let background = BACKGROUND.css();
    let elevated = ELEVATED.css();
    let border = BORDER.css();
    let foreground = FOREGROUND.css();
    let muted = MUTED.css();
    let subtle = SUBTLE.css();
    let accent = palette.accent.css();
    let accent_dim = palette.dim.css();
    let inverted = palette.inverted();
    let ink = inverted.ink.css();
    let ink_muted = inverted.muted.css();
    let inverted_ground = inverted.ground.css();

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

/* The one panel whose ground is the accent (`nxe_ui::surface`). No border: a
   coloured field is already separated from the window, and a hairline on it
   reads as a second idea. */
.inverted {{
    background-color: {inverted_ground};
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

/* The plugin's name. Set apart by size alone, like everything else here — the
   light face it is set in is the modifier's job (`font::title`). */
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

/* A filled part of a control. **Flat.** The length of the fill is the quantity;
   a ramp along it says the same thing a second time, and made four bars at four
   values end in the same pale colour. It also does not need a direction, so
   there is one class here rather than one per axis. */
.accent {{
    background-color: {accent};
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
    height: {SEGMENT}px;
    border-radius: {RADIUS_CONTROL}px;
    transition: background-color {TRANSITION_MS}ms, color {TRANSITION_MS}ms;
}}

.segment:hover {{
    color: {foreground};
}}

/* **Flat, not the gradient fill.** A segment is a state, not a quantity: the
   pale end would have nothing to mean, and a label sitting on a ramp changes
   contrast across its own width — the word is harder to read at one end than at
   the other. The gradient belongs to things that measure something. */
.segment:checked {{
    color: {background};
    background-color: {accent};
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
/* Text on the inverted ground (`nxe_ui::surface`). **Last in the file on
   purpose.** The stylesheet has no way to say: labels inside .inverted — the
   generated CSS is flat — so a label there says so itself, and these are
   single-class selectors like `.eyebrow` and `.label`. Ties go to whichever
   came last, so these have to. Put them earlier and `.eyebrow` wins, which is
   how the first version painted a grey eyebrow onto the accent. */
.ink {{
    color: {ink};
}}

.ink-muted {{
    color: {ink_muted};
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

/// The fonts, the icons, the palette and the stylesheet. Call once, at the top
/// of the window, before any view is built.
///
/// It also builds the [`hint`](crate::hint) model, for the same reason the
/// palette is a model: `header` shows it and any control anywhere under the
/// window writes to it, so it has to sit above both.
///
/// **The palette goes in twice, and it has to.** The stylesheet is generated
/// from it (vizia has no way to remove or replace a stylesheet once added, so
/// there is one per window), and the same palette is built as a `Model` so that
/// custom-drawn widgets can read it at draw time. The gallery leans on the
/// second half: a nested `Palette` model re-colours everything drawn under it,
/// which is how five palettes are seen at once even though the stylesheet can
/// only hold one.
pub fn install(cx: &mut Context, palette: Palette) {
    font::install(cx);
    icon::install(cx);
    palette.build(cx);
    crate::hint::Hint::default().build(cx);
    cx.add_stylesheet(CSS::String(stylesheet(palette)))
        .expect("the generated stylesheet is built from constants and cannot fail to parse");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The generated stylesheet must not contain a colour that did not come
    /// from a token — that is the whole point of generating it.
    #[test]
    fn the_stylesheet_has_no_colour_literals() {
        for (name, palette) in Palette::ALL {
            let css = stylesheet(palette);
            assert!(
                !css.contains('#'),
                "a hex colour reached {name}'s stylesheet"
            );
        }
    }

    /// Every token has to survive the round trip into CSS, or a typo in `css`
    /// would silently produce a transparent or black surface.
    #[test]
    fn tokens_render_as_rgba() {
        assert_eq!(Palette::AIR.accent.css(), "rgba(56, 189, 248, 1)");
        assert_eq!(Palette::AIR.dim.css(), "rgba(56, 189, 248, 0.18)");
        assert_eq!(BACKGROUND.css(), "rgba(10, 10, 10, 1)");
    }

    /// sRGB → OKLCh. The palettes were generated in this space; this is what
    /// keeps a hand-edited hex from quietly leaving it.
    fn oklch(token: Token) -> (f32, f32, f32) {
        fn linear(channel: u8) -> f32 {
            let c = f32::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let (r, g, b) = (linear(token.red), linear(token.green), linear(token.blue));
        let l = (0.412_221_5 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        let lightness = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
        let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
        let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
        (
            lightness,
            a.hypot(b),
            b.atan2(a).to_degrees().rem_euclid(360.0),
        )
    }

    /// One stop of the ramp: what it is called, and how to get it out of a
    /// palette.
    type Stop = (&'static str, fn(Palette) -> Token);

    const STOPS: [Stop; 3] = [
        ("bright", |p| p.bright),
        ("accent", |p| p.accent),
        ("deep", |p| p.deep),
    ];

    /// **The five palettes differ in hue and in nothing else.** Same lightness,
    /// same chroma, stop for stop — that is what lets a bar at half fill carry
    /// the same weight in all five windows, and it is the whole reason five
    /// accents do not read as five different designs.
    ///
    /// `deep` gets a looser bound on chroma: at that lightness the blue and the
    /// green run out of sRGB before the others do, so they are clipped into
    /// gamut rather than being made lighter.
    #[test]
    fn the_palettes_are_one_family() {
        for (stop, of) in STOPS {
            let mut lightness: Vec<f32> = Vec::new();
            let mut chroma: Vec<f32> = Vec::new();
            for (_, palette) in Palette::ALL {
                let (l, c, _) = oklch(of(palette));
                lightness.push(l);
                chroma.push(c);
            }
            let spread = |values: &[f32]| {
                values.iter().copied().fold(f32::MIN, f32::max)
                    - values.iter().copied().fold(f32::MAX, f32::min)
            };
            assert!(
                spread(&lightness) <= 0.01,
                "{stop} is not one lightness across the palettes: {lightness:?}"
            );
            let allowed = if stop == "deep" { 0.02 } else { 0.005 };
            assert!(
                spread(&chroma) <= allowed,
                "{stop} is not one chroma across the palettes: {chroma:?}"
            );
        }
    }

    /// And they have to be far enough apart to be the thing that tells the
    /// windows apart at a glance.
    #[test]
    fn the_palettes_are_told_apart_by_hue() {
        let hues: Vec<(&str, f32)> = Palette::ALL
            .into_iter()
            .map(|(name, palette)| (name, oklch(palette.accent).2))
            .collect();
        for (index, (name, hue)) in hues.iter().enumerate() {
            for (other, other_hue) in &hues[index + 1..] {
                let apart = (hue - other_hue).abs().min(360.0 - (hue - other_hue).abs());
                assert!(
                    apart >= 40.0,
                    "{name} and {other} are {apart:.0}° apart — too close to tell"
                );
            }
        }
    }

    /// A palette's ramp has to run in one direction, or two things told apart
    /// along it swap places in one window and not in another.
    #[test]
    fn every_ramp_runs_from_deep_to_bright() {
        for (name, palette) in Palette::ALL {
            let steps = [palette.deep, palette.accent, palette.bright];
            for pair in steps.windows(2) {
                assert!(
                    oklch(pair[0]).0 < oklch(pair[1]).0,
                    "{name}'s ramp does not get lighter at every step"
                );
            }
        }
    }

    /// A colour composited over what is behind it, then WCAG relative
    /// luminance. The inverted roles carry alpha, so the ground is part of the
    /// answer.
    ///
    /// **The blend happens before the gamma, not after.** femtovg composites
    /// encoded pixels, so black at 72 % over a light ground is far darker than
    /// mixing the two luminances would suggest — 5.6:1 rather than 2.8:1. Doing
    /// it the other way round failed this test on a surface that is perfectly
    /// readable.
    fn contrast(over: Token, ground: Token) -> f32 {
        fn channel(value: f32) -> f32 {
            let c = value / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let luminance = |token: Token| {
            let blend = |top: u8, bottom: u8| {
                channel(f32::from(top) * token.alpha + f32::from(bottom) * (1.0 - token.alpha))
            };
            0.2126 * blend(token.red, ground.red)
                + 0.7152 * blend(token.green, ground.green)
                + 0.0722 * blend(token.blue, ground.blue)
        };
        let (over, ground) = (luminance(over), luminance(ground.at(1.0)));
        let (light, dark) = if over > ground {
            (over, ground)
        } else {
            (ground, over)
        };
        (light + 0.05) / (dark + 0.05)
    }

    /// **Text has to survive the surface it is on** — including the one whose
    /// ground is the accent, where the neutrals are gone and what is left is
    /// black at three opacities.
    ///
    /// `ink` clears AAA (7:1) and `muted` clears AA (4.5:1) on all five
    /// palettes, inverted or not. **`subtle` is deliberately below the 3:1
    /// line for text**: it is gridlines, units and inert rows — structure
    /// rather than something to read — and it measures 3.0 inverted against
    /// 4.2 on black. If it ever has to say something, it is the wrong role.
    #[test]
    fn text_survives_both_grounds() {
        for (name, palette) in Palette::ALL {
            for (surface, palette) in [("plain", palette), ("inverted", palette.inverted())] {
                for (role, colour, least) in [
                    ("ink", palette.ink, 7.0),
                    ("muted", palette.muted, 4.5),
                    ("subtle", palette.subtle, 2.9),
                ] {
                    let ratio = contrast(colour, palette.ground);
                    assert!(
                        ratio >= least,
                        "{name} {surface}: {role} is {ratio:.1}:1 against its ground, under {least}"
                    );
                }
            }
        }
    }

    /// And the inverted surface has to actually be the accent, not a second
    /// dark panel — the whole point is that a glance lands on it.
    #[test]
    fn the_inverted_surface_is_the_accent() {
        for (name, palette) in Palette::ALL {
            let inverted = palette.inverted();
            assert_eq!(inverted.ground, palette.accent, "{name} did not invert");
            assert!(
                contrast(inverted.ground, palette.ground) >= 3.0,
                "{name}'s inverted panel does not stand out from the window"
            );
        }
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
        let css = stylesheet(Palette::AIR);
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
        let css = stylesheet(Palette::AIR);
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
            ".inverted",
            ".ink",
            ".ink-muted",
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
        let css = stylesheet(Palette::AIR);
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
