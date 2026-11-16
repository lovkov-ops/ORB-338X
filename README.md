<p align="center">
  <img src="assets/readme/logo.svg" alt="OpenRB-338x" width="402">
</p>

<div align="center">

# OpenRB-338x

</div>

<p align="center">
  <a href="https://github.com/lovkov-ops/OpenRB-338x/releases"><b>Download</b></a> ·
  <a href="assets/readme/screenshot.png"><b>Screenshot</b></a> ·
  <a href="https://github.com/pingw33n/rb-songs"><b>Song collection</b></a> ·
  <a href="https://github.com/pingw33n/rb-mods"><b>Mod collection</b></a>
</p>

OpenRB-338x is a reimplementation of [ReBirth RB-338](https://en.wikipedia.org/wiki/ReBirth_RB-338),
the 1997 software synth of two TB-303 bass lines, a TR-808 and a TR-909, for today's macOS, Linux
and Windows.

OpenRB-338x is a fork of [OpenRB-338](https://github.com/pingw33n/OpenRB-338) by pingw33n, with
extra features for the two bass synths. Everything the original does, it does too.

## New in OpenRB-338x

- **GENERATE menu** on each bass synth (the RNDM button in the step editor):
  - **SCALE AND KEY:** 37 scales and every key. The keys of the chosen scale glow green, and
    the second synth is offered only the scales that fit the first one.
  - **GENERATE:** writes a new pattern in the scale: any of down, up, accent, slide and note
    (with rests).
  - **ALTER:** changes the pattern that is there instead of starting over.
  - **SHIFT:** shifts the pattern by -2, -1, a random amount, +1 or +2 steps.
  - **RESET:** puts the menu back as it starts (the pattern stays); **UNDO** and **REDO** step
    through the menu's changes of the pattern.
  - Gray, white and green show what can be chosen next, what can, and what is chosen or done.
- **An LFO on CUTOFF, RESO, ENV MOD, DECAY and ACCENT,** in a row under each knob: RATE and
  AMOUNT knobs, a note button that ties the rate to the song's tempo, and a wave button that
  flips between a sine and a random sine. The knob's arc shows the moving value.
- **Step LEDs:** 16 LEDs in the step editor choose the step to edit (green); the steps of the
  pattern's length are lit red, and the step that plays flashes yellow.
- **The waveform as two pictures** (saw and square) to click, under TUNE, now a large knob
  like the others.
- **Hear the note in pitch mode:** while the song is stopped, a key pressed in pitch mode plays
  its note with the synth's current sound.
- **`.rbsx` songs:** a ReBirth song plus the GENERATE and LFO settings. Plain `.rbs` songs still
  open and save as before; Save As has a button for each format.
- **The Inter typeface** for the panel's labels, the logo and the windows, and this fork's own
  startup sound.
- **The drum machines as loud as the 303s** in a new song at its default values (808 +5 dB,
  909 +3.2 dB, as heard), in the app, its export and `orb render` (`orb render --original`
  keeps the original's sound bit for bit). A click on a drum's name over the steps plays it.
- **SOLO and MUTE** on every mixer channel (several at a time; a double click on a lit one
  turns all of its kind off), kept in `.rbsx` songs; the channel's LED shows when it sounds.
- **One set of settings for pattern and song mode:** knobs, faders and DIST/PCF/COMP stay as
  they are when the mode changes (the original keeps a set for each); changes recorded in
  song mode still play.
- **Copy, Cut and Paste of a pattern:** Cmd+C, Cmd+X and Cmd+V on macOS (Ctrl on Windows and
  Linux) work on the selected pattern of the row with the focus, with its steps, length and
  shuffle. The Edit menu and a right click on the lit pattern button have them too. A 303's
  pattern pastes into either 303.

## Features

- **Sounds exactly like the original.** The audio engine was rebuilt from ReBirth 2.01 to the
  bit. A song's exported loop equals the original's "Export Loop as Audio File" byte for byte. This
  is tested on 1,268 songs of the song collection, 531 of them on 43 third-party mods, and on a
  175-render reference corpus.
- **The whole machine on screen:** the two 303s, the 808 and the 909, the mixer, the delay,
  distortion, PCF and compressor, with pattern and song mode, the loop, shuffle and recording.
- **A scalable vector-graphics panel** that stays sharp at any window size and pixel density.
- **Songs:** opens ReBirth 2.x songs (`.rbs`, RBS 4.0 to 4.2) and ReBirth 1.x songs, and saves
  RBS 4.2 files that the original opens. File > Info shows and edits the song's title, text, web
  page and copyright.
- **Mods:** plays the drum samples of ReBirth mods (`.rbm`), chosen per song, with the mod's
  startup sound. The standard kit is built in, so no file of the original is needed.
- **Audio export** of the loop to WAV, FLAC or Ogg Vorbis.
- **Plays from the computer keyboard:** program the synths and select patterns without a mouse
  (Help > Keyboard Shortcuts).
- **`orb`, a command-line tool,** to work with songs and mods from the terminal:
  - `orb render`: render a song's loop to WAV, FLAC or Ogg, far faster than real time (a
    five-minute song takes about a second and a half);
  - `orb play`: play a song without the UI;
  - `orb info`: print a song's facts (format, title, mod, tempo, loop), also as JSON;
  - `orb mod`: recrypt mods, and unpack a mod to a folder or pack one from it.
- **Packages** for macOS (Apple silicon and Intel), Windows (x64 and ARM64) and Linux (x86_64 and
  ARM64).

## Not implemented

- MIDI, VST and ReWire. OpenRB-338x is a standalone application only, with no MIDI input or sync.
- Mod themes: a mod changes the drum sounds, not the look of the panel.
- The original's other Edit menu functions on patterns (clear, shift and the like), and undo.
- The original's preferences and its Windows menu.
- Export to AIFF (use WAV, FLAC or Ogg).

## Quick start

1. **Download** the package for your system and CPU from
   [Releases](https://github.com/lovkov-ops/OpenRB-338x/releases):

   | System | File |
   |---|---|
   | macOS, Apple silicon | `OpenRB-338x-install-macOS-AppleSilicon.dmg` |
   | macOS, Intel | `OpenRB-338x-install-macOS-Intel.dmg` |
   | Windows, x64 | `OpenRB-338x-install-Windows-x64.exe` |
   | Windows, ARM64 | `OpenRB-338x-install-Windows-ARM64.exe` |
   | Linux, x86_64 | `OpenRB-338x-Linux-x86_64.AppImage` |
   | Linux, ARM64 | `OpenRB-338x-Linux-ARM64.AppImage` |

   Plain archives (`.tar.gz`, or `.zip` on Windows) with the two binaries are there too.

2. **Install it.**
   - macOS: open the `.dmg` and drag OpenRB-338x to Applications.
   - Windows: run the installer. It can add `orb` to the PATH.
   - Linux: make the AppImage executable (`chmod +x OpenRB-338x-Linux-*.AppImage`).

3. **Optionally, get songs and mods:** download the
   [song collection](https://github.com/pingw33n/rb-songs) and the
   [mod collection](https://github.com/pingw33n/rb-mods), and unpack them anywhere.

4. **Run OpenRB-338x.** The binaries are not signed, so the first start needs your permission:
   - **macOS:** the first launch is refused. Open System Settings > Privacy & Security, click
     "Open Anyway" next to OpenRB-338x, and confirm. On macOS 14 and older, right-click the app >
     Open > Open works too. Or, in Terminal: `xattr -dr com.apple.quarantine
     /Applications/OpenRB-338x.app`.
   - **Windows:** SmartScreen says "Windows protected your PC". Click "More info", then
     "Run anyway".
   - **Linux:** nothing more. If the AppImage does not start because FUSE is missing, install
     `libfuse2` or run it with `--appimage-extract-and-run`.

5. **Point it at your songs and mods.**
   - **Songs:** File > Open... opens the file browser. Go to the folder of your songs and click
     "Use as songs folder", so the browser starts there next time. Then open a song (`.rbs`).
   - **Mods:** Mods > Choose file... opens the browser on mods. Go to your mods folder and click
     "Use as mods folder". The Mods menu then lists every mod in it, and a song finds its mod
     there by name. Mods must be recrypted to load (see below); the mods of the mod collection
     already are.

6. **Play:** press Play, or Space.

## Making mods work in OpenRB-338x

The mods of the [mod collection](https://github.com/pingw33n/rb-mods) already work: open them
as they are.

Other mods need one step first. A ReBirth mod's samples are encrypted with a key that ReBirth
reads from `RB20FUL.DAT`, a file on the ReBirth CD. Instead of making you keep that file around,
OpenRB-338x carries the small part of the key that mods use, and needs mods re-encrypted with that
part. `orb mod recrypt` does this:

```sh
# one mod
orb mod recrypt --rb20ful RB20FUL.DAT MyMod.rbm -o MyMod-recrypted.rbm

# every mod of a folder, into another folder
orb mod recrypt --rb20ful RB20FUL.DAT ~/Mods -o ~/Mods-recrypted

# the key read straight from the ReBirth CD image
orb mod recrypt --iso "ReBirth.iso" ~/Mods -o ~/Mods-recrypted
```

Recrypting changes nothing else in the mod: its samples, panel pictures and text stay as they
were. The recrypted file is still a complete mod, and the original ReBirth opens it as before.
`RB20FUL.DAT` must be the one of the 2001 reissue CD (its checksum is checked). A mod that is
already recrypted is only checked and copied.

## Building

See [docs/building.md](docs/building.md).

## License

OpenRB-338x is free software under the [GNU General Public License v3.0](LICENSE) (GPL-3.0-only).
Copyright (C) 2026 the OpenRB-338 authors.

ReBirth RB-338 is a trademark of Propellerhead Software (now Reason Studios). OpenRB-338x is not
affiliated with or endorsed by them. The built-in standard kit (`assets/Standard.rbm`) holds
ReBirth's own drum samples and panel pictures. See [assets/README.md](assets/README.md).
