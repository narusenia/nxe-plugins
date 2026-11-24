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
//! - [`nodes`] — where the reduction is allowed to go, as a curve the user draws
//! - [`display`] — the same curves on the figure's own logarithmic axis
//! - [`engine`] — the whole of it, and **every ear-tuned constant in one block**
//!
//! **The adaptive map is gone** (`PUM-10c`). It was the product's claim — a
//! long-term average deciding *where* resonance lives, so a singer's partials
//! would be left alone — and it does not work. Three statistics were measured
//! and none separates an intermittently-excited resonance from persistent
//! partials; the mean even puts them in the **wrong order**. The reasoning and
//! the numbers are in `REQ-PUM-003`, kept so that the next person to have the
//! idea starts from the measurements rather than from the idea.

pub mod display;
pub mod engine;
pub mod gain;
pub mod nodes;
pub mod quality;
pub mod reference;
pub mod smoothing;
pub mod stft;

pub use display::CURVE_POINTS;
pub use engine::{Controls, Curves, Engine, Settings};
pub use nodes::{NODES, Node, Range};
pub use quality::Quality;
pub use stft::{Frame, Stft};
