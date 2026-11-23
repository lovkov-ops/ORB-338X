//! This fork: the master's limiter, the LIMIT key and its AMT knob in the master section (in
//! place of the original's master COMP key). Not a device of the original: the mixer keeps its
//! two controllers ([`crate::frame::LIMIT`], [`crate::frame::LIMIT_AMT`]) and the frame runs it
//! on the master after the master gain, before the master meters.
//!
//! A look-ahead peak limiter, linked over both sides: AMT drives the master into it (0 dB at
//! 0 .. [`MAX_DRIVE_DB`] at 127), and no sample leaves above [`CEILING`]. The gain each sample
//! needs is held over the look-ahead window and averaged over it, so the gain is down by the
//! time a peak leaves the delay line ([`LOOK`] - 1 samples later) and moves without clicks; it
//! comes back up with a [`RELEASE_MS`] release. Switching on or off fades over [`LOOK`]
//! samples between the master as it is and the limited, delayed master.
//!
//! The delay line follows the master also while the limiter is off, so switching on starts
//! from the present sound (the gain starts at 1; the ceiling holds from the first sample).
//! Off (and faded out) the master is left as it is, bit for bit the original's.

use crate::frame::BLOCK;

/// The look-ahead window, in samples (one block, 1.45 ms at 44,100 Hz).
pub const LOOK: usize = BLOCK;
/// The highest sample the limiter lets out, -0.3 dB below full scale (1.0).
pub const CEILING: f64 = 0.966_050_878_195_792_3;
/// The drive at AMT 127, in dB.
pub const MAX_DRIVE_DB: f64 = 12.0;
/// The release, the time the gain takes to come back up by 1/e of the way.
pub const RELEASE_MS: f64 = 60.0;
/// The sample rate.
const RATE: f64 = 44_100.0;
/// The drive's smoothing per sample (a knob turned does not step the level).
const DRIVE_KEEP: f64 = 0.999;

fn st(x: f64) -> f32 {
    x as f32
}

/// The drive factor of AMT `amt` (0..=127).
pub fn drive(amt: u8) -> f64 {
    let db = MAX_DRIVE_DB * amt.min(127) as f64 / 127.0;
    10f64.powf(db / 20.0)
}

/// The limiter's state.
#[derive(Debug, Clone)]
pub struct Limiter {
    /// The driven master, left and right, a ring of [`LOOK`] samples.
    delay: [[f64; LOOK]; 2],
    /// The gain each of the last [`LOOK`] samples needs.
    need: [f64; LOOK],
    /// The released gain of the last [`LOOK`] samples, and its sum.
    released: [f64; LOOK],
    sum: f64,
    /// The ring position (the slot written next).
    pos: usize,
    /// The release follower.
    gain: f64,
    /// The drive now (it follows AMT's).
    drive: f64,
    /// 0 off .. 1 on: the fade between the master as it is and the limited one.
    mix: f64,
    release: f64,
}

impl Default for Limiter {
    fn default() -> Self {
        Limiter {
            delay: [[0.0; LOOK]; 2],
            need: [1.0; LOOK],
            released: [1.0; LOOK],
            sum: LOOK as f64,
            pos: 0,
            gain: 1.0,
            drive: 1.0,
            mix: 0.0,
            release: (-1.0 / (RELEASE_MS * 0.001 * RATE)).exp(),
        }
    }
}

