//! The panel's mockup (this fork): `assets/mockup/panel_1.svg` (the panel as it plays) and
//! `panel_2.svg` (the drum machines' FLIP faces), the user's design, drawn in panel pixels
//! (624 x 510). The parts of the art set are cut out of it: [`Mockup::shapes`] are its drawn
//! elements, each with its box in panel pixels, and [`Shape::svg`] writes one element back as
//! SVG, in panel pixels, so that a part is the shapes inside its box under a `viewBox` of that
//! box.

use std::fmt::Write;
use std::sync::Arc;

use resvg::usvg::{self, Node, Paint, Transform};

/// The two mockups, as committed.
pub const PANEL_1: &str = include_str!("../../../assets/mockup/panel_1.svg");
pub const PANEL_2: &str = include_str!("../../../assets/mockup/panel_2.svg");
/// The app icon (this fork), 512 x 512.
pub const ICON: &str = include_str!("../../../assets/mockup/icon.svg");
/// The 303's dark panels left and right of its keys (this fork), 34 x 58 and 112 x 58.
pub const TB303_LEFT: &str = include_str!("../../../assets/mockup/303_left.svg");
pub const TB303_RIGHT: &str = include_str!("../../../assets/mockup/303_right.svg");
/// The GENERATE menu (this fork), 143 x 100.
pub const RANDOMIZER: &str = include_str!("../../../assets/mockup/randomizer.svg");
/// The top bar with the ORB-338X logo and its icon (this fork), 592 x 50: the transport
/// section's coordinates.
pub const TOP_BAR: &str = include_str!("../../../assets/mockup/top_bar.svg");
/// The user's 303 (this fork), 366 x 115: the SUB knob's label is cut from it
/// (`cuts::tb303`).
pub const TB303: &str = include_str!("../../../assets/mockup/303.svg");
/// The user's panel with the effects rack (this fork), 624 x 510: the master's LIMIT key and
/// AMT knob are cut from it (`cuts::mixer_and_effects`).
pub const PANEL_FX: &str = include_str!("../../../assets/mockup/panel_fx.svg");
/// (this fork) The user's FLIP faces of the 808 and the 909 (`SVG FILES/808 flip.svg`,
/// `909 flip.svg`), moved to their places on the panel (the face group translated, the view
/// the panel's).
pub const FLIP_808: &str = include_str!("../../../assets/mockup/flip_808.svg");
pub const FLIP_909: &str = include_str!("../../../assets/mockup/flip_909.svg");

