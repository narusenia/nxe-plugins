//! The nonlinear engines behind the Design Kits: Fold, Fever and Buckle
//! (`plugins/kit/docs/implementation/nonlinear-kit-plan.md`).
//!
//! Each is a curve and an engine. The curve is a plain function, so the
//! window draws exactly what the audio runs. The engine runs the curve 4x
//! oversampled with the mix inside the oversampled loop, so the dry signal
//! goes through the same filters as the wet one and a half mix does not comb
//! (decision 5), then takes the DC out (decision 6).
//!
//! Host-agnostic and allocation-free once built. Devour takes these over when
//! it comes; until a second caller asks, they stay out of `nxe-audio`.

pub mod buckle;
pub mod fever;
pub mod fold;

pub use buckle::Buckle;
pub use fever::Fever;
pub use fold::Fold;

use nxe_audio::{Factor, Oversampler};

/// The oversampling every engine runs at.
pub const OVERSAMPLE: usize = 4;

/// Where the DC blocker turns over.
const DC_HZ: f32 = 10.0;

/// A first-order highpass at [`DC_HZ`].
#[derive(Clone, Debug)]
struct DcBlocker {
    pole: f32,
    previous_in: f32,
    previous_out: f32,
}

impl DcBlocker {
    fn new(sample_rate: f32) -> Self {
        Self {
            pole: (-std::f32::consts::TAU * DC_HZ / sample_rate).exp(),
            previous_in: 0.0,
            previous_out: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = input - self.previous_in + self.pole * self.previous_out;
        self.previous_in = input;
        self.previous_out = output;
        output
    }
}

/// One channel's oversampler and DC blocker.
#[derive(Clone)]
struct Channel {
    oversampler: Oversampler,
    dc: DcBlocker,
}

impl Channel {
    fn new(sample_rate: f32) -> Self {
        let mut oversampler = Oversampler::new();
        oversampler.set_factor(Factor::Four);
        Self {
            oversampler,
            dc: DcBlocker::new(sample_rate),
        }
    }

    /// `body` sees the oversampled signal and returns it mixed.
    fn process(&mut self, input: f32, body: impl FnMut(f32) -> f32) -> f32 {
        self.dc.process(self.oversampler.process(input, body))
    }
}

/// `dry` moved toward `wet` by `mix`.
fn mix(dry: f32, wet: f32, mix: f32) -> f32 {
    dry + mix * (wet - dry)
}

#[cfg(test)]
pub(crate) mod test_signal {
    /// A sine at `hz`, `amplitude` peak.
    pub fn sine(index: usize, hz: f32, amplitude: f32, sample_rate: f32) -> f32 {
        (std::f32::consts::TAU * hz * index as f32 / sample_rate).sin() * amplitude
    }

    pub fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dc_blocker_removes_an_offset() {
        let mut dc = DcBlocker::new(48_000.0);
        let mut last = 1.0;
        for _ in 0..48_000 {
            last = dc.process(0.5);
        }
        assert!(last.abs() < 1e-3, "{last}");
    }
}
