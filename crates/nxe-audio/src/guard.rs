//! Pulling something back when the band it feeds is already too loud, measured
//! **against a wider reference band** rather than against a threshold in dBFS.
//!
//! Velour's guards hold its presence and air generators (`velour_core::guard`);
//! Sparkleur's De-Harsh is the same block with one band (`REQ-SPK-007`). What
//! is shared is the mechanism, and nothing here decides where to listen or how
//! hard to pull — [`Settings`] does, and it comes from the caller.
//!
//! ## Relative, not absolute
//!
//! An absolute detector fires on every loud moment — which is a loud note, not
//! a harsh one — and suppressing those fights whatever else the product does
//! with dynamics. The useful side effect is that **it does not depend on input
//! gain**: the numerator and the denominator move together, so the ratio does
//! not.
//!
//! ## One reference, N bands
//!
//! The reference follower is shared. Giving each band its own would run the
//! same band-pass over the same signal N times for the same answer, and the
//! filter count is the part of this that costs (`VEL-16`).
//!
//! ## What it moves
//!
//! A gain the caller applies, and nothing else. Moving a curve's drive or bias
//! instead would make the band change character as it faded, and then nobody
//! could tell what had happened. It is also what separates this from a
//! de-esser: **the level is not reduced, whatever the gain feeds is.**

use crate::biquad::BandPass;

/// Below this the reference is treated as silence and no guard fires. Without
/// it, the ratio of two numbers made of noise floor decides whether the plugin
/// is allowed to work.
const FLOOR: f32 = 1e-10;

/// `10·log10(x)` is `10/log2(10)` times `log2(x)`.
const DECIBELS_PER_OCTAVE_POWER: f32 = 3.010_3;
/// And a gain in dB is `2^(dB / (20·log10(2)))`.
const DECIBELS_PER_OCTAVE_AMPLITUDE: f32 = 6.020_6;

/// A frequency range a follower watches.
#[derive(Clone, Copy, Debug)]
pub struct Band {
    pub low_hz: f32,
    pub high_hz: f32,
}

/// A guarded band: where it listens, and how far above the reference it may sit
/// before the guard starts pulling.
#[derive(Clone, Copy, Debug)]
pub struct GuardedBand {
    pub band: Band,
    /// A **band-to-reference power ratio**, not a level. This is the number
    /// that is settled by ear.
    pub threshold_db: f32,
}

/// Everything about a guard that is the product's rather than the mechanism's.
///
/// A `const` at the call site, so what a plugin guards and how hard reads as
/// one block instead of as five arguments.
#[derive(Clone, Copy, Debug)]
pub struct Settings<const N: usize> {
    /// Wide enough to stand for "the sound", narrow enough that rumble and air
    /// do not swamp it.
    pub reference: Band,
    pub bands: [GuardedBand; N],
    /// Fast enough to catch one event, slow enough not to chatter — a guard
    /// that opens and closes inside a syllable would be heard as the thing it
    /// is protecting against.
    pub attack_seconds: f32,
    pub release_seconds: f32,
    /// How many dB of reduction per dB of excess.
    pub slope: f32,
    /// The widest it may pull, in dB. Past this the band is gone rather than
    /// protected.
    pub max_reduction_db: f32,
}

/// A band-passed energy follower.
struct Follower {
    band: BandPass,
    power: crate::envelope::Power,
}

impl Follower {
    fn new(band: Band, attack_seconds: f32, release_seconds: f32, sample_rate: f32) -> Self {
        Self {
            band: BandPass::new(band.low_hz, band.high_hz, sample_rate),
            power: crate::envelope::Power::new(attack_seconds, release_seconds, sample_rate),
        }
    }

    /// **Energy, not amplitude.** The instantaneous ratio of two amplitudes
    /// swings from end to end on almost any material; power holds still enough
    /// to threshold (`nxe_dsp::PanScope` reached the same conclusion).
    fn push(&mut self, input: f32) -> f32 {
        let filtered = self.band.process(input);
        self.power.push(filtered * filtered)
    }

    fn reset(&mut self) {
        self.band.reset();
        self.power.reset();
    }
}

/// `N` guarded bands and the reference they share.
pub struct RelativeGuard<const N: usize> {
    reference: Follower,
    bands: [Follower; N],
    /// Linear gains, in the order of [`Settings::bands`]. `1.0` is "not holding
    /// back".
    gains: [f32; N],
    /// Stored as linear power ratios so the common case — nothing to do — is
    /// one multiplication and a comparison.
    thresholds: [f32; N],
    slope: f32,
    max_reduction_db: f32,
}

impl<const N: usize> RelativeGuard<N> {
    pub fn new(settings: Settings<N>, sample_rate: f32) -> Self {
        let follower = |band| {
            Follower::new(
                band,
                settings.attack_seconds,
                settings.release_seconds,
                sample_rate,
            )
        };
        Self {
            reference: follower(settings.reference),
            bands: settings.bands.map(|guarded| follower(guarded.band)),
            gains: [1.0; N],
            thresholds: settings
                .bands
                .map(|guarded| power_ratio(guarded.threshold_db)),
            slope: settings.slope,
            max_reduction_db: settings.max_reduction_db,
        }
    }

    /// Feeds one sample and updates the gains. **Host rate**, on the mono sum.
    ///
    /// The sum rather than each channel separately: a guard that fired on one
    /// side only would throw the source sideways. It also halves the filter
    /// count. The cost is that anti-phase content between the channels reads
    /// quieter than it is — acceptable for a source that lives in the middle.
    pub fn push(&mut self, mono: f32, amounts: [f32; N]) {
        let reference = self.reference.push(mono);

        for (index, amount) in amounts.into_iter().enumerate() {
            let energy = self.bands[index].push(mono);
            self.gains[index] = gain_of(
                energy,
                reference,
                self.thresholds[index],
                amount,
                self.slope,
                self.max_reduction_db,
            );
        }
    }

