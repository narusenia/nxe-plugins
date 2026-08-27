//! `DEPTH`, and the normalisation that keeps distance from being a fader.
//!
//! **This is the product's claim** (`REQ-VDP-008`): turning `DEPTH` moves the
//! voice, it does not move the level. If that breaks, the user is holding a wet
//! knob and a fader at once and the single macro means nothing.
//!
//! Specified in `plugins/vocal-depth/docs/specifications/dsp.md`,
//! "ラウドネス正規化".
//!
//! **The normalisation never looks at the signal** (`REQ-VDP-008`). It resolves
//! the wet chain's power against a fixed pink-weighted grid and divides by it,
//! the way `nxe_audio::shaper` divides by the RMS its curve returns for a probe
//! sine. The difference is that this chain is delays and LTI filters, so it
//! closes in the frequency domain and needs no probe to be run through it.

use nxe_audio::biquad::{BUTTERWORTH_Q, Coefficients};

use crate::direct;
use crate::reflections;

/// How many points the probe grid has.
///
/// The error this leaves is pinned by a test — the same treatment
/// `shaper::PROBE_POINTS` gets. More points do not move the answer.
pub const PROBE_POINTS: usize = 32;

/// The band the grid covers.
const PROBE_LOW_HZ: f32 = 20.0;
const PROBE_HIGH_HZ: f32 = 20_000.0;

/// The normalisation may not move the level further than this.
///
/// A limit, not a range anything in use should reach: it is here so that no
/// combination of hostile values turns into an infinity (`REQ-VDP-016`).
const GAIN_LIMIT_DB: f32 = 12.0;

/// Below this the resolved power says nothing and the gain passes through.
const POWER_FLOOR: f32 = 1e-8;

/// How much of the presence band's own power change the normalisation takes
/// out.
///
/// **This is one number with a decision behind it, and `REQ-VDP-008` cannot be
/// met at any value of it.** How much a band EQ changes a signal's total power
/// depends on how much of that signal is in the band, and that is a property of
/// the material: the presence band holds **10.3 %** of pink noise, **13.4 %** of
/// white, and **1.2 %** of a sparse harmonic phrase (`VDP-3`). A gain that may
/// not look at the signal (`REQ-VDP-008`) has to pick one of them.
///
/// What each choice costs, measured as the spread of output RMS across the whole
/// of `DEPTH` (`VDP-3`, the gate asks for 0.5 dB):
///
/// | compensation | pink | white | phrase | worst |
/// |---|---|---|---|---|
/// | 0.00 | 1.46 | 1.79 | 0.22 | 1.79 |
/// | 0.25 | 1.03 | 1.36 | 0.27 | 1.36 |
/// | 0.50 | 0.63 | 0.96 | 0.62 | **0.96** |
/// | 0.75 | 0.26 | 0.57 | 1.00 | 1.00 |
/// | 1.00 | 0.14 | 0.19 | 1.39 | 1.39 |
///
/// The best worst case is around 0.6 and lands at **0.8 dB**; dropping white,
/// which is not vocal material, the best is **0.62 dB at 0.5**. Neither reaches
/// 0.5 dB, and the other lever is the presence range itself: everything here
/// scales with it, so halving `+4 / -8 dB` halves the whole table.
///
/// **Left at 1.0 pending that decision**, which is the setting that keeps the
/// promise on broadband material — and the reflection term, which is broadband
/// and much larger, is compensated exactly at any value of this.
const PRESENCE_COMPENSATION: f32 = 0.6;

/// The same idea as [`PRESENCE_COMPENSATION`], for `DAMPING`: how much of the
/// corner's fall the normalisation takes out, counted in octaves.
///
/// **Measured the same way and it has the same wall** (`VDP-5`). What a lowpass
/// removes depends entirely on where the material's energy is, and the spread
/// across materials is wider here than for the presence band — white noise,
/// whose energy density *rises* with frequency, moves **4.4 dB** across `DEPTH`
/// with `DAMPING` open however this is set. That is why the probe is pink and
/// why white is not in the gate: it is not a stand-in for any voice
/// (`harmonics::pink` exists for the same reason).
///
/// Across `DEPTH` with `DAMPING` open, at `damping::DIRECT_NEAR` 0.55:
///
/// | compensation | pink | phrase | phrase + breath |
/// |---|---|---|---|
/// | 1.0 | 0.39 | 1.16 | 1.11 |
/// | **0.5** | **0.56** | **0.98** | **0.93** |
const DAMPING_COMPENSATION: f32 = 0.5;