/// A box in panel pixels: left, top, right, bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bx {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Bx {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Bx {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        }
    }

    pub fn w(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn h(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn cx(&self) -> f32 {
        (self.x0 + self.x1) / 2.0
    }

    pub fn cy(&self) -> f32 {
        (self.y0 + self.y1) / 2.0
    }

    /// Whether `o` lies inside this box, `tol` pixels of slack on each side.
    pub fn holds(&self, o: &Bx, tol: f32) -> bool {
        o.x0 >= self.x0 - tol
            && o.y0 >= self.y0 - tol
            && o.x1 <= self.x1 + tol
            && o.y1 <= self.y1 + tol
    }

    /// Whether the center of `o` lies inside this box.
    pub fn centers(&self, o: &Bx) -> bool {
        let (x, y) = (o.cx(), o.cy());
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }

    pub fn union(&self, o: &Bx) -> Bx {
        Bx {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
}

/// One drawn element of a mockup.
#[derive(Clone)]
pub struct Shape {
    /// The mockup it is in (1, 2).
    pub doc: u8,
    /// Its order in the document (the paint order).
    pub index: usize,
    /// The id of the top-level group it is in (`_303-a`, `mix-808`, ...).
    pub sec: String,
    /// The child indexes from the root group to it.
    pub chain: Vec<usize>,
    /// Its box with its stroke, in panel pixels.
    pub bbox: Bx,
    /// The angle in degrees of the rotation of its transform (a knob's pointer), 0 if none.
    pub rot: f32,
    /// Its fill color when it is a plain color: r, g, b, alpha 0..255 (the fill's opacity).
    pub color: Option<[u8; 4]>,
    /// Its stroke color when it is a plain color.
    pub stroke: Option<[u8; 4]>,
    /// The element, in panel pixels (its own transform), and the gradients it uses.
    body: String,
    defs: String,
}

impl Shape {
    /// The element as SVG (gradients inline, in a `<defs>` before it).
    pub fn svg(&self) -> String {
        if self.defs.is_empty() {
            self.body.clone()
        } else {
            format!("<defs>{}</defs>{}", self.defs, self.body)
        }
    }

    /// The element as SVG with every plain color `from` (fill or stroke) replaced by `to`
    /// (hex, `#rrggbb`).
    pub fn recolored(&self, from: &str, to: &str) -> String {
        self.svg().replace(from, to)
    }

    /// The element as SVG with an extra transform applied around it.
    pub fn transformed(&self, t: &str) -> String {
        format!(r#"<g transform="{t}">{}</g>"#, self.svg())
    }

    /// Its fill as `#rrggbb`, if a plain color.
    pub fn hex(&self) -> Option<String> {
        self.color
            .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
    }
}

/// A parsed mockup.
pub struct Mockup {
    pub shapes: Vec<Shape>,
}

impl Mockup {
    /// Parses mockup `svg`; `prefix` starts the ids of its gradients.
    pub fn parse(svg: &str, prefix: &str) -> Mockup {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).expect("mockup parses");
        let mut w = Walker {
            shapes: Vec::new(),
            grad: 0,
            prefix: prefix.to_string(),
            doc: match prefix {
                "mb" => 2,
                "m8" => 3,
                "m9" => 4,
                _ => 1,
            },
        };
        let root = tree.root();
        for (i, n) in root.children().iter().enumerate() {
            let sec = n.id().to_string();
            w.node(n, &sec, vec![i], 1.0);
        }
        Mockup { shapes: w.shapes }
    }

    pub fn panel_1() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(PANEL_1, "ma"))
    }

    pub fn panel_2() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(PANEL_2, "mb"))
    }

    /// The shapes of section `sec` (a top-level group id).
    pub fn icon() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(ICON, "mi"))
    }

    pub fn randomizer() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(RANDOMIZER, "mr"))
    }

    pub fn top_bar() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(TOP_BAR, "mt"))
    }

    pub fn tb303() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(TB303, "m3"))
    }

    pub fn flip_808() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(FLIP_808, "m8"))
    }

    pub fn flip_909() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(FLIP_909, "m9"))
    }

    pub fn panel_fx() -> &'static Mockup {
        static M: std::sync::OnceLock<Mockup> = std::sync::OnceLock::new();
        M.get_or_init(|| Mockup::parse(PANEL_FX, "mf"))
    }

    pub fn section<'a>(&'a self, sec: &'a str) -> impl Iterator<Item = &'a Shape> + 'a {
        self.shapes.iter().filter(move |s| s.sec == sec)
    }

    /// The shapes whose box lies inside `b` (`tol` pixels of slack).
    pub fn inside(&self, b: &Bx, tol: f32) -> Vec<&Shape> {
        self.shapes
            .iter()
            .filter(|s| b.holds(&s.bbox, tol))
            .collect()
    }
}

struct Walker {
    shapes: Vec<Shape>,
    grad: usize,
    /// The prefix of the gradient ids (unique across the two mockups).
    prefix: String,
    doc: u8,
}

