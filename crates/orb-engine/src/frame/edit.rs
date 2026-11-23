//! The panel's live device messages (task-055): song msg 41 with a device's own sub id, as the
//! exe's panel sends them while a song plays or is edited (`docs/song_load.md` section 4.10).
//!
//! Each entry point of [`Engine`] here is one message (or one message shape shared by the three
//! pattern devices) and does what the DLL's handler does: byte stores into the live parameter
//! block or into the device's pattern array. None of them computes anything.
//!
//! **Record.** The controller messages call the record hook `sub_10012BC0` → `sub_10021340`
//! after their store, with the message's controller and value as they are; it records the
//! change into the device's track only in song mode with record on (globals bytes 0 and 2,
//! [`crate::seq::Sequencer::record_hook`], task-057). Which controllers reach the hook is
//! [`hooks`]'s.
//!
//! **Bounds.** The DLL checks the field of a 303 step (0..5) and of a pattern header (0, 1) and
//! nothing else: a pattern above 31, a step above 15 or a drum instrument above 11 computes an
//! address `base + len * pattern + ...` that may lie in another step, another pattern or past the
//! pattern array. Within the array orb stores at that same address; past it (the live block and
//! the device's other state in the original) the store is dropped. The panel sends only the
//! device's current pattern (0..31), steps 0..15 and instruments 0..11.

use crate::song::Device;

use super::Engine;

/// The pattern array of a pattern device: [`PATTERNS`] patterns of `len` bytes, each a shuffle
/// flag, a length and 16 steps of `step` bytes (`docs/song_load.md` sections 4.6 to 4.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternShape {
    pub len: usize,
    pub step: usize,
}

/// The patterns of a pattern device.
pub const PATTERNS: usize = 32;

impl PatternShape {
    /// The 303s' 98-byte patterns of 6-byte steps `{note, slide, accent, up, down, rest}`, the
    /// drum machines' 194-byte patterns of 12-byte steps (one byte per instrument); `None` for the
    /// mixer and the effects.
    pub const fn of(d: Device) -> Option<PatternShape> {
        match d {
            Device::Tb303_1 | Device::Tb303_2 => Some(PatternShape {
                len: crate::tb303::PATTERN_LEN,
                step: 6,
            }),
            Device::Tr808 => Some(PatternShape {
                len: crate::tr808::PATTERN_LEN,
                step: 12,
            }),
            Device::Tr909 => Some(PatternShape {
                len: crate::tr909::PATTERN_LEN,
                step: 12,
            }),
            _ => None,
        }
    }

    /// The offset of step `s`'s byte `i` of pattern `p` from the array's start: the DLL's
    /// `2 + len*p + step*s + i`, unbounded.
    fn step_at(self, p: u8, s: u8, i: usize) -> usize {
        self.len * p as usize + 2 + self.step * s as usize + i
    }
}

/// The step-field bound of a device: the 303's handler takes fields 0..5 only (the switch at
/// 0x10025FD4, `ja` to the default); the drum machines' take any instrument byte (0x10028F9D,
/// 0x1002C61E).
fn field_ok(d: Device, field: u8) -> bool {
    match d {
        Device::Tb303_1 | Device::Tb303_2 => field <= 5,
        _ => true,
    }
}

/// Whether the `{ctrl, v}` message of device `d` calls the record hook for `ctrl`: the 303's sub
/// 40 for controllers 0..8 only (the cases of the switch at 0x10025D27 that call
/// `sub_10012BC0`, 0x10025D5A..0x10025EF4; 9, 10 and above 10 do not), the 808's 57 for 0..28
/// (0x10028923..0x10028E9C; 29, 30 and above do not), the 909's 74 for 0..29 (0x1002BF70..
/// 0x1002C51D), and the effects' subs 84, 92, 102 and 114 for every controller, the ones their
/// switch does not store included (0x1000FD6C, 0x10010F66, 0x1000F1CE, 0x10016E8A).
pub fn hooks(d: Device, ctrl: u8) -> bool {
    match d {
        Device::Tb303_1 | Device::Tb303_2 => {
            ctrl as usize <= crate::tb303::WAVEFORM || ctrl == crate::tb303::SUB
        }
        Device::Tr808 => ctrl as usize <= crate::tr808::LAST_CTRL,
        Device::Tr909 => ctrl as usize <= crate::tr909::LAST_CTRL,
        Device::Mixer => false,
        Device::Delay | Device::Distortion | Device::Compressor | Device::Pcf => true,
    }
}

