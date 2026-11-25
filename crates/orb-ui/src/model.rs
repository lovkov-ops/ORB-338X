//! The panel's state and what its controls do to it (docs/ui_spec.md sections 3-9, 11).
//! The model is the UI side only: every change that the engine must hear is also pushed to
//! `Panel::events`, which task-045 forwards to the player. Nothing here makes sound.

use std::collections::HashMap;

use orb_engine::frame::voices::VoiceMix;

use crate::behavior::{self, KNOB_MAX};

/// The drum machines' FLIP faces' sections (this fork), the 808's then the 909's.
pub const FLIP_FACES: [&str; 2] = ["tr808f", "tr909f"];

/// Ticks per bar.
pub const BAR: i32 = 768;
/// The last bar start (bar 999) in ticks.
pub const MAX_POS: i32 = 766_464;
const MAX_END: i32 = 767_232;

/// A change the engine must hear.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A control changed: the layout id and the value (knob and fader 0..=127, toggle and
    /// switch 0 or 1, display controls their number, the tempo in BPM * 1000, the
    /// positions in ticks).
    Set { id: String, value: i32 },
    /// A trigger button went down.
    Press { id: String },
    /// A pattern button selected `pattern` (0..=31) in row `row` (0..=3).
    Pattern { row: usize, pattern: u8 },
    Step808 {
        instrument: usize,
        step: usize,
        on: bool,
    },
    Step909 {
        instrument: usize,
        step: usize,
        value: u8,
    },
    /// A 303 step field changed (field 0 note, 1 slide, 2 accent, 3 up, 4 down, 5 rest).
    Step303 {
        synth: usize,
        pattern: u8,
        step: usize,
        field: u8,
        value: u8,
    },
    /// The Clear button emptied a 303 pattern (sent whole, msg 41 sub 44).
    Clear303 { synth: usize, pattern: u8 },
}

/// The transport (ticks, 768 per bar).
#[derive(Clone, Debug)]
pub struct Transport {
    /// BPM * 1000.
    pub tempo: i32,
    pub position: i32,
    pub loop_start: i32,
    pub loop_end: i32,
    pub song_mode: bool,
    pub playing: bool,
    pub recording: bool,
    pub looping: bool,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            tempo: 140_000,
            position: 0,
            loop_start: 0,
            loop_end: 4 * BAR,
            song_mode: false,
            playing: false,
            recording: false,
            looping: false,
        }
    }
}

impl Transport {
    pub fn tempo_step(&mut self, d: i32) {
        self.tempo = (self.tempo + d * 1000).clamp(20_000, 500_000);
    }

    /// Moves the position by `d` ticks from the start of the current bar (`sub_429DF2`).
    pub fn locate_by(&mut self, d: i32) {
        self.position = (d + BAR * (self.position / BAR)).clamp(0, MAX_POS);
    }

    /// Rewind and fast forward: 10 bars; they also turn record off.
    pub fn wind(&mut self, forward: bool) {
        self.recording = false;
        self.locate_by(if forward { 10 * BAR } else { -10 * BAR });
    }

    /// Bar up / down: one bar; down turns record off, up does not.
    pub fn bar_step(&mut self, up: bool) {
        if !up {
            self.recording = false;
        }
        self.locate_by(if up { BAR } else { -BAR });
    }

    /// Loop start up / down by `d` ticks (`sub_429E5B`); the length is kept.
    pub fn loop_start_step(&mut self, d: i32) {
        let len = self.loop_end - self.loop_start;
        let start = BAR * ((self.loop_start + d).clamp(0, MAX_POS) / BAR);
        self.loop_start = start;
        self.loop_end = (len + start).clamp(start + BAR, MAX_POS);
    }

    /// Loop length up / down by `d` ticks (`sub_429F18`), at least one bar.
    pub fn loop_length_step(&mut self, d: i32) {
        let hi = (MAX_END - self.loop_start).max(BAR);
        let len = BAR * ((self.loop_end - self.loop_start + d).clamp(BAR, hi) / BAR);
        self.loop_end = self.loop_start + len;
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop (`sub_429FE6`): record off; stops when playing; when stopped locates to the loop
    /// start, or to 0 when the position is at or before it.
    pub fn stop(&mut self) {
        self.recording = false;
        if self.playing {
            self.playing = false;
        } else if self.loop_start >= self.position {
            self.position = 0;
        } else {
            self.position = self.loop_start;
        }
    }

    pub fn tempo_display(&self) -> i32 {
        self.tempo / 1000
    }

    pub fn bar_display(&self) -> i32 {
        self.position / BAR + 1
    }

    pub fn loop_start_display(&self) -> i32 {
        self.loop_start / BAR + 1
    }

    pub fn loop_length_display(&self) -> i32 {
        (self.loop_end - self.loop_start) / BAR
    }
}

/// One pattern section (a row of the rack).
#[derive(Clone, Debug)]
pub struct Row {
    pub enabled: bool,
    /// The selected pattern 0..=31.
    pub pattern: u8,
    /// The shown bank 0..=3.
    pub bank: u8,
    /// Per pattern: the length 1..=16 and the shuffle flag.
    pub len: [u8; 32],
    pub shuffle: [bool; 32],
}

impl Default for Row {
    fn default() -> Self {
        Self {
            enabled: true,
            pattern: 0,
            bank: 0,
            len: [16; 32],
            shuffle: [false; 32],
        }
    }
}

impl Row {
    /// The pattern button (0..=7) that is lit: only when the selected pattern is in the shown
    /// bank.
    pub fn lit_button(&self) -> Option<u8> {
        (self.pattern / 8 == self.bank).then_some(self.pattern & 7)
    }
}

/// A step of a TB-303 pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step303 {
    pub note: u8,
    pub slide: bool,
    pub accent: bool,
    pub up: bool,
    pub down: bool,
    pub rest: bool,
}

impl Default for Step303 {
    /// A cleared step: note 0 (low C), rest on, no flags (`Clear`).
    fn default() -> Self {
        Self {
            note: 0,
            slide: false,
            accent: false,
            up: false,
            down: false,
            rest: true,
        }
    }
}

/// How long the lit note shows after a key in pitch mode, seconds [G].
const NOTE_FLASH: f64 = 0.4;

/// A TB-303's step editor.
#[derive(Clone, Debug)]
pub struct Synth303 {
    /// The edit step 0..=15.
    pub edit: u8,
    pub pitch_mode: bool,
    pub flash_until: f64,
    /// The note a key wrote last (this fork: pitch mode's flash shows it, not the next step's).
    pub flash_note: u8,
    /// The steps of the 32 patterns.
    pub patterns: Vec<[Step303; 16]>,
}

impl Default for Synth303 {
    fn default() -> Self {
        Self {
            edit: 0,
            pitch_mode: false,
            flash_until: 0.0,
            flash_note: 0,
            patterns: vec![[Step303::default(); 16]; 32],
        }
    }
}

impl Synth303 {
    /// Step and Back move the edit step by +1 / -1 modulo 16, whatever the pattern length.
    pub fn move_edit(&mut self, d: i32) {
        self.edit = (self.edit as i32 + d).rem_euclid(16) as u8;
    }

    /// The pitch mode button; switching it on first sets the edit step to step 1.
    pub fn toggle_pitch(&mut self) {
        self.pitch_mode = !self.pitch_mode;
        if self.pitch_mode {
            self.edit = 0;
        }
    }

    /// Whether the key LED of `note` is lit: the edit step's note; in pitch mode only briefly
    /// after a key press, and then (this fork) the note that key wrote.
    pub fn note_lit(&self, pattern: u8, note: u8, now: f64) -> bool {
        if self.pitch_mode {
            return now < self.flash_until && self.flash_note == note;
        }
        self.patterns[pattern as usize][self.edit as usize].note == note
    }

    pub fn step(&self, pattern: u8) -> &Step303 {
        &self.patterns[pattern as usize][self.edit as usize]
    }
}

