//! Every part of the panel's art, drawn in code (flat style of the ui-poc
//! experiment). A part's size is the size of the control in docs/ui_spec.md,
//! in panel pixels; its viewBox equals that size.
//!
//! Naming: `<control>_<state>`. Push buttons have `rest` and `pressed`,
//! toggles `off` and `on`, buttons that light up also `lit`.

use crate::font::{self, Anchor, label, text};
use crate::svg::{Part, Svg, arc_path, n, pal, polar};

/// The sweep of a knob: the pointer turns from -KNOB_SWEEP to +KNOB_SWEEP
/// degrees (0 = straight up, clockwise positive) as the value goes 0 to 1.
pub const KNOB_SWEEP: f32 = 135.0;

/// A knob style: size, the center, the value ring, the body and the pointer.
pub struct KnobDesign {
    pub name: &'static str,
    pub w: f32,
    pub h: f32,
    pub cx: f32,
    pub cy: f32,
    /// Radius of the value ring, 0 for none (the app paints the arc itself).
    pub ring_r: f32,
    pub ring_w: f32,
    pub body_r: f32,
    pub ptr0: f32,
    pub ptr1: f32,
    pub ptr_w: f32,
    pub body: &'static str,
    pub pointer: &'static str,
    /// The color of the value arc the app paints over the ring track.
    pub arc: &'static str,
    pub track: &'static str,
}

pub const KNOBS: [KnobDesign; 6] = [
    KnobDesign {
        // (this fork: 10% under the original 30 px, the same look)
        name: "knob_303",
        w: 27.0,
        h: 27.0,
        cx: 13.5,
        cy: 13.5,
        ring_r: 12.24,
        ring_w: 1.44,
        body_r: 9.75,
        ptr0: 1.8,
        ptr1: 6.0,
        ptr_w: 1.3,
        body: KNOB_CAP_303,
        pointer: pal::BLACK,
        arc: pal::RED,
        track: "#00000026",
    },
    KnobDesign {
        name: "knob_808_red",
        w: 13.0,
        h: 13.0,
        cx: 6.5,
        cy: 6.5,
        ring_r: 5.9,
        ring_w: 0.9,
        body_r: 4.7,
        ptr0: 1.4,
        ptr1: 4.0,
        ptr_w: 1.0,
        body: pal::RED,
        pointer: pal::WHITE,
        arc: pal::YELLOW,
        track: "#ffffff22",
    },
    KnobDesign {
        name: "knob_808_white",
        w: 13.0,
        h: 13.0,
        cx: 6.5,
        cy: 6.5,
        ring_r: 5.9,
        ring_w: 0.9,
        body_r: 4.7,
        ptr0: 1.4,
        ptr1: 4.0,
        ptr_w: 1.0,
        body: "#e6e8ea",
        pointer: pal::BLACK,
        arc: pal::ORANGE,
        track: "#ffffff22",
    },
    KnobDesign {
        name: "knob_909",
        w: 12.0,
        h: 12.0,
        cx: 6.0,
        cy: 6.0,
        ring_r: 5.4,
        ring_w: 0.9,
        body_r: 4.2,
        ptr0: 1.2,
        ptr1: 3.7,
        ptr_w: 1.0,
        body: pal::BLACK,
        pointer: pal::WHITE,
        arc: pal::ORANGE,
        track: "#00000022",
    },
    // The mixer's and the effects' knobs.
    KnobDesign {
        name: "knob_22",
        w: 22.0,
        h: 22.0,
        cx: 11.0,
        cy: 11.0,
        ring_r: 10.1,
        ring_w: 1.4,
        body_r: 7.9,
        ptr0: 2.4,
        ptr1: 6.7,
        ptr_w: 1.5,
        body: pal::SLATE_DARK,
        pointer: pal::INK,
        arc: pal::AMBER,
        track: "#00000040",
    },
    // The 303's LFO knobs (this fork): the 909's small knob with a red arc.
    KnobDesign {
        name: "knob_lfo",
        w: 12.0,
        h: 12.0,
        cx: 6.0,
        cy: 6.0,
        ring_r: 5.3,
        ring_w: 0.9,
        body_r: 4.0,
        ptr0: 1.3,
        ptr1: 3.5,
        ptr_w: 1.2,
        body: pal::BLACK,
        pointer: pal::WHITE,
        arc: pal::RED,
        track: "#0000002a",
    },
];

fn knobs(out: &mut Vec<Part>) {
    for k in &KNOBS {
        out.push(Part::new(format!("{}_body", k.name), k.w, k.h, |s| {
            s.path_stroke(
                &arc_path(k.cx, k.cy, k.ring_r, -KNOB_SWEEP, KNOB_SWEEP),
                k.track,
                k.ring_w,
            );
            if k.name == "knob_303" {
                knob_body_303(s, k.cx, k.cy);
            } else {
                s.circle(k.cx, k.cy, k.body_r, k.body);
                s.circle_stroke(k.cx, k.cy, k.body_r - 0.6, "#ffffff2a", 0.5);
            }
        }));
        out.push(Part::new(format!("{}_pointer", k.name), k.w, k.h, |s| {
            // Drawn pointing up; the app rotates it around (cx, cy).
            let (x0, y0) = polar(k.cx, k.cy, k.ptr0, 0.0);
            let (x1, y1) = polar(k.cx, k.cy, k.ptr1, 0.0);
            s.line(x0, y0, x1, y1, k.pointer, k.ptr_w);
        }));
    }
}

/// The 303's large knob (this fork, as the reference picture): a dark rim, a gray bezel with a
/// darker inner edge, and a cap whose part above a chord just over the center is gray, the
/// rest light (the color of the 303's paper).
const KNOB_CAP_303: &str = "#c9cbd1";
fn knob_body_303(s: &mut Svg, cx: f32, cy: f32) {
    s.circle(cx, cy, 9.75, "#56575c");
    s.circle(cx, cy, 8.9, "#acafb8");
    s.circle(cx, cy, 6.6, "#888b92");
    s.circle(cx, cy, 6.1, KNOB_CAP_303);
    let (r, d) = (6.1_f32, 1.0_f32);
    let half = (r * r - d * d).sqrt();
    s.path(
        &format!(
            "M{} {} A{r} {r} 0 0 1 {} {} Z",
            n(cx - half),
            n(cy - d),
            n(cx + half),
            n(cy - d)
        ),
        "#acafb8",
    );
}

/// The four colors of the 808's steps, which the app icon and the wordmark's underline reuse.
const STEP_COLORS: [&str; 4] = [pal::RED, pal::ORANGE, pal::YELLOW, pal::WHITE];

/// The app icon (task-115): the 303's saw running into a square wave in orange over four
/// step keys in the 808's colors, on a slate tile; 64 x 64 with the tile inset like a
/// macOS icon's. This fork adds a white x over the square wave's last low line.
fn app_icon(out: &mut Vec<Part>) {
    out.push(Part::new("app_icon", 64.0, 64.0, |s| {
        s.rect(6.0, 6.0, 52.0, 52.0, 12.0, pal::SLATE_DARK);
        s.rect(6.0, 6.0, 52.0, 26.0, 12.0, "#ffffff0c");
        s.rect_stroke(6.5, 6.5, 51.0, 51.0, 11.5, "#ffffff1a", 1.0);
        s.path_stroke("M13 33 L24 17 V33 H30 V17 H41 V33 H51", "#00000050", 5.0);
        s.path_stroke("M13 32 L24 16 V32 H30 V16 H41 V32 H51", pal::ORANGE, 4.0);
        // this fork's x: white, right, over the square wave's last low line
        s.path_stroke("M45 21.5 L51 27.5 M51 21.5 L45 27.5", "#00000050", 3.0);
        s.path_stroke("M45 20.5 L51 26.5 M51 20.5 L45 26.5", pal::WHITE, 2.6);
        for (i, color) in STEP_COLORS.into_iter().enumerate() {
            let x = 12.0 + 10.75 * i as f32;
            s.rect(x, 39.0, 8.5, 9.0, 2.0, "#00000050");
            s.rect(x, 38.0, 8.5, 9.0, 2.0, color);
        }
    }));
}

fn fader(out: &mut Vec<Part>) {
    out.push(Part::new("fader_track", 15.0, 54.0, |s| {
        for i in 0..=10 {
            let y = 5.5 + i as f32 * 4.3;
            let (x0, x1) = if i % 5 == 0 { (0.8, 3.8) } else { (1.8, 3.8) };
            s.line(x0, y, x1, y, pal::INK_DIM, 0.6);
            s.line(15.0 - x1, y, 15.0 - x0, y, pal::INK_DIM, 0.6);
        }
        s.rect(6.0, 3.0, 3.0, 48.0, 1.5, "#000000a0");
    }));
    out.push(Part::new("fader_handle", 15.0, 11.0, |s| {
        s.rect(1.0, 0.0, 13.0, 11.0, 2.5, pal::STEEL);
        s.rect(1.0, 0.0, 13.0, 3.0, 2.5, "#ffffff40");
        s.line(3.0, 5.5, 12.0, 5.5, pal::BLACK, 1.2);
    }));
}

/// A slide switch with a thumb at position `pos` (0 = left or top).
fn switch(out: &mut Vec<Part>, name: &str, w: f32, h: f32, horizontal: bool) {
    for pos in 0..2 {
        out.push(Part::new(format!("{name}_{pos}"), w, h, |s| {
            s.rect(0.0, 0.0, w, h, 2.0, pal::BLACK);
            if horizontal {
                let tw = w / 2.0;
                s.rect(
                    pos as f32 * tw + 1.0,
                    1.0,
                    tw - 2.0,
                    h - 2.0,
                    1.5,
                    pal::STEEL,
                );
            } else {
                let th = h / 2.0;
                s.rect(
                    1.0,
                    pos as f32 * th + 1.0,
                    w - 2.0,
                    th - 2.0,
                    1.5,
                    pal::STEEL,
                );
            }
        }));
    }
}

/// The saw and square glyphs with their left edges at `sx` and `qx`, 6 px tall around `yc`.
/// The saw (at `sx`) and the square (at `qx`) of the 303's waveform, each 7.5 x 6 around
/// `yc`, in their own colors and stroke width.
fn wave_glyphs(s: &mut Svg, sx: f32, qx: f32, yc: f32, colors: (&str, &str), width: f32) {
    let (t, b) = (yc - 3.0, yc + 3.0);
    s.path_stroke(
        &format!(
            "M{sx} {b} L{} {t} V{b} L{} {t} V{b}",
            n(sx + 3.75),
            n(sx + 7.5)
        ),
        colors.0,
        width,
    );
    s.path_stroke(
        &format!(
            "M{qx} {b} V{t} H{} V{b} H{} V{t} H{}",
            n(qx + 2.5),
            n(qx + 5.0),
            n(qx + 7.5)
        ),
        colors.1,
        width,
    );
}

