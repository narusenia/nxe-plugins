//! The design tokens: colour, spacing, type sizes, the window width.
//!
//! **The palettes came from the v0 vizia `nxe-ui`, value for value**, and the
//! tests that keep them one family came with them.
//!
//! **A widget takes its palette as an argument.** vizia found it by walking up
//! the tree; egui has no tree, and an argument is the honest version of the
//! same thing — the inverted surface hands its children
//! [`Palette::inverted`], and nothing else has to know.

use egui::Color32;

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

    pub fn color(self) -> Color32 {
        Color32::from_rgba_unmultiplied(
            self.red,
            self.green,
            self.blue,
            (self.alpha * 255.0).round() as u8,
        )
    }

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

    /// The same colour at a different opacity.
    pub fn at(self, alpha: f32) -> Self {
        Self { alpha, ..self }
    }
}

impl From<Token> for Color32 {
    fn from(token: Token) -> Self {
        token.color()
    }
}

// Surfaces and text. Fully neutral — every channel equal — so the accent is
// the only hue anywhere in the window.
pub const BACKGROUND: Token = Token::rgb(0x0A, 0x0A, 0x0A);
pub const ELEVATED: Token = Token::rgb(0x1F, 0x1F, 0x1F);
pub const BORDER: Token = Token::rgb(0x2A, 0x2A, 0x2A);
pub const FOREGROUND: Token = Token::rgb(0xFA, 0xFA, 0xFA);
pub const MUTED: Token = Token::rgb(0xA3, 0xA3, 0xA3);
pub const SUBTLE: Token = Token::rgb(0x73, 0x73, 0x73);

/// **The one second colour**, for a reading past where it should be: over
/// 0 VU, a true peak over full scale, a loudness over its target. Only the
/// Peek meters use it — the exception `design-system.md` records (PEEK-2).
pub const WARN: Token = Token::rgb(0xFF, 0x7A, 0x45);

/// One accent, and no other hue anywhere **inside a window**.
///
/// **The hue is the only thing that changes between plugins.** Every stop is
/// built at the same OKLCH lightness and chroma and differs only in hue, so a
/// bar at half fill has the same weight in all five windows. Told apart at a
/// glance, one product when opened side by side — the same reason the
/// windows share a width (`.agents/rules/ui.md`).
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
    /// clear of Diorama's rose.
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

    /// Moss. Hue 118.
    ///
    /// **The hue was chosen by what was left.** Every accent has to sit 40°
    /// from every other, and the five already placed — 35, 158, 232.7, 300,
    /// 345 — leave exactly one gap wide enough: the 123° between Sparkleur and
    /// Doubler. That admits 75 to 118, and **118 is the end of it that gets
    /// out of the brown band** at the `deep` stop, which is the trap
    /// `SPARKLEUR` above records. At 96 the deep stop is olive-brown; at 118 it
    /// is a dark moss, and its chroma lands on the family's without clipping.
    pub const PUMICE: Self = Self::ramp(
        Token::rgb(0xC0, 0xD0, 0x83),
        Token::rgb(0xA7, 0xBA, 0x4B),
        Token::rgb(0x5E, 0x6C, 0x00),
    );

    /// No hue: the Peek meters (`peek-plan.md`, decision 4). A meter sits
    /// beside other plugins and must not compete with their colour.
    ///
    /// **Not in [`Palette::ALL`]**, which is the Basic family the hue tests
    /// hold to one lightness and 40° apart; an achromatic ramp has no hue to
    /// test.
    pub const PEEK: Self = Self::ramp(
        Token::rgb(0xFA, 0xFA, 0xFA),
        Token::rgb(0xE5, 0xE5, 0xE5),
        Token::rgb(0x73, 0x73, 0x73),
    );

    /// Acid green: the Kits cut from the Design series (`nonlinear-kit-plan.md`,
    /// decision 3, the canvas's Kit 7). **Not in [`Palette::ALL`]**: it is
    /// Design's colour, not one of the Basic family's.
    pub const DESIGN_KIT: Self = Self::ramp(
        Token::rgb(0xE9, 0xFF, 0x8F),
        Token::rgb(0xD7, 0xFF, 0x3A),
        Token::rgb(0x6B, 0x80, 0x10),
    );

    /// Every palette with the name of the plugin that wears it. For the gallery
    /// and for the test that keeps them one family.
    pub const ALL: [(&'static str, Self); 6] = [
        ("Doubler", Self::DOUBLER),
        ("Velour", Self::VELOUR),
        ("Sparkleur", Self::SPARKLEUR),
        ("Air", Self::AIR),
        ("Diorama", Self::PARALLAX),
        ("Pumice", Self::PUMICE),
    ];
}

