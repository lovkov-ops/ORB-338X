//! The parts cut out of the mockup (this fork, mockup.rs): [`apply`] replaces the generated
//! parts of the sections done so far by the mockup's own shapes, [`place`] puts their controls
//! where the mockup has them.
//!
//! A control of the mockup is a group of it, named here by its chain of child indexes
//! (`"1.21"`: the shuffle knob, the 21st child of the top bar, the document's child 1). A part
//! is that group's shapes under a `viewBox` of the part's box; the section's background is
//! what is left of the section when its controls are taken out. A state the mockup does not
//! show (a pressed button) is the shown one recolored, the way the generated parts make it.

use std::collections::{HashMap, HashSet};

use crate::mockup::{Bx, Mockup, Shape};
use crate::svg::Part;

/// The shapes of group `chain` (`"1.21"`) of mockup `m`.
pub fn grp<'a>(m: &'a Mockup, chain: &str) -> Vec<&'a Shape> {
    let want: Vec<usize> = chain.split('.').map(|c| c.parse().unwrap()).collect();
    let v: Vec<&Shape> = m
        .shapes
        .iter()
        .filter(|s| s.chain.starts_with(&want))
        .collect();
    assert!(!v.is_empty(), "mockup group {chain} is empty");
    v
}

/// The shapes of several groups, in paint order.
pub fn grps<'a>(m: &'a Mockup, chains: &[&str]) -> Vec<&'a Shape> {
    let mut v: Vec<&Shape> = chains.iter().flat_map(|c| grp(m, c)).collect();
    v.sort_by_key(|s| s.index);
    v.dedup_by_key(|s| s.index);
    v
}

/// The box of the shapes.
pub fn bbox(v: &[&Shape]) -> Bx {
    v.iter().skip(1).fold(v[0].bbox, |b, s| b.union(&s.bbox))
}

/// The SVG of the shapes, in paint order.
pub fn body(v: &[&Shape]) -> String {
    v.iter().map(|s| s.svg()).collect()
}

/// A part of size `w` x `h` (rounded up to whole pixels, as every part is) whose top-left
/// corner is panel point (`x`, `y`) of the mockup.
pub fn part(name: &str, x: f32, y: f32, w: f32, h: f32, body: &str) -> Part {
    Part {
        name: name.to_string(),
        w: (w - 0.01).ceil(),
        h: (h - 0.01).ceil(),
        body: format!(
            r#"<g transform="translate({} {})">{body}</g>"#,
            crate::svg::n(-x),
            crate::svg::n(-y)
        ),
    }
}

/// A part of the shapes, its box their box rounded out to whole pixels.
pub fn part_of(name: &str, v: &[&Shape]) -> (Part, Bx) {
    let b = bbox(v);
    let (x, y) = (b.x0.floor(), b.y0.floor());
    let (w, h) = ((b.x1 - x).ceil(), (b.y1 - y).ceil());
    (part(name, x, y, w, h, &body(v)), Bx::new(x, y, w, h))
}

/// `body` with the shapes' plain colors swapped as `swaps` says (hex `#rrggbb`).
pub fn recolor(v: &[&Shape], swaps: &[(&str, &str)]) -> String {
    let mut s = body(v);
    for (a, b) in swaps {
        s = s.replace(&format!("\"{a}\""), &format!("\"{b}\""));
    }
    s
}

/// The shapes' SVG painted black at `opacity`: a shade over a pressed button.
pub fn shade(v: &[&Shape], opacity: f32) -> String {
    let mut s = String::new();
    for sh in v {
        let mut t = sh.svg();
        if let Some(hex) = sh.hex() {
            t = t.replace(&format!("fill=\"{hex}\""), "fill=\"#000000\"");
        }
        s.push_str(&t);
    }
    format!(r#"<g opacity="{opacity}">{s}</g>"#)
}

/// Controls of a section and kind, each placed at the nearest of the centers.
type Near = (String, &'static str, Vec<(f32, f32)>);

/// What the cut parts of the done sections are, and where their controls go.
pub struct Cuts {
    pub parts: Vec<Part>,
    /// Control id (or `id#kind`): its top-left corner in panel pixels.
    pub places: HashMap<String, (f32, f32)>,
    /// The mockup's shapes taken by controls (mockup, index), left out of the backgrounds.
    taken: HashSet<(u8, usize)>,
    /// Controls placed by the nearest of the mockup's centers: section, kind, the centers.
    near: Vec<Near>,
}

impl Cuts {
    fn new() -> Self {
        Cuts {
            parts: Vec::new(),
            places: HashMap::new(),
            taken: HashSet::new(),
            near: Vec::new(),
        }
    }

    fn take(&mut self, v: &[&Shape]) {
        self.taken.extend(v.iter().map(|s| (s.doc, s.index)));
    }

    fn place(&mut self, id: &str, x: f32, y: f32) {
        self.places.insert(id.to_string(), (x, y));
    }

    /// The background `name` of section `sec` at its panel box `b`: the section's shapes no
    /// control took.
    fn background(&mut self, m: &Mockup, sec: &str, name: &str, b: Bx) {
        let v: Vec<&Shape> = m
            .section(sec)
            .filter(|s| !self.taken.contains(&(s.doc, s.index)))
            .collect();
        self.parts
            .push(part(name, b.x0, b.y0, b.w(), b.h(), &body(&v)));
    }
}

/// The cut parts and places of every section done.
pub fn cuts() -> &'static Cuts {
    static C: std::sync::OnceLock<Cuts> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let mut c = Cuts::new();
        top_bar(&mut c);
        for r in 0..4 {
            pattern(&mut c, r);
        }
        tb303(&mut c, 0);
        tb303(&mut c, 1);
        tr808(&mut c);
        tr909(&mut c);
        mixer_and_effects(&mut c);
        c
    })
}

/// Replaces the generated parts by the cut ones of the same name (and adds new ones).
pub fn apply(parts: &mut Vec<Part>) {
    for p in &cuts().parts {
        match parts.iter_mut().find(|q| q.name == p.name) {
            Some(q) => *q = p.clone(),
            None => parts.push(p.clone()),
        }
    }
}

/// Puts the controls of the done sections where the mockup has them.
pub fn place(l: &mut crate::layout::Layout) {
    let places = &cuts().places;
    for (sec, kind, centers) in &cuts().near {
        for c in l
            .ctls
            .iter_mut()
            .filter(|c| &c.section == sec && c.kind == *kind)
        {
            let (x, y) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
            let d = |p: &(f32, f32)| (p.0 - x).powi(2) + (p.1 - y).powi(2);
            let best = centers
                .iter()
                .min_by(|a, b| d(a).total_cmp(&d(b)))
                .expect("centers");
            assert!(d(best) < 14.0 * 14.0, "{} has no mockup center near", c.id);
            c.x = best.0 - c.w / 2.0;
            c.y = best.1 - c.h / 2.0;
        }
    }
    for c in &mut l.ctls {
        // `id#kind` for an id two controls share (the 303's ACCENT knob and flag)
        let both = format!("{}#{}", c.id, c.kind);
        if let Some(&(x, y)) = places.get(&both).or_else(|| places.get(&c.id)) {
            c.x = x;
            c.y = y;
        }
    }
}

/// The seven segments a .. g of each digit 0 ..= 9.
const DIGITS: [[bool; 7]; 10] = {
    const T: bool = true;
    const F: bool = false;
    [
        [T, T, T, T, T, T, F],
        [F, T, T, F, F, F, F],
        [T, T, F, T, T, F, T],
        [T, T, T, T, F, F, T],
        [F, T, T, F, F, T, T],
        [T, F, T, T, F, T, T],
        [T, F, T, T, T, T, T],
        [T, T, T, F, F, F, F],
        [T, T, T, T, T, T, T],
        [T, T, T, T, F, T, T],
    ]
};

