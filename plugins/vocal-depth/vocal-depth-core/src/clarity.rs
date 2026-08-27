//! `CLARITY`: putting back the intelligibility distance took away.
//!
//! **The guard, inverted** (`REQ-VDP-006`). `nxe_audio::guard::RelativeGuard`
//! pulls a band down when it sits *above* a reference; what is wanted here is a
//! push up when a band sits *below* one. The sign is not a parameter of that
//! block — but **the bands are**, and swapping them turns the same arithmetic
//! around:
//!
//! ```text
//! guard's "guarded band"  ->  200 Hz - 1.2 kHz   (the body of the voice)
//! guard's "reference"     ->  2 - 5 kHz          (the presence band)
//! ```
//!
//! Now "the guarded band is excessive relative to the reference" is exactly
//! "the presence band is deficient relative to the body", the reduction the
//! guard reports is the deficit in dB, and it is applied as a **lift** on the
//! presence section `crate::direct` already owns. **No new filter, no new
//! follower, and no new sign convention.**
//!
//! **The reference band sits below the detection band, not around it.** Getting
//! that wrong is what made Sparkleur's De-Harsh read 10 dB of harshness as 2
//! (`SPK-18`): a reference that contains the band it is measuring moves with it.
//!
//! Specified in `plugins/vocal-depth/docs/specifications/dsp.md`, "CLARITY".

use nxe_audio::guard::{Band, GuardedBand, RelativeGuard, Settings as GuardSettings};

/// The band the lift lands on — the same one `crate::direct` moves, because it
/// *is* that section.
const PRESENCE_LOW_HZ: f32 = crate::direct::PRESENCE_LOW_HZ;
const PRESENCE_HIGH_HZ: f32 = crate::direct::PRESENCE_HIGH_HZ;

/// What the presence band is compared against: the body of the voice.
///
/// **Below the presence band and not overlapping it** (`SPK-18`).
const BODY_LOW_HZ: f32 = 200.0;
const BODY_HIGH_HZ: f32 = 1_200.0;

/// How far the body may sit above the presence band before this reads as a
/// deficit.
///
/// **Measured, not chosen** (`VDP-7`, and `SPK-18` is why it is measured from a
/// distribution rather than from one sample). The ratio is wildly
/// material-dependent — see the module's tests for the table — so this sits
/// above where ordinary material lives, and what pushes past it is the chain's
/// own presence cut rather than the material.
const THRESHOLD_DB: f32 = 21.0;

/// The most it may put back. Past this, the far end is not far any more.
const MAX_LIFT_DB: f32 = 6.0;

/// How much lift per dB of deficit.
const SLOPE: f32 = 1.0;

/// Fast enough to follow a phrase, slow enough not to chatter inside a
/// syllable — the same reasoning `nxe_audio::guard::Settings` documents.
const ATTACK_SECONDS: f32 = 0.030;
const RELEASE_SECONDS: f32 = 0.200;

pub struct Clarity {
    guard: RelativeGuard<1>,
    /// `CLARITY · distance`, which is what makes both ends exact.
    amount: f32,
    lift_db: f32,
}

