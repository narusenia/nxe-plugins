# nxe-plugins

Audio plugins in Rust, built on [nih-plug](https://github.com/robbert-vdh/nih-plug)
with a [Vizia](https://github.com/vizia/vizia) UI. Each plugin exports **CLAP**
and **VST3**.

Five plugins ship today, all at `0.2.0`. What is being built next, and why in
that order, is in
[`docs/implementation/roadmap.md`](docs/implementation/roadmap.md).

## Plugins

| Plugin | What it is | Release |
|---|---|---|
| [NXE Doubler](plugins/doubler/docs/requirements/REQ-DBL.md) | Multi-voice doubler — 2/4/8 detuned, delayed, humanized voices from one source | [`doubler-v0.2.0`](https://github.com/narusenia/nxe-plugins/releases/tag/doubler-v0.2.0) |
| [NXE Velour](plugins/velour/docs/requirements/REQ-VEL.md) | Vocal presence saturator — three parallel harmonic generators added to an untouched dry path | [`velour-v0.2.0`](https://github.com/narusenia/nxe-plugins/releases/tag/velour-v0.2.0) |
| [NXE Sparkleur](plugins/sparkleur/docs/requirements/REQ-SPK.md) | Multiband dynamics with a transient-gated harmonic generator — five bands, upward and downward | [`sparkleur-v0.2.0`](https://github.com/narusenia/nxe-plugins/releases/tag/sparkleur-v0.2.0) |
| [NXE Air](plugins/air/docs/requirements/REQ-AIR.md) | Signal-driven texture generator — harmonics and noise placed where the source is not, following what it does | [`air-v0.2.0`](https://github.com/narusenia/nxe-plugins/releases/tag/air-v0.2.0) |
| [NXE Diorama](plugins/diorama/docs/requirements/REQ-DIO.md) | A vocal's distance as one knob — early reflections and a direct path that is itself processed, no reverb tail | [`diorama-v0.2.0`](https://github.com/narusenia/nxe-plugins/releases/tag/diorama-v0.2.0) |

**They are meant to be opened side by side.** One width, one typeface, one
accent per plugin from a family that varies only in hue, and the same shape in
every window: the figure on the accent, the controls under it, and a strip
along the bottom carrying the readings and a line describing whatever the
pointer is on.

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
zip per platform (`nxe-doubler-0.2.0-macos.zip`). Unpack it and copy the bundles
into:

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
