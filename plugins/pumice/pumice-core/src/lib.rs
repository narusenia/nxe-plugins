//! The Pumice DSP: dynamic resonance suppression for a single vocal.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no host API
//! (`REQ-PUM-015`). Everything on the audio path is allocation-free once built.
//!
//! **The gate has passed** (`PUM-1`, 2026-09-01): Ableton Live in VST3 and
//! Studio One Pro in CLAP both compensate for the reported latency, and both
//! recover from a `QUALITY` change while the transport runs. The FFT approach
//! is settled.
//!
//! What is here:
//!
//! - [`quality`] — how big the transform is, and therefore how much latency
//! - [`stft`] — the overlap-add buffering the transform runs inside
//!
//! **No detection and no gain yet** (`PUM-3` onward). [`Stft`] hands a caller
//! a frame of bins and puts back whatever it finds there, so the reconstruction
//! can be measured before anything is asked to change it.

pub mod quality;
pub mod stft;

pub use quality::Quality;
pub use stft::{Frame, Stft};
