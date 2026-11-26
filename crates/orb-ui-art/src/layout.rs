//! Where every control sits, and the composition of the full panel from the
//! parts. The list is written to `layout.tsv` and drawn into `panel.svg`.
//! The sections and the controls' functions are the original's
//! (docs/ui_spec.md); the placement inside a section is orb's own design and
//! may differ from the original's rectangles.

use std::collections::HashMap;

use crate::parts::{
    BLACK_KEY_W_303, BLACK_NOTES_303, CLEAR_Y_303, FLAGS_303, KEY_ROWS_303, KNOB_SWEEP, KNOB_W_303,
    KNOB_Y_303, KNOB_Y_909, KNOBS, KNOBS_909, LEFT_X_303, LEGEND808_Y, LFO_MODE_W, LFO_MODE_Y_303,
    LOGO_X, LOGO_Y, NAMES_Y_909, PILL_W_303, PILL808_H, PILL808_W, PITCH_LED_Y_303, PITCH_Y_303,
    STEP808_Y, STEPS_Y_909, TUNE_Y_303, UPPER808_Y, WAVE_W_303, WAVE_X_303, WAVE_Y_303, WHITE_303,
    black_x_303, col808, column_909, knob_x_303, knob_y_909, lfo_knobs_303, rndm_303, step_led_303,
    step808_x,
};
use crate::svg::{Part, Svg, arc_path, n, pal};

pub const PANEL_W: f32 = 624.0;
pub const PANEL_H: f32 = 510.0;

/// How a control is composed from parts.
pub enum Extra {
    /// One part at (x, y).
    Plain,
    /// `<knob>_body`, the value arc, and `<knob>_pointer` rotated by the value.
    Knob {
        design: usize,
        v: f32,
        bipolar: bool,
    },
    /// `fader_track` and `fader_handle` at `(1 - v) * 43` px from the top.
    Fader { v: f32 },
    /// `n` digit frames, right-aligned in the display rectangle.
    Digits { n: usize, text: String },
}

pub struct Ctl {
    pub id: String,
    pub section: String,
    pub kind: &'static str,
    pub part: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub extra: Extra,
}

pub struct Layout {
    /// Section backgrounds: part name and panel position.
    pub backgrounds: Vec<(String, f32, f32)>,
    pub ctls: Vec<Ctl>,
}

struct Builder<'a> {
    sizes: &'a HashMap<String, (f32, f32)>,
    layout: Layout,
    section: String,
    ox: f32,
    oy: f32,
}

impl Builder<'_> {
    fn section(&mut self, name: &str, bg: &str, ox: f32, oy: f32) {
        self.section = name.to_string();
        self.ox = ox;
        self.oy = oy;
        self.layout.backgrounds.push((bg.to_string(), ox, oy));
    }

    /// A section with no background of its own (this fork: a face the app draws over another
    /// section's, `ccomp`).
    fn section_over(&mut self, name: &str, ox: f32, oy: f32) {
        self.section = name.to_string();
        self.ox = ox;
        self.oy = oy;
    }

    fn size(&self, part: &str) -> (f32, f32) {
        *self
            .sizes
            .get(part)
            .unwrap_or_else(|| panic!("layout names a missing part {part}"))
    }

    fn push(&mut self, id: &str, kind: &'static str, part: &str, x: f32, y: f32, extra: Extra) {
        let (w, h) = match &extra {
            Extra::Knob { design, .. } => (KNOBS[*design].w, KNOBS[*design].h),
            _ => self.size(part),
        };
        self.layout.ctls.push(Ctl {
            id: format!("{}.{id}", self.section),
            section: self.section.clone(),
            kind,
            part: part.to_string(),
            x: self.ox + x,
            y: self.oy + y,
            w,
            h,
            extra,
        });
    }

    fn part(&mut self, id: &str, kind: &'static str, part: &str, x: f32, y: f32) {
        self.push(id, kind, part, x, y, Extra::Plain);
    }

    fn knob(&mut self, id: &str, design: usize, x: f32, y: f32, v: f32) {
        self.knob_of("knob", id, design, x, y, v);
    }

    /// A knob of kind `kind`: "knob" for the engine's controls, "lfoknob" for this fork's LFO
    /// knobs (drawn and turned the same, but no engine control of their own).
    fn knob_of(&mut self, kind: &'static str, id: &str, design: usize, x: f32, y: f32, v: f32) {
        let name = KNOBS[design].name;
        self.push(
            id,
            kind,
            name,
            x,
            y,
            Extra::Knob {
                design,
                v,
                bipolar: false,
            },
        );
    }

    fn pan(&mut self, id: &str, design: usize, x: f32, y: f32, v: f32) {
        let name = KNOBS[design].name;
        self.push(
            id,
            "knob",
            name,
            x,
            y,
            Extra::Knob {
                design,
                v,
                bipolar: true,
            },
        );
    }

    fn fader(&mut self, id: &str, x: f32, y: f32, v: f32) {
        self.push(id, "fader", "fader_track", x, y, Extra::Fader { v });
    }

    /// A seven-segment display: the rectangle holds `digits` frames of 9 x 16. This fork draws
    /// every display with the lighter unlit segments ("segb").
    fn display(&mut self, id: &str, x: f32, y: f32, w: f32, digits: usize, text: &str) {
        self.display_of("segb", id, x, y, w, digits, text);
    }

    /// A display drawn with the digit parts `<prefix>_0..9` and `<prefix>_blank` ("seg", or
    /// this fork's "segb" with lighter unlit segments).
    #[allow(clippy::too_many_arguments)]
    fn display_of(
        &mut self,
        prefix: &str,
        id: &str,
        x: f32,
        y: f32,
        w: f32,
        digits: usize,
        text: &str,
    ) {
        self.layout.ctls.push(Ctl {
            id: format!("{}.{id}", self.section),
            section: self.section.clone(),
            kind: "display",
            part: format!("{prefix}_0..9,{prefix}_blank"),
            x: self.ox + x,
            y: self.oy + y,
            w,
            h: 16.0,
            extra: Extra::Digits {
                n: digits,
                text: text.to_string(),
            },
        });
    }
}

