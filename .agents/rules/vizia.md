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

**The same rule is how a control becomes untouchable.** A custom-drawn widget
with no size of its own — `Bar` is one — collapses to nothing inside an
`Auto`-sized row, and what is left is a hairline that still draws, still binds,
and cannot be hit. It reads as "this control does nothing", not as a layout
problem. Velour's whole Advanced table shipped that way (`VEL-14`). **Give every
custom widget an explicit height**; the gallery's panels are the reference for
what each one expects.

**`font-size` takes a bare number, not a length.** This revision parses it as a
keyword (`medium`, `x-small`, …) or an `f32` and nothing else, so `font-size:
12px` fails to parse and **the declaration is dropped silently** — the label
renders at the 16 px default. Every other length in the stylesheet does take
`px`, which is what makes this one easy to miss: the design simply looked large,
and no size ever changed anything. Write `font-size: 12;`.

More generally: **a CSS declaration this revision cannot parse costs nothing at
runtime and says nothing.** When a stylesheet change appears to do nothing, check
the value type in `vizia_style` before assuming the rule did not match.

**`cx.add_timer` never fires on baseview.** `process_timers` — and
`emit_scheduled_events` with it — is called by `vizia_winit` and by nothing else.
Every plugin editor and the gallery run on baseview, so a timer compiles, starts,
and does nothing at all. For a periodic update use `cx.spawn`, which hands out a
`ContextProxy`; baseview does install an event proxy, and `proxy.emit` returns
`Err` once the window is gone, which is the thread's cue to stop.

(If a timer is ever used on a backend that does run them: **`TimerAction::Tick`
carries the elapsed time, not the interval**, so `action == TimerAction::Tick(interval)`
compiles, reads as correct, and is false on essentially every tick. Match on the
shape.)

**A tooltip's content is hit-testable even though the tooltip is not.**
`.tooltip(…)` builds the view as a child of the anchor with `hoverable(false)`
and `top: 100%` — so it hangs directly over whatever sits *below* the control,
and the label inside it, invisible at `opacity: 0`, swallows the clicks meant
for that. Mark the content `.decoration` (`theme::hint` does). It also has no
placement logic: near the right edge it runs off the window unless something
sets `left: 1s; right: 0px` on it.

**Centring a container's children costs a stretching child its width — and its
height.** `child-left: 1s` / `child-right: 1s` are two more `Stretch`es for the
row to divide, so a child asking for `Stretch(1.0)` gets a *third* of the space
rather than all of it. Velour's transfer-curve window was drawn 44 px wide
inside a 132 px column for exactly this reason (`VEL-13`). Centre the thing that
needs centring — put the stretch on the `Label`, not on the column that also
holds a full-width view.

**An absolutely-positioned label at the far edge runs off it.** A caller
placing axis marks with `left: Percentage(…)` puts the last one at 100 %, which
is where it *starts* — "20k" renders as "20" with the rest outside the box. Hang
the last one off the other side instead: `left: 1s; right: 0px`. Same absence of
placement logic as the tooltip above.

**`.class("row")` does this vertically to everything in it.** The class carries
`child-top: 1s` and `child-bottom: 1s`, so a `Stretch(1.0)` child of a 176 px
row is 58 px tall and whatever is inside it hangs out of the bottom. Sparkleur's
transfer window shipped that way for one build (`SPK-15`). A row whose children
already have heights of their own does not want the class.

**A widget must not move its own value optimistically.** `binding_system`
re-reads every bound lens and compares with `Data::same`, so **a write the caller
clamps produces no update** — the value did not change. A widget that moved its
local copy first is then left showing something nobody accepted, and nothing ever
corrects it. Report the gesture and redraw from what comes back.

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

## 字送りと行間は無い

`letter-spacing` と `line-height` は**このリビジョンのプロパティ表に存在しない**
（`vizia_style/src/property.rs` が解析する名前の一覧が正）。スイス様式の
字送りを効かせた大文字ラベルは書けないので、**階層はサイズ・ウェイト・色・
罫線で作る**。

使えるものは確認済み: **片側の罫線**（`border-top-width` など）、`text-align`、
`font-weight`、`transition`、`opacity`、`clip-path`、`min-*` / `max-*`、
`z-index`、`box-shadow`。

**グラデーションは `background-image` にだけ書ける。** `background-color` に
`linear-gradient` を渡しても解析されない。両方指定すると色の**上に**勾配が
乗るのではなく、色が下に残る。radial は無い。
