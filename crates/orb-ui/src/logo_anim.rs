//! The startup animation of the logo ORB-338X (task-114, redone for this fork with the user's
//! top bar logo): the pieces appear one after another, each fading in:
//!
//! - the letters O, R, B, -, 3, 3, 8, X, each over [`LETTER_FADE_S`], [`LETTER_STEP_S`] apart from
//!   0;
//! - the subtitle, whole, over [`SUB_FADE_S`] from [`SUB_START_S`];
//! - last the icon, over [`ICON_FADE_S`] from [`ICON_START_S`], with a soft flash: a glow around it
//!   that rises with it and dies away by [`END_S`].
//!
//! The whole animation lasts as long as the startup sound ([`END_S`], 82 688 frames at
//! 44.1 kHz). It plays once per app start on the panel and at every open of the About window.
//! The final frame is the whole `logo` part itself, so it equals the static logo exactly. The
//! pieces are the `LOGO_PIECES` of orb-ui-art, each on the whole logo canvas.
//!
//! Developer option: `ORB_LOGO_T=SECONDS` freezes the animation at that time (for screenshots
//! of a moment); it applies to the panel and to the About window alike.

use eframe::egui::{self, Color32, Rect, Ui};

use crate::app;

/// The letters' pieces, in the order they appear.
pub const LETTERS: [&str; 8] = [
    "logo_o",
    "logo_r",
    "logo_b",
    "logo_dash",
    "logo_3a",
    "logo_3b",
    "logo_8",
    "logo_x",
];
/// Each letter's start after the one before it, and its fade.
pub const LETTER_STEP_S: f32 = 0.11;
pub const LETTER_FADE_S: f32 = 0.12;
/// The subtitle's start (after the X is in) and its fade.
pub const SUB_START_S: f32 = 0.95;
pub const SUB_FADE_S: f32 = 0.25;
/// The icon's start (after the subtitle is in) and its fade; its flash dies away by [`END_S`].
pub const ICON_START_S: f32 = 1.25;
pub const ICON_FADE_S: f32 = 0.15;
/// The end of the whole animation: the startup sound's length (`startup::sample`, a test checks
/// it).
pub const END_S: f32 = 82_688.0 / 44_100.0;

/// The whole logo part and its size (a test checks them against orb-ui-art's).
pub const LOGO_PART: &str = "logo";
pub const LOGO_W: f32 = 116.0;
pub const LOGO_H: f32 = 31.0;
/// The center of the icon in the logo part, where it flashes, and its size (orb-ui-art's
/// `LOGO_ICON`; a test checks it).
pub const ICON_CENTER: (f32, f32) = (101.0, 17.0);
pub const ICON_SIZE: f32 = 24.0;

/// The animation's state at one moment: each piece's opacity, and the icon's flash (0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    /// The letters' opacities, in [`LETTERS`] order.
    pub letters: [f32; 8],
    pub sub: f32,
    pub icon: f32,
    pub flash: f32,
}

/// 0 before `t0`, 1 after `t0 + d`, linear between.
fn ramp(t: f32, t0: f32, d: f32) -> f32 {
    ((t - t0) / d).clamp(0.0, 1.0)
}

impl Frame {
    /// The state `t` seconds after the start.
    pub fn at(t: f32) -> Self {
        let mut letters = [0.0; 8];
        for (i, a) in letters.iter_mut().enumerate() {
            *a = ramp(t, LETTER_STEP_S * i as f32, LETTER_FADE_S);
        }
        // the flash rises with the icon and fades out after it, eased so it stays soft
        let peak = ICON_START_S + ICON_FADE_S;
        let flash = if t < peak {
            ramp(t, ICON_START_S, ICON_FADE_S)
        } else {
            let k = 1.0 - ramp(t, peak, END_S - peak);
            k * k
        };
        Self {
            letters,
            sub: ramp(t, SUB_START_S, SUB_FADE_S),
            icon: ramp(t, ICON_START_S, ICON_FADE_S),
            flash,
        }
    }
}