/// A round LED lens of radius `r` in a recessed socket, centered at (`cx`, `cy`): a radial
/// gradient gives it the dome, a white ellipse the gloss, a halo the glow when lit. The
/// socket is translucent, so it takes the color of what it sits on. `hue` is "green" or
/// "amber".
fn led_3d(s: &mut Svg, cx: f32, cy: f32, r: f32, on: bool, hue: &str) {
    let (id, stops) = match (hue, on) {
        ("green", true) => (
            "ledg1",
            concat!(
                "<stop offset=\"0\" stop-color=\"#c8f8cc\"/>",
                "<stop offset=\"0.3\" stop-color=\"#6fe07e\"/>",
                "<stop offset=\"0.75\" stop-color=\"#38b84d\"/>",
                "<stop offset=\"1\" stop-color=\"#1f7a30\"/>",
            ),
        ),
        ("green", false) => (
            "ledg0",
            concat!(
                "<stop offset=\"0\" stop-color=\"#4f6a55\"/>",
                "<stop offset=\"0.5\" stop-color=\"#2f4536\"/>",
                "<stop offset=\"1\" stop-color=\"#1c2a20\"/>",
            ),
        ),
        (_, true) => (
            "leda1",
            concat!(
                "<stop offset=\"0\" stop-color=\"#fff0b8\"/>",
                "<stop offset=\"0.3\" stop-color=\"#ffd04a\"/>",
                "<stop offset=\"0.75\" stop-color=\"#f0a300\"/>",
                "<stop offset=\"1\" stop-color=\"#9a6400\"/>",
            ),
        ),
        (_, false) => (
            "leda0",
            concat!(
                "<stop offset=\"0\" stop-color=\"#5d5340\"/>",
                "<stop offset=\"0.5\" stop-color=\"#3b3526\"/>",
                "<stop offset=\"1\" stop-color=\"#231f14\"/>",
            ),
        ),
    };
    s.raw(&format!(
        "<defs><radialGradient id=\"{id}\" cx=\"0.38\" cy=\"0.32\" r=\"0.75\">{stops}\
         </radialGradient></defs>"
    ));
    // the socket: the lighter ring peeking out below reads as the recess's lower edge
    s.circle_stroke(cx, cy + 0.45, r + 1.3, "#ffffff2c", 0.7);
    s.circle(cx, cy, r + 1.4, "#00000062");
    s.circle_stroke(cx, cy - 0.15, r + 0.9, "#00000050", 0.8);
    if on {
        let halo = if hue == "green" { "#5fe070" } else { "#ffb000" };
        s.circle(cx, cy, r + 2.6, &format!("{halo}24"));
    }
    s.circle(cx, cy, r, &format!("url(#{id})"));
    s.raw(&format!(
        "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"#ffffff\" opacity=\"{}\"/>",
        n(cx - r * 0.25),
        n(cy - r * 0.45),
        n(r * 0.45),
        n(r * 0.28),
        if on { "0.55" } else { "0.18" }
    ));
}

/// The 303's keys (this fork, as the reference picture): a border (dark on black, gray on the
/// paper), a light rim, thicker at the bottom, and a white face. `face` is the face's color.
#[allow(clippy::too_many_arguments)]
fn key_303(s: &mut Svg, x: f32, y: f32, w: f32, h: f32, border: &str, face: &str) {
    s.rect(x, y, w, h, 2.0, border);
    s.rect(x + 0.75, y + 0.75, w - 1.5, h - 1.5, 1.6, "#c4c7cb");
    s.rect(x + 1.25, y + 1.25, w - 2.5, h - 3.25, 1.3, face);
}
pub const KEY_FACE_303: &str = "#fbfbf8";
const KEY_BORDER_303: &str = "#727274";
const KEY_BORDER_DARK_303: &str = "#0d0e0f";
/// A key of the chosen scale (the GENERATE menu): as if a green LED inside the key's slightly
/// translucent plastic lit it, the face a pale green, brighter in its middle and fading to the
/// edges.
fn key_scale_303(s: &mut Svg, x: f32, y: f32, w: f32, h: f32, border: &str) {
    key_303(s, x, y, w, h, border, "#e6f6e8");
    s.raw(concat!(
        "<defs><radialGradient id=\"scale_glow\" cx=\"0.5\" cy=\"0.45\" r=\"0.6\">",
        "<stop offset=\"0\" stop-color=\"#5fd872\" stop-opacity=\"1\"/>",
        "<stop offset=\"0.5\" stop-color=\"#8ae498\" stop-opacity=\"0.75\"/>",
        "<stop offset=\"1\" stop-color=\"#bdeec4\" stop-opacity=\"0.3\"/>",
        "</radialGradient></defs>",
    ));
    s.rect(
        x + 1.25,
        y + 1.25,
        w - 2.5,
        h - 3.25,
        1.3,
        "url(#scale_glow)",
    );
}

/// A keycap: a face over a 1.8 px darker lip, with a thin outline.
fn keycap(s: &mut Svg, x: f32, y: f32, w: f32, h: f32, face: &str, lip: &str) {
    s.rect(x, y, w, h, 2.2, lip);
    s.rect(x, y, w, h - 1.8, 2.2, face);
    s.rect_stroke(x + 0.25, y + 0.25, w - 0.5, h - 0.5, 2.2, "#00000055", 0.5);
}

/// The discreet 3D of the 303's keys and pills: a darker base, the face inset by 0.5 px on
/// the sides and 1.5 px short at the bottom. `r` is the corner radius.
#[allow(clippy::too_many_arguments)]
fn key_3d(s: &mut Svg, x: f32, y: f32, w: f32, h: f32, r: f32, base: &str, face: &str) {
    s.rect(x, y, w, h, r, base);
    s.rect(x + 0.5, y, w - 1.0, h - 1.5, r - 0.3, face);
}

/// The mixer's routing key, 28 x 13: a face over a darker base that shows as a soft rim and a
/// lip, an amber LED at the left and the label left-aligned after it.
fn mixsw_body(s: &mut Svg, label_: &str, on: bool) {
    s.rect(0.0, 0.0, 28.0, 13.0, 2.5, "#2a323d");
    s.rect(0.6, 0.6, 26.8, 10.6, 2.0, "#4b5767");
    s.rect(0.6, 0.6, 26.8, 4.6, 2.0, "#ffffff0c");
    led_3d(s, 5.4, 6.2, 1.8, on, "amber");
    let color = if on { pal::INK } else { pal::INK_DIM };
    text(s, 10.4, 6.2, 3.8, Anchor::Start, color, label_);
}

/// The LFO's mode buttons under a 303 knob (this fork): two equal cells on one black frame,
/// `LFO_MODE_W` x `LFO_MODE_H`. Parts `lfo_mode_<s><h>`: the left cell a note, tempo sync (`s`
/// 1: green with a glow, else a dull blue); the right cell the wave, one button that flips it
/// (`h` 0: the sine, red with a glow; `h` 1: the random sine, sample & hold, green with a glow).
fn lfo_modes(out: &mut Vec<Part>) {
    let (w, h) = (LFO_MODE_W, LFO_MODE_H);
    let cell = w / 2.0;
    let yc = h / 2.0;
    // the waves, 8 wide, centered in the right cell
    let x0 = cell + cell / 2.0 - 4.0;
    let sine = format!(
        "M{} {yc} C{} {} {} {} {} {yc} S{} {} {} {yc}",
        n(x0),
        n(x0 + 1.5),
        n(yc - 3.4),
        n(x0 + 2.5),
        n(yc - 3.4),
        n(x0 + 4.0),
        n(x0 + 6.5),
        n(yc + 3.4),
        n(x0 + 8.0),
    );
    // uneven peaks: a sine whose every half has its own height
    let random = format!(
        "M{} {yc} C{} {} {} {} {} {yc} S{} {} {} {yc} S{} {} {} {yc} S{} {} {} {yc}",
        n(x0),
        n(x0 + 0.7),
        n(yc - 1.8),
        n(x0 + 1.3),
        n(yc - 1.8),
        n(x0 + 2.0),
        n(x0 + 3.3),
        n(yc + 3.6),
        n(x0 + 4.0),
        n(x0 + 5.3),
        n(yc - 3.6),
        n(x0 + 6.0),
        n(x0 + 7.3),
        n(yc + 2.0),
        n(x0 + 8.0),
    );
    // the note: a head, its stem and its flag, centered in the left cell
    let nx = cell / 2.0;
    let flag = format!(
        "M{} {} C{} {} {} {} {} {}",
        n(nx + 0.9),
        n(yc - 3.6),
        n(nx + 1.6),
        n(yc - 2.4),
        n(nx + 3.2),
        n(yc - 2.2),
        n(nx + 2.6),
        n(yc - 0.2),
    );
    for sync in [false, true] {
        for hold in [false, true] {
            let name = format!("lfo_mode_{}{}", sync as u8, hold as u8);
            out.push(Part::new(name, w, h, |s| {
                s.rect(0.0, 0.0, w, h, 2.5, pal::BLACK);
                s.rect(0.6, 0.6, w - 1.2, h - 1.2, 2.0, "#26282b");
                s.line(cell, 2.5, cell, h - 2.5, "#0e0f10", 1.0);
                // the note: tempo sync, green with a glow when on
                if sync {
                    s.rect(1.5, 1.5, cell - 3.0, h - 3.0, 2.0, WAVE_GLOW);
                }
                let c = if sync { pal::GREEN } else { LFO_SYNC_OFF };
                s.raw(&format!(
                    r#"<ellipse cx="{}" cy="{}" rx="1.5" ry="1.1" transform="rotate(-25 {} {})" fill="{c}"/>"#,
                    n(nx - 0.5),
                    n(yc + 2.3),
                    n(nx - 0.5),
                    n(yc + 2.3),
                ));
                s.line(nx + 0.9, yc + 2.1, nx + 0.9, yc - 3.6, c, 0.7);
                s.path_stroke(&flag, c, 0.7);
                // the wave: the sine red, the random sine green, each with its glow
                let (d, color, glow) = if hold {
                    (&random, pal::GREEN, WAVE_GLOW)
                } else {
                    (&sine, pal::RED, "#e2413b59")
                };
                s.path_stroke(d, glow, 2.8);
                s.path_stroke(d, color, 1.2);
            }));
        }
    }
}

