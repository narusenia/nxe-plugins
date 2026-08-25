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
- The generated constant module is generated, not edited. Regenerate it from
  Lucide's `font/info.json` when the font is updated.
- Stroke width is not adjustable in the font. An icon that needs a variable
  stroke has to be drawn as a path in `View::draw`; that is a deliberate,
  documented exception, not the default.

## Parameter interaction

- A knob or slider bound to a parameter supports: vertical drag, fine drag with
  `Shift`, double-click to reset to default, and a tooltip showing the value
  the host would display.
- **A macro control and a per-voice control must never overwrite each other.**
  The per-voice value is a normalized shape and the macro scales it (see
  `plugins/doubler/docs/specifications/ui.md`). Any UI that writes a computed
  value back into the other layer is wrong.