const KNOB_303: usize = 0;
const KNOB_808_RED: usize = 1;
const KNOB_808_WHITE: usize = 2;
const KNOB_909: usize = 3;
const KNOB_LFO: usize = 5;
const KNOB_FX: usize = 6;
/// The mixer's, the master's and the effects' small knobs (this fork; the 303's LFO knobs are
/// [`KNOB_LFO`]).
const KNOB_SMALL: usize = 7;
/// The instruments' column (the original's and the mockup's 90: 4 px between it and the
/// pattern sections, the focus bar in them).
const INSTR_X: f32 = 90.0;
/// The top of the header meters (the mixer's and the effects', 12 high): centered on the
/// header's face, 2..18.7, above its 1.3 px darker lip.
const HEADER_METER_Y: f32 = 4.35;
const KNOB_22: usize = 4;

/// The control rectangles of the whole panel, with the demo state used for
/// the composed preview (pattern mode off, song mode, playing).
pub fn layout(parts: &[Part]) -> Layout {
    let sizes: HashMap<String, (f32, f32)> =
        parts.iter().map(|p| (p.name.clone(), (p.w, p.h))).collect();
    let mut b = Builder {
        sizes: &sizes,
        layout: Layout {
            backgrounds: Vec::new(),
            ctls: Vec::new(),
        },
        section: String::new(),
        ox: 0.0,
        oy: 0.0,
    };
    b.section("wood_left", "wood", 0.0, 0.0);
    b.section("wood_right", "wood", 608.0, 0.0);
    transport(&mut b);
    for r in 0..4 {
        pattern(&mut b, r);
    }
    tb303(&mut b, 0);
    tb303(&mut b, 1);
    tr808(&mut b);
    tr909(&mut b);
    tr808_flip(&mut b);
    tr909_flip(&mut b);
    for c in 0..4 {
        mixer(&mut b, c);
    }
    for c in 0..4 {
        channel_comp(&mut b, c);
    }
    master(&mut b);
    effects(&mut b);
    // the sections cut out of the mockup (this fork): their controls where it has them
    crate::cuts::place(&mut b.layout);
    b.layout
}

fn transport(b: &mut Builder) {
    b.section("transport", "bg_transport", 16.0, 0.0);
    // the wordmark over the transport background (its pieces are not drawn here)
    b.layout
        .backgrounds
        .push(("logo".to_string(), LOGO_X, LOGO_Y));
    // centered with its label in the space left of the tempo display (0..44)
    b.knob("shuffle", KNOB_22, 11.0, 10.3, 0.3);
    b.display("tempo", 44.0, 27.0, 27.0, 3, "140");
    b.part("tempo_up", "repeat", "arrow_inc_rest", 73.0, 26.0);
    b.part("tempo_down", "repeat", "arrow_dec_rest", 73.0, 35.0);
    b.part("mode", "switch", "switch_mode_1", 161.0, 2.0);
    b.part("play", "trigger", "btn_play_lit", 96.0, 21.0);
    b.part("stop", "trigger", "btn_stop_rest", 128.0, 21.0);
    b.part("rewind", "wind", "btn_rewind_rest", 160.0, 21.0);
    b.part("ff", "wind", "btn_ff_rest", 192.0, 21.0);
    b.part("record", "toggle", "btn_record_rest", 224.0, 21.0);
    // BAR, START and LENGTH: their unlit segments a little lighter (this fork)
    b.display_of("segb", "bar", 271.0, 27.0, 27.0, 3, "  5");
    b.part("bar_up", "wind", "arrow_inc_rest", 300.0, 26.0);
    b.part("bar_down", "wind", "arrow_dec_rest", 300.0, 35.0);
    b.display_of("segb", "loop_start", 329.0, 27.0, 27.0, 3, "  5");
    b.part("loop_start_up", "repeat", "arrow_inc_rest", 358.0, 26.0);
    b.part("loop_start_down", "repeat", "arrow_dec_rest", 358.0, 35.0);
    b.display_of("segb", "loop_length", 372.0, 27.0, 27.0, 3, "  4");
    b.part("loop_length_up", "repeat", "arrow_inc_rest", 401.0, 26.0);
    b.part("loop_length_down", "repeat", "arrow_dec_rest", 401.0, 35.0);
    // level with the length arrows (26..44)
    b.part("loop", "toggle", "toggle_loop_on", 417.0, 26.0);
}