fn switches(out: &mut Vec<Part>) {
    lfo_modes(out);
    // The 303's waveform (this fork): no slider, the saw and the square side by side on one
    // black frame of the LFO mode buttons' size and look, each half a click target; the
    // chosen one (position 0 saw, 1 square) green with a soft glow, the other a dull red
    // like an unlit LED.
    let (w, h) = (WAVE_W_303, WAVE_H_303);
    let half = w / 2.0;
    for pos in 0..2 {
        out.push(Part::new(format!("switch_wave_{pos}"), w, h, |s| {
            s.rect(0.0, 0.0, w, h, 2.5, pal::BLACK);
            s.rect(0.6, 0.6, w - 1.2, h - 1.2, 2.0, "#26282b");
            s.line(half, 2.5, half, h - 2.5, "#0e0f10", 1.0);
            let (sx, qx) = (half / 2.0 - 3.75, half + half / 2.0 - 3.75);
            let on = if pos == 0 {
                (pal::GREEN, WAVE_OFF)
            } else {
                (WAVE_OFF, pal::GREEN)
            };
            let glow = if pos == 0 {
                (WAVE_GLOW, "none")
            } else {
                ("none", WAVE_GLOW)
            };
            wave_glyphs(s, sx, qx, h / 2.0, glow, 2.8);
            wave_glyphs(s, sx, qx, h / 2.0, on, 1.3);
        }));
    }
    switch(out, "switch_808", 11.0, 17.0, false);
    switch(out, "switch_fx", 9.0, 18.0, false);
    switch(out, "switch_mode", 26.0, 19.0, true);
    // Dist, PCF and Comp routing keys of the mixer (and the master's).
    for (kind, text_) in [("dist", "DIST"), ("pcf", "PCF"), ("comp", "COMP")] {
        for (state, on) in [("off", false), ("on", true)] {
            out.push(Part::new(
                format!("mixsw_{kind}_{state}"),
                28.0,
                13.0,
                |s| mixsw_body(s, text_, on),
            ));
        }
    }
}

fn power_buttons(out: &mut Vec<Part>) {
    // The pattern sections' (13) and the mixer's and effects' (14) on/off buttons: a round 3D
    // LED, no bezel.
    for (w, h) in [(13.0, 13.0), (14.0, 14.0)] {
        for (state, on) in [("off", false), ("on", true)] {
            out.push(Part::new(
                format!("power_{}x{}_{state}", n(w), n(h)),
                w,
                h,
                |s| led_3d(s, w / 2.0, h / 2.0, 2.6, on, "green"),
            ));
        }
    }
    // The mixer channel's SOLO LED (this fork): the same LED in amber.
    for (state, on) in [("off", false), ("on", true)] {
        out.push(Part::new(format!("solo_14x14_{state}"), 14.0, 14.0, |s| {
            led_3d(s, 7.0, 7.0, 2.6, on, "amber")
        }));
    }
}

fn transport_buttons(out: &mut Vec<Part>) {
    let icons: [(&str, bool); 5] = [
        ("play", true),
        ("stop", false),
        ("rewind", false),
        ("ff", false),
        ("record", true),
    ];
    for (name, has_lit) in icons {
        let states: &[&str] = if has_lit {
            &["rest", "pressed", "lit"]
        } else {
            &["rest", "pressed"]
        };
        for &state in states {
            out.push(Part::new(format!("btn_{name}_{state}"), 32.0, 25.0, |s| {
                let fill = match (state, name) {
                    ("lit", "record") => pal::RED,
                    ("lit", _) => pal::SLATE_LINE,
                    _ => pal::SLATE_DARK,
                };
                s.rect(1.0, 1.0, 30.0, 23.0, 4.0, fill);
                if state == "pressed" {
                    s.rect(1.0, 1.0, 30.0, 23.0, 4.0, pal::SHADE);
                    s.rect_stroke(1.5, 1.5, 29.0, 22.0, 3.5, "#00000066", 1.0);
                }
                let (cx, cy) = (16.0, 12.5);
                let ink = pal::INK;
                match name {
                    "play" => {
                        s.path(
                            &format!(
                                "M{} {} L{} {} L{} {} Z",
                                n(cx - 4.5),
                                n(cy - 6.0),
                                n(cx + 6.0),
                                n(cy),
                                n(cx - 4.5),
                                n(cy + 6.0)
                            ),
                            pal::GREEN,
                        );
                    }
                    "stop" => {
                        s.rect(cx - 5.0, cy - 5.0, 10.0, 10.0, 1.2, ink);
                    }
                    "rewind" => {
                        for dx in [-5.5, 1.5] {
                            s.path(
                                &format!(
                                    "M{} {} L{} {} L{} {} Z",
                                    n(cx + dx + 5.5),
                                    n(cy - 5.5),
                                    n(cx + dx),
                                    n(cy),
                                    n(cx + dx + 5.5),
                                    n(cy + 5.5)
                                ),
                                ink,
                            );
                        }
                    }
                    "ff" => {
                        for dx in [-6.5, 0.0] {
                            s.path(
                                &format!(
                                    "M{} {} L{} {} L{} {} Z",
                                    n(cx + dx),
                                    n(cy - 5.5),
                                    n(cx + dx + 5.5),
                                    n(cy),
                                    n(cx + dx),
                                    n(cy + 5.5)
                                ),
                                ink,
                            );
                        }
                    }
                    _ => {
                        s.circle(
                            cx,
                            cy,
                            5.5,
                            if state == "lit" { pal::WHITE } else { pal::RED },
                        );
                    }
                }
            }));
        }
    }
    // The tempo, bar and loop arrows.
    for (name, up) in [("inc", true), ("dec", false)] {
        for state in ["rest", "pressed"] {
            out.push(Part::new(format!("arrow_{name}_{state}"), 11.0, 9.0, |s| {
                s.rect(
                    0.5,
                    0.5,
                    10.0,
                    8.0,
                    1.8,
                    if state == "pressed" {
                        pal::SLATE_LINE
                    } else {
                        pal::SLATE_DARK
                    },
                );
                let d = if up {
                    "M2.8 6 L5.5 2.6 L8.2 6 Z"
                } else {
                    "M2.8 3 L5.5 6.4 L8.2 3 Z"
                };
                s.path(d, pal::INK);
            }));
        }
    }
    // The pattern/song mode has switch_mode; loop on/off and shuffle are toggles.
    for (state, on) in [("off", false), ("on", true)] {
        out.push(Part::new(format!("toggle_loop_{state}"), 18.0, 18.0, |s| {
            s.rect(
                0.5,
                0.5,
                17.0,
                17.0,
                3.0,
                if on { pal::AMBER } else { pal::SLATE_DARK },
            );
            let ink = if on { pal::BLACK } else { pal::INK };
            // The repeat sign: two rounded half-loops, each ending in an arrowhead (the
            // top one points right, the bottom one left), symmetric about the center.
            s.path_stroke("M4.5 9.5 V7.5 Q4.5 5.5 6.5 5.5 H11", ink, 1.3);
            s.path_stroke("M13.5 8.5 V10.5 Q13.5 12.5 11.5 12.5 H7", ink, 1.3);
            s.path("M11 3 L14 5.5 L11 8 Z", ink);
            s.path("M7 15 L4 12.5 L7 10 Z", ink);
        }));
        out.push(Part::new(
            format!("toggle_shuffle_{state}"),
            30.0,
            12.0,
            |s| {
                if on {
                    keycap(s, 0.0, 0.0, 30.0, 12.0, "#e8503f", "#9c2b25");
                } else {
                    keycap(s, 0.0, 0.0, 30.0, 12.0, "#dde1e6", "#a7afba");
                }
                let ink = if on { pal::WHITE } else { pal::BLACK };
                label(s, 15.0, 5.3, 3.2, ink, "SHUFFLE");
            },
        ));
        // The 909's flam button (this fork): a light key like a step key, its light red when on.
        out.push(Part::new(format!("toggle_flam_{state}"), 15.0, 16.0, |s| {
            key909(s, 15.0, 16.0, "#f6f4ec");
            led909(s, 7.5, 8.0, if on { pal::RED } else { LED_OFF_909 });
        }));
        // the 909's FLIP (this fork): the same key, lower
        out.push(Part::new(format!("flip909_{state}"), 15.0, 13.0, |s| {
            key909(s, 15.0, 13.0, "#f6f4ec");
            led909(s, 7.5, 6.5, if on { pal::RED } else { LED_OFF_909 });
        }));
    }
}

fn pattern_buttons(out: &mut Vec<Part>) {
    for k in 1..=8 {
        for (state, on) in [("off", false), ("on", true)] {
            out.push(Part::new(format!("pat_btn_{k}_{state}"), 14.0, 15.0, |s| {
                if on {
                    keycap(s, 0.0, 0.0, 14.0, 15.0, "#e8503f", "#9c2b25");
                } else {
                    keycap(s, 0.0, 0.0, 14.0, 15.0, "#dde1e6", "#a7afba");
                }
                let ink = if on { pal::WHITE } else { pal::BLACK };
                label(s, 7.0, 6.8, 5.5, ink, &k.to_string());
            }));
        }
    }
    for bank in ["A", "B", "C", "D"] {
        for (state, on) in [("off", false), ("on", true)] {
            out.push(Part::new(
                format!("bank_btn_{bank}_{state}"),
                14.0,
                15.0,
                |s| {
                    if on {
                        keycap(s, 0.0, 0.0, 14.0, 15.0, pal::ORANGE, "#a85d14");
                    } else {
                        keycap(s, 0.0, 0.0, 14.0, 15.0, "#4e5a6b", "#2c3542");
                    }
                    let ink = if on { pal::BLACK } else { pal::INK_DIM };
                    label(s, 7.0, 6.8, 5.5, ink, bank);
                },
            ));
        }
    }
}

