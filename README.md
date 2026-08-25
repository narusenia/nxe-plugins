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
| [NXE Doubler](plugins/doubler/docs/requirements/REQ-DBL.md) | Multi-voice doubler — 2/4/8 detuned, delayed, humanized voices from one source | Design |

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
mise run check              # fmt + clippy + tests
mise run bundle doubler     # build the CLAP and VST3 bundles
mise run install doubler    # bundle, then install into the user plugin folders
mise run gallery            # run the shared UI widget gallery as a desktop app
```

## Installing a release build

Releases attach a zip per platform. Unpack it and copy the bundles into:

| | CLAP | VST3 |
|---|---|---|
| macOS | `~/Library/Audio/Plug-Ins/CLAP` | `~/Library/Audio/Plug-Ins/VST3` |
| Windows | `%COMMONPROGRAMFILES%\CLAP` | `%COMMONPROGRAMFILES%\VST3` |
| Linux | `~/.clap` | `~/.vst3` |

**macOS builds are not signed or notarized.** The first time you load one,
Gatekeeper will refuse it. Clear the quarantine attribute:

```bash
xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/'NXE Doubler.clap'
```

## License

GPL-3.0. nih-plug's VST3 wrapper derives from the Steinberg VST3 SDK, which is
GPLv3, so anything linking it is too. See [LICENSE](LICENSE).
