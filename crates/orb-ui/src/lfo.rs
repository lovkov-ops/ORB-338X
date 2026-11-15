//! The LFOs of a bass synth's five large knobs (this fork): CUTOFF, RESO, ENV MOD, DECAY and
//! ACCENT each have their own. An LFO moves its knob's value up and down around the value the
//! knob is set to, while the knob stays where the user put it.
//!
//! Two small knobs drive it: RATE and AMOUNT, how far the value moves (0: no movement, full:
//! the whole range above and below the knob's value, clipped to 0..=127). They sit under the
//! knob, left and right of two buttons that choose how: the note ties the rate to the song's
//! tempo (off: free, in hertz), and the wave button flips between the sine (a smooth up and
//! down) and the random sine, sample & hold: glides to random heights anywhere in the range,
//! each at least `MIN_JUMP` away from the last, each glide 1, 2 or 3 half cycles long (at
//! random: 70%, 20%, 10%), so it goes up or down at random, by random amounts, and in an
//! uneven time.
//!
//! Free, the rate knob goes from `FREE_MIN_HZ` to `FREE_MAX_HZ` on a log scale. Tied to the
//! tempo it picks one cycle's length from `SYNC_BEATS`, and while the song plays the cycle is
//! locked to the song position, so it starts on the bar.

use eframe::egui::{self, Context, PointerButton, Rect, Sense, Ui};

use crate::app::{self, App};
use crate::behavior;
use crate::layout::Ctl;
use crate::random::Rng;

/// The free rate's range, in cycles a second.
pub const FREE_MIN_HZ: f64 = 0.05;
pub const FREE_MAX_HZ: f64 = 20.0;
/// The tempo-tied cycle lengths in beats, from the rate knob's bottom to its top: 4 bars, 2
/// bars, 1 bar, 1/2, 1/4, 1/8, 1/16, 1/32.
pub const SYNC_BEATS: [f64; 8] = [16.0, 8.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.125];
/// The song position's ticks in a beat (768 a bar).
pub const TICKS_PER_BEAT: f64 = 192.0;

/// The rate knob's default value (0..=127: the middle, 1 cycle a second free, a 1/4 or 1/2 note
/// tied to the tempo) and the amount's (no movement).
pub const RATE_DEFAULT: u8 = 64;
pub const AMOUNT_DEFAULT: u8 = 0;

/// The knobs with an LFO, by the name in their id (`tb303_N.<name>`), left to right.
pub const PARAMS: [&str; 5] = ["cutoff", "reso", "envmod", "decay", "accent"];

/// The synth (0 or 1) of a `tb303_N` section.
fn synth_of_section(sec: &str) -> Option<usize> {
    match sec {
        "tb303_1" => Some(0),
        "tb303_2" => Some(1),
        _ => None,
    }
}

/// The synth and the knob (an index into `PARAMS`) whose LFO a knob `id` is the target of
/// (`tb303_N.cutoff` .. `tb303_N.accent`).
pub fn target_of(id: &str) -> Option<(usize, usize)> {
    let (sec, name) = id.split_once('.')?;
    Some((
        synth_of_section(sec)?,
        PARAMS.iter().position(|p| *p == name)?,
    ))
}

/// The ids of a synth's knob `param` (an index into `PARAMS`), its LFO's RATE and AMOUNT knobs
/// and its mode buttons.
pub fn ids(synth: usize, param: usize) -> [String; 4] {
    let (n, p) = (synth + 1, PARAMS[param]);
    [
        format!("tb303_{n}.{p}"),
        format!("tb303_{n}.{p}_lfo"),
        format!("tb303_{n}.{p}_amt"),
        format!("tb303_{n}.{p}_lfo_mode"),
    ]
}

