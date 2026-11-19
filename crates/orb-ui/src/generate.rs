//! The drop-down menu of the bass synths' RNDM buttons (the GENERATE menu): the parameters of
//! the random note generator. A press of the button opens its synth's menu under it and a
//! second press closes it; Escape and a click outside close it too.
//!
//! The menu (this fork's third design, the user's `assets/mockup/randomizer.svg`, 143 x 100) is
//! a black table with rounded corners and gray lines under the first four rows. A wide left
//! column holds the choices, a narrow right column what they are for; the last row, under the
//! lines, holds RESET, UNDO and REDO. Every choice reads in one of three colors: gray while it
//! cannot be used yet, white when it can, green once chosen or done (the text only, on the
//! menu's black), so the menu shows the next step by itself:
//!
//! 1. SCALE | KEY: a press opens the field's list under it (the mouse wheel or the slider scroll
//!    it); a press on an item chooses it. SCALE first, then KEY; BAZE 2 is offered only what fits
//!    BAZE 1 (`scale.rs`). The 303's keys of the chosen scale are lit light cyan.
//! 2. Five buttons, UP (up arrow), DOWN (down arrow), ACCENT (>), SLIDE (a slash) and NOTE (a note;
//!    the symbols are the art's `gen_sym_*` parts, tinted), usable once the scale and key are
//!    chosen; then GENERATE, usable once a button is chosen: it writes the chosen fields of the
//!    synth's pattern anew in the scale (`random.rs`).
//! 3. The same five buttons and ALTER, usable once GENERATE has run: it moves the chosen fields of
//!    the steps to other steps along one random chain (`random::alter`).
//! 4. Five buttons, -2, -1, ?, +1 and +2 (one at a time) and SHIFT, usable once GENERATE has run:
//!    it moves the whole pattern that many steps around in a ring (`?`: a random 1 to 15 steps;
//!    `random::rotate`).
//! 5. RESET at the left, in red: the menu as it starts (the pattern stays); UNDO and REDO at the
//!    right: the pattern before (after) the last GENERATE, ALTER or SHIFT, usable while there is
//!    one.
//!
//! An action's name is green once it ran with the buttons chosen now; a change of its buttons
//! makes it white again.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Key, Modifiers, PointerButton, Pos2, Rect, Sense,
    Stroke, Ui, Vec2,
};

use crate::app::{App, View};
use crate::palette::{INK, INK_DIM, SLATE_DARK};
use crate::random::{self, Rng};
use crate::scale::{self, KEYS, SCALES, ScaleKeys};

/// The actions in the right column of rows 2 to 4, by `Op`.
pub const ACTIONS: [&str; 3] = ["GENERATE", "ALTER", "SHIFT"];
/// The rows: SCALE/KEY, the three actions' rows, and RESET/UNDO/REDO.
pub const ROW_COUNT: usize = 5;
/// The menu's size in panel pixels.
pub const MENU_W: f32 = 143.0;
pub const MENU_H: f32 = 100.0;
/// The top and the height of each row, from the menu's top (the text sits in the middle; the
/// lines are at the bottoms of the first four rows, and the last row is lower down).
pub const ROW_Y: [f32; ROW_COUNT] = [3.5, 20.5, 37.5, 54.5, 76.0];
pub const ROW_H: [f32; ROW_COUNT] = [17.0, 17.0, 17.0, 17.0, 24.0];
/// Where the left column (the scale, the five buttons) and the right one (the key note, the
/// actions) start, from the menu's left, and their widths, the menu's margins of 4 left out.
pub const COL_X: [f32; 2] = [4.0, 88.5];
pub const COL_W: [f32; 2] = [84.5, 50.5];
/// The last row's buttons: RESET, UNDO, REDO, where each starts and its width.
pub const BOTTOM_X: [f32; 3] = [4.0, 71.5, 105.5];
pub const BOTTOM_W: [f32; 3] = [33.0, 34.0, 34.0];
/// The slot (left to right) of each of GENERATE's and ALTER's buttons, by `random::PARAMS`
/// (DOWN, UP, ...): the drawing has UP first.
pub const SLOT: [usize; 5] = [1, 0, 2, 3, 4];
/// The size of the words and of SHIFT's steps, and the space added between letters.
const TEXT_SIZE: f32 = 7.0;
const STEP_SIZE: f32 = 8.0;
const TRACKING: f32 = 0.35;
/// How far apart the two drawings of a word are (they make it bold).
const BOLD: f32 = 0.3;
/// The last row's buttons.
pub const RESET: &str = "RESET";
pub const UNDO: &str = "UNDO";
pub const REDO: &str = "REDO";
/// The widths of the left column (the scale, the five buttons) and the right one (the key
/// note, the actions) in panel pixels, and of the last row's three buttons.
/// The menu's corner radius in panel pixels.
const CORNER: f32 = 4.5;
/// The undo steps kept per synth.
const UNDO_DEPTH: usize = 64;
/// The height of a list's item in panel pixels.
pub const CELL_H: f32 = 14.0;
/// How far short of the menu's edges the lines stop, in panel pixels.
const LINE_INSET: f32 = 4.0;
/// The space between the button's bottom edge and the menu, in panel pixels.
const GAP: f32 = 2.0;
/// The items a list shows at once, and the width of its slider (panel pixels).
pub const LIST_ROWS: usize = 5;
const SLIDER_W: f32 = 5.0;
/// RESET's red (the drawing's).
pub const RED: Color32 = Color32::from_rgb(0xe2, 0x41, 0x3b);
/// A chosen or done choice (the drawing's green).
pub const GREEN: Color32 = Color32::from_rgb(0x4a, 0xcf, 0x62);
/// The lines between the rows and the columns.
const LINE: Color32 = Color32::from_gray(0x5a);
const FILL: Color32 = Color32::BLACK;
/// A field that cannot be used yet.
const OFF: Color32 = Color32::from_gray(0x55);
const LIST_FILL: Color32 = Color32::from_gray(0x14);

