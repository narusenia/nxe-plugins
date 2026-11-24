//! The stereo picture: recent sample pairs for a goniometer, the balance and
//! the correlation.

use nxe_dsp::{Correlation, Level};

/// Every this many samples, one pair is kept for the picture.
const DECIMATION: usize = 2;

pub struct Stereo<const POINTS: usize> {
    lefts: [f32; POINTS],
    rights: [f32; POINTS],
    next: usize,
    skipped: usize,
    correlation: Correlation,
    left_level: Level,
    right_level: Level,
}

impl<const POINTS: usize> Stereo<POINTS> {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lefts: [0.0; POINTS],
            rights: [0.0; POINTS],
            next: 0,
            skipped: 0,
            correlation: Correlation::new(sample_rate),
            left_level: Level::new(sample_rate),
            right_level: Level::new(sample_rate),
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        self.correlation.push(left, right);
        self.left_level.push(left);
        self.right_level.push(right);
        self.skipped += 1;
        if self.skipped >= DECIMATION {
            self.skipped = 0;
            self.lefts[self.next] = left;
            self.rights[self.next] = right;
            self.next = (self.next + 1) % POINTS;
        }
    }

    pub fn lefts(&self) -> &[f32; POINTS] {
        &self.lefts
    }

    pub fn rights(&self) -> &[f32; POINTS] {
        &self.rights
    }

    /// −1 out of phase, 0 unrelated, +1 the same on both sides.
    pub fn correlation(&self) -> f32 {
        self.correlation.value()
    }

    /// −1 all left, +1 all right, 0 even or silent.
    pub fn balance(&self) -> f32 {
        let (left, right) = (
            self.left_level.rms().powi(2),
            self.right_level.rms().powi(2),
        );
        if left + right < 1e-12 {
            0.0
        } else {
            (right - left) / (right + left)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn a_signal_on_the_left_leans_left() {
        let mut stereo: Stereo<64> = Stereo::new(48_000.0);
        for index in 0..48_000 {
            let sample = (TAU * 440.0 * index as f32 / 48_000.0).sin();
            stereo.push(sample, sample * 0.1);
        }
        assert!(stereo.balance() < -0.9, "{}", stereo.balance());
        assert!(stereo.correlation() > 0.9);
    }
}