/// The value a double click sets on an LFO knob (`App::knob`).
pub fn knob_default(id: &str) -> Option<u8> {
    let (sec, name) = id.split_once('.')?;
    synth_of_section(sec)?;
    let (param, which) = name.rsplit_once('_')?;
    if !PARAMS.contains(&param) {
        return None;
    }
    match which {
        "lfo" => Some(RATE_DEFAULT),
        "amt" => Some(AMOUNT_DEFAULT),
        _ => None,
    }
}

/// The LFO's wave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    #[default]
    Sine,
    /// Sample & hold, the random sine: a new random height every half cycle, glided to.
    SampleHold,
}

/// The buttons under the knob: tempo sync on or off, and the wave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mode {
    pub sync: bool,
    pub shape: Shape,
}

/// How far the random sine's next height is at least from the last (of the range -1..=1):
/// enough that every half cycle moves audibly.
pub const MIN_JUMP: f64 = 0.6;

/// The random sine's glide lengths in half cycles and how often each comes (they add up to 1).
pub const GLIDES: [(i64, f64); 3] = [(1, 0.7), (2, 0.2), (3, 0.1)];
/// The longest glide of the random sine, in half cycles.
pub const MAX_GLIDE: i64 = 3;

/// A random sine glide's length: 1, 2 or 3 half cycles, as often as `GLIDES` says.
fn glide(rng: &mut Rng) -> i64 {
    let mut x = rng.unit();
    for (len, p) in GLIDES {
        if x < p {
            return len;
        }
        x -= p;
    }
    1
}

/// The random sine's next height after `from`: anywhere in -1..=1, at least `MIN_JUMP` away.
fn peak(rng: &mut Rng, from: f64) -> f64 {
    loop {
        let h = rng.unit() * 2.0 - 1.0;
        if (h - from).abs() >= MIN_JUMP {
            return h;
        }
    }
}

/// The rate in cycles a second for the rate knob at `rate` (0..=1) when free.
pub fn free_hz(rate: f64) -> f64 {
    FREE_MIN_HZ * (FREE_MAX_HZ / FREE_MIN_HZ).powf(rate.clamp(0.0, 1.0))
}

/// The cycle's length in beats for the rate knob at `rate` (0..=1) when tied to the tempo.
pub fn sync_beats(rate: f64) -> f64 {
    let i = (rate.clamp(0.0, 1.0) * (SYNC_BEATS.len() - 1) as f64).round() as usize;
    SYNC_BEATS[i]
}

/// The cutoff value sent: `base` (0..=127) moved by `amount` (0..=1) times the wave's `v`
/// (-1..=1), the full amount reaching the whole range.
pub fn modulate(base: u8, amount: f64, v: f64) -> u8 {
    (base as f64 + amount.clamp(0.0, 1.0) * 127.0 * v.clamp(-1.0, 1.0))
        .round()
        .clamp(0.0, 127.0) as u8
}

/// One synth's LFO as it runs: where it is in its cycles, the sample & hold heights, and the
/// knob's value it moves around.
#[derive(Clone, Copy, Debug)]
pub struct Runner {
    /// Cycles run so far (the whole part counts the cycles, the fraction is the phase).
    pub cycles: f64,
    /// The sample & hold: the height its glide starts from and the one it glides to.
    held: (f64, f64),
    /// The glide's first half cycle and its length in half cycles (1..=`MAX_GLIDE`).
    held_cycle: i64,
    held_len: i64,
    rng: Rng,
    /// The knob's value the LFO moves around while it runs (None: not running).
    pub base: Option<u8>,
    /// The last value sent to the engine.
    pub sent: Option<u8>,
}

