//! The Vocal Depth DSP.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no OS audio API.
//! Takes a sample rate and plain values in, writes samples out.
//!
//! The algorithm is specified in
//! `plugins/vocal-depth/docs/specifications/dsp.md` and built in units `VDP-1`
//! onward (`plugins/vocal-depth/docs/implementation/vocal-depth-plan.md`).

pub mod direct;
pub mod reflections;

pub use direct::Direct;
pub use reflections::Reflections;
