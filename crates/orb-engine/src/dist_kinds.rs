//! This fork: the DIST's other kinds, picked by the lamps of its panel
//! ([`crate::frame::DIST_KIND`]). Kind [`CLASSIC`] is the original's distortion ([`crate::dist`]),
//! bit for bit; the others are this fork's, each with its own meaning of AMOUNT and SHAPE:
//!
//! | kind | AMOUNT | SHAPE |
//! |---|---|---|
//! | OD, overdrive | drive | tone, dark to bright |
//! | FUZZ | fuzz | bias: from smooth to a gated sputter |
//! | RECT, rectifier | how much is rectified | from half-wave to full-wave |
//! | BIT, bitcrusher | bits, 16 down to 4 | sample rate, 44.1 kHz down to 690 Hz |
//! | WF, wavefolder | folds | symmetry |
//! | SIN, sine shaper | drive | phase (even harmonics) |
//! | AMP, guitar amp | gain | the speaker's tone |
//!
//! The classic distortion still runs on a copy of the bus (its meter and its counters as ever,
//! [`crate::frame`]); the kind runs on the bus, each channel's (and side's) its own state.

use crate::frame::Block;

/// The kinds, in the lamps' order (top row, then bottom row).
pub const CLASSIC: u8 = 0;
pub const OD: u8 = 1;
pub const FUZZ: u8 = 2;
pub const RECT: u8 = 3;
pub const BIT: u8 = 4;
pub const WF: u8 = 5;
pub const SIN: u8 = 6;
pub const AMP: u8 = 7;
/// How many kinds there are.
pub const KINDS: u8 = 8;
/// The kinds' names, as the lamps' labels.
pub const NAMES: [&str; KINDS as usize] = ["DIST", "OD", "FUZZ", "RECT", "BIT", "WF", "SIN", "AMP"];

/// The sample rate.
const RATE: f64 = 44_100.0;
/// The DC blocker's pole (about 35 Hz).
const DC_POLE: f64 = 0.995;
/// Each kind's output level (index the kind), set so that a kind sounds about as loud as the
/// classic distortion at the same AMOUNT.
const OUT: [f64; KINDS as usize] = [1.0, 0.40, 0.39, 0.71, 1.0, 0.47, 0.42, 0.32];

/// Gain of `db` decibels.
fn db(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// A one-pole lowpass's coefficient for cutoff `fc` Hz.
fn lp_coef(fc: f64) -> f64 {
    1.0 - (-2.0 * std::f64::consts::PI * fc / RATE).exp()
}

/// The triangle fold of `x` into -1..1 (the identity inside it, mirrored at its edges).
fn fold(x: f64) -> f64 {
    let t = (x + 1.0) * 0.25;
    4.0 * (t - (t + 0.5).floor()).abs() - 1.0
}

/// One channel's (one side's) kind and its memory.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KindFx {
    /// The kind the memory is for (another kind starts from silence).
    kind: u8,
    /// One-pole lowpasses (the tone; the amp's stage and its speaker's two).
    lp: [f64; 3],
    /// A one-pole lowpass of the input (the amp's highpass).
    hp: f64,
    /// The DC blocker's last input and output.
    dc_x: f64,
    dc_y: f64,
    /// The bitcrusher's held sample and its clock.
    hold: f64,
    clock: f64,
}

impl KindFx {
    /// Silence in the memory (the engine's reset, a channel that leaves the DIST).
    pub fn silence(&mut self) {
        *self = KindFx {
            kind: self.kind,
            ..KindFx::default()
        };
    }

    fn dc_block(&mut self, x: f64) -> f64 {
        let y = x - self.dc_x + DC_POLE * self.dc_y;
        self.dc_x = x;
        self.dc_y = y;
        y
    }

