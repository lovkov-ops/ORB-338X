//! Help > About (task-102, laid out in task-115): the icon, the wordmark, the version and the
//! project link in a small centered window. The version string is computed by `build.rs`
//! (`crate::version`).

use eframe::egui::{self, Context, Key, Modifiers, RichText};

use crate::app::{self, App};
use crate::logo_anim::{self, LOGO_H, LOGO_W};
use crate::palette::INK_DIM;

/// The wordmark part of the panel (`logo_anim::LOGO_PART`) is shown at this scale.
const LOGO_SCALE: f32 = 2.0;
/// The app icon part and the size it is shown at.
pub const ICON_PART: &str = "app_icon";
const ICON_SIZE: f32 = 64.0;
/// The content area below the title bar and the x of the version and the links. This fork
/// sets the text in Inter, wider than egui's own font: the lines start under the icon (its x,
/// 18) instead of under the wordmark, and the window is 470 wide, so the longest line
/// ("Song collection: <link>") fits with room to spare.
const CONTENT_W: f32 = 470.0;
/// The width of the label column of the collection lines (the links start at TEXT_X + this).
const LABEL_W: f32 = 120.0;
/// The lines under the wordmark at 22 pt from y 104 (this fork's order): the fork's credit,
/// the four `LINKS`, the version.
const CONTENT_H: f32 = 244.0;
const TEXT_X: f32 = 18.0;

/// The name of the app.
pub const NAME: &str = "OpenRB-338x";
/// The page of the original project's creator (this fork's About names them under its own
/// credit).
pub const CREATOR_URL: &str = "https://github.com/pingw33n";
/// Where the latest release of this fork is downloaded.
pub const DOWNLOAD_URL: &str = "https://github.com/lovkov-ops/OpenRB-338x/releases/latest";
/// The credit of this fork, the first line under the wordmark: the text, then the fork's
/// author linked to the fork.
pub const FORK_CREDIT: (&str, &str, &str) = (
    "Xtended features injected by",
    "lovkov-ops",
    "https://github.com/lovkov-ops/OpenRB-338x",
);
/// The user's public collections, shown under the project page as `label url` (task-138).
/// The user's song collection.
pub const SONGS_URL: &str = "https://github.com/pingw33n/rb-songs";
/// The user's mod collection.
pub const MODS_URL: &str = "https://github.com/pingw33n/rb-mods";
/// The label and the link of each collection line.
pub const COLLECTIONS: [(&str, &str); 2] = [
    ("Song collection:", SONGS_URL),
    ("Mod collection:", MODS_URL),
];
/// The link lines under the fork's credit, in order, as (label, link text, url): the download
/// of this fork's latest release (its long address shown as words), the original creator, the
/// collections.
pub const LINKS: [(&str, &str, &str); 4] = [
    ("Download:", "OpenRB-338x latest release", DOWNLOAD_URL),
    ("Original creator:", CREATOR_URL, CREATOR_URL),
    (COLLECTIONS[0].0, COLLECTIONS[0].1, COLLECTIONS[0].1),
    (COLLECTIONS[1].0, COLLECTIONS[1].1, COLLECTIONS[1].1),
];
/// The version: `YYYY.MMDD.HHMM (<[dev-]hash[-dirty]>)` (`src/version.rs`).
pub const VERSION: &str = env!("ORB_UI_VERSION");

/// The id of the About window of an open (`App::about_serial`).
pub fn about_window_id(serial: u32) -> egui::Id {
    egui::Id::unique(("orb_about_window", serial))
}

impl App {
    /// Help > About.
    pub fn show_about(&mut self) {
        if !self.about_open {
            // a new id: no stored place, so the window opens at its default, the center
            self.about_serial += 1;
            // the logo animation starts with the window's first frame (`about_ui`)
            self.about_start_due = true;
        }
        self.about_open = true;
    }

    /// The id of the About window now (for tests, to look its place up).
    pub fn about_id(&self) -> egui::Id {
        about_window_id(self.about_serial)
    }

