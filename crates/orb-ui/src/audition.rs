//! The note of a 303 key in pitch mode, heard (this fork): the original makes no sound when a
//! key is pressed, the note is heard only when its step plays. Here, while the song is stopped,
//! a key pressed in pitch mode is played once: the song as the panel shows it is loaded into a
//! second engine of its own, the 303's selected pattern there gets the note on its first step
//! and rests after it, every other mixer channel is muted, and half a second of pattern-mode
//! playback is rendered. The player plays that as its preview sample (the one the mod's startup
//! sound uses), mixed after the song, so the song and the running engine are not touched.

use std::sync::Arc;

use orb_engine::frame::export::REALTIME_FRAMES;
use orb_engine::frame::{Activation, Devices, Engine};
use orb_engine::song::{Device, Song};
use orb_engine::tables::Tables;

use crate::app::App;
use crate::model::Step303;

/// How long the heard note is, in frames (44100 a second): the step and the tail of its decay.
pub const FRAMES: usize = 26_460;

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
    engine.locate(0);
    engine.play();
    let mut buf = vec![0i16; 2 * REALTIME_FRAMES];
    let mut out = Vec::with_capacity(FRAMES);
    while out.len() < FRAMES {
        let n = REALTIME_FRAMES.min(FRAMES - out.len());
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
    Some(out)
}

impl App {
    /// A key pressed in pitch mode this frame ([`crate::model::Panel::audition`]): heard while
    /// the song is stopped and there is a player.
    pub(crate) fn audition_step(&mut self) {
        let Some((synth, pattern, step)) = self.panel.audition.take() else {
            return;
        };
        if self.panel.transport.playing || self.link.is_none() {
            return;
        }
        let song = self.song_bytes().ok().and_then(|bytes| {
            let parsed = rbs::Song::parse(&bytes).ok()?;
            orb_engine::load::convert(&parsed).ok()
        });
        let Some(song) = song else {
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