    /// Kind `kind` with AMOUNT `amount` and SHAPE `shape` on bus `b`, in place; nothing when
    /// `on` is false (the DIST switched off) or the kind is [`CLASSIC`].
    pub fn process(&mut self, kind: u8, on: bool, amount: u8, shape: u8, b: &mut Block) {
        if !on || kind == CLASSIC || kind >= KINDS {
            return;
        }
        if kind != self.kind {
            *self = KindFx {
                kind,
                ..KindFx::default()
            };
        }
        let a = amount.min(127) as f64 / 127.0;
        let s = shape.min(127) as f64 / 127.0;
        let out = OUT[kind as usize];
        match kind {
            OD => {
                let g = db(a * 30.0);
                let bias = 0.15f64;
                let off = bias.tanh();
                let k = lp_coef(800.0 * 2f64.powf(s * 4.0));
                for x in b.iter_mut() {
                    let y = (g * *x as f64 + bias).tanh() - off;
                    self.lp[0] += k * (y - self.lp[0]);
                    *x = (self.lp[0] * out) as f32;
                }
            }
            FUZZ => {
                let g = db(12.0 + a * 36.0);
                let bias = s * 0.8;
                for x in b.iter_mut() {
                    let u = g * *x as f64 - bias;
                    // the positive side clips hard, the negative side softly and lower
                    let y = if u > 0.0 {
                        u / (1.0 + u)
                    } else {
                        0.6 * (u / 0.6).tanh()
                    };
                    *x = (self.dc_block(y) * out) as f32;
                }
            }
            RECT => {
                let k = 1.0 + 3.0 * a;
                for x in b.iter_mut() {
                    let x0 = *x as f64;
                    // half-wave (SHAPE 0) to full-wave (127)
                    let r = if x0 > 0.0 { x0 } else { -s * x0 };
                    let v = x0 + a * (2.0 * r - x0);
                    let y = (k * v).tanh();
                    *x = (self.dc_block(y) * out) as f32;
                }
            }
            BIT => {
                let q = 2f64.powf(15.0 - a * 12.0);
                let step = 2f64.powf(-s * 6.0);
                for x in b.iter_mut() {
                    self.clock += step;
                    if self.clock >= 1.0 {
                        self.clock -= 1.0;
                        self.hold = (*x as f64 * q).round() / q;
                    }
                    *x = (self.hold * out) as f32;
                }
            }
            WF => {
                let g = 1.0 + a * 9.0;
                let bias = s * 0.5;
                for x in b.iter_mut() {
                    let y = fold(g * *x as f64 + bias);
                    *x = (self.dc_block(y) * out) as f32;
                }
            }
            SIN => {
                let g = std::f64::consts::FRAC_PI_2 * (1.0 + a * 7.0);
                let ph = s * std::f64::consts::FRAC_PI_2;
                let off = ph.sin();
                for x in b.iter_mut() {
                    let y = (g * *x as f64 + ph).sin() - off;
                    *x = (self.dc_block(y) * out) as f32;
                }
            }
            _ => {
                // AMP: the lows out, two gain stages, the speaker's two lowpasses
                let g = db(6.0 + a * 36.0);
                let bias = 0.2f64;
                let off = bias.tanh();
                let hp = lp_coef(90.0);
                let mid = lp_coef(6000.0);
                let cab = lp_coef(2000.0 + s * 4000.0);
                for x in b.iter_mut() {
                    let x0 = *x as f64;
                    self.hp += hp * (x0 - self.hp);
                    let y1 = (g * (x0 - self.hp) + bias).tanh() - off;
                    self.lp[0] += mid * (y1 - self.lp[0]);
                    let y2 = (2.0 * self.lp[0]).tanh();
                    self.lp[1] += cab * (y2 - self.lp[1]);
                    self.lp[2] += cab * (self.lp[1] - self.lp[2]);
                    *x = (self.lp[2] * out) as f32;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::BLOCK;

    fn saw(blocks: usize, amp: f32) -> Vec<Block> {
        let mut v = Vec::new();
        let mut ph = 0.0f64;
        for _ in 0..blocks {
            let mut b = [0.0f32; BLOCK];
            for x in b.iter_mut() {
                *x = ((ph * 2.0 - 1.0) * amp as f64) as f32;
                ph = (ph + 110.0 / RATE).fract();
            }
            v.push(b);
        }
        v
    }

    fn rms(v: &[Block]) -> f64 {
        let n = (v.len() * BLOCK) as f64;
        (v.iter().flatten().map(|&x| (x as f64).powi(2)).sum::<f64>() / n).sqrt()
    }

    #[test]
    fn the_classic_kind_and_the_dist_off_leave_the_bus() {
        let mut fx = KindFx::default();
        let input = saw(4, 0.4);
        for (kind, on) in [(CLASSIC, true), (OD, false), (AMP, false)] {
            for b in &input {
                let mut y = *b;
                fx.process(kind, on, 100, 60, &mut y);
                assert_eq!(&y, b);
            }
        }
    }

    #[test]
    fn every_kind_changes_the_sound_and_stays_in_range() {
        let input = saw(200, 0.4);
        for kind in 1..KINDS {
            for (amount, shape) in [(0, 0), (70, 0), (127, 127), (64, 64)] {
                let mut fx = KindFx::default();
                let mut out = input.clone();
                for b in out.iter_mut() {
                    fx.process(kind, true, amount, shape, b);
                }
                let r = rms(&out[50..]);
                assert!(out.iter().flatten().all(|x| x.is_finite() && x.abs() < 1.5));
                assert!(
                    r > 0.01,
                    "{} {amount} {shape}: rms {r}",
                    NAMES[kind as usize]
                );
                assert_ne!(out, input, "{}", NAMES[kind as usize]);
            }
        }
    }

    #[test]
    fn silence_stays_silence() {
        for kind in 1..KINDS {
            let mut fx = KindFx::default();
            let mut b = [0.0f32; BLOCK];
            for _ in 0..10 {
                fx.process(kind, true, 127, 0, &mut b);
            }
            assert!(b.iter().all(|x| x.abs() < 1e-3), "{}", NAMES[kind as usize]);
        }
    }

    #[test]
    fn rect_full_wave_doubles_the_pitch() {
        // a sine full-wave rectified: its zero crossings (after the DC blocker) twice as many
        let mut fx = KindFx::default();
        let mut v = Vec::new();
        for blk in 0..400 {
            let mut b = [0.0f32; BLOCK];
            for (i, x) in b.iter_mut().enumerate() {
                let t = (blk * BLOCK + i) as f64 / RATE;
                *x = (0.4 * (2.0 * std::f64::consts::PI * 100.0 * t).sin()) as f32;
            }
            let before = b;
            fx.process(RECT, true, 127, 127, &mut b);
            if blk >= 200 {
                v.push((before, b));
            }
        }
        let crossings = |f: &dyn Fn(&(Block, Block)) -> Block| {
            let s: Vec<f32> = v.iter().flat_map(f).collect();
            s.windows(2)
                .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
                .count() as f64
        };
        let ratio = crossings(&|p| p.1) / crossings(&|p| p.0);
        assert!((ratio - 2.0).abs() < 0.1, "{ratio}");
    }
}