/// The whole panel's state.
#[derive(Clone, Debug)]
pub struct Panel {
    pub transport: Transport,
    pub rows: [Row; 4],
    /// The row with the focus 0..=3.
    pub focus: usize,
    pub synth: [Synth303; 2],
    /// The selected instruments (0 = AC) of the two drum machines.
    pub inst808: usize,
    pub inst909: usize,
    pub steps808: [[bool; 16]; 12],
    pub steps909: [[u8; 16]; 12],
    /// The playing step (the running light), from the engine; `None` when stopped.
    pub playing_step808: Option<usize>,
    pub playing_step909: Option<usize>,
    /// The playing step of each 303 (live byte 9); `None` when stopped. Tab and Shift+Tab
    /// edit it (docs/ui_spec.md section 10).
    pub playing_step303: [Option<usize>; 2],
    /// The global PCF and Comp routing ids: 0 none, PCF `c + 1`, Comp `c + 2` (master 1).
    pub pcf_id: u8,
    pub comp_id: u8,
    /// This fork: the PCF (`[c][0]`) and the COMP (`[1]`) also on channel c, besides the
    /// channel `pcf_id` and `comp_id` choose (the mixer's `MORE` controllers, `mixerN.pcf_more`
    /// and `.comp_more`).
    pub more: [[bool; 2]; 4],
    /// This fork: the effect each effects slot holds, top to bottom (the mixer's `SLOT`
    /// controllers, `mixer.slot1`..`4`; [`FX_KINDS`] order).
    pub slots: [u8; 4],
    /// Meter states 0..=10 by control id (mixer, master, effect input); engine-fed.
    pub meters: HashMap<String, u8>,
    /// The compressor's nine gain-reduction LEDs: 0 off, 1 green, 2 amber, 3 red.
    pub gr_leds: [u8; 9],
    /// This fork: the second COMP's ([`Panel::gr_leds`]).
    pub gr_leds_b: [u8; 9],
    /// This fork: each mixer channel's own compressor's ([`Panel::gr_leds`]).
    pub gr_ccomp: [[u8; 9]; 4],
    /// This fork: whether each mixer channel shows its COMP face (its C key opens it, the face's
    /// back key closes it); the panel's state, no message.
    pub ccomp_open: [bool; 4],
    /// The song's sound model: the Options menu's "Vintage ReBirth Synth Sound" (GLOB byte 482,
    /// the engine's global byte 21). Changes go through [`Panel::set_vintage`].
    pub vintage: bool,
    /// What the engine must hear, drained by the caller.
    pub events: Vec<Event>,
    /// The mixer channels' SOLO and MUTE buttons (this fork). A channel plays when it is not
    /// muted and, while any channel is soloed, is soloed itself; that is the engine's channel
    /// enable, `mixerN.mute` (1: plays), which the panel sends ([`Panel::apply_channels`]).
    pub solo: [bool; 4],
    pub mute: [bool; 4],
    /// The 303s' TUNE values as the engine had them (this fork): kept while the knob stays on
    /// their semitone, so a song saves the value it had, not its semitone's own.
    tune_raw: HashMap<String, i32>,
    /// A 303 key pressed in pitch mode (this fork, `audition.rs`): the synth, the pattern and
    /// the step it wrote, for the app to play the note; taken by the app.
    pub audition: Option<(usize, u8, usize)>,
    /// The knob or fader being dragged (set by the window): the engine's value does not move it.
    pub held: Option<String>,
    knobs: HashMap<String, i32>,
    faders: HashMap<String, i32>,
    toggles: HashMap<String, bool>,
    switches: HashMap<String, u8>,
    numbers: HashMap<String, i32>,
    flam_button: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

/// `c` for a section name like `mixer3` -> 2.
/// This fork: a mixer channel's DIST, PCF or COMP knob (on above 0, in place of the original's
/// keys): the channel (0..4) and the effect.
pub fn mixer_insert(id: &str) -> Option<(usize, &str)> {
    let (sec, name) = id.split_once('.')?;
    let c = index_of(sec, "mixer").filter(|&c| c < 4)?;
    matches!(name, "dist" | "pcf" | "comp").then_some((c, name))
}

/// This fork: a FLIP face's effects lamp's sound (1..12) and slot (0..4) by its name
/// (`fx3_1`).
pub fn flip_lamp(name: &str) -> Option<(usize, usize)> {
    let (k, s) = name.strip_prefix("fx")?.split_once('_')?;
    let (k, s) = (k.parse().ok()?, s.parse().ok()?);
    ((1..12).contains(&k) && s < 4).then_some((k, s))
}

/// This fork: the slots the layout draws the effects in (the original's), which the panel
/// moves each effect's face and knobs from to the slot that holds it ([`Panel::slot_dy`]).
const LAYOUT_SLOTS: [u8; 4] = {
    use orb_engine::frame::*;
    [FX_DELAY, FX_DIST, FX_PCF, FX_COMP]
};

/// This fork: the channel 0..=3 of a mixer channel's section (`mixer2`, its COMP face
/// `ccomp2`).
pub fn mixer_channel(sec: &str) -> usize {
    let n = sec.trim_start_matches("mixer").trim_start_matches("ccomp");
    n.parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .unwrap_or(0)
        .min(3)
}

/// This fork: the effects a slot may hold, by the engine's codes (`orb_engine::frame::FX_RVRB`
/// ..), as section and knob names.
pub const FX_KINDS: [&str; 8] = ["rvrb", "delay", "dist", "pcf", "cho", "fla", "pha", "comp"];

/// This fork: the effects a slot may hold (a slot's menu offers only these): not the
/// compressor, each channel's own (`ccomp`).
pub const FX_READY: [u8; 7] = {
    use orb_engine::frame::*;
    [FX_RVRB, FX_DELAY, FX_DIST, FX_PCF, FX_CHO, FX_FLA, FX_PHA]
};

/// This fork: the effect code of a section or knob name (`delay`, `dist`, ...).
pub fn fx_code(name: &str) -> Option<u8> {
    FX_KINDS.iter().position(|&k| k == name).map(|i| i as u8)
}

/// This fork: the effect and its instance (0 the song's, 1 the second: `delay_b`) of a section
/// or a mixer knob or label name.
pub fn fx_instance(name: &str) -> Option<(u8, usize)> {
    let name = name.strip_suffix("_label").unwrap_or(name);
    match name.strip_suffix("_b") {
        Some(base) => fx_code(base).map(|fx| (fx, 1)),
        None => fx_code(name).map(|fx| (fx, 0)),
    }
}

/// This fork: the section and kind of a DIST's kind lamp (`dist_b.kind3` → `("dist_b", 3)`).
pub fn dist_kind_lamp(id: &str) -> Option<(&str, i32)> {
    let (sec, name) = id.split_once('.')?;
    let k: i32 = name.strip_prefix("kind")?.parse().ok()?;
    (matches!(sec, "dist" | "dist_b") && (0..8).contains(&k)).then_some((sec, k))
}

/// This fork: the channel (0..4) and the effect (0 PCF, 1 COMP) of a `mixerN.pcf_more` or
/// `.comp_more` id.
pub fn more_of(id: &str) -> Option<(usize, usize)> {
    let (c, name) = mixer_insert(id.strip_suffix("_more")?)?;
    Some((c, ["pcf", "comp"].iter().position(|&n| n == name)?))
}

fn index_of(section: &str, prefix: &str) -> Option<usize> {
    section
        .strip_prefix(prefix)?
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
}

impl Panel {
    pub fn new() -> Self {
        let mut p = Self {
            transport: Transport::default(),
            rows: Default::default(),
            focus: 0,
            synth: Default::default(),
            inst808: 1,
            inst909: 1,
            steps808: [[false; 16]; 12],
            steps909: [[0; 16]; 12],
            playing_step808: None,
            playing_step909: None,
            playing_step303: [None; 2],
            pcf_id: 0,
            comp_id: 0,
            more: [[false; 2]; 4],
            slots: orb_engine::frame::DEFAULT_SLOTS,
            meters: HashMap::new(),
            gr_leds: [0; 9],
            gr_leds_b: [0; 9],
            gr_ccomp: [[0; 9]; 4],
            ccomp_open: [false; 4],
            vintage: false,
            events: Vec::new(),
            solo: [false; 4],
            mute: [false; 4],
            tune_raw: HashMap::new(),
            audition: None,
            held: None,
            knobs: HashMap::new(),
            faders: HashMap::new(),
            toggles: HashMap::new(),
            switches: HashMap::new(),
            numbers: HashMap::new(),
            flam_button: false,
        };
        for c in crate::layout::controls() {
            match c.kind.as_str() {
                "knob" => {
                    // (this fork: the mixer's DIST, PCF and COMP knobs start off)
                    let v = match c.name() {
                        "shuffle" | "delay" | "limit_amt" => 0,
                        // this fork: the channels' sends into the chorus, flanger and phaser
                        "cho" | "fla" | "pha" | "cho_b" | "fla_b" | "pha_b" | "rvrb" | "rvrb_b"
                            if c.section.starts_with("mixer") =>
                        {
                            0
                        }
                        _ if mixer_insert(&c.id).is_some() => 0,
                        _ => 500,
                    };
                    p.knobs.insert(c.id, v);
                }
                "fader" => {
                    let v = if c.section.starts_with("mixer") || c.section == "master" {
                        750
                    } else {
                        500
                    };
                    p.faders.insert(c.id, v);
                }
                "toggle" => {
                    // Pattern sections and mixer channels start on, the effects off. (Every
                    // toggle has an entry, so that the engine's value is taken: the mixer
                    // channel's distortion insert, the 303's and 909's flags.)
                    let on = (c.name() == "on" || c.name() == "mute")
                        && (c.section.starts_with("pattern") || c.section.starts_with("mixer"))
                        // the FLIP faces' FX keys (this fork): every sound's effects on
                        || (FLIP_FACES.contains(&c.section.as_str()) && c.name().starts_with("fx"));
                    p.toggles.insert(c.id, on);
                }
                "hswitch" | "vswitch" => {
                    p.switches.insert(c.id, u8::from(c.section == "tr808"));
                }
                _ => {}
            }
        }
        p.numbers.insert("delay.steps".into(), 3);
        p.numbers.insert("delay_b.steps".into(), 3);
        p.numbers.insert("pcf.pattern".into(), 0);
        p.numbers.insert("pcf_b.pattern".into(), 0);
        // the chorus's and the flanger's TYPE, the phaser's STAGES (this fork)
        for sfx in ["", "_b"] {
            p.numbers.insert(format!("cho{sfx}.type"), 1);
            p.numbers.insert(format!("fla{sfx}.type"), 1);
            p.numbers.insert(format!("pha{sfx}.stages"), 6);
            p.numbers.insert(format!("rvrb{sfx}.type"), 0);
        }
        // the DISTs' kinds (this fork): the classic one
        p.numbers.insert("dist.kind".into(), 0);
        p.numbers.insert("dist_b.kind".into(), 0);
        // the channels' enables (this fork: no control of their own, SOLO and MUTE set them)
        for c in 1..=4 {
            p.toggles.insert(format!("mixer{c}.mute"), true);
        }
        // the mixer's PCF and COMP knobs (this fork) as pcf_id and comp_id choose
        p.sync_insert_knobs();
        p
    }

    /// Sends each mixer channel's enable as SOLO and MUTE make it (this fork): a soloed channel
    /// plays, muted or not; while any channel is soloed the others are off; else a channel
    /// plays unless muted. The mute is kept, so a channel's SOLO going off gives it back.
    pub fn apply_channels(&mut self) {
        let any_solo = self.solo.iter().any(|&s| s);
        for c in 0..4 {
            let on = self.solo[c] || (!any_solo && !self.mute[c]);
            let id = format!("mixer{}.mute", c + 1);
            if self.toggles.get(&id).copied() != Some(on) {
                self.toggles.insert(id.clone(), on);
                self.set_event(&id, on as i32);
            }
        }
    }

    /// A Shift click on a SOLO (`mute` false) or MUTE: every channel's goes off.
    pub fn clear_channels(&mut self, mute: bool) {
        if mute {
            self.mute = [false; 4];
        } else {
            self.solo = [false; 4];
        }
        self.apply_channels();
    }

    /// Whether a mixer channel plays (its enable): the MUTE LED (this fork) is green while it
    /// does, signal or not (the VU meter shows the signal).
    pub fn channel_active(&self, c: usize) -> bool {
        self.toggle_on(&format!("mixer{}.mute", c + 1))
    }

    /// A lively state for screenshots: song mode, playing, some steps, levels and meters.
    pub fn demo() -> Self {
        let mut p = Self::new();
        p.transport.song_mode = true;
        p.transport.playing = true;
        p.transport.position = 4 * BAR;
        p.transport.loop_start = 2 * BAR;
        p.transport.loop_end = 6 * BAR;
        p.transport.looping = true;
        p.rows[1].pattern = 2;
        p.rows[2].pattern = 1;
        p.rows[3].pattern = 5;
        p.rows[3].bank = 0;
        p.rows[0].len[0] = 16;
        p.rows[1].len[2] = 12;
        p.rows[1].shuffle[2] = true;
        for (i, s) in p.synth[0].patterns[0].iter_mut().enumerate() {
            s.note = [0, 3, 7, 10, 12, 7, 3, 0][i % 8];
            s.rest = i % 5 == 4;
            s.accent = i % 4 == 0;
        }
        p.synth[0].edit = 3;
        for i in [0, 4, 8, 12] {
            p.steps808[1][i] = true;
        }
        for i in [4, 12] {
            p.steps808[2][i] = true;
        }
        for i in (0..16).step_by(2) {
            p.steps808[11][i] = true;
        }
        p.playing_step808 = Some(6);
        p.playing_step909 = Some(6);
        for (i, v) in [1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 3, 0, 2, 0, 0, 0]
            .into_iter()
            .enumerate()
        {
            p.steps909[1][i] = v;
        }
        p.pcf_id = 2;
        p.sync_insert_knobs();
        p.toggles.insert("pcf.on".into(), true);
        p.toggles.insert("comp.on".into(), true);
        p.toggles.insert("delay.on".into(), true);
        p.toggles.insert("transport.record".into(), false);
        p.put_knob("mixer1.dist", KNOB_MAX);
        p.set_knob("mixer2.delay", 300);
        p.set_knob("transport.shuffle", 300);
        p.set_knob("tb303_1.cutoff", 350);
        p.set_knob("tb303_1.reso", 620);
        p.set_knob("mixer1.pan", 250);
        p.set_knob("mixer4.pan", 800);
        p.set_fader("mixer1.level", 640);
        p.set_fader("mixer3.level", 880);
        for (id, v) in [
            ("mixer1.meter", 6),
            ("mixer2.meter", 5),
            ("mixer3.meter", 8),
            ("mixer4.meter", 3),
            ("master.meter_l", 8),
            ("master.meter_r", 7),
            ("pcf.meter", 4),
            ("comp.meter", 3),
            ("delay.meter", 2),
        ] {
            p.meters.insert(id.into(), v);
        }
        p.gr_leds = [0, 0, 0, 2, 1, 0, 0, 0, 0];
        p.events.clear();
        p
    }

    fn emit(&mut self, e: Event) {
        self.events.push(e);
    }

    pub(crate) fn set_event(&mut self, id: &str, value: i32) {
        self.emit(Event::Set {
            id: id.into(),
            value,
        });
    }

    /// Turns the Vintage sound on or off and sends it (song msg 35 `{21, v}`).
    pub fn set_vintage(&mut self, on: bool) {
        if self.vintage != on {
            self.vintage = on;
            self.set_event("song.vintage", on as i32);
        }
    }

    /// The value the engine holds for the control `id` (the one the panel would send, 0..=127;
    /// a toggle 0 or 1): what a saved song stores. `None` for an id that is not a controller.
    pub fn value(&self, id: &str) -> Option<u8> {
        // the mixer's DIST knob holds the original's key, 0 or 1; PCF's and COMP's are the
        // mixer's choice of channel (pcf_id, comp_id), no controller of their own
        match mixer_insert(id) {
            Some((_, "dist")) => return Some(u8::from(self.knob_on(id))),
            Some(_) => return None,
            None => {}
        }
        if let Some(&p) = self.knobs.get(id)
            && behavior::is_tune(id)
        {
            let s = behavior::tune_semis(p);
            let v = match self.tune_raw.get(id) {
                Some(&raw) if behavior::tune_from_value(raw) == s => raw,
                _ => behavior::tune_value(s),
            };
            return Some(v as u8);
        }
        if let Some(&p) = self.knobs.get(id) {
            return Some(behavior::knob_send(p) as u8);
        }
        if let Some(&p) = self.faders.get(id) {
            return Some(behavior::fader_send(p) as u8);
        }
        if let Some(&s) = self.switches.get(id) {
            return Some(s);
        }
        if let Some(&n) = self.numbers.get(id) {
            return Some(n as u8);
        }
        if self.toggles.contains_key(id) || (id.starts_with("pattern") && id.ends_with(".on")) {
            return Some(self.toggle_on(id) as u8);
        }
        None
    }

