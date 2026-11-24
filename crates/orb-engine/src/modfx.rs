//! This fork: the chorus (CHO), the flanger (FLA) and the phaser (PHA), send effects in the
//! effects slots ([`crate::frame::MOD`]). Not devices of the original: the mixer keeps their
//! controllers, each channel's knob of the slot sends into the effect's own send bus (as into
//! the delay), and the frame runs the effect after the delays, its output into the master.
//! An effect no slot holds, switched off or sent nothing does not run (the original's sound).
//!
//! Each effect's controllers ([`ON`]..): ON, TYPE (STAGES for the phaser), MODE (0 stereo, the
//! switch up; else mono), then its four knobs:
//!
//! | effect | TYPE | knobs |
//! |---|---|---|
//! | CHO | 1 classic, 2 Juno, 3 ensemble, 4 vibrato | DELAY, RATE, DEPTH, VOICES |
//! | FLA | 1 classic, 2 jet, 3 slow and deep, 4 short metal | DELAY, RATE, DEPTH, FDBK |
//! | PHA | STAGES 2..12 | TONE, RATE, DEPTH, FDBK |

use crate::frame::Block;

/// The effects, in the order of their controllers.
pub const CHO: usize = 0;
pub const FLA: usize = 1;
pub const PHA: usize = 2;
/// Each effect's controllers.
pub const ON: usize = 0;
pub const TYPE: usize = 1;
pub const MODE: usize = 2;
pub const K1: usize = 3;
pub const K2: usize = 4;
pub const K3: usize = 5;
pub const K4: usize = 6;
pub const PARAMS: usize = 7;
/// The TYPEs' range (the chorus and the flanger) and the phaser's STAGES.
pub const TYPES: u8 = 4;
pub const STAGES_MIN: u8 = 2;
pub const STAGES_MAX: u8 = 12;
/// The controllers at first: on, the first TYPE (the phaser's 6 stages), stereo.
pub const DEFAULTS: [[u8; PARAMS]; 3] = [
    [1, 1, 0, 50, 30, 60, 42],
    [1, 1, 0, 40, 25, 80, 70],
    [1, 6, 0, 60, 25, 90, 60],
];

/// The sample rate.
const RATE: f64 = 44_100.0;
/// The delay lines' length (a power of two, 93 ms).
const LINE: usize = 4096;
/// How fast the delay time follows its knob (per sample).
const GLIDE: f64 = 0.0005;
/// The meter's decay per mixer update.
const METER_DECAY: f64 = 0.92;

/// A knob's 0..1.
fn n(v: u8) -> f64 {
    v.min(127) as f64 / 127.0
}

/// The LFO rate of a RATE knob: 0.05 Hz to 5 Hz, exponential.
fn rate_hz(v: u8) -> f64 {
    0.05 * 100f64.powf(n(v))
}

/// A sine of phase `p` (turns).
fn sine(p: f64) -> f64 {
    (2.0 * std::f64::consts::PI * p).sin()
}

/// A triangle of phase `p` (turns), -1..1.
fn triangle(p: f64) -> f64 {
    let p = p - p.floor();
    if p < 0.5 {
        4.0 * p - 1.0
    } else {
        3.0 - 4.0 * p
    }
}

/// The TYPE of a controller value, 1..=[`TYPES`].
pub fn kind_of(v: u8) -> u8 {
    v.clamp(1, TYPES)
}

/// The phaser's stages of a controller value: even, [`STAGES_MIN`]..=[`STAGES_MAX`].
pub fn stages_of(v: u8) -> usize {
    (v.clamp(STAGES_MIN, STAGES_MAX) & !1) as usize
}

/// One chorus, flanger or phaser and its memory.
#[derive(Clone, Debug)]
pub struct ModFx {
    fx: usize,
    /// Each side's delay line (the flanger's feedback makes them differ) and the write place.
    line: [Vec<f64>; 2],
    pos: usize,
    /// The LFO's phase (turns) and the ensemble's second LFO's.
    phase: f64,
    phase2: f64,
    /// The delay time, gliding to its knob (samples); `None` before the first block.
    base: Option<f64>,
    /// Each side's lowpass (the Juno chorus) and last output (the feedback).
    lp: [f64; 2],
    fb: [f64; 2],
    /// The phaser's stages' memory, each side's: (last input, last output).
    ap: [[(f64, f64); STAGES_MAX as usize]; 2],
    /// The output's peak (the meter).
    peak: f64,
}

