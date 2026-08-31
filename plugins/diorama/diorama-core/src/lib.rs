//! The Diorama DSP.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no OS audio API.
//! Takes a sample rate and plain values in, writes samples out.
//!
//! The algorithm is specified in
//! `plugins/diorama/docs/specifications/dsp.md` and built in units `DIO-1`
//! onward (`plugins/diorama/docs/implementation/diorama-plan.md`).

pub mod clarity;
pub mod damping;
pub mod depth;
pub mod direct;
pub mod engine;
pub mod reflections;
pub mod width;

pub use clarity::Clarity;
pub use damping::Damping;
pub use depth::Macros;
pub use direct::Direct;
pub use engine::Engine;
pub use reflections::Reflections;
pub use width::Width;