fn pattern(b: &mut Builder, r: usize) {
    let oy = 50.0 + 115.0 * r as f32;
    b.section(&format!("pattern{}", r + 1), "bg_pattern", 16.0, oy);
    b.part("on", "toggle", "power_13x13_on", 4.0, 4.0);
    let lit = [0usize, 2, 1, 5][r];
    for i in 0..8 {
        let state = if i == lit { "on" } else { "off" };
        b.part(
            &format!("pattern{}", i + 1),
            "grid",
            &format!("pat_btn_{}_{state}", i + 1),
            4.0 + 16.0 * (i % 4) as f32,
            25.0 + 18.0 * (i / 4) as f32,
        );
    }
    for (i, name) in ["A", "B", "C", "D"].into_iter().enumerate() {
        let state = if i == 0 { "on" } else { "off" };
        b.part(
            &format!("bank{name}"),
            "grid",
            &format!("bank_btn_{name}_{state}"),
            4.0 + 16.0 * i as f32,
            64.0,
        );
    }
    // level with the steps display (95..111)
    b.part("shuffle", "toggle", "toggle_shuffle_off", 3.0, 97.0);
    b.display("steps", 36.0, 95.0, 19.0, 2, "16");
    b.part("steps_up", "repeat", "arrow_inc_rest", 56.0, 94.0);
    b.part("steps_down", "repeat", "arrow_dec_rest", 56.0, 103.0);
    b.section(
        &format!("focus{}", r + 1),
        if r == 0 { "focus_on" } else { "focus_off" },
        INSTR_X - 4.0,
        oy,
    );
}

fn tb303(b: &mut Builder, r: usize) {
    let oy = 50.0 + 115.0 * r as f32;
    b.section(&format!("tb303_{}", r + 1), "bg_tb303", INSTR_X, oy);
    // The waveform: the saw and the square on one frame over the pitch/clear block, under the
    // label; a click on either chooses it (orb-ui's App::control).
    b.part("wave", "hswitch", "switch_wave_0", WAVE_X_303, WAVE_Y_303);
    // The sub-oscillator's level (this fork): the LFO rows' small knob right of the waveform,
    // level with it (cuts.rs puts it where the user's 303.svg has it)
    b.knob(
        "sub",
        KNOB_LFO,
        WAVE_X_303 + WAVE_W_303 + 2.0,
        WAVE_Y_303 - 0.5,
        0.0,
    );
    // TUNE over the waveform in its column (this fork); the other five spread along the paper
    b.knob(
        "tune",
        KNOB_303,
        LEFT_X_303 - KNOB_W_303 / 2.0,
        TUNE_Y_303,
        0.5,
    );
    let vals = [0.35, 0.6, 0.45, 0.4, 0.55];
    let half = KNOBS[KNOB_LFO].w / 2.0;
    for (k, name) in ["cutoff", "reso", "envmod", "decay", "accent"]
        .into_iter()
        .enumerate()
    {
        b.knob(name, KNOB_303, knob_x_303(k), KNOB_Y_303, vals[k]);
        // its LFO (this fork): the RATE knob up left of it, under its label, the AMOUNT knob
        // down right of it (the 909's small knob), and the mode buttons (S, sine, sample &
        // hold) under it
        let ((rx, ry), (ax, ay)) = lfo_knobs_303(k);
        let lfo = format!("{name}_lfo");
        b.knob_of("lfoknob", &lfo, KNOB_LFO, rx - half, ry - half, 0.5);
        b.knob_of(
            "lfoknob",
            &format!("{name}_amt"),
            KNOB_LFO,
            ax - half,
            ay - half,
            0.0,
        );
        b.part(
            &format!("{name}_lfo_mode"),
            "lfomode",
            "lfo_mode_00",
            knob_x_303(k) + KNOB_W_303 / 2.0 - LFO_MODE_W / 2.0,
            LFO_MODE_Y_303,
        );
    }
    // The step LEDs (this fork, instead of the edit step display and its arrows): a click
    // chooses the edit step; and the GENERATE menu's button, RNDM, right of them.
    for i in 0..16 {
        let (x, y) = step_led_303(i);
        b.part(&format!("stepled{}", i + 1), "stepled", "stepled_on", x, y);
    }
    let (gx, gy) = rndm_303();
    b.part("generate", "trigger", "btn_rndm_rest", gx, gy);
    // Pitch mode and clear, centered in the band's left block (x 12..46).
    b.part(
        "pitch_led",
        "led",
        "led_8_off",
        LEFT_X_303 - 4.0,
        PITCH_LED_Y_303,
    );
    b.part(
        "pitch_button",
        "trigger",
        "btn_pitch_rest",
        LEFT_X_303 - PILL_W_303 / 2.0,
        PITCH_Y_303,
    );
    b.part(
        "clear",
        "trigger",
        "btn_clear_rest",
        LEFT_X_303 - PILL_W_303 / 2.0,
        CLEAR_Y_303,
    );
    // The keys, in the rows of `KEY_ROWS_303`: each LED above its key.
    let [black_led_y, black_y, white_led_y, white_y] = KEY_ROWS_303;
    let white_notes = [0, 2, 4, 5, 7, 9, 11, 12];
    let black_notes = BLACK_NOTES_303;
    let note_lit = 0; // C
    for (x, note) in WHITE_303.into_iter().zip(white_notes) {
        b.part(
            &format!("key{note}"),
            "trigger",
            "key_white_rest",
            x,
            white_y,
        );
        let led = if note == note_lit {
            "led_8_on"
        } else {
            "led_8_off"
        };
        b.part(&format!("key{note}_led"), "led", led, x + 1.0, white_led_y);
    }
    for (i, note) in black_notes {
        // the part is whole pixels wide, the key centered in it
        let x = black_x_303(i) - (BLACK_KEY_W_303.ceil() - BLACK_KEY_W_303) / 2.0;
        b.part(
            &format!("key{note}"),
            "trigger",
            "key_black_rest",
            x,
            black_y,
        );
        b.part(
            &format!("key{note}_led"),
            "led",
            "led_8_off",
            x + 1.0,
            black_led_y,
        );
    }
    // The flags and note/rest (its LED lit = note) in one row, LEDs like the keys'.
    for (x, (name, led, lit)) in FLAGS_303.into_iter().zip([
        ("down", "down_led", false),
        ("up", "up_led", false),
        ("accent", "accent_led", true),
        ("slide", "slide_led", false),
        ("note_pause", "note_led", true),
    ]) {
        b.part(name, "trigger", "btn_flag_rest", x, white_y);
        b.part(
            led,
            "led",
            if lit { "led_8_on" } else { "led_8_off" },
            x + 1.0,
            white_led_y,
        );
    }
}

