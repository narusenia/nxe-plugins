//! The Air DSP: a high-frequency texture generated from the input and placed
//! where the input is not.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-AIR-015`). Everything on the audio path is allocation-free once built.
//!
//! The design is in `plugins/air/docs/specifications/dsp.md`; the units it is
//! built in are in `plugins/air/docs/implementation/air-plan.md`. Blocks shared
//! with Velour and Sparkleur live in [`nxe_audio`].

pub mod harmonic;
pub mod noise;

pub use harmonic::Harmonic;
pub use noise::Noise;