fn small_buttons(out: &mut Vec<Part>) {
    // the 303's pitch mode and clear keys: white keys, the pitch key's border dark on black
    for (border, kind) in [(KEY_BORDER_DARK_303, "pitch"), (KEY_BORDER_303, "clear")] {
        for state in ["rest", "pressed"] {
            let (w, h) = (PILL_W_303, PILL_H_303);
            out.push(Part::new(format!("btn_{kind}_{state}"), w, h, |s| {
                key_303(s, 0.0, 0.0, w, h, border, KEY_FACE_303);
                if state == "pressed" {
                    s.rect(0.0, 0.0, w, h, 2.0, "#80808066");
                }
            }));
        }
    }
    // The 303's RNDM pill (this fork): the GENERATE menu's button, its name on it.
    for state in ["rest", "pressed"] {
        let (w, h) = (RNDM_W_303, RNDM_H_303);
        out.push(Part::new(format!("btn_rndm_{state}"), w, h, |s| {
            key_303(
                s,
                0.0,
                0.0,
                w - 0.5,
                h - 0.5,
                KEY_BORDER_DARK_303,
                KEY_FACE_303,
            );
            if state == "pressed" {
                s.rect(0.0, 0.0, w - 0.5, h - 0.5, 2.0, "#80808066");
            }
            label(
                s,
                (w - 0.5) / 2.0,
                (h - 0.5) / 2.0 - 0.5,
                4.0,
                pal::BLACK,
                "RNDM",
            );
        }));
    }
    // ... and while its menu is open: the button as it is, only its name green with a soft
    // glow (a blurred copy under it)
    {
        let (w, h) = (RNDM_W_303, RNDM_H_303);
        out.push(Part::new("btn_rndm_on", w, h, |s| {
            key_303(s, 0.0, 0.0, w - 0.5, h - 0.5, KEY_BORDER_DARK_303, KEY_FACE_303);
            s.raw(
                r#"<filter id="rndm_glow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="0.9"/></filter><g filter="url(#rndm_glow)" opacity="0.9">"#,
            );
            label(s, (w - 0.5) / 2.0, (h - 0.5) / 2.0 - 0.5, 4.0, pal::GREEN, "RNDM");
            s.raw("</g>");
            label(s, (w - 0.5) / 2.0, (h - 0.5) / 2.0 - 0.5, 4.0, pal::GREEN, "RNDM");
        }));
    }
    // The 303's keys and flags (this fork): white keys with a border, gray on the paper and
    // dark on the black keys' slots; a key of the chosen scale (the GENERATE menu) has a green
    // face (`_scale`).
    let (ww, bw, kh) = (WHITE_KEY_W_303, BLACK_KEY_W_303, KEY_H_303);
    for state in ["rest", "pressed", "scale"] {
        for (name, w, border) in [
            ("key_white", ww, KEY_BORDER_303),
            ("key_black", bw, KEY_BORDER_DARK_303),
            ("btn_flag", ww, KEY_BORDER_303),
        ] {
            if name == "btn_flag" && state == "scale" {
                continue;
            }
            // the part in whole pixels, the key centered in it
            let (pw, ph) = (w.ceil(), kh.ceil());
            let x = (pw - w) / 2.0;
            out.push(Part::new(format!("{name}_{state}"), pw, ph, |s| {
                if state == "scale" {
                    key_scale_303(s, x, 0.0, w, kh, border);
                } else {
                    key_303(s, x, 0.0, w, kh, border, KEY_FACE_303);
                }
                if state == "pressed" {
                    s.rect(x, 0.0, w, kh, 2.0, "#80808066");
                }
            }));
        }
    }
}

fn leds(out: &mut Vec<Part>) {
    for (state, color) in [("off", "#6b302e"), ("on", pal::RED)] {
        out.push(Part::new(format!("led_8_{state}"), 8.0, 8.0, |s| {
            if state == "on" {
                s.circle(4.0, 4.0, 4.0, "#e2413b40");
            }
            s.circle(4.0, 4.0, 2.75, color);
            s.circle_stroke(4.0, 4.0, 2.75, "#00000059", 0.5);
        }));
    }
    // The 303's step LEDs (this fork): lit red for a step of the pattern's length, off past it
    // (as `led_8_off`), green for the edit step, yellow with a bright glow for the step that
    // plays (the running light).
    let d = STEP_LED_303;
    for (state, color, glow) in [
        ("off", "#4a2a28", None),
        ("on", pal::RED, Some("#e2413b40")),
        ("sel", pal::GREEN, Some("#53d86a59")),
        ("play", pal::YELLOW, Some("#f2c53199")),
    ] {
        // square, with a square glow when lit
        out.push(Part::new(format!("stepled_{state}"), d, d, |s| {
            if let Some(g) = glow {
                s.rect(0.0, 0.0, d, d, 1.5, g);
            }
            s.rect(1.0, 1.0, d - 2.0, d - 2.0, 0.8, color);
            s.rect_stroke(1.0, 1.0, d - 2.0, d - 2.0, 0.8, "#00000040", 0.5);
        }));
    }
    // The 9 LEDs of the compressor's gain-reduction meter.
    for (state, color) in [
        ("off", "#00000066"),
        ("green", pal::GREEN),
        ("amber", pal::AMBER),
        ("red", pal::RED),
    ] {
        out.push(Part::new(format!("gr_led_{state}"), 7.0, 10.0, |s| {
            s.rect(0.5, 0.5, 6.0, 9.0, 1.5, "#15181d");
            s.rect(1.5, 1.5, 4.0, 7.0, 1.0, color);
        }));
    }
}

fn steps(out: &mut Vec<Part>) {
    let colors = [
        ("red", pal::RED),
        ("orange", pal::ORANGE),
        ("yellow", pal::YELLOW),
        ("white", pal::WHITE),
    ];
    // the 808's step keys (this fork): a colored key with a light in it, lit red (white on the
    // red keys) when on
    for (name, color) in colors {
        for (state, on) in [("off", false), ("on", true)] {
            out.push(Part::new(
                format!("step808_{name}_{state}"),
                STEP808_W,
                STEP808_H,
                |s| {
                    let (w, h) = (STEP808_W, STEP808_H);
                    s.rect(0.0, 0.5, w, h - 0.5, 2.5, &shade(color, 0.62));
                    s.rect(0.0, 0.0, w, h - 1.5, 2.5, color);
                    s.rect(0.8, 0.8, w - 1.6, 4.0, 2.0, "#ffffff22");
                    let led = match (on, name) {
                        (false, _) => &shade(color, 0.55)[..],
                        (true, "red") => pal::WHITE,
                        (true, _) => pal::RED,
                    };
                    if on {
                        s.circle(w / 2.0, 5.5, 3.6, "#ff5a4d40");
                    }
                    s.circle(w / 2.0, 5.5, 2.4, led);
                    s.circle_stroke(w / 2.0, 5.5, 2.4, "#00000059", 0.6);
                },
            ));
        }
    }
    // the 909's step keys (this fork): light keys in a gray frame, in groups of four, the light
    // on top in the color of the value
    for (grp, fill) in [("a", "#f6f4ec"), ("b", "#e9e0c9")] {
        for (value, led) in [
            (0, LED_OFF_909),
            (1, pal::ORANGE),
            (2, pal::RED),
            (3, pal::GREEN),
        ] {
            out.push(Part::new(
                format!("step909_{grp}_{value}"),
                21.0,
                STEP909_H,
                |s| {
                    s.g_at(0.5, 0.0);
                    key909(s, 20.0, STEP909_H, fill);
                    s.end_g();
                    led909(s, 10.5, 5.0, led);
                },
            ));
        }
    }
    // Overlays that mark the selected instrument in the legend rows.
    for (w, h) in [(22.0, 14.0), (42.0, 14.0), (21.0, 14.0)] {
        out.push(Part::new(format!("sel_{}x{}", n(w), n(h)), w, h, |s| {
            s.rect(0.0, 0.0, w, h, 2.0, "#f08a2440");
            s.rect_stroke(0.75, 0.75, w - 1.5, h - 1.5, 1.5, pal::ORANGE, 1.5);
        }));
    }
}

const SEG: [(f32, f32, f32, f32); 7] = [
    (2.0, 1.0, 5.0, 1.6),  // a
    (6.2, 2.4, 1.6, 5.2),  // b
    (6.2, 8.4, 1.6, 5.2),  // c
    (2.0, 13.4, 5.0, 1.6), // d
    (1.2, 8.4, 1.6, 5.2),  // e
    (1.2, 2.4, 1.6, 5.2),  // f
    (2.0, 7.2, 5.0, 1.6),  // g
];

fn mask(c: char) -> u8 {
    match c {
        '0' => 0b0111111,
        '1' => 0b0000110,
        '2' => 0b1011011,
        '3' => 0b1001111,
        '4' => 0b1100110,
        '5' => 0b1101101,
        '6' => 0b1111101,
        '7' => 0b0000111,
        '8' => 0b1111111,
        '9' => 0b1101111,
        _ => 0,
    }
}

/// The 11 frames of the seven-segment display: digits 0..9 and blank. The
/// unlit segments stay faintly visible (a blank is the disabled display). `segb_*` (this fork)
/// are the same with the unlit segments a little lighter: every display of the panel.
fn digits(out: &mut Vec<Part>) {
    for (prefix, off) in [("seg", pal::LCD_OFF), ("segb", LCD_OFF_LIGHT)] {
        for c in "0123456789 ".chars() {
            let name = if c == ' ' {
                format!("{prefix}_blank")
            } else {
                format!("{prefix}_{c}")
            };
            out.push(Part::new(name, 9.0, 16.0, |s| {
                for (k, &(x, y, w, h)) in SEG.iter().enumerate() {
                    let on = mask(c) & (1 << k) != 0;
                    s.rect(x, y, w, h, 0.6, if on { pal::LCD_ON } else { off });
                }
            }));
        }
    }
}

/// The unlit segments of the panel's displays (this fork): a little lighter than `LCD_OFF`.
const LCD_OFF_LIGHT: &str = "#3a1a17";

/// A level meter with 10 segments and 11 states (0 = silent, 10 = clip).
fn meters(out: &mut Vec<Part>) {
    // The channels' and effects' header meter (bare segments on the header), and the
    // master's two (segments in a dark well).
    for (w, h) in [(8.0, 14.0), (15.0, 56.0)] {
        let header = w == 8.0;
        let pad = if header { 0.0 } else { 1.5 };
        for level in 0..=10 {
            out.push(Part::new(
                format!("meter_{}x{}_{level}", n(w), n(h)),
                w,
                h,
                |s| {
                    if !header {
                        s.rect(0.0, 0.0, w, h, 1.5, "#101216");
                    }
                    let seg_h = (h - 2.0 * pad) / 10.0;
                    for i in 0..10 {
                        let y = h - pad - (i as f32 + 1.0) * seg_h;
                        let color = match i {
                            0..=5 => pal::GREEN,
                            6..=7 => pal::AMBER,
                            _ => pal::RED,
                        };
                        let on = i < level;
                        let off = if header { "#00000040" } else { "#ffffff14" };
                        s.rect(
                            pad,
                            y + 0.25,
                            w - 2.0 * pad,
                            seg_h - 0.5,
                            0.3,
                            if on { color } else { off },
                        );
                    }
                },
            ));
        }
    }
}

