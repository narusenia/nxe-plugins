//! The Parasite DSP (`plugins/parasite/docs/implementation/parasite-plan.md`).
//!
//! Host-agnostic and allocation-free on the audio path. `pitch` is the first
//! block of the Harmonic engine and is written not to know what Parasite is;
//! `engine` is Parasite itself.

pub mod engine;
pub mod pitch;

pub use engine::{Parasite, Readings, SCOPE_POINTS, Settings, Species};
