//! The KIT menu of the drum machines' FLIP faces (this fork): one drum machine plays another
//! kit (the built-in one or a mod of the Mods folder) than the song's, the other keeps its own.
//!
//! The song's kit (the Mods menu, saved in the song) stays what it is; a machine's own kit
//! lasts until the song changes (a new or opened song, or a kit chosen in the Mods menu, which
//! both machines take). `.rbsx` songs keep it (rbsx.rs).

use eframe::egui::{self, Response};
use orb_cli::mods::{Kit, STANDARD};
use orb_engine::song::Device;

use crate::app::App;

/// The drum machines, by the panel's index (0 the 808, 1 the 909).
pub const MACHINES: [Device; 2] = [Device::Tr808, Device::Tr909];

/// What a click in the KIT menu chose.
enum Pick {
    /// The song's kit again.
    Song,
    Kit(Kit),
}

impl App {
    /// Drum machine `k`'s own kit, `None` while it plays the song's.
    pub fn kit_of(&self, k: usize) -> Option<&Kit> {
        let link = self.link.as_ref()?;
        if link.has_kit_of(MACHINES[k]) {
            self.machine_kit[k].as_ref()
        } else {
            None
        }
    }

    /// The KIT menu of drum machine `k`, opened by a click on `r`.
    pub(crate) fn kit_menu(&mut self, r: &Response, k: usize) {
        let mut pick = None;
        let own = self.kit_of(k).cloned();
        egui::Popup::menu(r).show(|ui| {
            let song = format!("Song's kit ({})", self.doc.mod_name);
            if ui.selectable_label(own.is_none(), song).clicked() {
                pick = Some(Pick::Song);
                ui.close();
            }
            ui.separator();
            if ui
                .selectable_label(own == Some(Kit::Standard), STANDARD)
                .clicked()
            {
                pick = Some(Pick::Kit(Kit::Standard));
                ui.close();
            }
            if let Some(Ok(list)) = &self.mod_list {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for e in list {
                            let kit = Kit::Mod(e.path.clone());
                            let on = own.as_ref() == Some(&kit);
                            if ui.selectable_label(on, &e.name).clicked() {
                                pick = Some(Pick::Kit(kit));
                                ui.close();
                            }
                        }
                    });
            }
        });
        match pick {
            Some(Pick::Song) => self.use_kit_of(k, None),
            Some(Pick::Kit(kit)) => self.use_kit_of(k, Some(kit)),
            None => {}
        }
    }

    /// Drum machine `k` plays `kit` (`None`: the song's kit again). A kit that cannot be read
    /// is shown and changes nothing.
    pub fn use_kit_of(&mut self, k: usize, kit: Option<Kit>) {
        self.action(|s| {
            let want = kit.clone().unwrap_or_else(|| s.doc.kit.clone());
            let samples = match s.kit_samples(&want) {
                Ok(Some(samples)) => samples,
                Ok(None) => rbm::standard::builtin_samples(),
                Err(e) => {
                    s.fail(e);
                    return;
                }
            };
            let Some(link) = &mut s.link else {
                return;
            };
            link.set_kit_of(MACHINES[k], samples, kit.is_some(), &mut s.panel);
            s.machine_kit[k] = kit;
        });
    }
}