/// The lit and unlit color of the segments.
const SEG_ON: &str = "#ff3b2f";
const SEG_OFF: &str = "#3a1a17";

/// The digit frames `{family}_0` .. `_9` and `_blank`, 9 x 16, cut from the seven segments
/// `seg` (a .. g, the mockup's order) of a digit whose cell's top-left corner is (`x`, `y`).
fn digits(c: &mut Cuts, family: &str, seg: &[&Shape], x: f32, y: f32) {
    let one = |on: [bool; 7]| -> String {
        seg.iter()
            .zip(on)
            .map(|(s, on)| {
                let col = if on { SEG_ON } else { SEG_OFF };
                let hex = s.hex().unwrap();
                s.svg()
                    .replace(&format!("\"{hex}\""), &format!("\"{col}\""))
            })
            .collect()
    };
    for (d, on) in DIGITS.iter().enumerate() {
        c.parts
            .push(part(&format!("{family}_{d}"), x, y, 9.0, 16.0, &one(*on)));
    }
    c.parts.push(part(
        &format!("{family}_blank"),
        x,
        y,
        9.0,
        16.0,
        &one([false; 7]),
    ));
}

/// The rotation in degrees of the pointer shapes `v` (the mockup draws a knob at some value).
fn pointer_angle(v: &[&Shape]) -> f32 {
    v[0].rot
}

/// A knob's body and pointer parts, `w` x `h` around the center (`cx`, `cy`) of the mockup's
/// knob: the pointer turned back upright.
fn knob(c: &mut Cuts, name: &str, body_v: &[&Shape], ptr: &[&Shape], k: (f32, f32, f32, f32)) {
    let (cx, cy, w, h) = k;
    let (x, y) = (cx - w / 2.0, cy - h / 2.0);
    c.parts
        .push(part(&format!("{name}_body"), x, y, w, h, &body(body_v)));
    let a = -pointer_angle(ptr);
    let up = format!(
        r#"<g transform="rotate({} {} {})">{}</g>"#,
        crate::svg::n(a),
        crate::svg::n(cx),
        crate::svg::n(cy),
        body(ptr)
    );
    c.parts
        .push(part(&format!("{name}_pointer"), x, y, w, h, &up));
}