impl Walker {
    fn node(&mut self, n: &Node, sec: &str, chain: Vec<usize>, opacity: f32) {
        match n {
            Node::Group(g) => {
                let op = opacity * g.opacity().get();
                for (i, c) in g.children().iter().enumerate() {
                    let mut ch = chain.clone();
                    ch.push(i);
                    self.node(c, sec, ch, op);
                }
            }
            Node::Path(p) => {
                if !p.is_visible() {
                    return;
                }
                let t = p.abs_transform();
                let mut defs = String::new();
                let mut body = String::from("<path");
                let _ = write!(body, r#" d="{}""#, path_data(p.data()));
                if t != Transform::identity() {
                    let _ = write!(body, r#" transform="{}""#, matrix(t));
                }
                let mut color = None;
                match p.fill() {
                    Some(f) => {
                        let (paint, c) = self.paint(f.paint(), &mut defs);
                        let _ = write!(body, r#" fill="{paint}""#);
                        let o = f.opacity().get();
                        if o < 1.0 {
                            let _ = write!(body, r#" fill-opacity="{}""#, num(o));
                        }
                        if f.rule() == usvg::FillRule::EvenOdd {
                            body.push_str(r#" fill-rule="evenodd""#);
                        }
                        color = c.map(|c| [c[0], c[1], c[2], (o * 255.0).round() as u8]);
                    }
                    None => body.push_str(r#" fill="none""#),
                }
                let mut stroke = None;
                if let Some(s) = p.stroke() {
                    let (paint, c) = self.paint(s.paint(), &mut defs);
                    let o = s.opacity().get();
                    stroke = c.map(|c| [c[0], c[1], c[2], (o * 255.0).round() as u8]);
                    let _ = write!(
                        body,
                        r#" stroke="{paint}" stroke-width="{}""#,
                        num(s.width().get())
                    );
                    if o < 1.0 {
                        let _ = write!(body, r#" stroke-opacity="{}""#, num(o));
                    }
                    let cap = match s.linecap() {
                        usvg::LineCap::Butt => "butt",
                        usvg::LineCap::Round => "round",
                        usvg::LineCap::Square => "square",
                    };
                    let join = match s.linejoin() {
                        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => "miter",
                        usvg::LineJoin::Round => "round",
                        usvg::LineJoin::Bevel => "bevel",
                    };
                    let _ = write!(
                        body,
                        " stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" \
                         stroke-miterlimit=\"{}\"",
                        num(s.miterlimit().get())
                    );
                    if let Some(d) = s.dasharray() {
                        let d: Vec<String> = d.iter().map(|v| num(*v)).collect();
                        let _ = write!(body, r#" stroke-dasharray="{}""#, d.join(" "));
                    }
                }
                if p.paint_order() == usvg::PaintOrder::StrokeAndFill {
                    body.push_str(r#" paint-order="stroke""#);
                }
                if opacity < 1.0 {
                    let _ = write!(body, r#" opacity="{}""#, num(opacity));
                }
                body.push_str("/>");
                let b = p.abs_stroke_bounding_box();
                let index = self.shapes.len();
                self.shapes.push(Shape {
                    doc: self.doc,
                    index,
                    sec: sec.to_string(),
                    chain,
                    bbox: Bx {
                        x0: b.left(),
                        y0: b.top(),
                        x1: b.right(),
                        y1: b.bottom(),
                    },
                    rot: t.ky.atan2(t.sx).to_degrees(),
                    color,
                    stroke,
                    body,
                    defs,
                });
            }
            Node::Image(im) => {
                let t = im.abs_transform();
                let size = im.size();
                let data = match im.kind() {
                    usvg::ImageKind::PNG(d) => ("image/png", Arc::clone(d)),
                    usvg::ImageKind::JPEG(d) => ("image/jpeg", Arc::clone(d)),
                    _ => return,
                };
                let mut body = String::new();
                let _ = write!(
                    body,
                    r#"<image width="{}" height="{}" transform="{}" href="data:{};base64,{}""#,
                    num(size.width()),
                    num(size.height()),
                    matrix(t),
                    data.0,
                    base64(&data.1)
                );
                if opacity < 1.0 {
                    let _ = write!(body, r#" opacity="{}""#, num(opacity));
                }
                body.push_str("/>");
                let b = im.abs_bounding_box();
                let index = self.shapes.len();
                self.shapes.push(Shape {
                    doc: self.doc,
                    index,
                    sec: sec.to_string(),
                    chain,
                    bbox: Bx {
                        x0: b.left(),
                        y0: b.top(),
                        x1: b.right(),
                        y1: b.bottom(),
                    },
                    rot: 0.0,
                    color: None,
                    stroke: None,
                    body,
                    defs: String::new(),
                });
            }
            Node::Text(_) => {}
        }
    }

    /// The paint as an SVG paint value (a gradient written into `defs`), and its color when
    /// plain.
    fn paint(&mut self, p: &Paint, defs: &mut String) -> (String, Option<[u8; 3]>) {
        match p {
            Paint::Color(c) => (hex(c), Some([c.red, c.green, c.blue])),
            Paint::LinearGradient(g) => {
                self.grad += 1;
                let id = format!("{}{}", self.prefix, self.grad);
                let _ = write!(
                    defs,
                    "<linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" \
                     y1=\"{}\" x2=\"{}\" y2=\"{}\" gradientTransform=\"{}\"{}>{}</linearGradient>",
                    num(g.x1()),
                    num(g.y1()),
                    num(g.x2()),
                    num(g.y2()),
                    matrix(g.transform()),
                    spread(g.spread_method()),
                    stops(g.stops())
                );
                (format!("url(#{id})"), None)
            }
            Paint::RadialGradient(g) => {
                self.grad += 1;
                let id = format!("{}{}", self.prefix, self.grad);
                let _ = write!(
                    defs,
                    "<radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"{}\" \
                     cy=\"{}\" r=\"{}\" fx=\"{}\" fy=\"{}\" gradientTransform=\"{}\"{}>{}\
                     </radialGradient>",
                    num(g.cx()),
                    num(g.cy()),
                    num(g.r().get()),
                    num(g.fx()),
                    num(g.fy()),
                    matrix(g.transform()),
                    spread(g.spread_method()),
                    stops(g.stops())
                );
                (format!("url(#{id})"), None)
            }
            Paint::Pattern(_) => ("none".into(), None),
        }
    }
}

fn hex(c: &usvg::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue)
}

fn spread(s: usvg::SpreadMethod) -> &'static str {
    match s {
        usvg::SpreadMethod::Pad => "",
        usvg::SpreadMethod::Reflect => r#" spreadMethod="reflect""#,
        usvg::SpreadMethod::Repeat => r#" spreadMethod="repeat""#,
    }
}

fn stops(s: &[usvg::Stop]) -> String {
    let mut out = String::new();
    for st in s {
        let _ = write!(
            out,
            r#"<stop offset="{}" stop-color="{}""#,
            num(st.offset().get()),
            hex(&st.color())
        );
        if st.opacity().get() < 1.0 {
            let _ = write!(out, r#" stop-opacity="{}""#, num(st.opacity().get()));
        }
        out.push_str("/>");
    }
    out
}

/// A number with at most four decimals and no trailing zeros.
fn num(v: f32) -> String {
    let mut s = format!("{v:.4}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

fn matrix(t: Transform) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        num(t.sx),
        num(t.ky),
        num(t.kx),
        num(t.sy),
        num(t.tx),
        num(t.ty)
    )
}

fn path_data(p: &usvg::tiny_skia_path::Path) -> String {
    use usvg::tiny_skia_path::PathSegment as S;
    let mut d = String::new();
    for seg in p.segments() {
        match seg {
            S::MoveTo(a) => {
                let _ = write!(d, "M{} {}", num(a.x), num(a.y));
            }
            S::LineTo(a) => {
                let _ = write!(d, "L{} {}", num(a.x), num(a.y));
            }
            S::QuadTo(a, b) => {
                let _ = write!(d, "Q{} {} {} {}", num(a.x), num(a.y), num(b.x), num(b.y));
            }
            S::CubicTo(a, b, c) => {
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    num(a.x),
                    num(a.y),
                    num(b.x),
                    num(b.y),
                    num(c.x),
                    num(c.y)
                );
            }
            S::Close => d.push('Z'),
        }
    }
    d
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let v = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= c.len() {
                out.push(T[(v >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// An SVG document of the shapes, cut to box `b` (its size the box's, rounded up to whole
/// pixels, the box's top-left corner at the origin).
pub fn cut<'a>(b: &Bx, shapes: impl IntoIterator<Item = &'a Shape>) -> String {
    let mut s = String::new();
    for sh in shapes {
        s.push_str(&sh.svg());
    }
    document(b, &s)
}

/// An SVG document of `body` (panel pixels), cut to box `b`.
pub fn document(b: &Bx, body: &str) -> String {
    let (w, h) = (b.w().ceil(), b.h().ceil());
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
         viewBox=\"{} {} {} {}\">{body}</svg>",
        num(w),
        num(h),
        num(b.x0),
        num(b.y0),
        num(w),
        num(h)
    )
}

/// A listing of every shape (the `ui-art mockup` command): index, section, chain, box,
/// rotation, fill and stroke.
pub fn dump(m: &Mockup) -> String {
    let mut out = String::new();
    let c = |c: Option<[u8; 4]>| {
        c.map_or("-".to_string(), |c| {
            format!("#{:02x}{:02x}{:02x}/{}", c[0], c[1], c[2], c[3])
        })
    };
    for s in &m.shapes {
        let chain: Vec<String> = s.chain.iter().map(|i| i.to_string()).collect();
        let sw = s
            .body
            .split("stroke-width=\"")
            .nth(1)
            .and_then(|t| t.split('"').next())
            .unwrap_or("-");
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.1}\t{}\t{}\t{sw}",
            s.index,
            s.sec,
            chain.join("."),
            s.bbox.x0,
            s.bbox.y0,
            s.bbox.w(),
            s.bbox.h(),
            s.rot,
            c(s.color),
            c(s.stroke)
        );
    }
    out
}
