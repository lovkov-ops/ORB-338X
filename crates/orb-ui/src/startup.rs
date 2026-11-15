//! This fork's startup sound (`assets/startup.wav`): played instead of the standard kit's own
//! (the original program's) where the app plays the startup sound of the built-in kit: at the
//! start, when a song with the built-in kit is opened, and on a click on the logo. A mod keeps
//! its own startup sound.

use std::sync::{Arc, OnceLock};

use orb_engine::samples::Sample;

const WAV: &[u8] = include_bytes!("../../../assets/startup.wav");

/// The sound as the preview player plays it: mono int16 at 44100 Hz.
pub fn sample() -> Arc<Sample> {
    static S: OnceLock<Arc<Sample>> = OnceLock::new();
    S.get_or_init(|| Arc::new(Sample::from_audio(&mono(WAV).unwrap_or_default())))
        .clone()
}

/// The 16-bit PCM of a WAV file, its channels averaged to mono; None for any other format.
pub fn mono(wav: &[u8]) -> Option<Vec<i16>> {
    if wav.get(0..4)? != b"RIFF" || wav.get(8..12)? != b"WAVE" {
        return None;
    }
    let (mut channels, mut bits, mut data) = (0u16, 0u16, None);
    let mut at = 12;
    while at + 8 <= wav.len() {
        let id = &wav[at..at + 4];
        let len = u32::from_le_bytes(wav[at + 4..at + 8].try_into().ok()?) as usize;
        let body = wav.get(at + 8..(at + 8 + len).min(wav.len()))?;
        match id {
            b"fmt " => {
                let format = u16::from_le_bytes(body.get(0..2)?.try_into().ok()?);
                if format != 1 {
                    return None;
                }
                channels = u16::from_le_bytes(body.get(2..4)?.try_into().ok()?);
                bits = u16::from_le_bytes(body.get(14..16)?.try_into().ok()?);
            }
            b"data" => data = Some(body),
            _ => {}
        }
        // chunks are padded to an even length
        at += 8 + len + (len & 1);
    }
    let data = data?;
    if bits != 16 || channels == 0 {
        return None;
    }
    let n = channels as usize;
    Some(
        data.chunks_exact(2 * n)
            .map(|frame| {
                let sum: i32 = frame
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&s| i16::from_le_bytes(s) as i32)
                    .sum();
                (sum / n as i32) as i16
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_startup_sound_is_read() {
        let a = mono(WAV).expect("16-bit PCM WAV");
        // 1.875 s at 44100 Hz
        assert_eq!(a.len(), 82_688);
        assert!(a.iter().any(|&s| s.unsigned_abs() > 1000), "not silent");
        assert_eq!(sample().frames(), a.len());
    }

    #[test]
    fn stereo_is_averaged_and_other_formats_are_refused() {
        let mut w = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\x08\0\0\0".to_vec();
        for v in [100i16, 300, -50, -150] {
            w.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(mono(&w), Some(vec![200, -100]));
        assert_eq!(mono(b"not a wav"), None);
    }
}
