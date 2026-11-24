//! This fork (not the original): a drum machine's sounds each with their own pan and their own
//! lamps, one for each effects slot (the panel's FLIP face).
//!
//! The original renders all of a drum machine's voices into one mono channel bus, which goes
//! through the channel's inserts (distortion, PCF, compressor), its delay send and its pan. Here,
//! while a drum machine's [`VoiceMix`] is not neutral, its voices are rendered one by one
//! ([`super::ChannelDevice::process_voices`]) into a left and a right bus for each set of lamps
//! ([`SplitBuses`]), each voice by its own pan. The voices with every lamp on go through the
//! inserts as the channel's sound does (the left side through the song's inserts, the right
//! one through a copy of them, [`RightInserts`]); the voices of another set go through their
//! own copies of the inserts in their lit slots ([`GroupChain`]), around the others. Each slot's
//! send effect hears the voices whose lamp of that slot is on. Then the channel's level and
//! pan apply as for every channel.
//!
//! The pan is a balance: in the middle both sides take the voice whole, so a neutral mix (every
//! pan in the middle, every lamp on) is the original's sound, and the engine then takes the
//! original's path, bit for bit.

use super::{BLOCK, Block, BlockContext, Devices, InsertEffect};

/// The pan in the middle (the panel's knob at its default).
pub const PAN_CENTER: u8 = 64;

/// Every slot's lamp on (bit s the slot s, top to bottom).
pub const ALL: u8 = 0b1111;

/// A drum machine's sounds' pans and slot lamps, by the panel's instrument index
/// (1 BD .. 11; 0, the accent, has no sound of its own and is not used).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceMix {
    /// 0 left .. 64 the middle .. 127 right.
    pub pan: [u8; 12],
    /// The lamps (bit s the effects slot s): off, the sound goes around that slot's insert, or
    /// does not go into its send effect.
    pub lamps: [u8; 12],
}

impl Default for VoiceMix {
    fn default() -> Self {
        VoiceMix {
            pan: [PAN_CENTER; 12],
            lamps: [ALL; 12],
        }
    }
}

impl VoiceMix {
    /// Every pan in the middle and every lamp on: the original's sound.
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

/// The buses of a split drum machine: the left and right of the voices with every lamp on
/// (`wet_l`, `wet_r`, then the channel's sides), of each other set of lamps (`parts[set]`),
/// and each slot's send source (`sends[s]`, made when the sets are summed).
pub struct SplitBuses {
    pub wet_l: Block,
    pub wet_r: Block,
    pub parts: [[Block; 2]; ALL as usize],
    /// The sets of `parts` a voice went into (bit set).
    pub used: u16,
    pub sends: [Block; 4],
}

impl SplitBuses {
    pub fn new() -> Self {
        SplitBuses {
            wet_l: [0.0; BLOCK],
            wet_r: [0.0; BLOCK],
            parts: [[[0.0; BLOCK]; 2]; ALL as usize],
            used: 0,
            sends: [[0.0; BLOCK]; 4],
        }
    }

    /// Adds instrument `inst`'s voice block `v` by `mix`.
    pub fn add(&mut self, mix: &VoiceMix, inst: usize, v: &Block) {
        let (gl, gr) = mix.gains(inst);
        let set = mix.lamps.get(inst).copied().unwrap_or(ALL) & ALL;
        let (l, r) = if set == ALL {
            (&mut self.wet_l, &mut self.wet_r)
        } else {
            self.used |= 1 << set;
            let [l, r] = &mut self.parts[set as usize];
            (l, r)
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
        self.set_params(src.live());
    }

    /// The changed bytes of `params` (the first controllers), into the copy (this fork: a
    /// second insert's own controllers, [`super::MixerParams::wet_b`]).
    pub fn set_params(&mut self, params: &[u8]) {
        for (i, (&v, old)) in params.iter().zip(self.live.iter_mut()).enumerate() {
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

/// This fork: the inserts a set of a split drum machine's lamps (not all on) goes through:
/// for each slot its lamp of holds an insert the channel hears, that insert's copies on the
/// left and the right side (made from the song's insert, given the first or the second one's
/// controllers), and the DIST kinds' and the OTT's memory of each.
#[derive(Default)]
pub struct GroupChain {
    pub ins: [[Option<Box<InsertCopy>>; 2]; 4],
    pub kind: [[crate::dist_kinds::KindFx; 2]; 4],
    pub ott: [[crate::ott::Ott; 2]; 4],
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
        m.lamps[3] = 0b0101;
        assert!(!m.neutral());
    }

    #[test]
    fn a_sound_goes_to_the_buses_of_its_lamps() {
        let mut m = VoiceMix::default();
        m.lamps[2] = 0;
        m.pan[2] = 0;
        m.lamps[3] = 0b0110;
        let mut b = SplitBuses::new();
        let v = [0.5f32; BLOCK];
        b.add(&m, 1, &v);
        b.add(&m, 2, &v);
        b.add(&m, 3, &v);
        assert_eq!((b.wet_l[0], b.wet_r[0]), (0.5, 0.5));
        assert_eq!((b.parts[0][0][0], b.parts[0][1][0]), (0.5, 0.0));
        assert_eq!(b.parts[6][1][0], 0.5);
        assert_eq!(b.used, 1 | 1 << 6);
    }
}