    /// The gain whatever the band guards should be multiplied by.
    pub fn gain(&self, index: usize) -> f32 {
        self.gains[index]
    }

    /// How far one guard is pulling, in dB — for a display. Zero when it is
    /// doing nothing.
    pub fn reduction_db(&self, index: usize) -> f32 {
        let gain = self.gain(index);
        if gain >= 1.0 {
            0.0
        } else {
            20.0 * gain.log10()
        }
    }

    pub fn reset(&mut self) {
        self.reference.reset();
        for band in &mut self.bands {
            band.reset();
        }
        self.gains = [1.0; N];
    }
}

/// A power ratio in dB as a plain ratio.
pub fn power_ratio(decibels: f32) -> f32 {
    10.0f32.powf(decibels / 10.0)
}

/// The gain one guard should apply.
///
/// Pure, so the one thing that matters — that it is flat until the ratio crosses
/// the threshold, and that it never inverts — is testable without running a
/// signal.
pub fn gain_of(
    band_energy: f32,
    reference_energy: f32,
    threshold: f32,
    amount: f32,
    slope: f32,
    max_reduction_db: f32,
) -> f32 {
    let amount = if amount.is_finite() {
        amount.clamp(0.0, 1.0)
    } else {
        0.0
    };

    // `amount == 0` has to be *exactly* off, not nearly: a guard nobody asked
    // for must not colour the sound.
    if amount == 0.0 || reference_energy < FLOOR || !band_energy.is_finite() {
        return 1.0;
    }

    // The common case is nothing to do, and it costs one multiplication.
    let allowed = reference_energy * threshold;
    if band_energy <= allowed {
        return 1.0;
    }

    // `log2`/`exp2` rather than `log10`/`powf`: same answer, cheaper, and only
    // reached while a guard is actually firing.
    let excess_db = DECIBELS_PER_OCTAVE_POWER * (band_energy / allowed).log2();
    let reduction_db = (amount * slope * excess_db).min(max_reduction_db);
    (-reduction_db / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmonics::tone;

    const RATE: f32 = 48_000.0;
    const SLOPE: f32 = 1.0;
    const MAX_REDUCTION_DB: f32 = 18.0;

    #[test]
    fn the_gain_is_flat_below_the_threshold_and_monotonic_above_it() {
        let threshold = power_ratio(-8.0);
        let reference = 1.0;
        let gain = |band| gain_of(band, reference, threshold, 1.0, SLOPE, MAX_REDUCTION_DB);

        // Below and at the threshold: untouched.
        for band in [0.0f32, threshold * 0.1, threshold * 0.99, threshold] {
            assert_eq!(gain(band), 1.0, "{band}");
        }

        // Above: falling, never inverting, and stopping at the limit rather
        // than running away.
        let floor = (-MAX_REDUCTION_DB / DECIBELS_PER_OCTAVE_AMPLITUDE).exp2();
        let mut previous = 1.0;
        for multiple in [1.1f32, 2.0, 10.0, 100.0, 1e6] {
            let reading = gain(threshold * multiple);
            assert!(
                reading <= previous,
                "{multiple} gave {reading}, was {previous}"
            );
            assert!(
                reading >= floor - 1e-6,
                "{multiple} went past the limit: {reading}"
            );
            previous = reading;
        }
        // And it does reach the limit, or the bound above is vacuous.
        assert!((previous - floor).abs() < 1e-6, "it stopped at {previous}");
    }

    #[test]
    fn hostile_values_neither_panic_nor_produce_nonsense() {
        let wild = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -1e9, 0.0, 1e9];
        for band in wild {
            for reference in wild {
                for amount in wild {
                    let gain = gain_of(
                        band,
                        reference,
                        power_ratio(-8.0),
                        amount,
                        SLOPE,
                        MAX_REDUCTION_DB,
                    );
                    assert!(
                        gain.is_finite() && gain > 0.0,
                        "{band}/{reference}/{amount}"
                    );
                }
            }
        }
    }

    /// **The shape Sparkleur's De-Harsh asks for** (`REQ-SPK-007`): one band,
    /// same mechanism. Velour drives the `N = 2` case
    /// (`velour_core::guard::tests`).
    #[test]
    fn a_single_band_guard_pulls_on_its_own_band() {
        const SETTINGS: Settings<1> = Settings {
            reference: Band {
                low_hz: 300.0,
                high_hz: 8_000.0,
            },
            bands: [GuardedBand {
                band: Band {
                    low_hz: 2_000.0,
                    high_hz: 5_000.0,
                },
                threshold_db: -8.0,
            }],
            attack_seconds: 0.002,
            release_seconds: 0.060,
            slope: SLOPE,
            max_reduction_db: MAX_REDUCTION_DB,
        };

        let signal = |harsh: f32| -> Vec<f32> {
            let body = tone(0.3, 400.0, RATE, 12_000);
            let edge = tone(harsh, 2_800.0, RATE, 12_000);
            body.iter().zip(&edge).map(|(a, b)| a + b).collect()
        };
        let settle = |input: &[f32]| {
            let mut guard = RelativeGuard::new(SETTINGS, RATE);
            for sample in input {
                guard.push(*sample, [1.0]);
            }
            guard.reduction_db(0)
        };

        assert_eq!(settle(&signal(0.02)), 0.0, "it fired on a balanced signal");
        let harsh = settle(&signal(0.8));
        assert!(harsh < -3.0, "it barely reacted: {harsh:.1} dB");
    }
}
