//! The one envelope detector `EMOTION` and `DENSITY` share, and where it is
//! centred.
//!
//! The follower itself is [`nxe_audio::envelope`]; what is Velour about it is
//! the pair of time constants and [`REFERENCE_DB`].
//!
//! ## Why one
//!
//! `EMOTION` moves the *quality* of the harmonics with how hard the singer is
//! working; `DENSITY` levels their *quantity*. Both are answers to the same
//! question — how loud is this phrase — and two detectors with two sets of time
//! constants would answer it differently, which is how the two controls would
//! start fighting (`REQ-VEL-008`).
//!
//! ## Why the input, before the compressor
//!
//! `DENSITY`'s compressor sits on the generator bus, so if the detector read
//! the compressor's output it would read a signal `DENSITY` had already
//! flattened — and `EMOTION` would stop reacting as `DENSITY` came up. Reading
//! the input keeps the two orthogonal, and it is **the only point where that
//! collision is resolved** (`REQ-VEL-008`).

use nxe_audio::envelope::Envelope;

/// Fast enough to be inside the first note of a phrase, and **slow enough on
/// the way down not to fall back between syllables**: a single consonant must
/// not be able to move the character, or `EMOTION` reads as chatter rather than
/// as expression (`dsp.md`).
const ATTACK_SECONDS: f32 = 0.005;
const RELEASE_SECONDS: f32 = 0.150;

/// Where a vocal sits when it is not being pushed, in dBFS.
///
/// **Both consumers measure from here** — `EMOTION` centres its axis on it
/// (`crate::emotion`) and `DENSITY` references its makeup to it
/// (`crate::density`) — so it lives with the detector rather than in either of
/// them. One number to settle by ear (`VEL-17`) instead of two that could drift
/// apart.
pub const REFERENCE_DB: f32 = -18.0;

/// The follower both consumers read, tuned for a sung line.
pub fn vocal(sample_rate: f32) -> Envelope {
    Envelope::new(ATTACK_SECONDS, RELEASE_SECONDS, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// **The reason the release is 150 ms.** It has to hold across a gap
    /// between syllables, or `EMOTION` reads consonants rather than phrases.
    #[test]
    fn it_holds_through_a_gap() {
        let mut envelope = vocal(RATE);
        for _ in 0..(0.2 * RATE) as usize {
            envelope.push(1.0);
        }
        // 50 ms of nothing, which is a long consonant.
        for _ in 0..(0.05 * RATE) as usize {
            envelope.push(0.0);
        }
        let held = envelope.decibels();
        assert!(held > -3.0, "a 50 ms gap dropped it to {held:.2} dB");
    }
}
