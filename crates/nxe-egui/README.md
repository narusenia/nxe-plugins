# nxe-egui

The v2 widgets and tokens, on egui. The rules the windows follow are
[`.agents/rules/ui.md`](../../.agents/rules/ui.md); the shape of a v2 window is
[`docs/specifications/design-system.md`](../../docs/specifications/design-system.md).

```bash
cargo run -p nxe-egui --example gallery
NXE_GALLERY_DESIGN=1 cargo run -p nxe-egui --example gallery   # the Design window instead
```

## How a widget is used

A widget takes the palette, a value, and the text to show. It never writes the
value; it returns what the pointer did, and the plugin sends that on.

```rust
let control = knob(ui, palette, "Detune", normalized, &text, 44.0);
if control.gestures.begin { ctx.begin_edit(id) }
if let Some(value) = control.gestures.change { ctx.set_param(id, value.into()) }
if control.gestures.end { ctx.end_edit(id) }
if control.gestures.reset { /* set the default, with begin and end around it */ }
if let Some(typed) = control.typed { /* parse it; the unit is the plugin's */ }
```

Start every frame with `nxe_egui::window(ui)` and **draw nothing else when it
returns `false`**: it installs the faces, and egui only applies them from the
next pass.

## What is here

| Module | What |
|---|---|
| `theme` | `Token`, the six `Palette`s, spacing, type sizes, `WINDOW_WIDTH`, `SIDEBAR_WIDTH` |
| `font` | Inter Regular / Medium, Geist Mono; `tracked` for tracked capitals |
| `input` | the shared gesture (vertical drag, Shift fine, double-click reset, Cmd-click to type) |
| `knob`, `bar` | value controls |
| `entry` | the typed value (`UI-3`), opened from a knob's figure |
| `segment` | mutually exclusive choices |
| `number`, `toggle`, `select` | a named value, switch or list in a row of controls: `Root 220 Hz`, `Tuning [Free]`, `Chord Fifth ▾`; corner marks while held. Kit rows and Peek's choices |
| `readout` | an eyebrow over a rule and one fixed-width figure |
| `meter` | a level with a peak mark |
| `curve` | a signal as a monotone cubic, with an optional fill and glow |
| `surface` | the figure's accent panel, and `glow` |
| `sidebar` | the H4 left column: wordmark, subject figure, scale, macros, `NXE` |
| `preset` | the preset bar; knows names, not presets |
| `status` | the hint strip |
| `smooth` | display-side smoothing for signals read from a handoff (decision 10) |
| `text` | reading a typed value back: `number`, `percent`, `switch`, `choice`, `pan` — what a parameter's `parse` method calls |

The palettes came from the v0 vizia `nxe-ui` value for value, and the tests
that keep them one family came with them.
