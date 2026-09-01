# nxe-plugins repository guide

## Project overview

`nxe-plugins` is a monorepo of audio plugins written in Rust. Each plugin is
built on [nih-plug](https://github.com/robbert-vdh/nih-plug) and exports **CLAP
and VST3**; the UI is [Vizia](https://github.com/vizia/vizia) through
`nih_plug_vizia`. `nxe` comes from the author's handle `nxeu`.

**AU is out of scope** — nih-plug has no AU wrapper. Every plugin therefore
keeps its DSP in a separate host-agnostic crate (`<plugin>-core`) that depends
on neither nih-plug nor Vizia, so a future AU wrapper written against another
framework can call the same code.

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
- `plugins/doubler/doubler-core`: the Doubler DSP. Host-agnostic, allocation-free
  on the audio path
- `plugins/doubler/doubler`: the Doubler nih-plug wrapper — parameter
  declarations, the Vizia UI, and the binding between them
- `plugins/velour/velour-core`: the Velour DSP. Host-agnostic, allocation-free
  on the audio path. What is left here is what only makes sense as Velour —
  the shared blocks moved to `nxe-audio` in `SPK-1`, and `envelope` and
  `guard` are thin wrappers holding Velour's tuning
- `plugins/velour/velour`: the Velour nih-plug wrapper — parameter
  declarations, the Vizia UI, and the binding between them
- `plugins/sparkleur/sparkleur-core`: the Sparkleur DSP — five-band multiband
  dynamics with a transient-gated harmonic generator. Host-agnostic,
  allocation-free on the audio path
- `plugins/sparkleur/sparkleur`: the Sparkleur nih-plug wrapper — parameter
  declarations, the Vizia UI, and the binding between them
- `plugins/air/air-core`: the Air DSP — a high-frequency texture generated from
  the input and placed where the input is not. Host-agnostic, allocation-free
  on the audio path. `noise` is the only block written for it, and it is
  written not to know what Air is: two more plugins want a noise generator, and
  the second to ask moves the file into `nxe-audio` unchanged
- `plugins/air/air`: the Air nih-plug wrapper — parameter declarations, the
  Vizia UI, and the binding between them
- `plugins/diorama/diorama-core`: the Diorama DSP — a vocal's distance, built
  from early reflections and a direct path that is itself processed. **No
  reverb tail**: 10–120 ms only. Host-agnostic, allocation-free on the audio
  path
- `plugins/diorama/diorama`: the Diorama nih-plug wrapper. **Not shipped yet** —
  the only one of the five that has never been released, which is why it could
  still be renamed (`DIO-15`, it was `Vocal Depth`)
- `plugins/pumice/pumice-core`: the Pumice DSP — dynamic resonance suppression
  for a single vocal. **The only engine in the line with an FFT on the audio
  path**, and the only one whose plugin reports latency
  (`docs/specifications/architecture.md` says why both are exceptions rather
  than broken rules). Host-agnostic, allocation-free on the audio path; the
  overlap-add buffering is written here rather than borrowed from
  `nih_plug::util::stft`, which would move the crate to the GPL side
- `plugins/pumice/pumice`: the Pumice nih-plug wrapper. **Implemented up to its
  window**; what is left is an ear on the defaults. Its figure
  (`nxe_ui::node::NodeField`) is the only one in the line whose points can be
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
follows. That string is the plugin's `NAME`, its bundle name in `bundler.toml`,
and how it appears in a host's plugin list. The **crate** keeps the bare name
(`doubler`, `doubler-core`) so paths stay short, and the documents use the bare
name as shorthand.

**The window's wordmark is the bare name** — `Doubler`, with `NXE` beside it as
its own small mark (`UI-19`). The list has already been read by the time the
window is open, so repeating the vendor inside it buys nothing.

The vendor is `NXE`. CLAP ids are `com.nxe.<name>`. **A shipped `CLAP_ID` or
`VST3_CLASS_ID` must never change** — a host stores it in the project file, so
changing it silently breaks every session that used the plugin.

## Licensing

**Two licenses, and the boundary is whether the crate links `vst3-sys`.**
[`LICENSING.md`](LICENSING.md) is the full map and the reasoning.

- `nxe-audio`, `nxe-dsp`, `nxe-ui`, `<plugin>-core` — **MIT OR Apache-2.0**.
  None of them depends on `vst3-sys`, which is a consequence of the dependency
  rules in `docs/specifications/architecture.md`, not an accident
- `nxe-plug-ui` and the three wrapper crates — **GPL-3.0-only**. `nih_plug`
  depends on `vst3-sys` unconditionally, and `vst3-sys` is GPLv3

**nih-plug itself is ISC.** The GPL comes from `vst3-sys` alone. Adding a
dependency on `nih_plug` to a crate on the permissive side moves it to the GPL
side; do not do it without saying so in `LICENSING.md`.

**Both shipped bundles are GPLv3, `.clap` included.** Each plugin calls
`nih_export_clap!` and `nih_export_vst3!` from one cdylib and the bundler copies
that same binary into both, so "the CLAP build has no VST3 in it" is false here.

Steinberg relicensed the VST3 interface headers to MIT in 2025, so the reason
`vst3-sys` chose GPLv3 no longer holds upstream — but `vst3-sys` is GPLv3 by its
authors' choice, and only dropping the dependency changes anything.

## Verification

`mise run check` is the canonical entry point (fmt, clippy, tests). The
lefthook pre-commit hook runs the same tasks; install it once with
`mise run hooks:install`.

Local bundling and installation:

```bash
mise run bundle doubler    # cargo xtask bundle doubler --release
mise run install doubler   # bundle, then copy into ~/Library/Audio/Plug-Ins
```

## Rules

`.agents/rules/` states what must hold:

- `git.md` — commit messages, branches, pull request titles
- `rust.md` — Rust and real-time audio rules
- `ui.md` — what the windows have to look like and how they have to read
- `vizia.md` — what this vizia revision does, and what it silently does not
- `documentation.md` — which document owns what, and what a change obliges