impl Limiter {
    /// One block of the master, `l` and `r`, in place: limited when `on`, AMT `amt`.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], on: bool, amt: u8) {
        let target = drive(amt);
        let fade = 1.0 / LOOK as f64;
        if !on && self.mix == 0.0 {
            // off and faded out: only the delay line follows (so switching on starts from the
            // present sound); the gain waits at 1
            for (xl, xr) in l.iter().zip(r.iter()) {
                self.drive = self.drive * DRIVE_KEEP + target * (1.0 - DRIVE_KEEP);
                let i = self.pos;
                self.delay[0][i] = *xl as f64 * self.drive;
                self.delay[1][i] = *xr as f64 * self.drive;
                self.pos = (i + 1) % LOOK;
            }
            self.need = [1.0; LOOK];
            self.released = [1.0; LOOK];
            self.sum = LOOK as f64;
            self.gain = 1.0;
            return;
        }
        for (xl, xr) in l.iter_mut().zip(r.iter_mut()) {
            self.drive = self.drive * DRIVE_KEEP + target * (1.0 - DRIVE_KEEP);
            let (dl, dr) = (*xl as f64 * self.drive, *xr as f64 * self.drive);
            let peak = dl.abs().max(dr.abs());
            let i = self.pos;
            self.need[i] = if peak > CEILING { CEILING / peak } else { 1.0 };
            // the gain held over the window, released upwards, averaged over the window
            let hold = self.need.iter().copied().fold(1.0, f64::min);
            self.gain = if hold <= self.gain {
                hold
            } else {
                hold - (hold - self.gain) * self.release
            };
            self.sum += self.gain - self.released[i];
            self.released[i] = self.gain;
            self.delay[0][i] = dl;
            self.delay[1][i] = dr;
            self.pos = (i + 1) % LOOK;
            if self.pos == 0 {
                // the running sum anew, so that its rounding does not build up
                self.sum = self.released.iter().sum();
            }
            let g = self.sum / LOOK as f64;
            // the oldest sample in the ring, LOOK - 1 samples ago
            let o = self.pos;
            let ol = (self.delay[0][o] * g).clamp(-CEILING, CEILING);
            let or = (self.delay[1][o] * g).clamp(-CEILING, CEILING);
            self.mix = if on {
                (self.mix + fade).min(1.0)
            } else {
                (self.mix - fade).max(0.0)
            };
            if self.mix > 0.0 {
                let m = self.mix;
                *xl = st(*xl as f64 * (1.0 - m) + ol * m);
                *xr = st(*xr as f64 * (1.0 - m) + or * m);
            }
        }
    }

    /// Whether the limiter is heard (on, or still fading out).
    pub fn heard(&self) -> bool {
        self.mix > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(amp: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (amp * (i as f64 * 2.0 * std::f64::consts::PI * 220.0 / RATE).sin()) as f32)
            .collect()
    }

    fn run(lim: &mut Limiter, x: &[f32], on: bool, amt: u8) -> Vec<f32> {
        let mut l = x.to_vec();
        let mut r = x.to_vec();
        for (bl, br) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)) {
            lim.process(bl, br, on, amt);
        }
        assert_eq!(l, r);
        l
    }

    #[test]
    fn off_leaves_the_master_as_it_is() {
        let x = sine(1.7, 64 * 50);
        let mut lim = Limiter::default();
        assert_eq!(run(&mut lim, &x, false, 127), x);
        assert!(!lim.heard());
    }

    #[test]
    fn no_sample_leaves_above_the_ceiling() {
        // loud, and a sudden peak: after the fade in nothing passes the ceiling
        let mut x = sine(2.5, 64 * 200);
        x[64 * 120 + 7] = 9.0;
        let mut lim = Limiter::default();
        let y = run(&mut lim, &x, true, 127);
        let top = y[LOOK..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(top as f64 <= CEILING + 1e-6, "{top}");
        assert!(top as f64 > CEILING - 0.01, "{top}");
    }

    #[test]
    fn a_quiet_master_is_only_delayed_at_amt_0() {
        let x = sine(0.3, 64 * 40);
        let mut lim = Limiter::default();
        let y = run(&mut lim, &x, true, 0);
        for i in 64 * 10..x.len() {
            assert!((y[i] - x[i - (LOOK - 1)]).abs() < 1e-6, "{i}");
        }
    }

    #[test]
    fn amt_drives_the_master_up_to_12_db() {
        let x = sine(0.1, 64 * 400);
        let mut lim = Limiter::default();
        let y = run(&mut lim, &x, true, 127);
        let top = |v: &[f32]| v.iter().fold(0.0f32, |m, s| m.max(s.abs())) as f64;
        let ratio = top(&y[64 * 300..]) / top(&x[64 * 300..]);
        assert!((ratio - drive(127)).abs() < 0.01, "{ratio}");
        assert!((drive(127) - 3.981).abs() < 0.001);
    }

    #[test]
    fn switching_fades_without_a_jump() {
        let x = sine(0.5, 64 * 40);
        let mut lim = Limiter::default();
        let mut l = x.clone();
        let mut r = x.clone();
        for (k, (bl, br)) in l.chunks_mut(BLOCK).zip(r.chunks_mut(BLOCK)).enumerate() {
            lim.process(bl, br, (10..25).contains(&k), 0);
        }
        let jump = l.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        let step = x.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(jump < step * 1.5, "{jump} vs {step}");
        // faded out again: the master as it is
        assert_eq!(&l[64 * 30..], &x[64 * 30..]);
    }
}
