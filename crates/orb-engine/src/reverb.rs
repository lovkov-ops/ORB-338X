//! This fork: the reverb (RVRB), a send effect in the effects slots ([`crate::frame::RVRB`]),
//! as the chorus ([`crate::modfx`]): each channel's knob of the slot sends into its own send
//! bus, the frame runs it after the delays, its output into the master. A reverb no slot
//! holds, switched off or sent nothing does not run (the original's sound).
//!
//! Its controllers ([`ON`]..): ON, TYPE, MODE (0 stereo, the switch up; else mono), SIZE and
//! DECAY. The TYPEs ([`NAMES`], the display's letters):
//!
//! | TYPE | sound |
//! |---|---|
//! | HA, hall | a large hall: a predelay, a long and soft tail |
//! | rO, room | a small room: short, close |
//! | PL, plate | a plate: bright and dense, no predelay |
//! | SP, spring | an amp's spring: a dispersive "boing" |
//!
//! The hall, the room and the plate are a Schroeder-Moorer reverb (J. O. Smith's Freeverb
//! layout: eight damped combs into four allpasses per side, the right side's lines a little
//! longer); the spring is a feedback loop of a delay and a chain of dispersive allpasses.

use crate::frame::Block;

/// The controllers.
pub const ON: usize = 0;
pub const TYPE: usize = 1;
pub const MODE: usize = 2;
pub const SIZE: usize = 3;
pub const DECAY: usize = 4;
pub const PARAMS: usize = 5;
/// The TYPEs.
pub const HALL: u8 = 0;
pub const ROOM: u8 = 1;
pub const PLATE: u8 = 2;
pub const SPRING: u8 = 3;
pub const TYPES: u8 = 4;
/// The TYPEs' names, as the display's two letters show them.
pub const NAMES: [&str; TYPES as usize] = ["HA", "rO", "PL", "SP"];
/// The controllers at first: on, the hall, stereo, SIZE and DECAY in the middle.
pub const DEFAULTS: [u8; PARAMS] = [1, HALL, 0, 64, 64];

/// The sample rate.
const RATE: f64 = 44_100.0;
/// Freeverb's comb and allpass lengths (samples at 44.1 kHz), and the right side's spread.
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;
/// The longest a comb gets (its SIZE scale at most 1.6).
const COMB_MAX: usize = 2700;
/// The predelay's line (up to 80 ms).
const PRE_MAX: usize = 4096;
/// The input's level into the combs (Freeverb's fixed gain), and each TYPE's output level
/// (a return about 4 dB under its input at SIZE and DECAY in the middle).
const IN_GAIN: f64 = 0.015;
const OUT_GAIN: [f64; TYPES as usize] = [2.2, 2.2, 1.4, 1.8];
/// The spring: its dispersive allpasses, their coefficient, its loop's longest delay.
const SPRING_STAGES: usize = 12;
const SPRING_AP: f64 = -0.65;
const SPRING_MAX: usize = 4096;
/// The meter's decay per mixer update.
const METER_DECAY: f64 = 0.92;

/// A knob's 0..1.
fn n(v: u8) -> f64 {
    v.min(127) as f64 / 127.0
}

/// The TYPE of a controller value.
pub fn kind_of(v: u8) -> u8 {
    v.min(TYPES - 1)
}

/// A one-pole lowpass's coefficient for cutoff `fc` Hz.
fn lp_coef(fc: f64) -> f64 {
    1.0 - (-2.0 * std::f64::consts::PI * fc / RATE).exp()
}

/// A damped comb (a delay line read `len` samples back, its output lowpassed into its
/// feedback).
#[derive(Clone, Debug)]
struct Comb {
    buf: Vec<f64>,
    pos: usize,
    store: f64,
}

impl Comb {
    fn new(max: usize) -> Self {
        Comb {
            buf: vec![0.0; max],
            pos: 0,
            store: 0.0,
        }
    }

    fn run(&mut self, x: f64, len: usize, fb: f64, damp: f64) -> f64 {
        let m = self.buf.len();
        let out = self.buf[(self.pos + m - len.min(m - 1)) % m];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buf[self.pos] = x + self.store * fb;
        self.pos = (self.pos + 1) % m;
        out
    }
}

/// A Schroeder allpass of a fixed length.
#[derive(Clone, Debug)]
struct Allpass {
    buf: Vec<f64>,
    pos: usize,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Allpass {
            buf: vec![0.0; len],
            pos: 0,
        }
    }

    fn run(&mut self, x: f64, g: f64) -> f64 {
        let b = self.buf[self.pos];
        let y = b - x;
        self.buf[self.pos] = x + b * g;
        self.pos = (self.pos + 1) % self.buf.len();
        y
    }
}

