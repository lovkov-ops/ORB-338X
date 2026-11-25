//! The `.rbsx` song: an RBS 4.2 song (`.rbs`) with one more chunk, `OGEN`, that keeps the
//! GENERATE menu's settings (`generate.rs`): each bass synth's scale and key note, the buttons
//! chosen in RANDOM, ALTER and MOVE, and which of those rows are green.
//!
//! The chunk sits directly under `RB40`, after the song's own chunks, where the `rbs` crate
//! keeps any chunk it does not know; so everything that reads songs (the engine, `orb`, File >
//! Info, the export) reads an `.rbsx` as the song it holds. The chunk is taken out of the song
//! when it is read, whatever the file's name, and written only into an `.rbsx`: an `.rbs` that
//! the app saves stays a plain RBS 4.2 song for ReBirth and other programs.
//!
//! The chunk's body is text, one `name=value` per line under a first line naming the format
//! and its version, so a later version can add settings and still read this one's. Names it
//! does not know are skipped; a scale or key note that no longer fits is left out.

use std::path::Path;

use eframe::egui::{
    Align, Color32, Context, Frame, Id, Key, Layout, Margin, Modal, Modifiers, RichText, Stroke,
};

use crate::app::App;
use crate::browser::{Browser, Mode};
use crate::generate::Op;
use crate::lfo;
use crate::palette::{INK, INK_DIM, SLATE_DARK, SLATE_LINE};
use crate::random::{MOVES, PARAMS};
use crate::scale::{KEYS, SCALES, ScaleKeys};
use orb_engine::frame::voices::{self, VoiceMix};

/// The drum machines' names in the chunk (this fork's FLIP faces).
const DRUMS: [&str; 2] = ["tr808", "tr909"];

/// The chunk's tag.
pub const TAG: [u8; 4] = *b"OGEN";
/// The first line of the chunk's body: the format and its version.
/// Version 2 (this fork): the compressor is each channel's own, no slot's
/// ([`move_comp_to_channels`]).
const HEADER: &str = "ORB-338X GENERATE 2";
/// The file extension of a song with the GENERATE settings.
pub const EXTENSION: &str = "rbsx";

/// What the GENERATE menu keeps of both synths (`App::gen_state`), and the LFOs' settings of
/// their five large knobs (lfo.rs).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenState {
    pub scale_keys: ScaleKeys,
    /// Indexed by row (`Op`), synth, button.
    pub sel: [[[bool; 5]; 2]; 3],
    /// Indexed by row (`Op`), synth: the row's action is green.
    pub done: [[bool; 2]; 3],
    /// Whether GENERATE has run on each synth (ALTER and SHIFT wait for it).
    pub generated: [bool; 2],
    /// The mixer channels' SOLO and MUTE buttons (this fork; the RBS part keeps only whether a
    /// channel plays).
    pub mix_solo: [bool; 4],
    pub mix_mute: [bool; 4],
    /// Each synth's LFOs, by knob (`lfo::PARAMS`).
    pub lfo: [[LfoState; 5]; 2],
    /// The 808's and the 909's FLIP faces (this fork): whether each shows, its sounds' pans
    /// and effects switches, and the drum machine's own kit (the INFO name of a mod, or
    /// "Standard"; `None`: the song's kit).
    pub flip: [bool; 2],
    pub voice_mix: [VoiceMix; 2],
    pub kits: [Option<String>; 2],
    /// The 808's and the 909's sounds' SOLO and MUTE lamps (this fork), by instrument index
    /// (1 BD .. 11).
    pub drum_solo: [[bool; 12]; 2],
    pub drum_mute: [[bool; 12]; 2],
    /// Each synth's sub-oscillator (this fork): its level (0..=127, the pattern-mode value)
    /// and the song-mode track's events, `(tick, level)` in tick order. No `.rbs` controller
    /// is theirs, so only an `.rbsx` keeps them.
    pub sub: [u8; 2],
    pub sub_track: [Vec<(i32, u8)>; 2],
    /// The master's limiter (this fork): its LIMIT key (0 off, 1 on) and AMT knob (0..=127),
    /// the pattern-mode values, and their song-mode tracks' events, as `sub` and `sub_track`.
    pub limit: [u8; 2],
    pub limit_track: [Vec<(i32, u8)>; 2],
    /// The effects slots (this fork): each mixer controller of theirs that is not at a new
    /// song's value or has song-mode events (`crate::link::fork_mixer_ids`): the controller,
    /// its pattern-mode value and its track's events, in controller order.
    pub mixer_fx: Vec<FxSetting>,
    /// An older file (version 1), whose effects slots held the compressor: `mixer_fx` is
    /// still to be moved to the channels' compressors ([`move_comp_to_channels`]).
    pub comp_in_slots: bool,
}

/// One mixer controller of the effects slots an `.rbsx` keeps ([`GenState::mixer_fx`]): the
/// controller, its value and its song-mode track's events `(tick, value)`.
pub type FxSetting = (u8, u8, Vec<(i32, u8)>);

/// The names of the master's limiter's two values in an `.rbsx` (this fork): LIMIT, AMT.
const LIMIT_KEYS: [&str; 2] = ["master.limit", "master.limit_amt"];

/// A song-mode track's events as an `.rbsx` writes them: `tick:value,...`.
fn track_text(track: &[(i32, u8)]) -> String {
    let list: Vec<String> = track.iter().map(|(t, v)| format!("{t}:{v}")).collect();
    list.join(",")
}