/// The rows of five buttons and their actions: GENERATE makes new values, ALTER moves the values
/// there are to other steps, SHIFT (`Move`) moves the whole pattern (one button at a time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Random = 0,
    Alter = 1,
    Move = 2,
}

impl Op {
    pub const ALL: [Op; 3] = [Op::Random, Op::Alter, Op::Move];

    /// The menu row (0 is SCALE/KEY).
    pub fn row(self) -> usize {
        1 + self as usize
    }

    /// The action's name in the right column.
    pub fn name(self) -> &'static str {
        ACTIONS[self as usize]
    }
}

/// Row 1's two fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Scale,
    Key,
}

/// An open list: whose field, how far it is scrolled (in items) and, while the slider's thumb
/// is dragged, where on the thumb it was grabbed (screen points from its top).
#[derive(Clone, Copy, Debug)]
pub struct List {
    pub synth: usize,
    pub field: Field,
    pub scroll: f32,
    grab: Option<f32>,
}

/// The synth (0 for the upper, 1 for the lower) whose GENERATE button `id` is, if it is one.
pub fn synth_of(id: &str) -> Option<usize> {
    match id {
        "tb303_1.generate" => Some(0),
        "tb303_2.generate" => Some(1),
        _ => None,
    }
}

/// Whether the control `id` is one of the 303's keys (`tb303_N.keyM`, not its LED) whose note is
/// in its synth's chosen scale.
pub fn glows(sk: &ScaleKeys, id: &str) -> bool {
    let Some((sec, name)) = id.split_once('.') else {
        return false;
    };
    let synth = match sec {
        "tb303_1" => 0,
        "tb303_2" => 1,
        _ => return false,
    };
    let Some(n) = name
        .strip_prefix("key")
        .and_then(|n| n.parse::<usize>().ok())
    else {
        return false;
    };
    sk.get(synth).mask().is_some_and(|m| scale::key_in(m, n))
}

/// The items of a list: the indexes into `SCALES` or `KEYS`.
fn options(sk: &ScaleKeys, synth: usize, field: Field) -> Vec<usize> {
    match field {
        Field::Scale => sk.scale_options(synth),
        Field::Key => sk.key_options(synth),
    }
}

fn item_name(field: Field, i: usize) -> &'static str {
    match field {
        Field::Scale => SCALES[i].0,
        Field::Key => KEYS[i],
    }
}

impl App {
    /// A press of an RNDM button: opens its synth's menu, or closes it when it is open.
    pub fn toggle_generate(&mut self, synth: usize) {
        self.generate_open = if self.generate_open == Some(synth) {
            None
        } else {
            Some(synth)
        };
        self.generate_list = None;
    }