/// (x, y, design, id) of the 808's knobs.
const KNOBS_808: [(f32, f32, usize, &str); 22] = [
    (14.0, 9.0, KNOB_808_RED, "ac_level"),
    (36.0, 9.0, KNOB_808_RED, "bd_level"),
    (36.0, 31.0, KNOB_808_WHITE, "bd_tone"),
    (36.0, 54.0, KNOB_808_WHITE, "bd_decay"),
    (58.0, 9.0, KNOB_808_RED, "sd_level"),
    (58.0, 31.0, KNOB_808_WHITE, "sd_tone"),
    (58.0, 54.0, KNOB_808_WHITE, "sd_snappy"),
    (80.0, 9.0, KNOB_808_RED, "lt_level"),
    (80.0, 31.0, KNOB_808_WHITE, "lt_tuning"),
    (102.0, 9.0, KNOB_808_RED, "mt_level"),
    (102.0, 31.0, KNOB_808_WHITE, "mt_tuning"),
    (124.0, 9.0, KNOB_808_RED, "ht_level"),
    (124.0, 31.0, KNOB_808_WHITE, "ht_tuning"),
    (146.0, 9.0, KNOB_808_RED, "rs_level"),
    (168.0, 9.0, KNOB_808_RED, "cp_level"),
    (190.0, 9.0, KNOB_808_RED, "cb_level"),
    (212.0, 9.0, KNOB_808_RED, "cy_level"),
    (212.0, 31.0, KNOB_808_WHITE, "cy_tone"),
    (212.0, 54.0, KNOB_808_WHITE, "cy_decay"),
    (234.0, 9.0, KNOB_808_RED, "oh_level"),
    (234.0, 54.0, KNOB_808_WHITE, "oh_decay"),
    (256.0, 9.0, KNOB_808_RED, "ch_level"),
];

fn tr808(b: &mut Builder) {
    b.section("tr808", "bg_tr808", INSTR_X, 280.0);
    // this fork: the knobs centered in their instrument's column (KNOBS_808 keeps the
    // original's x, 22 px a column from 14: the column is (x - 14) / 22)
    for (i, &(x, y, design, id)) in KNOBS_808.iter().enumerate() {
        let col = ((x - 14.0) / 22.0).round() as usize;
        b.knob(
            id,
            design,
            col808(col) - 6.5,
            y,
            0.35 + 0.03 * (i % 12) as f32,
        );
    }
    // the conga, clave and maraca switches under their name pills (LT..CP's columns)
    for i in 0..5 {
        b.part(
            &format!("slot{}_switch", i + 3),
            "vswitch",
            "switch_808_1",
            col808(i + 3) - 5.5,
            // centered between the pills over and under it
            (UPPER808_Y + PILL808_H + LEGEND808_Y) / 2.0 - 6.5,
        );
    }
    // FLIP in the AC column, in the second knobs' row (this fork; phase 2 turns the panel)
    b.part("flip", "toggle", "flip_off", col808(0) - 8.5, 30.5);
    // each sound's MUTE over its SOLO, right of its LEVEL knob (this fork; cuts.rs places them)
    for k in 1..12 {
        b.part(
            &format!("mute{k}"),
            "sololed",
            "drum808mute_off",
            col808(k) + 8.0,
            8.0,
        );
        b.part(
            &format!("solo{k}"),
            "sololed",
            "drum808solo_off",
            col808(k) + 8.0,
            15.0,
        );
    }
    b.part(
        "select_bd",
        "area",
        "sel_22x14",
        col808(1) - PILL808_W / 2.0,
        LEGEND808_Y,
    );
    let colors = ["red", "orange", "yellow", "white"];
    for i in 0..16 {
        let on = i % 4 == 0;
        b.part(
            &format!("step{}", i + 1),
            "steps",
            &format!(
                "step808_{}_{}",
                colors[i / 4],
                if on { "on" } else { "off" }
            ),
            step808_x(i),
            STEP808_Y,
        );
    }
}

