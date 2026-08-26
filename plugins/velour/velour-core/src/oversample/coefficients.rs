//! Halfband coefficients for the oversampler. **Generated.**
//!
//! Regenerate with `mise run oversampler:design`. The reasoning behind the
//! stopband edges and the section counts is in
//! `scripts/design-oversampler.py`; what the numbers actually achieve is
//! measured by the tests in `super`.

/// The 48 kHz <-> 96 kHz halfband: **78.5 dB** in the stopband
/// (from 0.5833 of pi), passband ripple 6.1e-08 dB.
pub const STAGE_ONE: [f32; 7] = [
    0.029633068,
    0.062934965,
    0.108832896,
    0.23936339,
    0.43403667,
    0.6497191,
    0.8756988,
];

/// The 96 kHz <-> 192 kHz halfband: **78.6 dB** in the stopband
/// (from 0.7083 of pi), passband ripple 6.0e-08 dB.
pub const STAGE_TWO: [f32; 3] = [
    0.079232104,
    0.3097851,
    0.7050574,
];
