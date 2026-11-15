//! The windows' font (this fork): Inter (`assets/fonts`, SIL Open Font License 1.1, see
//! `assets/fonts/Inter-LICENSE.txt`) for every proportional text of egui: the menus, the
//! dialogs, the GENERATE menu, the status line. Monospace stays egui's own. The panel's labels
//! are drawn in Inter too, as outlines in the art (orb-ui-art's `font.rs`).

use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

/// Inter Medium: a little heavier than Regular, so the small light-on-dark text stays clear.
pub const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");

/// The font definitions with Inter first for proportional text.
pub fn definitions() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("Inter".into(), Arc::new(FontData::from_static(INTER)));
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter".into());
    fonts
}

/// Sets Inter as the context's proportional font (once, at the start).
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(definitions());
}
