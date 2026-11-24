//! This fork: the delay's TAPE ([`crate::frame::TAPE`], the lamp on the delay's face). Not a
//! device of the original: with it off the delay is the original's, bit for bit.
//!
//! With it on the delay reads its line a little later than its length, by a slow wow and a
//! faster flutter (the repeats' pitch wobbles), between two samples (linear), and what it
//! writes back into the line goes through a tape's color: a soft saturation, a lowpass and a
//! highpass, so that each repeat is a little dirtier and duller than the one before it.

/// The sample rate.
const RATE: f64 = 44_100.0;
/// The wow: its rate in Hz and its depth in samples (the extra delay swings 0..2x it).
const WOW_HZ: f64 = 0.8;
const WOW_DEPTH: f64 = 110.0;
/// The flutter: its rate in Hz and its depth in samples.
const FLUTTER_HZ: f64 = 7.5;
const FLUTTER_DEPTH: f64 = 7.0;
/// The saturation's drive (1/DRIVE its ceiling).
const DRIVE: f64 = 2.6;
/// The color's lowpass and highpass cutoffs, in Hz.
const LOWPASS_HZ: f64 = 2600.0;
const HIGHPASS_HZ: f64 = 150.0;

/// A one-pole lowpass's coefficient for cutoff `fc` Hz.
fn lp_coef(fc: f64) -> f64 {
    1.0 - (-2.0 * std::f64::consts::PI * fc / RATE).exp()
}

/// The longest extra delay the wobble gives, in samples.
pub const MAX_WOBBLE: f64 = 2.0 * (WOW_DEPTH + FLUTTER_DEPTH);

/// The tape's memory: the wobble's phases, the color's filters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tape {
    wow: f64,
    flutter: f64,
    lp: f64,
    hp: f64,
}

impl Tape {
    /// The next sample's extra delay, in samples (0..[`MAX_WOBBLE`]).
    pub fn wobble(&mut self) -> f64 {
        let tau = 2.0 * std::f64::consts::PI;
        self.wow = (self.wow + WOW_HZ / RATE).fract();
        self.flutter = (self.flutter + FLUTTER_HZ / RATE).fract();
        WOW_DEPTH * (1.0 - (tau * self.wow).cos())
            + FLUTTER_DEPTH * (1.0 - (tau * self.flutter).cos())
    }

    /// `v` through the tape's color (what the delay writes back into its line).
    pub fn color(&mut self, v: f64) -> f64 {
        let s = (DRIVE * v).tanh() / DRIVE;
        self.lp += lp_coef(LOWPASS_HZ) * (s - self.lp);
        self.hp += lp_coef(HIGHPASS_HZ) * (self.lp - self.hp);
        self.lp - self.hp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wobble_stays_in_its_range_and_moves() {
        let mut t = Tape::default();
        let v: Vec<f64> = (0..200_000).map(|_| t.wobble()).collect();
        assert!(v.iter().all(|&d| (0.0..=MAX_WOBBLE).contains(&d)));
        let (lo, hi) = v
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), &d| (a.min(d), b.max(d)));
        assert!(hi - lo > WOW_DEPTH, "{lo} {hi}");
    }

    #[test]
    fn the_color_darkens_and_tames_the_peaks() {
        // a loud high tone comes out quieter than a loud low one, and below the ceiling
        let level = |freq: f64| {
            let mut t = Tape::default();
            let mut peak = 0.0f64;
            for i in 0..20_000 {
                let x = 2.0 * (2.0 * std::f64::consts::PI * freq * i as f64 / RATE).sin();
                let y = t.color(x);
                if i > 10_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let (low, high) = (level(400.0), level(9000.0));
        // (the filters ring a little over the clipped wave's ceiling)
        assert!(low < 1.25 / DRIVE, "{low}");
        assert!(high < low * 0.6, "{low} {high}");
    }
}
