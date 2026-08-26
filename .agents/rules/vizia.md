---
paths:
  - "crates/nxe-ui/**"
  - "plugins/**/src/ui/**"
  - "plugins/**/*.css"
---

# UI rules

## Widget boundaries

- A widget in `nxe-ui` knows a value, a range, and a callback. It does not know
  what a parameter is. If a widget needs `ParamPtr` to work, it belongs in the
  plugin crate instead.
- Every widget added to `nxe-ui` is also added to `examples/gallery.rs` in the
  same change. A widget that is not in the gallery cannot be reviewed without
  opening a DAW, so it will not be.

## Styling

- Colors, spacing, and radii come from the theme tokens in `nxe-ui`, never as
  literals in a view. A hard-coded hex in a plugin's UI is a bug report against
  the token set: add the token.
- Layout and static appearance go in CSS. Reach for a custom `View::draw` only
  when the shape is genuinely not expressible as styled boxes and text — knobs,
  arcs, meters, curves.
- **No blur, no glow, no frosted glass.** femtovg has no blur, so anything that
  needs one has to be faked with layered geometry and will look worse than not
  doing it. Depth comes from value contrast and one-pixel borders.

## Icons

- Icons are Lucide glyphs from the embedded `lucide.ttf`, referenced through
  the generated constants (`nxe_ui::icon::CHEVRON_DOWN`), never as a raw escape
  in a view.
- **Build them with `icon::label`, and never set `font-family` in CSS.** On the
  vizia revision `nih_plug_vizia` pins, a stylesheet's `font-family` does not
  select an embedded font; the glyphs come from a fallback face instead. Because
  the codepoints are in the private use area, that failure renders as unrelated
  CJK glyphs rather than as a blank or a missing-glyph box, so it is easy to
  mistake for a broken font file. The family has to come from the modifier.
- The generated constant module is generated, not edited. Regenerate it from
  Lucide's `font/info.json` when the font is updated.
- Stroke width is not adjustable in the font. An icon that needs a variable
  stroke has to be drawn as a path in `View::draw`; that is a deliberate,
  documented exception, not the default.

## What this vizia revision does and does not do

`nih_plug_vizia` pins vizia to Robbert van der Helm's fork at tag
`patched-2024-05-06` (see `docs/specifications/architecture.md`). These are
things that cost real time to find. Read this section before assuming something
does not work because you wrote it wrong.

**`winit` and `baseview` are mutually exclusive.** `Application` is re-exported
under `cfg(all(not(feature = "winit"), feature = "baseview"))` and the mirror
image, so enabling both exports **neither** and nothing that uses `Application`
compiles — including `nih_plug_vizia` itself. The whole workspace is on
`baseview`; never enable `winit`, not even as a dev-dependency (resolver v2
unifies dev-dependency features once a dev target is built).

**A stylesheet's `font-family` does not select an embedded font.** The
declaration parses and the conversion to cosmic-text's family list looks correct
on inspection, but the glyphs come from a fallback face. Set the family with the
`font_family` modifier. For icons, `icon::label` does this. The failure mode is
worth knowing: private use codepoints in a fallback face render as unrelated CJK
glyphs, so it looks like a corrupt font file rather than a font that was never
selected.

**`draw_text` renders the view's own text, nothing else.** A custom `View` cannot
put labels at arbitrary positions inside itself, so a widget cannot label its own
gridlines or number its own dots. Two ways out, both used here: let the caller
place labels as absolutely-positioned siblings using the same mapping the widget
was given (`CurveView`), or report what the pointer is over so the caller can
highlight the matching row elsewhere (`PolarField`).

**The default font is set through `set_default_font`, not CSS.** Same reason as
the icon family — a stylesheet's `font-family` does not select an embedded face.
`theme::install` registers Geist and makes it the default, so a plain `Label`
needs nothing. A different family for one label needs the modifier;
`font::value` is that for figures.

**A container's `on_press` needs its content marked `pointer-events: none`.**
Vizia emits a press only when the entity hovered on mouse-up is the one hovered
on mouse-down (`hovered == triggered`). A pressable box with two labels in it
therefore fires only when the pointer happens not to cross from one label to
the other — which reads as "the button needs several clicks", not as a layout
problem. Put `.class("decoration")` on anything inside something pressable.

**Vizia's default text colour is black.** A `Label` with no colour disappears on
a dark surface. The stylesheet has a base `label` element rule for exactly this;
do not remove it.

**The CSS property names are not the web's.** `child-space` rather than padding,
`col-between` / `row-between` rather than gap, `space` / `left` / `top` rather
than margin, and `layout-type: row | column` to choose the axis. Layout is
Morphorm, not flexbox.

**A view's default width and height are `Stretch(1.0)`, and a stretching child
of an auto-sized parent resolves to zero.** `Handle::width` unset means Morphorm
picks the default, not "size to content". Wrapping content in an `HStack` inside
an `width: auto` container therefore renders a sliver with the content clipped
away — the same CSS works when the child is a `Label`, which sets `Auto` itself.
When a container's size comes from its content, say `.width(Auto)` on every
level, not just the outermost.

**Bind inside `build`'s closure.** `Res::set_or_bind` needs `cx`, and the handle
`build` returns holds it, so binding after the build does not compile. Inside the
closure, `cx.current()` is the new view.

**A custom type in a lens needs `Data`.** With `PartialEq` derived it is a
one-line impl.

**`on_press` actions must be `Send + Sync`.** Share a handler across children
with `Arc`, not `Rc`.

**The built-in `Knob` is not usable for a plugin.** It takes a `Lens` and offers
only `on_changing` — no way to tell the host a gesture started and ended, which
is what makes a host record an automation move as one edit. Same for adding
shift-fine, double-click reset, or type-a-value around it.

## Parameter interaction

- A knob or slider bound to a parameter supports: vertical drag, fine drag with
  `Shift`, double-click to reset to default, and a tooltip showing the value
  the host would display.
- **A lens can only map one field.** A value derived from two of them cannot be
  produced in a lens `map`; compute it when the inputs change and bind to the
  result. Getting this wrong is silent — the display simply stops responding to
  one of its inputs, which reads as "that control does nothing".
- **A macro control and a per-voice control must never overwrite each other.**
  The per-voice value is a normalized shape and the macro scales it (see
  `plugins/doubler/docs/specifications/ui.md`). Any UI that writes a computed
  value back into the other layer is wrong.
