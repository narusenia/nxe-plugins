//! The Doubler DSP.
//!
//! Host-agnostic by construction — no nih-plug, no Vizia, no OS audio API.
//! Takes a sample rate and plain values in, writes samples out.
//!
//! The algorithm is specified in `plugins/doubler/docs/specifications/dsp.md`
//! and built in units `DBL-1` onward
//! (`plugins/doubler/docs/implementation/doubler-plan.md`).

mod delay;
mod shifter;
mod voice;
mod wobble;

pub use delay::DelayLine;
pub use shifter::PitchShifter;
pub use voice::{DEFAULT_SHAPE, MAX_VOICES, Macros, VoiceEngine, VoiceShape, Voices};