impl Runner {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let first = rng.unit() * 2.0 - 1.0;
        let second = peak(&mut rng, first);
        let len = glide(&mut rng);
        Self {
            cycles: 0.0,
            held: (first, second),
            held_cycle: 0,
            held_len: len,
            rng,
            base: None,
            sent: None,
        }
    }

    /// Moves the LFO on by `dt` seconds. Tied to the tempo (`tempo` in BPM * 1000) while the
    /// song plays at `position` (ticks), the cycles follow the song position instead.
    pub fn advance(&mut self, dt: f64, rate: f64, mode: Mode, tempo: i32, position: Option<i32>) {
        if mode.sync {
            let beats = sync_beats(rate);
            match position {
                Some(p) => self.cycles = p.max(0) as f64 / (TICKS_PER_BEAT * beats),
                None => self.cycles += dt * (tempo.max(1) as f64 / 1000.0 / 60.0) / beats,
            }
        } else {
            self.cycles += dt * free_hz(rate);
        }
    }

    /// The wave's height now, -1..=1.
    pub fn value(&mut self, shape: Shape) -> f64 {
        let phase = self.cycles.fract();
        match shape {
            Shape::Sine => (std::f64::consts::TAU * phase).sin(),
            Shape::SampleHold => {
                // glides of 1 to 4 half cycles, each to a new random height, up or down
                let u = self.cycles * 2.0;
                let half = u.floor() as i64;
                let end = self.held_cycle + self.held_len;
                if half >= end && half < end + MAX_GLIDE {
                    // the glide ended: the next one glides on from where it was
                    self.held = (self.held.1, peak(&mut self.rng, self.held.1));
                    self.held_cycle = end;
                    self.held_len = glide(&mut self.rng);
                    // a step longer than a glide (a slow frame) may skip one more
                    while half >= self.held_cycle + self.held_len {
                        self.held = (self.held.1, peak(&mut self.rng, self.held.1));
                        self.held_cycle += self.held_len;
                        self.held_len = glide(&mut self.rng);
                    }
                } else if half < self.held_cycle || half >= end {
                    // a jump (the song located elsewhere): new heights from here
                    let a = self.rng.unit() * 2.0 - 1.0;
                    self.held = (a, peak(&mut self.rng, a));
                    self.held_cycle = half;
                    self.held_len = glide(&mut self.rng);
                }
                let (a, b) = self.held;
                let t = (u - self.held_cycle as f64) / self.held_len as f64;
                let k = (1.0 - (std::f64::consts::PI * t).cos()) / 2.0;
                a + (b - a) * k
            }
        }
    }
}

impl App {
    /// The mode buttons of a synth's knob `param`'s LFO.
    pub fn lfo_mode(&self, synth: usize, param: usize) -> Mode {
        self.lfo_mode[synth][param]
    }

    /// A press on an LFO's mode button: `button` 0 (the note) turns tempo sync on or off, 1
    /// (the wave) flips between the sine and the random sine (sample & hold).
    pub fn press_lfo_mode(&mut self, synth: usize, param: usize, button: usize) {
        let m = &mut self.lfo_mode[synth][param];
        let before = *m;
        match button {
            0 => m.sync = !m.sync,
            _ => {
                m.shape = match m.shape {
                    Shape::Sine => Shape::SampleHold,
                    Shape::SampleHold => Shape::Sine,
                }
            }
        }
        if *m != before {
            self.doc.modified = true;
        }
    }

    /// The value an LFO moves its knob's value around while it runs (the user's), and the value
    /// it sent last; None when it does not run.
    pub fn lfo_base(&self, synth: usize, param: usize) -> Option<u8> {
        self.lfo_run[synth][param].base
    }

    pub fn lfo_sent(&self, synth: usize, param: usize) -> Option<u8> {
        let run = &self.lfo_run[synth][param];
        run.base.and(run.sent)
    }

    /// All LFO knobs and buttons at their defaults, and the LFOs stopped (a new or an opened
    /// song; an .rbsx then sets its own, rbsx.rs).
    pub(crate) fn lfo_reset_knobs(&mut self) {
        let to = |v: u8| behavior::knob_from_engine(v as i32);
        for synth in 0..2 {
            for param in 0..PARAMS.len() {
                let [_, rate, amount, _] = ids(synth, param);
                self.panel.put_knob(&rate, to(RATE_DEFAULT));
                self.panel.put_knob(&amount, to(AMOUNT_DEFAULT));
                self.lfo_mode[synth][param] = Mode::default();
                self.lfo_run[synth][param].base = None;
                self.lfo_run[synth][param].sent = None;
            }
        }
    }

