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

- `crates/nxe-ui`: shared Vizia widgets, theme tokens, and the embedded Lucide
  icon font. **Depends on Vizia only, never on nih-plug** — widgets take a
  value plus a callback, and each plugin owns the thin adapter that binds them
  to its own parameters. `examples/gallery.rs` runs every widget as a plain
  desktop app, which is how UI work is iterated without launching a DAW.
  Its `README.md` is the guide for building an interface with it
- `plugins/doubler/doubler-core`: the Doubler DSP. Host-agnostic, allocation-free
  on the audio path
- `plugins/doubler/doubler`: the Doubler nih-plug wrapper — parameter
  declarations, the Vizia UI, and the binding between them
- `plugins/velour/docs`: Velour — the second plugin. **Documents only so far**;
  no crates yet. Requirements and the DSP and UI specifications are written,
  the implementation plan is not
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

The vendor is `NXE`. CLAP ids are `com.nxe.<name>`. **A shipped `CLAP_ID` or
`VST3_CLASS_ID` must never change** — a host stores it in the project file, so
changing it silently breaks every session that used the plugin.

## Licensing

The repository is **GPL-3.0**. This is not a preference: nih-plug's VST3 wrapper
derives from the Steinberg VST3 SDK and is GPLv3, so anything that links it is
too. Do not add a `LICENSE-MIT` or dual-license header to a crate that ends up
in a VST3 bundle.

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

- `rust.md` — Rust and real-time audio rules
- `vizia.md` — UI rules (widget boundaries, theming, icons)
- `documentation.md` — which document owns what, and what a change obliges