    // ---- knobs, faders, switches --------------------------------------------------------

    pub fn knob(&self, id: &str) -> i32 {
        self.knobs.get(id).copied().unwrap_or(0)
    }

    /// Sets a knob position and sends nothing (the fork's LFO knobs, restored from an .rbsx;
    /// lfo.rs).
    /// Sets a toggle's state without an event (a song's settings: rbsx.rs).
    pub fn put_toggle(&mut self, id: &str, on: bool) {
        self.toggles.insert(id.into(), on);
    }

    pub fn put_knob(&mut self, id: &str, p: i32) {
        self.knobs.insert(id.into(), p.clamp(0, KNOB_MAX));
    }

    /// Sets a knob position and sends the value when it changed.
    pub fn set_knob(&mut self, id: &str, p: i32) {
        let p = p.clamp(0, KNOB_MAX);
        // the 303's TUNE (this fork): on a whole semitone, sent as one
        if behavior::is_tune(id) {
            let s = behavior::tune_semis(p);
            let old = self.knobs.insert(id.into(), behavior::tune_pos(s));
            if old.map(behavior::tune_semis) != Some(s) {
                self.tune_raw
                    .insert(id.to_string(), behavior::tune_value(s));
                self.set_event(id, behavior::tune_value(s));
            }
            return;
        }
        if let Some((c, name)) = mixer_insert(id) {
            let old = self.knobs.insert(id.into(), p).unwrap_or(0);
            let (was, on) = (behavior::knob_send(old) > 0, self.knob_on(id));
            if on != was {
                self.insert_on(c, name, on);
            }
            // how much of the effect is heard (this fork): the knob's value while it is on
            if on && (!was || behavior::knob_send(old) != behavior::knob_send(p)) {
                self.set_event(&format!("{id}_wet"), behavior::knob_send(p));
            }
            return;
        }
        let old = self.knobs.insert(id.into(), p).unwrap_or(-1);
        let changed = old < 0 || behavior::knob_send(old) != behavior::knob_send(p);
        if changed {
            self.set_event(id, behavior::knob_send(p));
        }
    }

    /// This fork: how far below its layout place a control of section `section` named `name` is
    /// drawn: an effect's section by 90 px a slot, a mixer channel's knob and label of an effect
    /// by 22 px a slot, from the slot the layout has it in (the default slots) to its slot now.
    pub fn slot_dy(&self, section: &str, name: &str) -> f32 {
        let (fx, step) = if section.starts_with("mixer") {
            (fx_instance(name), 22.0)
        } else {
            (fx_instance(section), 90.0)
        };
        let Some((fx, inst)) = fx else { return 0.0 };
        // the layout has a second instance where the first is; the chorus, the flanger and the
        // phaser in the PCF's slot (the layout's slots: the original's DELAY, DIST, PCF, COMP)
        let home = LAYOUT_SLOTS
            .iter()
            .position(|&s| s == fx)
            .or((orb_engine::frame::FX_CHO..=orb_engine::frame::FX_PHA)
                .contains(&fx)
                .then_some(2))
            // the reverb in the delay's
            .or((fx == orb_engine::frame::FX_RVRB).then_some(0));
        match (home, self.slot_of(fx, inst)) {
            (Some(home), Some(now)) => step * (now as f32 - home as f32),
            _ => 0.0,
        }
    }

    /// This fork: the slot (0..4) that holds instance `inst` of effect `fx` (0 the upper of
    /// the slots that hold it, 1 the lower), if any.
    pub fn slot_of(&self, fx: u8, inst: usize) -> Option<usize> {
        (0..4).filter(|&s| self.slots[s] == fx).nth(inst)
    }

    /// This fork: whether a section's or a mixer knob's effect is in no slot (not drawn).
    pub fn out_of_slots(&self, section: &str, name: &str) -> bool {
        let fx = if section.starts_with("mixer") {
            fx_instance(name)
        } else {
            fx_instance(section)
        };
        fx.is_some_and(|(fx, inst)| self.slot_of(fx, inst).is_none())
    }

    /// This fork: whether effect `fx` may go into slot `s` (the effect there leaves the rack):
    /// an effect the engine has, in at most two slots.
    pub fn can_put(&self, s: usize, fx: u8) -> bool {
        let others = (0..4).filter(|&o| o != s && self.slots[o] == fx).count();
        s < 4 && self.slots[s] != fx && FX_READY.contains(&fx) && others <= 1
    }

    /// This fork: effect `fx` into slot `s` ([`Panel::can_put`]), sent; the effect that was
    /// there is in no slot then (not heard; its settings are kept).
    pub fn put_slot(&mut self, s: usize, fx: u8) {
        if self.can_put(s, fx) {
            self.slots[s] = fx;
            self.set_event(&format!("mixer.slot{}", s + 1), fx as i32);
        }
    }

    /// This fork: slots `a` and `b` swap their effects (both sent).
    pub fn swap_slots(&mut self, a: usize, b: usize) {
        if a >= 4 || b >= 4 || self.slots[a] == self.slots[b] {
            return;
        }
        self.slots.swap(a, b);
        for s in [a, b] {
            self.set_event(&format!("mixer.slot{}", s + 1), self.slots[s] as i32);
        }
    }

    /// Whether a knob is above 0 (the mixer's DIST, PCF and COMP knobs: on).
    pub fn knob_on(&self, id: &str) -> bool {
        self.knobs
            .get(id)
            .is_some_and(|&p| behavior::knob_send(p) > 0)
    }

    /// A mixer channel's DIST, PCF or COMP knob turned on or off (this fork): DIST sends the
    /// original's key. PCF and COMP: the first channel to take one is the one the original's id
    /// chooses (`pcf_id`, `comp_id`, what an `.rbs` keeps); more channels take it too, each with
    /// its own copy (`more`). When the id's channel lets go, the lowest other channel becomes it.
    fn insert_on(&mut self, c: usize, name: &str, on: bool) {
        let n = c + 1;
        let (k, base) = match name {
            "dist" => {
                self.set_event(&format!("mixer{n}.dist"), on as i32);
                return;
            }
            "pcf" => (0, 1),
            _ => (1, 2),
        };
        // the channel the id chooses, if any (the COMP's 1 is the master's, no channel's)
        let id_ch = |p: &Panel| {
            let v = if k == 0 { p.pcf_id } else { p.comp_id } as usize;
            v.checked_sub(base).filter(|&x| x < 4)
        };
        let set_id = |p: &mut Panel, ch: Option<usize>| {
            let v = ch.map_or(0, |x| (x + base) as u8);
            if k == 0 {
                p.pcf_id = v;
                p.set_event("mixer.pcf_id", v as i32);
            } else {
                p.comp_id = v;
                p.set_event("mixer.comp_id", v as i32);
            }
        };
        let set_more = |p: &mut Panel, ch: usize, v: bool| {
            if p.more[ch][k] != v {
                p.more[ch][k] = v;
                p.set_event(&format!("mixer{}.{name}_more", ch + 1), v as i32);
            }
        };
        match (on, id_ch(self)) {
            (true, None) => set_id(self, Some(c)),
            (true, Some(x)) if x != c => set_more(self, c, true),
            (true, Some(_)) => {}
            (false, Some(x)) if x == c => {
                let next = (0..4).find(|&j| self.more[j][k]);
                if let Some(j) = next {
                    set_more(self, j, false);
                }
                set_id(self, next);
            }
            (false, _) => set_more(self, c, false),
        }
        self.sync_insert_knobs();
    }

    /// The mixer's PCF and COMP knobs as `pcf_id`, `comp_id` and `more` have them: a channel
    /// that has the effect on (its knob at the top if it was at 0), the others at 0.
    fn sync_insert_knobs(&mut self) {
        for c in 0..4 {
            for (name, chosen) in [
                ("pcf", self.pcf_id as usize == c + 1 || self.more[c][0]),
                ("comp", self.comp_id as usize == c + 2 || self.more[c][1]),
            ] {
                let id = format!("mixer{}.{name}", c + 1);
                if !self.knobs.contains_key(&id) {
                    continue;
                }
                if chosen && !self.knob_on(&id) {
                    self.knobs.insert(id, KNOB_MAX);
                } else if !chosen && self.knob_on(&id) {
                    self.knobs.insert(id, 0);
                }
            }
        }
    }

    pub fn fader(&self, id: &str) -> i32 {
        self.faders.get(id).copied().unwrap_or(0)
    }

    pub fn set_fader(&mut self, id: &str, p: i32) {
        let p = p.clamp(0, behavior::FADER_MAX);
        let old = self.faders.insert(id.into(), p).unwrap_or(-1);
        if old < 0 || behavior::fader_send(old) != behavior::fader_send(p) {
            self.set_event(id, behavior::fader_send(p));
        }
    }

    pub fn switch(&self, id: &str) -> u8 {
        if id == "transport.mode" {
            return u8::from(self.transport.song_mode);
        }
        self.switches.get(id).copied().unwrap_or(0)
    }

    /// The position (0 or 1) a switch draws and a click flips from: its value by
    /// [`behavior::switch_position`]. [`Panel::switch`] keeps the value the song holds.
    pub fn switch_position(&self, id: &str) -> u8 {
        behavior::switch_position(id, self.switch(id))
    }

    pub fn set_switch(&mut self, id: &str, v: u8) {
        if id == "transport.mode" {
            self.transport.song_mode = v != 0;
            self.set_event(id, v as i32);
            if !self.transport.song_mode {
                // pattern mode turns record off: `{2, 0}` after `{0, 0}`, whatever record was
                // (`sub_42A058`, 0x42A4B0..0x42A4CC)
                self.transport.recording = false;
                self.set_event("transport.record", 0);
            }
            return;
        } else if self.switches.insert(id.into(), v) == Some(v) {
            return;
        }
        self.set_event(id, v as i32);
    }

    /// Selects a TR-808 / TR-909 instrument from a legend click.
    pub fn select_instrument(&mut self, machine808: bool, i: usize) {
        if machine808 {
            self.inst808 = i;
        } else {
            self.inst909 = i;
        }
    }

    // ---- the engine's values -------------------------------------------------------------

