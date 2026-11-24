//! The `.rbsx` song (`rbsx.rs`), headless with a real player: an `.rbsx` keeps the GENERATE
//! menu's settings with the song and gives them back when opened, an `.rbs` saved by the app
//! stays a plain RBS song, and Save asks before an `.rbs` with GENERATE settings loses them.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use eframe::egui::{self, Event, Modifiers, Pos2, RawInput, Rect, Vec2};
use orb_engine::samples::SampleSet;
use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};
use orb_ui::app::App;
use orb_ui::browser::Mode;
use orb_ui::generate::{Field, Op};
use orb_ui::layout::{PANEL_H, PANEL_W};
use orb_ui::link::Link;
use orb_ui::menu::Cmd;
use orb_ui::model::Panel;
use orb_ui::rbsx::{self, FormatAnswer, GenState};
use orb_ui::scale::SCALES;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tables() -> Arc<Tables> {
    static T: OnceLock<Arc<Tables>> = OnceLock::new();
    T.get_or_init(|| Arc::new(Tables::new())).clone()
}

fn samples() -> Arc<SampleSet> {
    rbm::standard::builtin_samples()
}

struct Harness {
    ctx: egui::Context,
    app: App,
    audio: Audio,
    time: f64,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let (link, audio) = Link::new(tables(), Some(samples()));
        let mut app = App::with_link(Panel::new(), link, None);
        app.samples = Some(samples());
        let mut h = Self {
            ctx,
            app,
            audio,
            time: 0.0,
        };
        h.calls(1);
        h
    }

    fn frame(&mut self) {
        self.time += 0.016;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(PANEL_W, PANEL_H))),
            time: Some(self.time),
            events: vec![Event::ModifiersChanged(Modifiers::NONE)],
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.show(ui));
        out.textures_delta.clear();
    }

    fn calls(&mut self, n: usize) {
        let mut out = vec![0i16; 2 * CALL_FRAMES * n];
        for c in out.chunks_mut(2 * CALL_FRAMES) {
            self.audio.render_i16(c);
        }
        self.frame();
    }

    fn open(&mut self, path: &Path) {
        self.app.open_song(path);
        assert!(self.app.errors.is_empty(), "{:?}", self.app.errors);
        self.calls(2);
    }

    /// BAZE 1 on C IONIAN, a RANDOM run of NOTE and ACCENT, MOVE's +1 chosen: settings to keep.
    fn set_up_generate(&mut self) {
        let ionian = SCALES.iter().position(|(n, _)| *n == "IONIAN").unwrap();
        self.app.press_field(0, Field::Scale);
        self.app.choose_item(ionian);
        self.app.press_field(0, Field::Key);
        self.app.choose_item(0);
        self.app.set_random_seed(3);
        self.app.press_op(0, Op::Random);
        self.app.press_op_param(0, Op::Random, orb_ui::random::NOTE);
        self.app
            .press_op_param(0, Op::Random, orb_ui::random::ACCENT);
        self.app.press_op(0, Op::Random);
        self.app.press_op(0, Op::Move);
        self.app.press_op_param(0, Op::Move, 3);
        self.calls(2);
    }
}

fn song() -> PathBuf {
    root().join("tests/songs/diag/songs/303_saw_lowcut.rbs")
}

#[test]
fn an_rbsx_keeps_the_generate_settings_and_its_song() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    h.open(&song());
    assert!(
        h.app.gen_state().is_empty(),
        "an .rbs opens with an empty menu"
    );
    h.set_up_generate();
    let kept = h.app.gen_state();
    assert!(!kept.is_empty());
    let pattern = h.app.panel.pattern_303(0);

    // Save As with a name and no extension: an .rbsx
    h.app.command(Cmd::SaveAs);
    assert!(h.app.browsing());
    let out = dir.path().join("mine.rbsx");
    h.app.chosen(Mode::SaveSong, out.clone());
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);
    assert!(!h.app.doc.modified);

    // the file is an RBS song with the OGEN chunk at its end
    let bytes = std::fs::read(&out).unwrap();
    let mut parsed = rbs::Song::parse(&bytes).unwrap();
    assert_eq!(rbsx::split(&mut parsed), Some(kept.clone()));

    // a new app opens it: the song and the menu come back
    let mut h2 = Harness::new();
    h2.open(&out);
    assert_eq!(h2.app.gen_state(), kept);
    assert_eq!(h2.app.panel.pattern_303(0), pattern);
    assert!(h2.app.row_done(0, 0) && h2.app.row_done(0, 1));
    // File > New empties the menu again
    h2.app.new_now();
    h2.calls(1);
    assert!(h2.app.gen_state().is_empty());
}

