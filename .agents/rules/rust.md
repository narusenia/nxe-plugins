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

## Block size

**Do not use nih-plug's block-based APIs** (`iter_blocks`, `Smoother::next_block`,
or reading a parameter once per buffer) without revisiting `REQ-DBL-012` first.
The plugins guarantee that the output does not depend on the host's block size,
and today that holds for a structural reason rather than a tested one: `process`
is a plain per-sample loop, every smoother is polled exactly once per sample,
and the DSP holds no per-block state. A block-based read would break the
guarantee quietly, in a way only a host with an unusual buffer size would show.

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

## Things that cost time here

**Assert on a constant at compile time, not in a test.** clippy rejects
`assert!` on a constant expression, and it is right: `const _: () =
assert!(...)` next to the constant fails the build instead of a test run nobody
had to execute. The radius guards in `nxe_ui::theme` are the example.

**A string replacement against a formatted file silently does nothing.**
Editing by matching source text you wrote earlier fails the moment `rustfmt` has
reflowed it — and unlike a bad edit, a no-op edit still compiles, so the only
symptom is that the behaviour never changed. This has already produced one
"fixed" claim that was not fixed and one feature that was never wired up. After
any edit that is supposed to change behaviour, **grep the file for the new text
before believing it landed.**

**Check the exit code, not the output.** `cargo clippy ... | tail` reports
`tail`'s status, so a `&&` chain after it runs even though clippy failed. This
has already put unformatted and lint-failing code into a commit twice. Run the
three checks so their status is visible:

```bash
cargo fmt --all -- --check; cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace
```

**A pure function is the testable part.** Interaction, drawing and hosting can
only be judged by looking, but the arithmetic underneath cannot. Every widget
and DSP block here splits the arithmetic out — `Drag::value_after`,
`Bar::span`, `Geometry::position`, `DelayLine::read` — and that is where the
bugs were actually caught.

**A doubled or mirrored value is where the sign error lives.** The bipolar bar
growing the wrong way, the polar field's angle winding backwards, the delay
line's index direction: each of these was a test that failed once and then never
again. Write that test before believing the drawing.

## Dependencies

- Adding a dependency needs a reason written down in the plan that adds it.
  Prefer a few lines of arithmetic over a crate for a filter, a window, or an
  interpolator — those are the parts worth owning.
- No FFT crate unless a requirement actually needs the frequency domain. The
  current plugins do not.