/// A track's events from their text, leniently: an event that does not parse, or a tick below
/// 0 or between positions (24 ticks), is left out; values above 127 are 127; tick order.
fn track_of(text: &str) -> Vec<(i32, u8)> {
    let mut track: Vec<(i32, u8)> = text
        .split(',')
        .filter_map(|e| {
            let (t, l) = e.trim().split_once(':')?;
            let t: i32 = t.trim().parse().ok()?;
            let l: u32 = l.trim().parse().ok()?;
            // the engine's ticks: positions of 24 ticks, not below 0
            (t >= 0 && t % 24 == 0).then_some((t, l.min(127) as u8))
        })
        .collect();
    track.sort_by_key(|&(t, _)| t);
    track
}

/// An LFO's settings: its RATE and AMOUNT knobs' values (0..=127) and its buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LfoState {
    pub rate: u8,
    pub amount: u8,
    pub mode: lfo::Mode,
}

impl Default for LfoState {
    fn default() -> Self {
        Self {
            rate: lfo::RATE_DEFAULT,
            amount: lfo::AMOUNT_DEFAULT,
            mode: lfo::Mode::default(),
        }
    }
}

impl GenState {
    /// Whether nothing is set: a song like that needs no chunk.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Whether `path` names an `.rbsx` song (any case).
pub fn is_rbsx(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

fn op_name(op: Op) -> &'static str {
    match op {
        Op::Random => "random",
        Op::Alter => "alter",
        Op::Move => "move",
    }
}

/// The chunk's body for `state`.
pub fn encode(state: &GenState) -> Vec<u8> {
    let mut lines = vec![HEADER.to_string()];
    for synth in 0..2 {
        let c = state.scale_keys.get(synth);
        let n = synth + 1;
        lines.push(format!(
            "baze{n}.scale={}",
            c.scale.map_or("", |s| SCALES[s].0)
        ));
        lines.push(format!("baze{n}.key={}", c.key.map_or("", |k| KEYS[k])));
        for op in Op::ALL {
            let names = if op == Op::Move { MOVES } else { PARAMS };
            let on: Vec<&str> = (0..5)
                .filter(|&i| state.sel[op as usize][synth][i])
                .map(|i| names[i])
                .collect();
            lines.push(format!("baze{n}.{}={}", op_name(op), on.join(",")));
            let done = state.done[op as usize][synth] as u8;
            lines.push(format!("baze{n}.{}.done={done}", op_name(op)));
        }
        lines.push(format!(
            "baze{n}.generated={}",
            state.generated[synth] as u8
        ));
        for (param, name) in lfo::PARAMS.iter().enumerate() {
            let l = state.lfo[synth][param];
            lines.push(format!("baze{n}.{name}.lfo={}", l.rate));
            lines.push(format!("baze{n}.{name}.amount={}", l.amount));
            lines.push(format!("baze{n}.{name}.sync={}", l.mode.sync as u8));
            let shape = match l.mode.shape {
                lfo::Shape::Sine => "sine",
                lfo::Shape::SampleHold => "sh",
            };
            lines.push(format!("baze{n}.{name}.shape={shape}"));
        }
        lines.push(format!("baze{n}.sub={}", state.sub[synth]));
        lines.push(format!(
            "baze{n}.sub.track={}",
            track_text(&state.sub_track[synth])
        ));
    }
    // the master's limiter
    for (i, key) in LIMIT_KEYS.into_iter().enumerate() {
        lines.push(format!("{key}={}", state.limit[i]));
        lines.push(format!("{key}.track={}", track_text(&state.limit_track[i])));
    }
    // the effects slots, by their panel ids
    let names = crate::link::fork_mixer_ids();
    for (g, v, track) in &state.mixer_fx {
        if let Some((id, _)) = names.iter().find(|x| x.1 == *g) {
            lines.push(format!("{id}={v}"));
            lines.push(format!("{id}.track={}", track_text(track)));
        }
    }
    for c in 0..4 {
        let n = c + 1;
        lines.push(format!("mixer{n}.solo={}", state.mix_solo[c] as u8));
        lines.push(format!("mixer{n}.mute={}", state.mix_mute[c] as u8));
    }
    // the drum machines' FLIP faces: 11 pans (0..127) and 11 sounds' effects lamps (bit s the
    // lamp of slot s, 0..15), BD first
    for (k, dev) in DRUMS.into_iter().enumerate() {
        let m = &state.voice_mix[k];
        let pans: Vec<String> = m.pan[1..12].iter().map(|p| p.to_string()).collect();
        let lamps: Vec<String> = m.lamps[1..12].iter().map(|l| l.to_string()).collect();
        lines.push(format!("{dev}.flip={}", state.flip[k] as u8));
        lines.push(format!("{dev}.pan={}", pans.join(",")));
        lines.push(format!("{dev}.lamps={}", lamps.join(",")));
        // each sound's SOLO and MUTE lamp, 0 or 1, BD first
        let bits = |b: &[bool; 12]| {
            let v: Vec<&str> = b[1..12]
                .iter()
                .map(|&b| if b { "1" } else { "0" })
                .collect();
            v.join(",")
        };
        lines.push(format!("{dev}.solo={}", bits(&state.drum_solo[k])));
        lines.push(format!("{dev}.mute={}", bits(&state.drum_mute[k])));
        lines.push(format!(
            "{dev}.kit={}",
            state.kits[k].as_deref().unwrap_or("")
        ));
    }
    let mut body = lines.join("\n").into_bytes();
    body.push(b'\n');
    // IFF chunks are an even number of bytes
    if body.len() % 2 == 1 {
        body.push(b'\n');
    }
    body
}

/// The settings of a chunk's body; None when it is not this format.
pub fn decode(body: &[u8]) -> Option<GenState> {
    let text = std::str::from_utf8(body).ok()?;
    let mut lines = text.lines();
    let header = lines.next()?.trim();
    let version: u32 = header.strip_prefix("ORB-338X GENERATE ")?.parse().ok()?;
    if version < 1 {
        return None;
    }
    let values: Vec<(&str, &str)> = lines
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    let get = |name: &str| values.iter().find(|(k, _)| *k == name).map(|(_, v)| *v);
    let mut state = GenState {
        comp_in_slots: version < 2,
        ..Default::default()
    };
    // BAZE 1 first: BAZE 2 is checked against it
    for synth in 0..2 {
        let n = synth + 1;
        let scale = get(&format!("baze{n}.scale"))
            .and_then(|v| SCALES.iter().position(|(name, _)| *name == v));
        let key = get(&format!("baze{n}.key")).and_then(|v| KEYS.iter().position(|k| *k == v));
        if let Some(s) = scale {
            state.scale_keys.set_scale(synth, s);
            if let Some(k) = key {
                state.scale_keys.set_key(synth, k);
            }
        }
        for op in Op::ALL {
            let names = if op == Op::Move { MOVES } else { PARAMS };
            if let Some(v) = get(&format!("baze{n}.{}", op_name(op))) {
                for item in v.split(',').map(str::trim) {
                    if let Some(i) = names.iter().position(|x| *x == item) {
                        state.sel[op as usize][synth][i] = true;
                        // MOVE has one button on at most: the first one named
                        if op == Op::Move {
                            break;
                        }
                    }
                }
            }
            state.done[op as usize][synth] =
                get(&format!("baze{n}.{}.done", op_name(op))) == Some("1");
        }
        // an older file without it: GENERATE ran if its row was done
        state.generated[synth] = get(&format!("baze{n}.generated")) == Some("1")
            || state.done[Op::Random as usize][synth];
        for (param, name) in lfo::PARAMS.iter().enumerate() {
            let l = &mut state.lfo[synth][param];
            let key = |field: &str| format!("baze{n}.{name}.{field}");
            let num = |field: &str, d: u8| {
                get(&key(field))
                    .and_then(|v| v.parse::<u8>().ok())
                    .map_or(d, |v| v.min(127))
            };
            l.rate = num("lfo", lfo::RATE_DEFAULT);
            l.amount = num("amount", lfo::AMOUNT_DEFAULT);
            l.mode.sync = get(&key("sync")) == Some("1");
            l.mode.shape = match get(&key("shape")) {
                Some("sh") => lfo::Shape::SampleHold,
                _ => lfo::Shape::Sine,
            };
        }
        state.sub[synth] = get(&format!("baze{n}.sub"))
            .and_then(|v| v.parse::<u32>().ok())
            .map_or(0, |v| v.min(127) as u8);
        if let Some(v) = get(&format!("baze{n}.sub.track")) {
            state.sub_track[synth] = track_of(v);
        }
    }
    // the master's limiter (an older file without it: off)
    for (i, key) in LIMIT_KEYS.into_iter().enumerate() {
        let max = if i == 0 { 1 } else { 127 };
        state.limit[i] = get(key)
            .and_then(|v| v.parse::<u32>().ok())
            .map_or(0, |v| v.min(max) as u8);
        if let Some(v) = get(&format!("{key}.track")) {
            state.limit_track[i] = track_of(v)
                .into_iter()
                .map(|(t, v)| (t, v.min(max as u8)))
                .collect();
        }
    }
    // the effects slots (an older file without them: a new song's)
    for (id, g) in crate::link::fork_mixer_ids() {
        let Some(v) = get(&id).and_then(|v| v.parse::<u32>().ok()) else {
            continue;
        };
        let track = get(&format!("{id}.track"))
            .map(track_of)
            .unwrap_or_default();
        state.mixer_fx.push((g, v.min(127) as u8, track));
    }
    for c in 0..4 {
        let n = c + 1;
        state.mix_solo[c] = get(&format!("mixer{n}.solo")) == Some("1");
        state.mix_mute[c] = get(&format!("mixer{n}.mute")) == Some("1");
    }
    for (k, dev) in DRUMS.into_iter().enumerate() {
        state.flip[k] = get(&format!("{dev}.flip")) == Some("1");
        let m = &mut state.voice_mix[k];
        if let Some(v) = get(&format!("{dev}.pan")) {
            for (i, p) in v.split(',').take(11).enumerate() {
                if let Ok(p) = p.trim().parse::<u8>() {
                    m.pan[i + 1] = p.min(127);
                }
            }
        }
        if let Some(v) = get(&format!("{dev}.lamps")) {
            for (i, l) in v.split(',').take(11).enumerate() {
                if let Ok(l) = l.trim().parse::<u8>() {
                    m.lamps[i + 1] = l & voices::ALL;
                }
            }
        } else if let Some(v) = get(&format!("{dev}.fx")) {
            // the older files: one FX key a sound, on as every lamp on
            for (i, f) in v.split(',').take(11).enumerate() {
                m.lamps[i + 1] = if f.trim() == "0" { 0 } else { voices::ALL };
            }
        }
        for (key, lit) in [
            ("solo", &mut state.drum_solo[k]),
            ("mute", &mut state.drum_mute[k]),
        ] {
            if let Some(v) = get(&format!("{dev}.{key}")) {
                for (i, b) in v.split(',').take(11).enumerate() {
                    lit[i + 1] = b.trim() == "1";
                }
            }
        }
        state.kits[k] = get(&format!("{dev}.kit"))
            .filter(|v| !v.is_empty())
            .map(String::from);
    }
    Some(state)
}

/// An older file's effects slots (version 1, where the compressor was one of the slots' effects)
/// as this version's: the channels that heard the compressor get their own with its settings,
/// `comp` the song's compressor's ON, AMOUNT and THRES (its RATIO and THRES), and the slot
/// that held it holds the reverb. The song's channel (`comp_id` 2..5 channel 1..4) keeps the
/// song's compressor; channels that heard only the second compressor get its settings. The
/// channels' COMP knobs (the wet amounts), the compressor on more channels and the second
/// compressor go. True when no slot held the compressor: the song's compressor on a channel
/// was not heard, and is to be off.
pub fn move_comp_to_channels(fx: &mut Vec<FxSetting>, comp_id: u8, comp: [u8; 3]) -> bool {
    use orb_engine::frame::{
        CCOMP, COMP_B, DEFAULT_SLOTS, FX_COMP, FX_DELAY, FX_DIST, FX_PCF, FX_RVRB, MORE,
        MixerParams, OTT, SLOT, WET, WET_B,
    };
    let new = MixerParams::default();
    let get = |fx: &[FxSetting], g: u8| {
        fx.iter()
            .find(|x| x.0 == g)
            .map_or_else(|| new.fork(g).unwrap_or(0), |x| x.1)
    };
    // the older files' slots: DELAY, DIST, PCF, COMP
    let old = [FX_DELAY, FX_DIST, FX_PCF, FX_COMP];
    let slots: [u8; 4] = std::array::from_fn(|s| {
        fx.iter()
            .find(|x| x.0 == SLOT + s as u8)
            .map_or(old[s], |x| x.1)
    });
    let comps = slots.iter().filter(|&&f| f == FX_COMP).count();
    let ott = [get(fx, OTT), get(fx, OTT + 1)];
    let comp_b = [get(fx, COMP_B), get(fx, COMP_B + 1), get(fx, COMP_B + 2)];
    let more: [bool; 4] = std::array::from_fn(|c| get(fx, MORE + 2 * c as u8 + 1) != 0);
    let wet_b: [u8; 4] = std::array::from_fn(|c| get(fx, WET_B + 3 * c as u8 + 2));
    let song = |c: usize| comp_id == c as u8 + 2;
    let mut own = [None; 4];
    for c in 0..4 {
        if song(c) {
            continue;
        }
        if comps >= 1 && more[c] && comp[0] != 0 {
            own[c] = Some([1, comp[1], comp[2], ott[0]]);
        } else if comps >= 2 && wet_b[c] > 0 && comp_b[0] != 0 {
            own[c] = Some([1, comp_b[1], comp_b[2], ott[1]]);
        }
    }
    // the older compressor's controllers go; the slots' compressor is the reverb
    let gone = |g: u8| {
        (0..4u8).any(|c| g == WET + 3 * c + 2 || g == MORE + 2 * c + 1 || g == WET_B + 3 * c + 2)
            || (COMP_B..COMP_B + 3).contains(&g)
            || g == OTT + 1
            || (CCOMP..CCOMP + 16).contains(&g)
    };
    fx.retain(|x| !gone(x.0));
    for s in 0..4u8 {
        let at = fx.iter().position(|x| x.0 == SLOT + s);
        let mut v = at.map_or(old[s as usize], |i| fx[i].1);
        if v == FX_COMP {
            v = FX_RVRB;
        }
        let mut track = at.map(|i| fx.remove(i).2).unwrap_or_default();
        for e in &mut track {
            if e.1 == FX_COMP {
                e.1 = FX_RVRB;
            }
        }
        if v != DEFAULT_SLOTS[s as usize] || !track.is_empty() {
            fx.push((SLOT + s, v, track));
        }
    }
    for (c, p) in own.iter().enumerate() {
        for (i, &v) in p.iter().flatten().enumerate() {
            let g = CCOMP + 4 * c as u8 + i as u8;
            if Some(v) != new.fork(g) {
                fx.push((g, v, Vec::new()));
            }
        }
    }
    fx.sort_by_key(|x| x.0);
    comps == 0 && (2..=5).contains(&comp_id)
}

/// Takes the `OGEN` chunk out of `song` and reads it: the settings it kept, if it had one.
pub fn split(song: &mut rbs::Song) -> Option<GenState> {
    let at = song.unknown.iter().position(|u| u.tag == TAG)?;
    let chunk = song.unknown.remove(at);
    song.unknown.retain(|u| u.tag != TAG);
    decode(&chunk.body)
}

/// `song` with `state` in its `OGEN` chunk, after its own chunks; an empty state adds none.
pub fn attach(song: &rbs::Song, state: &GenState) -> rbs::Song {
    let mut out = song.clone();
    out.unknown.retain(|u| u.tag != TAG);
    if !state.is_empty() {
        out.unknown.push(rbs::Unknown {
            // after everything (the writer clamps it to the end)
            index: usize::MAX,
            tag: TAG,
            body: encode(state),
        });
    }
    out
}

/// The answers of Save's question for an `.rbs` song with GENERATE settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatAnswer {
    /// Save As an `.rbsx` (the name and folder of the song, `.rbsx`), keeping the settings.
    Rbsx,
    /// Save the `.rbs` as it is, without the settings.
    Rbs,
    Cancel,
}

