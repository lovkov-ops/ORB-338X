//! The note of a 303 key in pitch mode, heard (this fork): the original makes no sound when a
//! key is pressed, the note is heard only when its step plays. Here, while the song is stopped,
//! a key pressed in pitch mode is played once: the song as the panel shows it is loaded into a
//! second engine of its own, the 303's selected pattern there gets the note on its first step
//! and rests after it, every other mixer channel is muted, and half a second of pattern-mode
//! playback is rendered. The player plays that as its preview sample (the one the mod's startup
//! sound uses), mixed after the song, so the song and the running engine are not touched.
//!
//! A click on a drum machine's instrument button (the legend over its steps) plays that drum
//! the same way, at any time: one hit of the instrument on the first step, with the knobs as
//! they are and the kit that plays (`render_drum`).

use std::sync::Arc;

use orb_engine::frame::export::REALTIME_FRAMES;
use orb_engine::frame::{Activation, Devices, Engine};
use orb_engine::samples::SampleSet;
use orb_engine::song::{Device, Song};
use orb_engine::tables::Tables;

use crate::app::App;
use crate::model::Step303;

/// How long the heard note is, in frames (44100 a second): the step and the tail of its decay.
pub const FRAMES: usize = 26_460;
/// How long a heard drum hit is: long enough for a cymbal's tail.
pub const DRUM_FRAMES: usize = 66_150;

/// `engine`'s pattern-mode playback from the start, `frames` long, mono (left and right
/// averaged).
fn play_mono(engine: &mut Engine, frames: usize) -> Vec<i16> {
    engine.locate(0);
    engine.play();
    let mut buf = vec![0i16; 2 * REALTIME_FRAMES];
    let mut out = Vec::with_capacity(frames);
    while out.len() < frames {
        let n = REALTIME_FRAMES.min(frames - out.len());
        engine.render(&mut buf, n);
        out.extend(
            buf[..2 * n]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[l, r]| ((*l as i32 + *r as i32) / 2) as i16),
        );
    }
    engine.stop();
    out
}

/// Renders one hit of drum `inst` (the panel's instrument index, 1 BD .. 11; 0 the accent,
/// which makes no sound of its own: None) of the 808 (`is808`) or the 909, as the first step of
/// its pattern `pattern` of `song`, the other channels muted, with the drum `samples` of the
/// kit that plays: mono, [`DRUM_FRAMES`] long.
pub fn render_drum(
    tables: Arc<Tables>,
    samples: Option<Arc<SampleSet>>,
    song: &Song,
    is808: bool,
    pattern: u8,
    inst: usize,
) -> Option<Vec<i16>> {
    if inst == 0 || inst >= 12 {
        return None;
    }
    let mut engine = Engine::new(tables, Devices::implemented(samples));
    engine.load(song).ok()?;
    engine.activate(Activation::ReplacesCurrent {
        current_playing: false,
    });
    engine.stop();
    engine.set_song_mode(0);
    let (dev, channel) = if is808 {
        (Device::Tr808, 2)
    } else {
        (Device::Tr909, 3)
    };
    for s in 0..16u8 {
        let mut bytes = [0u8; 12];
        if s == 0 {
            bytes[inst] = 1;
        }
        engine.step(dev, pattern, s, &bytes);
    }
    engine.pattern_field(dev, pattern, 1, 16);
    engine.device_param(dev, 1, pattern);
    for c in 0..4u8 {
        engine.mixer_channel(c, 0, (c == channel) as u8);
    }
    Some(play_mono(&mut engine, DRUM_FRAMES))
}

/// Renders 303 `synth`'s `step` as the first step of its pattern `pattern` of `song`, the other
/// channels muted: mono int16 (left and right averaged), [`FRAMES`] long. None when the engine
/// refuses the song.
pub fn render_note(
    tables: Arc<Tables>,
    song: &Song,
    synth: usize,
    pattern: u8,
    step: Step303,
) -> Option<Vec<i16>> {
    // a 303 plays no drum sample: the engine's silent placeholders are enough
    let mut engine = Engine::new(tables, Devices::implemented(None));
    engine.load(song).ok()?;
    engine.activate(Activation::ReplacesCurrent {
        current_playing: false,
    });
    engine.stop();
    engine.set_song_mode(0);
    let dev = if synth == 0 {
        Device::Tb303_1
    } else {
        Device::Tb303_2
    };
    // the note on step 1, rests after it, in a pattern long enough not to repeat it
    let bytes = |s: Step303, rest: bool| {
        [
            s.note,
            s.slide as u8,
            s.accent as u8,
            s.up as u8,
            s.down as u8,
            rest as u8,
        ]
    };
    for s in 0..16u8 {
        let b = if s == 0 {
            bytes(step, false)
        } else {
            bytes(Step303::default(), true)
        };
        engine.step(dev, pattern, s, &b);
    }
    engine.pattern_field(dev, pattern, 1, 16);
    engine.device_param(dev, 1, pattern);
    for c in 0..4u8 {
        engine.mixer_channel(c, 0, (c as usize == synth) as u8);
    }
    Some(play_mono(&mut engine, FRAMES))
}