    /// Takes the engine's value `v` for the control `id` without sending it back: a knob or
    /// fader moves only when the value it would send differs (so the position the user dragged
    /// to is kept), a toggle, switch or number takes it. A control being dragged keeps its
    /// position. Unknown ids are ignored.
    pub fn adopt(&mut self, id: &str, v: i32) {
        if self.held.as_deref() == Some(id) {
            return;
        }
        // a pattern section's on/off switch is the row's state (the toggle map is not read)
        if let Some(r) = id
            .strip_suffix(".on")
            .and_then(|s| index_of(s, "pattern"))
            .filter(|&r| r < 4)
        {
            self.rows[r].enabled = v != 0;
        } else if let Some(knob) = id
            .strip_suffix("_wet")
            .filter(|k| mixer_insert(k).is_some())
        {
            // an insert's wet amount (this fork): the knob's value while it is on
            if self.held.as_deref() != Some(knob)
                && self.knob_on(knob)
                && v > 0
                && behavior::knob_send(self.knob(knob)) != v
            {
                self.knobs
                    .insert(knob.into(), behavior::knob_from_engine(v));
            }
        } else if let Some((_, "dist")) = mixer_insert(id) {
            // the original's key, 0 or 1: the knob at 0, or at the top if it was at 0
            let on = v != 0;
            if on != self.knob_on(id) {
                self.knobs.insert(id.into(), if on { KNOB_MAX } else { 0 });
            }
        } else if let Some(p) = self.knobs.get_mut(id) {
            if behavior::is_tune(id) {
                // the TUNE knob on the semitone the engine plays (this fork); the value kept
                *p = behavior::tune_pos(behavior::tune_from_value(v));
                self.tune_raw.insert(id.to_string(), v);
            } else if behavior::knob_send(*p) != v {
                *p = behavior::knob_from_engine(v);
            }
        } else if let Some(p) = self.faders.get_mut(id) {
            if behavior::fader_send(*p) != v {
                *p = behavior::knob_from_engine(v).min(behavior::FADER_MAX);
            }
        } else if let Some(t) = self.toggles.get_mut(id) {
            *t = v != 0;
        } else if let Some(s) = self.switches.get_mut(id) {
            *s = v as u8;
        } else if let Some(n) = self.numbers.get_mut(id) {
            *n = v;
        } else if let Some(s) = id
            .strip_prefix("mixer.slot")
            .and_then(|n| n.parse::<usize>().ok())
            .and_then(|n| n.checked_sub(1))
            .filter(|&s| s < 4)
        {
            self.slots[s] = v as u8;
        } else if let Some((c, k)) = more_of(id) {
            self.more[c][k] = v != 0;
            self.sync_insert_knobs();
        } else if id == "mixer.pcf_id" {
            self.pcf_id = v as u8;
            self.sync_insert_knobs();
        } else if id == "mixer.comp_id" {
            self.comp_id = v as u8;
            self.sync_insert_knobs();
        }
    }

    /// Takes the pattern the engine selects in row `r`; the shown bank follows it.
    pub fn adopt_pattern(&mut self, r: usize, pattern: u8) {
        let row = &mut self.rows[r];
        if row.pattern != pattern {
            row.pattern = pattern;
            row.bank = pattern / 8;
        }
    }

    // ---- the drum machines' FLIP faces (this fork) -----------------------------------------

    /// Whether drum machine `k` (0 the 808, 1 the 909) shows its FLIP face.
    pub fn flipped(&self, k: usize) -> bool {
        self.toggles
            .get(["tr808.flip", "tr909.flip"][k.min(1)])
            .copied()
            .unwrap_or(false)
    }

    /// Whether control `c` is hidden by a FLIP state: a FLIP face's controls while it is off;
    /// while it is on, the knobs it covers (every knob of the drum machine but the 909's FLAM).
    pub fn hidden(&self, c: &crate::layout::Ctl) -> bool {
        match c.section.as_str() {
            "tr808f" => !self.flipped(0),
            "tr909f" => !self.flipped(1),
            // (the FLIP face has no instrument switches: their sounds' upper pills choose)
            "tr808" => matches!(c.kind.as_str(), "knob" | "vswitch") && self.flipped(0),
            "tr909" => c.kind == "knob" && c.name() != "flam" && self.flipped(1),
            // a mixer channel's COMP face (this fork) over its MIX face, but the header's LEDs
            // and meter
            sec if sec.starts_with("ccomp") => !self.ccomp_open[mixer_channel(sec)],
            sec if sec.starts_with("mixer")
                && self.ccomp_open[mixer_channel(sec)]
                && !matches!(c.name(), "activity" | "solo" | "meter") =>
            {
                true
            }
            // an effect no slot holds (this fork), its face and its mixer knobs
            sec => self.out_of_slots(sec, c.name()),
        }
    }

