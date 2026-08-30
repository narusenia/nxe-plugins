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
- **The three plugins share one width.** Opened side by side they are one
  product; different widths make them look like three. Heights differ, because
  the amount inside them differs.
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
- **A gradient means a quantity.** Use `palette.paint` where the fill
  measures something: how far a bar got, how loud a meter is, how far a knob
  turned. Pass **the whole track**, not the filled part, so two controls at
  different values are the same colour where they overlap.
- **A state is flat.** On/off has no "further" for the pale end to mean, and a
  word sitting on a ramp changes contrast across its own width — a selected
  segment read badly, which is how this rule got written (`SPK-19`). Traces and
  rules are flat too: neither is a quantity.
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
  is what fixing the decimal count was meant to prevent.
- **Corners are square.** `RADIUS_CONTROL` and `RADIUS_CARD` are zero and a
  compile-time assertion keeps them near it. A grid is drawn with straight
  lines.

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