    /// The user moved knob `id`: when an LFO runs on it, its value is the new one to move
    /// around.
    pub(crate) fn lfo_knob_moved(&mut self, id: &str) {
        if let Some((k, p)) = target_of(id)
            && self.lfo_run[k][p].base.is_some()
        {
            self.lfo_run[k][p].base = self.panel.value(id);
        }
    }

    /// The knob position of the value an LFO sent last for knob `id`, while it runs on it: the
    /// arc shows it (the pointer keeps the user's value).
    pub(crate) fn lfo_arc(&self, id: &str) -> Option<i32> {
        let (k, p) = target_of(id)?;
        self.lfo_sent(k, p)
            .map(|v| behavior::knob_from_engine(v as i32))
    }

    /// The mode buttons under a knob: a click on a third of them presses that button.
    pub(crate) fn lfo_mode_control(&mut self, ui: &mut Ui, c: &Ctl, r: Rect) {
        let Some((k, p)) = target_of(c.id.strip_suffix("_lfo_mode").unwrap_or("")) else {
            return;
        };
        let resp = ui.interact(r, egui::Id::unique(("lfo_mode", &c.id)), Sense::click());
        let pressed = ui.input(|i| i.pointer.button_pressed(PointerButton::Primary));
        if pressed
            && resp.contains_pointer()
            && let Some(at) = ui.input(|i| i.pointer.press_origin())
        {
            let half = ((at.x - r.min.x) / r.width() * 2.0).floor().clamp(0.0, 1.0) as usize;
            self.press_lfo_mode(k, p, half);
        }
        let m = self.lfo_mode[k][p];
        let hold = m.shape == Shape::SampleHold;
        let name = format!("lfo_mode_{}{}", m.sync as u8, hold as u8);
        app::paint_part(ui, &name, r);
    }