impl App {
    /// Whether Save's question about the `.rbsx` is open.
    pub fn format_prompting(&self) -> bool {
        self.format_prompt
    }

    /// Answers Save's question: an `.rbsx` opens Save As on the song's name with `.rbsx` (its
    /// overwrite check included), an `.rbs` saves the file as it is.
    pub fn format_answer(&mut self, answer: FormatAnswer) {
        if !std::mem::take(&mut self.format_prompt) {
            return;
        }
        let Some(path) = self.doc.path.clone() else {
            return;
        };
        match answer {
            FormatAnswer::Rbsx => {
                let name = path.with_extension(EXTENSION);
                let name = name.file_name().map(|n| n.to_string_lossy().into_owned());
                let mut b = Browser::new(Mode::SaveSong, &self.settings);
                if let Some(dir) = path.parent() {
                    b = b.with_dir(dir);
                }
                self.browser = Some(b.with_name(&name.unwrap_or_default()));
            }
            FormatAnswer::Rbs => self.save_to(&path),
            // a Save the unsaved-changes prompt started ends here
            FormatAnswer::Cancel => self.after_save = None,
        }
    }

    /// Draws Save's question and takes its keys: Enter or X saves an `.rbsx`, R the `.rbs`,
    /// Escape cancels.
    pub fn format_prompt_ui(&mut self, ctx: &Context) {
        if !self.format_prompt {
            return;
        }
        let key = |k: Key| ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k));
        let mut answer = None;
        if key(Key::Enter) || key(Key::X) {
            answer = Some(FormatAnswer::Rbsx);
        } else if key(Key::R) {
            answer = Some(FormatAnswer::Rbs);
        } else if key(Key::Escape) {
            answer = Some(FormatAnswer::Cancel);
        }
        let frame = Frame::new()
            .fill(SLATE_DARK)
            .stroke(Stroke::new(1.0, SLATE_LINE))
            .inner_margin(Margin::same(12))
            .corner_radius(3.0);
        Modal::new(Id::unique("orb_rbsx_format"))
            .frame(frame)
            .backdrop_color(Color32::from_black_alpha(170))
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.label(
                    RichText::new(format!("Save {} as .rbsx?", self.doc.name()))
                        .color(INK)
                        .strong()
                        .size(15.0),
                );
                ui.label(
                    RichText::new(
                        "The song has GENERATE settings. An .rbsx keeps them with the song; \
                         an .rbs stays a plain ReBirth song without them.",
                    )
                    .color(INK_DIM),
                );
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        answer = Some(FormatAnswer::Cancel);
                    }
                    if ui.button("Save as .rbs").clicked() {
                        answer = Some(FormatAnswer::Rbs);
                    }
                    if ui.button("Save as .rbsx").clicked() {
                        answer = Some(FormatAnswer::Rbsx);
                    }
                });
            });
        if let Some(a) = answer {
            self.format_answer(a);
        }
    }

    /// The GENERATE menu's settings of both synths, as an `.rbsx` keeps them.
    pub fn gen_state(&self) -> GenState {
        let lfo = std::array::from_fn(|k| {
            std::array::from_fn(|p| {
                let [_, rate, amount, _] = lfo::ids(k, p);
                LfoState {
                    rate: self.panel.value(&rate).unwrap_or(lfo::RATE_DEFAULT),
                    amount: self.panel.value(&amount).unwrap_or(lfo::AMOUNT_DEFAULT),
                    mode: self.lfo_mode[k][p],
                }
            })
        });
        GenState {
            scale_keys: self.scale_keys,
            sel: self.op_sel,
            done: self.op_done,
            generated: self.generated,
            mix_solo: self.panel.solo,
            mix_mute: self.panel.mute,
            lfo,
            flip: [self.panel.flipped(0), self.panel.flipped(1)],
            voice_mix: [self.panel.voice_mix(0), self.panel.voice_mix(1)],
            drum_solo: self.panel.drum_solo,
            drum_mute: self.panel.drum_mute,
            // the sub's level as a save keeps the 303's knobs: in song mode the pattern-mode
            // copy, else the panel's
            sub: std::array::from_fn(|k| {
                let saved = self.link.as_ref().and_then(|l| l.song_mode_blocks());
                saved.map_or_else(
                    || {
                        self.panel
                            .value(&format!("tb303_{}.sub", k + 1))
                            .unwrap_or(0)
                    },
                    |b| b.sub[k],
                )
            }),
            sub_track: std::array::from_fn(|k| {
                self.link
                    .as_ref()
                    .map(|l| crate::document::sub_events(l.loaded_tracks(), k))
                    .unwrap_or_default()
            }),
            // the master's limiter as a save keeps the mixer: in song mode the pattern-mode
            // copy, else the panel's
            limit: std::array::from_fn(|i| {
                let saved = self.link.as_ref().and_then(|l| l.song_mode_blocks());
                saved.map_or_else(
                    || self.panel.value(LIMIT_KEYS[i]).unwrap_or(0),
                    |b| [b.mixer.limit, b.mixer.limit_amt][i],
                )
            }),
            limit_track: std::array::from_fn(|i| {
                self.link
                    .as_ref()
                    .map(|l| crate::document::limit_events(l.loaded_tracks(), i))
                    .unwrap_or_default()
            }),
            mixer_fx: self.mixer_fx_state(),
            comp_in_slots: false,
            kits: std::array::from_fn(|k| {
                self.kit_of(k).map(|kit| match kit {
                    orb_cli::mods::Kit::Standard => orb_cli::mods::STANDARD.to_string(),
                    orb_cli::mods::Kit::Mod(p) => crate::session::mod_info(p)
                        .map(|i| i.name)
                        .unwrap_or_default(),
                })
            }),
        }
    }

    /// The effects slots' mixer controllers an `.rbsx` keeps ([`GenState::mixer_fx`]), as a
    /// save keeps the mixer: in song mode the pattern-mode copy, else the live values.
    fn mixer_fx_state(&self) -> Vec<FxSetting> {
        let Some(link) = &self.link else {
            return Vec::new();
        };
        let m = link.song_mode_blocks().map_or(*link.mixer(), |b| b.mixer);
        let new_song = orb_engine::frame::MixerParams::default();
        crate::link::fork_mixer_ids()
            .into_iter()
            .filter_map(|(_, g)| {
                let v = m.fork(g)?;
                let track = crate::document::fork_events(link.loaded_tracks(), g);
                (Some(v) != new_song.fork(g) || !track.is_empty()).then_some((g, v, track))
            })
            .collect()
    }

    /// Sets the GENERATE menu as a song kept it (an empty state for a song without one); the
    /// menu's open list and a row being chosen are let go.
    pub fn set_gen_state(&mut self, state: GenState) {
        self.scale_keys = state.scale_keys;
        self.op_sel = state.sel;
        self.op_done = state.done;
        self.generated = state.generated;
        // SOLO and MUTE as the song kept them; a song without them (an .rbs, an older .rbsx):
        // MUTE on where the channel does not play
        if state
            .mix_solo
            .iter()
            .chain(state.mix_mute.iter())
            .any(|&b| b)
        {
            self.panel.solo = state.mix_solo;
            self.panel.mute = state.mix_mute;
        } else {
            self.panel.solo = [false; 4];
            self.panel.mute =
                std::array::from_fn(|c| !self.panel.toggle_on(&format!("mixer{}.mute", c + 1)));
        }
        self.panel.apply_channels();
        self.generate_list = None;
        // another song: the UNDO steps were the other song's
        self.gen_undo = Default::default();
        self.gen_redo = Default::default();
        // the LFOs stop; their knobs and buttons as the song kept them
        self.lfo_reset_knobs();
        let to = |v: u8| crate::behavior::knob_from_engine(v as i32);
        for (k, knobs) in state.lfo.iter().enumerate() {
            for (p, l) in knobs.iter().enumerate() {
                let [_, rate, amount, _] = lfo::ids(k, p);
                self.panel.put_knob(&rate, to(l.rate));
                self.panel.put_knob(&amount, to(l.amount));
                self.lfo_mode[k][p] = l.mode;
            }
        }
        // the 303s' sub-oscillators (this fork): their knobs, and the level and track events
        // into the song
        for k in 0..2 {
            self.panel
                .put_knob(&format!("tb303_{}.sub", k + 1), to(state.sub[k]));
            if let Some(link) = &mut self.link {
                link.send_sub(k, state.sub[k], &state.sub_track[k]);
            }
        }
        // the master's limiter (this fork): its key and knob, and the values and track events
        // into the song
        self.panel.put_toggle(LIMIT_KEYS[0], state.limit[0] != 0);
        self.panel.put_knob(LIMIT_KEYS[1], to(state.limit[1]));
        if let Some(link) = &mut self.link {
            for (i, ctrl) in crate::document::LIMIT_CTRLS.into_iter().enumerate() {
                let (v, track) = (state.limit[i], &state.limit_track[i]);
                link.send_fork_track(orb_engine::song::Device::Mixer, ctrl, v, track);
            }
            // the effects slots (this fork): the panel follows the engine
            for (ctrl, v, track) in &state.mixer_fx {
                link.send_fork_track(orb_engine::song::Device::Mixer, *ctrl, *v, track);
            }
        }
        // the drum machines' FLIP faces (this fork) and their own kits
        self.voice_mix_seen = state.voice_mix;
        for (k, sec) in crate::model::FLIP_FACES.into_iter().enumerate() {
            let dev = ["tr808", "tr909"][k];
            self.panel.put_toggle(&format!("{dev}.flip"), state.flip[k]);
            let m = &state.voice_mix[k];
            for i in 1..12 {
                // a knob already at the value keeps its position (the middle is 500, not 504)
                let id = format!("{sec}.pan{i}");
                if crate::behavior::knob_send(self.panel.knob(&id)) != m.pan[i] as i32 {
                    self.panel.put_knob(&id, to(m.pan[i]));
                }
                for s in 0..4 {
                    let on = m.lamps[i] & (1 << s) != 0;
                    self.panel.put_toggle(&format!("{sec}.fx{i}_{s}"), on);
                }
            }
            // each sound's SOLO and MUTE (none in a song without them), heard as the panel
            // makes the mix
            self.panel.drum_solo[k] = state.drum_solo[k];
            self.panel.drum_mute[k] = state.drum_mute[k];
            self.voice_mix_seen[k].silent = self.panel.voice_mix(k).silent;
            if let Some(name) = &state.kits[k] {
                let kit = self.kit_for(name);
                self.put_kit_of(k, Some(kit));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str) -> usize {
        SCALES.iter().position(|(n, _)| *n == name).unwrap()
    }

    fn k(name: &str) -> usize {
        KEYS.iter().position(|n| *n == name).unwrap()
    }

    fn sample() -> GenState {
        let mut st = GenState::default();
        st.scale_keys.set_scale(0, s("IONIAN"));
        st.scale_keys.set_key(0, k("C"));
        st.scale_keys.set_scale(1, s("DORIAN"));
        st.scale_keys.set_key(1, k("D"));
        st.sel[Op::Random as usize][0] = [false, false, true, true, true];
        st.sel[Op::Alter as usize][1] = [true, false, false, false, true];
        st.sel[Op::Move as usize][0] = [false, false, false, true, false];
        st.done[Op::Random as usize][0] = true;
        st.generated[0] = true;
        st.mix_solo[1] = true;
        st.mix_mute[3] = true;
        st.drum_solo[0][2] = true;
        st.drum_mute[1][11] = true;
        st.done[Op::Move as usize][0] = true;
        st.lfo[1][0] = LfoState {
            rate: 90,
            amount: 40,
            mode: lfo::Mode {
                sync: true,
                shape: lfo::Shape::SampleHold,
            },
        };
        st.flip[1] = true;
        st.voice_mix[0].pan[3] = 10;
        st.voice_mix[1].lamps[11] = 0;
        st.voice_mix[0].lamps[2] = 0b0101;
        st.voice_mix[1].pan[1] = 127;
        st.kits[1] = Some("PBE2".into());
        st.sub = [0, 90];
        st.sub_track[1] = vec![(0, 40), (984, 110), (1008, 40)];
        st.limit = [1, 70];
        st.limit_track[1] = vec![(0, 70), (768, 127)];
        st.mixer_fx = vec![
            (orb_engine::frame::SLOT, 3, vec![]),
            (orb_engine::frame::WET_B + 4, 100, vec![(0, 100), (384, 20)]),
            (orb_engine::frame::PCF_B + 4, 30, vec![]),
        ];
        st
    }

    /// The older files' FX keys (one a sound): on as every lamp on, off as none.
    #[test]
    fn an_older_files_fx_keys_light_every_lamp_or_none() {
        let st = sample();
        let text = String::from_utf8(encode(&st)).unwrap();
        let lamps = |dev: &str| {
            let line = text
                .lines()
                .find(|l| l.starts_with(&format!("{dev}.lamps=")));
            line.unwrap().to_string()
        };
        let old = text
            .replace(&lamps("tr808"), "tr808.fx=1,0,1,1,1,1,1,1,1,1,1")
            .replace(&lamps("tr909"), "");
        let got = decode(old.as_bytes()).unwrap();
        let m = &got.voice_mix[0];
        assert!(m.lamps[1] == voices::ALL && m.lamps[2] == 0 && m.lamps[11] == voices::ALL);
        assert_eq!(got.voice_mix[1].lamps, [voices::ALL; 12], "no key: all on");
    }

    #[test]
    fn the_settings_come_back_as_they_were_written() {
        let st = sample();
        let body = encode(&st);
        assert_eq!(body.len() % 2, 0, "an even chunk");
        assert_eq!(decode(&body), Some(st));
        let text = String::from_utf8(body).unwrap();
        assert!(text.starts_with("ORB-338X GENERATE 2\n"));
        assert!(text.contains("baze1.scale=IONIAN\n"));
        assert!(text.contains("baze1.random=ACCENT,SLIDE,NOTE\n"));
        assert!(text.contains("baze1.move=+1\n"));
        assert!(text.contains("baze2.cutoff.lfo=90\nbaze2.cutoff.amount=40\n"));
        assert!(text.contains("baze2.cutoff.sync=1\nbaze2.cutoff.shape=sh\n"));
        assert!(text.contains("baze1.cutoff.amount=0\n"));
        // (this fork) the effects slots, by their panel ids
        assert!(text.contains("mixer.slot1=3\nmixer.slot1.track=\n"));
        assert!(text.contains("mixer2.pcf_b=100\nmixer2.pcf_b.track=0:100,384:20\n"));
        assert!(text.contains("pcf_b.pattern=30\n"));
        assert!(text.contains("baze2.decay.lfo=64\nbaze2.decay.amount=0\n"));
        assert_eq!(
            decode(&encode(&GenState::default())),
            Some(GenState::default())
        );
        // (this fork) an older file has no `generated`: a done GENERATE row means it ran
        assert!(text.contains("baze1.generated=1\n"));
        assert!(text.contains("mixer2.solo=1\nmixer2.mute=0\n"));
        // (this fork) the FLIP faces
        assert!(text.contains("tr808.flip=0\ntr808.pan=64,64,10,64,"));
        assert!(text.contains("tr909.flip=1\ntr909.pan=127,64,"));
        assert!(text.contains("tr808.lamps=15,5,15,15,15,15,15,15,15,15,15\n"));
        assert!(text.contains("tr909.lamps=15,15,15,15,15,15,15,15,15,15,0\ntr909.solo="));
        assert!(text.contains("tr909.mute=0,0,0,0,0,0,0,0,0,0,1\ntr909.kit=PBE2\n"));
        // (this fork) each drum sound's SOLO and MUTE
        assert!(text.contains("tr808.solo=0,1,0,0,0,0,0,0,0,0,0\ntr808.mute=0,0,0,"));
        assert!(text.contains("mixer4.solo=0\nmixer4.mute=1\n"));
        // (this fork) the sub-oscillators
        assert!(text.contains("baze1.sub=0\nbaze1.sub.track=\n"));
        assert!(text.contains("baze2.sub=90\nbaze2.sub.track=0:40,984:110,1008:40\n"));
        // (this fork) the master's limiter
        assert!(text.contains("master.limit=1\nmaster.limit.track=\n"));
        assert!(text.contains("master.limit_amt=70\nmaster.limit_amt.track=0:70,768:127\n"));
        // the key is 0 or 1
        let on = text.replace("master.limit=1\n", "master.limit=5\n");
        assert_eq!(decode(on.as_bytes()).map(|s| s.limit), Some([1, 70]));
        let old = text.replace("baze1.generated=1\n", "");
        assert_eq!(
            decode(old.as_bytes()).map(|s| s.generated),
            Some([true, false])
        );
    }

    /// (this fork) An older file's compressor of the slots goes to the channels that heard it.
    #[test]
    fn an_older_files_compressor_goes_to_the_channels() {
        use orb_engine::frame::{
            CCOMP, COMP_B, FX_COMP, FX_PHA, FX_RVRB, MORE, OTT, SLOT, WET, WET_B,
        };
        let v1 = b"ORB-338X GENERATE 1\n";
        assert!(decode(v1).unwrap().comp_in_slots);
        assert!(!decode(&encode(&GenState::default())).unwrap().comp_in_slots);
        let song_comp = [1, 90, 30];
        // the default slots: the song's channel 2 keeps it; channel 3 had it as more, its OTT
        // on; the COMP knob of channel 2 goes
        let mut fx = vec![
            (WET + 3 + 2, 40, vec![]),
            (MORE + 2 * 2 + 1, 1, vec![]),
            (OTT, 1, vec![]),
        ];
        assert!(!move_comp_to_channels(&mut fx, 3, song_comp));
        let c3 = CCOMP + 8;
        let want = vec![
            (OTT, 1, vec![]),
            (c3, 1, vec![]),
            (c3 + 1, 90, vec![]),
            (c3 + 2, 30, vec![]),
            (c3 + 3, 1, vec![]),
        ];
        assert_eq!(fx, want);
        // a second COMP: channel 1 heard it, with its own settings; both slots the reverb
        let mut fx = vec![
            (SLOT + 1, FX_COMP, vec![(0, FX_COMP), (96, FX_PHA)]),
            (WET_B + 2, 127, vec![]),
            (COMP_B + 1, 20, vec![]),
            (COMP_B + 2, 100, vec![]),
        ];
        assert!(!move_comp_to_channels(&mut fx, 0, song_comp));
        let want = vec![
            (SLOT + 1, FX_RVRB, vec![(0, FX_RVRB), (96, FX_PHA)]),
            (CCOMP, 1, vec![]),
            (CCOMP + 1, 20, vec![]),
            (CCOMP + 2, 100, vec![]),
        ];
        assert_eq!(fx, want);
        // no slot held it: the song's compressor on channel 1 was not heard
        let mut fx = vec![(SLOT + 3, FX_PHA, vec![]), (MORE + 1, 1, vec![])];
        assert!(move_comp_to_channels(&mut fx, 2, song_comp));
        assert_eq!(fx, vec![(SLOT + 3, FX_PHA, vec![])]);
        // on the master: as it was
        let mut fx = vec![(SLOT + 3, FX_PHA, vec![])];
        assert!(!move_comp_to_channels(&mut fx, 1, song_comp));
    }

    #[test]
    fn a_sub_track_is_read_leniently() {
        // out of order, off the tick grid, negative, too loud, not a number
        let text = "ORB-338X GENERATE 1\nbaze1.sub=300\nbaze1.sub.track=48:5,7:1,-24:3,0:200,x\n";
        let st = decode(text.as_bytes()).unwrap();
        assert_eq!(st.sub, [127, 0]);
        assert_eq!(st.sub_track[0], [(0, 127), (48, 5)]);
        assert!(st.sub_track[1].is_empty());
    }

    #[test]
    fn decoding_is_lenient_but_keeps_the_rules() {
        // unknown names are skipped, MOVE keeps one button, a BAZE 2 that does not fit BAZE 1
        // is left out, and something else than this format is no state
        let body = b"ORB-338X GENERATE 2\nfuture.thing=7\nbaze1.scale=IONIAN\nbaze1.key=C\n\
                     baze1.move=-2,+2\nbaze2.scale=BLUES?\nbaze2.key=D\n\
                     baze2.scale=HARM MINOR\n";
        let st = decode(body).unwrap();
        assert_eq!(
            st.scale_keys.get(0).mask(),
            Some(crate::scale::mask(s("IONIAN"), 0))
        );
        assert_eq!(
            st.sel[Op::Move as usize][0],
            [true, false, false, false, false]
        );
        assert_eq!(st.scale_keys.get(1), Default::default());
        assert_eq!(decode(b"something else"), None);
        assert_eq!(decode(&[0xff, 0xfe]), None);
    }

    #[test]
    fn an_rbs_song_carries_the_chunk_at_its_end_and_gives_it_back() {
        let song = rbs::Song::empty();
        let plain = song.write().unwrap();
        // nothing set: no chunk, the same bytes
        assert_eq!(attach(&song, &GenState::default()).write().unwrap(), plain);
        let with = attach(&song, &sample()).write().unwrap();
        assert!(with.len() > plain.len());
        // the song's own chunks come first, unchanged; the RB40 size grows
        assert_eq!(with[12..plain.len()], plain[12..]);
        let mut back = rbs::Song::parse(&with).unwrap();
        assert_eq!(split(&mut back), Some(sample()));
        // taken out again, the song writes as it was
        assert_eq!(back.write().unwrap(), plain);
        let mut none = rbs::Song::parse(&plain).unwrap();
        assert_eq!(split(&mut none), None);
    }

    #[test]
    fn the_rbsx_extension_is_known_in_any_case() {
        assert!(is_rbsx(Path::new("a/Song.rbsx")));
        assert!(is_rbsx(Path::new("Song.RBSX")));
        assert!(!is_rbsx(Path::new("Song.rbs")));
        assert!(!is_rbsx(Path::new("Song")));
    }
}
