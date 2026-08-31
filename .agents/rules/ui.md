---
paths:
  - "crates/nxe-ui/**"
  - "plugins/**/src/ui/**"
  - "plugins/**/docs/specifications/ui.md"
---

# Interface rules

**What the windows have to look like and how they have to read.** How the
toolkit behaves — what parses, what silently does nothing — is
[`vizia.md`](vizia.md). How to build with the widgets is
[`crates/nxe-ui/README.md`](../../crates/nxe-ui/README.md).

Every rule here was paid for. Where one has a scar, the scar is named.

## The window

- **A window's height is arithmetic, never a number found by looking.** Add up
  the parts: `theme::SPACE_*`, `header::HEIGHT`, `readout::HEIGHT`, the figure,
  `knob_block_height(size)`, the table. Text lines have `theme::LINE_*` for
  exactly this reason — an `Auto` label takes its height from font metrics the
  caller does not have, and a window sized around one runs off the bottom the
  next time a row is added. It ran off five times in one afternoon before this
  rule (`SPK-19`).
- **The five plugins share one width, and it is one constant**
  (`theme::WINDOW_WIDTH`). Opened side by side they are one product; different
  widths make them look like five. **Five copies of the same number are
  identical by accident**, which is how they drift — the same reason the header
  is one function. Heights differ, because the amount inside them differs, and
  each is the sum of its own parts.
- **A stretched row under a fixed one loses the class's `row-between` too.**
  `.root` sets `row-between: SPACE_4`, and a window whose root holds the content
  row plus a status bar gives that 16 px away without anything on screen
  changing colour — the gap lands where the window is black either way. What it
  does is take 16 px out of the row, and then the row's *fixed* children draw at
  full size and overflow while anything `Stretch(1.0)` beside them comes out
  short. Sparkleur's meter strip stopped 16 px above the table next to it and
  read as a meter that did not reach the bottom. **Set `row_between(0)` on the
  root explicitly**, next to the `child_space(0)` that is already there for the
  same kind of reason.
- **Never ask the host to resize the editor.** A disclosure that resized the
  window wedged Ableton (`DBL-*`). A control that has to become reachable does
  so inside a fixed window.
- **Fixed sizes come from the parts, so a figure's height is one constant.**
  `field::HEIGHT` is the same number in every plugin: two figures tuned
  separately are each right on their own and wrong beside each other.

## Naming a region

- **A region's name is an eyebrow over a rule** (`.eyebrow` inside `.heading`),
  not a label. A region's name is not a control's name — set at the label size
  it joins the row of controls under it instead of reading as structure.
- **At most one `.readout` per region.** A panel where every figure is the
  headline size has no subject.
- **Rules are the structural device.** This design has no shadows and no
  rounding, so the grid is drawn with lines. Per-side borders
  (`border-bottom-width`) are how.

## Colour

- **One accent, and no second hue anywhere — inside a window.** Things of the
  same kind are told apart along the accent's own ramp (`palette.deep` →
  `palette.bright`), never by introducing a colour.
- **Between windows, the hue is the only thing that changes.** Each plugin wears
  its own `theme::Palette`, and the five are built at the same OKLCH lightness
  and chroma stop for stop, so a bar at half fill carries the same weight in all
  of them. A test fixes it (`the_palettes_are_one_family`); a hand-edited hex
  that leaves the family fails it. **Five accents that were not one family would
  read as five different designs**, which is the thing the shared width exists
  to prevent.
- **A custom-drawn widget reads the palette, never a constant.**
  `theme::palette(cx)` walks up to the nearest `Palette` model. There are no
  `ACCENT*` constants to reach for — that is deliberate, because a constant is
  how five windows would quietly become one colour again.
- **Two inverted surfaces, and no more.** The status bar along the bottom
  (`nxe_ui::status`), which carries the line about whatever the pointer is on;
  and **the window's figure**, which is what a glance should land on.