/// The 808's FLIP face (this fork): its own section over the 808's, which the app shows
/// instead of the 808's knobs while FLIP is on: KIT (the kit menu) in the AC column, a PAN knob
/// and four effects lamps in every sound's column (pan`k`, fx`k`_`i`: the panel's instrument
/// index, the effects slot), each lamp with its slot's effect's name (the cuts place them).
fn tr808_flip(b: &mut Builder) {
    b.section("tr808f", "bg_tr808_flip", INSTR_X, 280.0);
    b.part("kit", "kit", "sel_22x14", col808(0) - 11.0, 1.0);
    for c in 1..12 {
        b.knob(&format!("pan{c}"), KNOB_808_RED, col808(c) - 6.5, 9.0, 0.5);
        for i in 0..4 {
            let y = 32.5 + 7.0 * i as f32;
            let id = format!("fx{c}_{i}");
            b.part(&id, "toggle", "flip808lamp_on", col808(c) - 12.0, y);
        }
    }
}

/// The 909's FLIP face (this fork), as the 808's: KIT for the accent's header, a PAN knob and
/// four effects lamps in the middle of every sound's group.
fn tr909_flip(b: &mut Builder) {
    b.section("tr909f", "bg_tr909_flip", INSTR_X, 395.0);
    b.part("kit", "kit", "sel_22x14", 4.0, 2.0);
    let half = KNOBS[KNOB_909].w / 2.0;
    for g in 1..12 {
        let x = column_909(g, 2);
        b.knob(&format!("pan{g}"), KNOB_909, x - half, KNOB_Y_909[0], 0.5);
        for i in 0..4 {
            let y = 41.5 + 7.0 * i as f32;
            let id = format!("fx{g}_{i}");
            b.part(&id, "toggle", "flip909lamp_on", x - 12.0, y);
        }
    }
}

fn tr909(b: &mut Builder) {
    b.section("tr909", "bg_tr909", INSTR_X, 395.0);
    let half = KNOBS[KNOB_909].w / 2.0;
    for (i, (group, col, lower, id, _)) in KNOBS_909.into_iter().enumerate() {
        let (x, y) = (column_909(group, col) - half, knob_y_909(group, lower));
        b.knob(id, KNOB_909, x, y, 0.3 + 0.025 * (i % 16) as f32);
    }
    // FLIP under the LEVEL knob (with the lower knobs), the flam key level with the step
    // keys (as high as they are), both centered in the AC column (4..26)
    b.part("flip", "toggle", "flip909_off", 7.5, KNOB_Y_909[1]);
    b.part("flam_button", "toggle", "toggle_flam_off", 7.5, STEPS_Y_909);
    // each sound's MUTE left of its SOLO, over its LEVEL label (this fork; cuts.rs places them)
    for k in 1..12 {
        let x = column_909(k, 2);
        b.part(
            &format!("mute{k}"),
            "sololed",
            "drum909mute_off",
            x - 9.0,
            15.5,
        );
        b.part(&format!("solo{k}"), "sololed", "drum909solo_off", x, 15.5);
    }
    b.part("select_bd", "area", "sel_42x14", 26.0, NAMES_Y_909);
    for i in 0..16 {
        let value = [1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 3, 0, 2, 0, 0, 0][i];
        let grp = if (i / 4) % 2 == 0 { "a" } else { "b" };
        b.part(
            &format!("step{}", i + 1),
            "steps",
            &format!("step909_{grp}_{value}"),
            // under the name row's cells (26..362)
            26.0 + 21.0 * i as f32,
            STEPS_Y_909,
        );
    }
}