    /// The background part drawn for `name`: a drum machine's FLIP face when it is on.
    pub fn background<'a>(&self, name: &'a str) -> &'a str {
        match name {
            "bg_tr808" if self.flipped(0) => "bg_tr808_flip",
            "bg_tr909" if self.flipped(1) => "bg_tr909_flip",
            _ => name,
        }
    }

    /// Slot `slot`'s effects lamp of every sound of drum machine `k` on or off (a double click
    /// on a lamp).
    pub fn set_all_fx(&mut self, k: usize, slot: usize, on: bool) {
        let sec = FLIP_FACES[k.min(1)];
        for i in 1..12 {
            self.toggles.insert(format!("{sec}.fx{i}_{slot}"), on);
        }
    }

    /// Drum machine `k`'s sounds' pans and effects lamps as its FLIP face sets them.
    pub fn voice_mix(&self, k: usize) -> VoiceMix {
        let sec = FLIP_FACES[k.min(1)];
        let mut m = VoiceMix::default();
        for i in 1..12 {
            let pan = behavior::knob_send(self.knob(&format!("{sec}.pan{i}")));
            m.pan[i] = pan.clamp(0, 127) as u8;
            m.lamps[i] = (0..4)
                .filter(|s| {
                    self.toggles
                        .get(&format!("{sec}.fx{i}_{s}"))
                        .copied()
                        .unwrap_or(true)
                })
                .fold(0, |b, s| b | 1 << s);
        }
        m
    }

    /// The label part drawn over an effects lamp of a FLIP face (this fork): the name of the
    /// effect its slot holds (`flip808lbl_dist`).
    pub fn flip_label(&self, c: &crate::layout::Ctl) -> Option<String> {
        if !FLIP_FACES.contains(&c.section.as_str()) {
            return None;
        }
        let (_, slot) = flip_lamp(c.name())?;
        let kind = FX_KINDS.get(self.slots[slot] as usize)?;
        let look = c.part.split("lamp_").next()?;
        Some(format!("{look}lbl_{kind}"))
    }

    // ---- toggles and presses ------------------------------------------------------------

    /// The on/off state of a toggle-like control.
    pub fn toggle_on(&self, id: &str) -> bool {
        let (sec, name) = id.split_once('.').unwrap_or((id, ""));
        match (sec, name) {
            ("transport", "loop") => self.transport.looping,
            ("transport", "record") => self.transport.recording,
            ("tr909", "flam_button") => self.flam_button,
            (_, "shuffle") if sec.starts_with("pattern") => {
                let r = index_of(sec, "pattern").unwrap_or(0).min(3);
                self.rows[r].shuffle[self.rows[r].pattern as usize]
            }
            (_, "on") if sec.starts_with("pattern") => {
                self.rows[index_of(sec, "pattern").unwrap_or(0).min(3)].enabled
            }
            (_, "solo") if sec.starts_with("mixer") => {
                self.solo[index_of(sec, "mixer").unwrap_or(0).min(3)]
            }
            // MUTE: the channel's green LED (this fork; `mutebtn` its older name)
            (_, "mutebtn" | "activity") if sec.starts_with("mixer") => {
                self.mute[index_of(sec, "mixer").unwrap_or(0).min(3)]
            }
            _ => self.toggles.get(id).copied().unwrap_or(false),
        }
    }

    /// Whether a radio control is lit: a DIST's kind lamp (this fork, `dist.kind3`) when the
    /// DIST has that kind; others as toggles.
    pub fn radio_on(&self, id: &str) -> bool {
        match dist_kind_lamp(id) {
            Some((sec, k)) => self.numbers.get(&format!("{sec}.kind")).copied() == Some(k),
            None => self.toggle_on(id),
        }
    }

    /// The mouse-down action of a toggle, trigger, grid, radio or mode control.
    /// `now` is the time in seconds (for the 303 note flash).
    pub fn press(&mut self, id: &str, now: f64) {
        let Some((sec, name)) = id.split_once('.') else {
            return;
        };
        match sec {
            "transport" => self.press_transport(name),
            s if s.starts_with("pattern") => {
                let r = index_of(s, "pattern").unwrap_or(0).min(3);
                self.press_pattern(r, name);
            }
            s if s.starts_with("tb303_") => {
                let k = index_of(s, "tb303_").unwrap_or(0).min(1);
                self.press_303(k, name, now);
            }
            "tr909" if name == "flam_button" => {
                self.flam_button = !self.flam_button;
            }
            // FLIP (this fork): the drum machine's other face; the panel's state, no message
            // and a sound's effects switch on that face (voice_mix)
            "tr808" | "tr909" if name == "flip" => {
                let on = !self.toggles.get(id).copied().unwrap_or(false);
                self.toggles.insert(id.into(), on);
            }
            "tr808f" | "tr909f" if name.starts_with("fx") => {
                let on = !self.toggles.get(id).copied().unwrap_or(true);
                self.toggles.insert(id.into(), on);
            }
            s if s.starts_with("mixer") => {
                let c = index_of(s, "mixer").unwrap_or(0).min(3);
                match name {
                    // SOLO and MUTE (this fork)
                    "solo" => {
                        self.solo[c] = !self.solo[c];
                        self.apply_channels();
                    }
                    "mutebtn" | "activity" => {
                        self.mute[c] = !self.mute[c];
                        self.apply_channels();
                    }
                    // the C key (this fork): the channel's COMP face
                    "c" => self.ccomp_open[c] = true,
                    _ => self.flip(id),
                }
            }
            // a channel's COMP face (this fork): the COMP and OTT lamps, and the back key to
            // the MIX face. A lamp turns its kind on and the other off; the lit one, both off.
            s if s.starts_with("ccomp") => match name {
                "back" => self.ccomp_open[mixer_channel(s)] = false,
                "on" | "ott" => {
                    let ott = name == "ott";
                    let lit = self.ccomp_mode(mixer_channel(s)) == Some(ott);
                    // off before the OTT flag changes, on after: the engine never hears
                    // the other kind for a moment
                    let (on, flag) = (format!("{s}.on"), format!("{s}.ott"));
                    if lit {
                        self.set_toggle(&on, false);
                        self.set_toggle(&flag, false);
                    } else {
                        self.set_toggle(&flag, ott);
                        self.set_toggle(&on, true);
                    }
                }
                _ => {}
            },
            // the master's LIMIT key (this fork, in place of the COMP key)
            "master" if name == "limit" => self.flip(id),
            // a DIST's kind lamp (this fork): the DIST takes that kind
            "dist" | "dist_b" if dist_kind_lamp(id).is_some() => {
                let k = dist_kind_lamp(id).map_or(0, |x| x.1);
                let kind = format!("{sec}.kind");
                if self.numbers.get(&kind) != Some(&k) {
                    self.numbers.insert(kind.clone(), k);
                    self.set_event(&kind, k);
                }
            }
            _ => {
                // an effect's ON, and (this fork) the delays' TAPE and the COMPs' OTT lamps
                if name == "on" || crate::link::TAPE_OTT.iter().any(|x| x.0 == id) {
                    self.flip(id);
                }
            }
        }
    }

    /// Mixer channel `c`'s compressor (this fork): `None` off, `Some(false)` the COMP,
    /// `Some(true)` the OTT (the engine's ON with its OTT flag).
    pub fn ccomp_mode(&self, c: usize) -> Option<bool> {
        let n = c + 1;
        self.toggle_on(&format!("ccomp{n}.on"))
            .then(|| self.toggle_on(&format!("ccomp{n}.ott")))
    }

    /// Toggle `id` to `v`, sending it when it changes.
    fn set_toggle(&mut self, id: &str, v: bool) {
        if self.toggle_on(id) != v {
            self.flip(id);
        }
    }

    fn flip(&mut self, id: &str) {
        let v = !self.toggles.get(id).copied().unwrap_or(false);
        self.toggles.insert(id.into(), v);
        self.set_event(id, v as i32);
    }

    fn press_transport(&mut self, name: &str) {
        let t = &mut self.transport;
        match name {
            "play" => {
                t.play();
                self.emit(Event::Press {
                    id: "transport.play".into(),
                });
            }
            "stop" => {
                // Playing: stop `{1, 0}`; stopped: locate `{8, t}` (`sub_429FE6`).
                let was_playing = t.playing;
                let was_recording = t.recording;
                t.stop();
                if was_recording {
                    self.set_event("transport.record", 0);
                }
                if was_playing {
                    self.emit(Event::Press {
                        id: "transport.stop".into(),
                    });
                } else {
                    let position = self.transport.position;
                    self.set_event("transport.position", position);
                }
            }
            // the button works in song mode only (its test at 0x42A1C7 is inside the song-mode
            // block of `sub_42A058`)
            "record" if t.song_mode => {
                t.recording = !t.recording;
                let v = t.recording as i32;
                self.set_event("transport.record", v);
            }
            "loop" => {
                t.looping = !t.looping;
                let v = t.looping as i32;
                self.set_event("transport.loop", v);
            }
            "mode" => {
                let v = !self.transport.song_mode as u8;
                self.set_switch("transport.mode", v);
            }
            _ => {}
        }
    }

    fn press_pattern(&mut self, r: usize, name: &str) {
        if let Some(n) = name.strip_prefix("pattern") {
            let b: u8 = n.parse().unwrap_or(1);
            let row = &mut self.rows[r];
            row.pattern = (b - 1) + 8 * row.bank;
            let pattern = row.pattern;
            self.focus = r;
            self.emit(Event::Pattern { row: r, pattern });
        } else if let Some(letter) = name.strip_prefix("bank") {
            self.rows[r].bank = letter.as_bytes()[0] - b'A';
        } else if name == "shuffle" {
            let row = &mut self.rows[r];
            let i = row.pattern as usize;
            row.shuffle[i] = !row.shuffle[i];
            let v = row.shuffle[i] as i32;
            self.set_event(&format!("pattern{}.shuffle", r + 1), v);
        } else if name == "on" {
            let row = &mut self.rows[r];
            row.enabled = !row.enabled;
            let v = row.enabled as i32;
            self.set_event(&format!("pattern{}.on", r + 1), v);
        }
    }

    fn press_303(&mut self, k: usize, name: &str, now: f64) {
        let pattern = self.rows[k].pattern;
        let s = &mut self.synth[k];
        let edit = s.edit as usize;
        let mut sent: Option<(u8, u8)> = None;
        let mut also: Option<(u8, u8)> = None;
        if let Some(n) = name.strip_prefix("key") {
            let note: u8 = n.parse().unwrap_or(0);
            s.patterns[pattern as usize][edit].note = note;
            s.flash_until = now + NOTE_FLASH;
            s.flash_note = note;
            sent = Some((0, note));
            if s.pitch_mode {
                // this fork: the key's step plays, a rest becomes a note (as Note/Pause would)
                let st = &mut s.patterns[pattern as usize][edit];
                if st.rest {
                    st.rest = false;
                    also = Some((5, 0));
                }
                s.move_edit(1);
                self.audition = Some((k, pattern, edit));
            }
        } else {
            let st = &mut s.patterns[pattern as usize][edit];
            match name {
                "slide" => {
                    st.slide = !st.slide;
                    sent = Some((1, st.slide as u8));
                }
                "accent" => {
                    st.accent = !st.accent;
                    sent = Some((2, st.accent as u8));
                }
                "up" => {
                    st.up = !st.up;
                    sent = Some((3, st.up as u8));
                }
                "down" => {
                    st.down = !st.down;
                    sent = Some((4, st.down as u8));
                }
                "note_pause" => {
                    st.rest = !st.rest;
                    sent = Some((5, st.rest as u8));
                }
                "step" => s.move_edit(1),
                "back" => s.move_edit(-1),
                "pitch_button" => s.toggle_pitch(),
                "clear" => {
                    s.patterns[pattern as usize] = [Step303::default(); 16];
                    self.events.push(Event::Clear303 { synth: k, pattern });
                }
                _ => {}
            }
        }
        for (field, value) in sent.into_iter().chain(also) {
            self.emit(Event::Step303 {
                synth: k,
                pattern,
                step: edit,
                field,
                value,
            });
        }
    }

    // ---- the arrows next to the displays ------------------------------------------------

    /// One step of a repeat or wind button: `id` is the arrow's layout id (`..._up` or
    /// `..._down`).
    pub fn arrow(&mut self, id: &str) {
        // Rewind, Fast forward, Bar, Loop start and Loop length are tested inside the
        // song-mode block of `sub_42A058` (`this+44` at 0x42A074, `je 0x42A2B5` skips it to the
        // loop switch and the tempo arrows): in pattern mode they do nothing at all.
        if !self.transport.song_mode
            && (matches!(id, "transport.rewind" | "transport.ff")
                || id.starts_with("transport.bar_")
                || id.starts_with("transport.loop_start_")
                || id.starts_with("transport.loop_length_"))
        {
            return;
        }
        if id == "transport.rewind" || id == "transport.ff" {
            // record goes off with the move: `{2, 0}` first
            if self.transport.recording {
                self.set_event("transport.record", 0);
            }
            self.transport.wind(id == "transport.ff");
            let position = self.transport.position;
            self.set_event("transport.position", position);
            return;
        }
        let Some((sec, name)) = id.split_once('.') else {
            return;
        };
        let (stem, up) = if let Some(s) = name.strip_suffix("_up") {
            (s, true)
        } else if let Some(s) = name.strip_suffix("_down") {
            (s, false)
        } else {
            return;
        };
        let d = if up { 1 } else { -1 };
        let (disp, value) = match (sec, stem) {
            ("transport", "tempo") => {
                self.transport.tempo_step(d);
                (format!("{sec}.{stem}"), self.transport.tempo)
            }
            ("transport", "bar") => {
                if !up && self.transport.recording {
                    self.set_event("transport.record", 0);
                }
                self.transport.bar_step(up);
                (format!("{sec}.{stem}"), self.transport.position)
            }
            ("transport", "loop_start") => {
                // Sends `{12, start}` then `{16, end}` (`sub_429E5B`).
                self.transport.loop_start_step(d * BAR);
                let start = self.transport.loop_start;
                self.set_event("transport.loop_start", start);
                ("transport.loop_end".to_string(), self.transport.loop_end)
            }
            ("transport", "loop_length") => {
                self.transport.loop_length_step(d * BAR);
                ("transport.loop_end".to_string(), self.transport.loop_end)
            }
            (s, "steps") if s.starts_with("pattern") => {
                let r = index_of(s, "pattern").unwrap_or(0).min(3);
                let row = &mut self.rows[r];
                let i = row.pattern as usize;
                row.len[i] = (row.len[i] as i32 + d).clamp(1, 16) as u8;
                (format!("{sec}.{stem}"), row.len[i] as i32)
            }
            ("delay" | "delay_b", "steps") => {
                let id = format!("{sec}.steps");
                let v = (self.numbers[&id] + d).clamp(1, 32);
                self.numbers.insert(id.clone(), v);
                (id, v)
            }
            // the chorus's and the flanger's TYPE, the phaser's STAGES (this fork)
            ("cho" | "cho_b" | "fla" | "fla_b", "type") => {
                let id = format!("{sec}.type");
                let v = (self.numbers[&id] + d).clamp(1, 4);
                self.numbers.insert(id.clone(), v);
                (id, v)
            }
            // the reverb's TYPE (this fork): HA, rO, PL, SP
            ("rvrb" | "rvrb_b", "type") => {
                let id = format!("{sec}.type");
                let v = (self.numbers[&id] + d).clamp(0, 3);
                self.numbers.insert(id.clone(), v);
                (id, v)
            }
            ("pha" | "pha_b", "stages") => {
                let id = format!("{sec}.stages");
                let v = (self.numbers[&id] + 2 * d).clamp(2, 12);
                self.numbers.insert(id.clone(), v);
                (id, v)
            }
            ("pcf" | "pcf_b", "pattern") => {
                let id = format!("{sec}.pattern");
                let v = (self.numbers[&id] + d).clamp(0, 54);
                self.numbers.insert(id.clone(), v);
                (id, v)
            }
            _ => return,
        };
        self.set_event(&disp, value);
    }

    // ---- the drum machines --------------------------------------------------------------

    /// Sets a TR-808 step of the selected instrument.
    pub fn set_step808(&mut self, step: usize, on: bool) {
        self.put_step808(self.inst808, step, on);
    }

    pub(crate) fn put_step808(&mut self, i: usize, step: usize, on: bool) {
        if self.steps808[i][step] != on {
            self.steps808[i][step] = on;
            self.emit(Event::Step808 {
                instrument: i,
                step,
                on,
            });
        }
    }

    /// Sets a TR-909 step of the selected instrument; a hi-hat step clears the other hi-hat.
    pub fn set_step909(&mut self, step: usize, value: u8) {
        self.put_step909(self.inst909, step, value);
    }

    pub(crate) fn put_step909(&mut self, i: usize, step: usize, value: u8) {
        if self.steps909[i][step] == value {
            return;
        }
        self.steps909[i][step] = value;
        self.emit(Event::Step909 {
            instrument: i,
            step,
            value,
        });
        if value != 0 && (i == 8 || i == 9) {
            let other = 17 - i;
            if self.steps909[other][step] != 0 {
                self.steps909[other][step] = 0;
                self.emit(Event::Step909 {
                    instrument: other,
                    step,
                    value: 0,
                });
            }
        }
    }

    /// The new value a mouse down on a 909 step gives, from the current step and modifiers.
    pub fn step909_new_value(&self, step: usize, m: behavior::Mods) -> u8 {
        behavior::tr909_step_value(
            self.steps909[self.inst909][step],
            self.inst909 == 0,
            self.flam_button,
            m,
        )
    }

    // ---- the computer keyboard's own actions (docs/ui_spec.md section 10) ---------------------

    /// Numpad 1 / 2: locates to the loop start / the loop end (clamped to the last bar start)
    /// [C] (task-081): `sub_42D1D3` cases 5 / 6 (0x42D24C, 0x42D269) call 0x429DBF with
    /// `this+0x106C` / `this+0x1070` and send msg 8 (0x429D77); the song-mode byte is not
    /// tested, so it works in pattern mode too.
    pub fn locate_loop(&mut self, end: bool) {
        let t = &mut self.transport;
        t.position = (if end { t.loop_end } else { t.loop_start }).clamp(0, MAX_POS);
        let position = t.position;
        self.set_event("transport.position", position);
    }

    /// A click on 303 `k`'s step LED `i` (0..=15, this fork): that step becomes the edit step.
    pub fn choose_edit_step(&mut self, k: usize, i: usize) {
        self.synth[k].edit = i.min(15) as u8;
    }

    /// The 16 steps of 303 `k`'s selected pattern (the GENERATE menu's RANDOM, generate.rs).
    pub fn pattern_303(&self, k: usize) -> [Step303; 16] {
        self.synth[k].patterns[self.rows[k].pattern as usize]
    }

    /// Writes the 16 steps of 303 `k`'s selected pattern and sends each field that changed, as
    /// a press of its button or key would (the GENERATE menu's RANDOM, generate.rs).
    pub fn set_pattern_303(&mut self, k: usize, steps: [Step303; 16]) {
        self.set_pattern_303_at(k, self.rows[k].pattern, steps);
    }

    /// [`Panel::set_pattern_303`] for 303 `k`'s pattern `pattern`, selected or not (the GENERATE
    /// menu's UNDO and REDO, generate.rs).
    pub fn set_pattern_303_at(&mut self, k: usize, pattern: u8, steps: [Step303; 16]) {
        let old = std::mem::replace(&mut self.synth[k].patterns[pattern as usize], steps);
        for (step, (o, n)) in old.iter().zip(steps.iter()).enumerate() {
            let fields = [
                (o.note, n.note),
                (o.slide as u8, n.slide as u8),
                (o.accent as u8, n.accent as u8),
                (o.up as u8, n.up as u8),
                (o.down as u8, n.down as u8),
                (o.rest as u8, n.rest as u8),
            ];
            for (field, (a, b)) in fields.into_iter().enumerate() {
                if a != b {
                    self.emit(Event::Step303 {
                        synth: k,
                        pattern,
                        step,
                        field: field as u8,
                        value: b,
                    });
                }
            }
        }
    }

    /// Tab / Shift+Tab on the focused 303: the step that is playing becomes a note
    /// (`rest` false) or a rest (`rest` true), in the pattern it plays. Nothing when the 303
    /// does not play.
    pub fn tap_303(&mut self, k: usize, rest: bool) {
        let Some(step) = self.playing_step303[k] else {
            return;
        };
        let pattern = self.rows[k].pattern;
        let st = &mut self.synth[k].patterns[pattern as usize][step];
        if st.rest != rest {
            st.rest = rest;
            self.emit(Event::Step303 {
                synth: k,
                pattern,
                step,
                field: 5,
                value: rest as u8,
            });
        }
    }

    /// A drum tap key: instrument `i` (0 = AC) of the 808 (`is808`) or the 909 is set on the
    /// step that is playing (the 909 with value 1, the AC row with 2), whatever the selected
    /// instrument is. Nothing when the machine does not play.
    pub fn tap_drum(&mut self, is808: bool, i: usize) {
        if is808 {
            if let Some(step) = self.playing_step808 {
                self.put_step808(i, step, true);
            }
        } else if let Some(step) = self.playing_step909 {
            self.put_step909(i, step, if i == 0 { 2 } else { 1 });
        }
    }

    // ---- what the displays show ---------------------------------------------------------

    /// The number a display shows; `None` is a disabled display. `id` is the display's
    /// layout id.
    pub fn display(&self, id: &str) -> Option<i32> {
        let (sec, name) = id.split_once('.')?;
        let t = &self.transport;
        match (sec, name) {
            ("transport", "tempo") => Some(t.tempo_display()),
            ("transport", "bar") => t.song_mode.then(|| t.bar_display()),
            ("transport", "loop_start") => t.song_mode.then(|| t.loop_start_display()),
            ("transport", "loop_length") => t.song_mode.then(|| t.loop_length_display()),
            (s, "steps") if s.starts_with("pattern") => {
                let r = &self.rows[index_of(s, "pattern")?.min(3)];
                Some(r.len[r.pattern as usize] as i32)
            }
            (s, "edit_step") if s.starts_with("tb303_") => {
                Some(self.synth[index_of(s, "tb303_")?.min(1)].edit as i32 + 1)
            }
            _ => self.numbers.get(id).copied(),
        }
    }

    /// The part to draw for a control in its current state. `pressed` is a mouse button held
    /// on it, `now` the time in seconds.
    pub fn part_name(&self, c: &crate::layout::Ctl, pressed: bool, now: f64) -> String {
        let fam = c.family();
        let (sec, name) = (c.section.as_str(), c.name());
        let on_off = |on: bool| format!("{fam}{}", if on { "on" } else { "off" });
        let push = || format!("{fam}{}", if pressed { "pressed" } else { "rest" });
        match c.kind.as_str() {
            "toggle" if c.id == "transport.record" => {
                if self.transport.recording {
                    format!("{fam}lit")
                } else {
                    push()
                }
            }
            "toggle" if c.id == "tr909.flam_button" => on_off(self.flam_button),
            // the channel COMP face's lamps (this fork): COMP lit for the compressor, OTT for
            // the OTT, never both
            "toggle" if sec.starts_with("ccomp") => {
                let mode = self.ccomp_mode(mixer_channel(sec));
                on_off(mode == Some(name == "ott"))
            }
            "toggle" => on_off(self.toggle_on(&c.id)),
            // a mixer channel's C key (this fork): green while the channel's COMP or OTT is on
            "ccompkey" => on_off(self.ccomp_mode(mixer_channel(sec)).is_some()),
            "radio" => on_off(self.radio_on(&c.id)),
            "switch" | "hswitch" | "vswitch" => format!("{fam}{}", self.switch_position(&c.id)),
            "trigger" if c.id == "transport.play" => {
                if self.transport.playing {
                    format!("{fam}lit")
                } else {
                    push()
                }
            }
            "trigger" | "wind" | "repeat" => push(),
            "led" => on_off(self.led_on(c, now)),
            // the COMP's RATIO and THRES labels (this fork): BAL and GAIN while its OTT is on
            // (a channel's COMP face: while its OTT lamp is lit)
            "ottlabel" if sec.starts_with("ccomp") => {
                on_off(self.ccomp_mode(mixer_channel(sec)) == Some(true))
            }
            "ottlabel" => on_off(self.toggle_on(&format!("{sec}.ott"))),
            // the mixer channel's MUTE LED (this fork): green while it plays
            "chanled" => on_off(self.channel_active(index_of(sec, "mixer").unwrap_or(0).min(3))),
            "grid" => {
                if let Some(n) = name.strip_prefix("pattern") {
                    let r = index_of(sec, "pattern").unwrap_or(0).min(3);
                    let n: u8 = n.parse().unwrap_or(1);
                    on_off(self.rows[r].lit_button() == Some(n - 1))
                } else {
                    let r = index_of(sec, "pattern").unwrap_or(0).min(3);
                    let letter = name.strip_prefix("bank").unwrap_or("A").as_bytes()[0] - b'A';
                    on_off(self.rows[r].bank == letter)
                }
            }
            "steps" => self.step_part(c),
            // the 303's step LEDs (this fork): yellow the step that plays (the running light, as
            // the 909's), green the edit step, red a step of the selected pattern's length, off
            // past it
            "stepled" => {
                let k = index_of(sec, "tb303_").unwrap_or(0).min(1);
                let i: usize = name
                    .strip_prefix("stepled")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(1);
                let row = &self.rows[k];
                let state = if self.playing_step303[k] == Some(i - 1) {
                    "play"
                } else if self.synth[k].edit as usize == i - 1 {
                    "sel"
                } else if i <= row.len[row.pattern as usize] as usize {
                    "on"
                } else {
                    "off"
                };
                format!("{fam}{state}")
            }
            "meter" if name.starts_with("gr") => {
                let i: usize = name[2..].parse().unwrap_or(1);
                let leds = if sec == "comp_b" {
                    &self.gr_leds_b
                } else if sec.starts_with("ccomp") {
                    &self.gr_ccomp[mixer_channel(sec)]
                } else {
                    &self.gr_leds
                };
                let s = ["off", "green", "amber", "red"][leds[i - 1].min(3) as usize];
                format!("gr_led_{s}")
            }
            "meter" => format!(
                "{fam}{}",
                self.meters.get(&c.id).copied().unwrap_or(0).min(10)
            ),
            _ => c.part.clone(),
        }
    }

    /// The next time after `now` the panel's look changes by itself, not by input or the engine
    /// (task-153): the end of a 303's lit-note flash in pitch mode. None: nothing is timed.
    pub fn next_change(&self, now: f64) -> Option<f64> {
        self.synth
            .iter()
            .filter(|s| s.pitch_mode && now < s.flash_until)
            .map(|s| s.flash_until)
            .min_by(f64::total_cmp)
    }

    fn led_on(&self, c: &crate::layout::Ctl, now: f64) -> bool {
        let Some(k) = index_of(&c.section, "tb303_") else {
            return false;
        };
        let k = k.min(1);
        let s = &self.synth[k.min(1)];
        let pattern = self.rows[k].pattern;
        let step = s.step(pattern);
        match c.name() {
            "pitch_led" => s.pitch_mode,
            "down_led" => step.down,
            "up_led" => step.up,
            "accent_led" => step.accent,
            "slide_led" => step.slide,
            "note_led" => !step.rest,
            n => n
                .strip_prefix("key")
                .and_then(|n| n.strip_suffix("_led"))
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|note| s.note_lit(pattern, note, now)),
        }
    }

    fn step_part(&self, c: &crate::layout::Ctl) -> String {
        let step: usize = c
            .name()
            .strip_prefix("step")
            .unwrap()
            .parse::<usize>()
            .unwrap()
            - 1;
        if c.section == "tr808" {
            let lit = self.steps808[self.inst808][step];
            let on = lit != (self.playing_step808 == Some(step));
            let color = ["red", "orange", "yellow", "white"][step / 4];
            format!("step808_{color}_{}", if on { "on" } else { "off" })
        } else {
            let mut v = self.steps909[self.inst909][step];
            if self.playing_step909 == Some(step) {
                v = if v == 0 { 2 } else { 0 };
            }
            let grp = if (step / 4).is_multiple_of(2) {
                "a"
            } else {
                "b"
            };
            // A song's step byte may be above 3. The original draws every value above 2 as the
            // flam frame (0x444F0C..0x444F18, docs/ui_spec.md section 7), and that is what the
            // engine plays: the flam pass retriggers every value above 2 (tr909.rs, 0x1002B320),
            // so 4 is an accent with a flam and 5..=255 the flam hit alone.
            format!("step909_{grp}_{}", v.min(3))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::Mods;

    /// (this fork) The 303's step LEDs: green the edit step, red the steps of the pattern's
    /// length, off past it; a click chooses the edit step.
    #[test]
    fn step_leds_show_the_edit_step_and_the_length() {
        let led = |i: usize| crate::layout::Ctl {
            id: format!("tb303_1.stepled{i}"),
            section: "tb303_1".into(),
            kind: "stepled".into(),
            part: "stepled_on".into(),
            x: 0.0,
            y: 0.0,
            w: 8.0,
            h: 8.0,
        };
        let mut p = Panel::new();
        let states = |p: &Panel| -> String {
            (1..=16)
                .map(|i| match p.part_name(&led(i), false, 0.0).as_str() {
                    "stepled_sel" => 'S',
                    "stepled_play" => 'P',
                    "stepled_on" => 'o',
                    _ => '.',
                })
                .collect()
        };
        assert_eq!(states(&p), "Sooooooooooooooo");
        p.choose_edit_step(0, 4);
        p.arrow("pattern1.steps_down");
        p.arrow("pattern1.steps_down");
        assert_eq!(states(&p), "ooooSooooooooo..");
        // past the length the edit step can still be chosen
        p.choose_edit_step(0, 15);
        assert_eq!(states(&p), "oooooooooooooo.S");
        assert_eq!(p.display("tb303_1.edit_step"), Some(16));
        // the second 303 and the other patterns keep their own
        assert_eq!(p.synth[1].edit, 0);
        p.press("pattern1.pattern2", 0.0);
        assert_eq!(states(&p), "oooooooooooooooS");
        // the step that plays is yellow, over the edit step's green too (the running light)
        p.playing_step303[0] = Some(2);
        assert_eq!(states(&p), "ooPooooooooooooS");
        p.playing_step303[0] = Some(15);
        assert_eq!(states(&p), "oooooooooooooooP");
        assert!(p.playing_step303[1].is_none());
    }

    /// (this fork) FLIP shows a drum machine's other face: its knobs hide (the 909's FLAM
    /// stays) and the face's PAN knobs and effects lamps show; they make the sounds' mix.
    #[test]
    fn flip_shows_the_pans_and_effects_switches() {
        let mut p = Panel::new();
        let ctl = |id: &str| {
            crate::layout::controls()
                .into_iter()
                .find(|c| c.id == id)
                .unwrap()
        };
        assert!(!p.flipped(0) && !p.hidden(&ctl("tr808.bd_level")));
        assert!(p.hidden(&ctl("tr808f.pan1")));
        assert!(p.voice_mix(0).neutral() && p.voice_mix(1).neutral());
        p.press("tr808.flip", 0.0);
        assert!(p.flipped(0) && p.hidden(&ctl("tr808.bd_level")));
        assert!(!p.hidden(&ctl("tr808f.pan1")) && !p.hidden(&ctl("tr808.flip")));
        assert_eq!(p.background("bg_tr808"), "bg_tr808_flip");
        p.press("tr909.flip", 0.0);
        assert!(p.hidden(&ctl("tr909.bd_tune")) && !p.hidden(&ctl("tr909.flam")));
        assert!(
            p.hidden(&ctl("tr808.slot3_switch")),
            "no instrument switches"
        );
        p.press("tr808f.fx3_1", 0.0);
        p.set_knob("tr909f.pan2", 0);
        let (m8, m9) = (p.voice_mix(0), p.voice_mix(1));
        assert!(m8.lamps[3] == 0b1101 && m8.lamps[2] == 0b1111 && m8.pan[3] == 64);
        assert!(m9.pan[2] == 0 && m9.lamps[2] == 0b1111);
        // a slot's lamp of every sound at once (a double click)
        p.set_all_fx(1, 2, false);
        assert!(p.voice_mix(1).lamps.iter().skip(1).all(|&l| l == 0b1011));
        // each lamp says the effect its slot holds
        let lbl = p.flip_label(&ctl("tr909f.fx4_2"));
        assert_eq!(lbl.as_deref(), Some("flip909lbl_pcf"));
        p.press("tr808.flip", 0.0);
        assert!(
            !p.flipped(0) && p.voice_mix(0).lamps[3] == 0b1101,
            "the mix stays without the face"
        );
        assert!(!p.hidden(&ctl("tr808.slot3_switch")));
    }

    /// (this fork) SOLO and MUTE set the channels' enables: soloed ones play alone, muted or
    /// not, and the others as their MUTE says once no SOLO is on; clearing turns every one of a
    /// kind off.
    #[test]
    fn solo_and_mute_set_the_channel_enables() {
        let mut p = Panel::new();
        let plays = |p: &Panel| -> [bool; 4] {
            std::array::from_fn(|c| p.toggle_on(&format!("mixer{}.mute", c + 1)))
        };
        assert_eq!(plays(&p), [true; 4]);
        p.press("mixer2.solo", 0.0);
        assert_eq!(plays(&p), [false, true, false, false]);
        assert!(p.toggle_on("mixer2.solo"));
        p.press("mixer4.solo", 0.0);
        assert_eq!(
            plays(&p),
            [false, true, false, true],
            "two soloed: both play"
        );
        assert!(p.events.contains(&Event::Set {
            id: "mixer1.mute".into(),
            value: 0
        }));
        // SOLO wins over MUTE: the muted channel plays while soloed
        p.press("mixer4.mutebtn", 0.0);
        assert_eq!(plays(&p), [false, true, false, true]);
        // its SOLO off: it is muted again, as it was
        p.press("mixer4.solo", 0.0);
        assert_eq!(plays(&p), [false, true, false, false]);
        p.press("mixer4.solo", 0.0);
        assert_eq!(
            plays(&p),
            [false, true, false, true],
            "soloed again: it plays"
        );
        p.clear_channels(false);
        assert_eq!(p.solo, [false; 4]);
        assert_eq!(plays(&p), [true, true, true, false], "the mute stays");
        p.press("mixer1.mutebtn", 0.0);
        assert_eq!(plays(&p), [false, true, true, false]);
        p.clear_channels(true);
        assert_eq!(plays(&p), [true; 4]);
        // the MUTE LED: green while the channel plays, with or without a signal
        assert!(
            (0..4).all(|c| p.channel_active(c)),
            "at the start: all green"
        );
        p.meters.insert("mixer3.meter".into(), 4);
        p.press("mixer3.mutebtn", 0.0);
        assert!(!p.channel_active(2), "muted: dark");
        assert!(p.channel_active(0), "no signal: still green");
        p.press("mixer3.mutebtn", 0.0);
        assert!(p.channel_active(2), "unmuted: green again");
    }

    /// (this fork) TUNE moves by whole semitones, -12..=12, and every one is sent as a value
    /// the engine plays as that semitone.
    #[test]
    fn tune_moves_by_whole_semitones() {
        for s in -12..=12 {
            assert_eq!(behavior::tune_semis(behavior::tune_pos(s)), s);
            assert_eq!(behavior::tune_from_value(behavior::tune_value(s)), s, "{s}");
        }
        assert_eq!(behavior::tune_semis(500), 0, "the middle is the default");
        let mut p = Panel::new();
        p.events.clear();
        // a drag a little past the middle stays on it and sends nothing
        p.set_knob("tb303_1.tune", 515);
        assert!(p.events.is_empty());
        assert_eq!(p.knob("tb303_1.tune"), 500);
        // two fields to the right: D
        p.set_knob("tb303_1.tune", behavior::tune_pos(2) + 10);
        assert_eq!(p.knob("tb303_1.tune"), behavior::tune_pos(2));
        assert_eq!(
            p.events,
            vec![Event::Set {
                id: "tb303_1.tune".into(),
                value: behavior::tune_value(2)
            }]
        );
        assert_eq!(p.value("tb303_1.tune"), Some(behavior::tune_value(2) as u8));
        // the engine's value of an older song lands on its semitone
        p.adopt("tb303_1.tune", 30);
        assert_eq!(behavior::tune_semis(p.knob("tb303_1.tune")), -6);
    }

    #[test]
    fn tempo_clamps_and_steps_by_one_bpm() {
        let mut t = Transport::default();
        t.tempo_step(1);
        assert_eq!(t.tempo_display(), 141);
        for _ in 0..1000 {
            t.tempo_step(-1);
        }
        assert_eq!(t.tempo, 20_000);
        for _ in 0..1000 {
            t.tempo_step(1);
        }
        assert_eq!(t.tempo, 500_000);
    }

    #[test]
    fn stop_stop_stop_reaches_bar_one() {
        let mut t = Transport {
            playing: true,
            recording: true,
            position: 5 * BAR + 100,
            loop_start: 2 * BAR,
            loop_end: 6 * BAR,
            ..Transport::default()
        };
        t.stop();
        assert!(!t.playing && !t.recording);
        assert_eq!(t.position, 5 * BAR + 100);
        t.stop();
        assert_eq!(t.position, 2 * BAR);
        t.stop();
        assert_eq!(t.position, 0);
        assert_eq!(t.bar_display(), 1);
    }

    #[test]
    fn wind_and_bar_steps() {
        let mut t = Transport {
            position: 3 * BAR + 5,
            recording: true,
            ..Transport::default()
        };
        t.bar_step(true);
        assert_eq!((t.position, t.recording), (4 * BAR, true));
        t.bar_step(false);
        assert_eq!((t.position, t.recording), (3 * BAR, false));
        t.recording = true;
        t.wind(true);
        assert_eq!((t.bar_display(), t.recording), (14, false));
        t.wind(false);
        t.wind(false);
        assert_eq!(t.position, 0);
        for _ in 0..200 {
            t.wind(true);
        }
        assert_eq!(t.position, MAX_POS);
        assert_eq!(t.bar_display(), 999);
    }

    #[test]
    fn loop_start_keeps_length_and_length_has_a_bar_minimum() {
        let mut t = Transport {
            loop_start: 2 * BAR,
            loop_end: 6 * BAR,
            ..Transport::default()
        };
        t.loop_start_step(BAR);
        assert_eq!((t.loop_start_display(), t.loop_length_display()), (4, 4));
        t.loop_length_step(-10 * BAR);
        assert_eq!(t.loop_length_display(), 1);
        t.loop_length_step(2 * BAR);
        assert_eq!(t.loop_length_display(), 3);
        t.loop_start_step(-100 * BAR);
        assert_eq!(t.loop_start, 0);
    }

    #[test]
    fn pattern_bank_and_lit_button() {
        let mut p = Panel::new();
        p.press("pattern1.bankC", 0.0);
        assert_eq!(p.rows[0].pattern, 0);
        assert_eq!(p.rows[0].lit_button(), None);
        p.press("pattern1.pattern3", 0.0);
        assert_eq!(p.rows[0].pattern, 18);
        assert_eq!(p.rows[0].lit_button(), Some(2));
        assert_eq!(
            p.events.last(),
            Some(&Event::Pattern {
                row: 0,
                pattern: 18
            })
        );
        let ctl = |id: &str| {
            crate::layout::controls()
                .into_iter()
                .find(|c| c.id == id)
                .unwrap()
        };
        assert_eq!(
            p.part_name(&ctl("pattern1.pattern3"), false, 0.0),
            "pat_btn_3_on"
        );
        assert_eq!(
            p.part_name(&ctl("pattern1.pattern1"), false, 0.0),
            "pat_btn_1_off"
        );
        assert_eq!(
            p.part_name(&ctl("pattern1.bankC"), false, 0.0),
            "bank_btn_C_on"
        );
        p.press("pattern4.pattern2", 0.0);
        assert_eq!(p.focus, 3);
    }

    #[test]
    fn steps_arrows_clamp_per_pattern() {
        let mut p = Panel::new();
        for _ in 0..40 {
            p.arrow("pattern2.steps_down");
        }
        assert_eq!(p.display("pattern2.steps"), Some(1));
        assert_eq!(p.display("pattern1.steps"), Some(16));
        for _ in 0..40 {
            p.arrow("pattern2.steps_up");
        }
        assert_eq!(p.display("pattern2.steps"), Some(16));
        for _ in 0..40 {
            p.arrow("delay.steps_up");
        }
        assert_eq!(p.display("delay.steps"), Some(32));
        for _ in 0..80 {
            p.arrow("pcf.pattern_down");
        }
        assert_eq!(p.display("pcf.pattern"), Some(0));
    }

    #[test]
    fn displays_are_disabled_in_pattern_mode() {
        let mut p = Panel::new();
        assert_eq!(p.display("transport.bar"), None);
        assert_eq!(p.display("transport.tempo"), Some(140));
        p.set_switch("transport.mode", 1);
        assert_eq!(p.display("transport.bar"), Some(1));
        assert_eq!(p.display("transport.loop_length"), Some(4));
        p.press("transport.record", 0.0);
        assert!(p.transport.recording);
        p.press("transport.mode", 0.0);
        assert!(!p.transport.song_mode && !p.transport.recording);
    }

    #[test]
    fn mixer_pcf_and_comp_take_more_channels() {
        // this fork: knobs, on above 0; the first channel is the original's id, more channels
        // take it too (MORE)
        let mut p = Panel::new();
        p.set_knob("mixer2.pcf", 300);
        assert_eq!(p.pcf_id, 2);
        p.events.clear();
        p.set_knob("mixer3.pcf", 500);
        assert_eq!(p.pcf_id, 2);
        assert!(p.more[2][0] && p.knob_on("mixer2.pcf") && p.knob_on("mixer3.pcf"));
        assert!(p.events.contains(&Event::Set {
            id: "mixer3.pcf_more".into(),
            value: 1
        }));
        // the id's channel lets go: the other channel becomes the id's
        p.set_knob("mixer2.pcf", 0);
        assert_eq!(p.pcf_id, 3);
        assert!(!p.more[2][0] && !p.knob_on("mixer2.pcf") && p.knob_on("mixer3.pcf"));
        p.set_knob("mixer3.pcf", 0);
        assert_eq!(p.pcf_id, 0);
        p.set_knob("mixer1.comp", 200);
        assert_eq!(p.comp_id, 2);
        p.set_knob("mixer2.comp", 200);
        assert_eq!(p.comp_id, 2);
        assert!(p.more[1][1] && p.knob_on("mixer1.comp") && p.knob_on("mixer2.comp"));
        // a more channel lets go: only its flag
        p.set_knob("mixer2.comp", 0);
        assert_eq!(p.comp_id, 2);
        assert!(!p.more[1][1] && !p.knob_on("mixer2.comp"));
        // the engine's choice turns the knobs: the chosen ones up from 0, the others to 0
        p.adopt("mixer.pcf_id", 4);
        p.adopt("mixer1.pcf_more", 1);
        assert!(p.knob_on("mixer4.pcf") && p.knob_on("mixer1.pcf") && !p.knob_on("mixer3.pcf"));
        p.adopt("mixer.comp_id", 0);
        assert!(!p.knob_on("mixer1.comp"));
        // the master's LIMIT key (this fork) leaves the channels' COMP as it is
        p.set_knob("mixer2.comp", 200);
        p.events.clear();
        p.press("master.limit", 0.0);
        assert!(p.toggle_on("master.limit") && p.comp_id == 3);
        assert_eq!(
            p.events,
            [Event::Set {
                id: "master.limit".into(),
                value: 1
            }]
        );
        // Dist is independent per channel, sent as the original's key (0 or 1) once; the
        // knob's value while on is how much of the effect is heard (this fork)
        p.events.clear();
        p.set_knob("mixer1.dist", 100);
        p.set_knob("mixer1.dist", 400);
        p.set_knob("mixer2.dist", 100);
        assert!(p.knob_on("mixer1.dist") && p.knob_on("mixer2.dist"));
        assert_eq!(p.value("mixer1.dist"), Some(1));
        let set = |id: &str, value| Event::Set {
            id: id.into(),
            value,
        };
        let (a, b) = (behavior::knob_send(100), behavior::knob_send(400));
        assert_eq!(
            p.events,
            [
                set("mixer1.dist", 1),
                set("mixer1.dist_wet", a),
                set("mixer1.dist_wet", b),
                set("mixer2.dist", 1),
                set("mixer2.dist_wet", a),
            ]
        );
        p.events.clear();
        p.set_knob("mixer1.dist", 0);
        assert_eq!(p.value("mixer1.dist"), Some(0));
        assert_eq!(p.events, [set("mixer1.dist", 0)]);
        // the engine's wet amount turns a knob that is on, not one that is off
        p.adopt("mixer2.dist_wet", 127);
        p.adopt("mixer1.dist_wet", 127);
        assert_eq!(p.knob("mixer2.dist"), KNOB_MAX);
        assert_eq!(p.knob("mixer1.dist"), 0);
    }

    #[test]
    fn a_slots_menu_swaps_effects_and_moves_their_controls() {
        use orb_engine::frame::{FX_COMP, FX_DELAY, FX_DIST, FX_PCF, FX_RVRB};
        // this fork: the DIST slot (the second) and the reverb's (the fourth) swap; the reverb
        // is drawn in the delay's place (the first)
        let mut p = Panel::new();
        assert_eq!(p.slot_dy("rvrb", "size"), 270.0);
        assert_eq!(p.slot_dy("mixer3", "rvrb"), 66.0);
        p.swap_slots(1, 3);
        assert_eq!(p.slots, [FX_DELAY, FX_RVRB, FX_PCF, FX_DIST]);
        let set = |id: &str, value| Event::Set {
            id: id.into(),
            value,
        };
        assert_eq!(p.events, [set("mixer.slot2", 0), set("mixer.slot4", 2)]);
        // the sections 90 px a slot, the mixer's knobs and labels 22 px
        assert_eq!(p.slot_dy("rvrb", "size"), 90.0);
        assert_eq!(p.slot_dy("dist", "amount"), 180.0);
        assert_eq!(p.slot_dy("mixer3", "rvrb"), 22.0);
        assert_eq!(p.slot_dy("mixer3", "dist_label"), 44.0);
        assert_eq!(p.slot_dy("mixer3", "pan"), 0.0);
        assert_eq!(p.slot_dy("delay", "pan"), 0.0);
        // a second delay in the PCF's slot (the third): the PCF leaves the rack
        assert!(p.out_of_slots("delay_b", "pan") && !p.out_of_slots("pcf", "freq"));
        p.events.clear();
        p.put_slot(2, FX_DELAY);
        assert_eq!(p.slots, [FX_DELAY, FX_RVRB, FX_DELAY, FX_DIST]);
        assert_eq!(p.events, [set("mixer.slot3", 1)]);
        assert!(!p.out_of_slots("delay_b", "pan") && p.out_of_slots("pcf", "freq"));
        assert!(p.out_of_slots("mixer2", "pcf_label") && !p.out_of_slots("mixer2", "delay_b"));
        assert_eq!(p.slot_dy("delay_b", "pan"), 180.0);
        assert_eq!(p.slot_dy("mixer1", "delay_b"), 44.0);
        // not a third delay; a second DIST, not a third
        assert!(!p.can_put(1, FX_DELAY));
        assert!(p.can_put(1, FX_DIST));
        p.put_slot(1, FX_DIST);
        assert_eq!(p.slots, [FX_DELAY, FX_DIST, FX_DELAY, FX_DIST]);
        assert!(!p.can_put(0, FX_DIST));
        // the second DIST's face and knobs where the lower slot is (the fourth)
        assert!(!p.out_of_slots("dist_b", "amount") && p.out_of_slots("comp_b", "ratio"));
        assert_eq!(p.slot_dy("dist_b", "amount"), 180.0);
        assert_eq!(p.slot_dy("mixer2", "dist_b"), 44.0);
        // the compressor is no slot's (each channel's own)
        assert!(!p.can_put(1, FX_COMP));
        p.put_slot(1, FX_RVRB);
        // the PCF back
        assert!(p.can_put(0, FX_PCF));
        // the engine's slots
        p.adopt("mixer.slot2", 2);
        p.adopt("mixer.slot3", 3);
        p.adopt("mixer.slot4", 0);
        assert_eq!(p.slots, orb_engine::frame::DEFAULT_SLOTS);
    }

    #[test]
    fn knob_events_only_when_the_sent_value_changes() {
        let mut p = Panel::new();
        p.set_knob("tb303_1.cutoff", 500);
        p.events.clear();
        p.set_knob("tb303_1.cutoff", 501);
        assert!(p.events.is_empty());
        p.set_knob("tb303_1.cutoff", 520);
        assert_eq!(
            p.events,
            [Event::Set {
                id: "tb303_1.cutoff".into(),
                value: 66
            }]
        );
    }

    #[test]
    fn the_legend_selects_the_instrument() {
        let mut p = Panel::new();
        assert_eq!(p.inst808, 1);
        p.select_instrument(true, 7);
        assert_eq!(p.inst808, 7);
    }

    #[test]
    fn tr909_hihats_exclude_each_other() {
        let mut p = Panel::new();
        p.inst909 = 8;
        p.set_step909(3, 1);
        p.inst909 = 9;
        p.set_step909(3, 2);
        assert_eq!(p.steps909[8][3], 0);
        assert_eq!(p.steps909[9][3], 2);
        p.inst909 = 1;
        assert_eq!(p.step909_new_value(3, Mods::default()), 1);
    }

    #[test]
    fn running_light_inverts_the_playing_step() {
        let mut p = Panel::new();
        p.steps808[1][0] = true;
        p.playing_step808 = Some(0);
        let c = crate::layout::controls()
            .into_iter()
            .find(|c| c.id == "tr808.step1")
            .unwrap();
        assert_eq!(p.part_name(&c, false, 0.0), "step808_red_off");
        p.playing_step808 = Some(1);
        assert_eq!(p.part_name(&c, false, 0.0), "step808_red_on");
        let c9 = crate::layout::controls()
            .into_iter()
            .find(|c| c.id == "tr909.step5")
            .unwrap();
        p.playing_step909 = Some(4);
        assert_eq!(p.part_name(&c9, false, 0.0), "step909_b_2");
    }

    #[test]
    fn synth303_step_editor() {
        let mut p = Panel::new();
        p.press("tb303_1.back", 0.0);
        assert_eq!(p.display("tb303_1.edit_step"), Some(16));
        p.press("tb303_1.step", 0.0);
        assert_eq!(p.display("tb303_1.edit_step"), Some(1));
        // Outside pitch mode a key never advances.
        p.press("tb303_1.key7", 0.0);
        assert_eq!(p.synth[0].step(0).note, 7);
        assert_eq!(p.synth[0].edit, 0);
        // Pitch mode: on resets to step 1, a key advances.
        p.synth[0].edit = 5;
        p.press("tb303_1.pitch_button", 0.0);
        assert_eq!(p.synth[0].edit, 0);
        p.press("tb303_1.key4", 1.0);
        assert_eq!((p.synth[0].patterns[0][0].note, p.synth[0].edit), (4, 1));
        // The lit note flashes in pitch mode.
        assert!(p.synth[0].flash_until > 1.0);
        // Flags flip, rest flips, Clear empties.
        p.press("tb303_1.accent", 0.0);
        p.press("tb303_1.note_pause", 0.0);
        let s = *p.synth[0].step(0);
        assert!(s.accent && !s.rest);
        p.press("tb303_1.clear", 0.0);
        assert_eq!(*p.synth[0].step(0), Step303::default());
        assert_eq!(p.synth[1].edit, 0);
    }

    /// (this fork) In pitch mode a key's step becomes a note if it was a rest, sent after the
    /// pitch, and the key LED flashes the note just written, not the next step's.
    #[test]
    fn a_pitch_mode_key_makes_its_step_a_note() {
        let mut p = Panel::new();
        assert!(
            p.synth[0].patterns[0][0].rest,
            "a new song's steps are rests"
        );
        p.press("tb303_1.pitch_button", 0.0);
        p.events.clear();
        p.press("tb303_1.key5", 1.0);
        assert!(!p.synth[0].patterns[0][0].rest);
        assert_eq!(p.synth[0].patterns[0][0].note, 5);
        let sent: Vec<_> = p
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Step303 {
                    step, field, value, ..
                } => Some((*step, *field, *value)),
                _ => None,
            })
            .collect();
        assert_eq!(sent, [(0, 0, 5), (0, 5, 0)]);
        assert!(p.synth[0].note_lit(0, 5, 1.0));
        assert!(!p.synth[0].note_lit(0, 0, 1.0));
        assert!(!p.synth[0].note_lit(0, 5, 10.0), "only briefly");
        // a step that already plays sends the pitch alone
        p.synth[0].edit = 0;
        p.events.clear();
        p.press("tb303_1.key7", 2.0);
        let n = p
            .events
            .iter()
            .filter(|e| matches!(e, Event::Step303 { .. }))
            .count();
        assert_eq!(n, 1);
    }

    #[test]
    fn every_control_has_a_part_for_its_state() {
        // No panic and a non-empty part name for all 352 controls in two states.
        let mut p = Panel::demo();
        for pressed in [false, true] {
            for c in crate::layout::controls() {
                // (the fork's LFO knobs are drawn as knobs, from layers)
                if matches!(
                    c.kind.as_str(),
                    "knob" | "lfoknob" | "fader" | "display" | "area"
                ) {
                    continue;
                }
                let n = p.part_name(&c, pressed, 0.0);
                assert!(crate::art::part(&n).is_some(), "{}: no part {n}", c.id);
            }
            p.rows[0].bank = 2;
        }
    }

    #[test]
    fn every_switch_value_draws_an_existing_part_by_the_engine_rule() {
        // A song may hold any byte in a switch's controller (task-156: a 303 waveform of 127).
        let mut p = Panel::new();
        let switches: Vec<_> = crate::layout::controls()
            .into_iter()
            .filter(|c| matches!(c.kind.as_str(), "switch" | "hswitch" | "vswitch"))
            .collect();
        // (this fork: the second delay's 3/16 and the second PCF's mode, + 2; the chorus's, the
        // flanger's and the phaser's MODE, first and second, + 6; the reverb's, + 2)
        assert_eq!(switches.len(), 20);
        for c in &switches {
            if c.id == "transport.mode" {
                // the mode switch shows a flag, not a controller value
                for on in [false, true] {
                    p.transport.song_mode = on;
                    let n = p.part_name(c, false, 0.0);
                    assert_eq!(n, format!("{}{}", c.family(), u8::from(on)));
                    assert!(crate::art::part(&n).is_some(), "no part {n}");
                }
                continue;
            }
            for v in 0..=255u8 {
                p.adopt(&c.id, v as i32);
                let n = p.part_name(c, false, 0.0);
                assert!(
                    crate::art::part(&n).is_some(),
                    "{} = {v}: no part {n}",
                    c.id
                );
                let want = if c.id == "pcf.mode" || c.id == "pcf_b.mode" {
                    u8::from(v == 1)
                } else {
                    u8::from(v != 0)
                };
                assert_eq!(n, format!("{}{want}", c.family()), "{} = {v}", c.id);
                // the stored value stays what the song holds (a save writes it back)
                assert_eq!(p.switch(&c.id), v, "{}", c.id);
                assert_eq!(p.value(&c.id), Some(v), "{}", c.id);
            }
        }
    }

    #[test]
    fn every_909_step_value_draws_the_frame_of_what_plays() {
        let mut p = Panel::new();
        let steps: Vec<_> = crate::layout::controls()
            .into_iter()
            .filter(|c| c.kind == "steps" && c.section == "tr909")
            .collect();
        assert_eq!(steps.len(), 16);
        for v in 0..=255u8 {
            for playing in [None, Some(3)] {
                p.steps909[p.inst909] = [v; 16];
                p.playing_step909 = playing;
                for c in &steps {
                    let n = p.part_name(c, false, 0.0);
                    assert!(
                        crate::art::part(&n).is_some(),
                        "{} = {v}: no part {n}",
                        c.id
                    );
                    // 0 dark, 1 low, 2 accent, and the flam frame for every value above 2
                    // (the engine's flam pass; the original's draw, 0x444F0C); the playing
                    // step is inverted
                    let k: usize = c.name()["step".len()..].parse::<usize>().unwrap() - 1;
                    let frame = match (playing == Some(k), v) {
                        (true, 0) => 2,
                        (true, _) => 0,
                        (false, _) => v.min(3),
                    };
                    let grp = if (k / 4).is_multiple_of(2) { "a" } else { "b" };
                    assert_eq!(n, format!("step909_{grp}_{frame}"), "{} = {v}", c.id);
                }
            }
        }
    }
}
