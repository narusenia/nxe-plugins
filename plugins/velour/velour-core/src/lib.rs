//! The Velour DSP: three parallel harmonic generators added to an untouched
//! dry path.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-VEL-015`). Everything on the audio path is allocation-free once built.
//!
//! The design is in `plugins/velour/docs/specifications/dsp.md`; the units it
//! is built in are in `plugins/velour/docs/implementation/velour-plan.md`.
//!
//! The curve, the oversampler and the biquads used to live here. Sparkleur
//! asked for them, so they are in [`nxe_audio`] now (`SPK-1`,
//! `docs/specifications/architecture.md`).

pub mod bands;
pub mod density;
pub mod emotion;
pub mod engine;
pub mod envelope;
pub mod guard;
pub mod texture;

/// The measurement helpers the tests and benches here are written against.
/// Re-exported rather than imported everywhere because they moved with the
/// modules they were written for (`SPK-1`).
pub use nxe_audio::harmonics;

pub use bands::{BANDS, Band, Generator};
pub use density::Density;
pub use engine::{BAND_COUNT, Engine, Levels, Shape};
pub use envelope::Envelope;
pub use guard::{Guarded, Guards};
