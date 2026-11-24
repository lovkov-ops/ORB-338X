//! The update check (this fork): at startup, and from Help > Check for Updates, the app asks
//! GitHub for the fork's latest release and, when it is newer than this build, offers its
//! installer for this system. The app never replaces itself: Download opens the installer's
//! link in the browser, and the user installs it as before (the builds are not signed).
//!
//! - The latest release is `tag_name` of [`LATEST_API`] (`v` and the build time, `v2026.1124.1900`;
//!   drafts and pre-releases are not "latest"). The installed version is the number of [`VERSION`];
//!   both are zero-padded `YYYY.MMDD.HHMM`, so a later build compares higher as text
//!   (`version.rs`).
//! - The check at startup runs only in a release build (the main branch, clean: `VERSION` has no
//!   `dev-`, `-dirty` or `unknown`) and when Options > Check for Updates at Startup is on
//!   (`Settings::check_updates`). It says nothing when there is no newer release, when the network
//!   or GitHub fails, or when the newer release is the one the user skipped
//!   (`Settings::skip_version`).
//! - Help > Check for Updates always asks and always answers: the offer, "up to date", or why the
//!   check failed. It offers a skipped release again.
//! - The request runs on its own thread, so neither the start nor a frame waits for it.
//! - `ORB_UPDATE_AS=<YYYY.MMDD.HHMM>` makes the app take that number as its own and check as a
//!   release build, to try the offer with an older number.

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui::{self, Context, Key, Modifiers, RichText};

use crate::about::{NAME, VERSION};
use crate::app::App;
use crate::palette::INK_DIM;

/// GitHub's latest release of this fork.
pub const LATEST_API: &str = "https://api.github.com/repos/lovkov-ops/ORB-338X/releases/latest";
/// The page of the latest release (the offer's link when this system has no installer).
pub const LATEST_PAGE: &str = "https://github.com/lovkov-ops/ORB-338X/releases/latest";
/// The latest release's files: `<this>/<asset name>` (`scripts/release-assets.sh` names them
/// without the version, so this link always gives the newest one).
pub const LATEST_DOWNLOAD: &str = "https://github.com/lovkov-ops/ORB-338X/releases/latest/download";
/// How long the check may take in all.
const TIMEOUT: Duration = Duration::from_secs(15);

/// Whether `v` is a version number: `YYYY.MMDD.HHMM`, digits only.
pub fn is_number(v: &str) -> bool {
    let p: Vec<&str> = v.split('.').collect();
    p.len() == 3
        && [4, 4, 4] == [p[0].len(), p[1].len(), p[2].len()]
        && p.iter().all(|s| s.bytes().all(|b| b.is_ascii_digit()))
}

/// The version number of a release tag (`v2026.1124.1900` gives `2026.1124.1900`).
pub fn tag_number(tag: &str) -> Option<&str> {
    tag.strip_prefix('v').filter(|n| is_number(n))
}

/// The version number of a version string (`2026.1124.1900 (2fb834f)`) and whether it is a
/// release build: of the main branch and clean (`version.rs`).
pub fn installed(version: &str) -> Option<(&str, bool)> {
    let (n, git) = version.split_once(' ')?;
    let release = !(git.contains("dev-") || git.contains("-dirty") || git.contains("unknown"));
    is_number(n).then_some((n, release))
}

/// Whether the release `latest` is newer than the build `own` (both version numbers).
pub fn newer(own: &str, latest: &str) -> bool {
    latest > own
}

/// The name of this system's installer in a release (`scripts/release-assets.sh`).
pub fn asset_name(os: &str, arch: &str) -> Option<&'static str> {
    Some(match (os, arch) {
        ("macos", "aarch64") => "ORB-338X-install-macOS-AppleSilicon.dmg",
        ("macos", "x86_64") => "ORB-338X-install-macOS-Intel.dmg",
        ("windows", "x86_64") => "ORB-338X-install-Windows-x64.exe",
        ("windows", "aarch64") => "ORB-338X-install-Windows-ARM64.exe",
        ("linux", "x86_64") => "ORB-338X-Linux-x86_64.AppImage",
        ("linux", "aarch64") => "ORB-338X-Linux-ARM64.AppImage",
        _ => return None,
    })
}

/// Where Download leads on this system: its installer of the latest release, else the
/// release's page.
pub fn download_url() -> String {
    match asset_name(std::env::consts::OS, std::env::consts::ARCH) {
        Some(a) => format!("{LATEST_DOWNLOAD}/{a}"),
        None => LATEST_PAGE.to_string(),
    }
}

/// The `tag_name` of a release's JSON.
pub fn tag_of(json: &str) -> Result<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("GitHub's answer is not JSON: {e}"))?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| "GitHub's answer has no release tag".to_string())
}

/// Asks GitHub for the latest release's tag (blocking).
fn fetch() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .https_only(true)
        .user_agent(format!("{NAME}-update-check"))
        .build()
        .into();
    let json = agent
        .get(LATEST_API)
        .header("Accept", "application/vnd.github+json")
        .call()
        .and_then(|mut r| r.body_mut().read_to_string())
        .map_err(|e| format!("cannot reach GitHub: {e}"))?;
    tag_of(&json)
}