fn mixer(b: &mut Builder, c: usize) {
    b.section(
        &format!("mixer{}", c + 1),
        "bg_mixer",
        456.0,
        50.0 + 115.0 * c as f32,
    );
    // The header (2..20, this fork): the green LED (green while the channel sounds; a click
    // mutes the channel, MUTE), the amber SOLO LED, MIX and the meter, 2 px from its edges.
    b.part("activity", "chanled", "power_14x14_off", 4.0, 4.0);
    b.part("solo", "toggle", "solo_14x14_off", 19.0, 4.0);
    b.part(
        "meter",
        "meter",
        &format!("meter_8x12_{}", [6, 5, 8, 3][c]),
        58.0,
        HEADER_METER_Y,
    );
    // PAN and the fader on the left; on the right (this fork) a small knob per effects slot
    // under its label, then the EQ's HI, MID and LO knobs. cuts.rs puts them
    // where the user's panel has them. The slots hold DELAY, DIST, PCF and COMP: DELAY's knob
    // is the send, DIST's, PCF's and COMP's are on above 0.
    b.pan("pan", KNOB_22, 8.0, 26.0, 0.5);
    b.fader("level", 11.5, 58.0, [0.7, 0.7, 0.8, 0.75][c]);
    let pcf = if c == 1 { 1.0 } else { 0.0 };
    for (k, (id, v)) in [
        ("delay", [0.0, 0.25, 0.1, 0.0][c]),
        ("dist", 0.0),
        ("pcf", pcf),
        ("comp", 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        b.part(
            &format!("{id}_label"),
            "label",
            &format!("mixlbl_{id}"),
            31.0,
            34.0,
        );
        b.knob(id, KNOB_SMALL, 36.0, 26.0 + 22.0 * k as f32, v);
    }
    // the EQ's HI, MID and LO knobs (this fork), 0 dB in the middle
    for (k, band) in ["hi", "mid", "lo"].into_iter().enumerate() {
        b.pan(
            &format!("eq_{band}"),
            KNOB_SMALL,
            54.0,
            37.0 + 22.0 * k as f32,
            0.5,
        );
    }
    // the second effects' knobs (this fork: the delay's send, the others' wet amounts, 0
    // off), shown when two slots hold the effect
    // the chorus's, the flanger's and the phaser's sends (this fork), shown when a slot holds
    // the effect
    for id in ["rvrb", "rvrb_b"] {
        b.part(&format!("{id}_label"), "label", "mixlbl_rvrb", 31.0, 34.0);
        b.knob(id, KNOB_SMALL, 36.0, 26.0, 0.0);
    }
    for id in ["cho", "fla", "pha", "cho_b", "fla_b", "pha_b"] {
        b.part(
            &format!("{id}_label"),
            "label",
            &format!("mixlbl_{}", id.trim_end_matches("_b")),
            31.0,
            34.0,
        );
        b.knob(id, KNOB_SMALL, 36.0, 70.0, 0.0);
    }
    // the C key under LO (this fork): opens the channel's COMP face, lit while its
    // compressor is on
    b.part("c", "ccompkey", "mixc_off", 54.5, 100.0);
    for id in ["delay", "dist", "pcf", "comp"] {
        b.part(
            &format!("{id}_b_label"),
            "label",
            &format!("mixlbl_{id}"),
            31.0,
            34.0,
        );
        b.knob(&format!("{id}_b"), KNOB_SMALL, 36.0, 26.0, 0.0);
    }
}

/// A mixer channel's COMP face (this fork): its own section over the channel's, which the app
/// shows instead of the MIX face while the channel's C key has it open: the gain-reduction
/// LEDs, the ON and OTT lamps, RATIO and THRES, and the back key (the cuts place them; the
/// background `bg_ccomp` is drawn in place of `bg_mixer`).
fn channel_comp(b: &mut Builder, c: usize) {
    b.section_over(&format!("ccomp{}", c + 1), 456.0, 50.0 + 115.0 * c as f32);
    for i in 0..9 {
        b.part(
            &format!("gr{}", i + 1),
            "meter",
            "gr_led_off",
            3.5 + 7.0 * i as f32,
            25.0,
        );
    }
    b.part("on", "toggle", "ccompon_off", 22.0, 47.0);
    b.part("ott", "toggle", "ccompott_off", 38.0, 47.0);
    b.knob("ratio", KNOB_22, 6.0, 66.0, 0.3);
    b.knob("thres", KNOB_22, 42.0, 66.0, 0.6);
    b.part("ratio_label", "ottlabel", "ccomplbl_ratio_off", 5.0, 90.4);
    b.part("thres_label", "ottlabel", "ccomplbl_thres_off", 41.0, 90.4);
    b.part("back", "trigger", "ccompback_rest", 54.5, 100.0);
    // the 303s' channels (this fork): the sidechain, the SC knob and the lamp that picks the
    // BD (off the 808's, on the 909's) with its label (the background `bg_ccomp_sc`)
    if c < 2 {
        b.knob("sc", KNOB_SMALL, 9.5, 92.0, 0.0);
        b.part("sc_src", "toggle", "ccompsrc_off", 30.0, 95.0);
        b.part("sc_label", "sclabel", "ccompsclbl_off", 29.0, 107.0);
    }
}

fn master(b: &mut Builder) {
    b.section("master", "bg_master", 526.0, 50.0);
    b.part("meter_l", "meter", "meter_15x56_8", 16.0, 24.0);
    b.part("meter_r", "meter", "meter_15x56_7", 51.0, 24.0);
    b.fader("fader", 33.0, 25.0, 0.8);
    // this fork: the LIMIT key and its AMT knob in place of the COMP key (cuts.rs puts them
    // where the user's panel has them)
    b.part("limit", "toggle", "mixsw_limit_off", 7.0, 82.0);
    b.knob("limit_amt", KNOB_SMALL, 48.0, 83.0, 0.0);
}

/// An effect's header (2..20): the on/off button and the meter 2 px from its edges.
fn fx_header(b: &mut Builder, on: bool, level: u8) {
    let state = if on { "on" } else { "off" };
    b.part("on", "toggle", &format!("power_14x14_{state}"), 4.0, 4.0);
    // the meter centered on the header's face (2..18.7, above its darker lip)
    b.part(
        "meter",
        "meter",
        &format!("meter_8x12_{level}"),
        70.0,
        HEADER_METER_Y,
    );
}

/// The effects slots under the master (this fork), 90 px each: DELAY (150..240), DIST (..330),
/// PCF (..420) and COMP (..510). Each title opens the effect menu.
fn effects(b: &mut Builder) {
    b.section("delay", "bg_delay", 526.0, 150.0);
    fx_header(b, false, 0);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    b.display("steps", 9.0, 23.0, 19.0, 2, " 3");
    b.part("steps_up", "repeat", "arrow_inc_rest", 29.0, 22.0);
    b.part("steps_down", "repeat", "arrow_dec_rest", 29.0, 31.0);
    b.part("triplet", "vswitch", "switch_fx_0", 49.0, 22.0);
    b.pan("pan", KNOB_FX, 10.0, 55.0, 0.5);
    b.knob("feedback", KNOB_FX, 51.0, 55.0, 0.4);
    // this fork: the TAPE's lamp
    b.part("tape", "toggle", "distlamp_off", 34.5, 50.8);

    // the reverb (this fork) and its second one, shown when a slot holds it, in the delay's
    // slot here (the panel moves each to its slot)
    for sec in ["rvrb", "rvrb_b"] {
        b.section(sec, &format!("bg_{sec}"), 526.0, 150.0);
        fx_header(b, true, 4);
        b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
        b.display("type", 9.0, 23.0, 18.0, 2, "HA");
        b.part("type_up", "repeat", "arrow_inc_rest", 29.0, 22.0);
        b.part("type_down", "repeat", "arrow_dec_rest", 29.0, 31.0);
        b.part("mode", "vswitch", "switch_fx_0", 49.0, 22.0);
        b.knob("size", KNOB_FX, 10.0, 55.0, 0.5);
        b.knob("decay", KNOB_FX, 51.0, 55.0, 0.5);
    }

    // the second delay (this fork), shown when two slots hold the delay
    b.section("delay_b", "bg_delay_b", 526.0, 150.0);
    fx_header(b, false, 0);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    b.display("steps", 9.0, 23.0, 19.0, 2, " 3");
    b.part("steps_up", "repeat", "arrow_inc_rest", 29.0, 22.0);
    b.part("steps_down", "repeat", "arrow_dec_rest", 29.0, 31.0);
    b.part("triplet", "vswitch", "switch_fx_0", 49.0, 22.0);
    b.pan("pan", KNOB_FX, 10.0, 55.0, 0.5);
    b.knob("feedback", KNOB_FX, 51.0, 55.0, 0.4);
    // this fork: the TAPE's lamp
    b.part("tape", "toggle", "distlamp_off", 34.5, 50.8);

    // DIST, PCF and COMP, and (this fork) their second ones where the first ones are (shown
    // when two slots hold the effect; the panel moves each to its slot)
    for sfx in ["", "_b"] {
        dist(b, sfx);
        pcf(b, sfx);
        comp(b, sfx);
    }
    // the chorus, the flanger and the phaser (this fork) and their second ones, shown when a
    // slot holds them, in the PCF's slot here (the panel moves each to its slot)
    for sfx in ["", "_b"] {
        for (sec, _, disp, knobs) in crate::cuts::MOD_FACES {
            modfx(b, &format!("{sec}{sfx}"), disp, knobs);
        }
    }
}

/// A chorus's, flanger's or phaser's face (this fork): as the PCF's.
fn modfx(b: &mut Builder, sec: &str, disp: &str, knobs: [&str; 4]) {
    b.section(sec, &format!("bg_{sec}"), 526.0, 330.0);
    fx_header(b, true, 4);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    b.display(disp, 10.0, 23.0, 18.0, 2, "01");
    b.part(
        &format!("{disp}_up"),
        "repeat",
        "arrow_inc_rest",
        29.0,
        22.0,
    );
    b.part(
        &format!("{disp}_down"),
        "repeat",
        "arrow_dec_rest",
        29.0,
        31.0,
    );
    b.part("mode", "vswitch", "switch_fx_0", 49.0, 22.0);
    for (i, id) in knobs.into_iter().enumerate() {
        b.knob(id, KNOB_SMALL, 4.0 + 19.0 * i as f32, 60.0, 0.5);
    }
}

fn dist(b: &mut Builder, sfx: &str) {
    b.section(
        &format!("dist{sfx}"),
        &format!("bg_dist{sfx}"),
        526.0,
        240.0,
    );
    fx_header(b, false, 0);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    b.knob("amount", KNOB_FX, 10.0, 27.0, 0.4);
    b.knob("shape", KNOB_FX, 51.0, 27.0, 0.5);
    // this fork: the kinds' lamps, two rows of four (the classic one lit)
    for i in 0..8 {
        let part = if i == 0 {
            "distlamp_on"
        } else {
            "distlamp_off"
        };
        let (x, y) = (9.5 + 16.0 * (i % 4) as f32, 22.5 + 15.0 * (i / 4) as f32);
        b.part(&format!("kind{i}"), "radio", part, x, y);
    }
}

fn pcf(b: &mut Builder, sfx: &str) {
    b.section(&format!("pcf{sfx}"), &format!("bg_pcf{sfx}"), 526.0, 330.0);
    fx_header(b, true, 4);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    b.display("pattern", 10.0, 23.0, 18.0, 2, "25");
    b.part("pattern_up", "repeat", "arrow_inc_rest", 29.0, 22.0);
    b.part("pattern_down", "repeat", "arrow_dec_rest", 29.0, 31.0);
    b.part("mode", "vswitch", "switch_fx_1", 49.0, 22.0);
    // this fork: small knobs in place of the sliders
    for (x, id, v) in [
        (4.0, "freq", 0.6),
        (23.0, "q", 0.3),
        (42.0, "amt", 0.5),
        (61.0, "dec", 0.4),
    ] {
        b.knob(id, KNOB_SMALL, x, 60.0, v);
    }
}

fn comp(b: &mut Builder, sfx: &str) {
    b.section(
        &format!("comp{sfx}"),
        &format!("bg_comp{sfx}"),
        526.0,
        420.0,
    );
    fx_header(b, true, 3);
    b.part("menu", "fxmenu", "hit_fx_title", 23.0, 2.0);
    for i in 0..9 {
        // Demo: a little reduction (LEDs left of the middle), the rest dark.
        let state = match i {
            0..=2 => "off",
            3 => "amber",
            4 => "green",
            _ => "off",
        };
        b.part(
            &format!("gr{}", i + 1),
            "meter",
            &format!("gr_led_{state}"),
            // the 63 px meter centered at x 41
            9.5 + 7.0 * i as f32,
            25.0,
        );
    }
    b.knob("ratio", KNOB_FX, 10.0, 54.0, 0.3);
    b.knob("threshold", KNOB_FX, 51.0, 54.0, 0.6);
    // this fork: the OTT's lamp, and the knobs' labels (BAL and GAIN while it is on)
    b.part("ott", "toggle", "distlamp_off", 34.5, 50.8);
    b.part("ratio_label", "ottlabel", "complbl_ratio_off", 9.0, 79.4);
    b.part("thres_label", "ottlabel", "complbl_thres_off", 50.0, 79.4);
}

/// The `layout.tsv` text: one control per line, panel pixel rectangles.
pub fn tsv(l: &Layout) -> String {
    let mut s = String::from("id\tsection\tkind\tpart\tx\ty\tw\th\n");
    for c in &l.ctls {
        s.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            c.id,
            c.section,
            c.kind,
            c.part,
            n(c.x),
            n(c.y),
            n(c.w),
            n(c.h)
        ));
    }
    s
}