    /// One step of every LFO, after the panel took the engine's values: a running LFO sends its
    /// knob's moved value to the engine and puts the user's value back on the knob. An LFO
    /// whose amount goes to 0 stops and sends the user's value back.
    pub(crate) fn lfo_step(&mut self, ctx: &Context, now: f64) {
        let dt = (now - self.lfo_time.unwrap_or(now)).clamp(0.0, 0.25);
        self.lfo_time = Some(now);
        let mut running = false;
        for k in 0..2 {
            for p in 0..PARAMS.len() {
                running |= self.lfo_step_one(k, p, dt);
            }
        }
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// [`App::lfo_step`] for synth `k`'s knob `p`: whether its LFO runs.
    fn lfo_step_one(&mut self, k: usize, p: usize, dt: f64) -> bool {
        let [knob, rate, amount, _] = ids(k, p);
        let unit = |v: Option<u8>, d: u8| v.unwrap_or(d) as f64 / 127.0;
        let amount = unit(self.panel.value(&amount), AMOUNT_DEFAULT);
        let rate = unit(self.panel.value(&rate), RATE_DEFAULT);
        let Some(link) = &mut self.link else {
            return false;
        };
        let run = &mut self.lfo_run[k][p];
        if amount <= 0.0 {
            if let Some(b) = run.base.take() {
                link.send_value(&knob, b);
                self.panel.adopt(&knob, b as i32);
            }
            run.sent = None;
            return false;
        }
        let base = match run.base {
            Some(b) => b,
            None => {
                let b = self.panel.value(&knob).unwrap_or(0);
                run.base = Some(b);
                b
            }
        };
        let t = &self.panel.transport;
        let position = t.playing.then_some(t.position);
        let mode = self.lfo_mode[k][p];
        run.advance(dt, rate, mode, t.tempo, position);
        let v = modulate(base, amount, run.value(mode.shape));
        if run.sent != Some(v) {
            link.send_value(&knob, v);
            run.sent = Some(v);
        }
        // the knob shows the user's value, not the engine's moved one
        self.panel.adopt(&knob, base as i32);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn the_free_rate_runs_from_slow_to_fast() {
        assert!(close(free_hz(0.0), FREE_MIN_HZ));
        assert!(close(free_hz(1.0), FREE_MAX_HZ));
        assert!(close(free_hz(0.5), (FREE_MIN_HZ * FREE_MAX_HZ).sqrt()));
        assert!(free_hz(0.3) < free_hz(0.6));
    }

    #[test]
    fn the_tempo_rate_takes_one_of_eight_lengths() {
        assert_eq!(sync_beats(0.0), 16.0);
        assert_eq!(sync_beats(1.0), 0.125);
        assert_eq!(sync_beats(4.0 / 7.0), 1.0);
        // a quarter note at 120 BPM is half a second
        let mut r = Runner::new(1);
        let mode = Mode {
            sync: true,
            shape: Shape::Sine,
        };
        r.advance(0.5, 4.0 / 7.0, mode, 120_000, None);
        assert!(close(r.cycles, 1.0));
        // while the song plays the cycles follow its position: a bar of 1/4 notes is 4
        r.advance(0.0, 4.0 / 7.0, mode, 120_000, Some(768));
        assert!(close(r.cycles, 4.0));
    }

    #[test]
    fn the_sine_goes_up_and_down_once_a_cycle() {
        let mut r = Runner::new(1);
        let mode = Mode::default();
        let rate = 0.0; // 0.05 Hz: 5 s to a quarter cycle
        assert!(close(r.value(Shape::Sine), 0.0));
        r.advance(5.0, rate, mode, 0, None);
        assert!(close(r.value(Shape::Sine), 1.0));
        r.advance(10.0, rate, mode, 0, None);
        assert!(close(r.value(Shape::Sine), -1.0));
    }

    #[test]
    fn sample_and_hold_glides_to_a_new_random_height_each_half_cycle() {
        let mut r = Runner::new(7);
        let mode = Mode {
            sync: false,
            shape: Shape::SampleHold,
        };
        let mut starts = vec![];
        for _ in 0..40 {
            // the start of each cycle, then its middle: always within -1..=1, and smooth
            starts.push(r.value(Shape::SampleHold));
            r.advance(10.0, 0.0, mode, 0, None); // half a cycle at 0.05 Hz
            let mid = r.value(Shape::SampleHold);
            assert!((-1.0..=1.0).contains(&mid));
            r.advance(10.0, 0.0, mode, 0, None);
        }
        // the heights differ from cycle to cycle
        assert!(starts.windows(2).filter(|w| !close(w[0], w[1])).count() > 30);
        // a cycle ends where the next starts: no jump
        let mut r = Runner::new(3);
        r.advance(19.999_999, 0.0, mode, 0, None);
        let before = r.value(Shape::SampleHold);
        r.advance(0.000_002, 0.0, mode, 0, None);
        assert!((r.value(Shape::SampleHold) - before).abs() < 1e-3);
    }

    /// How far a wave travels (up and down added) and how often it turns, over 20 cycles at
    /// 0.05 Hz in small steps.
    fn travel(shape: Shape) -> (f64, usize) {
        let mut r = Runner::new(7);
        let mode = Mode { sync: false, shape };
        let (mut dist, mut turns, mut last, mut dir) = (0.0, 0, r.value(shape), 0.0f64);
        for _ in 0..20 * 400 {
            r.advance(20.0 / 400.0, 0.0, mode, 0, None);
            let v = r.value(shape);
            let d = v - last;
            if d.abs() > 1e-12 {
                if dir != 0.0 && d.signum() != dir.signum() {
                    turns += 1;
                }
                dir = d;
            }
            dist += d.abs();
            last = v;
        }
        (dist, turns)
    }

    /// (this fork) The random sine is no sine: it glides to random heights over the whole
    /// range, up or down at random, each glide 1, 2 or 3 half cycles long at random.
    #[test]
    fn the_random_sine_moves_at_random_heights_and_times() {
        let (sine, sine_turns) = travel(Shape::Sine);
        let (random, random_turns) = travel(Shape::SampleHold);
        assert!((39..=41).contains(&sine_turns), "{sine_turns}");
        assert!((4..30).contains(&random_turns), "{random_turns}");
        assert!(
            random > 0.15 * sine && random < 1.1 * sine,
            "{random} vs {sine}"
        );
        // the glides: their heights and lengths, half a cycle (10 s at 0.05 Hz) at a time
        let mut r = Runner::new(11);
        let mode = Mode {
            sync: false,
            shape: Shape::SampleHold,
        };
        let mut glides: Vec<(i64, i64, f64)> = vec![];
        for _ in 0..400 {
            r.advance(10.0, 0.0, mode, 0, None);
            r.value(Shape::SampleHold); // the heights are drawn as the wave is read
            let g = (r.held_cycle, r.held_len, r.held.1);
            if glides.last() != Some(&g) {
                glides.push(g);
            }
        }
        // one after another, no gap; every length from 1 to 3
        for w in glides.windows(2) {
            assert_eq!(w[0].0 + w[0].1, w[1].0);
        }
        for len in 1..=MAX_GLIDE {
            assert!(glides.iter().any(|g| g.1 == len), "a glide of {len}");
        }
        assert!(glides.iter().all(|g| (1..=MAX_GLIDE).contains(&g.1)));
        // as often as GLIDES says: 70%, 20%, 10% (of many draws, within a few percent)
        let mut rng = Rng::new(17);
        let mut count = [0usize; 4];
        for _ in 0..20_000 {
            count[glide(&mut rng) as usize] += 1;
        }
        for (len, p) in GLIDES {
            let share = count[len as usize] as f64 / 20_000.0;
            assert!((share - p).abs() < 0.02, "{len}: {share} vs {p}");
        }
        // the heights spread over the range, the small ones too; two in a row can go the same
        // way; each one at least MIN_JUMP from the last
        let peaks: Vec<f64> = glides.iter().map(|g| g.2).collect();
        assert!(peaks.iter().any(|&h| h > 0.8) && peaks.iter().any(|&h| h < -0.8));
        assert!(peaks.iter().any(|&h| h.abs() < 0.3), "small heights too");
        let ups: Vec<bool> = peaks.windows(2).map(|w| w[1] > w[0]).collect();
        assert!(
            ups.windows(2).any(|u| u[0] == u[1]),
            "not always up and down in turn"
        );
        for w in peaks.windows(2) {
            assert!((w[1] - w[0]).abs() >= MIN_JUMP - 1e-9);
        }
        // a jump back (the song located to its start) starts anew, without a gap in the curve
        let mut r = Runner::new(3);
        r.advance(1000.0, 0.0, mode, 0, None);
        r.value(Shape::SampleHold);
        r.cycles = 0.0;
        let v = r.value(Shape::SampleHold);
        assert!((-1.0..=1.0).contains(&v));
        assert_eq!(r.held_cycle, 0);
    }

    #[test]
    fn the_amount_moves_the_value_around_the_knob_within_the_range() {
        assert_eq!(modulate(64, 0.0, 1.0), 64);
        assert_eq!(modulate(64, 0.5, 1.0), 127);
        assert_eq!(modulate(64, 0.25, -1.0), 32);
        assert_eq!(modulate(100, 1.0, 1.0), 127);
        assert_eq!(modulate(10, 1.0, -1.0), 0);
    }
}
