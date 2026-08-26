//! The Velour DSP: three parallel harmonic generators added to an untouched
//! dry path.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-VEL-015`). Everything on the audio path is allocation-free once built.
//!
//! The design is in `plugins/velour/docs/specifications/dsp.md`; the units it
//! is built in are in `plugins/velour/docs/implementation/velour-plan.md`.
//!
//! **Modules marked "move candidate" are written not to know anything about
//! Velour**, because Sparkleur will want them. They stay here until it does
//! (`docs/specifications/architecture.md`).

pub mod bands;
pub mod biquad;
pub mod emotion;
pub mod engine;
pub mod envelope;
pub mod guard;
pub mod harmonics;
pub mod oversample;
pub mod shaper;
pub mod texture;

pub use bands::{BANDS, Band, Generator};
pub use engine::{BAND_COUNT, Engine, Levels, Shape};
pub use envelope::Envelope;
pub use guard::{Guarded, Guards};
pub use oversample::{Factor, Oversampler};
pub use shaper::Shaper;