    /// Whether row `row` (0 is SCALE/KEY) of a synth's menu is done: the scale and key note
    /// chosen, or the row's action run with the buttons chosen now (its name green).
    pub fn row_done(&self, synth: usize, row: usize) -> bool {
        match row {
            0 => self.scale_keys.get(synth).mask().is_some(),
            _ => Op::ALL
                .into_iter()
                .any(|op| op.row() == row && self.op_done[op as usize][synth]),
        }
    }

    // ---- the GENERATE, ALTER and SHIFT rows -----------------------------------------------

    /// A synth's chosen buttons of a row, indexed by `random::PARAMS` (SHIFT: `random::MOVES`).
    pub fn op_selection(&self, synth: usize, op: Op) -> [bool; 5] {
        self.op_sel[op as usize][synth]
    }

    /// Whether GENERATE has run on the synth since its menu started (or was reset): ALTER and
    /// SHIFT wait for it.
    pub fn generated(&self, synth: usize) -> bool {
        self.generated[synth]
    }

    /// RESET: the synth's menu back to how it starts: no scale or key note (BAZE 2's goes too
    /// when it is BAZE 1's), no button chosen, no action done, ALTER and SHIFT waiting for
    /// GENERATE again. The pattern and the UNDO steps stay.
    pub fn reset_generate(&mut self, synth: usize) {
        self.scale_keys.clear(synth);
        for op in Op::ALL {
            self.op_sel[op as usize][synth] = [false; 5];
            self.op_done[op as usize][synth] = false;
        }
        self.generated[synth] = false;
        if synth == 0 && self.scale_keys.get(1).mask().is_none() {
            // BAZE 2 lost its scale with BAZE 1's: its GENERATE row waits too
            self.op_done[Op::Random as usize][1] = false;
        }
        self.generate_list = None;
    }

    /// The panel rectangle of RESET: the last row's left button.
    pub fn reset_rect(&self, synth: usize) -> Option<(f32, f32, f32, f32)> {
        self.bottom_rect(synth, 0)
    }

    /// The panel rectangle of the last row's button `i` (RESET, UNDO, REDO).
    fn bottom_rect(&self, synth: usize, i: usize) -> Option<(f32, f32, f32, f32)> {
        let (x, y, ..) = self.generate_rect(synth)?;
        let last = ROW_COUNT - 1;
        Some((x + BOTTOM_X[i], y + ROW_Y[last], BOTTOM_W[i], ROW_H[last]))
    }

    /// The panel rectangle of UNDO (`redo` false) or REDO: the last row's right buttons.
    pub fn undo_rect(&self, synth: usize, redo: bool) -> Option<(f32, f32, f32, f32)> {
        self.bottom_rect(synth, if redo { 2 } else { 1 })
    }

    /// Restarts the random numbers from `seed` (tests).
    pub fn set_random_seed(&mut self, seed: u64) {
        self.rng = Rng::new(seed);
    }

    /// Whether a row's buttons can be chosen: GENERATE's once the synth's scale and key note
    /// are (it makes notes of the scale), ALTER's and SHIFT's once GENERATE has run.
    pub fn op_usable(&self, synth: usize, op: Op) -> bool {
        match op {
            Op::Random => self.scale_keys.get(synth).mask().is_some(),
            _ => self.generated[synth],
        }
    }

    /// Whether a row's action can be pressed: its buttons usable and one of them chosen.
    pub fn action_usable(&self, synth: usize, op: Op) -> bool {
        self.op_usable(synth, op) && self.op_sel[op as usize][synth].iter().any(|&o| o)
    }

    /// A press on GENERATE, ALTER or SHIFT: runs the row on the synth's pattern with its chosen
    /// buttons (an UNDO step), and the action's name turns green. Nothing while it is gray.
    pub fn press_op(&mut self, synth: usize, op: Op) {
        self.generate_list = None;
        if !self.action_usable(synth, op) {
            return;
        }
        let on = self.op_sel[op as usize][synth];
        let before = self.panel.pattern_303(synth);
        let mut steps = before;
        match op {
            Op::Random => {
                let mask = self.scale_keys.get(synth).mask();
                random::randomize(&mut steps, on, mask, &mut self.rng);
                self.generated[synth] = true;
            }
            Op::Alter => random::alter(&mut steps, on, &mut self.rng),
            Op::Move => {
                let choice = on.iter().position(|&o| o).unwrap_or(random::MOVE_RANDOM);
                random::rotate(&mut steps, random::move_offset(choice, &mut self.rng));
            }
        }
        self.op_done[op as usize][synth] = true;
        if steps != before {
            let pattern = self.panel.rows[synth].pattern;
            let undo = &mut self.gen_undo[synth];
            undo.push((pattern, before));
            if undo.len() > UNDO_DEPTH {
                undo.remove(0);
            }
            self.gen_redo[synth].clear();
            self.panel.set_pattern_303(synth, steps);
        }
    }