#[test]
fn an_rbs_saved_by_the_app_stays_a_plain_rbs_and_save_asks_first() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("old.rbs");
    std::fs::copy(song(), &src).unwrap();
    let mut h = Harness::new();
    h.open(&src);
    // no GENERATE settings: Save writes the .rbs without asking
    h.app.panel.set_knob("tb303_1.cutoff", 700);
    h.frame();
    h.app.command(Cmd::Save);
    assert!(!h.app.format_prompting());
    let plain = std::fs::read(&src).unwrap();
    let mut parsed = rbs::Song::parse(&plain).unwrap();
    assert_eq!(rbsx::split(&mut parsed), None);

    // with GENERATE settings, Save asks; Cancel saves nothing
    h.set_up_generate();
    assert!(h.app.doc.modified, "the menu's changes count as changes");
    h.app.command(Cmd::Save);
    assert!(h.app.format_prompting());
    h.app.format_answer(FormatAnswer::Cancel);
    assert!(!h.app.format_prompting());
    assert_eq!(std::fs::read(&src).unwrap(), plain);
    assert!(h.app.doc.modified);

    // ".rbs": the file is saved without the settings, a plain RBS song
    h.app.command(Cmd::Save);
    h.app.format_answer(FormatAnswer::Rbs);
    assert!(!h.app.doc.modified);
    let mut parsed = rbs::Song::parse(&std::fs::read(&src).unwrap()).unwrap();
    assert_eq!(rbsx::split(&mut parsed), None);
    assert!(parsed.unknown.is_empty());

    // ".rbsx": Save As opens on the same folder with the name as .rbsx
    h.app.panel.set_knob("tb303_1.cutoff", 800);
    h.frame();
    h.app.command(Cmd::Save);
    h.app.format_answer(FormatAnswer::Rbsx);
    assert!(h.app.browsing());
    let rbsx_path = dir.path().join("old.rbsx");
    h.app.chosen(Mode::SaveSong, rbsx_path.clone());
    assert_eq!(h.app.doc.path.as_deref(), Some(rbsx_path.as_path()));
    let mut parsed = rbs::Song::parse(&std::fs::read(&rbsx_path).unwrap()).unwrap();
    assert_eq!(rbsx::split(&mut parsed), Some(h.app.gen_state()));
    // the song is now an .rbsx: Save writes it without asking
    h.app.panel.set_knob("tb303_1.cutoff", 600);
    h.frame();
    h.app.command(Cmd::Save);
    assert!(!h.app.format_prompting());
    assert!(!h.app.doc.modified);
}

#[test]
fn every_shipped_rbs_song_still_opens_with_an_empty_menu() {
    let mut h = Harness::new();
    let dir = root().join("tests/songs/shipped");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("rbs") {
            // (a song on a mod we do not have plays the built-in kit and says so: not our test)
            h.app.open_song(&p);
            h.app
                .errors
                .retain(|e| !e.contains("was not found in the Mods folder"));
            assert!(
                h.app.errors.is_empty(),
                "{}: {:?}",
                p.display(),
                h.app.errors
            );
            h.calls(2);
            assert_eq!(h.app.doc.path.as_deref(), Some(p.as_path()));
            assert_eq!(h.app.gen_state(), GenState::default(), "{}", p.display());
            n += 1;
        }
    }
    assert!(n > 0);
}