/// The top bar: the shuffle knob, the displays and their arrows, the mode switch, the
/// transport buttons, the loop button and the logo.
fn top_bar(c: &mut Cuts) {
    let m = Mockup::panel_1();
    // the shuffle knob: its track and body, the value arc (drawn by the app) left out
    let kb = grps(m, &["1.21.0", "1.21.1"]);
    let kp = grp(m, "1.21.3");
    let ka = grp(m, "1.21.2");
    let body_c = bbox(&grp(m, "1.21.1"));
    let (cx, cy) = (body_c.cx(), body_c.cy());
    knob(c, "knob_22", &kb, &kp, (cx, cy, 22.0, 22.0));
    c.take(&kb);
    c.take(&kp);
    c.take(&ka);
    c.place("transport.shuffle", cx - 11.0, cy - 11.0);

    // the displays: the segments are the digit frames, the wells stay in the background
    let segs = |d: &str, k: usize| -> Vec<&Shape> {
        (0..7)
            .map(|i| grp(m, &format!("{d}.{}", 2 + 7 * k + i))[0])
            .collect()
    };
    let tempo = segs("1.4", 0);
    let cell = bbox(&tempo);
    // the cell's top-left corner: the segments sit 1.2 in and 1.5 down
    let (sx, sy) = ((cell.x0 - 1.2).round(), (cell.y0 - 1.5).round());
    digits(c, "seg", &tempo, sx, sy);
    digits(c, "segb", &tempo, sx, sy);
    for (d, id) in [
        ("1.4", "tempo"),
        ("1.5", "bar"),
        ("1.6", "loop_start"),
        ("1.7", "loop_length"),
    ] {
        for k in 0..3 {
            c.take(&segs(d, k));
        }
        let b = bbox(&segs(d, 0));
        c.place(
            &format!("transport.{id}"),
            (b.x0 - 1.2).round(),
            (b.y0 - 1.5).round(),
        );
    }

    // the arrows: the two keys of each pair (its frame stays in the background)
    let inc = grps(m, &["1.22.1", "1.22.2"]);
    let dec = grps(m, &["1.22.3", "1.22.4"]);
    for (name, v) in [("arrow_inc", &inc), ("arrow_dec", &dec)] {
        let b = bbox(&grp(
            m,
            if name == "arrow_inc" {
                "1.22.1"
            } else {
                "1.22.3"
            },
        ));
        let rest = body(v);
        let pressed = recolor(v, &[("#2c3542", "#56647a")]);
        c.parts.push(part(
            &format!("{name}_rest"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &rest,
        ));
        c.parts.push(part(
            &format!("{name}_pressed"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &pressed,
        ));
    }
    for (g, id) in [
        ("1.22", "tempo"),
        ("1.39", "bar"),
        ("1.40", "loop_start"),
        ("1.41", "loop_length"),
    ] {
        let up = grp(m, &format!("{g}.1"));
        let down = grp(m, &format!("{g}.3"));
        c.take(&grp(m, &format!("{g}.1")));
        c.take(&grp(m, &format!("{g}.2")));
        c.take(&grp(m, &format!("{g}.3")));
        c.take(&grp(m, &format!("{g}.4")));
        let (u, d) = (bbox(&up), bbox(&down));
        c.place(&format!("transport.{id}_up"), u.x0, u.y0);
        c.place(&format!("transport.{id}_down"), d.x0, d.y0);
    }

    // the pattern/song switch: its handle on the right (song); on the left for pattern
    let track = grp(m, "1.23");
    let handle = grps(m, &["1.24", "1.25"]);
    let t = bbox(&track);
    let hb = bbox(&handle);
    let shift = (t.x0 + 1.0) - hb.x0;
    let right = format!("{}{}", body(&track), body(&handle));
    let left = format!(
        r#"{}<g transform="translate({} 0)">{}</g>"#,
        body(&track),
        crate::svg::n(shift),
        body(&handle)
    );
    c.parts
        .push(part("switch_mode_1", t.x0, t.y0, t.w(), t.h(), &right));
    c.parts
        .push(part("switch_mode_0", t.x0, t.y0, t.w(), t.h(), &left));
    c.take(&track);
    c.take(&handle);
    c.place("transport.mode", t.x0, t.y0);

    // the transport keys: face and icon; lit is the play key's face, pressed a shade
    let keys: [(&str, &[&str], bool); 5] = [
        ("play", &["1.27", "1.28"], true),
        ("stop", &["1.29", "1.30"], false),
        ("rewind", &["1.31", "1.32", "1.33"], false),
        ("ff", &["1.34", "1.35", "1.36"], false),
        ("record", &["1.37", "1.38"], true),
    ];
    for (name, chains, has_lit) in keys {
        let v = grps(m, chains);
        let face = grp(m, chains[0]);
        let b = bbox(&face);
        let face_hex = face[0].hex().unwrap();
        let rest = recolor(&face, &[(face_hex.as_str(), "#2c3542")]) + &body(&v[1..]);
        let pressed = format!("{rest}{}", shade(&face, 0.2));
        let mk = |state: &str, svg: &str| {
            part(
                &format!("btn_{name}_{state}"),
                b.x0,
                b.y0,
                b.w(),
                b.h(),
                svg,
            )
        };
        c.parts.push(mk("rest", &rest));
        c.parts.push(mk("pressed", &pressed));
        if has_lit {
            let lit = if name == "record" {
                recolor(&face, &[(face_hex.as_str(), "#e2413b")])
                    + &recolor(&v[1..], &[("#e2413b", "#fbfbf8")])
            } else {
                recolor(&face, &[(face_hex.as_str(), "#56647a")]) + &body(&v[1..])
            };
            c.parts.push(mk("lit", &lit));
        }
        c.take(&v);
        c.place(&format!("transport.{name}"), b.x0, b.y0);
    }

    // the loop key: on is the mockup's (amber), off slate with a light sign
    let face = grp(m, "1.42.0");
    let sign = grps(m, &["1.42.1", "1.42.2", "1.42.3", "1.42.4"]);
    let b = bbox(&face);
    let on = format!("{}{}", body(&face), body(&sign));
    let off =
        recolor(&face, &[("#ffb000", "#2c3542")]) + &recolor(&sign, &[("#1b1c1e", "#e9edf2")]);
    c.parts
        .push(part("toggle_loop_on", b.x0, b.y0, b.w(), b.h(), &on));
    c.parts
        .push(part("toggle_loop_off", b.x0, b.y0, b.w(), b.h(), &off));
    c.take(&face);
    c.take(&sign);
    c.place("transport.loop", b.x0, b.y0);

    // the logo: the mockup's is the generated one (parts.rs, whose pieces the startup animation
    // moves), so it stays; its shapes are left out of the background
    for g in 13..=20 {
        c.take(&grp(m, &format!("1.{g}")));
    }

    c.background(
        m,
        "top-bar",
        "bg_transport",
        Bx::new(16.0, 0.0, 592.0, 50.0),
    );

    // the wood at the panel's sides
    let wood = grp(m, "0.1");
    c.parts
        .push(part("wood", 0.0, 0.0, 16.0, 510.0, &body(&wood)));
}

/// The shapes `v` with shape `i`'s plain fill painted `cols[i]` (`""` keeps it).
fn restyle(v: &[&Shape], cols: &[&str]) -> String {
    let mut out = String::new();
    for (i, sh) in v.iter().enumerate() {
        let mut t = sh.svg();
        if let (Some(hex), Some(col)) = (sh.hex(), cols.get(i))
            && !col.is_empty()
        {
            t = t.replace(&format!("fill=\"{hex}\""), &format!("fill=\"{col}\""));
        }
        out.push_str(&t);
    }
    out
}

/// An LED part `w` x `h` around the center of the shapes' circle `core` (the dark disc).
fn led(c: &mut Cuts, name: &str, v: &[&Shape], core: &Shape, w: f32, h: f32) {
    let (cx, cy) = (core.bbox.cx(), core.bbox.cy());
    c.parts
        .push(part(name, cx - w / 2.0, cy - h / 2.0, w, h, &body(v)));
}

/// The pattern sections of mockup 1, by row.
const PATTERN_SEC: [&str; 4] = [
    "pattern-303-a",
    "pattern-303-b",
    "pattern-808",
    "pattern-909",
];

/// The pattern section of row `r`: the on LED, the eight pattern and four bank buttons, the
/// shuffle key, the steps display and its arrows, and the row's focus bar.
fn pattern(c: &mut Cuts, r: usize) {
    let m = Mockup::panel_1();
    let g = 6 + r;
    let at = |k: usize| format!("{g}.{k}");
    let sec = format!("pattern{}", r + 1);
    // the on LED (lit in the mockup; unlit as the mixer's second channel has it)
    let on = grp(m, &at(8));
    if r == 0 {
        led(c, "power_13x13_on", &on, grp(m, &at(8))[1], 13.0, 13.0);
        let off = grp(m, "11.4");
        led(c, "power_13x13_off", &off, off[1], 13.0, 13.0);
    }
    let core = on[1].bbox;
    c.place(&format!("{sec}.on"), core.cx() - 6.5, core.cy() - 6.5);
    c.take(&on);

    // the pattern buttons: rim, face, digit; the lit one red with a white digit
    const PAT_ON: [&str; 3] = ["#9c2b25", "#e8503f", "#fbfbf8"];
    const PAT_OFF: [&str; 3] = ["#a7afba", "#dde1e6", "#1b1c1e"];
    for k in 0..8 {
        let v = grp(m, &at(9 + k));
        let b = bbox(&v);
        if r == 0 {
            for (state, cols) in [("on", PAT_ON), ("off", PAT_OFF)] {
                let name = format!("pat_btn_{}_{state}", k + 1);
                c.parts
                    .push(part(&name, b.x0, b.y0, b.w(), b.h(), &restyle(&v, &cols)));
            }
        }
        c.take(&v);
        c.place(&format!("{sec}.pattern{}", k + 1), b.x0, b.y0);
    }
    // the bank buttons: the chosen one orange
    const BANK_ON: [&str; 3] = ["#a85d14", "#f08a24", "#1b1c1e"];
    const BANK_OFF: [&str; 3] = ["#2c3542", "#4e5a6b", "#9aa7b8"];
    for (i, name) in ["A", "B", "C", "D"].into_iter().enumerate() {
        let v = grp(m, &at(17 + i));
        let b = bbox(&v);
        if r == 0 {
            for (state, cols) in [("on", BANK_ON), ("off", BANK_OFF)] {
                let part_name = format!("bank_btn_{name}_{state}");
                c.parts.push(part(
                    &part_name,
                    b.x0,
                    b.y0,
                    b.w(),
                    b.h(),
                    &restyle(&v, &cols),
                ));
            }
        }
        c.take(&v);
        c.place(&format!("{sec}.bank{name}"), b.x0, b.y0);
    }
    // the shuffle key: light, red while on
    let v = grp(m, &at(21));
    let b = bbox(&v);
    if r == 0 {
        for (state, cols) in [("on", PAT_ON), ("off", PAT_OFF)] {
            let name = format!("toggle_shuffle_{state}");
            c.parts
                .push(part(&name, b.x0, b.y0, b.w(), b.h(), &restyle(&v, &cols)));
        }
    }
    c.take(&v);
    c.place(&format!("{sec}.shuffle"), b.x0, b.y0);
    // the steps display (its well stays) and its arrows (their frame stays)
    let digits_v: Vec<&Shape> = (0..14)
        .map(|i| grp(m, &format!("{g}.5.2.{i}"))[0])
        .collect();
    c.take(&digits_v);
    let (x, y) = display_at(&digits_v[..7], 19.0, 2);
    c.place(&format!("{sec}.steps"), x, y);
    for (k, id) in [(1, "steps_up"), (3, "steps_down")] {
        let key = grp(m, &format!("{g}.22.{k}"));
        let b = bbox(&key);
        c.take(&key);
        c.take(&grp(m, &format!("{g}.22.{}", k + 1)));
        c.place(&format!("{sec}.{id}"), b.x0, b.y0);
    }
    // the row's focus bar, right of the section
    let gap = grp(m, &at(6));
    let bar = grp(m, &at(7));
    if r == 0 {
        let b = bbox(&gap);
        let lit = format!("{}{}", body(&gap), body(&bar));
        let unlit = format!("{}{}", body(&gap), recolor(&bar, &[("#f08a24", "#2c3542")]));
        c.parts
            .push(part("focus_on", b.x0, b.y0, b.w(), b.h(), &lit));
        c.parts
            .push(part("focus_off", b.x0, b.y0, b.w(), b.h(), &unlit));
    }
    c.take(&gap);
    c.take(&bar);
    if r == 0 {
        c.background(
            m,
            PATTERN_SEC[0],
            "bg_pattern",
            Bx::new(16.0, 50.0, 70.0, 115.0),
        );
    }
}

/// A press overlay over box `b`: the generated parts' gray veil.
fn veil(b: &Bx) -> String {
    format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"2\" fill=\"#808080\" \
         fill-opacity=\"0.4\"/>",
        crate::svg::n(b.x0),
        crate::svg::n(b.y0),
        crate::svg::n(b.w()),
        crate::svg::n(b.h())
    )
}

/// The shape's SVG with color `from` (fill or stroke) replaced by `to`.
fn swap(s: &Shape, from: &str, to: &str) -> String {
    s.svg()
        .replace(&format!("\"{from}\""), &format!("\"{to}\""))
}

/// The SVG of the shapes moved by (`dx`, `dy`).
fn moved(v: &[&Shape], dx: f32, dy: f32) -> String {
    format!(
        r#"<g transform="translate({} {})">{}</g>"#,
        crate::svg::n(dx),
        crate::svg::n(dy),
        body(v)
    )
}

/// The ids of the 303's keys by the mockup's key groups: the white keys (left to right),
/// then the black ones.
const KEYS_303: [(usize, &str); 13] = [
    (93, "key0"),
    (94, "key2"),
    (95, "key4"),
    (96, "key5"),
    (97, "key7"),
    (98, "key9"),
    (99, "key11"),
    (100, "key12"),
    (101, "key1"),
    (102, "key3"),
    (103, "key6"),
    (108, "key8"),
    (110, "key10"),
];

/// The 303's flags, by the mockup's groups.
const FLAGS_303: [(usize, &str, &str); 5] = [
    (111, "down", "down_led"),
    (112, "up", "up_led"),
    (113, "accent", "accent_led"),
    (114, "slide", "slide_led"),
    (115, "note_pause", "note_led"),
];

/// The 303's chosen waveform and its LFOs' tempo sync, lit (this fork: light cyan).
const CYAN: &str = "#b0f6ff";
/// The face of a 303 key of the chosen scale (this fork: between green and cyan).
const SCALE_KEY: &str = "#6ee6c4";

/// The 303's knobs, left to right.
const KNOBS_303: [&str; 6] = ["tune", "cutoff", "reso", "envmod", "decay", "accent"];

/// Bass synth `k` (0, 1): its waveform, knobs, LFO rows, key band and step editor.
fn tb303(c: &mut Cuts, k: usize) {
    let m = Mockup::panel_1();
    let g = 2 + k;
    let at = |i: usize| format!("{g}.{i}");
    let sub = |i: usize, j: usize| format!("{g}.{i}.{j}");
    let sec = format!("tb303_{}", k + 1);
    let first = k == 0;

    // the waveform: the saw (chosen, white) and the square (dim)
    let wave = grp(m, &at(67));
    if first {
        let frame = grps(m, &[&sub(67, 0), &sub(67, 1), &sub(67, 2)]);
        let saw = grp(m, &sub(67, 4))[0];
        let square = grp(m, &sub(67, 5))[0];
        let b = bbox(&frame);
        // the chosen one light cyan (this fork's wish over the mockup's white)
        let (white, dim) = ("#fbfbf8", "#68696b");
        let w0 = format!("{}{}{}", body(&frame), swap(saw, white, CYAN), square.svg());
        let w1 = format!(
            "{}{}{}",
            body(&frame),
            swap(saw, white, dim),
            swap(square, dim, CYAN)
        );
        c.parts
            .push(part("switch_wave_0", b.x0, b.y0, b.w(), b.h(), &w0));
        c.parts
            .push(part("switch_wave_1", b.x0, b.y0, b.w(), b.h(), &w1));
    }
    let b = bbox(&wave);
    c.take(&wave);
    c.place(&format!("{sec}.wave"), b.x0, b.y0);

    // the knobs: track, bezel and body; the lower cap turns with the pointer
    for (i, id) in KNOBS_303.iter().enumerate() {
        let gi = 68 + i;
        let v = grp(m, &at(gi));
        let bezel = bbox(&grp(m, &format!("{g}.{gi}.1")));
        let (cx, cy) = (bezel.cx(), bezel.cy());
        if first && i == 0 {
            let kb = grps(m, &[&sub(gi, 0), &sub(gi, 1), &sub(gi, 2)]);
            let kp = grps(m, &[&sub(gi, 3), &sub(gi, 5)]);
            knob(c, "knob_303", &kb, &kp, (cx, cy, 27.0, 27.0));
        }
        c.take(&v);
        c.place(&format!("{sec}.{id}#knob"), cx - 13.5, cy - 13.5);
    }

    // the LFO rows: RATE and AMOUNT knobs and the mode buttons, under CUTOFF .. ACCENT
    for (j, id) in KNOBS_303[1..].iter().enumerate() {
        let (rate, amt, mode) = (74 + 3 * j, 75 + 3 * j, 76 + 3 * j);
        for (gi, ctl) in [(rate, "lfo"), (amt, "amt")] {
            let v = grp(m, &at(gi));
            let body_b = bbox(&grp(m, &sub(gi, 1)));
            let (cx, cy) = (body_b.cx(), body_b.cy());
            if first && j == 0 && ctl == "lfo" {
                let kb = grps(m, &[&sub(gi, 0), &sub(gi, 1)]);
                let kp = grp(m, &sub(gi, 3));
                knob(c, "knob_lfo", &kb, &kp, (cx, cy, 12.0, 12.0));
            }
            c.take(&v);
            c.place(&format!("{sec}.{id}_{ctl}"), cx - 6.0, cy - 6.0);
        }
        let v = grp(m, &at(mode));
        let b = bbox(&v);
        if first && j == 0 {
            // the note lit (tempo sync) or dim; the sine or (from the next row) the random sine
            let frame = grps(m, &[&sub(mode, 0), &sub(mode, 1), &sub(mode, 2)]);
            let note = grp(m, &sub(mode, 3));
            let sine = grp(m, &sub(mode, 4));
            let random = grp(m, &sub(mode + 3, 4));
            let dx = b.x0 - bbox(&grp(m, &at(mode + 3))).x0;
            for sync in [false, true] {
                for hold in [false, true] {
                    let n = if sync {
                        recolor(&note, &[("#3593ff", CYAN)])
                    } else {
                        recolor(&note, &[("#3593ff", "#34557f")])
                    };
                    // the sine light red, as bright as the cyan (this fork's wish over the
                    // mockup's yellow)
                    let w = if hold {
                        moved(&random, dx, 0.0)
                    } else {
                        recolor(&sine, &[("#ffb000", "#ff9a94")])
                    };
                    let name = format!("lfo_mode_{}{}", sync as u8, hold as u8);
                    let svg = format!("{}{n}{w}", body(&frame));
                    c.parts.push(part(&name, b.x0, b.y0, b.w(), b.h(), &svg));
                }
            }
        }
        c.take(&v);
        c.place(&format!("{sec}.{id}_lfo_mode"), b.x0, b.y0);
    }

    // the step LEDs and their numbers
    for i in 0..16 {
        let led_v = grp(m, &format!("{g}.89.{i}"));
        let num = grp(m, &format!("{g}.89.{}", 16 + i));
        let lb = bbox(&led_v);
        let (x, y) = (lb.x0 - 1.0, lb.y0 - 1.0);
        if first {
            if i == 0 {
                for (state, col) in [
                    ("off", "#4a2a28"),
                    ("on", "#e2413b"),
                    ("sel", "#53d86a"),
                    ("play", "#f2c531"),
                ] {
                    let svg = recolor(&led_v, &[("#e2413b", col)]);
                    c.parts
                        .push(part(&format!("stepled_{state}"), x, y, 8.0, 8.0, &svg));
                }
            }
            let n = i + 1;
            c.parts
                .push(part(&format!("stepnum_{n}"), x, y, 8.0, 8.0, &body(&num)));
            let dim = recolor(&num, &[("#1b1c1e", "#9a5c56")]);
            c.parts
                .push(part(&format!("stepnum_{n}_off"), x, y, 8.0, 8.0, &dim));
        }
        c.take(&led_v);
        c.take(&num);
        c.place(&format!("{sec}.stepled{}", i + 1), x, y);
    }

    // RNDM: the GENERATE menu's key; its name green while the menu is open
    let v = grp(m, &at(90));
    let b = bbox(&v);
    if first {
        let rest = body(&v);
        c.parts
            .push(part("btn_rndm_rest", b.x0, b.y0, b.w(), b.h(), &rest));
        let pressed = format!("{rest}{}", veil(&bbox(&grp(m, &sub(90, 1)))));
        c.parts
            .push(part("btn_rndm_pressed", b.x0, b.y0, b.w(), b.h(), &pressed));
        let on = recolor(&v, &[("#1b1c1e", "#2fbf4a")]);
        c.parts
            .push(part("btn_rndm_on", b.x0, b.y0, b.w(), b.h(), &on));
    }
    c.take(&v);
    c.place(&format!("{sec}.generate"), b.x0, b.y0);

    // an LED part 8 x 8 around the mockup's 6 x 6 LED
    let led_at = |v: &[&Shape]| {
        let b = bbox(v);
        (b.cx() - 4.0, b.cy() - 4.0)
    };
    // pitch mode (its LED over it) and clear
    for (gi, name, id) in [
        (91, "btn_pitch", "pitch_button"),
        (92, "btn_clear", "clear"),
    ] {
        let key = grps(m, &[&sub(gi, 0), &sub(gi, 1)]);
        let b = bbox(&key);
        if first {
            let rest = body(&key);
            let pressed = format!("{rest}{}", veil(&bbox(&grp(m, &sub(gi, 0)))));
            c.parts.push(part(
                &format!("{name}_rest"),
                b.x0,
                b.y0,
                b.w(),
                b.h(),
                &rest,
            ));
            c.parts.push(part(
                &format!("{name}_pressed"),
                b.x0,
                b.y0,
                b.w(),
                b.h(),
                &pressed,
            ));
        }
        c.take(&grp(m, &at(gi)));
        c.place(&format!("{sec}.{id}"), b.x0, b.y0);
    }
    let pitch_led = grp(m, &sub(91, 2));
    let (x, y) = led_at(&pitch_led);
    c.place(&format!("{sec}.pitch_led"), x, y);

    // the keys and the flags: the key (border, rim, face) and its LED above it
    let key_parts = |c: &mut Cuts, name: &str, gi: usize, scale: bool| {
        let key = grps(m, &[&sub(gi, 0), &sub(gi, 1)]);
        let b = bbox(&key);
        let rest = body(&key);
        let pressed = format!("{rest}{}", veil(&bbox(&grp(m, &sub(gi, 0)))));
        c.parts.push(part(
            &format!("{name}_rest"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &rest,
        ));
        c.parts.push(part(
            &format!("{name}_pressed"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &pressed,
        ));
        if scale {
            let lit = recolor(&key, &[("#fbfbf8", SCALE_KEY)]);
            c.parts.push(part(
                &format!("{name}_scale"),
                b.x0,
                b.y0,
                b.w(),
                b.h(),
                &lit,
            ));
        }
    };
    if first {
        key_parts(c, "key_white", 94, true);
        key_parts(c, "key_black", 101, true);
        key_parts(c, "btn_flag", 111, false);
        // the LEDs: lit as the first key's, unlit as the second's
        for (gi, name) in [(93, "led_8_on"), (94, "led_8_off")] {
            let v = grp(m, &sub(gi, 2));
            let (x, y) = led_at(&v);
            c.parts.push(part(name, x, y, 8.0, 8.0, &body(&v)));
        }
    }
    let mut keys: Vec<(usize, String, String)> = KEYS_303
        .iter()
        .map(|&(gi, id)| (gi, id.to_string(), format!("{id}_led")))
        .collect();
    keys.extend(
        FLAGS_303
            .iter()
            .map(|&(gi, id, led)| (gi, id.to_string(), led.to_string())),
    );
    for (gi, id, led_id) in keys {
        let key = grps(m, &[&sub(gi, 0), &sub(gi, 1)]);
        let b = bbox(&key);
        c.place(&format!("{sec}.{id}#trigger"), b.x0, b.y0);
        let (x, y) = led_at(&grp(m, &sub(gi, 2)));
        c.place(&format!("{sec}.{led_id}"), x, y);
        c.take(&grp(m, &at(gi)));
    }
    // the mockup's second copy of a black key
    c.take(&grp(m, &at(109)));

    if first {
        c.background(m, "_303-a", "bg_tb303", Bx::new(90.0, 50.0, 366.0, 115.0));
    }
}

/// The shape's SVG with its plain fill `color` at `opacity` (1: opaque).
fn tint(s: &Shape, color: &str, opacity: f32) -> String {
    let hex = s.hex().unwrap_or_default();
    let mut svg = s.svg();
    if let Some(i) = svg.find(" fill-opacity=\"") {
        let end = svg[i + 15..].find('"').unwrap() + i + 16;
        svg.replace_range(i..end, "");
    }
    let to = if opacity < 1.0 {
        format!("fill=\"{color}\" fill-opacity=\"{opacity}\"")
    } else {
        format!("fill=\"{color}\"")
    };
    svg.replace(&format!("fill=\"{hex}\""), &to)
}

/// The center of the shapes' box.
fn center(v: &[&Shape]) -> (f32, f32) {
    let b = bbox(v);
    (b.cx(), b.cy())
}

/// The 808's knobs in the order of the mockup's groups 4.21 ...
const KNOBS_808: [&str; 22] = [
    "ac_level",
    "bd_level",
    "bd_tone",
    "bd_decay",
    "sd_level",
    "sd_tone",
    "sd_snappy",
    "lt_level",
    "lt_tuning",
    "mt_level",
    "mt_tuning",
    "ht_level",
    "ht_tuning",
    "rs_level",
    "cp_level",
    "cb_level",
    "cy_level",
    "cy_tone",
    "cy_decay",
    "oh_level",
    "oh_decay",
    "ch_level",
];

/// The 808's name pills by the mockup's groups: the twelve lower ones (AC .. CH), then the
/// upper ones (LC .. MA).
const PILLS_808: [(usize, &str); 17] = [
    (4, "AC"),
    (5, "BD"),
    (6, "SD"),
    (8, "LT"),
    (10, "MT"),
    (12, "HT"),
    (14, "RS"),
    (16, "CP"),
    (17, "CB"),
    (18, "CY"),
    (19, "OH"),
    (20, "CH"),
    (7, "LC"),
    (9, "MC"),
    (11, "HC"),
    (13, "CL"),
    (15, "MA"),
];

/// A drum knob's body and pointer parts from mockup group `g` (track, body, outline, arc,
/// pointer), 13 x 13.
fn drum_knob(c: &mut Cuts, m: &Mockup, name: &str, g: &str) {
    let kb = grps(
        m,
        &[&format!("{g}.0"), &format!("{g}.1"), &format!("{g}.2")],
    );
    let kp = grp(m, &format!("{g}.4"));
    let (cx, cy) = center(&grp(m, &format!("{g}.1")));
    knob(c, name, &kb, &kp, (cx, cy, 13.0, 13.0));
}

/// Drum machine A, both faces: knobs, the instrument switches, FLIP, the name pills, the steps;
/// on the FLIP face (mockup 2) the pans and the effects keys.
fn tr808(c: &mut Cuts) {
    let (m, m2) = (Mockup::panel_1(), Mockup::panel_2());
    drum_knob(c, m, "knob_808_red", "4.21");
    drum_knob(c, m, "knob_808_white", "4.23");
    for (i, id) in KNOBS_808.iter().enumerate() {
        let g = format!("4.{}", 21 + i);
        let (cx, cy) = center(&grp(m, &format!("{g}.1")));
        c.take(&grp(m, &g));
        c.place(&format!("tr808.{id}"), cx - 6.5, cy - 6.5);
    }
    // the instrument switches: the handle down (1, the mockup's) or up (0)
    for s in 0..5 {
        let g = format!("4.{}", 43 + s);
        let v = grp(m, &g);
        let b = bbox(&v);
        if s == 0 {
            let slot = grp(m, &format!("{g}.0"));
            let handle = grps(m, &[&format!("{g}.1"), &format!("{g}.2")]);
            let dy = (b.y0 + 1.0) - bbox(&handle).y0;
            let down = body(&v);
            let up = format!("{}{}", body(&slot), moved(&handle, 0.0, dy));
            c.parts
                .push(part("switch_808_1", b.x0, b.y0, b.w(), b.h(), &down));
            c.parts
                .push(part("switch_808_0", b.x0, b.y0, b.w(), b.h(), &up));
        }
        c.take(&v);
        c.place(&format!("tr808.slot{}_switch", s + 3), b.x0, b.y0);
    }
    // FLIP: off on the face, on (its LED red) on the FLIP face; the effects keys the same key
    let flip = grp(m, "4.48");
    let b = bbox(&flip);
    c.parts
        .push(part("flip_off", b.x0, b.y0, b.w(), b.h(), &body(&flip)));
    let flip_on = grp(m2, "4.37");
    c.parts
        .push(part("flip_on", b.x0, b.y0, b.w(), b.h(), &body(&flip_on)));
    c.take(&flip);
    c.take(&flip_on);
    c.place("tr808.flip", b.x0, b.y0);
    // the steps: lit and unlit of each color
    for (ci, color) in ["red", "orange", "yellow", "white"].into_iter().enumerate() {
        for (state, g) in [("on", 49 + 4 * ci), ("off", 50 + 4 * ci)] {
            let v = grp(m, &format!("4.{g}"));
            let b = bbox(&v);
            let name = format!("step808_{color}_{state}");
            c.parts
                .push(part(&name, b.x0, b.y0, b.w(), b.h(), &body(&v)));
        }
    }
    for i in 0..16 {
        let v = grp(m, &format!("4.{}", 49 + i));
        let b = bbox(&v);
        c.take(&v);
        c.place(&format!("tr808.step{}", i + 1), b.x0, b.y0);
        let v2 = grp(m2, &format!("4.{}", 50 + i));
        c.take(&v2);
    }
    // the name pills: chosen, the red veil of the mockup's chosen BD over each
    let veil_v = grp(m, "4.5.1");
    let vb = bbox(&veil_v);
    for (g, name) in PILLS_808 {
        let pill = grp(m, &format!("4.{g}.0"));
        let text = grp(m, &format!("4.{g}.{}", if g == 5 { 2 } else { 1 }));
        let b = bbox(&pill);
        let svg = format!(
            "{}{}{}",
            body(&pill),
            moved(&veil_v, b.x0 - vb.x0, b.y0 - vb.y0),
            body(&text)
        );
        c.parts.push(part(
            &format!("pill808_{name}_on"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &svg,
        ));
    }
    c.take(&veil_v);
    c.take(&grp(m2, "4.5.1"));
    // the FLIP face: the pans and the effects keys
    for k in 1..=11 {
        let g = format!("4.{}", 20 + k);
        let (cx, cy) = center(&grp(m2, &format!("{g}.1")));
        c.take(&grp(m2, &g));
        c.place(&format!("tr808f.pan{k}"), cx - 6.5, cy - 6.5);
        let fx = grp(m2, &format!("4.{}", 38 + k));
        let b = bbox(&fx);
        c.take(&fx);
        c.place(&format!("tr808f.fx{k}"), b.x0, b.y0);
    }
    for s in 0..5 {
        c.take(&grp(m2, &format!("4.{}", 32 + s)));
    }
    let face = Bx::new(90.0, 280.0, 366.0, 115.0);
    c.background(m, "_808", "bg_tr808", face);
    c.background(m2, "_808-flipped", "bg_tr808_flip", face);
}

/// The 909's names by the mockup's lower pill groups 5.16 ... (RC first, then BD .. CC).
const NAMES_909: [(usize, &str); 11] = [
    (16, "RC"),
    (17, "BD"),
    (18, "SD"),
    (19, "LT"),
    (20, "MT"),
    (21, "HT"),
    (22, "RS"),
    (23, "CP"),
    (24, "CH"),
    (25, "OH"),
    (26, "CC"),
];

/// Drum machine B, both faces: knobs (placed by the nearest of the mockup's), FLIP, flam, the
/// header and name pills, the steps; on the FLIP face the pans and the effects keys.
fn tr909(c: &mut Cuts) {
    let (m, m2) = (Mockup::panel_1(), Mockup::panel_2());
    drum_knob(c, m, "knob_909", "5.27");
    let mut centers = Vec::new();
    for g in 27..=54 {
        let v = grp(m, &format!("5.{g}"));
        centers.push(center(&grp(m, &format!("5.{g}.1"))));
        c.take(&v);
    }
    c.near.push(("tr909".into(), "knob", centers));
    // FLIP (off on the face, on orange on the FLIP face) and flam (on red, off as FLIP's LED)
    let flip = grp(m, "5.56");
    let b = bbox(&flip);
    c.parts
        .push(part("flip909_off", b.x0, b.y0, b.w(), b.h(), &body(&flip)));
    let flip_on = grp(m2, "5.40");
    c.parts.push(part(
        "flip909_on",
        b.x0,
        b.y0,
        b.w(),
        b.h(),
        &body(&flip_on),
    ));
    c.take(&flip);
    c.take(&flip_on);
    c.place("tr909.flip", b.x0, b.y0);
    let flam = grp(m, "5.55");
    let fb = bbox(&flam);
    let led = grp(m, "5.55.2")[0];
    let key = grps(m, &["5.55.0", "5.55.1"]);
    let off_led = tint(led, "#000000", 0.188);
    // on: its LED green (this fork's wish over the mockup's red)
    let on = format!("{}{}", body(&key), tint(led, "#53d86a", 1.0));
    c.parts
        .push(part("toggle_flam_on", fb.x0, fb.y0, fb.w(), fb.h(), &on));
    let off = format!("{}{off_led}", body(&key));
    c.parts
        .push(part("toggle_flam_off", fb.x0, fb.y0, fb.w(), fb.h(), &off));
    c.take(&flam);
    c.take(&grp(m2, "5.39"));
    c.take(&grp(m2, "5.38"));
    c.place("tr909.flam_button", fb.x0, fb.y0);
    // the effects keys of the FLIP face: on green, off dim
    let fx_on = grp(m2, "5.41");
    let fx_off = grp(m2, "5.42");
    let (bon, boff) = (bbox(&fx_on), bbox(&fx_off));
    c.parts.push(part(
        "fx909_on",
        bon.x0,
        bon.y0,
        bon.w(),
        bon.h(),
        &body(&fx_on),
    ));
    c.parts.push(part(
        "fx909_off",
        boff.x0,
        boff.y0,
        boff.w(),
        boff.h(),
        &body(&fx_off),
    ));
    for k in 1..=11 {
        let g = format!("5.{}", 26 + k);
        let (cx, cy) = center(&grp(m2, &format!("{g}.1")));
        c.take(&grp(m2, &g));
        c.place(&format!("tr909f.pan{k}"), cx - 6.5, cy - 6.5);
        let fx = grp(m2, &format!("5.{}", 40 + k));
        let b = bbox(&fx);
        c.take(&fx);
        c.place(&format!("tr909f.fx{k}"), b.x0, b.y0);
    }
    // the steps: the light (a) and darker (b) keys, unlit, and their LED in each value's
    // color (orange, red, green) taken from a key that shows it
    let led_of = |g: usize| grp(m, &format!("5.{g}.2"));
    let x_of = |g: usize| bbox(&grp(m, &format!("5.{g}"))).x0;
    for (grp_name, base) in [("a", 58usize), ("b", 62usize)] {
        let key = grps(m, &[&format!("5.{base}.0"), &format!("5.{base}.1")]);
        let b = bbox(&key);
        let x0 = x_of(base);
        for (value, src) in [(0usize, base), (1, 57), (2, 61), (3, 67)] {
            let l = moved(&led_of(src), x0 - x_of(src), 0.0);
            let name = format!("step909_{grp_name}_{value}");
            let svg = format!("{}{l}", body(&key));
            c.parts.push(part(&name, b.x0, b.y0, b.w(), b.h(), &svg));
        }
    }
    for i in 0..16 {
        let v = grp(m, &format!("5.{}", 57 + i));
        let b = bbox(&v);
        c.take(&v);
        c.place(&format!("tr909.step{}", i + 1), b.x0, b.y0);
        c.take(&grp(m2, &format!("5.{}", 52 + i)));
    }
    // the header pill of the accent, chosen: orange with dark letters
    let head = grp(m, "5.4");
    let hb = bbox(&head);
    let on = recolor(&grp(m, "5.4.0"), &[("#4f5050", "#f08a24")])
        + &recolor(&grp(m, "5.4.1"), &[("#f08a24", "#1b1c1e")]);
    c.parts
        .push(part("head909_AC_on", hb.x0, hb.y0, hb.w(), hb.h(), &on));
    // the name pills, chosen: the orange veil (the mockup's BD has it)
    for (g, name) in NAMES_909 {
        let pill = grp(m, &format!("5.{g}.0"));
        let text = grp(m, &format!("5.{g}.{}", if g == 17 { 2 } else { 1 }));
        let b = bbox(&pill);
        let veil_svg = format!("{}{}", tint(pill[0], "#f08a24", 0.749), pill[1].svg());
        let svg = format!("{}{veil_svg}{}", body(&pill), body(&text));
        c.parts.push(part(
            &format!("name909_{name}_on"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &svg,
        ));
    }
    c.take(&grp(m, "5.17.1"));
    c.take(&grp(m2, "5.17.1"));
    let face = Bx::new(90.0, 395.0, 366.0, 115.0);
    c.background(m, "_909", "bg_tr909", face);
    c.background(m2, "_909-flipped", "bg_tr909_flip", face);
}

/// A display's top-left corner so that its first digit's cell is the mockup's: `seg` the
/// first digit's segments, `w` the display's width, `n` its digits (the app draws the digits
/// right-aligned in it).
fn display_at(seg: &[&Shape], w: f32, n: usize) -> (f32, f32) {
    let b = bbox(seg);
    (b.x0 - 1.2 - (w - 9.0 * n as f32), b.y0 - 1.5)
}

/// A knob_22-like knob (track, body, an optional arc, pointer) of group `g`: its center (the
/// body's) and, when `name` is given, its parts.
fn knob22(c: &mut Cuts, m: &Mockup, g: &str, name: Option<&str>) -> (f32, f32) {
    let v = grp(m, g);
    let (cx, cy) = center(&grp(m, &format!("{g}.1")));
    if let Some(name) = name {
        // the pointer is the last child; the arc (amber) is drawn by the app
        let last = v.last().unwrap().chain.clone();
        let ptr: Vec<&Shape> = v.iter().copied().filter(|s| s.chain == last).collect();
        let kb: Vec<&Shape> = v
            .iter()
            .copied()
            .filter(|s| {
                s.chain != last
                    && s.stroke
                        .map(|c| c[..3] != [0xff, 0xb0, 0x00])
                        .unwrap_or(true)
            })
            .collect();
        knob(c, name, &kb, &ptr, (cx, cy, 22.0, 22.0));
    }
    c.take(&v);
    (cx, cy)
}

/// A meter's frames `{name}_0` .. `_10` from its well (`None`: no well) and its ten segments,
/// bottom first: `n` lit in the colors of the generated parts, the others unlit as the
/// mockup's.
fn meter(c: &mut Cuts, name: &str, b: Bx, well: Option<&[&Shape]>, segs: &[&Shape]) {
    const LIT: [&str; 10] = [
        "#53d86a", "#53d86a", "#53d86a", "#53d86a", "#53d86a", "#53d86a", "#ffb000", "#ffb000",
        "#e2413b", "#e2413b",
    ];
    for n in 0..=10 {
        let mut svg = well.map(body).unwrap_or_default();
        for (i, s) in segs.iter().enumerate() {
            svg.push_str(&if i < n {
                tint(s, LIT[i], 1.0)
            } else {
                tint(s, "#ffffff", 0.0784)
            });
        }
        c.parts
            .push(part(&format!("{name}_{n}"), b.x0, b.y0, b.w(), b.h(), &svg));
    }
}

/// The segments of a meter group `g`, bottom first (the mockup draws them bottom first).
fn segments<'a>(m: &'a Mockup, g: &str, from: usize) -> Vec<&'a Shape> {
    (from..from + 10)
        .map(|i| grp(m, &format!("{g}.{i}"))[0])
        .collect()
}

/// A fader at mockup groups `track` and `handle`: its place (the track's box 3 px above the
/// ticks and 0.5 px left of them, as the generated track).
fn fader(c: &mut Cuts, m: &Mockup, id: &str, track: &str, handle: &str, parts: bool) {
    let t = grp(m, track);
    let h = grp(m, handle);
    let tb = bbox(&t);
    let (x, y) = (tb.x0 - 0.5, tb.y0 - 3.0);
    if parts {
        c.parts
            .push(part("fader_track", x, y, 15.0, 54.0, &body(&t)));
        let hb = bbox(&h);
        c.parts
            .push(part("fader_handle", x, hb.y0, 15.0, 11.0, &body(&h)));
    }
    c.take(&t);
    c.take(&h);
    c.place(id, x, y);
}

/// The mixer channels, the master and the four effects.
fn mixer_and_effects(c: &mut Cuts) {
    let m = Mockup::panel_1();
    // the LEDs: on as channel 1's, off as channel 2's
    led(
        c,
        "power_14x14_on",
        &grp(m, "10.4"),
        grp(m, "10.4")[1],
        14.0,
        14.0,
    );
    led(
        c,
        "power_14x14_off",
        &grp(m, "11.4"),
        grp(m, "11.4")[1],
        14.0,
        14.0,
    );
    led(
        c,
        "solo_14x14_on",
        &grp(m, "10.5"),
        grp(m, "10.5")[1],
        14.0,
        14.0,
    );
    led(
        c,
        "solo_14x14_off",
        &grp(m, "11.5"),
        grp(m, "11.5")[1],
        14.0,
        14.0,
    );
    // the routing keys: off as channel 1's; on with channel 2's lit PCF LED and a light label
    let lit_led = grp(m, "11.15.2");
    let lit_b = bbox(&grp(m, "11.15"));
    for (g, name) in [("10.14", "dist"), ("10.15", "pcf"), ("10.16", "comp")] {
        let v = grp(m, g);
        let b = bbox(&v);
        let base = grps(m, &[&format!("{g}.0"), &format!("{g}.1")]);
        let label = grp(m, &format!("{g}.3"));
        c.parts.push(part(
            &format!("mixsw_{name}_off"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &body(&v),
        ));
        let on = format!(
            "{}{}{}",
            body(&base),
            moved(&lit_led, b.x0 - lit_b.x0, b.y0 - lit_b.y0),
            recolor(&label, &[("#9aa7b8", "#e9edf2")])
        );
        c.parts.push(part(
            &format!("mixsw_{name}_on"),
            b.x0,
            b.y0,
            b.w(),
            b.h(),
            &on,
        ));
    }
    // the header meter: 8 wide, its segments in the well's frame
    // (the mockup draws the segments bottom first)
    let segs = segments(m, "10.9", 0);
    let hb = bbox(&segs);
    let head_box = Bx::new(hb.x0, hb.y0, 8.0, hb.h().ceil());
    meter(c, "meter_8x12", head_box, None, &segs);
    for ch in 0..4 {
        let g = 10 + ch;
        let at = |i: usize| format!("{g}.{i}");
        let sec = format!("mixer{}", ch + 1);
        for (i, id) in [(4, "activity"), (5, "solo")] {
            let v = grp(m, &at(i));
            let (cx, cy) = center(&[v[1]]);
            c.take(&v);
            c.place(&format!("{sec}.{id}"), cx - 7.0, cy - 7.0);
        }
        let mv = grp(m, &at(9));
        c.take(&mv);
        let mb = bbox(&mv);
        c.place(&format!("{sec}.meter"), mb.x0, mb.y0);
        let first = ch == 0;
        let (px, py) = knob22(c, m, &at(10), None);
        c.place(&format!("{sec}.pan"), px - 11.0, py - 11.0);
        let (dx, dy) = knob22(c, m, &at(11), None);
        c.place(&format!("{sec}.delay"), dx - 11.0, dy - 11.0);
        fader(c, m, &format!("{sec}.level"), &at(12), &at(13), first);
        for (i, id) in [(14, "dist"), (15, "pcf"), (16, "comp")] {
            let v = grp(m, &at(i));
            let b = bbox(&v);
            c.take(&v);
            c.place(&format!("{sec}.{id}"), b.x0, b.y0);
        }
    }
    c.background(
        m,
        "mix-303-a",
        "bg_mixer",
        Bx::new(456.0, 50.0, 70.0, 115.0),
    );

    // the master: its meters (well and segments), fader and compressor key
    let ml = grp(m, "14.14");
    let mlb = bbox(&ml);
    let well = grp(m, "14.14.0");
    meter(c, "meter_15x56", mlb, Some(&well), &segments(m, "14.14", 1));
    for (g, id) in [("14.14", "meter_l"), ("14.15", "meter_r")] {
        let v = grp(m, g);
        let b = bbox(&v);
        c.take(&v);
        c.place(&format!("master.{id}"), b.x0, b.y0);
    }
    fader(c, m, "master.fader", "14.16", "14.17", false);
    let v = grp(m, "14.18");
    let b = bbox(&v);
    c.take(&v);
    c.place("master.comp", b.x0, b.y0);
    c.background(m, "master", "bg_master", Bx::new(526.0, 50.0, 82.0, 98.0));

    // the effects: the header LED and meter, then each one's controls
    let header = |c: &mut Cuts, fx: &str, ledg: &str, meterg: &str| {
        let v = grp(m, ledg);
        let (cx, cy) = center(&[v[1]]);
        c.take(&v);
        c.place(&format!("{fx}.on"), cx - 7.0, cy - 7.0);
        let mv = grp(m, meterg);
        let mb = bbox(&mv);
        c.take(&mv);
        c.place(&format!("{fx}.meter"), mb.x0, mb.y0);
    };
    let arrows = |c: &mut Cuts, fx: &str, g: &str, id: &str| {
        for (k, dir) in [(1, "up"), (3, "down")] {
            let key = grp(m, &format!("{g}.{k}"));
            let b = bbox(&key);
            c.take(&key);
            c.take(&grp(m, &format!("{g}.{}", k + 1)));
            c.place(&format!("{fx}.{id}_{dir}"), b.x0, b.y0);
        }
    };
    let digits_of = |g: &str, n: usize| -> Vec<&Shape> {
        (0..7 * n)
            .map(|i| grp(m, &format!("{g}.2.{i}"))[0])
            .collect()
    };
    // a vertical switch: the handle up (0, the delay's) or down (1, the PCF's)
    let sw = grp(m, "15.15");
    let swb = bbox(&sw);
    let slot = grp(m, "15.15.0");
    let handle = grp(m, "15.15.1");
    let down = bbox(&grp(m, "17.17.1")).y0 - bbox(&grp(m, "17.17")).y0;
    let up = bbox(&handle).y0 - swb.y0;
    c.parts.push(part(
        "switch_fx_0",
        swb.x0,
        swb.y0,
        swb.w(),
        swb.h(),
        &body(&sw),
    ));
    let sw1 = format!("{}{}", body(&slot), moved(&handle, 0.0, down - up));
    c.parts
        .push(part("switch_fx_1", swb.x0, swb.y0, swb.w(), swb.h(), &sw1));

    // delay
    header(c, "delay", "15.12", "15.13");
    let d = digits_of("15.5", 2);
    c.take(&d);
    let (x, y) = display_at(&d[..7], 19.0, 2);
    c.place("delay.steps", x, y);
    arrows(c, "delay", "15.14", "steps");
    c.take(&sw);
    c.place("delay.triplet", swb.x0, swb.y0);
    let (px, py) = knob22(c, m, "15.16", Some("knob_fx"));
    c.place("delay.pan", px - 11.0, py - 11.0);
    let (fx, fy) = knob22(c, m, "15.17", None);
    c.place("delay.feedback", fx - 11.0, fy - 11.0);
    c.background(m, "delay", "bg_delay", Bx::new(526.0, 148.0, 82.0, 93.0));
    // dist
    header(c, "dist", "16.7", "16.8");
    for (g, id) in [("16.9", "amount"), ("16.10", "shape")] {
        let (x, y) = knob22(c, m, g, None);
        c.place(&format!("dist.{id}"), x - 11.0, y - 11.0);
    }
    c.background(m, "dist", "bg_dist", Bx::new(526.0, 241.0, 82.0, 65.0));
    // pcf
    header(c, "pcf", "17.14", "17.15");
    let d = digits_of("17.5", 2);
    c.take(&d);
    let (x, y) = display_at(&d[..7], 18.0, 2);
    c.place("pcf.pattern", x, y);
    arrows(c, "pcf", "17.16", "pattern");
    let v = grp(m, "17.17");
    let b = bbox(&v);
    c.take(&v);
    c.place("pcf.mode", b.x0, b.y0);
    for (i, id) in ["freq", "q", "amt", "dec"].into_iter().enumerate() {
        let t = format!("17.{}", 18 + 2 * i);
        let h = format!("17.{}", 19 + 2 * i);
        fader(c, m, &format!("pcf.{id}"), &t, &h, false);
    }
    c.background(m, "pcf", "bg_pcf", Bx::new(526.0, 306.0, 82.0, 111.0));
    // comp: the gain-reduction LEDs (well and light)
    header(c, "comp", "18.12", "18.13");
    for i in 0..9 {
        let well = grp(m, &format!("18.14.{}", 2 * i));
        let light = grp(m, &format!("18.14.{}", 2 * i + 1));
        let wb = bbox(&well);
        let (x, y) = (wb.x0 - 0.5, wb.y0 - 0.5);
        if i == 0 {
            for (state, col) in [
                ("off", None),
                ("green", Some("#53d86a")),
                ("amber", Some("#ffb000")),
                ("red", Some("#e2413b")),
            ] {
                let l = match col {
                    None => body(&light),
                    Some(col) => tint(light[0], col, 1.0),
                };
                let svg = format!("{}{l}", body(&well));
                c.parts
                    .push(part(&format!("gr_led_{state}"), x, y, 7.0, 10.0, &svg));
            }
        }
        c.take(&well);
        c.take(&light);
        c.place(&format!("comp.gr{}", i + 1), x, y);
    }
    for (g, id) in [("18.15", "ratio"), ("18.16", "threshold")] {
        let (x, y) = knob22(c, m, g, None);
        c.place(&format!("comp.{id}"), x - 11.0, y - 11.0);
    }
    c.background(m, "comp", "bg_comp", Bx::new(526.0, 417.0, 82.0, 93.0));
}
