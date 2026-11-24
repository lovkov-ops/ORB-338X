//! This fork: a mixer channel's EQ, the HI, MID and LO knobs of each channel. Not a device of
//! the original: the mixer keeps its controllers ([`crate::frame::EQ`]) and the frame runs it
//! on the channel after its inserts, before its pan, fader and delay send.
//!
//! Three bands, each -12 .. +12 dB, 0 dB at [`NEUTRAL`] (the knob in the middle): LO a low
//! shelf at 80 Hz, MID a peak at 2.5 kHz, HI a high shelf at 12 kHz (the biquads of R.
//! Bristow-Johnson's "Audio EQ Cookbook", shelf slope 1). A band at [`NEUTRAL`] is not run, so
//! with all three there the channel is the original's, bit for bit.

use crate::frame::Block;

/// The value of a band's controller at 0 dB.
pub const NEUTRAL: u8 = 64;
/// The bands' frequencies: LO, MID, HI.
pub const FREQS: [f64; 3] = [80.0, 2500.0, 12_000.0];
/// The gain at the knob's ends, in dB.
pub const RANGE_DB: f64 = 12.0;
/// The MID band's width (Q).
const MID_Q: f64 = 0.7;
/// The sample rate.
const RATE: f64 = 44_100.0;
/// The buses a channel's EQ runs on: the bus (or a split drum machine's left side), the right
/// side and the four slots' send sources of a split drum machine.
pub const SIDES: usize = 6;

/// The gain in dB of controller value `v` (0 -12 dB, [`NEUTRAL`] 0 dB, 127 +12 dB).
pub fn gain_db(v: u8) -> f64 {
    let v = v.min(127) as f64 - NEUTRAL as f64;
    if v >= 0.0 {
        v / (127.0 - NEUTRAL as f64) * RANGE_DB
    } else {
        v / NEUTRAL as f64 * RANGE_DB
    }
}