/// The reverb and its memory.
#[derive(Clone, Debug)]
pub struct Reverb {
    /// Each side's combs and allpasses.
    combs: [Vec<Comb>; 2],
    aps: [Vec<Allpass>; 2],
    /// The predelay's line and its write place.
    pre: Vec<f64>,
    pre_pos: usize,
    /// The spring's loop: its line and write place, its allpasses' memory per side, its
    /// filters' memory (highpass input, lowpass).
    spring: Vec<f64>,
    spring_pos: usize,
    disp: [[(f64, f64); SPRING_STAGES]; 2],
    hp: f64,
    lp: [f64; 2],
    /// The output's peak (the meter).
    peak: f64,
}

impl Default for Reverb {
    fn default() -> Self {
        Self::new()
    }
}

impl Reverb {
    /// A silent reverb.
    pub fn new() -> Self {
        let combs = |extra: usize| {
            (0..COMBS.len())
                .map(|_| Comb::new(COMB_MAX + extra))
                .collect()
        };
        let aps = |extra: usize| ALLPASSES.iter().map(|&l| Allpass::new(l + extra)).collect();
        Reverb {
            combs: [combs(0), combs(SPREAD)],
            aps: [aps(0), aps(SPREAD)],
            pre: vec![0.0; PRE_MAX],
            pre_pos: 0,
            spring: vec![0.0; SPRING_MAX],
            spring_pos: 0,
            disp: [[(0.0, 0.0); SPRING_STAGES]; 2],
            hp: 0.0,
            lp: [0.0; 2],
            peak: 0.0,
        }
    }

    /// The meter byte of the output's peak since the last call (127 full scale); the peak then
    /// decays.
    pub fn meter(&mut self) -> u8 {
        let m = (self.peak.min(1.0) * 127.0) as u8;
        self.peak *= METER_DECAY;
        m
    }

    /// One block: the send bus `send` (mono) through the reverb with controllers `p`, its
    /// output added into the master `l` and `r`.
    pub fn process(&mut self, p: &[u8; PARAMS], send: &Block, l: &mut Block, r: &mut Block) {
        let stereo = p[MODE] == 0;
        let kind = kind_of(p[TYPE]);
        let (size, decay) = (n(p[SIZE]), n(p[DECAY]));
        // the tail's length (RT60, seconds): 0.2 s to 10 s, the room's and the spring's
        // shorter
        let rt = 0.2
            * 50f64.powf(decay)
            * match kind {
                ROOM => 0.35,
                SPRING => 0.4,
                _ => 1.0,
            };
        if kind == SPRING {
            self.spring_block(size, rt, stereo, send, l, r);
            return;
        }
        let gain = OUT_GAIN[kind as usize];
        let (scale, pre_ms, damp, ap_g) = match kind {
            HALL => (1.0 + 0.6 * size, 15.0 + 45.0 * size, 0.4, 0.5),
            ROOM => (0.3 + 0.4 * size, 2.0 + 8.0 * size, 0.55, 0.5),
            _ => (0.6 + 0.4 * size, 0.0, 0.15, 0.7),
        };
        let pre = ((pre_ms * RATE / 1000.0) as usize).min(PRE_MAX - 1);
        // each comb's feedback for that RT60: -60 dB after rt seconds
        let mut len = [[0usize; 8]; 2];
        let mut fb = [[0.0f64; 8]; 2];
        for side in 0..2 {
            for (i, &c) in COMBS.iter().enumerate() {
                let l = ((c + side * SPREAD) as f64 * scale) as usize;
                len[side][i] = l.clamp(1, COMB_MAX - 1);
                fb[side][i] = 10f64.powf(-3.0 * l as f64 / (rt * RATE)).min(0.98);
            }
        }
        for (j, &x) in send.iter().enumerate() {
            self.pre[self.pre_pos] = x as f64;
            let at = (self.pre_pos + PRE_MAX - pre) % PRE_MAX;
            let x = self.pre[at] * IN_GAIN;
            self.pre_pos = (self.pre_pos + 1) % PRE_MAX;
            let mut out = [0.0; 2];
            for (side, o) in out.iter_mut().enumerate() {
                let mut s = 0.0;
                for (i, comb) in self.combs[side].iter_mut().enumerate() {
                    s += comb.run(x, len[side][i], fb[side][i], damp);
                }
                for ap in self.aps[side].iter_mut() {
                    s = ap.run(s, ap_g);
                }
                *o = s * gain;
            }
            self.out(j, out, stereo, l, r);
        }
    }