- **The status bar is empty at rest.** What the window is for is already on the
  header's right, and a strip that is always talking stops being read.
- **Anything with its own ground on an inverted surface must read the palette
  for it.** A stylesheet cannot see a nested palette. Sparkleur's transfer curve
  was a `.panel` on the inverted figure and went *completely black* — its traces
  followed the palette and inverted, its ground came from the CSS and did not.
  **A figure is drawn by hand and styled by CSS, and only one half follows the
  surface**; whatever carries a ground has to be given one
  (`background-color` from `theme::palette(cx).ground`, read at build time
  inside the surface).
- **Inversion is a palette, not a second concept.** The surface builds
  `Palette::inverted` as a nested model and everything under it comes out right
  without knowing the surface exists. On that ground the accent has nowhere to
  go — the ground *is* the accent — so a mark is near-black and a fill runs from
  black into the surface, which keeps "paler means further" true.
- **A word on the accent ground says so itself** (`.ink` / `.ink-muted`). The
  generated stylesheet is flat and cannot say "labels inside this panel", so
  forgetting the class leaves near white on the accent. **Put few enough words
  there that a miss is obvious.**
- **Every fill is flat** (`UI-21`). The length of a bar is the quantity; a ramp
  running along it says the same thing twice, and it made four bars at four
  values all end in the same pale colour — **the end of the fill stopped telling
  you anything**. The reference this design follows uses no gradients at all.
- **This replaced a rule with a boundary in it.** "A gradient means a quantity,
  a state is flat" failed once at that boundary: a selected segment's label sat
  on a ramp and changed contrast across its own width (`SPK-19`). There is no
  boundary now.
- **`bright` and `deep` are for telling kinds apart, not for filling.** Where
  several of the same thing sit side by side — voice pairs, bands — step between
  them with `Token::mix`. That is the only ramp left.
- **The window is painted by `.class("root")`.** Forget it and the ground is
  the host's black while every `.panel` sits at `BACKGROUND`, so the panels read
  as lighter boxes. The theme's "two levels, not three" needs the window to be
  one of them.

## Type

- **Hierarchy comes from size, weight, colour and rules.** Not from tracking:
  `letter-spacing` and `line-height` do not exist in this vizia revision, so a
  design that needs them cannot be built here.
- **Figures are set in the mono face** (`font::value`). A proportional face
  changes width between a `1` and an `8`, and a value that jitters under a drag
  is what fixing the decimal count was meant to prevent. **Inter's tabular
  figures are not a way out**: `tnum` is an OpenType feature and this vizia
  revision has no way to turn one on.
- **No weight carries meaning. One face, and hierarchy from size and colour.**
  The wordmark was tried in bold (17 px: another label with the volume turned
  up) and in light (26 and 20 px: faint — a name that has to be large to be
  legible is not quiet). It is the plain face at 18. **Both attempts shipped for
  a commit and came back**; if a third is proposed, the principle is what is
  wrong, and it gets rewritten rather than worked around.
- **The wordmark is the product's name with no vendor on it.** `Sparkleur`, not
  `NXE Sparkleur` — the plugin list has already been read by the time the window
  is open.
- **The vendor's mark goes in the window's quietest corner, not beside the
  wordmark.** Next to it, it read as a second wordmark: two marks competing in
  the corner a window is read from. A mark is found, not announced.
- **Corners are square.** `RADIUS_CONTROL` and `RADIUS_CARD` are zero and a
  compile-time assertion keeps them near it. A grid is drawn with straight
  lines.

## Marks

- **A mark never replaces its word.** `nxe_ui::pictogram::heading` and
  `::label` are the two ways one is placed, and both put the name beside it.
  The symbol is what makes a column findable at a glance; the word is what
  makes it understandable the first time. A window of symbols alone buys the
  same unreadability the reference designs sell.