impl Engine {
    /// This fork: before the first recorded move of a controller this fork adds (the 303's
    /// sub-oscillator, the master's limiter), its value so far becomes the track's start value
    /// (a tick-0 event, as every original controller has from the song ctor), so the song plays
    /// it up to the move and a locate before it chases back. A song that never records it has
    /// no such event.
    fn fork_start(&mut self, d: Device, ctrl: u8) {
        let f = &mut self.frame;
        if f.seq.globals.song_mode == 0 || f.seq.globals.byte2 == 0 {
            return;
        }
        let i = d.id() as usize;
        if f.seq.events(i).iter().any(|e| e.ctrl == ctrl) {
            return;
        }
        let Some(v) = super::live_value(&mut f.devices, d, ctrl) else {
            return;
        };
        if f.seq.track_mut(i).insert(0, ctrl, v) {
            f.seq.revision += 1;
        }
    }

    /// The record hook for device `d` ([`crate::seq::Sequencer::record_hook`]).
    fn record_hook(&mut self, d: Device, ctrl: u8, value: u8) {
        self.frame.seq.record_hook(d, ctrl, value);
    }

    /// The pattern array of device `d` and its shape, if `d` is an implemented pattern device.
    fn patterns_of(&mut self, d: Device) -> Option<(&mut [u8], PatternShape)> {
        let shape = PatternShape::of(d)?;
        let c = match d {
            Device::Tb303_1 => 0,
            Device::Tb303_2 => 1,
            Device::Tr808 => 2,
            _ => 3,
        };
        let p = self.frame.devices.channels[c].patterns_mut()?;
        Some((p, shape))
    }

    /// The byte at `at` of device `d`'s pattern array, if inside it.
    fn pattern_byte(&mut self, d: Device, at: usize) -> Option<&mut u8> {
        self.patterns_of(d)?.0.get_mut(at)
    }

    /// The `{ctrl, v}` message of device `d`, the panel's knobs and switches: 303 sub 40
    /// (0x10025CFC), 808 57 (0x100288C5), 909 74 (0x1002BF12), delay 84 (0x1000FCE5), dist 92
    /// (0x10010EF1), compressor 102 (0x1000F178), PCF 114 (0x10016DFB). For the controllers the
    /// device's slot 11 accepts it is slot 11's store ([`super::SongDevice::param_message`]);
    /// the pattern devices also store one byte beyond their last step byte (303 controller 10,
    /// 808 30, 909 31). Then the record hook for the controllers [`hooks`] names. The mixer has
    /// no such message (its are [`Engine::mixer_global`] and [`Engine::mixer_channel`]).
    pub fn device_param(&mut self, d: Device, ctrl: u8, value: u8) {
        if matches!(d, Device::Tb303_1 | Device::Tb303_2) && ctrl == crate::tb303::SUB {
            self.fork_start(d, crate::tb303::SUB);
        }
        if d != Device::Mixer {
            self.frame.devices.device_mut(d).param_message(ctrl, value);
        }
        if hooks(d, ctrl) {
            self.record_hook(d, ctrl, value);
        }
    }

