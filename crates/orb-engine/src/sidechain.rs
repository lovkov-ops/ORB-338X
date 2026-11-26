//! This fork: the sidechain of the 303s' mixer channels, the SC knob and the 808 / 909 lamp
//! on their COMP faces. Not a device of the original: the mixer keeps its controllers
//! ([`crate::frame::SC`]) and the frame runs it on the channel after its EQ, before its meter,
//! its sends, its pan and its fader.
//!
//! Every BD the chosen drum machine triggers (heard or not: a muted or silent BD still counts,
//! a "ghost" kick) dips the channel: down in [`ATTACK_BEATS`] of a beat, back up along half a
//! cosine in [`RELEASE_BEATS`], both following the song's tempo. The SC knob is how deep the
//! dip goes: 0 none (the channel untouched, bit for bit), full the channel silent at the dip's
//! bottom.

use crate::frame::Block;

/// The sample rate.
const RATE: f64 = 44_100.0;
/// How long the dip takes to reach its bottom, in beats (1/64 of a beat: 8 ms at 120 BPM).
pub const ATTACK_BEATS: f64 = 1.0 / 64.0;
/// How long the channel takes to come back up, in beats (an eighth note).
pub const RELEASE_BEATS: f64 = 0.5;

/// One channel's sidechain state.
#[derive(Debug, Clone, Default)]
pub struct Ducker {
    /// How far down the channel was at the last sample, 0 (not at all) .. 1 (the bottom).
    env: f64,
    /// The samples since the last hit, while its dip lasts.
    at: Option<f64>,
    /// How far down the channel was at the hit: the fall starts there.
    from: f64,
}

impl Ducker {
    /// A BD hit: a new dip, from where the channel is now.
    pub fn hit(&mut self) {
        self.from = self.env;
        self.at = Some(0.0);
    }

    /// How far down (0..1) the dip is `t` samples after its hit, given its fall and rise in
    /// samples; `None` once it is over.
    fn depth_at(&self, t: f64, attack: f64, release: f64) -> Option<f64> {
        if t < attack {
            Some(self.from + (1.0 - self.from) * t / attack)
        } else if t < attack + release {
            let x = (t - attack) / release;
            Some(0.5 * (1.0 + (std::f64::consts::PI * x).cos()))
        } else {
            None
        }
    }

    /// One block of the channel's bus: the dip goes on (whatever `amount`), and the bus takes
    /// it when `amount` (the SC knob, 0..=127) is above 0. `tempo` is the song's BPM*1000.
    pub fn process(&mut self, bus: &mut Block, amount: u8, tempo: u32) {
        let Some(mut t) = self.at else {
            self.env = 0.0;
            return;
        };
        let beat = RATE * 60_000.0 / tempo.max(1) as f64;
        let attack = (beat * ATTACK_BEATS).max(1.0);
        let release = (beat * RELEASE_BEATS).max(1.0);
        let depth = amount.min(127) as f64 / 127.0;
        for x in bus.iter_mut() {
            let Some(env) = self.depth_at(t, attack, release) else {
                self.at = None;
                self.env = 0.0;
                return;
            };
            self.env = env;
            if depth > 0.0 {
                *x = (*x as f64 * (1.0 - depth * env)) as f32;
            }
            t += 1.0;
        }
        self.at = Some(t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::BLOCK;

    /// 120 BPM.
    const TEMPO: u32 = 120_000;

    #[test]
    fn without_a_hit_or_at_no_amount_the_bus_is_untouched() {
        let mut d = Ducker::default();
        let mut bus = [0.5f32; BLOCK];
        d.process(&mut bus, 127, TEMPO);
        assert_eq!(bus, [0.5; BLOCK]);
        d.hit();
        d.process(&mut bus, 0, TEMPO);
        assert_eq!(bus, [0.5; BLOCK]);
    }

    #[test]
    fn a_hit_dips_the_bus_and_it_comes_back_with_the_tempo() {
        let mut d = Ducker::default();
        d.hit();
        let mut levels = vec![];
        // half a beat and a bit at 120 BPM: 11025 samples and more
        for _ in 0..200 {
            let mut bus = [1.0f32; BLOCK];
            d.process(&mut bus, 127, TEMPO);
            levels.extend(bus);
        }
        // the bottom at 1/64 of a beat (about 344 samples): silent at the full amount
        let attack = (RATE * 0.5 * ATTACK_BEATS) as usize;
        assert!(levels[attack + 1] < 0.01, "{}", levels[attack + 1]);
        assert!(levels[attack / 2] > 0.4 && levels[attack / 2] < 0.6);
        // half way up a quarter of a beat later, all the way after half a beat
        let mid = attack + (RATE * 0.5 * RELEASE_BEATS / 2.0) as usize;
        assert!((levels[mid] - 0.5).abs() < 0.01, "{}", levels[mid]);
        let end = attack + (RATE * 0.5 * RELEASE_BEATS) as usize + 2;
        assert!(levels[end..].iter().all(|&x| x == 1.0));
        // twice the tempo: the dip half as long
        let mut d = Ducker::default();
        d.hit();
        let mut fast = vec![];
        for _ in 0..200 {
            let mut bus = [1.0f32; BLOCK];
            d.process(&mut bus, 127, 2 * TEMPO);
            fast.extend(bus);
        }
        assert!(fast[end / 2 + 2..].iter().all(|&x| x == 1.0));
    }

    #[test]
    fn the_amount_sets_how_deep_the_dip_goes() {
        let mut d = Ducker::default();
        d.hit();
        let mut levels = vec![];
        for _ in 0..10 {
            let mut bus = [1.0f32; BLOCK];
            d.process(&mut bus, 64, TEMPO);
            levels.extend(bus);
        }
        let low = levels.iter().cloned().fold(1.0f32, f32::min);
        assert!((low - (1.0 - 64.0 / 127.0) as f32).abs() < 1e-3, "{low}");
    }
}