    /// A press on one of a row's five buttons, while the row is usable: turns it on or off
    /// (SHIFT's one at a time); the row's action is then not done with the new choice.
    pub fn press_op_param(&mut self, synth: usize, op: Op, param: usize) {
        self.generate_list = None;
        if !self.op_usable(synth, op) {
            return;
        }
        let sel = &mut self.op_sel[op as usize][synth];
        let on = !sel[param];
        if op == Op::Move {
            *sel = [false; 5];
        }
        sel[param] = on;
        self.op_done[op as usize][synth] = false;
    }

    /// Whether UNDO (`redo` false) or REDO has a step for the synth.
    pub fn can_undo(&self, synth: usize, redo: bool) -> bool {
        if redo {
            !self.gen_redo[synth].is_empty()
        } else {
            !self.gen_undo[synth].is_empty()
        }
    }

    /// UNDO (`redo` false): the pattern before the synth's last GENERATE, ALTER or SHIFT, put
    /// back into the pattern it changed; REDO: the change again. Nothing without a step.
    pub fn undo_generate(&mut self, synth: usize, redo: bool) {
        self.generate_list = None;
        let (from, to) = if redo {
            (&mut self.gen_redo[synth], &mut self.gen_undo[synth])
        } else {
            (&mut self.gen_undo[synth], &mut self.gen_redo[synth])
        };
        let Some((pattern, steps)) = from.pop() else {
            return;
        };
        to.push((pattern, self.panel.synth[synth].patterns[pattern as usize]));
        self.panel.set_pattern_303_at(synth, pattern, steps);
    }

    /// The panel rectangle of a row's action (the right column).
    pub fn op_label_rect(&self, synth: usize, op: Op) -> Option<(f32, f32, f32, f32)> {
        let (x, y, ..) = self.generate_rect(synth)?;
        let r = op.row();
        Some((x + COL_X[1], y + ROW_Y[r], COL_W[1], ROW_H[r]))
    }

    /// The panel rectangle of a row's button `param` (the left column split in five; GENERATE's
    /// and ALTER's in the order of `SLOT`).
    pub fn op_button_rect(
        &self,
        synth: usize,
        op: Op,
        param: usize,
    ) -> Option<(f32, f32, f32, f32)> {
        let (x, y, ..) = self.generate_rect(synth)?;
        let w = COL_W[0] / random::PARAMS.len() as f32;
        let slot = if op == Op::Move { param } else { SLOT[param] };
        let r = op.row();
        Some((x + COL_X[0] + w * slot as f32, y + ROW_Y[r], w, ROW_H[r]))
    }

    /// The synth whose menu is open, if any.
    pub fn generate_open(&self) -> Option<usize> {
        self.generate_open
    }

    /// The open list's synth and field, if any.
    pub fn generate_list(&self) -> Option<(usize, Field)> {
        self.generate_list.map(|l| (l.synth, l.field))
    }

    /// The scale and key note of both synths.
    pub fn scale_keys(&self) -> &ScaleKeys {
        &self.scale_keys
    }

    /// The panel rectangle (x, y, w, h) of a synth's GENERATE button.
    fn generate_button(&self, synth: usize) -> Option<(f32, f32, f32, f32)> {
        let id = format!("tb303_{}.generate", synth + 1);
        let b = self.ctls.iter().find(|c| c.id == id)?;
        Some((b.x, b.y, b.w, b.h))
    }

    /// The menu's panel rectangle (x, y, w, h): under the button, the right edges aligned.
    pub fn generate_rect(&self, synth: usize) -> Option<(f32, f32, f32, f32)> {
        let (bx, by, bw, bh) = self.generate_button(synth)?;
        Some((bx + bw - MENU_W, by + bh + GAP, MENU_W, MENU_H))
    }