/// The spacing scale. Five steps on a four-pixel grid; nothing between them.
pub const SPACE_1: f32 = 4.0;
pub const SPACE_2: f32 = 8.0;
pub const SPACE_3: f32 = 12.0;
pub const SPACE_4: f32 = 16.0;
pub const SPACE_5: f32 = 24.0;

/// Type sizes. The v0 scale, with one size added for the sidebar's subject.
///
/// - eyebrow: a region's name, tracked capitals over a rule
/// - value: a control's figure, in mono at full contrast — smaller than the
///   label on purpose, because mono at full contrast already wins
/// - label: a control's name
/// - readout: the one figure per region that is the answer
/// - title: the wordmark
/// - subject: **the largest figure in the window, and there is one** — the
///   sidebar's number (`design-system.md`, H4)
pub const FONT_EYEBROW: f32 = 9.0;
pub const FONT_VALUE: f32 = 10.0;
pub const FONT_LABEL: f32 = 12.0;
pub const FONT_READOUT: f32 = 13.0;
pub const FONT_TITLE: f32 = 18.0;
pub const FONT_SUBJECT: f32 = 40.0;

/// The height a one-line text occupies at each size. **A window's height is
/// the sum of its parts**, and a line whose height came from font metrics is
/// not a part anyone can add up (`.agents/rules/ui.md`).
pub const LINE_EYEBROW: f32 = 12.0;
pub const LINE_VALUE: f32 = 14.0;
pub const LINE_LABEL: f32 = 16.0;
pub const LINE_READOUT: f32 = 16.0;
pub const LINE_TITLE: f32 = 24.0;
pub const LINE_SUBJECT: f32 = 48.0;

const _: () = assert!(LINE_EYEBROW > FONT_EYEBROW, "the eyebrow will clip");
const _: () = assert!(LINE_VALUE > FONT_VALUE, "the value will clip");
const _: () = assert!(LINE_LABEL > FONT_LABEL, "the label will clip");
const _: () = assert!(LINE_READOUT > FONT_READOUT, "the readout will clip");
const _: () = assert!(LINE_TITLE > FONT_TITLE, "the wordmark will clip");
const _: () = assert!(LINE_SUBJECT > FONT_SUBJECT, "the subject will clip");

/// Tracking for the tracked capitals (eyebrows and the `NXE` mark), in points
/// per glyph. **New in v2** — vizia had no letter-spacing (`design-system.md`).
pub const TRACKING: f32 = 1.2;

/// The rule. One kind, one pixel.
pub const RULE: f32 = 1.0;

/// **The one width every plugin window is** (`.agents/rules/ui.md`). Heights
/// differ, and each is the sum of its own parts.
pub const WINDOW_WIDTH: u32 = 880;

const _: () = assert!(WINDOW_WIDTH.is_multiple_of(4), "the window is off the grid");

/// The sidebar's width in the H4 layout. A quarter of the window, on the grid.
pub const SIDEBAR_WIDTH: f32 = 220.0;

#[cfg(test)]
mod tests {
    use super::*;

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

    /// **The palettes differ in hue and in nothing else.** Same lightness,
    /// same chroma, stop for stop — that is what lets a bar at half fill carry
    /// the same weight in every window, and it is the whole reason six accents
    /// do not read as six different designs.
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
    /// **The blend happens before the gamma, not after.** egui composites
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
}