/// The whole panel as one SVG (the demo state), 624 x 510 panel pixels.
pub fn compose(parts: &[Part], l: &Layout) -> String {
    let by: HashMap<&str, &Part> = parts.iter().map(|p| (p.name.as_str(), p)).collect();
    let get = |name: &str| {
        *by.get(name)
            .unwrap_or_else(|| panic!("missing part {name}"))
    };
    let mut s = Svg::new();
    s.rect(0.0, 0.0, PANEL_W, PANEL_H, 0.0, pal::BG);
    // the demo has the original's effects, one of each: the second ones' faces and knobs and
    // the chorus's, the flanger's and the phaser's (this fork) are left out
    let second = |id: &str| {
        let modfx = ["rvrb", "cho", "fla", "pha"].iter().any(|fx| {
            id.starts_with(&format!("{fx}."))
                || id.starts_with(&format!("{fx}_b."))
                || id.ends_with(&format!(".{fx}"))
                || id.ends_with(&format!(".{fx}_label"))
                || id == format!("bg_{fx}")
        });
        modfx
            || id.contains("_b.")
            || id.ends_with("_b")
            || id.contains("_b_")
            // the channels' COMP faces (this fork) are closed
            || id.starts_with("ccomp")
    };
    for (name, x, y) in l.backgrounds.iter().filter(|b| !second(&b.0)) {
        s.g_at(*x, *y).raw(&get(name).body).end_g();
    }
    for c in l.ctls.iter().filter(|c| !second(&c.id)) {
        match &c.extra {
            Extra::Plain => {
                s.g_at(c.x, c.y).raw(&get(&c.part).body).end_g();
            }
            Extra::Knob { design, v, bipolar } => {
                let k = &KNOBS[*design];
                s.g_at(c.x, c.y);
                s.raw(&get(&format!("{}_body", k.name)).body);
                let a = -KNOB_SWEEP + v * 2.0 * KNOB_SWEEP;
                if k.ring_r > 0.0 {
                    let a0 = if *bipolar { 0.0 } else { -KNOB_SWEEP };
                    if (a - a0).abs() > 0.5 {
                        s.path_stroke(&arc_path(k.cx, k.cy, k.ring_r, a0, a), k.arc, k.ring_w);
                    }
                }
                s.g_rotate(a, k.cx, k.cy);
                s.raw(&get(&format!("{}_pointer", k.name)).body);
                s.end_g().end_g();
            }
            Extra::Fader { v } => {
                s.g_at(c.x, c.y).raw(&get("fader_track").body).end_g();
                s.g_at(c.x, c.y + (1.0 - v) * 43.0)
                    .raw(&get("fader_handle").body)
                    .end_g();
            }
            Extra::Digits { n: count, text } => {
                let chars: Vec<char> = text.chars().collect();
                let prefix = c.part.split('_').next().unwrap_or("seg");
                for i in 0..*count {
                    let ch = chars.get(i).copied().unwrap_or(' ');
                    let name = if ch == ' ' {
                        format!("{prefix}_blank")
                    } else {
                        format!("{prefix}_{ch}")
                    };
                    let x = c.x + c.w - 9.0 * (*count - i) as f32;
                    s.g_at(x, c.y).raw(&get(&name).body).end_g();
                }
            }
        }
    }
    let (w, h) = (n(PANEL_W), n(PANEL_H));
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\">{}</svg>\n",
        s.s
    )
}