    /// The panel rectangle of row 1's scale field (the left column) or key field (the right).
    pub fn field_rect(&self, synth: usize, field: Field) -> Option<(f32, f32, f32, f32)> {
        let (x, y, ..) = self.generate_rect(synth)?;
        Some(match field {
            Field::Scale => (x + COL_X[0], y + ROW_Y[0], COL_W[0], ROW_H[0]),
            Field::Key => (x + COL_X[1], y + ROW_Y[0], COL_W[1], ROW_H[0]),
        })
    }

    /// The panel rectangle of a field's list, under the field; None when it has no items.
    pub fn list_rect(&self, synth: usize, field: Field) -> Option<(f32, f32, f32, f32)> {
        let n = options(&self.scale_keys, synth, field).len();
        if n == 0 {
            return None;
        }
        let (x, y, w, h) = self.field_rect(synth, field)?;
        Some((x, y + h, w, CELL_H * n.min(LIST_ROWS) as f32))
    }

    /// A press on a field: opens its list (scrolled to the chosen item), or closes it when it
    /// is open. A field with nothing to offer (BAZE 2 before BAZE 1, a key before a scale) does
    /// nothing.
    pub fn press_field(&mut self, synth: usize, field: Field) {
        if self
            .generate_list
            .is_some_and(|l| l.synth == synth && l.field == field)
        {
            self.generate_list = None;
            return;
        }
        let items = options(&self.scale_keys, synth, field);
        if items.is_empty() {
            self.generate_list = None;
            return;
        }
        let chosen = match field {
            Field::Scale => self.scale_keys.get(synth).scale,
            Field::Key => self.scale_keys.get(synth).key,
        };
        let at = chosen.and_then(|c| items.iter().position(|&i| i == c));
        let max = items.len().saturating_sub(LIST_ROWS) as f32;
        let scroll = at.map_or(0.0, |a| (a as f32 - 2.0).clamp(0.0, max));
        self.generate_list = Some(List {
            synth,
            field,
            scroll,
            grab: None,
        });
    }

    /// Chooses the open list's item at `row` (0 is the list's first item) and closes the list.
    pub fn choose_item(&mut self, row: usize) {
        let Some(l) = self.generate_list else {
            return;
        };
        let items = options(&self.scale_keys, l.synth, l.field);
        if let Some(&i) = items.get(row) {
            match l.field {
                Field::Scale => self.scale_keys.set_scale(l.synth, i),
                Field::Key => self.scale_keys.set_key(l.synth, i),
            };
        }
        self.generate_list = None;
    }

    /// Draws the open menu (and its open list) above the panel, after the panel's controls (so
    /// a press of the button has already opened or closed it this frame), and takes its input.
    pub(crate) fn generate_ui(&mut self, ui: &Ui, view: &View) {
        let Some(k) = self.generate_open else {
            return;
        };
        // a change of the menu's settings is a change of the song (an .rbsx keeps them)
        let before = self.gen_state();
        self.generate_menu(view, k, ui.ctx().clone());
        if self.gen_state() != before {
            self.doc.modified = true;
        }
    }

    /// [`App::generate_ui`] for the open menu of synth `k`.
    fn generate_menu(&mut self, view: &View, k: usize, ctx: egui::Context) {
        let (Some((x, y, w, h)), Some((bx, by, bw, bh))) =
            (self.generate_rect(k), self.generate_button(k))
        else {
            return;
        };
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.generate_list.is_some() {
                self.generate_list = None;
            } else {
                self.generate_open = None;
            }
            return;
        }
        let menu = view.rect(x, y, w, h);
        let button = view.rect(bx, by, bw, bh);
        let screen = |r: Option<(f32, f32, f32, f32)>| r.map(|(x, y, w, h)| view.rect(x, y, w, h));
        let scale_field = screen(self.field_rect(k, Field::Scale)).unwrap_or(Rect::NOTHING);
        let key_field = screen(self.field_rect(k, Field::Key)).unwrap_or(Rect::NOTHING);
        let reset = screen(self.reset_rect(k)).unwrap_or(Rect::NOTHING);
        let undo = screen(self.undo_rect(k, false)).unwrap_or(Rect::NOTHING);
        let redo = screen(self.undo_rect(k, true)).unwrap_or(Rect::NOTHING);
        let list_rect = self
            .generate_list
            .and_then(|l| screen(self.list_rect(l.synth, l.field)));
        let item_h = CELL_H * view.scale;
        let slider_w = SLIDER_W * view.scale;
        // GENERATE's, ALTER's and SHIFT's actions and buttons, indexed by `Op`
        let op_labels: [Rect; 3] =
            Op::ALL.map(|op| screen(self.op_label_rect(k, op)).unwrap_or(Rect::NOTHING));
        let op_buttons: [[Rect; 5]; 3] = Op::ALL.map(|op| {
            std::array::from_fn(|i| screen(self.op_button_rect(k, op, i)).unwrap_or(Rect::NOTHING))
        });

