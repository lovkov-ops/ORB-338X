//! This fork: the COMP's OTT ([`crate::frame::OTT`], the lamp on the compressor's face), a
//! multiband compressor that presses each band down above a threshold and lifts it up below
//! another. Not a device of the original: with it off the compressor is the original's, bit
//! for bit.
//!
//! The bus is split in three bands (Linkwitz-Riley crossovers, fourth order, at
//! [`CROSSOVERS`]; the low band through the high crossover's allpass, so the bands sum flat).
//! Each band's envelope follows its level; above [`DOWN_DB`] the band is pressed down
//! ([`DOWN_RATIO`]), below [`UP_DB`] lifted up ([`UP_RATIO`], at most [`UP_MAX_DB`], and not
//! at all near silence, [`FLOOR_DB`]). With OTT on the COMP's knobs mean other things: RATIO
//! is the BALANCE of the bands (left the lows louder, right the highs, the middle flat) and
//! THRES the GAIN (-12..+12 dB, 0 in the middle). How much OTT a channel hears is its COMP
//! knob on the mixer (the insert's wet).

use crate::frame::Block;

/// The sample rate.
const RATE: f64 = 44_100.0;
/// The crossovers, in Hz: low/mid and mid/high.
pub const CROSSOVERS: [f64; 2] = [120.0, 2500.0];
/// The downward threshold (dBFS) and ratio.
pub const DOWN_DB: f64 = -24.0;
pub const DOWN_RATIO: f64 = 4.0;
/// The upward threshold (dBFS), ratio and largest lift.
pub const UP_DB: f64 = -30.0;
pub const UP_RATIO: f64 = 4.0;
pub const UP_MAX_DB: f64 = 24.0;
/// No lift below this level (dBFS), full lift 10 dB above it.
pub const FLOOR_DB: f64 = -66.0;
/// The BALANCE's tilt at its ends, in dB (the lows up and the highs down, or the other way).
const TILT_DB: f64 = 6.0;
/// The GAIN's range at its ends, in dB.
const GAIN_DB: f64 = 12.0;
/// The bands' envelope attack and release times, in seconds (low, mid, high).
const ATTACK: [f64; 3] = [0.010, 0.005, 0.002];
const RELEASE: [f64; 3] = [0.150, 0.100, 0.060];
/// The output's trim, so that a typical sound (peaks about -12 dBFS) with the GAIN in the
/// middle is about as loud as without OTT.
const TRIM_DB: f64 = 6.0;

fn db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

fn time_coef(t: f64) -> f64 {
    1.0 - (-1.0 / (t * RATE)).exp()
}

/// A second-order Butterworth biquad's coefficients (`/a0`): a lowpass or a highpass.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Coef {
    b: [f64; 3],
    a: [f64; 2],
}

impl Coef {
    fn butter(fc: f64, high: bool) -> Coef {
        let w0 = 2.0 * std::f64::consts::PI * fc / RATE;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / std::f64::consts::SQRT_2;
        let a0 = 1.0 + alpha;
        let b = if high {
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0]
        } else {
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0]
        };
        Coef {
            b: b.map(|x| x / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
        }
    }
}

/// A biquad's memory (direct form I).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Biquad {
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn run(&mut self, c: &Coef, x: f64) -> f64 {
        let y = c.b[0] * x + c.b[1] * self.x[0] + c.b[2] * self.x[1]
            - c.a[0] * self.y[0]
            - c.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// The filters' coefficients: each crossover's lowpass and highpass.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Filters {
    lp: [Coef; 2],
    hp: [Coef; 2],
}

fn filters() -> Filters {
    Filters {
        lp: CROSSOVERS.map(|f| Coef::butter(f, false)),
        hp: CROSSOVERS.map(|f| Coef::butter(f, true)),
    }
}