/// What a finished check shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing (a check at startup with nothing to offer).
    Quiet,
    /// The release with this number is newer: the offer.
    Offer(String),
    /// The build is the latest release or newer (a check from the menu).
    UpToDate,
    /// The check failed, for this reason (a check from the menu).
    Failed(String),
}

/// What a check that got `result` (the latest tag, or why there is none) shows, for the build
/// `own`: from the menu (`manual`) every answer, at startup only an offer that is not of the
/// release `skip`.
pub fn outcome(
    own: &str,
    result: Result<String, String>,
    manual: bool,
    skip: Option<&str>,
) -> Outcome {
    let latest = result.and_then(|t| {
        tag_number(&t)
            .map(str::to_string)
            .ok_or(format!("the latest release has no version number: {t}"))
    });
    match latest {
        Ok(n) if newer(own, &n) && (manual || skip != Some(n.as_str())) => Outcome::Offer(n),
        Ok(_) if manual => Outcome::UpToDate,
        Err(e) if manual => Outcome::Failed(e),
        _ => Outcome::Quiet,
    }
}

/// The state of the update check.
#[derive(Default)]
pub struct Update {
    /// A check asked for and not started yet (the next frame starts it): whether it is from
    /// the menu.
    request: Option<bool>,
    /// The running check: its answer, and whether it is from the menu.
    pending: Option<(Receiver<Result<String, String>>, bool)>,
    /// What the update window shows (`None`: it is closed).
    shown: Option<Outcome>,
    /// The number of the window's opens: its id, so each open is centered (as `about_serial`).
    serial: u32,
}

/// The build's version number and whether it is a release build: `ORB_UPDATE_AS` when set to
/// a version number (a release build of that number), else [`VERSION`]'s.
fn own() -> Option<(String, bool)> {
    if let Ok(v) = std::env::var("ORB_UPDATE_AS")
        && is_number(&v)
    {
        return Some((v, true));
    }
    installed(VERSION).map(|(n, r)| (n.to_string(), r))
}

impl App {
    /// The check at startup: runs (from the first frame) when it is on and this is a release
    /// build.
    pub fn check_updates_at_startup(&mut self) {
        if self.settings.check_updates && own().is_some_and(|(_, release)| release) {
            self.update.request.get_or_insert(false);
        }
    }

    /// Help > Check for Updates (from the next frame).
    pub fn check_updates_now(&mut self) {
        self.update.request = Some(true);
    }

    fn start_update_check(&mut self, ctx: &Context, manual: bool) {
        if self.update.pending.is_some() {
            // one check at a time; one from the menu answers when it ends
            if manual && let Some((_, m)) = &mut self.update.pending {
                *m = true;
            }
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let _ = tx.send(fetch());
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.update.pending = Some((rx, manual)),
            Err(e) if manual => self.show_update(Outcome::Failed(e.to_string())),
            Err(_) => {}
        }
    }

    fn show_update(&mut self, o: Outcome) {
        if o != Outcome::Quiet {
            self.update.serial += 1;
            self.update.shown = Some(o);
        }
    }

    /// Whether a check is asked for or running (for tests).
    pub fn update_check_running(&self) -> bool {
        self.update.request.is_some() || self.update.pending.is_some()
    }

    /// What the update window shows, if it is open (for tests).
    pub fn update_shown(&self) -> Option<&Outcome> {
        self.update.shown.as_ref()
    }

