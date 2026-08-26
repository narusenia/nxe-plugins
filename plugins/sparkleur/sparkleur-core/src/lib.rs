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

pub mod character;
pub mod crossover;
pub mod detector;
pub mod dynamics;
pub mod engine;
pub mod protect;
pub mod sparkle;

pub use crossover::{BAND_COUNT, Crossover};
pub use detector::Detector;
// `dynamics::Settings` and `sparkle::Settings` are different things, so
// neither is re-exported here — the module name is what tells them apart.
pub use character::Character;
pub use dynamics::{Curve, Weights};
pub use engine::{Engine, Levels, Shape};
pub use protect::DeHarsh;
pub use sparkle::Sparkle;
