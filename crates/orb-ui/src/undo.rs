//! This fork: the Edit menu's Undo and Redo (Cmd+Z, Shift+Cmd+Z; Ctrl elsewhere) of every edit
//! of the song: the controls, the patterns, the effects' slots, SOLO and MUTE, the LFOs'
//! buttons. Not the transport, the pattern selection, the views, the song mode's recorded
//! changes, or opening a song, a mod or a kit (opening a song or a mod forgets the history).
//!
//! A frame with input keeps the state from before it ([`State`]); at the frame's end a state
//! that differs is one step of the history, but a knob's or a fader's drag (or its turns of the
//! wheel, its arrow keys) is one step however long it takes. Undo and Redo send the engine
//! only what differs, as the controls' own changes would, so a song that plays goes on.

use eframe::egui::{self, Ui};

use crate::app::App;
use crate::lfo::Mode;
use crate::model::{Edits, Event};

/// How many steps the history keeps.
pub const LIMIT: usize = 100;
/// How long after its last change a knob's or a fader's next change is still the same step
/// (seconds), when it is not held.
pub const SAME_STEP: f64 = 1.0;

/// What an undo brings back.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    panel: Edits,
    /// The link's pattern arrays (every pattern of the four rows); empty without a player.
    patterns: [Vec<u8>; 4],
    lfo_mode: [[Mode; 5]; 2],
}

/// The undo and redo steps.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<State>,
    redo: Vec<State>,
    /// The state at the start of a frame with input, compared at its end.
    before: Option<State>,
    /// Whether the panel made an edit this frame ([`App::note_edits`]): a state that changed
    /// without one is the engine's (a song's recorded changes), no step.
    pub(crate) edited: bool,
    /// The knob or fader of the last step and when it last moved.
    group: Option<(String, f64)>,
}

impl History {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// How many steps there are to undo.
    pub fn len(&self) -> usize {
        self.undo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    /// Forgets every step (a song or a mod opened); the frame's state too.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn push(&mut self, s: State) {
        self.undo.push(s);
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
}

/// Whether a frame's input may edit: a click, a key, the wheel, a held button.
fn has_input(i: &egui::InputState) -> bool {
    i.pointer.any_down()
        || i.events.iter().any(|e| {
            matches!(
                e,
                egui::Event::PointerButton { .. }
                    | egui::Event::Key { .. }
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Copy
                    | egui::Event::Cut
            )
        })
}

impl App {
    /// The state now; the panel's pending edits go to the pattern arrays first.
    fn undo_state(&mut self) -> State {
        self.note_edits();
        if let Some(link) = &mut self.link {
            link.forward(&mut self.panel);
        }
        State {
            panel: self.panel.edits(),
            patterns: self
                .link
                .as_ref()
                .map(|l| l.patterns().clone())
                .unwrap_or_default(),
            lfo_mode: self.lfo_mode,
        }
    }

    /// The start of a frame: the state before its input, if it has any.
    pub(crate) fn undo_frame_start(&mut self, ui: &Ui) {
        self.undo.edited = false;
        self.undo.before = None;
        if ui.input(has_input) {
            self.undo.before = Some(self.undo_state());
        }
    }

    /// The end of a frame: the state before it is a step of the history when the frame
    /// changed it (a knob's or a fader's drag: the step its first change made).
    pub(crate) fn undo_frame_end(&mut self, now: f64) {
        let Some(before) = self.undo.before.take() else {
            return;
        };
        let after = self.undo_state();
        let lfo = after.lfo_mode != before.lfo_mode;
        if after == before || !(self.undo.edited || lfo) {
            return;
        }
        let one = after
            .panel
            .one_control_from(&before.panel)
            .filter(|_| after.patterns == before.patterns && !lfo);
        let same = match (&one, &self.undo.group) {
            (Some(id), Some((g, t))) => {
                id == g && (self.panel.held.as_deref() == Some(id) || now - t < SAME_STEP)
            }
            _ => false,
        };
        if !same {
            self.undo.push(before);
        }
        self.undo.group = one.map(|id| (id, now));
    }

    /// Edit > Undo: the state before the last step.
    pub fn undo(&mut self) {
        self.undo_redo(false);
    }

    /// Edit > Redo: the state the last undo left.
    pub fn redo(&mut self) {
        self.undo_redo(true);
    }

    fn undo_redo(&mut self, redo: bool) {
        let from = if redo {
            &mut self.undo.redo
        } else {
            &mut self.undo.undo
        };
        let Some(to) = from.pop() else {
            return;
        };
        let now = self.undo_state();
        if redo {
            self.undo.undo.push(now);
        } else {
            self.undo.redo.push(now);
        }
        self.restore_state(&to);
        self.undo.group = None;
        self.doc.modified = true;
        // the frame's own input is not a step: its end compares with the state brought back
        if self.undo.before.is_some() {
            self.undo.before = Some(self.undo_state());
        }
    }

    /// Brings `s` back, sending the engine what differs.
    fn restore_state(&mut self, s: &State) {
        self.lfo_mode = s.lfo_mode;
        self.panel.restore(&s.panel);
        // an LFO that runs moves around the knob's value brought back
        for c in crate::layout::controls() {
            if c.kind == "knob" && c.section.starts_with("tb303_") {
                self.lfo_knob_moved(&c.id);
            }
        }
        let Some(link) = &mut self.link else {
            return;
        };
        for (row, then) in s.patterns.iter().enumerate() {
            let now = &link.patterns()[row];
            if now.len() != then.len() || then.is_empty() {
                continue;
            }
            let plen = then.len() / 32;
            for (i, (&a, &b)) in now.iter().zip(then).enumerate() {
                if a != b {
                    self.panel.events.push(Event::PatternByte {
                        row,
                        pattern: (i / plen) as u8,
                        at: i % plen,
                        value: b,
                    });
                }
            }
        }
        link.forward(&mut self.panel);
        link.refresh_views(&mut self.panel);
    }
}