/// The frozen time of `ORB_LOGO_T`, if set.
pub fn frozen_time() -> Option<f32> {
    static T: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("ORB_LOGO_T").ok()?.trim().parse().ok())
}

/// The animation time to draw now for an animation started at `start` (egui time): None when
/// it is over or off (the final state), else the seconds since the start. A frozen time wins.
pub fn time_now(now: f64, start: Option<f64>) -> Option<f32> {
    let t = match (frozen_time(), start) {
        (Some(t), _) => t,
        (None, Some(s)) => (now - s) as f32,
        (None, None) => return None,
    };
    (t < END_S).then_some(t.max(0.0))
}

/// One piece (or the whole part) at `alpha`, through the app's capped part painter
/// (`app::paint_part_tinted`, the texture-size cap of task-119).
fn paint_piece(ui: &Ui, name: &str, rect: Rect, alpha: f32) {
    if alpha > 0.0 {
        app::paint_part_tinted(ui, name, rect, Color32::WHITE.gamma_multiply(alpha));
    }
}

/// Paints the logo into `rect` (the logo canvas scaled): the whole part when `t` is None (the
/// final state), else the pieces at `t` seconds, and asks for the next frame.
pub fn paint_logo(ui: &mut Ui, rect: Rect, t: Option<f32>) {
    let Some(t) = t else {
        paint_piece(ui, LOGO_PART, rect, 1.0);
        return;
    };
    if frozen_time().is_none() {
        ui.ctx().request_repaint();
    }
    let k = rect.width() / LOGO_W;
    let f = Frame::at(t);
    for (name, a) in LETTERS.iter().zip(f.letters) {
        paint_piece(ui, name, rect, a);
    }
    paint_piece(ui, "logo_sub", rect, f.sub);
    if f.flash > 0.0 {
        // a soft glow behind the icon, and the icon itself a little larger and fainter over it
        let c = rect.min + egui::vec2(ICON_CENTER.0, ICON_CENTER.1) * k;
        let p = ui.painter();
        let r0 = ICON_SIZE / 2.0;
        for (r, a) in [(r0 + 7.0, 0.08), (r0 + 4.0, 0.13), (r0 + 1.5, 0.18)] {
            p.circle_filled(c, r * k, Color32::WHITE.gamma_multiply(a * f.flash));
        }
        let grow = 1.0 + 0.12 * f.flash;
        let big = Rect::from_min_size(c + (rect.min - c) * grow, rect.size() * grow);
        paint_piece(ui, "logo_icon", big, 0.5 * f.flash);
    }
    paint_piece(ui, "logo_icon", rect, f.icon);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// The pieces and the icon's place are the art's, and the animation lasts as long as the
    /// startup sound.
    #[test]
    fn the_pieces_are_the_arts() {
        use orb_ui_art::parts::{LOGO_ICON, LOGO_PIECES};
        assert_eq!(orb_ui_art::parts::LOGO_W, LOGO_W);
        assert_eq!(orb_ui_art::parts::LOGO_H, LOGO_H);
        assert_eq!(LOGO_PIECES[..8], LETTERS);
        assert_eq!(LOGO_PIECES[8..], ["logo_sub", "logo_icon"]);
        let (x, y, size) = LOGO_ICON;
        assert_eq!((x + size / 2.0, y + size / 2.0), ICON_CENTER);
        assert_eq!(size, ICON_SIZE);
        let sound = crate::startup::sample();
        assert!(close(END_S, sound.frames() as f32 / 44_100.0));
    }

    /// Runs the panel for a few frames at egui times from `t0` (so the SVGs are loaded) and
    /// returns the shapes of the last one.
    fn painted(ctx: &egui::Context, app: &mut crate::app::App, t0: f64) -> String {
        let mut last = String::new();
        for i in 0..6 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(crate::layout::PANEL_W, crate::layout::PANEL_H),
                )),
                time: Some(t0 + 0.016 * i as f64),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| app.show(ui));
            // the ids of the managed textures differ between runs: compare the rest
            last = format!("{:?}", out.shapes);
            while let Some(p) = last.find("Managed(") {
                let end = p + last[p..].find(')').unwrap() + 1;
                last.replace_range(p..end, "Texture");
            }
            out.textures_delta.clear();
        }
        last
    }

    /// After the animation the panel paints exactly what it paints with no animation (the
    /// static wordmark: the same shapes), and in the middle it paints something else.
    #[test]
    fn the_end_state_paints_the_static_wordmark() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut still = crate::app::App::new(crate::model::Panel::new(), None);
        let want = painted(&ctx, &mut still, 100.0);
        let mut anim = crate::app::App::new(crate::model::Panel::new(), None);
        anim.logo_anim = true;
        anim.logo_start = Some(100.0);
        if frozen_time().is_none() {
            let mid = painted(&ctx, &mut anim, 100.5);
            assert_ne!(mid, want, "mid-animation differs from the static logo");
            let got = painted(&ctx, &mut anim, 100.0 + END_S as f64 + 0.1);
            let i = got
                .bytes()
                .zip(want.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            let lo = i.saturating_sub(150);
            assert!(
                got == want,
                "differ at {i} of {}/{}: {:?} vs {:?}",
                got.len(),
                want.len(),
                &got[lo..(i + 150).min(got.len())],
                &want[lo..(i + 150).min(want.len())]
            );
        }
    }

    #[test]
    fn the_start() {
        let f = Frame::at(0.0);
        assert_eq!(f.letters, [0.0; 8]);
        assert_eq!((f.sub, f.icon, f.flash), (0.0, 0.0, 0.0));
    }

    /// O, R, B, -, 3, 3, 8, X one after another, then the subtitle, then the icon: each
    /// starts when the one before it is (nearly) in.
    #[test]
    fn the_pieces_come_in_order() {
        for i in 0..8 {
            let t0 = LETTER_STEP_S * i as f32;
            assert_eq!(Frame::at(t0).letters[i], 0.0, "{i}");
            assert!(
                close(Frame::at(t0 + LETTER_FADE_S / 2.0).letters[i], 0.5),
                "{i}"
            );
            if i > 0 {
                assert!(Frame::at(t0).letters[i - 1] > 0.9, "{i}");
            }
        }
        let x_in = LETTER_STEP_S * 7.0 + LETTER_FADE_S;
        assert!(x_in <= SUB_START_S);
        assert_eq!(Frame::at(SUB_START_S).sub, 0.0);
        assert!(close(Frame::at(SUB_START_S + SUB_FADE_S / 2.0).sub, 0.5));
        const { assert!(SUB_START_S + SUB_FADE_S <= ICON_START_S) };
        assert_eq!(Frame::at(ICON_START_S).icon, 0.0);
        assert!(close(Frame::at(ICON_START_S + ICON_FADE_S).icon, 1.0));
    }

    /// The icon's flash rises with it, is full when the icon is in, and dies away softly by
    /// the end.
    #[test]
    fn the_icon_flashes_softly() {
        let peak = ICON_START_S + ICON_FADE_S;
        assert_eq!(Frame::at(ICON_START_S).flash, 0.0);
        assert!(close(
            Frame::at(ICON_START_S + ICON_FADE_S / 2.0).flash,
            0.5
        ));
        assert!(close(Frame::at(peak).flash, 1.0));
        let a = Frame::at(peak + 0.1).flash;
        let b = Frame::at(peak + 0.3).flash;
        assert!(a < 1.0 && b < a && b > 0.0, "{a} {b}");
        assert_eq!(Frame::at(END_S).flash, 0.0);
    }

    /// At the end every piece is in its final state, and nothing changes after it.
    #[test]
    fn the_final_state() {
        let end = Frame::at(END_S);
        assert_eq!(end.letters, [1.0; 8]);
        assert_eq!((end.sub, end.icon, end.flash), (1.0, 1.0, 0.0));
        assert_eq!(Frame::at(10.0), end);
        // the clock: over (the whole part is painted) at END_S, none without a start
        assert_eq!(time_now(5.0, None), None);
        if frozen_time().is_none() {
            assert_eq!(time_now(5.0 + END_S as f64, Some(5.0)), None);
            assert!(time_now(5.5, Some(5.0)).is_some());
        }
    }
}
