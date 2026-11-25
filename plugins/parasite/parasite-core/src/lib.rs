//! The Parasite DSP (`plugins/parasite/docs/implementation/parasite-plan.md`).
//!
//! Host-agnostic and allocation-free on the audio path. Its pitch tracker
//! started here and moved to `nxe_audio::pitch` when Dew asked for it.

pub mod engine;

pub use engine::{Parasite, Readings, SCOPE_POINTS, Settings, Species};