- **They are drawn paths, and there is no icon font any more.** Lucide's
  strokes were baked into filled glyphs, so an icon could not take a weight
  (`UI-2`) — and these sit beside text at two sizes that want two. `UI-17`
  first kept the font for what is generic; by then it was **859 KB in every
  bundle for two icons**, so the two were drawn and the font came out.
- **Drawn for 12 px, because the smallest place one lands is a column
  heading.** Three grid units is the finest feature that survives there, and a
  test says so. Three drawings were replaced for failing it: `UP` and `DOWN`
  were a two-bar compression picture that became one grey pair of blocks, and
  `GAIN` was a fader that read as a plus sign and then a bipolar bar that read
  as a toggle — **worse than unclear, because it said the wrong thing about the
  control under it**.
- **Marks go on columns and lists, not on the macro knobs.** A knob's arc is
  already the picture, and a mark beside its name competes with it. The window's
  subject does not need finding.
- **Nothing is drawn with a curve.** Rules, right angles and flat fills, which
  is the same vocabulary as everything else on screen.

## Readings

- **Fixed width, right aligned** (`nxe_ui::readout`). A reading that changes
  length moves everything laid out after it, three ways: the sign appears
  crossing zero, a digit appears crossing ten, and a dash replaces the whole
  thing in silence. Always printing the sign fixes the first; only a fixed box
  fixes the others.
- **Print the sign even when it carries nothing.** Gain reduction only ever goes
  one way, so `-0.0` at rest is not information — but a minus that appears the
  moment a band starts working makes the figure twitch, which reads as the
  plugin being unsure.
- **Silence is a dash, not a number.** `-142.0 dB` is six characters of noise
  where a glance expects a level.
- **Cells of different content are the same height.** A strip with a bar in it
  and one without came out different heights, and every window below them
  stopped lining up (`SPK-19`).

## What is on screen

- **Every parameter has a control.** A parameter reachable only through the
  host's generic view compiles, saves, automates, and is never mentioned by the
  window — nothing notices. Each plugin has a test that scans `params.rs` for
  `#[id]` fields and fails if one has no mention in the views. Two controls were
  lost to a header rewrite in the one crate that lacked it (`SPK-19`).
- **Anything the audio thread already publishes is already paid for.** Two of
  Sparkleur's handoffs were written every block and read by nothing. Before
  adding an analyser, check what is already in `analysis.rs`.
- **And the mirror of that is worse: a handoff nobody writes.** The editor
  binds to it, the heartbeat reads it thirty times a second, and every figure
  sits at zero while the plugin makes sound — which looks exactly like a track
  with no signal on it. Air shipped one build that way when the whole
  publishing block was lost in an edit. **Nothing in the suite notices**, so
  all four plugins scan their own `analysis.rs` against their `lib.rs`
  (`analysis::tests::every_handoff_is_published`).
- **A protection that works invisibly is a control that does nothing.** If the
  plugin holds something back, the window says so — and says *how far*, because
  "it is being held back" does not tell anyone whether the setting is wrong or
  the material is.
- **Tabs are for controls that are not asked about together, not for saving
  space.** Sparkleur's thirty-three and Velour's twenty-two are asked band by
  band and cannot be asked of half a panel, so neither has tabs. Doubler's
  fifteen are fewer and it keeps them: all of them at once is more choice than
  "how wide, and how far apart" needs. **The count is not what decides it.**

## Judging it

- **Every widget in `nxe-ui` is in `examples/gallery.rs` in the same change.** A
  widget not in the gallery cannot be reviewed without opening a DAW, so it will
  not be.
- **Dimensions are only settled in a host.** Velour went 580 → 528, Sparkleur's
  figure 236 → 176 → 200. Expect the number written first to be wrong.
- **UI defects do not fail tests.** Every one found by looking at a host in
  `SPK-15` and `SPK-19` passed the whole suite. When something looks wrong,
  **measure the screenshot's pixels** — it turned "the panel background looks
  off" into "panels 0x0A, window 0x00" in one step.
