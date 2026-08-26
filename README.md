# nxe-plugins

Audio plugins in Rust, built on [nih-plug](https://github.com/robbert-vdh/nih-plug)
with a [Vizia](https://github.com/vizia/vizia) UI. Each plugin exports **CLAP**
and **VST3**.

> **Status: design only.** Nothing is implemented yet. The requirements and
> specifications are written; see [`docs/implementation/roadmap.md`](docs/implementation/roadmap.md)
> for what is being built and in what order.

## Plugins

| Plugin | What it is | Status |
|---|---|---|
| [NXE Doubler](plugins/doubler/docs/requirements/REQ-DBL.md) | Multi-voice doubler — 2/4/8 detuned, delayed, humanized voices from one source | `doubler-v0.1.1` |
| [NXE Velour](plugins/velour/docs/requirements/REQ-VEL.md) | Vocal presence saturator — three parallel harmonic generators added to an untouched dry path | `velour-v0.1.1` |
| [NXE Sparkleur](plugins/sparkleur/docs/requirements/REQ-SPK.md) | Multiband dynamics with a transient-gated harmonic generator — five bands, upward and downward | `sparkleur-v0.1.1` |

## Formats

CLAP and VST3, for macOS (Apple Silicon and Intel), Windows, and Linux.

**AU is not supported** and is not planned: nih-plug has no AU wrapper. Logic
Pro and GarageBand cannot load these plugins. Every plugin keeps its DSP in a
host-agnostic crate so that an AU wrapper remains possible later, but none
exists.

## Building

Requires Rust 1.95 or newer. [mise](https://mise.jdx.dev/) manages the
toolchain and tasks.

```bash
mise trust                  # once, after cloning
mise run check              # fmt + clippy + tests
mise run bundle doubler     # build the CLAP and VST3 bundles
mise run install doubler    # bundle, then install into the user plugin folders
mise run gallery            # run the shared UI widget gallery as a desktop app
```

Working on this repository? Start with [`AGENTS.md`](AGENTS.md) for the layout
and the rules, [`docs/README.md`](docs/README.md) for which document plays which
role, and [`crates/nxe-ui/README.md`](crates/nxe-ui/README.md) if you are
touching the interface.

## Installing a release build

Each plugin is released on its own, tagged `<plugin>-v<version>`, and attaches a
zip per platform (`nxe-doubler-0.1.0-macos.zip`). Unpack it and copy the bundles into:

| | CLAP | VST3 |
|---|---|---|
| macOS | `~/Library/Audio/Plug-Ins/CLAP` | `~/Library/Audio/Plug-Ins/VST3` |
| Windows | `%COMMONPROGRAMFILES%\CLAP` | `%COMMONPROGRAMFILES%\VST3` |
| Linux | `~/.clap` | `~/.vst3` |

**macOS builds are not signed or notarized.** The first time you load one,
Gatekeeper will refuse it. Clear the quarantine attribute:

```bash
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/'NXE Doubler.clap'
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/'NXE Velour.vst3'
```

## License

**Two licenses.** The shared crates — `nxe-audio`, `nxe-dsp`, `nxe-ui` and each
plugin's `<plugin>-core` — are **MIT OR Apache-2.0**, so the DSP and the widgets
can be reused anywhere. The wrapper crates and the shipped plugin bundles are
**GPL-3.0-only**, because `nih_plug` links `vst3-sys` and that is GPLv3.

[`LICENSING.md`](LICENSING.md) has the per-crate table and the reasoning.
[LICENSE-MIT](LICENSE-MIT) / [LICENSE-APACHE](LICENSE-APACHE) /
[LICENSE-GPL-3.0](LICENSE-GPL-3.0).