    /// This fork (not a message of the original): a 303's sub-oscillator as a song keeps it
    /// (its `.rbsx` part): the level, live and in the pattern-mode copy, and the track events
    /// `(tick, level)` song mode plays. Other devices: nothing.
    pub fn sub(&mut self, d: Device, level: u8, events: &[(i32, u8)]) {
        if matches!(d, Device::Tb303_1 | Device::Tb303_2) {
            self.fork_track(d, crate::tb303::SUB, level, events);
        }
    }

    /// This fork (not a message of the original): a controller this fork adds to device `d`
    /// (the 303's [`crate::tb303::SUB`], the mixer's [`super::LIMIT`] and
    /// [`super::LIMIT_AMT`]) as a song keeps it (its `.rbsx` part): the value, live and in the
    /// pattern-mode copy, and the track events `(tick, value)` song mode plays. A controller
    /// the device does not have: nothing.
    pub fn fork_track(&mut self, d: Device, ctrl: u8, value: u8, events: &[(i32, u8)]) {
        let f = &mut self.frame;
        let dev = f.devices.device_mut(d);
        if dev.fork_param(ctrl).is_none() {
            return;
        }
        dev.fork_set(ctrl, value);
        let i = d.id() as usize;
        for &(tick, v) in events {
            f.seq.track_mut(i).insert(tick.max(0), ctrl, v.min(127));
        }
        if !events.is_empty() {
            f.seq.revision += 1;
        }
    }

    /// This fork ([`super::voices`], not a message of the original): the pans and effects
    /// switches of the 808's (`d` [`Device::Tr808`]) or the 909's sounds. Other devices: nothing.
    pub fn voice_mix(&mut self, d: Device, mix: super::voices::VoiceMix) {
        match d {
            Device::Tr808 => self.frame.voice_mix[0] = mix,
            Device::Tr909 => self.frame.voice_mix[1] = mix,
            _ => {}
        }
    }

    /// Mixer msg 145 `{id, v}` (0x10015900, sender `sub_4131BC`): id 0 the live master
    /// (0x1001592C, no record hook), 1 the compressor id and 2 the PCF id (0x10015947,
    /// 0x10015989: slot 11's controllers 1 and 2, then the record hook with that controller,
    /// 0x1001596C, 0x100159AE), 3 and 4 nothing. This fork: [`super::LIMIT`] and
    /// [`super::LIMIT_AMT`], the master's limiter, and [`super::WET`].., the inserts' wet
    /// amounts, as 1 and 2 (song mode records them).
    pub fn mixer_global(&mut self, id: u8, value: u8) {
        let fork = super::is_fork_mixer(id);
        if fork {
            self.fork_start(Device::Mixer, id);
        }
        let m = &mut self.frame.devices.mixer;
        match id {
            0 => m.set_master(value),
            _ if id == 1 || id == 2 || fork => {
                super::SongDevice::set_param(m, id, value);
                self.record_hook(Device::Mixer, id, value);
            }
            _ => {}
        }
    }

    /// Mixer msg 147 `{c, k, v}` (0x100159DB, sender `sub_413206`): field 0 channel c's live
    /// enabled byte (0x10015A1F, the mute, no record hook); fields 1..4 its level, pan, delay send
    /// and dist switch (`+0x41..+0x44 + 6c`: slot 11's controller `5 + 6c + k`, then the record
    /// hook with that controller, 0x10015A75..); field 5 nothing. The DLL does not bound c; a
    /// channel above 3 is dropped (it would write past the channel blocks; the panel sends
    /// 0..3).
    pub fn mixer_channel(&mut self, c: u8, k: u8, value: u8) {
        if c > 3 {
            return;
        }
        let m = &mut self.frame.devices.mixer;
        match k {
            0 => m.set_enabled(c as usize, value),
            1..=4 => {
                let ctrl = 5 + 6 * c + k;
                super::SongDevice::set_param(m, ctrl, value);
                self.record_hook(Device::Mixer, ctrl, value);
            }
            _ => {}
        }
    }