// ---------------------------------------------------------------- backgrounds

/// The dark seam color first, then the section's face inset by one pixel.
fn face(s: &mut Svg, w: f32, h: f32, fill: &str) {
    s.rect(0.0, 0.0, w, h, 0.0, pal::BG);
    s.rect(1.0, 1.0, w - 2.0, h - 2.0, 3.0, fill);
}

/// `hex` (`#rrggbb`) with each channel times `f`.
fn shade(hex: &str, f: f32) -> String {
    let v = u32::from_str_radix(&hex[1..7], 16).expect("a #rrggbb color");
    let c = |sh: u32| (((v >> sh) & 0xff) as f32 * f).round().clamp(0.0, 255.0) as u32;
    format!("#{:02x}{:02x}{:02x}", c(16), c(8), c(0))
}

/// A section header with a 3D look: a lip in the header color darkened to 0.62 under the
/// face, which is 1.3 px shorter, a vertical gradient over the face and a thin white line
/// just below its top.
fn header(s: &mut Svg, x: f32, y: f32, w: f32, h: f32, fill: &str) {
    s.raw(concat!(
        "<defs><linearGradient id=\"hdr\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\">",
        "<stop offset=\"0\" stop-color=\"#ffffff\" stop-opacity=\"0.17\"/>",
        "<stop offset=\"0.55\" stop-color=\"#ffffff\" stop-opacity=\"0.02\"/>",
        "<stop offset=\"1\" stop-color=\"#000000\" stop-opacity=\"0.16\"/>",
        "</linearGradient></defs>",
    ));
    s.rect(x, y, w, h, 2.0, &shade(fill, 0.62));
    s.rect(x, y, w, h - 1.3, 2.0, fill);
    s.rect(x, y, w, h - 1.3, 2.0, "url(#hdr)");
    s.line(x + 2.0, y + 0.5, x + w - 2.0, y + 0.5, "#ffffff2e", 0.6);
}

/// The pattern sections' header color (rust).
const PATTERN_HEADER: &str = "#bf4a2b";
/// The mixer channels' header color (sea).
const MIXER_HEADER: &str = "#3b8a8f";

fn well(s: &mut Svg, x: f32, y: f32, w: f32, h: f32) {
    s.rect(x - 0.5, y - 0.5, w + 1.0, h + 1.0, 2.0, "#0c0707");
    s.rect(x, y, w, h, 1.5, pal::LCD_BG);
}

fn wood(out: &mut Vec<Part>) {
    out.push(Part::new("wood", 16.0, 510.0, |s| {
        // Three planks with seams and a few grain strokes.
        let tones = [pal::WOOD, pal::WOOD_LIGHT, pal::WOOD_DARK];
        for (i, tone) in tones.iter().enumerate() {
            s.rect(i as f32 * 5.34, 0.0, 5.34, 510.0, 0.0, tone);
        }
        s.line(5.34, 0.0, 5.34, 510.0, "#00000040", 0.8);
        s.line(10.67, 0.0, 10.67, 510.0, "#00000040", 0.8);
        let mut seed = 0x9e37_79b9_u32;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..60 {
            let x = 0.8 + (next() % 1440) as f32 / 100.0;
            let y = (next() % 49000) as f32 / 100.0;
            let len = 8.0 + (next() % 2800) as f32 / 100.0;
            s.line(x, y, x + 0.4, y + len, "#00000026", 0.5);
        }
        s.line(0.5, 0.0, 0.5, 510.0, "#ffffff18", 1.0);
        s.line(15.5, 0.0, 15.5, 510.0, "#00000040", 1.0);
    }));
}

fn transport_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_transport", 592.0, 50.0, |s| {
        face(s, 592.0, 50.0, pal::SLATE);
        let dim = pal::INK_DIM;
        // centered with its knob in the space left of the tempo display (0..44)
        label(s, 22.0, 38.0, 3.4, dim, "SHUFFLE");
        label(s, 57.5, 21.5, 3.4, dim, "TEMPO");
        for x in [44.0, 271.0, 329.0, 372.0] {
            well(s, x, 27.0, 27.0, 16.0);
        }
        text(s, 158.0, 11.5, 4.0, Anchor::End, pal::INK, "PATTERN");
        text(s, 190.0, 11.5, 4.0, Anchor::Start, pal::INK, "SONG");
        label(s, 284.5, 21.5, 3.4, dim, "BAR");
        label(s, 342.5, 21.5, 3.4, dim, "START");
        label(s, 385.5, 21.5, 3.4, dim, "LENGTH");
        s.line(263.0, 8.0, 263.0, 42.0, pal::SLATE_LINE, 1.0);
    }));
}

/// The size of the logo parts, and of the logo's click area (`layout::on_logo` of orb-ui).
pub const LOGO_W: f32 = 134.0;
pub const LOGO_H: f32 = 48.0;
/// Where the logo sits on the panel (the transport section is at x = 16, the logo at 451 in it).
/// This fork moved it 7 px left of the original's 474, so the wider wordmark OpenRB-338x ends
/// about 10 px before the transport's right edge (608).
pub const LOGO_X: f32 = 467.0;
pub const LOGO_Y: f32 = 1.0;
/// The pieces of the logo, each on the whole `LOGO_W` x `LOGO_H` canvas: every piece is placed
/// at (0, 0) of the logo, so an animation only offsets or fades a piece. In the order the
/// startup animation shows them (logo_anim.rs of orb-ui): OpenRB-, 338, the four underline
/// segments left to right, the x, the subtitle.
pub const LOGO_PIECES: [&str; 8] = [
    "logo_openrb",
    "logo_338",
    "logo_line_0",
    "logo_line_1",
    "logo_line_2",
    "logo_line_3",
    "logo_x",
    "logo_sub",
];

/// The left edge of the wordmark, its underline and its subtitle in the logo part.
pub const LOGO_TEXT_X: f32 = 5.0;
/// The wordmark's cap height.
pub const LOGO_CAP: f32 = 12.5;
/// The wordmark's vertical center: the descender of its `p` clear of the underline (y 28).
pub const LOGO_TEXT_Y: f32 = 16.0;
/// The space between the underline's four segments.
const LOGO_LINE_GAP: f32 = 1.5;

/// Where the wordmark's `x` starts in the logo part, and its center (the startup animation
/// flashes it there).
pub fn logo_x_pos() -> (f32, f32) {
    let cap = LOGO_CAP;
    let x_338 = LOGO_TEXT_X + font::text_width(cap, "OpenRB-") + font::gap(cap);
    let x_x = x_338 + font::text_width(cap, "338") + font::gap(cap);
    (x_x, x_x + font::text_width(cap, "x") / 2.0)
}

/// The wordmark of this fork, `OpenRB-338x`: the whole, and its pieces (`LOGO_PIECES`) for
/// task-114. `OpenRB-` is INK, `338` ORANGE and the `x` WHITE. The underline is four segments
/// in the 808's step colors, from the wordmark's left edge to the outer edge of the x's stroke.
fn logo(out: &mut Vec<Part>) {
    let cap = LOGO_CAP;
    let x0 = LOGO_TEXT_X;
    let x_338 = x0 + font::text_width(cap, "OpenRB-") + font::gap(cap);
    let (x_x, _) = logo_x_pos();
    // the x's round stroke ends half a stroke past its box: the underline goes there too
    let word_w = x_x + font::text_width(cap, "x") + font::stroke(cap) / 2.0 - x0;
    let seg_w = (word_w - 3.0 * LOGO_LINE_GAP) / 4.0;
    let y = LOGO_TEXT_Y;
    let openrb = |s: &mut Svg| text(s, x0, y, cap, Anchor::Start, pal::INK, "OpenRB-");
    let n338 = |s: &mut Svg| text(s, x_338, y, cap, Anchor::Start, pal::ORANGE, "338");
    let x = |s: &mut Svg| text(s, x_x, y, cap, Anchor::Start, pal::WHITE, "x");
    let line = |s: &mut Svg, i: usize| {
        let x = x0 + (seg_w + LOGO_LINE_GAP) * i as f32;
        s.rect(x, 28.0, seg_w, 1.5, 0.75, STEP_COLORS[i]);
    };
    let sub = |s: &mut Svg| {
        let y = 37.0;
        text(
            s,
            x0,
            y,
            3.4,
            Anchor::Start,
            pal::INK_DIM,
            "BASS LINE AND DRUM MACHINE",
        );
    };
    // the whole is the pieces in `LOGO_PIECES` order
    out.push(Part::new("logo", LOGO_W, LOGO_H, |s| {
        openrb(s);
        n338(s);
        for i in 0..4 {
            line(s, i);
        }
        x(s);
        sub(s);
    }));
    out.push(Part::new("logo_openrb", LOGO_W, LOGO_H, openrb));
    out.push(Part::new("logo_338", LOGO_W, LOGO_H, n338));
    for i in 0..4 {
        out.push(Part::new(format!("logo_line_{i}"), LOGO_W, LOGO_H, |s| {
            line(s, i)
        }));
    }
    out.push(Part::new("logo_x", LOGO_W, LOGO_H, x));
    out.push(Part::new("logo_sub", LOGO_W, LOGO_H, sub));
}

fn pattern_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_pattern", 70.0, 115.0, |s| {
        face(s, 70.0, 115.0, pal::SLATE);
        header(s, 2.0, 2.0, 66.0, 17.0, PATTERN_HEADER);
        // centered between the on/off button (4..17) and the header's right edge
        label(s, 42.5, 10.5, 4.0, pal::WHITE, "PATTERN");
        label(s, 45.5, 91.3, 3.2, pal::INK_DIM, "STEPS");
        well(s, 36.0, 95.0, 19.0, 16.0);
    }));
    for (name, fill) in [("focus_off", pal::SLATE_DARK), ("focus_on", pal::ORANGE)] {
        out.push(Part::new(name, 4.0, 115.0, |s| {
            s.rect(0.0, 0.0, 4.0, 115.0, 0.0, pal::BG);
            s.rect(1.0, 2.0, 2.0, 111.0, 1.0, fill);
        }));
    }
}