    /// Whether the About window is open.
    pub fn about_is_open(&self) -> bool {
        self.about_open
    }

    /// Draws the About window when it is open. Escape and the close button close it.
    pub fn about_ui(&mut self, ctx: &Context) {
        if !self.about_open {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.about_open = false;
            return;
        }
        let now = ctx.input(|i| i.time);
        if std::mem::take(&mut self.about_start_due) {
            self.about_start = Some(now);
        }
        let t = logo_anim::time_now(now, self.about_start.filter(|_| self.logo_anim));
        let mut open = true;
        // centered the way File > Info is (info.rs): pivot, default place, an id per open
        egui::Window::new(format!("About {NAME}"))
            .id(self.about_id())
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(0.0))
            .show(ctx, |ui| {
                // The layout of the mockup session (task-115), points from the top left of
                // the content: the icon, the wordmark at 2x, and under its subtitle the
                // version and the link, left-aligned. The images are SVG, so the loader
                // rasterizes them at the window's pixel density.
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(CONTENT_W, CONTENT_H), egui::Sense::hover());
                let at = |x: f32, y: f32| rect.min + egui::vec2(x, y);
                // the icon fades in with the logo's animation (task-114)
                let icon_a = t.map_or(1.0, |t| logo_anim::Frame::at(t).icon);
                app::paint_part_tinted(
                    ui,
                    ICON_PART,
                    egui::Rect::from_min_size(at(18.0, 18.0), egui::vec2(ICON_SIZE, ICON_SIZE)),
                    egui::Color32::WHITE.gamma_multiply(icon_a),
                );
                let logo_size = egui::vec2(LOGO_W, LOGO_H) * LOGO_SCALE;
                logo_anim::paint_logo(ui, egui::Rect::from_min_size(at(92.0, 14.0), logo_size), t);
                let line_at = |ui: &mut egui::Ui, x: f32, y: f32, f: &dyn Fn(&mut egui::Ui)| {
                    let r = egui::Rect::from_min_size(at(x, y), egui::vec2(CONTENT_W - x, 18.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r), f);
                };
                let line = |ui: &mut egui::Ui, y: f32, f: &dyn Fn(&mut egui::Ui)| {
                    line_at(ui, TEXT_X, y, f);
                };
                // the fork's credit first, then the links, the version last
                line(ui, 104.0, &|ui| {
                    let (text, who, url) = FORK_CREDIT;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(RichText::new(text).color(INK_DIM));
                        ui.hyperlink_to(who, url);
                    });
                });
                for (i, (label, text, url)) in LINKS.into_iter().enumerate() {
                    // the label and the link each in a rect of their own, so the links start at
                    // exactly the same x whatever the width of the label
                    let y = 126.0 + 22.0 * i as f32;
                    line(ui, y, &|ui| {
                        ui.label(RichText::new(label).color(INK_DIM));
                    });
                    line_at(ui, TEXT_X + LABEL_W, y, &|ui| {
                        ui.hyperlink_to(text, url);
                    });
                }
                line(ui, 126.0 + 22.0 * LINKS.len() as f32, &|ui| {
                    ui.label(RichText::new(format!("Version {VERSION}")).color(INK_DIM));
                });
            });
        if !open {
            self.about_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art;
    use crate::logo_anim::LOGO_PART;

    /// The About window shows the panel's own wordmark part, at its aspect ratio.
    #[test]
    fn the_logo_part_is_the_panels_wordmark() {
        let part = art::part(LOGO_PART).expect("the logo part");
        let svg = std::str::from_utf8(part.svg).unwrap();
        let want =
            format!("width=\"{LOGO_W}\" height=\"{LOGO_H}\" viewBox=\"0 0 {LOGO_W} {LOGO_H}\"");
        assert!(svg.contains(&want), "{want} in the part's header");
        // the panel draws the same part over its transport background
        assert!(
            crate::layout::backgrounds()
                .iter()
                .any(|b| b.0 == LOGO_PART)
        );
    }
}