/// Where a damping corner sits with nothing asked of it
/// (`crate::damping::OPEN_HZ`, repeated here so the modelling does not depend
/// on the module it models).
const OPEN_HZ: f32 = 20_000.0;

/// The main controls. All `0..=1` except where noted, all clamped: they arrive
/// from a host (`REQ-VDP-016`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Macros {
    /// Close (0) to far (1). One control, seven consequences
    /// (`REQ-VDP-002`).
    pub depth: f32,
    /// How near the direct sound is on top of `depth`, 0.5 neutral.
    pub direct: f32,
    /// How much reflection there is. Exactly none at zero.
    pub room: f32,
    /// How much high-frequency loss distance brings. Exactly none at zero
    /// (`REQ-VDP-005`).
    pub damping: f32,
    /// How far the space spreads. **The reflections only** — the voice stays
    /// where it is (`REQ-VDP-007`).
    pub width: f32,
    /// Dry to wet. **Exactly dry at zero, bit for bit** (`REQ-VDP-001`).
    pub mix: f32,
    /// Final gain, linear. **Exactly 1.0 by default**, because the bit-identity
    /// at `mix` = 0 rides on multiplying by one.
    pub output: f32,
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            depth: 0.5,
            direct: 0.5,
            room: 0.5,
            damping: 0.5,
            width: 0.6,
            mix: 1.0,
            output: 1.0,
        }
    }
}

impl Macros {
    /// Clamps every field into the range it is allowed, sending a non-finite
    /// value to something harmless.
    pub fn sanitised(self) -> Self {
        Self {
            depth: unit(self.depth),
            direct: unit(self.direct),
            room: unit(self.room),
            damping: unit(self.damping),
            width: unit(self.width),
            mix: unit(self.mix),
            output: if self.output.is_finite() {
                self.output.clamp(0.0, 4.0)
            } else {
                1.0
            },
        }
    }

    /// What the direct path is asked for. `clarity_lift_db` is `VDP-7`'s.
    pub fn direct_settings(self, clarity_lift_db: f32) -> direct::Settings {
        direct::Settings {
            distance: self.depth,
            presence: self.direct,
            clarity_lift_db,
        }
    }

    /// What the reflections are asked for.
    pub fn reflection_settings(self) -> reflections::Settings {
        reflections::Settings {
            distance: self.depth,
            amount: self.room,
        }
    }
}

/// What the stages resolved to, which is all the normalisation is allowed to
/// know (`REQ-VDP-008`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    /// [`direct::Direct::presence_db`].
    pub presence_db: f32,
    /// [`reflections::Reflections::tap_energy`], which already carries the
    /// bus's own gain.
    pub tap_energy: f32,
    /// [`crate::damping::Damping::direct_corner_hz`]. **`None` means the
    /// audio path is using `Coefficients::PASS`**, which is not the same as a
    /// corner at 20 kHz.
    pub direct_corner_hz: Option<f32>,
    /// [`crate::damping::Damping::reflected_corner_hz`].
    pub reflected_corner_hz: Option<f32>,
    /// [`crate::width::Width::reflected_power_factor`]. **The one term here
    /// that is exactly right for every material**, because the reflections'
    /// correlation is this design's property rather than the signal's.
    pub width_power_factor: f32,
}

/// The fixed grid the wet power is resolved on.
///
/// **Pink-weighted.** White is not a stand-in for ordinary material — its top
/// end is lifted by nothing but bandwidth (`nxe_audio::harmonics::pink` exists
/// for the same reason).
pub struct Probe {
    frequencies: [f32; PROBE_POINTS],
    weight: f32,
}

impl Probe {
    /// Log-spaced points from 20 Hz to 20 kHz, **equally weighted**.
    ///
    /// **Equal weights on a log grid *is* pink.** Pink has constant energy per
    /// octave and a log step is a constant fraction of an octave, so the `1/f`
    /// belongs to the grid spacing, not to the weights. Writing `1/f` on top of
    /// a log grid tilts it twice — which is what the first version did, and it
    /// weighted 20 Hz **150 times** above 3 kHz. The presence band then barely
    /// reached the sum and the normalisation had almost nothing to correct with
    /// (`VDP-3`).
    ///
    /// Built once, in `new`. Nothing here runs on the audio thread.
    pub fn new() -> Self {
        let mut frequencies = [0.0; PROBE_POINTS];

        let ratio = (PROBE_HIGH_HZ / PROBE_LOW_HZ).ln() / (PROBE_POINTS - 1) as f32;
        for (index, hz) in frequencies.iter_mut().enumerate() {
            *hz = PROBE_LOW_HZ * (ratio * index as f32).exp();
        }

        Self {
            frequencies,
            weight: 1.0 / PROBE_POINTS as f32,
        }
    }

