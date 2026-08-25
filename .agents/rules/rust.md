---
paths:
  - "crates/**/*.rs"
  - "plugins/**/*.rs"
  - "**/Cargo.toml"
---

# Rust and real-time audio rules

## The audio thread

- **Allocate nothing in `process()`.** Every buffer — delay lines, scratch
  vectors, per-voice state — is sized and allocated in
  `Plugin::initialize()`, where the maximum block size and sample rate are
  known. A `Vec::push`, a `Box::new`, a `format!`, or a `collect()` on the
  audio path is a bug, not a style preference.
- **No locks, no channels that block, no file or network I/O** on the audio
  path. GUI → audio communication goes through parameters (nih-plug already
  makes those lock-free) or an atomic; audio → GUI goes through atomics or a
  ring buffer that never blocks the writer.
- **No panics.** Index with the length you allocated, not with a value derived
  from a parameter. A parameter is host-controlled input: clamp it.
- Denormals: keep filter and delay feedback paths from decaying into denormal
  territory (add a tiny DC offset or flush) rather than relying on the host to
  have set the CPU flag.

## Crate boundaries

- A `<plugin>-core` crate **must not depend on nih-plug, Vizia, or any host
  API**. It takes sample rate and plain values in, and writes samples out. This
  is what makes an AU wrapper possible later, and it is also what makes the DSP
  testable without a host.
- `nxe-ui` **must not depend on nih-plug**. Widgets take a value and a
  callback. The plugin crate owns the adapter that binds them to its
  parameters.
- Dependency direction is one way: `plugin` → (`plugin-core`, `nxe-ui`).
  Nothing points back.

## Tests

- Non-trivial DSP leaves one runnable check behind: the smallest test that
  fails if the logic breaks. An impulse through a delay line landing at the
  expected sample, a sine through the pitch shifter coming out at the expected
  ratio, wet RMS staying constant as the voice count changes.
- No test frameworks beyond `#[test]`. `criterion` is allowed for benchmarks
  that back a stated CPU budget, and only for those.

## Dependencies

- Adding a dependency needs a reason written down in the plan that adds it.
  Prefer a few lines of arithmetic over a crate for a filter, a window, or an
  interpolator — those are the parts worth owning.
- No FFT crate unless a requirement actually needs the frequency domain. The
  current plugins do not.
