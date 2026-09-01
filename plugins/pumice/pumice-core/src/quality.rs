//! How big the transform is, and therefore how much latency the plugin costs.
//!
//! **The size follows the sample rate so that the *bin width* stays put**
//! (`REQ-PUM-017`). A fixed sample count would halve the resolution at 96 kHz,
//! and `SHARPNESS` — which is a width in octaves measured against those bins —
//! would quietly mean something different on a session opened at another rate.
//!
//! ## Why the targets are 48 kHz's own bin widths
//!
//! `46.875`, `23.4375` and `11.71875` are `48000 / 1024`, `/ 2048` and
//! `/ 4096`. Choosing them means the common rate lands on a power of two
//! exactly and every other rate rounds toward it, rather than every rate being
//! slightly wrong including the one almost everyone uses.
//!
//! **44.1 kHz is the one that pays**: all three steps come out 8.1 % below
//! target (`NORMAL` is 21.5 Hz, not 23.4). That is inside the ±10 % the
//! requirement allows, and the alternative — centring the targets between the
//! two rates — moves the error onto 48 kHz instead of removing it.

/// The largest transform this crate will ever build: 192 kHz at [`Quality::High`].
///
/// **Buffers are allocated for this and never for anything else**
/// (`REQ-PUM-008`). Changing `QUALITY` at run time then changes how much of a
/// buffer is used, not how big it is, which is what keeps the allocation out of
/// `process()`.
pub const MAX_BLOCK: usize = 16_384;

/// Smallest transform, so a nonsense sample rate cannot produce a degenerate
/// one. Well below anything a host offers.
pub const MIN_BLOCK: usize = 256;

/// Hop is `block / OVERLAP` — **75 % overlap**.
///
/// A time-varying gain is applied per block, and at 50 % the seam between
/// windows is audible on a gain that moved between them. This is the reason the
/// number is 4 and not 2; it is not a resolution setting.
pub const OVERLAP: usize = 4;

/// The three steps a user can choose between (`REQ-PUM-008`).
///
/// **Not automatable in the wrapper.** Each step reports a different latency,
/// and a host asked to redo delay compensation at every automation point would
/// glitch at every automation point.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    Low,
    #[default]
    Normal,
    High,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Normal, Quality::High];

    /// The bin width this step aims for, in Hz.
    pub const fn target_bin_hz(self) -> f32 {
        match self {
            Quality::Low => 46.875,
            Quality::Normal => 23.437_5,
            Quality::High => 11.718_75,
        }
    }

    /// The transform size at this rate: the power of two nearest to
    /// `sample_rate / target`.
    ///
    /// **Nearest, not next-above.** Next-above would push 96 kHz `NORMAL` from
    /// 4096 to 8192 — the ratio is 4103, three bins past the boundary — and
    /// double the latency for nothing.
    pub fn block(self, sample_rate: f32) -> usize {
        if !sample_rate.is_finite() || sample_rate <= 0.0 {
            return MIN_BLOCK;
        }

        let ideal = sample_rate / self.target_bin_hz();
        let exponent = ideal.log2().round().clamp(0.0, 31.0) as u32;
        (1usize << exponent).clamp(MIN_BLOCK, MAX_BLOCK)
    }

    pub fn hop(self, sample_rate: f32) -> usize {
        self.block(sample_rate) / OVERLAP
    }

    /// What the plugin reports to the host, in samples.
    ///
    /// **`block − hop`, not `block`.** An overlap-add that hands back the
    /// finished part of the ring as soon as it is finished owes the host three
    /// quarters of a window, not a whole one. `nih_plug`'s `StftHelper` reports
    /// the whole one; writing the buffering here rather than borrowing it
    /// (`REQ-PUM-015`) is worth 10 ms because of this line.
    pub fn latency(self, sample_rate: f32) -> usize {
        let block = self.block(sample_rate);
        block - block / OVERLAP
    }
}

/// The largest latency any step reports at this rate.
///
/// What the dry delay line is sized for, so that changing `QUALITY` never
/// allocates (`REQ-PUM-008`).
pub fn max_latency(sample_rate: f32) -> usize {
    Quality::ALL
        .iter()
        .map(|quality| quality.latency(sample_rate))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `dsp.md`'s table, fixed here so that a mistake in the formula fails a
    /// test rather than shipping as a different-sounding plugin at 96 kHz.
    #[test]
    fn the_block_table_holds() {
        let expected = [
            //          44.1k  48k    96k    192k
            (Quality::Low, [1024, 1024, 2048, 4096]),
            (Quality::Normal, [2048, 2048, 4096, 8192]),
            (Quality::High, [4096, 4096, 8192, 16384]),
        ];
        let rates = [44_100.0, 48_000.0, 96_000.0, 192_000.0];

        for (quality, blocks) in expected {
            for (rate, block) in rates.iter().zip(blocks) {
                assert_eq!(quality.block(*rate), block, "{quality:?} at {rate} Hz");
            }
        }
    }

    /// `REQ-PUM-008`: every rate and step lands within 10 % of its target.
    #[test]
    fn every_bin_width_is_within_ten_percent() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            for quality in Quality::ALL {
                let bin = rate / quality.block(rate) as f32;
                let error = (bin - quality.target_bin_hz()).abs() / quality.target_bin_hz();
                assert!(
                    error <= 0.10,
                    "{quality:?} at {rate} Hz: {bin} Hz is {:.1} % off",
                    error * 100.0
                );
            }
        }
    }

    #[test]
    fn latency_is_three_quarters_of_the_block() {
        assert_eq!(Quality::Normal.latency(48_000.0), 1536);
        assert_eq!(Quality::Low.latency(48_000.0), 768);
        assert_eq!(Quality::High.latency(48_000.0), 3072);
    }

    /// Nothing may ask for a buffer bigger than the one that gets allocated.
    #[test]
    fn no_step_at_any_rate_exceeds_the_maximum() {
        for rate in [8_000.0, 44_100.0, 48_000.0, 96_000.0, 192_000.0, 384_000.0] {
            for quality in Quality::ALL {
                assert!(quality.block(rate) <= MAX_BLOCK, "{quality:?} at {rate}");
                assert!(quality.block(rate) >= MIN_BLOCK, "{quality:?} at {rate}");
            }
        }
    }

    /// A host that reports nonsense must not produce a degenerate transform —
    /// parameters and buffer configs are host input (`.agents/rules/rust.md`).
    #[test]
    fn a_nonsense_rate_does_not_panic() {
        for rate in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            for quality in Quality::ALL {
                assert!(quality.block(rate) >= MIN_BLOCK);
            }
        }
    }

    #[test]
    fn max_latency_is_the_high_step() {
        for rate in [44_100.0, 48_000.0, 96_000.0, 192_000.0] {
            assert_eq!(max_latency(rate), Quality::High.latency(rate));
        }
    }
}