/// The gain in dB a band at level `lvl` dBFS gets (before the BALANCE and the GAIN).
pub fn band_gain_db(lvl: f64) -> f64 {
    if lvl > DOWN_DB {
        DOWN_DB + (lvl - DOWN_DB) / DOWN_RATIO - lvl
    } else if lvl < UP_DB {
        let up = (UP_DB - (UP_DB - lvl) / UP_RATIO - lvl).min(UP_MAX_DB);
        up * ((lvl - FLOOR_DB) / 10.0).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// One bus's OTT: its crossovers' and its envelopes' memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ott {
    /// The low/mid crossover's lowpass and highpass, two biquads each (fourth order).
    lo: [Biquad; 2],
    rest: [Biquad; 2],
    /// The mid/high crossover's, on the rest.
    mid: [Biquad; 2],
    high: [Biquad; 2],
    /// The low band through the mid/high crossover's lowpass and highpass (its allpass).
    lo_lp: [Biquad; 2],
    lo_hp: [Biquad; 2],
    /// The bands' envelopes.
    env: [f64; 3],
    /// The filters' coefficients, made at the first block (no lock in the audio thread).
    filters: Option<Filters>,
}

impl Ott {
    /// Silence in the memory (the engine's reset).
    pub fn silence(&mut self) {
        *self = Ott::default();
    }

    /// The OTT on bus `b`, in place, with the COMP's ENABLED `on` (nothing when off), RATIO
    /// as the BALANCE `bal` and THRES as the GAIN `gain`.
    pub fn process(&mut self, on: bool, bal: u8, gain: u8, b: &mut Block) {
        if !on {
            return;
        }
        let f = *self.filters.get_or_insert_with(filters);
        let tilt = (bal.min(127) as f64 - 64.0) / 63.0;
        let band_db = [-tilt * TILT_DB, 0.0, tilt * TILT_DB];
        let out = db((gain.min(127) as f64 - 64.0) / 63.0 * GAIN_DB + TRIM_DB);
        let att = ATTACK.map(time_coef);
        let rel = RELEASE.map(time_coef);
        let two = |q: &mut [Biquad; 2], c: &Coef, x: f64| {
            let y = q[0].run(c, x);
            q[1].run(c, y)
        };
        for x in b.iter_mut() {
            let x0 = *x as f64;
            let lo = two(&mut self.lo, &f.lp[0], x0);
            let rest = two(&mut self.rest, &f.hp[0], x0);
            let lo = two(&mut self.lo_lp, &f.lp[1], lo) + two(&mut self.lo_hp, &f.hp[1], lo);
            let mid = two(&mut self.mid, &f.lp[1], rest);
            let high = two(&mut self.high, &f.hp[1], rest);
            let mut y = 0.0;
            for (k, band) in [lo, mid, high].into_iter().enumerate() {
                let a = band.abs();
                let c = if a > self.env[k] { att[k] } else { rel[k] };
                self.env[k] += c * (a - self.env[k]);
                let lvl = 20.0 * (self.env[k] + 1e-12).log10();
                y += band * db(band_gain_db(lvl) + band_db[k]);
            }
            *x = (y * out) as f32;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::BLOCK;

    fn sine(freq: f64, amp: f64, blocks: usize) -> Vec<Block> {
        let mut v = Vec::new();
        for blk in 0..blocks {
            let mut b = [0.0f32; BLOCK];
            for (i, x) in b.iter_mut().enumerate() {
                let t = (blk * BLOCK + i) as f64 / RATE;
                *x = (amp * (2.0 * std::f64::consts::PI * freq * t).sin()) as f32;
            }
            v.push(b);
        }
        v
    }

    fn rms(v: &[Block]) -> f64 {
        let n = (v.len() * BLOCK) as f64;
        (v.iter().flatten().map(|&x| (x as f64).powi(2)).sum::<f64>() / n).sqrt()
    }

    fn run(input: &[Block], bal: u8, gain: u8) -> Vec<Block> {
        let mut o = Ott::default();
        let mut out = input.to_vec();
        for b in out.iter_mut() {
            o.process(true, bal, gain, b);
        }
        out
    }

    #[test]
    fn the_gain_curve() {
        assert_eq!(band_gain_db(-30.0), 0.0);
        assert!((band_gain_db(0.0) + 18.0).abs() < 1e-9);
        assert!((band_gain_db(-48.0) - 13.5).abs() < 1e-9);
        assert_eq!(band_gain_db(-90.0), 0.0);
    }

    #[test]
    fn off_leaves_the_bus_and_silence_stays_silence() {
        let input = sine(440.0, 0.5, 4);
        let mut o = Ott::default();
        for b in &input {
            let mut y = *b;
            o.process(false, 127, 127, &mut y);
            assert_eq!(&y, b);
        }
        let out = run(&[[0.0; BLOCK]; 20], 64, 127);
        assert!(out.iter().flatten().all(|&x| x == 0.0));
    }

    #[test]
    fn loud_comes_down_and_quiet_comes_up() {
        let (loud, quiet) = (sine(1000.0, 0.9, 300), sine(1000.0, 0.01, 300));
        let r = |v: &[Block]| rms(&v[150..]);
        assert!(r(&run(&loud, 64, 64)) < r(&loud) * 0.5);
        assert!(r(&run(&quiet, 64, 64)) > r(&quiet) * 2.0);
    }

    #[test]
    fn the_balance_tilts_and_the_gain_lifts() {
        let lowtone = sine(60.0, 0.1, 300);
        let r = |v: &[Block]| rms(&v[150..]);
        // (less than the tilt's 12 dB: the mids lift what the crossover leaves of the tone)
        let (a, b) = (r(&run(&lowtone, 0, 64)), r(&run(&lowtone, 127, 64)));
        assert!(a > b * 2.0, "{a} {b}");
        let mid = sine(800.0, 0.1, 300);
        let ratio = r(&run(&mid, 64, 127)) / r(&run(&mid, 64, 64));
        assert!((20.0 * ratio.log10() - GAIN_DB).abs() < 0.5, "{ratio}");
    }
}
