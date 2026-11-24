# nxe-plugins repository guide

## Project overview

`nxe-plugins` is a monorepo of audio plugins written in Rust. `nxe` comes from
the author's handle `nxeu`.

**The repository is mid-migration (v2, `docs/implementation/v2-plan.md`).**
Plugins move one at a time from [nih-plug](https://github.com/robbert-vdh/nih-plug)
+ [Vizia](https://github.com/vizia/vizia) to
[truce](https://github.com/truce-audio/truce) + egui, and ship again as
`NXE <name> 2`:

- **On truce**: all six — Doubler, Sparkleur, Velour, Air, Diorama and Pumice. Exports **CLAP, VST3 and AU v2** (AU on macOS only);
  `truce.toml` at the root declares them
- **Pumice was never released**, so it ships as `NXE Pumice` 1.0.0, not as a
  "2" (`v2-plan.md`, decision 16)
- The nih-plug side (`nxe-ui`, `nxe-plug-ui`, xtask, `bundler.toml`) is no
  longer used by any plugin and goes in its own change

v0.x (all five on nih-plug) is kept on the `release/v0` branch; fixes to the
shipped 0.2 builds go there, not to `main`.

Every plugin keeps its DSP in a separate host-agnostic crate (`<plugin>-core`)
that depends on no plugin framework and no UI toolkit. That is what lets the
migration replace a wrapper without touching the sound.

Treat the implementation as authoritative when a planning document disagrees
with the code, and fix the document in the same change.

## Repository map

- `crates/nxe-audio`: shared audio **processing** — the harmonic curve, the
  oversampler, biquads, envelope followers, the relative guard, and the
  harmonic measurement the tests are written against. Host-agnostic and
  interface-agnostic. **Separate from `nxe-dsp` on purpose**: a bug here
  breaks the sound, a bug there breaks a picture
- `crates/nxe-plug-ui`: the adapter between nih-plug's parameters and the
  `nxe-ui` widgets — **the only crate allowed to know both**. Separate from
  `nxe-ui` so that `examples/gallery` never links nih-plug
- `crates/nxe-ui`: shared Vizia widgets, theme tokens, and the plugins' own
  drawn symbols. **Depends on Vizia only, never on nih-plug** — widgets take a
  value plus a callback, and each plugin owns the thin adapter that binds them
  to its own parameters. `examples/gallery.rs` runs every widget as a plain
  desktop app, which is how UI work is iterated without launching a DAW.
  Its `README.md` is the guide for building an interface with it
- `crates/nxe-egui`: the v2 widgets and tokens, on egui. **Depends on egui
  only, never on truce** — a widget takes a value and hands back what the
  pointer did. `examples/gallery.rs` runs every widget in an eframe window.
  The palettes are `nxe-ui`'s value for value; **change one in both or in
  neither** until `nxe-ui` goes with the last vizia plugin. Its `README.md` is
  the guide
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
  for a single vocal. **The only engine in the line with an FFT on the audio
  path**, and the only one whose plugin reports latency
  (`docs/specifications/architecture.md` says why both are exceptions rather
  than broken rules). Host-agnostic, allocation-free on the audio path; the
  overlap-add buffering is written here rather than borrowed from
  `nih_plug::util::stft`, which would move the crate to the GPL side
- `plugins/pumice/pumice`: the Pumice 1.0 truce wrapper — parameter
  declarations, the egui editor, and the binding between them. What is left
  is an ear on the defaults. Its figure is the only one in the line whose
  points can be
  put anywhere, added and removed. **Its adaptive detection was dropped from
  v1** — the product's whole claim, measured and found not to work, with the
  three failed statistics recorded in `REQ-PUM-003` rather than deleted
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

Plugins ship as **`NXE <name>`** — `NXE Doubler`, and so on for everything that
follows; from v2 as **`NXE <name> 2`**. That string is the plugin's `name` in
`truce.toml` (or `NAME` and its `bundler.toml` entry while it is on nih-plug),
and how it appears in a host's plugin list. The **crate** keeps the bare name
(`doubler`, `doubler-core`) so paths stay short, and the documents use the bare
name as shorthand.

**The window's wordmark is the bare name** — `Doubler`, with `NXE` beside it as
its own small mark (`UI-19`). The list has already been read by the time the
window is open, so repeating the vendor inside it buys nothing.

The vendor is `NXE`. On nih-plug, CLAP ids are `com.nxe.<name>`; on truce,
`bundle_id = "<name>2"` gives `com.nxe.<name>2`, and truce derives the VST3
class id and the state hash from the same key. The AU manufacturer is `Nxeu`,
and every plugin's AU `fourcc` is fixed in `truce.toml`. **None of these may
change once shipped** — a host stores them in the project file, so changing one
silently breaks every session that used the plugin. **Parameter ids on truce
are a hash of the field name**, so renaming a parameter's field is the same
kind of break; its display `name` is free.

## Licensing

**Two licenses, and the boundary is whether the crate links `vst3-sys`.**
[`LICENSING.md`](LICENSING.md) is the full map and the reasoning.

- `nxe-audio`, `nxe-dsp`, `nxe-ui`, `<plugin>-core`, and **every wrapper on
  truce** — **MIT OR Apache-2.0**. truce's own license is MIT / Apache-2.0
  with a rider that only covers reselling truce as a framework, which shipping
  plugins is not
- `nxe-plug-ui` and the wrappers still on nih-plug — **GPL-3.0-only**.
  `nih_plug` depends on `vst3-sys` unconditionally, and `vst3-sys` is GPLv3

**nih-plug itself is ISC.** The GPL comes from `vst3-sys` alone. Adding a
dependency on `nih_plug` to a crate on the permissive side moves it to the GPL
side; do not do it without saying so in `LICENSING.md`.

**A nih-plug plugin's bundles are GPLv3, `.clap` included.** Each calls
`nih_export_clap!` and `nih_export_vst3!` from one cdylib and the bundler copies
that same binary into both, so "the CLAP build has no VST3 in it" is false here.

## Verification

`mise run check` is the canonical entry point (fmt, clippy, tests). The
lefthook pre-commit hook runs the same tasks; install it once with
`mise run hooks:install`.

Local bundling and installation of a plugin still on nih-plug:

```bash
mise run bundle velour     # cargo xtask bundle velour --release
mise run install velour    # bundle, then copy into ~/Library/Audio/Plug-Ins
```

A plugin on truce goes through `cargo truce`, which `mise.toml` pins to the
same version as the crates:

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
- `vizia.md` — what this vizia revision does, and what it silently does not
- `documentation.md` — which document owns what, and what a change obliges
