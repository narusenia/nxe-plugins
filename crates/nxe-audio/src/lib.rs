//! The processing every NXE plugin shares: the harmonic curve, the
//! oversampler, biquads, envelope followers and the relative guard.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API.
//! Everything on the audio path is allocation-free once built.
//!
//! ## Why this is not `nxe-dsp`
//!
//! Both crates are host-agnostic, allocation-free and run on the audio thread.
//! They are separate because **the risk is a different class**: a bug in
//! `nxe-dsp` breaks a picture, a bug here breaks the sound
//! (`docs/specifications/architecture.md`). Putting them in one crate would
//! erase that difference and make "which one does this go in" a question with
//! no answer.
//!
//! ## Where this came from
//!
//! Every module here was written inside `velour-core` and moved out when
//! Sparkleur asked for it (`REQ-SPK-015`, `SPK-1`). That is the rule the
//! architecture states — a shared crate is created by the second caller, not
//! in anticipation of one — so the `REQ-VEL-*` references in these modules are
//! not stale: they are the reason the code has the shape it has.

pub mod biquad;
pub mod harmonics;
pub mod oversample;
pub mod shaper;

pub use oversample::{Factor, Oversampler};
pub use shaper::Shaper;