/// The 303's key band (this fork, as the reference picture): one black frame (x, y, w, h)
/// holding pitch mode and clear (12..46), the keys on light paper (46..242) and the flags with
/// the step LEDs (242..354), 1 px of black between them. The pitch/clear and the flags blocks
/// are black above and light below their splits.
pub const BAND_303: (f32, f32, f32, f32) = (12.0, 54.0, 342.0, 58.0);
const PAPER_X_303: (f32, f32) = (46.5, 241.5);
pub const KEYS_PAPER_303: &str = "#e4e5e9";
pub const PITCH_SPLIT_303: f32 = 88.0;
pub const FLAGS_SPLIT_303: f32 = 80.0;
pub const BLACK_NOTES_303: [(usize, u8); 5] = [(0, 1), (1, 3), (3, 6), (4, 8), (5, 10)];
/// The keyboard's lines: 21 px apart from the first, left of the first C, to the last, right
/// of the last C, down to just over the note names (their center height).
const LINE_X0_303: f32 = 59.75;
const LINE_END_303: f32 = 103.75;
const NOTE_Y_303: f32 = 108.0;
/// The black keys' slots: 13.5 wide, centered on a line, from the band's top.
const SLOT_W_303: f32 = 13.5;
const SLOT_END_303: f32 = 81.75;

/// The rows of the key band (tops): the black keys' LEDs, the black keys, the white keys' (and
/// flags') LEDs, the white keys and the flags.
pub const KEY_ROWS_303: [f32; 4] = [55.75, 65.0, 80.75, 90.0];
/// The widths of the white keys (and flags) and of the black keys, and their height.
pub const WHITE_KEY_W_303: f32 = 10.0;
pub const BLACK_KEY_W_303: f32 = 9.5;
pub const KEY_H_303: f32 = 14.5;
/// The tops of the pitch mode LED, the pitch mode key and the clear key, the keys' size, and
/// the centers of the PITCH and CLEAR labels.
pub const PITCH_LED_Y_303: f32 = 62.75;
pub const PITCH_Y_303: f32 = 72.0;
pub const CLEAR_Y_303: f32 = 95.0;
pub const PILL_W_303: f32 = 22.0;
pub const PILL_H_303: f32 = 12.0;
const PITCH_LABEL_Y_303: f32 = 57.6;
const CLEAR_LABEL_Y_303: f32 = 91.3;
/// The left edges of the 303's white keys (centered between the lines) and its flags (DOWN,
/// UP, ACCENT, SLIDE, NOTE), the flags 22 px apart.
pub const WHITE_303: [f32; 8] = [65.25, 86.25, 107.25, 128.25, 149.25, 170.25, 191.25, 212.25];
pub const FLAGS_303: [f32; 5] = [250.0, 272.0, 294.0, 316.0, 338.0];
/// The left edge of the black key right of white key `i`: centered on the line between them.
pub fn black_x_303(i: usize) -> f32 {
    LINE_X0_303 + 21.0 * (i + 1) as f32 - BLACK_KEY_W_303 / 2.0
}
/// The step LEDs (this fork, instead of the edit step display): 2 rows of 8 in the flags
/// block's top part, steps 1-8 above 9-16, left-aligned with the flags; their size and pitch.
/// The EDIT STEP label is over them at the block's top, the RNDM pill right of them.
pub const STEP_LED_303: f32 = 8.0;
const STEP_LED_PITCH_303: f32 = 9.0;
const STEP_LED_X_303: f32 = 246.5;
const STEP_LED_Y_303: f32 = 61.0;
const EDIT_LABEL_Y_303: f32 = 57.6;

/// The left and top of step `i`'s LED (0..=15).
pub fn step_led_303(i: usize) -> (f32, f32) {
    (
        STEP_LED_X_303 + STEP_LED_PITCH_303 * (i % 8) as f32,
        STEP_LED_Y_303 + STEP_LED_PITCH_303 * (i / 8) as f32,
    )
}

/// The RNDM button (this fork: the GENERATE menu's button, moved from the knob row): right of
/// the step LEDs, 4 px from the band's right edge (354), level with the two LED rows.
pub const RNDM_W_303: f32 = 29.0;
pub const RNDM_H_303: f32 = 15.0;
pub fn rndm_303() -> (f32, f32) {
    (354.0 - 4.0 - RNDM_W_303 + 0.25, STEP_LED_Y_303 + 1.0)
}
/// The five large knobs (CUTOFF to ACCENT): their size (`knob_303`) and top. TUNE is the small
/// knob under the waveform.
pub const KNOB_W_303: f32 = 27.0;
pub const KNOB_Y_303: f32 = 9.0;

/// How far a large knob's group reaches from its center: the row under it, left to the RATE
/// knob or its label, right to the AMOUNT knob or its label (the wider).
fn lfo_reach_303() -> (f32, f32) {
    let half = KNOBS[3].w / 2.0; // the 909's small knob (KNOBS[3])
    let cap = LFO_LABEL_CAP_303;
    let reach = |t: &str| LFO_DX_303 + half.max(font::text_width(cap, t) / 2.0);
    (reach("RATE"), reach("AMT"))
}

/// The left edge of large knob `k` (0 CUTOFF to 4 ACCENT). The six large knobs, TUNE first,
/// are evenly spaced (this fork): TUNE centered on the pitch/clear column (`LEFT_X_303`), and
/// the space from the section's left edge to TUNE's group (its knob, wider than the waveform
/// under it) the same as from the last group (its AMOUNT label) to the right edge (366).
pub fn knob_x_303(k: usize) -> f32 {
    let (_, right) = lfo_reach_303();
    let margin = LEFT_X_303 - KNOB_W_303.max(WAVE_W_303) / 2.0;
    let last = 366.0 - margin - right.max(KNOB_W_303 / 2.0);
    let pitch = (last - LEFT_X_303) / 5.0;
    LEFT_X_303 + pitch * (k + 1) as f32 - KNOB_W_303 / 2.0
}
/// The LFO row under each large knob (this fork): the mode buttons centered under it, the RATE
/// knob left of them and the AMOUNT knob right of them (the 909's small knob), 2 px apart,
/// each labeled above. The knobs' centers' offset from the large knob's center line and their
/// center height, the labels' center height and cap height, the mode buttons' top.
pub const LFO_DX_303: f32 = LFO_MODE_W / 2.0 + 1.0 + 6.0;
const LFO_KNOB_CY_303: f32 = 46.0;
const LFO_LABEL_Y_303: f32 = 37.3;
const LFO_LABEL_CAP_303: f32 = 3.0;
pub const LFO_MODE_Y_303: f32 = LFO_KNOB_CY_303 - LFO_MODE_H / 2.0;

/// The centers of large knob `k`'s RATE and AMOUNT knobs (0 CUTOFF to 4 ACCENT).
pub fn lfo_knobs_303(k: usize) -> ((f32, f32), (f32, f32)) {
    let cx = knob_x_303(k) + KNOB_W_303 / 2.0;
    (
        (cx - LFO_DX_303, LFO_KNOB_CY_303),
        (cx + LFO_DX_303, LFO_KNOB_CY_303),
    )
}

/// The TUNE knob: a large knob like the five others (`knob_303`, this fork), centered on the
/// column under its label in the knobs' label row, at their top; the waveform is under it.
pub const TUNE_Y_303: f32 = KNOB_Y_303;
const TUNE_LABEL_Y_303: f32 = 5.5;
/// The center x of the pitch mode and clear block.
pub const LEFT_X_303: f32 = 29.0;

/// The waveform switch under TUNE, centered on its column (this fork): as wide and as high as
/// the LFO mode buttons under the other knobs, and in their row; labeled above as they are.
pub const WAVE_W_303: f32 = LFO_MODE_W;
pub const WAVE_H_303: f32 = LFO_MODE_H;
pub const WAVE_X_303: f32 = LEFT_X_303 - WAVE_W_303 / 2.0;
pub const WAVE_Y_303: f32 = LFO_MODE_Y_303;
/// The waveform not chosen (a dull red, as an unlit LED) and the green glow of the chosen one
/// (and of the LFO's random sine and sync).
const WAVE_OFF: &str = "#6b302e";
const WAVE_GLOW: &str = "#53d86a40";
/// The LFO mode buttons (`lfo_mode_*`): their size (two equal cells, the note and the wave),
/// and the note's color when off (a dull blue).
pub const LFO_MODE_W: f32 = 2.0 * 12.0;
pub const LFO_MODE_H: f32 = 11.0;
const LFO_SYNC_OFF: &str = "#34557f";

