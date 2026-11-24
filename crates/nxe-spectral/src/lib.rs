//! The overlap-add STFT every spectral NXE plugin runs inside, and the
//! transform sizes that keep the bin width put across sample rates.
//!
//! Written inside `pumice-core` and moved here when the Spectral Kit became
//! its second caller (`plugins/kit/docs/implementation/spectral-kit-plan.md`).
//! The `REQ-PUM-*` references in these modules are why the code has the shape
//! it has, not stale links. Host-agnostic and allocation-free once built.

pub mod quality;
pub mod stft;

pub use quality::Quality;
pub use realfft::num_complex::Complex;
pub use stft::{Frame, Stft};