    /// Starts a check asked for, takes a finished check's answer and draws the update window
    /// when it is open.
    pub fn update_ui(&mut self, ctx: &Context) {
        if let Some(manual) = self.update.request.take() {
            self.start_update_check(ctx, manual);
        }
        if let Some((rx, manual)) = &self.update.pending {
            let manual = *manual;
            match rx.try_recv() {
                Ok(result) => {
                    self.update.pending = None;
                    let own = own().map(|(n, _)| n).unwrap_or_default();
                    let skip = self.settings.skip_version.clone();
                    self.show_update(outcome(&own, result, manual, skip.as_deref()));
                }
                Err(TryRecvError::Disconnected) => self.update.pending = None,
                Err(TryRecvError::Empty) => {}
            }
        }
        let Some(shown) = self.update.shown.clone() else {
            return;
        };
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.update.shown = None;
            return;
        }
        let own = own().map(|(n, _)| n).unwrap_or_default();
        let mut open = true;
        let mut close = false;
        let title = match shown {
            Outcome::Offer(_) => "Update Available",
            _ => "Check for Updates",
        };
        egui::Window::new(title)
            .id(egui::Id::unique(("orb_update_window", self.update.serial)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                ui.set_width(360.0);
                ui.add_space(4.0);
                match &shown {
                    Outcome::Offer(n) => {
                        ui.label(format!("A new version of {NAME} is available."));
                        ui.add_space(4.0);
                        ui.label(RichText::new(format!("New: {n}\nYours: {own}")).color(INK_DIM));
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if ui.button("Download").clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(download_url()));
                                close = true;
                            }
                            if ui.button("Later").clicked() {
                                close = true;
                            }
                            if ui.button("Skip This Version").clicked() {
                                self.settings.skip_version = Some(n.clone());
                                if let Err(e) = self.settings.save() {
                                    self.fail(e);
                                }
                                close = true;
                            }
                        });
                    }
                    Outcome::UpToDate => {
                        ui.label(format!("{NAME} is up to date ({own})."));
                        ui.add_space(10.0);
                        close |= ui.button("OK").clicked();
                    }
                    Outcome::Failed(e) => {
                        ui.label("Could not check for updates.");
                        ui.add_space(4.0);
                        ui.label(RichText::new(e).color(INK_DIM));
                        ui.add_space(10.0);
                        close |= ui.button("OK").clicked();
                    }
                    Outcome::Quiet => {}
                }
            });
        if !open || close {
            self.update.shown = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_tags() {
        assert!(is_number("2026.1124.1900"));
        for bad in [
            "2026.1124",
            "2026.1124.19000",
            "v2026.1124.1900",
            "2026.11a4.1900",
            "",
        ] {
            assert!(!is_number(bad), "{bad}");
        }
        assert_eq!(tag_number("v2026.1124.1900"), Some("2026.1124.1900"));
        assert_eq!(tag_number("2026.1124.1900"), None);
        assert_eq!(tag_number("v1.0"), None);
    }

    #[test]
    fn the_installed_version_and_release_builds() {
        assert_eq!(
            installed("2026.1124.1900 (2fb834f)"),
            Some(("2026.1124.1900", true))
        );
        for dev in [
            "2026.1124.1900 (dev-2fb834f)",
            "2026.1124.1900 (2fb834f-dirty)",
            "2026.1124.1900 (unknown)",
        ] {
            assert_eq!(installed(dev), Some(("2026.1124.1900", false)), "{dev}");
        }
        assert_eq!(installed("garbage"), None);
        // the build's own string parses
        assert!(installed(VERSION).is_some(), "{VERSION}");
    }

    #[test]
    fn a_later_release_is_newer() {
        assert!(newer("2026.1124.1639", "2026.1124.1900"));
        assert!(newer("2026.1231.2359", "2027.0101.0000"));
        assert!(!newer("2026.1124.1900", "2026.1124.1900"));
        assert!(!newer("2026.1125.0000", "2026.1124.1900"));
    }

    #[test]
    fn every_system_has_its_installer() {
        let names = [
            asset_name("macos", "aarch64"),
            asset_name("macos", "x86_64"),
            asset_name("windows", "x86_64"),
            asset_name("windows", "aarch64"),
            asset_name("linux", "x86_64"),
            asset_name("linux", "aarch64"),
        ];
        assert!(names.iter().all(|n| n.is_some()));
        assert_eq!(asset_name("freebsd", "x86_64"), None);
        // the names scripts/release-assets.sh gives the installers
        let script = include_str!("../../../scripts/release-assets.sh");
        for n in names.into_iter().flatten() {
            assert!(script.contains(n), "{n} is not a release asset");
        }
        assert!(download_url().starts_with("https://github.com/lovkov-ops/ORB-338X/releases/"));
    }

    #[test]
    fn the_tag_of_github_s_answer() {
        let json = r#"{"url":"x","tag_name":"v2026.1124.1900","draft":false}"#;
        assert_eq!(tag_of(json).as_deref(), Ok("v2026.1124.1900"));
        assert!(tag_of("{}").is_err());
        assert!(tag_of("<html>").is_err());
    }

    #[test]
    fn what_a_check_shows() {
        let own = "2026.1124.1639";
        let tag = |t: &str| Ok(t.to_string());
        let new = "v2026.1124.1900";
        // a newer release: offered at startup and from the menu
        let offer = Outcome::Offer("2026.1124.1900".into());
        assert_eq!(outcome(own, tag(new), false, None), offer);
        assert_eq!(outcome(own, tag(new), true, None), offer);
        // skipped: quiet at startup, offered from the menu; another skip does not count
        let skip = Some("2026.1124.1900");
        assert_eq!(outcome(own, tag(new), false, skip), Outcome::Quiet);
        assert_eq!(outcome(own, tag(new), true, skip), offer);
        assert_eq!(outcome(own, tag(new), false, Some("2026.0101.0000")), offer);
        // the same or older: quiet at startup, "up to date" from the menu
        for t in ["v2026.1124.1639", "v2026.1001.0000"] {
            assert_eq!(outcome(own, tag(t), false, None), Outcome::Quiet);
            assert_eq!(outcome(own, tag(t), true, None), Outcome::UpToDate);
        }
        // a failure or a tag without a number: quiet at startup, the reason from the menu
        for r in [Err("offline".to_string()), tag("v1.0")] {
            assert_eq!(outcome(own, r.clone(), false, None), Outcome::Quiet);
            assert!(matches!(outcome(own, r, true, None), Outcome::Failed(_)));
        }
    }
}
