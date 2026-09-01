//! The Pumice DSP: dynamic resonance suppression for a single vocal.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-PUM-015`). Everything on the audio path is allocation-free once built.
//!
//! **Nothing here processes audio yet.** `PUM-1` is a gate: the plugin reports
//! latency and delays by exactly that much, and four DAWs have to compensate
//! for it before a single line of the engine is worth writing
//! (`../docs/implementation/pumice-plan.md`). What lives here now is the
//! arithmetic that decides *how much* latency — the transform size — because
//! that is a property of the engine rather than of the wrapper, and
//! `.agents/rules/rust.md` keeps DSP out of the wrapper.

pub mod quality;

pub use quality::Quality;