/// (this fork) An .rbsx keeps the drum machines' FLIP faces: which shows, the sounds' pans and
/// effects switches and a machine's own kit; they come back with the song, and the engine
/// takes the mix.
#[test]
fn an_rbsx_keeps_the_flip_faces_and_a_machines_kit() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    h.open(&song());
    h.app.panel.press("tr909.flip", 0.0);
    h.app.panel.press("tr909f.fx2", 0.0);
    h.app.panel.set_knob("tr808f.pan3", 0);
    h.app.use_kit_of(
        0,
        Some(orb_cli::mods::Kit::Mod(root().join("assets/Standard.rbm"))),
    );
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);
    h.calls(2);
    assert!(h.app.doc.modified);
    let kept = h.app.gen_state();
    assert!(kept.flip[1] && !kept.voice_mix[1].fx[2] && kept.voice_mix[0].pan[3] == 0);
    assert!(kept.kits[0].is_some());
    let out = dir.path().join("flip.rbsx");
    h.app.command(Cmd::SaveAs);
    h.app.chosen(Mode::SaveSong, out.clone());
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);

    let mut h2 = Harness::new();
    h2.open(&out);
    assert_eq!(h2.app.gen_state(), kept);
    assert!(h2.app.panel.flipped(1) && !h2.app.panel.flipped(0));
    assert!(h2.app.kit_of(0).is_some() && h2.app.kit_of(1).is_none());
    assert!(!h2.app.doc.modified, "opening is no edit");
}

/// (this fork) An .rbsx keeps each 303's sub-oscillator: its level and its song-mode track
/// events, which the song's own tracks (the .rbs part) never carry; they come back with the
/// song, into the knob and the engine. A plain .rbs opens with the sub off.
#[test]
fn an_rbsx_keeps_the_303s_sub_and_its_moves() {
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    h.open(&song());
    h.app.panel.set_knob("tb303_2.sub", 1000);
    h.calls(2);
    let moves = [(0, 40), (768, 100), (1536, 20)];
    h.app.link.as_mut().unwrap().send_sub(1, 127, &moves);
    h.calls(2);
    let kept = h.app.gen_state();
    assert_eq!(kept.sub, [0, 127]);
    assert_eq!(kept.sub_track, [vec![], moves.to_vec()]);
    let out = dir.path().join("sub.rbsx");
    h.app.command(Cmd::SaveAs);
    h.app.chosen(Mode::SaveSong, out.clone());
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);
    // the song's own tracks have no event of the sub's controller
    let file = rbs::Song::parse(&std::fs::read(&out).unwrap()).unwrap();
    assert!(
        file.tracks
            .iter()
            .flat_map(|t| &t.events)
            .all(|e| e.ctrl != orb_engine::tb303::SUB)
    );

    let mut h2 = Harness::new();
    h2.open(&out);
    h2.calls(2);
    assert_eq!(h2.app.gen_state(), kept);
    assert_eq!(h2.app.panel.value("tb303_2.sub"), Some(127));
    let snap = *h2.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!(snap.sub, [0, 127]);
    assert!(!h2.app.doc.modified, "opening is no edit");

    // a plain .rbs: the sub off
    let mut h3 = Harness::new();
    h3.open(&song());
    h3.calls(2);
    assert_eq!(h3.app.gen_state().sub, [0, 0]);
    let snap = *h3.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!(snap.sub, [0, 0]);
}

#[test]
fn an_rbsx_keeps_the_master_limiter_and_its_moves() {
    use orb_engine::frame::{LIMIT, LIMIT_AMT};
    use orb_engine::song::Device;
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    h.open(&song());
    h.app.panel.press("master.limit", 0.0);
    h.app.panel.set_knob("master.limit_amt", 1000);
    // the frame sends them, the next calls play them
    h.calls(2);
    h.calls(2);
    let snap = *h.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!((snap.mixer.limit, snap.mixer.limit_amt), (1, 127));
    let moves = [(0, 127), (768, 30)];
    h.app
        .link
        .as_mut()
        .unwrap()
        .send_fork_track(Device::Mixer, LIMIT_AMT, 127, &moves);
    h.calls(2);
    let kept = h.app.gen_state();
    assert_eq!(kept.limit, [1, 127]);
    assert_eq!(kept.limit_track, [vec![], moves.to_vec()]);
    let out = dir.path().join("limit.rbsx");
    h.app.command(Cmd::SaveAs);
    h.app.chosen(Mode::SaveSong, out.clone());
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);
    // the song's own tracks have no event of the limiter's controllers
    let file = rbs::Song::parse(&std::fs::read(&out).unwrap()).unwrap();
    assert!(
        file.tracks
            .iter()
            .flat_map(|t| &t.events)
            .all(|e| e.ctrl != LIMIT && e.ctrl != LIMIT_AMT)
    );

    let mut h2 = Harness::new();
    h2.open(&out);
    h2.calls(2);
    assert_eq!(h2.app.gen_state(), kept);
    assert!(h2.app.panel.toggle_on("master.limit"));
    assert_eq!(h2.app.panel.value("master.limit_amt"), Some(127));
    let snap = *h2.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!((snap.mixer.limit, snap.mixer.limit_amt), (1, 127));
    assert!(!h2.app.doc.modified, "opening is no edit");

    // a plain .rbs: the limiter off
    let mut h3 = Harness::new();
    h3.open(&song());
    h3.calls(2);
    assert_eq!(h3.app.gen_state().limit, [0, 0]);
    assert!(!h3.app.panel.toggle_on("master.limit"));
}