        let (pressed, down, hover, wheel) = ctx.input(|i| {
            (
                i.pointer
                    .button_pressed(PointerButton::Primary)
                    .then(|| i.pointer.press_origin())
                    .flatten(),
                i.pointer.primary_down(),
                i.pointer.hover_pos(),
                i.smooth_scroll_delta.y,
            )
        });

        if let Some(p) = pressed {
            let label = Op::ALL
                .into_iter()
                .find(|&op| op_labels[op as usize].contains(p));
            let param = Op::ALL.into_iter().find_map(|op| {
                op_buttons[op as usize]
                    .iter()
                    .position(|b| b.contains(p))
                    .map(|i| (op, i))
            });
            match list_rect {
                Some(lr) if lr.contains(p) => self.press_list(lr, p, item_h, slider_w),
                _ if scale_field.contains(p) => self.press_field(k, Field::Scale),
                _ if key_field.contains(p) => self.press_field(k, Field::Key),
                _ if reset.contains(p) => self.reset_generate(k),
                _ if undo.contains(p) => self.undo_generate(k, false),
                _ if redo.contains(p) => self.undo_generate(k, true),
                _ if label.is_some() => self.press_op(k, label.unwrap_or(Op::Random)),
                _ if param.is_some() => {
                    let (op, i) = param.unwrap_or((Op::Random, 0));
                    self.press_op_param(k, op, i);
                }
                _ => {
                    self.generate_list = None;
                    if !menu.contains(p) && !button.contains(p) {
                        self.generate_open = None;
                        return;
                    }
                }
            }
        }
        if let (Some(l), Some(lr)) = (&mut self.generate_list, list_rect) {
            let n = options(&self.scale_keys, l.synth, l.field).len();
            let max = n.saturating_sub(LIST_ROWS) as f32;
            if let Some(grab) = l.grab {
                match (down, hover) {
                    (true, Some(p)) => {
                        let (track, thumb_h) = slider_track(lr, n, slider_w);
                        let room = (track.height() - thumb_h).max(1.0);
                        l.scroll = ((p.y - grab - track.min.y) / room * max).clamp(0.0, max);
                    }
                    _ => l.grab = None,
                }
            }
            if wheel != 0.0 && hover.is_some_and(|p| lr.contains(p)) {
                l.scroll = (l.scroll - wheel / item_h).clamp(0.0, max);
            }
        }

