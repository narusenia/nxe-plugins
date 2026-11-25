# nxe-plugins repository guide

## Project overview

`nxe-plugins` is a monorepo of audio plugins written in Rust. `nxe` comes from
the author's handle `nxeu`.

**v2 (`docs/implementation/v2-plan.md`) moved every plugin** from
[nih-plug](https://github.com/robbert-vdh/nih-plug) + [Vizia](https://github.com/vizia/vizia)
to [truce](https://github.com/truce-audio/truce) + egui; they ship again as
`<name> 2`. All six export **CLAP, VST3 and AU v2** (AU on macOS only),
declared in `truce.toml` at the root. **Pumice was never released**, so it
ships as `Pumice` 1.0.0, not as a "2" (`v2-plan.md`, decision 16).
Nothing in the workspace depends on nih-plug or vizia any more.

v0.x (all five on nih-plug) is kept on the `release/v0` branch; fixes to the
shipped 0.2 builds go there, not to `main`.

Every plugin keeps its DSP in a separate host-agnostic crate (`<plugin>-core`)
that depends on no plugin framework and no UI toolkit. That is what let the
migration replace a wrapper without touching the sound.

Treat the implementation as authoritative when a planning document disagrees
with the code, and fix the document in the same change.

## Repository map

- `crates/nxe-audio`: shared audio **processing** — the harmonic curve, the
  oversampler, biquads, envelope followers, the relative guard, the YIN
  pitch tracker (moved from `parasite-core` when Dew asked), and the
  harmonic measurement the tests are written against. Host-agnostic and
  interface-agnostic. **Separate from `nxe-dsp` on purpose**: a bug here
  breaks the sound, a bug there breaks a picture
- `crates/nxe-spectral`: the overlap-add STFT, moved out of `pumice-core`
  when the Spectral Kit became its second caller. `pumice-core` re-exports it
  under the old paths
- `crates/nxe-egui`: the shared widgets, theme tokens, fonts and the NXE mark,
  on egui. **Depends on egui only, never on truce** — a widget takes a value
  and hands back what the pointer did, and each plugin owns the binding to its
  own parameters. `examples/gallery.rs` runs every widget in an eframe window,
  which is how UI work is iterated without launching a DAW (`mise run
  gallery`). Its `README.md` is the guide
- `plugins/doubler/doubler-core`: the Doubler DSP. Host-agnostic, allocation-free
  on the audio path
- `plugins/doubler/doubler`: the Doubler 2 truce wrapper — parameter
  declarations, the egui editor, and the binding between them. The editor is
  a placeholder until `V2-7`
- `plugins/velour/velour-core`: the Velour DSP. Host-agnostic, allocation-free
  on the audio path. What is left here is what only makes sense as Velour —
  the shared blocks moved to `nxe-audio` in `SPK-1`, and `envelope` and
  `guard` are thin wrappers holding Velour's tuning
- `plugins/velour/velour`: the Velour 2 truce wrapper — parameter
  declarations, the egui editor, and the binding between them
- `plugins/sparkleur/sparkleur-core`: the Sparkleur DSP — five-band multiband
  dynamics with a transient-gated harmonic generator. Host-agnostic,
  allocation-free on the audio path
- `plugins/sparkleur/sparkleur`: the Sparkleur 2 truce wrapper — parameter
  declarations, the egui editor, and the binding between them
- `plugins/air/air-core`: the Air DSP — a high-frequency texture generated from
  the input and placed where the input is not. Host-agnostic, allocation-free
  on the audio path. `noise` is the only block written for it, and it is
  written not to know what Air is: two more plugins want a noise generator, and
  the second to ask moves the file into `nxe-audio` unchanged
- `plugins/air/air`: the Air 2 truce wrapper — parameter declarations, the
  egui editor, and the binding between them
- `plugins/diorama/diorama-core`: the Diorama DSP — a vocal's distance, built
  from early reflections and a direct path that is itself processed. **No
  reverb tail**: 10–120 ms only. Host-agnostic, allocation-free on the audio
  path
- `plugins/diorama/diorama`: the Diorama 2 truce wrapper — parameter
  declarations, the egui editor, and the binding between them. Shipped as
  `NXE Diorama` 0.2.0 after the rename in `DIO-15` (it was `Vocal Depth`)
- `plugins/pumice/pumice-core`: the Pumice DSP — dynamic resonance suppression
  for a single vocal. **The first engine in the line with an FFT on the audio
  path**, and the first whose plugin reports latency
  (`docs/specifications/architecture.md` says why both are exceptions rather
  than broken rules; the Spectral Kits follow it). Host-agnostic,
  allocation-free on the audio path; its overlap-add buffering now lives in
  `nxe-spectral`, written rather than borrowed from `nih_plug::util::stft`,
  which would move the crate to the GPL side
- `plugins/pumice/pumice`: the Pumice 1.0 truce wrapper — parameter
  declarations, the egui editor, and the binding between them. What is left
  is an ear on the defaults. Its figure is the only one in the line whose
  points can be
  put anywhere, added and removed. **Its adaptive detection was dropped from
  v1** — the product's whole claim, measured and found not to work, with the
  three failed statistics recorded in `REQ-PUM-003` rather than deleted
- `plugins/peek/peek-core`: the measurements behind every Peek meter. **One
  crate for the whole series**, not one per plugin: a single meter and `Peek`
  read the same implementation, so they cannot disagree. No meter touches the
  signal
- `plugins/peek/peek-ui`: the Peek figures, one per measurement, drawn into
  whatever rect they are given — so a single meter and a cell of `Peek` are
  the same picture. Its `bind` is the only part that knows truce
- `plugins/peek/peek-<measurement>`: one meter per plugin (Level, VU, Stereo,
  Spectrum, Spectrogram, Loudness, Scope) on the Kit window
  (`nxe_egui::kit`). Thin: declarations, and a window that calls `peek-ui`
- `plugins/peek/peek`: every meter at once. **The one window in the line the
  host may resize**; it never asks the host to resize it
- `plugins/kit/nonlinear-core`: the engines behind the Design Kits — Fold,
  Fever, Buckle. Each is a plain curve the window draws and an engine that
  runs it 4x oversampled, the mix inside the oversampled loop. Devour takes
  them over when it comes
- `plugins/kit/kit-ui`: what every Kit window shares — the binding to truce,
  the preset bar over a table of factory presets, the frame, and the transfer
  curve figure
- `plugins/kit/spectral-core`: the engines behind the Spectral Kits —
  Invert, Shear, Afterimage — on `nxe-spectral`'s STFT. They report a block of
  latency, as Pumice does
- `plugins/kit/resonance-core`: the engines behind the Colour Kits — Dew
  and Halo — and the Colour flagships built on them, Meniscus and Prisma
- `plugins/kit/particle-core`: the engines behind the Effloresce Kits —
  Glint and Pollen. Effloresce takes them over when it comes
- `plugins/kit/<name>`: one Kit per plugin (`fold`, `fever`, `buckle`,
  `invert`, `shear`, `afterimage`, `dew`, `halo`, `glint`, `pollen`), on the Kit window with
  `Palette::DESIGN_KIT`, or `Palette::COLOUR_KIT` for the Colour Kits.
  Thin: declarations, presets, and the figure
- `plugins/meniscus/meniscus`: the Meniscus truce wrapper, the first Colour
  flagship, on the Colour window (`nxe_egui::colour`, Colour 6: rounded
  neutral panels, colour only on the notes). Its sound is
  `resonance_core::meniscus`
- `plugins/prisma/prisma`: the Prisma truce wrapper, on the same window.
  **The one effect in the line that takes MIDI** (`midi_input` in
  `truce.toml`, an `aumf` AU). Its sound is `resonance_core::prisma`
- `plugins/parasite/parasite-core`: the Parasite DSP — an oscillator that
  follows the input's pitch and feeds on it, four ways (FM, Ring, Sync,
  Voice). Its pitch tracker is `nxe_audio::pitch`. Reports a fixed ~10 ms
  latency
- `plugins/parasite/parasite`: the Parasite truce wrapper, the first Design
  flagship, on the Design window (`nxe_egui::design`, Design 7: a square
  figure and three columns of thin sliders). It borrows `kit-ui`'s binding
  and preset bar until Singularity needs them too
- `docs/`: monorepo-wide documents (architecture, cross-plugin backlog and
  roadmap). Indexed by `docs/README.md`
- `plugins/<name>/docs/`: that plugin's own requirements, specifications, and
  implementation plans. A plugin is self-contained, so it can be split into its
  own repository without breaking its documents

Important references:

- `docs/HANDOVER.md` — **start here.** What is working, what is not, and which
  traps cost the most time
- `docs/README.md` — documentation index (which document plays which role)
- `docs/specifications/architecture.md` — crate layout, dependency direction,
  build and release, how to add a new plugin
- `docs/implementation/backlog.md` — every implementation unit on one page
- `docs/implementation/roadmap.md` — the order and why it is that order

## Naming

From v2 a plugin's name is **the bare name** — `Doubler 2`, `Pumice` — with
no `NXE` in front (v0 shipped as `NXE Doubler`). The vendor is a field of its
own (CLAP / VST3 vendor `NXE`, AU manufacturer `Nxeu`), and hosts group by it,
so repeating it in the name only doubled it (`NXE: NXE Doubler 2` in Logic).
That string is the plugin's `name` in `truce.toml`, and how it appears in a
host's plugin list. The **crate** keeps the bare name
(`doubler`, `doubler-core`) so paths stay short, and the documents use the bare
name as shorthand.

**The window's wordmark is the bare name** — `Doubler`, with `NXE` beside it as
its own small mark (`UI-19`). The list has already been read by the time the
window is open, so repeating the vendor inside it buys nothing.

The vendor is `NXE`. The v0 CLAP ids were `com.nxe.<name>`; on truce,
`bundle_id = "<name>2"` gives `com.nxe.<name>2`, and truce derives the VST3
class id and the state hash from the same key. The AU manufacturer is `Nxeu`,
and every plugin's AU `fourcc` is fixed in `truce.toml`. **None of these may
change once shipped** — a host stores them in the project file, so changing one
silently breaks every session that used the plugin. **Parameter ids on truce
are a hash of the field name**, so renaming a parameter's field is the same
kind of break; its display `name` is free.

## Licensing

**Everything is MIT OR Apache-2.0.** [`LICENSING.md`](LICENSING.md) is the
full map and the reasoning. truce's own license is MIT / Apache-2.0 with a
rider that only covers reselling truce as a framework, which shipping plugins
is not.

**The v0 builds were GPLv3**, because nih-plug links `vst3-sys` (GPLv3)
unconditionally. Do not add a dependency that links `vst3-sys` or anything
else GPL without saying so in `LICENSING.md` — it would move the crate, and
every bundle built from it, to the GPL.

## Verification

`mise run check` is the canonical entry point (fmt, clippy, tests). The
lefthook pre-commit hook runs the same tasks; install it once with
`mise run hooks:install`.

A plugin is built, installed and validated through `cargo truce`, which
`mise.toml` pins to the same version as the crates (`mise run install doubler`
is the first line):

```bash
cargo truce install --user --clap --vst3 --au2 -p doubler
cargo truce validate --clap --pluginval --auval -p doubler
```

CI runs that validate on macOS (`.github/workflows/check.yml`). **On a
development Mac, `auval` can report `didn't find the component` for a plugin
the DAWs list and load fine** — it has happened more than once, and other
vendors' components vanish from `auval -a` at the same time. Trust the CI run
and the host, not a local `auval`, until the registrar recovers.

## Rules

`.agents/rules/` states what must hold:

- `git.md` — commit messages, branches, pull request titles
- `rust.md` — Rust and real-time audio rules
- `ui.md` — what the windows have to look like and how they have to read
- `documentation.md` — which document owns what, and what a change obliges