/// This fork: an `.rbsx` keeps the effects slots: which effect each slot holds, the second
/// effects' settings and the channels' knobs of them, and their song-mode moves.
#[test]
fn an_rbsx_keeps_the_effects_slots() {
    use orb_engine::frame::{FX_PCF, WET_B};
    use orb_engine::song::Device;
    let dir = tempfile::tempdir().unwrap();
    let mut h = Harness::new();
    h.open(&song());
    // a second PCF in the delay's slot, channel 2 hears it, its pattern one up
    h.app.panel.put_slot(0, FX_PCF);
    h.app.panel.set_knob("mixer2.pcf_b", 1000);
    h.app.panel.arrow("pcf_b.pattern_up");
    // and channel 1's EQ: the lows up, the highs down
    h.app.panel.set_knob("mixer1.eq_lo", 1000);
    h.app.panel.set_knob("mixer1.eq_hi", 0);
    h.calls(2);
    h.calls(2);
    let moves = [(0, 127), (384, 40)];
    h.app
        .link
        .as_mut()
        .unwrap()
        .send_fork_track(Device::Mixer, WET_B + 4, 127, &moves);
    h.calls(2);
    let snap = *h.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!(snap.mixer.slots[0], FX_PCF);
    assert_eq!(snap.mixer.wet_b[1][1], 127);
    assert_eq!(snap.mixer.eq[0], [127, 64, 0]);
    let kept = h.app.gen_state();
    assert!(
        kept.mixer_fx
            .iter()
            .any(|(g, v, t)| *g == WET_B + 4 && *v == 127 && t == &moves.to_vec())
    );
    let out = dir.path().join("slots.rbsx");
    h.app.command(Cmd::SaveAs);
    h.app.chosen(Mode::SaveSong, out.clone());
    assert!(h.app.errors.is_empty(), "{:?}", h.app.errors);
    // the song's own tracks have none of their events
    let file = rbs::Song::parse(&std::fs::read(&out).unwrap()).unwrap();
    assert!(
        file.tracks
            .iter()
            .flat_map(|t| &t.events)
            .all(|e| !orb_engine::frame::is_fork_mixer(e.ctrl))
    );

    let mut h2 = Harness::new();
    h2.open(&out);
    h2.calls(2);
    h2.calls(2);
    assert_eq!(h2.app.gen_state(), kept);
    let snap2 = *h2.app.link.as_mut().unwrap().player().snapshot();
    assert_eq!(snap2.mixer.slots, snap.mixer.slots);
    assert_eq!(snap2.mixer.wet_b, snap.mixer.wet_b);
    assert_eq!(snap2.mixer.pcf_b, snap.mixer.pcf_b);
    assert_eq!(snap2.mixer.eq, snap.mixer.eq);
    assert_eq!(h2.app.panel.value("mixer1.eq_lo"), Some(127));
    assert_eq!(h2.app.panel.slots, snap.mixer.slots);
    assert!(!h2.app.panel.out_of_slots("pcf_b", "freq"));
    assert!(!h2.app.doc.modified, "opening is no edit");

    // a plain .rbs: the original's slots
    let mut h3 = Harness::new();
    h3.open(&song());
    h3.calls(2);
    assert!(h3.app.gen_state().mixer_fx.is_empty());
    assert_eq!(h3.app.panel.slots, orb_engine::frame::DEFAULT_SLOTS);
}