impl ModFx {
    /// Effect `fx` ([`CHO`], [`FLA`], [`PHA`]), silent.
    pub fn new(fx: usize) -> Self {
        ModFx {
            fx: fx.min(PHA),
            line: [vec![0.0; LINE], vec![0.0; LINE]],
            pos: 0,
            phase: 0.0,
            phase2: 0.0,
            base: None,
            lp: [0.0; 2],
            fb: [0.0; 2],
            ap: [[(0.0, 0.0); STAGES_MAX as usize]; 2],
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

    /// The line `side` read `d` samples back (linear between samples).
    fn read(&self, side: usize, d: f64) -> f64 {
        let d = d.clamp(1.0, (LINE - 2) as f64);
        let at = self.pos as f64 + LINE as f64 - d;
        let i = at.floor();
        let f = at - i;
        let i = i as usize;
        let a = self.line[side][i & (LINE - 1)];
        let b = self.line[side][(i + 1) & (LINE - 1)];
        a + (b - a) * f
    }

    /// One block: the send bus `send` (mono) through the effect with controllers `p`, its
    /// output added into the master `l` and `r`.
    pub fn process(&mut self, p: &[u8; PARAMS], send: &Block, l: &mut Block, r: &mut Block) {
        let stereo = p[MODE] == 0;
        let sides = if stereo { 2 } else { 1 };
        let ms = RATE / 1000.0;
        match self.fx {
            CHO => {
                let kind = kind_of(p[TYPE]);
                let mut voices = 1 + (n(p[K4]) * 3.999) as usize;
                let mut d0 = (5.0 + 25.0 * n(p[K1])) * ms;
                let mut depth = n(p[K3]) * (d0 / ms - 1.0).min(8.0) * ms;
                if kind == 4 {
                    // vibrato: one voice, short and deep
                    voices = 1;
                    d0 = (2.0 + 6.0 * n(p[K1])) * ms;
                    depth = n(p[K3]) * (d0 - ms);
                }
                let f = rate_hz(p[K2]) / RATE;
                let lp = 1.0 - (-2.0 * std::f64::consts::PI * 7000.0 / RATE).exp();
                let mut base = self.base.unwrap_or(d0);
                for (j, &x) in send.iter().enumerate() {
                    let x = x as f64;
                    self.line[0][self.pos] = x;
                    base += (d0 - base) * GLIDE;
                    let mut out = [0.0; 2];
                    for (side, o) in out.iter_mut().enumerate().take(sides) {
                        let mut sum = 0.0;
                        for v in 0..voices {
                            let ph = self.phase
                                + v as f64 / voices as f64
                                + side as f64 * 0.5 / voices as f64;
                            let lfo = if kind == 2 { triangle(ph) } else { sine(ph) };
                            let mut d = base + depth * lfo;
                            if kind == 3 {
                                d += 0.15 * ms * sine(self.phase2 + v as f64 / 3.0);
                            }
                            sum += self.read(0, d);
                        }
                        let mut y = sum / (voices as f64).sqrt();
                        if kind == 2 {
                            self.lp[side] += lp * (y - self.lp[side]);
                            y = self.lp[side];
                        }
                        *o = y;
                    }
                    self.out(j, out, stereo, l, r);
                    self.phase = (self.phase + f).fract();
                    self.phase2 = (self.phase2 + 6.3 / RATE).fract();
                    self.pos = (self.pos + 1) & (LINE - 1);
                }
                self.base = Some(base);
            }
            FLA => {
                let kind = kind_of(p[TYPE]);
                let mut d0 = (0.3 + 9.7 * n(p[K1]).powi(2)) * ms;
                let mut f = rate_hz(p[K2]) / RATE;
                let mut depth = n(p[K3]) * 0.95;
                let mut fb = n(p[K4]) * 0.95;
                match kind {
                    2 => fb = -fb,
                    3 => {
                        f /= 4.0;
                        depth = 0.95;
                    }
                    4 => {
                        d0 /= 3.0;
                        fb = (fb + 0.2).min(0.95);
                    }
                    _ => {}
                }
                let mut base = self.base.unwrap_or(d0);
                for (j, &x) in send.iter().enumerate() {
                    let x = x as f64;
                    base += (d0 - base) * GLIDE;
                    let mut out = [0.0; 2];
                    for (side, o) in out.iter_mut().enumerate().take(sides) {
                        let lfo = sine(self.phase + side as f64 * 0.25);
                        let d = (base * (1.0 + depth * lfo)).max(0.05 * ms);
                        let y = self.read(side, d);
                        self.fb[side] = y;
                        *o = 0.6 * (x + y);
                    }
                    for side in 0..2 {
                        let w = x + fb * self.fb[side.min(sides - 1)];
                        // soft above full scale: the feedback cannot run away
                        self.line[side][self.pos] = (w * 0.5).tanh() * 2.0;
                    }
                    self.out(j, out, stereo, l, r);
                    self.phase = (self.phase + f).fract();
                    self.pos = (self.pos + 1) & (LINE - 1);
                }
                self.base = Some(base);
            }
            _ => {
                let stages = stages_of(p[TYPE]);
                let fc0 = 150.0 * 2f64.powf(4.0 * n(p[K1]));
                let f = rate_hz(p[K2]) / RATE;
                let oct = 3.0 * n(p[K3]);
                let fb = 0.75 * n(p[K4]);
                for (j, &x) in send.iter().enumerate() {
                    let x = x as f64;
                    let mut out = [0.0; 2];
                    for (side, o) in out.iter_mut().enumerate().take(sides) {
                        let lfo = sine(self.phase + side as f64 * 0.25);
                        let fc = (fc0 * 2f64.powf(oct * lfo)).min(RATE * 0.45);
                        let t = (std::f64::consts::PI * fc / RATE).tan();
                        let a = (t - 1.0) / (t + 1.0);
                        let mut u = x + fb * self.fb[side];
                        for s in self.ap[side].iter_mut().take(stages) {
                            let y = a * u + s.0 - a * s.1;
                            *s = (u, y);
                            u = y;
                        }
                        self.fb[side] = u;
                        // soft above full scale: the feedback's resonance cannot run away
                        *o = (0.6 * (x + u)).tanh();
                    }
                    self.out(j, out, stereo, l, r);
                    self.phase = (self.phase + f).fract();
                }
            }
        }
    }

    /// Sample `j` of the output `out` (mono: side 0 on both) into the master.
    fn out(&mut self, j: usize, out: [f64; 2], stereo: bool, l: &mut Block, r: &mut Block) {
        let (a, b) = if stereo {
            (out[0], out[1])
        } else {
            (out[0], out[0])
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

    /// The effect's output (left, right) of a 110 Hz saw sent for `blocks` blocks.
    fn run(fx: usize, p: [u8; PARAMS], blocks: usize) -> Vec<(f32, f32)> {
        let mut m = ModFx::new(fx);
        let mut v = Vec::new();
        let mut ph = 0.0f64;
        for _ in 0..blocks {
            let mut s = [0.0f32; BLOCK];
            for x in s.iter_mut() {
                *x = ((ph * 2.0 - 1.0) * 0.4) as f32;
                ph = (ph + 110.0 / RATE).fract();
            }
            let (mut l, mut r) = ([0.0f32; BLOCK], [0.0f32; BLOCK]);
            m.process(&p, &s, &mut l, &mut r);
            v.extend(l.iter().copied().zip(r.iter().copied()));
        }
        v
    }

    #[test]
    fn every_effect_and_type_sounds_in_range() {
        for fx in [CHO, FLA, PHA] {
            let types: &[u8] = if fx == PHA {
                &[2, 6, 12]
            } else {
                &[1, 2, 3, 4]
            };
            for &t in types {
                for knobs in [[0u8; 4], [127; 4], [64; 4]] {
                    let mut p = DEFAULTS[fx];
                    p[TYPE] = t;
                    p[K1..].copy_from_slice(&knobs);
                    let out = run(fx, p, 200);
                    assert!(
                        out.iter()
                            .all(|(a, b)| a.is_finite() && a.abs() < 2.0 && b.abs() < 2.0),
                        "fx {fx} type {t} {knobs:?}"
                    );
                    let rms = (out[BLOCK * 50..]
                        .iter()
                        .map(|(a, _)| (*a as f64).powi(2))
                        .sum::<f64>()
                        / (out.len() - BLOCK * 50) as f64)
                        .sqrt();
                    assert!(rms > 0.02, "fx {fx} type {t} {knobs:?}: {rms}");
                }
            }
        }
    }

    #[test]
    fn stereo_differs_mono_does_not() {
        for fx in [CHO, FLA, PHA] {
            let st = run(fx, DEFAULTS[fx], 100);
            assert!(st.iter().any(|(a, b)| a != b), "fx {fx}");
            let mut p = DEFAULTS[fx];
            p[MODE] = 1;
            assert!(run(fx, p, 100).iter().all(|(a, b)| a == b), "fx {fx}");
        }
    }

    #[test]
    fn the_controllers_ranges() {
        assert_eq!((kind_of(0), kind_of(3), kind_of(9)), (1, 3, 4));
        assert_eq!((stages_of(0), stages_of(7), stages_of(200)), (2, 6, 12));
        let mut m = ModFx::new(CHO);
        assert_eq!(m.meter(), 0);
        let _ = run(CHO, DEFAULTS[CHO], 1);
        m.peak = 0.5;
        assert_eq!(m.meter(), 63);
        assert!(m.meter() < 63);
    }
}
