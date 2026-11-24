//! Fever: a saturator that heats up (`nonlinear-kit-plan.md`, decision 7).
//!
//! The input's power heats it; it cools on its own, over `recovery` seconds.
//! The heat raises the drive and pushes the curve off centre, and the hotter
//! it is the more both wander. The wander is a fixed pseudo-random sequence
//! rounded off at a few hertz, so the same input gives the same output.
//!
//! ```text
//! heat'    = HEATING·fever·power − heat/recovery        (0..=1)
//! drive    = 1 + 7·heat·(1 + w/4)
//! bias     = heat·w/2
//! shape(x) = (tanh(drive·x + bias) − tanh(bias)) / √drive
//! ```

use crate::{Channel, mix};

/// How fast a full `fever` heats on a unit of power, per second. A −12 dBFS
/// sine heats a cold Fever about halfway in a second.
pub const HEATING: f32 = 16.0;
pub const RECOVERY_MIN: f32 = 0.2;
pub const RECOVERY_MAX: f32 = 10.0;
/// How far the drive climbs when fully hot.
const DRIVE_RANGE: f32 = 7.0;
/// Where the wander is rounded off.
const WANDER_HZ: f32 = 3.0;

/// The curve at a given drive and bias, as the window draws it.
pub fn shape(x: f32, drive: f32, bias: f32) -> f32 {
    // shortcut: `1/√drive` is a rough loudness match, not a measured one;
    // replace with a gain fitted on programme once the curve settles.
    ((drive * x + bias).tanh() - bias.tanh()) / drive.sqrt()
}

/// The drive and bias at `heat` (`0..=1`) and wander `w` (`-1..=1`).
pub fn drive_and_bias(heat: f32, wander: f32) -> (f32, f32) {
    (
        1.0 + DRIVE_RANGE * heat * (1.0 + wander * 0.25),
        heat * wander * 0.5,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// `0..=1`.
    pub fever: f32,
    /// Seconds, `RECOVERY_MIN..=RECOVERY_MAX`.
    pub recovery: f32,
    /// `0..=1`.
    pub mix: f32,
}

/// White noise rounded off by one pole and scaled back to about `±1`.
#[derive(Clone, Debug)]
struct Wander {
    state: u32,
    pole: f32,
    gain: f32,
    value: f32,
}

impl Wander {
    fn new(sample_rate: f32) -> Self {
        let pole = (-std::f32::consts::TAU * WANDER_HZ / sample_rate).exp();
        // A uniform `±1` has variance 1/3; one pole keeps (1−a)/(1+a) of it.
        let gain = (3.0 * (1.0 + pole) / (1.0 - pole)).sqrt();
        Self {
            state: 0x9E37_79B9,
            pole,
            gain,
            value: 0.0,
        }
    }

    fn next(&mut self) -> f32 {
        // xorshift32: no allocation, and the same sequence every run.
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        let white = self.state as f32 / u32::MAX as f32 * 2.0 - 1.0;
        self.value = white + self.pole * (self.value - white);
        (self.value * self.gain).clamp(-1.0, 1.0)
    }
}

pub struct Fever {
    channels: [Channel; 2],
    wander: Wander,
    heat: f32,
    period: f32,
}

impl Fever {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            channels: [Channel::new(sample_rate), Channel::new(sample_rate)],
            wander: Wander::new(sample_rate),
            heat: 0.0,
            period: 1.0 / sample_rate,
        }
    }

    /// How hot it is, `0..=1`.
    pub fn heat(&self) -> f32 {
        self.heat
    }

    pub fn process(&mut self, input: [f32; 2], settings: &Settings) -> [f32; 2] {
        let power = 0.5 * (input[0] * input[0] + input[1] * input[1]);
        let recovery = settings.recovery.max(RECOVERY_MIN);
        let rise = HEATING * settings.fever * power - self.heat / recovery;
        self.heat = (self.heat + rise * self.period).clamp(0.0, 1.0);

        let (drive, bias) = drive_and_bias(self.heat, self.wander.next());
        let amount = settings.mix;
        let mut output = [0.0; 2];
        for (index, channel) in self.channels.iter_mut().enumerate() {
            output[index] = channel.process(input[index], |value| {
                mix(value, shape(value, drive, bias), amount)
            });
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_signal::sine;

    const RATE: f32 = 48_000.0;

    fn run(fever: &mut Fever, settings: &Settings, amplitude: f32, seconds: f32) -> Vec<f32> {
        (0..(RATE * seconds) as usize)
            .map(|index| {
                let x = sine(index, 220.0, amplitude, RATE);
                fever.process([x, x], settings)[0]
            })
            .collect()
    }

    #[test]
    fn a_cold_fever_is_close_to_unity_on_a_quiet_signal() {
        let x = 1e-3;
        assert!((shape(x, 1.0, 0.0) / x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn it_heats_on_a_loud_signal_and_cools_in_silence() {
        let settings = Settings {
            fever: 1.0,
            recovery: 1.0,
            mix: 1.0,
        };
        let mut fever = Fever::new(RATE);
        run(&mut fever, &settings, 0.5, 1.0);
        let hot = fever.heat();
        assert!(hot > 0.5, "{hot}");
        run(&mut fever, &settings, 0.0, 3.0);
        assert!(fever.heat() < hot * 0.1, "{}", fever.heat());
    }

    #[test]
    fn no_fever_never_heats() {
        let settings = Settings {
            fever: 0.0,
            recovery: 10.0,
            mix: 1.0,
        };
        let mut fever = Fever::new(RATE);
        run(&mut fever, &settings, 1.0, 0.5);
        assert_eq!(fever.heat(), 0.0);
    }

    #[test]
    fn the_same_input_gives_the_same_output() {
        let settings = Settings {
            fever: 1.0,
            recovery: 2.0,
            mix: 1.0,
        };
        let a = run(&mut Fever::new(RATE), &settings, 0.8, 0.5);
        let b = run(&mut Fever::new(RATE), &settings, 0.8, 0.5);
        assert_eq!(a, b);
        assert!(a.iter().all(|x| x.is_finite() && x.abs() < 2.0));
    }

    #[test]
    fn the_wander_stays_in_range_and_moves() {
        let mut wander = Wander::new(RATE);
        let values: Vec<f32> = (0..RATE as usize).map(|_| wander.next()).collect();
        assert!(values.iter().all(|w| (-1.0..=1.0).contains(w)));
        let spread = values.iter().fold(0.0f32, |m, w| m.max(w.abs()));
        assert!(spread > 0.2, "{spread}");
    }
}