fn tb303_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_tb303", 366.0, 115.0, |s| {
        face(s, 366.0, 115.0, KNOB_CAP_303);
        let ink = pal::BLACK;
        // the waveform switch under TUNE, labeled above it like the LFO row's knobs
        label(
            s,
            LEFT_X_303,
            LFO_LABEL_Y_303,
            LFO_LABEL_CAP_303,
            ink,
            "WAVEFORM",
        );
        // TUNE: the small knob over the waveform, labeled above it
        label(s, LEFT_X_303, TUNE_LABEL_Y_303, 3.6, ink, "TUNE");
        for (k, name) in ["CUTOFF", "RESO", "ENV MOD", "DECAY", "ACCENT"]
            .into_iter()
            .enumerate()
        {
            label(s, knob_x_303(k) + KNOB_W_303 / 2.0, 5.5, 3.6, ink, name);
        }
        // each large knob's LFO row: RATE and AMOUNT labeled above their knobs, SYNC above
        // the mode buttons between them
        let cap = LFO_LABEL_CAP_303;
        for k in 0..5 {
            let ((rx, _), (ax, _)) = lfo_knobs_303(k);
            // RATE, then SYN over the note (tempo sync) and WF over the wave, then AMT
            let mid = (rx + ax) / 2.0;
            label(s, rx, LFO_LABEL_Y_303, cap, ink, "RATE");
            label(s, mid - LFO_MODE_W / 4.0, LFO_LABEL_Y_303, cap, ink, "SYN");
            label(s, mid + LFO_MODE_W / 4.0, LFO_LABEL_Y_303, cap, ink, "WF");
            label(s, ax, LFO_LABEL_Y_303, cap, ink, "AMT");
        }
        // The key band (this fork, a keyboard as the reference picture): one black frame; the
        // pitch/clear and the flags blocks black above and light below; the keys on light
        // paper with lines between the white keys, the black keys in black slots, every key
        // named under it.
        let (bx, by, bw, bh) = BAND_303;
        s.rect(bx, by, bw, bh, 4.0, pal::BLACK);
        let bottom = by + bh - 1.0;
        let (px0, px1) = PAPER_X_303;
        s.rect(px0, by + 1.0, px1 - px0, bh - 2.0, 0.0, KEYS_PAPER_303);
        for (x0, x1, split) in [
            (bx + 1.0, px0 - 1.0, PITCH_SPLIT_303),
            (px1 + 1.0, bx + bw - 1.0, FLAGS_SPLIT_303),
        ] {
            s.rect(x0, split, x1 - x0, bottom - split, 3.0, KEYS_PAPER_303);
            s.rect(x0, split, x1 - x0, 4.0, 0.0, KEYS_PAPER_303);
        }
        for k in 0..9 {
            let x = LINE_X0_303 + 21.0 * k as f32;
            s.line(x, by + 1.0, x, LINE_END_303, "#727274", 0.7);
        }
        for (i, _) in BLACK_NOTES_303 {
            let x = black_x_303(i) + BLACK_KEY_W_303 / 2.0 - SLOT_W_303 / 2.0;
            s.rect(x, by, SLOT_W_303, SLOT_END_303 - by, 2.0, pal::BLACK);
        }
        let ink = pal::BLACK;
        let note_y = NOTE_Y_303;
        for (x, name) in WHITE_303
            .into_iter()
            .zip(["C", "D", "E", "F", "G", "A", "B", "C"])
        {
            label(s, x + WHITE_KEY_W_303 / 2.0, note_y, 2.4, ink, name);
        }
        for ((i, _), name) in BLACK_NOTES_303.into_iter().zip(["C", "D", "F", "G", "A"]) {
            // the sharp: the letter and a small # up right of it, centered together
            let x = black_x_303(i) + BLACK_KEY_W_303 / 2.0;
            let (lw, sw) = (font::text_width(2.4, name), font::text_width(1.6, "#"));
            let x0 = x - (lw + 0.4 + sw) / 2.0;
            text(s, x0, note_y, 2.4, Anchor::Start, ink, name);
            text(s, x0 + lw + 0.4, note_y - 0.9, 1.6, Anchor::Start, ink, "#");
        }
        // "EDIT STEP" at the flags block's top, over the step LEDs
        let (lx, _) = step_led_303(0);
        let (rx, _) = step_led_303(7);
        label(
            s,
            (lx + rx + STEP_LED_303) / 2.0,
            EDIT_LABEL_Y_303,
            3.2,
            pal::INK,
            "EDIT STEP",
        );
        label(s, LEFT_X_303, PITCH_LABEL_Y_303, 3.2, pal::INK, "PITCH");
        // on the light lower halves: black
        label(s, LEFT_X_303, CLEAR_LABEL_Y_303, 3.2, pal::BLACK, "CLEAR");
        for (x, name) in FLAGS_303
            .into_iter()
            .zip(["DOWN", "UP", "ACCENT", "SLIDE", "NOTE"])
        {
            label(
                s,
                x + WHITE_KEY_W_303 / 2.0,
                NOTE_Y_303,
                3.2,
                pal::BLACK,
                name,
            );
        }
    }));
}

const NAMES_808: [&str; 12] = [
    "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
];
/// The mod's window (x, y, w, h) in the 808's space right of the legend.
pub const MOD_VIEW_808: (f32, f32, f32, f32) = (278.0, 5.0, 84.0, 105.0);

/// The 808's look (this fork): twelve equal columns across the section, one per instrument
/// (AC to CH), divided by lines; each column's knobs centered in it; the instruments' names on
/// cream pills (the chosen one red), the conga/clave/maraca names on pills over their
/// switches; sixteen step keys under them with a light in each.
pub const COL808_X0: f32 = 6.0;
pub const COL808_W: f32 = 29.5;
pub const PILL808_W: f32 = 22.0;
pub const PILL808_H: f32 = 9.0;
pub const LEGEND808_Y: f32 = 75.5;
pub const UPPER808_Y: f32 = 46.0;
pub const STEP808_Y: f32 = 88.0;
pub const STEP808_W: f32 = 20.0;
pub const STEP808_H: f32 = 20.0;
pub const STEP808_PITCH: f32 = 22.125;
const PILL_CREAM: &str = "#f5ecbf";
const PILL_INK: &str = "#2a2723";

/// The center x of the 808's column `col` (0 AC .. 11 CH).
pub fn col808(col: usize) -> f32 {
    COL808_X0 + COL808_W * (col as f32 + 0.5)
}

/// The left x of the 808's step key `i` (0..=15), the sixteen across the columns' width.
pub fn step808_x(i: usize) -> f32 {
    COL808_X0 + STEP808_PITCH * i as f32 + (STEP808_PITCH - STEP808_W) / 2.0
}

fn pill(s: &mut Svg, x: f32, y: f32, fill: &str, ink: &str, text_: &str) {
    s.rect(x, y, PILL808_W, PILL808_H, 2.0, fill);
    label(s, x + PILL808_W / 2.0, y + PILL808_H / 2.0, 3.2, ink, text_);
}

fn tr808_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_tr808", 366.0, 115.0, |s| {
        face(s, 366.0, 115.0, pal::CHARCOAL);
        let ink = pal::INK;
        // the columns' dividers, down to the step keys
        for c in 1..12 {
            let x = COL808_X0 + COL808_W * c as f32;
            s.line(x, 3.0, x, STEP808_Y - 2.5, "#00000059", 0.8);
        }
        for c in 0..12 {
            label(s, col808(c), 5.0, 2.8, ink, "LEVEL");
        }
        label(s, col808(0), 27.7, 2.8, ink, "FLIP");
        for (c, t) in [
            (1, "TONE"),
            (2, "TONE"),
            (3, "TUNE"),
            (4, "TUNE"),
            (5, "TUNE"),
            (9, "TONE"),
        ] {
            label(s, col808(c), 27.7, 2.8, ink, t);
        }
        for (c, t) in [(1, "DECAY"), (2, "SNAPPY"), (9, "DECAY"), (10, "DECAY")] {
            label(s, col808(c), 50.7, 2.8, ink, t);
        }
        for (k, t) in ["LC", "MC", "HC", "CL", "MA"].into_iter().enumerate() {
            let x = col808(k + 3) - PILL808_W / 2.0;
            pill(s, x, UPPER808_Y, PILL_CREAM, PILL_INK, t);
        }
        for (c, name) in NAMES_808.into_iter().enumerate() {
            let x = col808(c) - PILL808_W / 2.0;
            pill(s, x, LEGEND808_Y, PILL_CREAM, PILL_INK, name);
        }
    }));
    // a chosen instrument's pill: red with its name in white
    for name in NAMES_808.into_iter().chain(["LC", "MC", "HC", "CL", "MA"]) {
        out.push(Part::new(
            format!("pill808_{name}_on"),
            PILL808_W,
            PILL808_H,
            move |s| pill(s, 0.0, 0.0, pal::RED, pal::WHITE, name),
        ));
    }
    // the 808's FLIP: a light key with a light, red when on
    for (state, on) in [("off", false), ("on", true)] {
        out.push(Part::new(format!("flip_{state}"), 13.0, 13.0, |s| {
            key_3d(s, 0.0, 0.0, 13.0, 13.0, 2.0, "#c4c7cb", pal::WHITE);
            s.circle(6.5, 6.5, 2.4, if on { pal::RED } else { "#7a7e84" });
            s.circle_stroke(6.5, 6.5, 2.4, "#00000040", 0.5);
        }));
    }
    // The mod's window (no longer on the panel, this fork; the part stays for the app's code).
    let (_, _, w, h) = MOD_VIEW_808;
    out.push(Part::new("mod_view", w, h, move |s| {
        s.rect(0.0, 0.0, w, h, 2.0, "#101216");
        s.rect_stroke(0.25, 0.25, w - 0.5, h - 0.5, 2.0, "#000000", 0.5);
    }));
}

/// The 909's instrument groups: their left edges in section pixels.
pub const EDGES_909: [f32; 13] = [
    4.0, 26.0, 68.0, 110.0, 152.0, 194.0, 236.0, 257.0, 278.0, 299.0, 320.0, 341.0, 362.0,
];
const NAMES_909: [&str; 12] = [
    "AC", "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CC", "RC",
];

/// Where a 909 knob sits: (instrument group, column, lower row, id, label). The column of a
/// two-knob-wide group is 0 (left half) or 1 (right half); 2 is the whole group.
pub const KNOBS_909: [(usize, usize, bool, &str, &str); 28] = [
    (0, 2, false, "ac_level", "LEVEL"),
    (1, 0, false, "bd_tune", "TUNE"),
    (1, 0, true, "bd_attack", "ATTACK"),
    (1, 1, false, "bd_level", "LEVEL"),
    (1, 1, true, "bd_decay", "DECAY"),
    (2, 0, false, "sd_tune", "TUNE"),
    (2, 0, true, "sd_tone", "TONE"),
    (2, 1, false, "sd_level", "LEVEL"),
    (2, 1, true, "sd_snappy", "SNAPPY"),
    (3, 0, false, "lt_tune", "TUNE"),
    (3, 0, true, "lt_decay", "DECAY"),
    (3, 1, false, "lt_level", "LEVEL"),
    (4, 0, false, "mt_tune", "TUNE"),
    (4, 0, true, "mt_decay", "DECAY"),
    (4, 1, false, "mt_level", "LEVEL"),
    (5, 0, false, "ht_tune", "TUNE"),
    (5, 0, true, "ht_decay", "DECAY"),
    (5, 1, false, "ht_level", "LEVEL"),
    (6, 2, false, "rs_level", "LEVEL"),
    (7, 2, false, "cp_level", "LEVEL"),
    (8, 2, true, "ch_decay", "DECAY"),
    (9, 2, false, "hh_level", "LEVEL"),
    (9, 2, true, "oh_decay", "DECAY"),
    (10, 2, false, "cc_level", "LEVEL"),
    (10, 2, true, "cc_tune", "TUNE"),
    (11, 2, false, "rc_level", "LEVEL"),
    (11, 2, true, "rc_tune", "TUNE"),
    (0, 2, true, "flam", "FLAM"),
];
/// The 909's rows: the knobs' tops (upper, lower), the name row's top and the steps' top.
pub const KNOB_Y_909: [f32; 2] = [29.0, 56.0];
pub const NAMES_Y_909: f32 = 75.0;
pub const STEPS_Y_909: f32 = 89.0;
/// The header pills' top and height, the name pills' height and the step keys' height.
const HEAD909_Y: f32 = 4.0;
const HEAD909_H: f32 = 10.0;
const NAME909_H: f32 = 10.0;
pub const STEP909_H: f32 = 22.0;
/// The FLAM knob's top: under the FLAM label, over the flam key (95..111).
const FLAM_Y_909: f32 = 80.0;