        let sk = self.scale_keys;
        // gray: not usable yet; white: usable; green: chosen or done
        let ink = |usable: bool| if usable { INK } else { OFF };
        let actions: [(Rect, &str, Color32, bool); 3] = Op::ALL.map(|op| {
            let done = self.op_done[op as usize][k] && self.op_usable(k, op);
            let color = if done {
                GREEN
            } else {
                ink(self.action_usable(k, op))
            };
            (op_labels[op as usize], op.name(), color, done)
        });
        let buttons: [[(Rect, bool, Color32); 5]; 3] = Op::ALL.map(|op| {
            let usable = self.op_usable(k, op);
            std::array::from_fn(|i| {
                let on = self.op_sel[op as usize][k][i] && usable;
                let color = if on { GREEN } else { ink(usable) };
                (op_buttons[op as usize][i], on, color)
            })
        });
        let bottom = [
            (reset, RESET, RED, false),
            (undo, UNDO, ink(self.can_undo(k, false)), false),
            (redo, REDO, ink(self.can_undo(k, true)), false),
        ];
        egui::Area::new(egui::Id::unique(("orb_generate_menu", k)))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.min)
            .constrain(false)
            // solid from the first frame, like the panel's own parts
            .fade_in(false)
            .show(&ctx, |ui| {
                let (r, _) = ui.allocate_exact_size(menu.size(), Sense::click());
                paint_table(ui, r, view.scale);
                paint_fields(ui, &sk, k, scale_field, key_field, view.scale);
                for op in Op::ALL {
                    paint_op_buttons(ui, op, &buttons[op as usize], view.scale);
                }
                for cell in actions.iter().chain(bottom.iter()) {
                    paint_text_cell(ui, *cell, view.scale);
                }
            });
        if let (Some(l), Some(lr)) = (self.generate_list, list_rect) {
            egui::Area::new(egui::Id::unique(("orb_generate_list", k)))
                .order(egui::Order::Tooltip)
                .fixed_pos(lr.min)
                .constrain(false)
                .fade_in(false)
                .show(&ctx, |ui| {
                    let (r, _) = ui.allocate_exact_size(lr.size(), Sense::click());
                    paint_list(ui, &sk, &l, r, hover, view.scale);
                });
        }
    }

    /// A press inside the open list: on the slider it grabs the thumb (or jumps there), on an
    /// item it chooses the item.
    fn press_list(&mut self, lr: Rect, p: Pos2, item_h: f32, slider_w: f32) {
        let Some(l) = &mut self.generate_list else {
            return;
        };
        let n = options(&self.scale_keys, l.synth, l.field).len();
        if n > LIST_ROWS && p.x >= lr.max.x - slider_w {
            let (track, thumb_h) = slider_track(lr, n, slider_w);
            let max = (n - LIST_ROWS) as f32;
            let thumb_y = track.min.y + (track.height() - thumb_h) * l.scroll / max;
            let grab = if (thumb_y..thumb_y + thumb_h).contains(&p.y) {
                p.y - thumb_y
            } else {
                // a press on the track moves the thumb's middle there
                thumb_h / 2.0
            };
            let room = (track.height() - thumb_h).max(1.0);
            l.scroll = ((p.y - grab - track.min.y) / room * max).clamp(0.0, max);
            l.grab = Some(grab);
            return;
        }
        let row = ((p.y - lr.min.y) / item_h + l.scroll).floor().max(0.0) as usize;
        self.choose_item(row);
    }
}

/// The slider's track (the list's right edge) and the height of its thumb.
fn slider_track(lr: Rect, n: usize, slider_w: f32) -> (Rect, f32) {
    let track = Rect::from_min_max(Pos2::new(lr.max.x - slider_w, lr.min.y), lr.max);
    let thumb_h = track.height() * LIST_ROWS as f32 / n.max(1) as f32;
    (track, thumb_h)
}

/// The table: the black fill with rounded corners and the gray lines (stopping short of the
/// edges): under each of the first four rows, and the columns' line down to the last of them.
fn paint_table(ui: &Ui, r: Rect, scale: f32) {
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same((CORNER * scale).round() as u8), FILL);
    let stroke = Stroke::new(scale.max(1.0), LINE);
    let at = |x: f32, y: f32| Pos2::new(r.min.x + x * scale, r.min.y + y * scale);
    let last = ROW_Y[ROW_COUNT - 2] + ROW_H[ROW_COUNT - 2];
    for j in 0..ROW_COUNT - 1 {
        let y = ROW_Y[j] + ROW_H[j];
        p.line_segment([at(LINE_INSET, y), at(MENU_W - LINE_INSET, y)], stroke);
    }
    p.line_segment([at(COL_X[1], LINE_INSET), at(COL_X[1], last)], stroke);
}

/// `text` in `color`, its letters spaced as the drawing's, centered on `c`.
fn paint_text(ui: &Ui, c: Pos2, text: &str, size: f32, color: Color32, scale: f32) {
    let mut job = egui::text::LayoutJob::default();
    let tracking = TRACKING * scale;
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(size * scale),
            color,
            extra_letter_spacing: tracking,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);
    // the last letter's spacing is not part of the word
    let size = galley.size() - Vec2::new(tracking, 0.0);
    // bold as the drawing's: drawn twice, a hair apart
    let bold = BOLD * scale;
    let at = c - (size + Vec2::new(bold, 0.0)) / 2.0;
    ui.painter().galley(at, galley.clone(), color);
    ui.painter()
        .galley(at + Vec2::new(bold, 0.0), galley, color);
}

/// A cell with a word: an action, RESET, UNDO or REDO, in its color (green when done) on the
/// black of the menu.
fn paint_text_cell(ui: &Ui, (r, text, color, _): (Rect, &str, Color32, bool), scale: f32) {
    paint_text(ui, r.center(), text, TEXT_SIZE, color, scale);
}

fn font(scale: f32) -> FontId {
    FontId::proportional(TEXT_SIZE * scale)
}

