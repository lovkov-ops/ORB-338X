//! The panel's labels (this fork): Inter SemiBold (`assets/fonts`, SIL Open Font License 1.1,
//! see `assets/fonts/Inter-LICENSE.txt`), its glyphs' outlines written into the art as filled
//! paths, so the art needs no font at run time and renders the same everywhere (usvg has no
//! guaranteed system fonts). A text's size is given by its cap height; glyphs are placed by
//! their advance widths with a little tracking (no kerning).

use std::sync::OnceLock;

use ttf_parser::{Face, OutlineBuilder};

use crate::svg::{Svg, n};

const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");

/// The space added between two glyphs, in ems: small capitals read better a little apart.
const TRACKING: f32 = 0.04;

fn face() -> &'static Face<'static> {
    static FACE: OnceLock<Face<'static>> = OnceLock::new();
    FACE.get_or_init(|| Face::parse(INTER, 0).expect("Inter parses"))
}

/// Pixels per font unit at cap height `cap`.
fn scale(cap: f32) -> f32 {
    let f = face();
    cap / f.capital_height().unwrap_or(1490) as f32
}

/// The em in pixels at cap height `cap`.
fn em(cap: f32) -> f32 {
    face().units_per_em() as f32 * scale(cap)
}

/// A character's glyph and its advance in font units.
fn glyph(c: char) -> (ttf_parser::GlyphId, f32) {
    let f = face();
    let id = f
        .glyph_index(c)
        .unwrap_or_else(|| panic!("no glyph for {c:?}"));
    (id, f.glyph_hor_advance(id).unwrap_or(0) as f32)
}

#[derive(Clone, Copy, PartialEq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// The stroke width in pixels at cap height `cap`: none, the glyphs are filled outlines (the
/// hand-drawn stroke font this replaced had one).
pub fn stroke(_cap: f32) -> f32 {
    0.0
}

/// The space between two glyphs in pixels at cap height `cap` (the tracking).
pub fn gap(cap: f32) -> f32 {
    TRACKING * em(cap)
}

/// The width of `text` in pixels at cap height `cap`: the glyphs' advances and the gaps between
/// them.
pub fn text_width(cap: f32, text: &str) -> f32 {
    let advances: f32 = text.chars().map(|c| glyph(c).1).sum();
    let gaps = text.chars().count().saturating_sub(1) as f32;
    advances * scale(cap) + gaps * gap(cap)
}

/// Draws `text` with cap height `cap`, its vertical center at `yc`.
pub fn text(s: &mut Svg, x: f32, yc: f32, cap: f32, anchor: Anchor, color: &str, text: &str) {
    text_range(s, x, yc, cap, anchor, color, text, 0..text.chars().count());
}

/// Writes a glyph's outline as SVG path commands, scaled and placed (font units are y-up).
struct Pen<'a> {
    d: &'a mut String,
    x: f32,
    base: f32,
    k: f32,
}

impl Pen<'_> {
    fn pt(&self, x: f32, y: f32) -> String {
        format!("{} {}", n(self.x + x * self.k), n(self.base - y * self.k))
    }
}

impl OutlineBuilder for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.d.push_str(&format!("M{p}"));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.d.push_str(&format!("L{p}"));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = (self.pt(x1, y1), self.pt(x, y));
        self.d.push_str(&format!("Q{a} {b}"));
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b, c) = (self.pt(x1, y1), self.pt(x2, y2), self.pt(x, y));
        self.d.push_str(&format!("C{a} {b} {c}"));
    }
    fn close(&mut self) {
        self.d.push('Z');
    }
}

/// Draws only the glyphs `range` (character indices) of `text`, laid out exactly as `text`
/// lays them out: the pieces of one run drawn apart add up to the run, pixel for pixel.
#[allow(clippy::too_many_arguments)]
pub fn text_range(
    s: &mut Svg,
    x: f32,
    yc: f32,
    cap: f32,
    anchor: Anchor,
    color: &str,
    text: &str,
    range: std::ops::Range<usize>,
) {
    let w = text_width(cap, text);
    let x0 = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - w / 2.0,
        Anchor::End => x - w,
    };
    let k = scale(cap);
    let mut d = String::new();
    let mut dx = 0.0;
    for (i, c) in text.chars().enumerate() {
        let (id, adv) = glyph(c);
        if range.contains(&i) {
            let mut pen = Pen {
                d: &mut d,
                x: x0 + dx,
                base: yc + cap / 2.0,
                k,
            };
            face().outline_glyph(id, &mut pen);
        }
        dx += adv * k + gap(cap);
    }
    if !d.is_empty() {
        s.raw(&format!(r#"<path d="{d}" fill="{color}"/>"#));
    }
}

/// Centered text.
pub fn label(s: &mut Svg, x: f32, yc: f32, cap: f32, color: &str, t: &str) {
    text(s, x, yc, cap, Anchor::Middle, color, t);
}