    /// The spring's block: the input highpassed into a loop of a delay (25 ms to 60 ms by
    /// SIZE) and twelve dispersive allpasses, lowpassed, fed back for RT60 `rt`.
    fn spring_block(
        &mut self,
        size: f64,
        rt: f64,
        stereo: bool,
        send: &Block,
        l: &mut Block,
        r: &mut Block,
    ) {
        let d = ((25.0 + 35.0 * size) * RATE / 1000.0) as usize;
        let fb = 10f64.powf(-3.0 * d as f64 / (rt * RATE)).min(0.95);
        let hp = lp_coef(150.0);
        let lp = lp_coef(4500.0);
        // the right side reads the loop 7 ms later
        let taps = [d, d + (7.0 * RATE / 1000.0) as usize];
        for (j, &x) in send.iter().enumerate() {
            let x = x as f64;
            self.hp += hp * (x - self.hp);
            let input = (x - self.hp) * 0.5;
            let mut out = [0.0; 2];
            for (side, o) in out.iter_mut().enumerate() {
                let at =
                    (self.spring_pos + SPRING_MAX - taps[side].min(SPRING_MAX - 1)) % SPRING_MAX;
                let mut u = self.spring[at];
                for s in self.disp[side].iter_mut() {
                    let y = SPRING_AP * u + s.0 - SPRING_AP * s.1;
                    *s = (u, y);
                    u = y;
                }
                self.lp[side] += lp * (u - self.lp[side]);
                *o = self.lp[side];
            }
            self.spring[self.spring_pos] = input + fb * out[0];
            let out = out.map(|o| o * OUT_GAIN[SPRING as usize]);
            self.spring_pos = (self.spring_pos + 1) % SPRING_MAX;
            self.out(j, out, stereo, l, r);
        }
    }

    /// Sample `j` of the output `out` (mono: the sides' mean on both) into the master.
    fn out(&mut self, j: usize, out: [f64; 2], stereo: bool, l: &mut Block, r: &mut Block) {
        let (a, b) = if stereo {
            (out[0], out[1])
        } else {
            let m = 0.5 * (out[0] + out[1]);
            (m, m)
        };
        self.peak = self.peak.max(a.abs()).max(b.abs());
        l[j] = (l[j] as f64 + a) as f32;
        r[j] = (r[j] as f64 + b) as f32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::BLOCK;

    /// The reverb's output (left, right) of a 110 Hz saw sent for `on` blocks, then silence
    /// for the rest of `blocks`.
    fn run(p: [u8; PARAMS], on: usize, blocks: usize) -> Vec<(f32, f32)> {
        let mut rv = Reverb::new();
        let mut v = Vec::new();
        let mut ph = 0.0f64;
        for blk in 0..blocks {
            let mut s = [0.0f32; BLOCK];
            if blk < on {
                for x in s.iter_mut() {
                    *x = ((ph * 2.0 - 1.0) * 0.4) as f32;
                    ph = (ph + 110.0 / RATE).fract();
                }
            }
            let (mut l, mut r) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
            rv.process(&p, &s, &mut l, &mut r);
            v.extend(l.iter().copied().zip(r.iter().copied()));
        }
        v
    }

    fn rms(v: &[(f32, f32)]) -> f64 {
        (v.iter().map(|(a, _)| (*a as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    }

    #[test]
    fn every_type_sounds_in_range_and_its_tail_dies_away() {
        for t in 0..TYPES {
            for (size, decay) in [(0u8, 0u8), (127, 127), (64, 64)] {
                let p = [1, t, 0, size, decay];
                let out = run(p, 300, 300 + 7000);
                assert!(
                    out.iter()
                        .all(|(a, b)| a.is_finite() && a.abs() < 2.0 && b.abs() < 2.0),
                    "{} {size} {decay}",
                    NAMES[t as usize]
                );
                let during = rms(&out[100 * BLOCK..300 * BLOCK]);
                assert!(
                    during > 0.02,
                    "{} {size} {decay}: {during}",
                    NAMES[t as usize]
                );
                // ten seconds after the input stops: silent
                let end = rms(&out[out.len() - 20 * BLOCK..]);
                assert!(end < 0.002, "{} {size} {decay}: {end}", NAMES[t as usize]);
            }
        }
    }

    #[test]
    fn a_longer_decay_rings_longer() {
        for t in 0..TYPES {
            let tail = |decay: u8| {
                let out = run([1, t, 0, 64, decay], 100, 300);
                rms(&out[200 * BLOCK..])
            };
            assert!(tail(110) > 2.0 * tail(20), "{}", NAMES[t as usize]);
        }
    }

    #[test]
    fn stereo_differs_mono_does_not() {
        for t in 0..TYPES {
            let st = run([1, t, 0, 64, 64], 100, 150);
            assert!(st.iter().any(|(a, b)| a != b), "{}", NAMES[t as usize]);
            let mono = run([1, t, 1, 64, 64], 100, 150);
            assert!(mono.iter().all(|(a, b)| a == b), "{}", NAMES[t as usize]);
        }
    }
}