/// A biquad's coefficients, divided by a0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Coef {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Coef {
    /// Band `band` (0 LO, 1 MID, 2 HI) at `db`.
    fn of(band: usize, db: f64) -> Coef {
        let a = 10f64.powf(db / 40.0);
        let w0 = 2.0 * std::f64::consts::PI * FREQS[band] / RATE;
        let (sin, cos) = w0.sin_cos();
        let (b0, b1, b2, a0, a1, a2) = match band {
            1 => {
                let alpha = sin / (2.0 * MID_Q);
                (
                    1.0 + alpha * a,
                    -2.0 * cos,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos,
                    1.0 - alpha / a,
                )
            }
            _ => {
                // shelf slope 1: alpha = sin / 2 * sqrt(2)
                let alpha = sin / 2.0 * std::f64::consts::SQRT_2;
                let k = 2.0 * a.sqrt() * alpha;
                let (p, m) = (a + 1.0, a - 1.0);
                if band == 0 {
                    (
                        a * (p - m * cos + k),
                        2.0 * a * (m - p * cos),
                        a * (p - m * cos - k),
                        p + m * cos + k,
                        -2.0 * (m + p * cos),
                        p + m * cos - k,
                    )
                } else {
                    (
                        a * (p + m * cos + k),
                        -2.0 * a * (m + p * cos),
                        a * (p + m * cos - k),
                        p - m * cos + k,
                        2.0 * (m - p * cos),
                        p - m * cos - k,
                    )
                }
            }
        };
        Coef {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

/// A biquad's memory (direct form I: a coefficient change does not upset it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

/// One channel's EQ: its bands' values, coefficients and each side's memory.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelEq {
    set: [u8; 3],
    coef: [Coef; 3],
    state: [[State; 3]; SIDES],
}

impl Default for ChannelEq {
    fn default() -> Self {
        ChannelEq {
            set: [NEUTRAL; 3],
            coef: [Coef::default(); 3],
            state: Default::default(),
        }
    }
}

impl ChannelEq {
    /// The bands' controller values (LO, MID, HI); a band that leaves [`NEUTRAL`] starts from
    /// silence.
    pub fn set(&mut self, v: [u8; 3]) {
        for (band, &v) in v.iter().enumerate() {
            let v = v.min(127);
            if v == self.set[band] {
                continue;
            }
            if self.set[band] == NEUTRAL {
                for side in &mut self.state {
                    side[band] = State::default();
                }
            }
            self.set[band] = v;
            self.coef[band] = Coef::of(band, gain_db(v));
        }
    }

    /// Whether every band is at 0 dB (nothing runs).
    pub fn neutral(&self) -> bool {
        self.set == [NEUTRAL; 3]
    }

    /// The bands not at 0 dB on bus `b`, side `side` (0..[`SIDES`]), in place.
    pub fn process(&mut self, side: usize, b: &mut Block) {
        for band in 0..3 {
            if self.set[band] == NEUTRAL {
                continue;
            }
            let c = self.coef[band];
            let s = &mut self.state[side.min(SIDES - 1)][band];
            for x in b.iter_mut() {
                let x0 = *x as f64;
                let y = c.b0 * x0 + c.b1 * s.x1 + c.b2 * s.x2 - c.a1 * s.y1 - c.a2 * s.y2;
                s.x2 = s.x1;
                s.x1 = x0;
                s.y2 = s.y1;
                s.y1 = y;
                *x = y as f32;
            }
        }
    }

    /// Silence in every band's memory (the engine's reset).
    pub fn silence(&mut self) {
        self.state = Default::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::BLOCK;

    /// The level of a sine of `freq` Hz after the EQ, relative to before, in dB.
    fn response(set: [u8; 3], freq: f64) -> f64 {
        let mut eq = ChannelEq::default();
        eq.set(set);
        let (mut inp, mut out) = (0.0f64, 0.0f64);
        let mut n = 0usize;
        for blk in 0..400 {
            let mut b = [0.0f32; BLOCK];
            for (i, x) in b.iter_mut().enumerate() {
                let t = (blk * BLOCK + i) as f64 / RATE;
                *x = (0.25 * (2.0 * std::f64::consts::PI * freq * t).sin()) as f32;
            }
            let before = b;
            eq.process(0, &mut b);
            // after the filters settle
            if blk >= 200 {
                for (x, y) in before.iter().zip(b.iter()) {
                    inp += (*x as f64).powi(2);
                    out += (*y as f64).powi(2);
                    n += 1;
                }
            }
        }
        assert!(n > 0);
        10.0 * (out / inp).log10()
    }

    #[test]
    fn the_knobs_ends_and_middle() {
        assert_eq!(gain_db(NEUTRAL), 0.0);
        assert_eq!(gain_db(0), -RANGE_DB);
        assert_eq!(gain_db(127), RANGE_DB);
    }

    #[test]
    fn neutral_leaves_the_bus_as_it_is() {
        let mut eq = ChannelEq::default();
        assert!(eq.neutral());
        let mut b = [0.0f32; BLOCK];
        for (i, x) in b.iter_mut().enumerate() {
            *x = (i as f32 * 0.37).sin();
        }
        let before = b;
        eq.process(0, &mut b);
        assert_eq!(b, before);
        // a band moved and back: as it was
        eq.set([0, NEUTRAL, NEUTRAL]);
        assert!(!eq.neutral());
        eq.set([NEUTRAL; 3]);
        eq.process(0, &mut b);
        assert_eq!(b, before);
    }

    #[test]
    fn each_band_lifts_and_cuts_its_frequencies() {
        let n = NEUTRAL;
        // LO: the lows by 12 dB, the highs untouched
        assert!((response([127, n, n], 30.0) - 12.0).abs() < 0.5);
        assert!((response([0, n, n], 30.0) + 12.0).abs() < 0.5);
        assert!(response([127, n, n], 5000.0).abs() < 0.3);
        // MID: 2.5 kHz by 12 dB, the lows and highs hardly
        assert!((response([n, 127, n], 2500.0) - 12.0).abs() < 0.3);
        assert!((response([n, 0, n], 2500.0) + 12.0).abs() < 0.3);
        assert!(response([n, 127, n], 60.0).abs() < 1.0);
        // HI: the highs by 12 dB, the lows untouched
        assert!((response([n, n, 127], 18_000.0) - 12.0).abs() < 1.0);
        assert!(response([n, n, 127], 200.0).abs() < 0.3);
    }
}