/// The top of a 909 knob (see `KNOBS_909`).
pub fn knob_y_909(group: usize, lower: bool) -> f32 {
    if group == 0 && lower {
        FLAM_Y_909
    } else {
        KNOB_Y_909[usize::from(lower)]
    }
}

/// The center x of a 909 knob column (see `KNOBS_909`).
pub fn column_909(group: usize, col: usize) -> f32 {
    let (x0, x1) = (EDGES_909[group], EDGES_909[group + 1]);
    match col {
        0 => x0 + (x1 - x0) / 4.0,
        1 => x0 + 3.0 * (x1 - x0) / 4.0,
        _ => (x0 + x1) / 2.0,
    }
}

/// The 909's unlit step light.
const LED_OFF_909: &str = "#c2bfb5";

/// A 909 key (`w` by `h`): a gray frame around a light face.
fn key909(s: &mut Svg, w: f32, h: f32, fill: &str) {
    s.rect(0.0, 0.0, w, h, 2.5, "#8e8b83");
    s.rect(1.0, 1.0, w - 2.0, h - 2.0, 2.0, fill);
    s.rect(1.6, 1.4, w - 3.2, 3.0, 1.5, "#ffffff59");
}

/// A 909 key's light: a small pill centered at (`cx`, `cy`).
fn led909(s: &mut Svg, cx: f32, cy: f32, color: &str) {
    if color != LED_OFF_909 {
        s.rect(cx - 4.6, cy - 2.6, 9.2, 5.2, 2.6, &format!("{color}40"));
    }
    s.rect(cx - 3.5, cy - 1.5, 7.0, 3.0, 1.5, color);
    s.rect_stroke(cx - 3.5, cy - 1.5, 7.0, 3.0, 1.5, "#00000040", 0.5);
}

/// The 909's header pill of group `i` (dark, its name orange) and its name pill (light).
/// Its top is at `y`.
fn head909(s: &mut Svg, x: f32, y: f32, w: f32, fill: &str, ink: &str, name: &str) {
    s.rect(x, y, w, HEAD909_H, 2.0, fill);
    label(s, x + w / 2.0, y + HEAD909_H / 2.0, 3.6, ink, name);
}

fn name909(s: &mut Svg, x: f32, y: f32, w: f32, fill: &str, edge: &str, name: &str) {
    s.rect(x, y, w, NAME909_H, 2.0, fill);
    s.rect_stroke(x + 0.3, y + 0.3, w - 0.6, NAME909_H - 0.6, 1.8, edge, 0.6);
    label(s, x + w / 2.0, y + NAME909_H / 2.0, 3.4, PILL_INK, name);
}

/// The x and the width of the pill of the 909's group `i` (0 AC .. 11 RC).
pub fn pill909(i: usize) -> (f32, f32) {
    (EDGES_909[i] + 0.5, EDGES_909[i + 1] - EDGES_909[i] - 1.0)
}

const HEAD909_DARK: &str = "#4a4a48";

fn tr909_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_tr909", 366.0, 115.0, |s| {
        face(s, 366.0, 115.0, pal::CREAM);
        let ink = pal::BLACK;
        for (i, name) in NAMES_909.into_iter().enumerate() {
            let (x, w) = pill909(i);
            head909(s, x, HEAD909_Y, w, HEAD909_DARK, pal::ORANGE, name);
            if i > 0 {
                name909(s, x, NAMES_Y_909, w, "#e6e4dd", "#b3afa5", name);
                let x0 = EDGES_909[i];
                s.line(x0, 17.5, x0, 70.0, "#b9b5ab", 0.8);
            }
        }
        for (group, col, lower, _, t) in KNOBS_909 {
            let y = knob_y_909(group, lower) - 3.7;
            label(s, column_909(group, col), y, 2.8, ink, t);
        }
        label(s, column_909(0, 2), KNOB_Y_909[1] - 3.7, 2.8, ink, "FLIP");
    }));
    // the chosen instrument (this fork): its name pill orange; the accent's header pill
    for (i, name) in NAMES_909.into_iter().enumerate() {
        let (_, w) = pill909(i);
        if i == 0 {
            out.push(Part::new("head909_AC_on", w, HEAD909_H, move |s| {
                head909(s, 0.0, 0.0, w, pal::ORANGE, PILL_INK, name)
            }));
        } else {
            out.push(Part::new(
                format!("name909_{name}_on"),
                w,
                NAME909_H,
                move |s| name909(s, 0.0, 0.0, w, pal::ORANGE, "#a85d14", name),
            ));
        }
    }
}

fn mixer_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_mixer", 70.0, 115.0, |s| {
        face(s, 70.0, 115.0, pal::SLATE);
        // this fork: the header holds the green LED (the channel's sound; a click mutes it),
        // the amber SOLO LED, MIX and the meter
        header(s, 2.0, 2.0, 66.0, 18.0, MIXER_HEADER);
        label(s, 43.0, 11.0, 4.4, pal::WHITE, "MIX");
        // pan and delay send side by side; the fader and the routing keys below
        label(s, 19.0, 52.5, 3.0, pal::INK_DIM, "PAN");
        label(s, 51.0, 52.5, 3.0, pal::INK_DIM, "DELAY");
    }));
}

fn master_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_master", 82.0, 98.0, |s| {
        face(s, 82.0, 98.0, pal::SLATE);
        header(s, 2.0, 2.0, 78.0, 16.0, pal::GOLD);
        label(s, 41.0, 10.0, 4.4, pal::BLACK, "MASTER");
        for (f, t) in [
            (0.06, "CLIP"),
            (0.24, "-6"),
            (0.44, "-12"),
            (0.68, "-24"),
            (0.92, "-36"),
        ] {
            let y = 24.0 + 56.0 * f;
            text(s, 14.5, y, 2.8, Anchor::End, pal::INK_DIM, t);
            text(s, 67.5, y, 2.8, Anchor::Start, pal::INK_DIM, t);
        }
    }));
}

/// An effect's header: the title centered between the power button (4..18) and the meter
/// (70..78).
fn fx_head(s: &mut Svg, w: f32, title: &str) {
    header(s, 2.0, 2.0, w - 4.0, 18.0, pal::BLUE);
    label(s, 44.0, 11.0, 4.4, pal::WHITE, title);
}

fn fx_bg(out: &mut Vec<Part>) {
    out.push(Part::new("bg_pcf", 82.0, 111.0, |s| {
        face(s, 82.0, 111.0, pal::SLATE);
        fx_head(s, 82.0, "PCF");
        well(s, 10.0, 23.0, 18.0, 16.0);
        label(s, 19.0, 43.5, 2.8, pal::INK_DIM, "PATTERN");
        text(s, 60.0, 26.5, 3.2, Anchor::Start, pal::INK_DIM, "BP");
        text(s, 60.0, 35.5, 3.2, Anchor::Start, pal::INK_DIM, "LP");
        label(s, 53.5, 43.5, 2.8, pal::INK_DIM, "MODE");
        for (x, t) in [(11.5, "FREQ"), (30.5, "Q"), (49.5, "AMT"), (68.5, "DEC")] {
            label(s, x, 104.0, 3.0, pal::INK_DIM, t);
        }
    }));
    out.push(Part::new("bg_delay", 82.0, 93.0, |s| {
        face(s, 82.0, 93.0, pal::SLATE);
        fx_head(s, 82.0, "DELAY");
        well(s, 9.0, 23.0, 19.0, 16.0);
        label(s, 18.5, 43.5, 2.8, pal::INK_DIM, "STEPS");
        text(s, 60.0, 26.5, 3.2, Anchor::Start, pal::INK_DIM, "3");
        text(s, 60.0, 35.5, 3.2, Anchor::Start, pal::INK_DIM, "16");
        label(s, 53.5, 43.5, 2.8, pal::INK_DIM, "MODE");
        label(s, 21.0, 81.5, 3.0, pal::INK_DIM, "PAN");
        label(s, 62.0, 81.5, 3.0, pal::INK_DIM, "FEEDBACK");
    }));
    out.push(Part::new("bg_dist", 82.0, 65.0, |s| {
        face(s, 82.0, 65.0, pal::SLATE);
        fx_head(s, 82.0, "DIST");
        label(s, 21.0, 53.5, 3.0, pal::INK_DIM, "AMOUNT");
        label(s, 62.0, 53.5, 3.0, pal::INK_DIM, "SHAPE");
    }));
    out.push(Part::new("bg_comp", 82.0, 93.0, |s| {
        face(s, 82.0, 93.0, pal::SLATE);
        fx_head(s, 82.0, "COMP");
        // under the reduction meter's LEDs 1, 3, 5, 7 and 9 (centered at 41)
        for (i, t) in ["-40", "-20", "0", "+20", "+40"].into_iter().enumerate() {
            label(s, 13.0 + 14.0 * i as f32, 39.5, 2.8, pal::INK_DIM, t);
        }
        label(s, 21.0, 80.0, 3.0, pal::INK_DIM, "RATIO");
        label(s, 62.0, 80.0, 3.0, pal::INK_DIM, "THRES");
    }));
}

pub fn all_parts() -> Vec<Part> {
    let mut v = Vec::new();
    wood(&mut v);
    transport_bg(&mut v);
    logo(&mut v);
    pattern_bg(&mut v);
    tb303_bg(&mut v);
    tr808_bg(&mut v);
    tr909_bg(&mut v);
    mixer_bg(&mut v);
    master_bg(&mut v);
    fx_bg(&mut v);
    knobs(&mut v);
    fader(&mut v);
    switches(&mut v);
    power_buttons(&mut v);
    transport_buttons(&mut v);
    pattern_buttons(&mut v);
    small_buttons(&mut v);
    leds(&mut v);
    steps(&mut v);
    digits(&mut v);
    meters(&mut v);
    app_icon(&mut v);
    v
}
