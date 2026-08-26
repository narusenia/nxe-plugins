//! The Sparkleur DSP: five bands of up-and-down dynamics with a
//! transient-gated harmonic generator on the top one.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-SPK-015`). Everything on the audio path is allocation-free once built.
//!
//! The design is in `plugins/sparkleur/docs/specifications/dsp.md`; the units
//! it is built in are in
//! `plugins/sparkleur/docs/implementation/sparkleur-plan.md`. Blocks shared
//! with Velour live in [`nxe_audio`].

pub mod crossover;
pub mod detector;

pub use crossover::{BAND_COUNT, Crossover};
pub use detector::Detector;