    /// The gain that puts the wet chain back at the level it started from.
    ///
    /// Everything it needs comes out of the stages themselves ([`Resolved`]),
    /// so the tap table, the presence band and the damping corners each have
    /// exactly one owner.
    ///
    /// **Two things are deliberately not in here**, both because they depend on
    /// the signal and this may not (`REQ-VDP-008`): the transient's few
    /// hundredths of a dB (`VDP-2` measured 0.045 dB on steady material) and
    /// what `CLARITY` puts back (`VDP-7`).
    ///
    /// **The diffusion allpasses are not in here either**, and that one is
    /// free: an allpass does not change the total energy, so its coefficients
    /// can be retuned without touching this.
    pub fn gain(&self, resolved: Resolved, sample_rate: f32) -> f32 {
        let presence = Coefficients::peaking(
            direct::PRESENCE_CENTRE_HZ,
            direct::PRESENCE_Q,
            resolved.presence_db * PRESENCE_COMPENSATION,
            sample_rate,
        );
        let highpass = Coefficients::highpass(reflections::HIGHPASS_HZ, BUTTERWORTH_Q, sample_rate);
        // **The corners the parameters ask for, not the ones the transient
        // opens.** That one is signal-dependent, and it only ever lets more
        // through — bounded, and measured in `VDP-2`.
        let direct_damping = lowpass_or_pass(resolved.direct_corner_hz, sample_rate);
        let reflected_damping = lowpass_or_pass(resolved.reflected_corner_hz, sample_rate);
        let tap_energy = if resolved.tap_energy.is_finite() {
            resolved.tap_energy.max(0.0)
        } else {
            0.0
        } * if resolved.width_power_factor.is_finite() {
            resolved.width_power_factor.clamp(0.0, 1.0)
        } else {
            1.0
        };

        let mut power = 0.0;
        for &hz in &self.frequencies {
            // The direct path is minimum-phase sections in series, so their
            // magnitudes are the whole story.
            let direct_magnitude =
                presence.magnitude(hz, sample_rate) * direct_damping.magnitude(hz, sample_rate);

            // The reflections: incoherent with the direct sound above roughly
            // 90 Hz, and highpassed at 200 so the band where that is untrue has
            // no energy in it.
            let reflected =
                highpass.magnitude(hz, sample_rate) * reflected_damping.magnitude(hz, sample_rate);

            power += self.weight
                * (direct_magnitude * direct_magnitude + tap_energy * reflected * reflected);
        }

        if !power.is_finite() || power < POWER_FLOOR {
            return 1.0;
        }
        let gain = 1.0 / power.sqrt();
        let limit = 10.0f32.powf(GAIN_LIMIT_DB / 20.0);
        gain.clamp(1.0 / limit, limit)
    }
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}

