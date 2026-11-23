//! This fork (not the original): a drum machine's sounds each with their own pan and their own
//! effects switch (the panel's FLIP face).
//!
//! The original renders all of a drum machine's voices into one mono channel bus, which goes
//! through the channel's inserts (distortion, PCF, compressor), its delay send and its pan. Here,
//! while a drum machine's [`VoiceMix`] is not neutral, its voices are rendered one by one
//! ([`super::ChannelDevice::process_voices`]) into four buses: the voices with their effects on
//! into a left and a right "wet" bus, the others into a left and a right "dry" bus, each voice
//! by its own pan. The wet buses go through the inserts (the left one through the song's
//! inserts, the right one through a copy of them, [`RightInserts`]) and into the delay send;
//! the dry ones go around both. Then the channel's level and pan apply as for every channel.
//!
//! The pan is a balance: in the middle both sides take the voice whole, so a neutral mix (every
//! pan in the middle, every effect on) is the original's sound, and the engine then takes the
//! original's path, bit for bit.

use super::{BLOCK, Block, BlockContext, Devices, InsertEffect};

/// The pan in the middle (the panel's knob at its default).
pub const PAN_CENTER: u8 = 64;

/// A drum machine's sounds' pans and effects switches, by the panel's instrument index
/// (1 BD .. 11; 0, the accent, has no sound of its own and is not used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceMix {
    /// 0 left .. 64 the middle .. 127 right.
    pub pan: [u8; 12],
    /// Off: the sound goes around the channel's inserts and its delay send.
    pub fx: [bool; 12],
}

impl Default for VoiceMix {
    fn default() -> Self {
        VoiceMix {
            pan: [PAN_CENTER; 12],
            fx: [true; 12],
        }
    }
}

impl VoiceMix {
    /// Every pan in the middle and every effect on: the original's sound.
    pub fn neutral(&self) -> bool {
        *self == VoiceMix::default()
    }

    /// The left and right gains of instrument `inst`: a balance, 1 on both sides in the middle,
    /// the other side going down to 0 toward either end.
    pub fn gains(&self, inst: usize) -> (f64, f64) {
        let p = self.pan.get(inst).copied().unwrap_or(PAN_CENTER).min(127) as f64;
        let c = PAN_CENTER as f64;
        let l = if p <= c {
            1.0
        } else {
            (127.0 - p) / (127.0 - c)
        };
        let r = if p >= c { 1.0 } else { p / c };
        (l, r)
    }
}

/// The four buses of a split drum machine.
pub struct SplitBuses {
    pub wet_l: Block,
    pub wet_r: Block,
    pub dry_l: Block,
    pub dry_r: Block,
}

impl SplitBuses {
    pub fn new() -> Self {
        SplitBuses {
            wet_l: [0.0; BLOCK],
            wet_r: [0.0; BLOCK],
            dry_l: [0.0; BLOCK],
            dry_r: [0.0; BLOCK],
        }
    }

    /// Adds instrument `inst`'s voice block `v` by `mix`.
    pub fn add(&mut self, mix: &VoiceMix, inst: usize, v: &Block) {
        let (gl, gr) = mix.gains(inst);
        let fx = mix.fx.get(inst).copied().unwrap_or(true);
        let (l, r) = if fx {
            (&mut self.wet_l, &mut self.wet_r)
        } else {
            (&mut self.dry_l, &mut self.dry_r)
        };
        for ((a, b), &x) in l.iter_mut().zip(r.iter_mut()).zip(v.iter()) {
            *a = (*a as f64 + x as f64 * gl) as f32;
            *b = (*b as f64 + x as f64 * gr) as f32;
        }
    }
}

impl Default for SplitBuses {
    fn default() -> Self {
        Self::new()
    }
}

/// The copies of the distortion, the PCF and the compressor the right wet buses go through:
/// copied from the song's on first use, then given every parameter change of the song's
/// (the live controller bytes, through slot 11) and every mixer update, with their own
/// signal state.
/// This fork: a channel's own copy of an insert (the PCF, the compressor) on a channel besides
/// the one the original's id chooses ([`super::MixerParams::more`]). It follows the song's
/// insert's controllers; its meters and faults are not the song's.
pub struct InsertCopy {
    pub ins: Box<dyn InsertEffect>,
    live: Vec<u8>,
}

impl InsertCopy {
    /// A copy of `src` as it is now; `None` for an insert that cannot be copied.
    pub fn new(src: &dyn InsertEffect) -> Option<Self> {
        Some(InsertCopy {
            ins: src.fork_clone()?,
            live: src.live().to_vec(),
        })
    }

    /// The song's insert's changed controller bytes, into the copy.
    pub fn follow(&mut self, src: &dyn InsertEffect) {
        for (i, (&v, old)) in src.live().iter().zip(self.live.iter_mut()).enumerate() {
            if v != *old {
                self.ins.set_param(i as u8, v);
                *old = v;
            }
        }
    }
}

pub struct RightInserts {
    pub ins: [Option<Box<dyn InsertEffect>>; 3],
    live: [Vec<u8>; 3],
}

impl RightInserts {
    pub fn new(d: &Devices) -> Self {
        let src = [&d.dist, &d.pcf, &d.comp];
        RightInserts {
            ins: src.map(|i| i.fork_clone()),
            live: src.map(|i| i.live().to_vec()),
        }
    }

    /// The song's inserts' changed controller bytes, into the copies.
    pub fn follow(&mut self, d: &Devices) {
        let src = [&d.dist, &d.pcf, &d.comp];
        for ((ins, live), main) in self.ins.iter_mut().zip(&mut self.live).zip(src) {
            let Some(ins) = ins else { continue };
            for (i, (&v, old)) in main.live().iter().zip(live.iter_mut()).enumerate() {
                if v != *old {
                    ins.set_param(i as u8, v);
                    *old = v;
                }
            }
        }
    }

    /// Every mixer update, as the song's inserts get it.
    pub fn update(&mut self, ctx: &BlockContext) {
        for i in self.ins.iter_mut().flatten() {
            i.update(ctx);
        }
    }

    /// Insert `k` (0 distortion, 1 PCF, 2 compressor) on `bus`; a copy that cannot be made
    /// (a placeholder) leaves the bus as it is. Its faults are dropped: the original has no
    /// such call.
    pub fn process(&mut self, k: usize, bus: &mut Block, ctx: &BlockContext) {
        if let Some(i) = &mut self.ins[k] {
            i.process(bus, ctx);
            i.faulted();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_middle_takes_the_voice_whole_and_the_ends_one_side() {
        let mut m = VoiceMix::default();
        assert!(m.neutral());
        assert_eq!(m.gains(1), (1.0, 1.0));
        m.pan[1] = 0;
        assert_eq!(m.gains(1), (1.0, 0.0));
        m.pan[1] = 127;
        assert_eq!(m.gains(1), (0.0, 1.0));
        assert!(!m.neutral());
        let mut m = VoiceMix::default();
        m.fx[3] = false;
        assert!(!m.neutral());
    }

    #[test]
    fn a_sound_without_effects_goes_to_the_dry_buses() {
        let mut m = VoiceMix::default();
        m.fx[2] = false;
        m.pan[2] = 0;
        let mut b = SplitBuses::new();
        let v = [0.5f32; BLOCK];
        b.add(&m, 1, &v);
        b.add(&m, 2, &v);
        assert_eq!((b.wet_l[0], b.wet_r[0]), (0.5, 0.5));
        assert_eq!((b.dry_l[0], b.dry_r[0]), (0.5, 0.0));
    }
}