    /// The pattern-header message `{p, field, v}`: 303 sub 42 (0x10025F25), 808 59 (0x10028EFE),
    /// 909 76 (0x1002C57F); senders `sub_43ECCA`, `sub_441CC9`, `sub_445C88` (the pattern
    /// section's Shuffle button and Steps arrows). Field 0 is pattern p's shuffle flag, 1 its
    /// length; another field stores nothing. The step functions read the header live, so the
    /// change is heard from the next step.
    pub fn pattern_field(&mut self, d: Device, p: u8, field: u8, value: u8) {
        let Some(shape) = PatternShape::of(d) else {
            return;
        };
        if field <= 1
            && let Some(b) = self.pattern_byte(d, shape.len * p as usize + field as usize)
        {
            *b = value;
        }
    }

    /// The whole-header message `{p, shuffle, length}`: 303 sub 43 (0x10025F62), 808 60
    /// (0x10028F3B), 909 77 (0x1002C5BC), all `sub_100191E0`. The exe does not send it.
    pub fn pattern_header(&mut self, d: Device, p: u8, header: [u8; 2]) {
        for (field, v) in header.into_iter().enumerate() {
            self.pattern_field(d, p, field as u8, v);
        }
    }

    /// The whole-pattern message `{p, bytes}`: 303 sub 44 (0x10025F93, `sub_10018DC0`, 98 bytes),
    /// 808 61 (0x10028F6C, `sub_100192C0`, 194 bytes), 909 78 (0x1002C5ED, the same); senders
    /// `sub_43F617`, `sub_443257`, `sub_4471C5` (the 303's Clear, and the Edit menu's pattern
    /// functions `sub_42B319`, `sub_42B928`, `sub_42C204`). `bytes` is the pattern as the data
    /// block holds it; one of another length is ignored.
    pub fn pattern(&mut self, d: Device, p: u8, bytes: &[u8]) {
        let Some((pats, shape)) = self.patterns_of(d) else {
            return;
        };
        let at = shape.len * p as usize;
        if bytes.len() == shape.len
            && let Some(dst) = pats.get_mut(at..at + shape.len)
        {
            dst.copy_from_slice(bytes);
        }
    }

    /// The step-field message `{p, s, field, v}`: 303 sub 45 (0x10025FC4: field 0 note, 1 slide,
    /// 2 accent, 3 up, 4 down, 5 rest; another field stores nothing), 808 62 (0x10028F9D) and
    /// 909 79 (0x1002C61E: field = instrument in RBS column order, the value 0/1 for the 808,
    /// 0..3 for the 909); senders `sub_43ED1D` (the 303's step editor), `sub_441D1C` and
    /// `sub_445CDB` (the drums' step buttons and tap keys, through `sub_44305C` and
    /// `sub_447153`). Heard the next time the step plays.
    pub fn step_field(&mut self, d: Device, p: u8, s: u8, field: u8, value: u8) {
        let Some(shape) = PatternShape::of(d) else {
            return;
        };
        if field_ok(d, field)
            && let Some(b) = self.pattern_byte(d, shape.step_at(p, s, field as usize))
        {
            *b = value;
        }
    }

    /// The whole-step message `{p, s, bytes}`: 303 sub 46 (0x10026112, `sub_10019B00`, 6 bytes),
    /// 808 63 (0x10028FE2, `sub_10019840`, 12 bytes), 909 80 (0x1002C663, the same). The exe
    /// does not send it. `bytes` of another length than the device's step is ignored.
    pub fn step(&mut self, d: Device, p: u8, s: u8, bytes: &[u8]) {
        let Some(shape) = PatternShape::of(d) else {
            return;
        };
        if bytes.len() != shape.step {
            return;
        }
        for (i, &v) in bytes.iter().enumerate() {
            if let Some(b) = self.pattern_byte(d, shape.step_at(p, s, i)) {
                *b = v;
            }
        }
    }
}