impl App {
    /// A click on a drum machine's instrument button: the drum is heard (any time, the song
    /// playing or not). Nothing for the accent or without a player.
    pub(crate) fn audition_drum(&mut self, is808: bool, inst: usize) {
        if inst == 0 || self.link.is_none() {
            return;
        }
        let Some(song) = self.audition_song() else {
            return;
        };
        let tables = self
            .audition_tables
            .get_or_insert_with(|| Arc::new(Tables::new()))
            .clone();
        let row = if is808 { 2 } else { 3 };
        let pattern = self.panel.rows[row].pattern;
        let Some(link) = &mut self.link else {
            return;
        };
        if let Some(audio) = render_drum(tables, link.kit(), &song, is808, pattern, inst) {
            link.preview(&audio);
        }
    }

    /// The song as the panel shows it, for the engine of a heard note or drum.
    fn audition_song(&mut self) -> Option<Song> {
        let bytes = self.song_bytes().ok()?;
        let parsed = rbs::Song::parse(&bytes).ok()?;
        orb_engine::load::convert(&parsed).ok()
    }

    /// A key pressed in pitch mode this frame ([`crate::model::Panel::audition`]): heard while
    /// the song is stopped and there is a player.
    pub(crate) fn audition_step(&mut self) {
        let Some((synth, pattern, step)) = self.panel.audition.take() else {
            return;
        };
        if self.panel.transport.playing || self.link.is_none() {
            return;
        }
        let Some(song) = self.audition_song() else {
            return;
        };
        let tables = self
            .audition_tables
            .get_or_insert_with(|| Arc::new(Tables::new()))
            .clone();
        let note = self.panel.synth[synth].patterns[pattern as usize][step];
        if let (Some(audio), Some(link)) = (
            render_note(tables, &song, synth, pattern, note),
            &mut self.link,
        ) {
            link.preview(&audio);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> Song {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/songs/shipped/Silent Default Song.rbs");
        let parsed = rbs::Song::parse(&std::fs::read(p).unwrap()).unwrap();
        orb_engine::load::convert(&parsed).unwrap()
    }

    fn peak(a: &[i16]) -> i32 {
        a.iter().map(|&s| (s as i32).abs()).max().unwrap_or(0)
    }

    #[test]
    fn a_drum_button_plays_its_drum() {
        let tables = Arc::new(Tables::new());
        let song = song();
        let kit = Some(rbm::standard::builtin_samples());
        // the accent makes no sound of its own
        assert!(render_drum(tables.clone(), kit.clone(), &song, true, 0, 0).is_none());
        for is808 in [true, false] {
            let bd = render_drum(tables.clone(), kit.clone(), &song, is808, 0, 1).unwrap();
            let sd = render_drum(tables.clone(), kit.clone(), &song, is808, 0, 2).unwrap();
            assert_eq!(bd.len(), DRUM_FRAMES);
            assert!(peak(&bd) > 1000, "808 {is808}: BD peak {}", peak(&bd));
            assert!(peak(&sd) > 1000, "808 {is808}: SD peak {}", peak(&sd));
            assert_ne!(bd, sd, "each button its own drum");
        }
    }

    /// (this fork) With the fork's drum gain on (the app turns it on at its start), the drum
    /// machines are 10% louder than the original; off, they are the original's.
    #[test]
    fn the_drum_machines_are_ten_percent_louder_with_the_forks_gain() {
        let tables = Arc::new(Tables::new());
        let song = song();
        let kit = Some(rbm::standard::builtin_samples());
        let render = |on: bool| {
            orb_engine::frame::set_fork_drum_gain(on);
            let a = render_drum(tables.clone(), kit.clone(), &song, false, 0, 1).unwrap();
            orb_engine::frame::set_fork_drum_gain(false);
            a
        };
        let (orig, loud) = (render(false), render(true));
        let ratio = peak(&loud) as f64 / peak(&orig) as f64;
        assert!((ratio - 1.1).abs() < 0.02, "{ratio}");
    }

    #[test]
    fn a_note_is_heard_quickly_and_dies_out() {
        let tables = Arc::new(Tables::new());
        let song = song();
        for synth in 0..2 {
            let note = Step303 {
                note: 7,
                rest: false,
                ..Step303::default()
            };
            let t = std::time::Instant::now();
            let a = render_note(tables.clone(), &song, synth, 0, note).unwrap();
            assert_eq!(a.len(), FRAMES);
            assert!(peak(&a) > 1000, "synth {synth}: peak {}", peak(&a));
            // fast enough to play at the key press (a debug build is slower still)
            assert!(t.elapsed().as_millis() < 500, "{:?}", t.elapsed());
            // the decay dies out: the last tenth is quieter than the start
            assert!(peak(&a[FRAMES * 9 / 10..]) < peak(&a[..FRAMES / 4]));
        }
        // a different note is a different sound
        let hi = render_note(
            tables.clone(),
            &song,
            0,
            0,
            Step303 {
                note: 12,
                rest: false,
                ..Step303::default()
            },
        )
        .unwrap();
        let lo = render_note(
            tables,
            &song,
            0,
            0,
            Step303 {
                note: 0,
                rest: false,
                ..Step303::default()
            },
        )
        .unwrap();
        assert_ne!(hi, lo);
    }
}
