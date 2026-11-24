//! Where the controls and the section backgrounds sit, in panel pixels
//! (`assets/ui/layout.tsv` and the README's background table).

pub const PANEL_W: f32 = 624.0;
pub const PANEL_H: f32 = 510.0;

const LAYOUT_TSV: &str = include_str!("../../../assets/ui/layout.tsv");

/// A control's rectangle and the part that draws its default state.
#[derive(Clone, Debug)]
pub struct Ctl {
    pub id: String,
    pub section: String,
    pub kind: String,
    pub part: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Ctl {
    /// The part's name without its state: `btn_play_lit` -> `btn_play_`. Not for knobs,
    /// faders and displays, whose parts are composed.
    pub fn family(&self) -> &str {
        match self.part.rfind('_') {
            Some(i) => &self.part[..=i],
            None => &self.part,
        }
    }

    /// The control's name within its section (`tb303_1.cutoff` -> `cutoff`).
    pub fn name(&self) -> &str {
        self.id.split_once('.').map_or(&self.id, |(_, n)| n)
    }
}

/// Parses `layout.tsv`.
pub fn controls() -> Vec<Ctl> {
    LAYOUT_TSV
        .lines()
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            let n = |i: usize| f[i].parse::<f32>().expect("layout.tsv number");
            Ctl {
                id: f[0].into(),
                section: f[1].into(),
                kind: f[2].into(),
                part: f[3].into(),
                x: n(4),
                y: n(5),
                w: n(6),
                h: n(7),
            }
        })
        .collect()
}

/// The section backgrounds drawn first: (part, x, y). The row focus bars are not here.
pub fn backgrounds() -> Vec<(&'static str, f32, f32)> {
    let mut v = vec![
        ("wood", 0.0, 0.0),
        ("wood", 608.0, 0.0),
        ("bg_transport", 16.0, 0.0),
        ("logo", 482.0, 8.0),
    ];
    for r in 0..4 {
        let y = 50.0 + 115.0 * r as f32;
        v.push(("bg_pattern", 16.0, y));
        v.push((
            match r {
                0 | 1 => "bg_tb303",
                2 => "bg_tr808",
                _ => "bg_tr909",
            },
            90.0,
            y,
        ));
        v.push(("bg_mixer", 456.0, y));
    }
    v.push(("bg_master", 526.0, 50.0));
    // this fork's order: DELAY, DIST, PCF, COMP under the master
    // this fork: the four effects slots, 90 px each
    v.push(("bg_delay", 526.0, 150.0));
    // the second delay's face, drawn when two slots hold the delay
    v.push(("bg_delay_b", 526.0, 150.0));
    v.push(("bg_dist", 526.0, 240.0));
    v.push(("bg_pcf", 526.0, 330.0));
    v.push(("bg_comp", 526.0, 420.0));
    // the other second effects' faces (this fork), drawn when two slots hold the effect
    v.push(("bg_dist_b", 526.0, 240.0));
    v.push(("bg_pcf_b", 526.0, 330.0));
    v.push(("bg_comp_b", 526.0, 420.0));
    v
}

/// The focus bar of row `r` (0..=3): (x, y, w, h).
pub fn focus_bar(r: usize) -> (f32, f32, f32, f32) {
    (86.0, 50.0 + 115.0 * r as f32, 4.0, 115.0)
}

/// The row (0..=3) containing a panel point, as the window's click handler tests it.
pub fn row_at(x: f32, y: f32) -> Option<usize> {
    if !(16.0..526.0).contains(&x) || !(50.0..510.0).contains(&y) {
        return None;
    }
    Some(((y - 50.0) / 115.0) as usize)
}

/// Whether a panel point is on the logo, the hit rectangle of `sub_42A799`
/// (`sub_479B06(458, 1, 608, 49)` in transport coordinates, clipped to the transport: panel x
/// 474..607, y 1..48; docs/ui_spec.md section 3); this fork: the ORB-338X logo's own box
/// (orb-ui-art's `LOGO_X`, `LOGO_Y`, `LOGO_W`, `LOGO_H`), panel x 482..597, y 8..38.
pub fn on_logo(x: f32, y: f32) -> bool {
    (482.0..598.0).contains(&x) && (8.0..39.0).contains(&y)
}

/// The origins of the drum machine sections (panel pixels), for pointer math.
pub const TR808_ORIGIN: (f32, f32) = (90.0, 280.0);
pub const TR909_ORIGIN: (f32, f32) = (90.0, 395.0);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_has_every_control() {
        let c = controls();
        // (this fork: 16 step LEDs replace the edit step display and its two arrows on each
        // 303, 380 + 2 * 13; each mixer channel's on/off button is now its green LED (MUTE),
        // and it has an amber SOLO LED, + 4; the 909's FLIP, + 1)
        // (this fork's FLIP faces: KIT, 11 PAN knobs and 11 FX keys each, + 46; each 303's SUB
        // knob, + 2; the master's LIMIT key and AMT knob in place of its COMP key, + 1; the
        // mixer's slot knobs in place of its keys and their labels, + 16; the slots' menus, + 4;
        // the second delay's face, + 9, and its sends and their labels, + 8; the other second
        // effects' faces and knobs, + 54; the channels' EQ knobs, + 12)
        assert_eq!(c.len(), 563);
        assert!(c.iter().all(|c| c.w > 0.0 && c.h > 0.0));
        let play = c.iter().find(|c| c.id == "transport.play").unwrap();
        // the mockup's play key (this fork, orb-ui-art's cuts.rs)
        assert_eq!((play.x, play.y, play.w, play.h), (113.0, 22.0, 30.0, 23.0));
        assert_eq!(play.family(), "btn_play_");
        assert_eq!(play.name(), "play");
    }

    #[test]
    fn backgrounds_match_the_art_generator() {
        let parts = orb_ui_art::parts::all_parts();
        let l = orb_ui_art::layout::layout(&parts);
        let mut want: Vec<(String, f32, f32)> = l
            .backgrounds
            .into_iter()
            // the FLIP faces' backgrounds replace the drum machines' (Panel::background)
            .filter(|(n, _, _)| !n.starts_with("focus_") && !n.ends_with("_flip"))
            .collect();
        let mut got: Vec<(String, f32, f32)> = backgrounds()
            .into_iter()
            .map(|(n, x, y)| (n.to_string(), x, y))
            .collect();
        want.sort_by(|a, b| a.partial_cmp(b).unwrap());
        got.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(got, want);
    }

    #[test]
    fn rows() {
        assert_eq!(row_at(100.0, 60.0), Some(0));
        assert_eq!(row_at(100.0, 509.0), Some(3));
        assert_eq!(row_at(10.0, 60.0), None);
        assert_eq!(row_at(100.0, 20.0), None);
    }

    #[test]
    fn the_logo() {
        assert!(on_logo(482.0, 8.0) && on_logo(597.9, 38.9));
        assert!(!on_logo(481.9, 20.0) && !on_logo(598.0, 20.0));
        assert!(!on_logo(500.0, 7.9) && !on_logo(500.0, 39.0));
        // the logo part covers exactly the click area
        use orb_ui_art::parts::{LOGO_H, LOGO_W, LOGO_X, LOGO_Y};
        assert_eq!((LOGO_X, LOGO_Y), (482.0, 8.0));
        assert!(on_logo(LOGO_X, LOGO_Y) && on_logo(LOGO_X + LOGO_W - 0.1, LOGO_Y + LOGO_H - 0.1));
        assert!(!on_logo(LOGO_X + LOGO_W, LOGO_Y) && !on_logo(LOGO_X, LOGO_Y + LOGO_H));
    }
}