impl Clarity {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            guard: RelativeGuard::new(
                GuardSettings {
                    // **Swapped**: see the module docs.
                    reference: Band {
                        low_hz: PRESENCE_LOW_HZ,
                        high_hz: PRESENCE_HIGH_HZ,
                    },
                    bands: [GuardedBand {
                        band: Band {
                            low_hz: BODY_LOW_HZ,
                            high_hz: BODY_HIGH_HZ,
                        },
                        threshold_db: THRESHOLD_DB,
                    }],
                    attack_seconds: ATTACK_SECONDS,
                    release_seconds: RELEASE_SECONDS,
                    slope: SLOPE,
                    max_reduction_db: MAX_LIFT_DB,
                },
                sample_rate,
            ),
            amount: 0.0,
            lift_db: 0.0,
        }
    }

    /// **Block rate.**
    ///
    /// `amount` is `CLARITY` and `distance` is `DEPTH`. They are multiplied
    /// because both ends have to be *exactly* nothing: `CLARITY` = 0 because a
    /// mechanism nobody can switch off is one nobody can hear (`REQ-VDP-006`),
    /// and `DEPTH` = close because there is nothing to put back there.
    pub fn set(&mut self, amount: f32, distance: f32) {
        self.amount = unit(amount) * unit(distance);
    }

    /// One sample of the mono sum, returning how much to lift the presence
    /// band by, in dB. **Audio rate.**
    ///
    /// Detection is linked across the pair (`REQ-VDP-011`): one reading drives
    /// both channels, so this cannot move the image.
    pub fn push(&mut self, mono: f32) -> f32 {
        self.guard.push(mono, [self.amount]);
        // The guard reports a reduction, which is negative or zero. The deficit
        // is its magnitude.
        self.lift_db = -self.guard.reduction_db(0);
        self.lift_db
    }

    /// How far it is lifting, in dB. **Shown on screen** — a mechanism that
    /// works invisibly is a control that does nothing (`REQ-VDP-006`,
    /// `.agents/rules/ui.md`).
    pub fn lift_db(&self) -> f32 {
        self.lift_db
    }

    pub fn reset(&mut self) {
        self.guard.reset();
        self.lift_db = 0.0;
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
    use nxe_audio::biquad::BandPass;
    use nxe_audio::harmonics;

    const RATE: f32 = 48_000.0;
    /// A relative detector reads nonsense until its followers have filled
    /// (`SPK-18`).
    const DISCARD: usize = (RATE as usize) / 4;

    /// A stand-in for a sung phrase, the same one `crate::engine` uses.
    fn phrase(length: usize) -> Vec<f32> {
        let breath = harmonics::pink(1.0, length);
        let mut signal = vec![0.0; length];
        let mut phases = [0.0f32; 7];
        let partials = [
            (200.0, 1.0f32),
            (400.0, 0.5),
            (600.0, 0.3),
            (1_200.0, 0.2),
            (2_400.0, 0.12),
            (4_800.0, 0.06),
            (9_600.0, 0.03),
        ];
        for (index, sample) in signal.iter_mut().enumerate() {
            let t = index as f32 / RATE;
            let mut voice = 0.0;
            for (partial, (hz, amplitude)) in partials.iter().enumerate() {
                phases[partial] += std::f32::consts::TAU * hz / RATE;
                voice += amplitude * phases[partial].sin();
            }
            let syllable = (t * 4.0).fract();
            let envelope = if syllable < 0.1 {
                syllable / 0.1
            } else {
                ((1.0 - syllable) / 0.9).max(0.0)
            };
            *sample = 0.2 * envelope * (voice + 0.06 * breath[index]);
        }
        signal
    }

    /// The body-over-presence power ratio in dB, which is what the threshold is
    /// compared against.
    fn ratio_db(signal: &[f32]) -> f32 {
        let mut body = BandPass::new(BODY_LOW_HZ, BODY_HIGH_HZ, RATE);
        let mut presence = BandPass::new(PRESENCE_LOW_HZ, PRESENCE_HIGH_HZ, RATE);
        let mut body_energy = 0.0;
        let mut presence_energy = 0.0;
        for (index, &sample) in signal.iter().enumerate() {
            let low = body.process(sample);
            let high = presence.process(sample);
            if index >= DISCARD {
                body_energy += low * low;
                presence_energy += high * high;
            }
        }
        10.0 * (body_energy.max(1e-20) / presence_energy.max(1e-20)).log10()
    }

    /// The phrase with `depth` dB taken out of its presence band.
    ///
    /// **A peaking section, not a subtracted band-pass.** The first version of
    /// this helper used `x - 0.75·BandPass(x)` and took barely anything out —
    /// which is exactly what `VDP-3` measured and why
    /// `Coefficients::peaking` exists (the skirts give the cut back).
    fn scoured(depth_db: f32) -> Vec<f32> {
        let coefficients = nxe_audio::biquad::Coefficients::peaking(
            crate::direct::PRESENCE_CENTRE_HZ,
            crate::direct::PRESENCE_Q,
            -depth_db,
            RATE,
        );
        let mut section = nxe_audio::biquad::Biquad::new(coefficients);
        phrase(RATE as usize)
            .iter()
            .map(|&sample| section.process(sample))
            .collect()
    }

    /// The lift after running `signal` through, in dB.
    fn lift_after(clarity: &mut Clarity, signal: &[f32]) -> f32 {
        let mut worst = 0.0f32;
        for (index, &sample) in signal.iter().enumerate() {
            let lift = clarity.push(sample);
            if index >= DISCARD {
                worst = worst.max(lift);
            }
        }
        worst
    }

    /// **Where the threshold came from** (`VDP-7`). The ratio is
    /// material-dependent by 15 dB, so the threshold has to sit above where
    /// ordinary material lives — and this test is what keeps the numbers on
    /// record so it cannot be moved blind.
    ///
    /// | material | body over presence |
    /// |---|---|
    /// | pink noise | about +3 dB |
    /// | a sung phrase | about +18 dB |
    ///
    /// **The threshold is 21 dB**, above both, so what pushes past it is the
    /// chain's own presence cut rather than the material.
    #[test]
    fn the_threshold_sits_above_where_ordinary_material_lives() {
        let pink = ratio_db(&harmonics::pink(0.5, RATE as usize));
        let sung = ratio_db(&phrase(RATE as usize));

        assert!(
            (0.0..8.0).contains(&pink),
            "pink noise reads {pink:.1} dB, not the +3 on record"
        );
        assert!(
            (12.0..21.0).contains(&sung),
            "the phrase reads {sung:.1} dB, not the +18 on record"
        );
        assert!(
            THRESHOLD_DB > sung,
            "the threshold ({THRESHOLD_DB} dB) is below ordinary material ({sung:.1} dB)"
        );
    }

    /// **Zero is exactly nothing**, at either end (`REQ-VDP-006`).
    #[test]
    fn both_ends_are_exactly_nothing() {
        let signal = phrase(RATE as usize);

        let mut off = Clarity::new(RATE);
        off.set(0.0, 1.0);
        assert_eq!(lift_after(&mut off, &signal), 0.0, "CLARITY = 0 lifted");

        let mut close = Clarity::new(RATE);
        close.set(1.0, 0.0);
        assert_eq!(lift_after(&mut close, &signal), 0.0, "DEPTH = close lifted");
    }

    /// **Ordinary material gets nothing back** (`SPK-18`'s lesson: a protection
    /// that fires on everything is a colour, not a protection). Measured on
    /// broadband material, not on two tones — a signal with nothing in the
    /// detection band passes any threshold.
    #[test]
    fn ordinary_material_gets_no_lift() {
        for (name, signal) in [
            ("pink", harmonics::pink(0.5, RATE as usize)),
            ("white", harmonics::noise(0.5, RATE as usize)),
            ("phrase", phrase(RATE as usize)),
        ] {
            let mut clarity = Clarity::new(RATE);
            clarity.set(1.0, 1.0);
            let lift = lift_after(&mut clarity, &signal);
            assert_eq!(lift, 0.0, "{name} was lifted by {lift:.2} dB");
        }
    }

    /// And material the presence has actually been taken out of does get it
    /// back. **Without this the test above passes on a mechanism that never
    /// fires at all** (`VEL-10`).
    #[test]
    fn material_with_its_presence_removed_gets_it_back() {
        // 12 dB out of the presence band, which is more than `DEPTH` alone
        // takes — enough to clear the threshold on this material.
        let scoured = scoured(12.0);

        let mut clarity = Clarity::new(RATE);
        clarity.set(1.0, 1.0);
        let lift = lift_after(&mut clarity, &scoured);
        assert!(lift > 0.5, "nothing came back: {lift:.2} dB");
        assert!(
            lift <= MAX_LIFT_DB + 1e-3,
            "it came back too far: {lift:.2} dB"
        );
    }

    /// The lift grows with `DEPTH` (`REQ-VDP-006`).
    #[test]
    fn the_lift_grows_with_distance() {
        let scoured = scoured(12.0);

        let mut previous = -1.0;
        for step in 0..=4 {
            let mut clarity = Clarity::new(RATE);
            clarity.set(1.0, step as f32 / 4.0);
            let lift = lift_after(&mut clarity, &scoured);
            assert!(
                lift > previous,
                "the lift did not grow at DEPTH {}: {lift:.2} after {previous:.2}",
                step as f32 / 4.0
            );
            previous = lift;
        }
    }

    /// **A ratio cannot depend on the input level** (`REQ-VDP-006`), which is
    /// the whole reason the mechanism is relative.
    #[test]
    fn the_lift_ignores_input_gain() {
        let scoured = scoured(12.0);

        let lift_at = |gain: f32| {
            let scaled: Vec<f32> = scoured.iter().map(|s| s * gain).collect();
            let mut clarity = Clarity::new(RATE);
            clarity.set(1.0, 1.0);
            lift_after(&mut clarity, &scaled)
        };

        let reference = lift_at(1.0);
        for gain in [10.0f32.powf(-12.0 / 20.0), 10.0f32.powf(12.0 / 20.0)] {
            let measured = lift_at(gain);
            assert!(
                (measured - reference).abs() < 0.2,
                "gain {gain}: {measured:.3} dB against {reference:.3} dB"
            );
        }
    }

    /// Hostile values in, finite values out (`REQ-VDP-016`).
    #[test]
    fn hostile_values_stay_finite() {
        let mut clarity = Clarity::new(RATE);
        for (amount, distance) in [
            (f32::NAN, 1.0f32),
            (f32::INFINITY, f32::NEG_INFINITY),
            (1e9, -1e9),
        ] {
            clarity.set(amount, distance);
            assert!(clarity.lift_db().is_finite());
        }

        clarity.set(1.0, 1.0);
        for sample in [1e9f32, -1e9, 0.5] {
            let lift = clarity.push(sample);
            assert!(lift.is_finite(), "{sample} gave {lift}");
        }
    }
}
