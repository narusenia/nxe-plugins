//! Signal analysis shared by the NXE plugins.
//!
//! **What a plugin shows about the sound going through it**, as opposed to what
//! it is set to — a level, where the energy sits across the stereo image, what
//! frequencies are present. None of it changes the audio.
//!
//! Host-agnostic and interface-agnostic by construction, the same way a
//! `<plugin>-core` crate is: this code runs on the audio thread, so it links no
//! host API, and it hands out plain numbers, so it links no widget toolkit
//! (`docs/specifications/architecture.md`).
//!
//! **Everything here is allocation-free once built.** Buffers are sized at
//! construction from the sample rate; nothing on the analysis path allocates,
//! locks, or blocks (`.agents/rules/rust.md`).

mod correlation;
mod handoff;
mod level;
mod pan;
mod spectrum;

pub use correlation::Correlation;
pub use handoff::Handoff;
pub use level::Level;
pub use pan::PanScope;
pub use spectrum::Spectrum;