/// Row 1's scale and key note: the choice in green, else the field's name, white
/// when it can be chosen and gray while it cannot (the key before the scale, BAZE 2 before
/// BAZE 1).
fn paint_fields(ui: &Ui, sk: &ScaleKeys, synth: usize, sf: Rect, kf: Rect, scale: f32) {
    let c = sk.get(synth);
    let field = |r: Rect, chosen: Option<&'static str>, name: &'static str, usable: bool| {
        let (t, color) = match chosen {
            Some(t) => (t, GREEN),
            None => (name, if usable { INK } else { OFF }),
        };
        paint_text(ui, r.center(), t, TEXT_SIZE, color, scale);
    };
    field(
        sf,
        c.scale.map(|s| SCALES[s].0),
        "SCALE",
        !sk.scale_options(synth).is_empty(),
    );
    field(
        kf,
        c.key.map(|k| KEYS[k]),
        "KEY",
        !sk.key_options(synth).is_empty(),
    );
}

/// An open list: its items (the chosen one green, the one under the pointer highlighted) and,
/// when it has more items than it shows, the slider at its right edge.
fn paint_list(ui: &Ui, sk: &ScaleKeys, l: &List, r: Rect, hover: Option<Pos2>, scale: f32) {
    let items = options(sk, l.synth, l.field);
    let chosen = match l.field {
        Field::Scale => sk.get(l.synth).scale,
        Field::Key => sk.get(l.synth).key,
    };
    let item_h = CELL_H * scale;
    let slider_w = SLIDER_W * scale;
    let has_slider = items.len() > LIST_ROWS;
    let text_w = r.width() - if has_slider { slider_w } else { 0.0 };
    let p = ui.painter().with_clip_rect(r);
    p.rect_filled(r, 0.0, LIST_FILL);
    for (row, &i) in items.iter().enumerate() {
        let top = r.min.y + (row as f32 - l.scroll) * item_h;
        let item = Rect::from_min_size(Pos2::new(r.min.x, top), Vec2::new(text_w, item_h));
        if !item.intersects(r) {
            continue;
        }
        if l.grab.is_none() && hover.is_some_and(|h| item.contains(h) && r.contains(h)) {
            p.rect_filled(item, 0.0, SLATE_DARK);
        }
        let color = if chosen == Some(i) { GREEN } else { INK };
        p.text(
            item.center(),
            Align2::CENTER_CENTER,
            item_name(l.field, i),
            font(scale),
            color,
        );
    }
    if has_slider {
        let (track, thumb_h) = slider_track(r, items.len(), slider_w);
        let max = (items.len() - LIST_ROWS) as f32;
        let thumb_y = track.min.y + (track.height() - thumb_h) * l.scroll / max;
        p.rect_filled(track, 0.0, Color32::from_gray(0x2a));
        let thumb = Rect::from_min_size(
            Pos2::new(track.min.x + scale * 0.5, thumb_y),
            Vec2::new(track.width() - scale, thumb_h),
        );
        p.rect_filled(thumb, CornerRadius::same((1.5 * scale) as u8), INK_DIM);
    }
    p.rect_stroke(
        r,
        0.0,
        Stroke::new((0.6 * scale).max(1.0), LINE),
        egui::StrokeKind::Inside,
    );
}

/// A row's five buttons (GENERATE, ALTER, SHIFT): (rect, on, ink of the symbol). A chosen button
/// is green (the symbol only). GENERATE's and ALTER's symbols are the art's `gen_sym_*` parts
/// (white, tinted with the ink), 17 x 17 around the button's middle; SHIFT's steps are text.
fn paint_op_buttons(ui: &Ui, op: Op, buttons: &[(Rect, bool, Color32); 5], scale: f32) {
    for (param, &(r, _, ink)) in buttons.iter().enumerate() {
        if op == Op::Move {
            paint_text(ui, r.center(), random::MOVES[param], STEP_SIZE, ink, scale);
        } else {
            let name = SYMBOLS[SLOT[param]];
            let at = Rect::from_center_size(r.center(), Vec2::splat(17.0 * scale));
            crate::app::paint_part_tinted(ui, name, at, ink);
        }
    }
}

/// The parts of GENERATE's and ALTER's symbols, by slot (orb-ui-art's `GENERATE_SYMBOLS`).
pub const SYMBOLS: [&str; 5] = [
    "gen_sym_up",
    "gen_sym_down",
    "gen_sym_accent",
    "gen_sym_slide",
    "gen_sym_note",
];