/// A lowpass, or a pass-through when the caller says there is none.
///
/// The corner is moved back up by `DAMPING_COMPENSATION` before it is modelled;
/// see that constant.
fn lowpass_or_pass(hz: Option<f32>, sample_rate: f32) -> Coefficients {
    match hz {
        Some(hz) => {
            let modelled = hz * 2.0f32.powf((OPEN_HZ / hz).log2() * (1.0 - DAMPING_COMPENSATION));
            Coefficients::lowpass(modelled.min(OPEN_HZ), BUTTERWORTH_Q, sample_rate)
        }
        None => Coefficients::PASS,
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    /// A resolved chain with only the presence band and the taps doing
    /// anything — the damping corners wide open.
    fn resolved(presence_db: f32, tap_energy: f32) -> Resolved {
        Resolved {
            presence_db,
            tap_energy,
            direct_corner_hz: None,
            reflected_corner_hz: None,
            width_power_factor: 1.0,
        }
    }

    /// A chain that does nothing has to be left alone. If this drifts, every
    /// other measurement is offset by whatever it drifted.
    #[test]
    fn a_transparent_chain_needs_no_gain() {
        let probe = Probe::new();
        let gain = probe.gain(resolved(0.0, 0.0), RATE);
        assert!(
            (gain - 1.0).abs() < 1e-3,
            "a flat chain asked for {gain} of gain"
        );
    }

    /// The gain goes the other way from what the chain does: a band that is
    /// lifted gets pulled down, a band that is cut gets pushed up.
    #[test]
    fn the_gain_opposes_the_chain() {
        let probe = Probe::new();

        let lifted = probe.gain(resolved(6.0, 0.0), RATE);
        let cut = probe.gain(resolved(-6.0, 0.0), RATE);
        assert!(lifted < 1.0, "a lifted band was not pulled down: {lifted}");
        assert!(cut > 1.0, "a cut band was not pushed up: {cut}");

        // And adding reflections pulls it down further, monotonically.
        let mut previous = probe.gain(resolved(0.0, 0.0), RATE);
        for energy in [0.05f32, 0.1, 0.25, 0.5, 1.0] {
            let gain = probe.gain(resolved(0.0, energy), RATE);
            assert!(gain < previous, "tap energy {energy} did not pull down");
            previous = gain;
        }
    }

    /// 32 points is where the answer stops moving — the same claim
    /// `shaper::PROBE_POINTS` makes about 64, and the reason it is written down
    /// rather than assumed. Measured against four times the resolution:
    /// **0.011 dB at worst** (`VDP-3`).
    #[test]
    fn the_grid_is_dense_enough() {
        // The same integral at four times the resolution, computed by hand
        // here so the constant can be compared against something.
        let dense = |presence_db: f32, tap_energy: f32| {
            let points = PROBE_POINTS * 4;
            let ratio = (PROBE_HIGH_HZ / PROBE_LOW_HZ).ln() / (points - 1) as f32;
            let presence = Coefficients::peaking(
                direct::PRESENCE_CENTRE_HZ,
                direct::PRESENCE_Q,
                presence_db * PRESENCE_COMPENSATION,
                RATE,
            );
            let highpass = Coefficients::highpass(reflections::HIGHPASS_HZ, BUTTERWORTH_Q, RATE);

            let mut power = 0.0;
            for index in 0..points {
                let hz = PROBE_LOW_HZ * (ratio * index as f32).exp();
                let magnitude = presence.magnitude(hz, RATE);
                let reflected = highpass.magnitude(hz, RATE);
                power += magnitude * magnitude + tap_energy * reflected * reflected;
            }
            1.0 / (power / points as f32).sqrt()
        };

        let probe = Probe::new();
        for (presence_db, tap_energy) in [(0.0f32, 0.0f32), (4.0, 0.1), (-8.0, 0.5)] {
            let coarse = probe.gain(resolved(presence_db, tap_energy), RATE);
            let fine = dense(presence_db, tap_energy);
            let difference = 20.0 * (coarse / fine).log10();
            assert!(
                difference.abs() < 0.1,
                "{presence_db} dB / {tap_energy}: 32 points differ from 128 by {difference:.3} dB"
            );
        }
    }

    /// Hostile values cannot turn into an infinity or a zero divide
    /// (`REQ-VDP-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let probe = Probe::new();

        for (presence_db, tap_energy) in [
            (f32::NAN, 0.0f32),
            (0.0, f32::NAN),
            (f32::INFINITY, f32::INFINITY),
            (1e9, 1e9),
            (-1e9, -1e9),
        ] {
            let gain = probe.gain(resolved(presence_db, tap_energy), RATE);
            assert!(gain.is_finite(), "{presence_db} / {tap_energy} gave {gain}");
            assert!(gain > 0.0, "{presence_db} / {tap_energy} gave {gain}");
        }

        let sanitised = Macros {
            depth: f32::NAN,
            direct: f32::INFINITY,
            room: -1e9,
            damping: f32::NAN,
            width: f32::NEG_INFINITY,
            mix: 1e9,
            output: f32::NEG_INFINITY,
        }
        .sanitised();
        // **A non-finite value goes to zero, it is not clamped to the end it
        // came from.** `INFINITY` is not "the maximum" — it is a value that
        // means nothing, and 0 is the harmless reading of nothing.
        assert_eq!(sanitised.depth, 0.0);
        assert_eq!(sanitised.direct, 0.0);
        assert_eq!(sanitised.room, 0.0);
        assert_eq!(sanitised.damping, 0.0);
        assert_eq!(sanitised.width, 0.0);
        assert_eq!(sanitised.mix, 1.0);
        // `output` is the exception: 1.0 is the harmless reading there, since 0
        // would mute the plugin.
        assert_eq!(sanitised.output, 1.0);
    }
}
