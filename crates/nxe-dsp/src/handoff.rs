//! Getting analysis values from the audio thread to the interface.
//!
//! **Latest wins, and a torn read is fine.** The reader is a redraw at 60 Hz and
//! the writer is a block at whatever rate the host runs; a frame that mixes two
//! writes shows one bin from the previous block, which nobody can see on a
//! decaying meter. Buying consistency would cost a lock or a triple buffer, and
//! the audio thread may not wait for either (`REQ-DBL-011`).
//!
//! `f32` through `AtomicU32::to_bits` rather than a float atomic: it is the same
//! instruction and it costs no dependency.

use std::sync::atomic::{AtomicU32, Ordering};

/// A fixed set of values written by the audio thread and read by the interface.
///
/// Shared as an `Arc<Handoff<N>>` — the plugin keeps one end, the editor the
/// other. Neither side owns it, so the editor closing does not disturb the
/// audio thread.
pub struct Handoff<const N: usize> {
    bins: [AtomicU32; N],
}

impl<const N: usize> Handoff<N> {
    pub fn new() -> Self {
        Self {
            bins: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }

    /// Publishes a whole frame. Called from the audio thread.
    pub fn write(&self, values: &[f32; N]) {
        for (bin, value) in self.bins.iter().zip(values) {
            bin.store(value.to_bits(), Ordering::Relaxed);
        }
    }

    /// Reads the latest frame. Called from the interface.
    pub fn read(&self) -> [f32; N] {
        std::array::from_fn(|index| f32::from_bits(self.bins[index].load(Ordering::Relaxed)))
    }
}

impl<const N: usize> Default for Handoff<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_comes_back_as_it_went_in() {
        let handoff: Handoff<4> = Handoff::new();
        handoff.write(&[0.0, 0.25, -1.5, f32::MAX]);
        assert_eq!(handoff.read(), [0.0, 0.25, -1.5, f32::MAX]);
    }

    #[test]
    fn it_starts_at_zero() {
        let handoff: Handoff<3> = Handoff::new();
        assert_eq!(handoff.read(), [0.0; 3]);
    }

    /// The point of the type: one end can be handed to another thread. If this
    /// stops compiling, something in it grew a `Cell` or an `Rc`.
    #[test]
    fn it_can_be_shared_across_threads() {
        fn assert_shareable<T: Send + Sync>() {}
        assert_shareable::<Handoff<8>>();
    }
}
