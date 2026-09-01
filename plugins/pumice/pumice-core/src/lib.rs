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
//! - [`smoothing`] — averaging a spectrum over a width in octaves
//! - [`reference`] — what a bin is judged against, and by how much it exceeds it
//! - [`gain`] — from that excess to a gain, and the floor that stops warbling
//! - [`engine`] — the whole of it, and **every ear-tuned constant in one block**
//!
//! **`STATIC` only** (`PUM-3`). The long-term map that decides *where*
//! resonance lives — the half that makes this "the vocal soothe" rather than
//! "a soothe" — is `PUM-4`.

pub mod engine;
pub mod gain;
pub mod quality;
pub mod reference;
pub mod smoothing;
pub mod stft;

pub use engine::{Controls, Engine, Settings};
pub use quality::Quality;
pub use stft::{Frame, Stft};
