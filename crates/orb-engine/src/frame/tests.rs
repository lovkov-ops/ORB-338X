use std::sync::{Arc, OnceLock};

use super::export::{CHUNK_FRAMES, ExportRange, chunk_sizes, frame_count};
use super::*;
use crate::test_cell::{Rc, RefCell};

fn tables() -> Arc<Tables> {
    static T: OnceLock<Arc<Tables>> = OnceLock::new();
    T.get_or_init(|| Arc::new(Tables::new())).clone()
}

type Log = Rc<RefCell<Vec<String>>>;

/// A source that writes a constant.
struct Const(f32);
impl SongDevice for Const {}
impl ChannelDevice for Const {
    fn process(&mut self, bus: &mut Block, _ctx: &BlockContext) {
        bus.fill(self.0);
    }
}

/// A source whose samples differ in every block: `(64k + i) / 8192` in block k.
struct Ramp(u32);
impl SongDevice for Ramp {}
impl ChannelDevice for Ramp {
    fn process(&mut self, bus: &mut Block, _ctx: &BlockContext) {
        for (i, x) in bus.iter_mut().enumerate() {
            *x = ((self.0 * 64 + i as u32) % 8192) as f32 / 8192.0;
        }
        self.0 += 1;
    }
}

/// An insert that applies `f` and logs its calls.
struct Op {
    name: &'static str,
    f: fn(f32) -> f32,
    log: Log,
}
impl InsertEffect for Op {
    fn process(&mut self, bus: &mut Block, _ctx: &BlockContext) {
        self.log.borrow_mut().push(self.name.to_string());
        for x in bus.iter_mut() {
            *x = (self.f)(*x);
        }
    }
    fn process_stereo(&mut self, _l: &mut Block, _r: &mut Block, _ctx: &BlockContext) {
        self.log.borrow_mut().push(format!("{}.stereo", self.name));
    }
    fn update(&mut self, _ctx: &BlockContext) {
        self.log.borrow_mut().push(format!("{}.update", self.name));
    }
}
impl SongDevice for Op {
    fn reset(&mut self) {
        self.log.borrow_mut().push(format!("{}.reset", self.name));
    }
}

/// A send effect that records the send bus it gets, then refills it like the delay.
struct Recorder {
    seen: Rc<RefCell<Vec<Block>>>,
    log: Log,
}
impl SongDevice for Recorder {}
impl SendEffect for Recorder {
    fn process(&mut self, send: &mut Block, _l: &mut Block, _r: &mut Block, _ctx: &BlockContext) {
        self.seen.borrow_mut().push(*send);
        self.log.borrow_mut().push("delay".into());
        send.fill(GUARD);
    }
}

fn engine_with(devices: Devices, state: EngineState) -> Engine {
    Engine::with_state(tables(), devices, state)
}

/// A state with every gain, target and send at zero and the master at a steady 1.0, outside the
/// update phase.
fn quiet_state() -> EngineState {
    let mut s = EngineState::default();
    s.mixer.counter = 1;
    for ch in &mut s.mixer.channels {
        *ch = ChannelState {
            target_l: 0.0,
            target_r: 0.0,
            gain_l: 0.0,
            gain_r: 0.0,
            send: 0.0,
            ..ChannelState::default()
        };
    }
    s
}

fn block_once(e: &mut Engine) -> (Block, Block) {
    let mut l = [0.0; BLOCK];
    let mut r = [0.0; BLOCK];
    e.frame.block(&mut l, &mut r).unwrap();
    (l, r)
}

#[test]
fn constants_decode() {
    assert_eq!(GUARD, 1e-6f32);
    assert_eq!(KEEP, 0.93);
    assert_eq!(TAKE, 0.07);
    assert_eq!(RAMP, 1.0 / 32.0);
    assert_eq!(CLAMP_HI, 32767.0);
    assert_eq!(CLAMP_LO, -32768.0);
    assert_eq!(METER_DECAY, 0.92f32);
    assert_eq!(VINTAGE_OFFSET, 1e-6f32 as f64);
}

#[test]
fn int16_conversion() {
    let cases: [(f32, i16); 14] = [
        (0.0, 0),
        (GUARD, 0),
        (-GUARD, 0),
        (0.5, 16384),
        (-0.5, -16384),
        (1.0 / 32768.0, 1),
        (-1.0 / 32768.0, -1),
        (0.99999 / 32768.0, 0),
        (-1.99 / 32768.0, -1),
        (32767.5 / 32768.0, 32767),
        (1.0, 32767),
        (7.0, 32767),
        (-1.0, -32768),
        (-7.0, -32768),
    ];
    for (f, want) in cases {
        assert_eq!(to_i16(f), want, "{f:e}");
    }
    assert_eq!(to_i16(f32::NAN), -32768);
}

#[test]
fn constructor_state() {
    let e = Engine::new(tables(), Devices::placeholders());
    let t = &tables().level;
    let s = &e.frame.state;
    assert_eq!(s.counter, 10);
    assert_eq!(s.master, 1.0);
    assert_eq!(
        s.master_target,
        (2.0 * t[100] as f64 + 1e-6f32 as f64) as f32
    );
    for ch in &s.channels {
        let tgt = (1e-6f32 as f64 + t[100] as f64) as f32;
        assert_eq!((ch.target_l, ch.target_r), (tgt, tgt));
        assert_eq!((ch.gain_l, ch.gain_r), (GUARD, GUARD));
        assert_eq!(ch.send, GUARD);
        assert_eq!(ch.routing, Routing::default());
    }
    assert_eq!(e.output.leftover_pos, 0);
}

/// The tick-0 events of track `t` after the song ctor, as `(ctrl, value)` in controller order.
fn ctor_events(e: &Engine, t: usize) -> Vec<(u8, u8)> {
    let events = e.frame.seq.events(t);
    assert!(events.iter().all(|ev| ev.tick == 0));
    let mut v: Vec<_> = events.iter().map(|ev| (ev.ctrl, ev.value)).collect();
    v.sort();
    v
}

/// The ctor default of every automatable controller, as the song ctor's slot 8 calls put them
/// into the tracks (`docs/song_load.md` section 4.9): the parameter-default functions
/// `sub_10019CC0` / `sub_10019D50` (mixer), `sub_10018C20` (303), `sub_10019AB0` (delay),
/// `sub_10019B30` (distortion), `sub_10019C10` (PCF) and `sub_10019B90` (compressor), read in
/// the disassembly (task-034). The drum machines need samples; their default tables are pinned
/// by `init_track_holds_the_ctor_defaults` in `tr808::tests` and `tr909::tests`.
#[test]
fn constructor_tracks_hold_the_defaults() {
    let e = Engine::new(tables(), Devices::implemented(None));
    let mut mixer = vec![(1, 0), (2, 0)];
    for c in 0..4u8 {
        mixer.extend([
            (6 + 6 * c, 100),
            (7 + 6 * c, 64),
            (8 + 6 * c, 0),
            (9 + 6 * c, 0),
        ]);
    }
    let seq = |v: &[u8]| {
        v.iter()
            .enumerate()
            .map(|(c, &x)| (c as u8, x))
            .collect::<Vec<_>>()
    };
    assert_eq!(ctor_events(&e, 0), mixer);
    for t in [1, 2] {
        assert_eq!(ctor_events(&e, t), seq(&[1, 0, 64, 20, 90, 20, 100, 64, 0]));
    }
    assert_eq!(ctor_events(&e, 5), seq(&[1, 3, 1, 80, 64]));
    assert_eq!(ctor_events(&e, 6), seq(&[1, 70, 0]));
    assert_eq!(ctor_events(&e, 7), seq(&[1, 70, 64, 40, 24, 90, 1]));
    assert_eq!(ctor_events(&e, 8), seq(&[1, 70, 64]));
}

/// In song mode a mixer controller with no tick-0 event in the song returns to its ctor value
/// when the position goes back before its first event: the mixer's slot 8 (0x100160F0) put
/// that value at tick 0 of the track, and the chase finds it.
#[test]
fn song_mode_chase_restores_the_mixer_ctor_values() {
    let mut e = Engine::new(tables(), Devices::placeholders());
    let f = &mut e.frame;
    f.seq.set_song_mode(1, &mut f.devices);
    let events = [song::TrackEvent {
        tick: 768,
        ctrl: 6,
        value: 50,
    }];
    assert!(f.seq.add_events(0, &events));
    f.seq.locate(768, &mut f.devices);
    assert_eq!(f.devices.mixer.live.channels[0].level, 50);
    f.seq.locate(0, &mut f.devices);
    assert_eq!(f.devices.mixer.live.channels[0].level, 100);
}

/// The load sends each track in the song's batches, and a batch the pool refuses ends it
/// (`sub_10021010`; the exe throws `XOutOfMemory` and sends nothing more): the batches before
/// it stay, the later ones and msg 32 are not sent. The split decides: one batch of the whole
/// track would be refused at once.
#[test]
fn load_stops_at_a_refused_batch() {
    // after tick 0 (the ctor's events) on tracks of quantisation 1: every event takes a node
    let ev = |i: u32| song::TrackEvent {
        tick: 24 * (i / 128 + 1),
        ctrl: (i % 128) as u8,
        value: 1,
    };
    let mut s = crate::load::convert(&rbs::Song::empty()).unwrap();
    // playing, so that msg 36's stop does not prune the fill's repeated values
    s.globals.play = 1;
    s.tracks = Default::default();
    s.tracks[0] = vec![song::TrackEvent {
        tick: 24,
        ctrl: 6,
        value: 3,
    }];
    s.tracks[5] = (0..2000).map(ev).collect();
    s.tracks[6] = (0..5).map(ev).collect();
    let fresh = || {
        let mut e = Engine::new(tables(), Devices::placeholders());
        // another list holds all but 1500 nodes (in the DLL: the other songs of the process)
        let free = e.frame.seq.free_nodes();
        let fill: Vec<_> = (0..(free - 1500) as u32).map(ev).collect();
        assert!(e.frame.seq.add_events(8, &fill));
        assert_eq!(e.frame.seq.free_nodes(), 1500);
        e
    };
    let batch = |track, len| song::Batch { track, len };
    let events = |e: &Engine, t| e.frame.seq.events(t).len();
    // the exe's plan: track 0, then 1284 + 716 events of track 5, then track 6
    s.batches = vec![batch(0, 1), batch(5, 1284), batch(5, 716), batch(6, 5)];
    let mut e = fresh();
    let before = [0, 5, 6].map(|t| events(&e, t));
    let want = LoadRefused {
        track: 5,
        first: 1284,
        count: 716,
        free: 1500 - 1 - 1284,
    };
    assert_eq!(e.load(&s), Err(want));
    assert_eq!(e.load_refused(), Some(&want));
    assert_eq!(events(&e, 0), before[0] + 1);
    assert_eq!(events(&e, 5), before[1] + 1284);
    assert_eq!(events(&e, 6), before[2]);
    assert_eq!(e.frame.seq.free_nodes(), 215);
    // no plan: one batch per track, and track 5's 2000 are refused whole
    s.batches.clear();
    let mut e = fresh();
    let err = e.load(&s).unwrap_err();
    assert_eq!(
        (err.track, err.first, err.count, err.free),
        (5, 0, 2000, 1499)
    );
    assert_eq!(events(&e, 5), before[1]);
    // a plan that leaves events over sends them as one more batch per track
    s.tracks[5].truncate(1000);
    s.batches = vec![batch(5, 600)];
    let mut e = fresh();
    assert_eq!(e.load(&s), Ok(()));
    assert_eq!(e.load_refused(), None);
    assert_eq!(events(&e, 5), before[1] + 1000);
    assert_eq!(events(&e, 6), before[2] + 5);
    assert_eq!(e.frame.seq.free_nodes(), 1500 - 1 - 1000 - 5);
}

/// A file of more than 100000 events, read by the loader into batches of 1284 file events:
/// the load stops at the first batch longer than the free count, on a batch boundary.
#[test]
fn a_file_above_the_pool_is_refused() {
    // values alternate per position, so that no stop prune frees nodes
    let events = |n: u32, ctrls: [u8; 2]| -> Vec<rbs::Event> {
        (0..n)
            .map(|i| rbs::Event {
                pos: i / 2,
                ctrl: ctrls[(i % 2) as usize],
                value: (i / 2 % 2) as u8 + 1,
            })
            .collect()
    };
    let mut f = rbs::Song::empty();
    // delay feedback and pan, distortion amount and shape
    f.tracks[5].events = events(60_000, [3, 4]);
    f.tracks[6].events = events(41_000, [1, 2]);
    let s = crate::load::convert(&f).unwrap();
    assert_eq!(s.tracks[5].len() + s.tracks[6].len(), 101_000);
    let mut e = Engine::new(tables(), Devices::placeholders());
    let ctor = e.frame.seq.events(6).len();
    let err = e.load(&s).unwrap_err();
    // track 5's 47 batches and track 6's 31 full ones pass; its last, of 1196, is refused
    assert_eq!(
        (err.track, err.first, err.count),
        (6, 31 * 1284, 41_000 % 1284)
    );
    assert!(err.free < err.count);
    assert_eq!(e.frame.seq.events(6).len(), ctor + 31 * 1284);
    assert_eq!(e.frame.seq.free_nodes(), err.free as usize);
    // half of it loads
    f.tracks[6].events.clear();
    let s = crate::load::convert(&f).unwrap();
    let mut e = Engine::new(tables(), Devices::placeholders());
    assert_eq!(e.load(&s), Ok(()));
}

#[test]
fn pan_law_and_sends() {
    let mut e = engine_with(Devices::placeholders(), EngineState::default());
    let t = tables().level;
    let g = 1e-6f32 as f64;
    let p = &mut e.frame.devices.mixer.live;
    p.master = 127;
    p.channels[0] = ChannelParams {
        enabled: 1,
        level: 127,
        pan: 0,
        send: 127,
        dist: 0,
    };
    p.channels[1] = ChannelParams {
        enabled: 1,
        level: 127,
        pan: 127,
        send: 64,
        dist: 0,
    };
    p.channels[2] = ChannelParams {
        enabled: 0,
        level: 127,
        pan: 64,
        send: 127,
        dist: 0,
    };
    p.channels[3] = ChannelParams {
        enabled: 1,
        level: 100,
        pan: 96,
        send: 0,
        dist: 0,
    };
    e.frame.mixer_update();
    let c = &e.frame.state.channels;
    // hard left: d = -64, q = 128, R = T[0] = 0
    assert_eq!(c[0].target_l, (g + 1.0) as f32);
    assert_eq!(c[0].target_r, GUARD);
    assert_eq!(c[0].send, (1.0 + g) as f32);
    // hard right: d = 63, q = 124, L = T[127 - 124]
    assert_eq!(c[1].target_l, (g + t[3] as f64) as f32);
    assert_eq!(c[1].target_r, (g + 1.0) as f32);
    assert_eq!(c[1].send, (t[64] as f64 + g) as f32);
    // off: the level is 0
    assert_eq!(
        (c[2].target_l, c[2].target_r, c[2].send),
        (GUARD, GUARD, GUARD)
    );
    // d = 32, q = 32: L = T[100 - 32]
    assert_eq!(c[3].target_l, (g + t[68] as f64) as f32);
    assert_eq!(c[3].target_r, (g + t[100] as f64) as f32);
    assert_eq!(e.frame.state.master_target, (2.0 + g) as f32);

    // Vintage: (PAN.L + PAN.R) * T[send] * 0.5 + 1e-6
    e.frame.vintage = true;
    e.frame.mixer_update();
    let c = &e.frame.state.channels;
    assert_eq!(c[0].send, (1.0 * 0.5 + g) as f32);
    assert_eq!(
        c[1].send,
        ((t[3] as f64 + 1.0) * t[64] as f64 * 0.5 + g) as f32
    );
    assert_eq!(c[3].send, GUARD);
}

#[test]
fn smoothing_and_ramp() {
    let mut d = Devices::placeholders();
    d.channels[0] = Box::new(Const(1.0));
    let mut s = quiet_state();
    s.mixer.channels[0].target_l = 1.0;
    s.mixer.channels[0].target_r = 0.5;
    let mut e = engine_with(d, s);
    let (l, r) = block_once(&mut e);
    let ch = &e.frame.state.channels[0];
    // new = 0*0.93 + target*0.07, stored as f32
    assert_eq!(ch.gain_l, 0.07f64 as f32);
    assert_eq!(ch.gain_r, (0.5 * 0.07f64) as f32);
    // the master: 1*0.93 + 1*0.07, unrounded
    let m = 0.93f64 + 0.07;
    // pair p uses the gain after p steps of 0.07/32 (the ramp starts at the old gain)
    let (dl, dr) = (0.07 / 32.0, 0.5 * 0.07 / 32.0);
    let (mut gl, mut gr) = (0.0f64, 0.0f64);
    for p in 0..32 {
        let want_l = ((gl + 1e-6f32 as f64) as f32 as f64 * m) as f32;
        let want_r = ((gr + 1e-6f32 as f64) as f32 as f64 * m) as f32;
        assert_eq!((l[2 * p], l[2 * p + 1]), (want_l, want_l), "pair {p}");
        assert_eq!((r[2 * p], r[2 * p + 1]), (want_r, want_r), "pair {p}");
        gl += dl;
        gr += dr;
    }
    assert_eq!(l[0], GUARD);
    // the last pair: 31 steps, 0.0678125 (+ 1e-6), not yet the stored 0.07
    assert!((l[63] as f64 - 0.0678135).abs() < 1e-7);
    // the next block ramps from the stored f32, towards 0.07 + 0.07*0.93
    let (l2, _) = block_once(&mut e);
    assert_eq!(
        l2[0],
        ((0.07f32 as f64 + 1e-6f32 as f64) as f32 as f64 * m) as f32
    );
}

#[test]
fn smoothing_settles_on_an_f32_fixed_point() {
    let mut d = Devices::placeholders();
    d.channels[0] = Box::new(Const(0.25));
    let mut s = quiet_state();
    s.mixer.channels[0].target_l = 0.8;
    let mut e = engine_with(d, s);
    for _ in 0..2000 {
        block_once(&mut e);
    }
    // (the updates have replaced the target by the default level's)
    let ch = e.frame.state.channels[0];
    let g = ch.gain_l;
    assert_eq!(((g as f64) * 0.93 + ch.target_l as f64 * 0.07) as f32, g);
    assert!((g - ch.target_l).abs() < 1e-6);
}

#[test]
fn send_accumulation() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log: Log = Rc::default();
    let mut d = Devices::placeholders();
    d.channels[0] = Box::new(Const(0.5));
    d.channels[2] = Box::new(Const(-0.75));
    d.delay = Box::new(Recorder {
        seen: seen.clone(),
        log: log.clone(),
    });
    let mut s = quiet_state();
    s.mixer.channels[0].send = 0.25;
    s.mixer.channels[2].send = 0.5;
    let mut e = engine_with(d, s);
    block_once(&mut e);
    block_once(&mut e);
    let seen = seen.borrow();
    // from the guard: ((1e-6 + 0.5*0.25) + 0*1e-6) + (-0.75*0.5), each sum stored as f32
    let a = (0.125 + 1e-6f32 as f64) as f32;
    let want = (-0.375 + a as f64) as f32;
    assert_eq!(seen[0], [want; BLOCK]);
    // the send effect refilled the bus, so the second block starts from the guard again
    assert_eq!(seen[1], seen[0]);
}

#[test]
fn routing_order() {
    let log: Log = Rc::default();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut d = Devices::placeholders();
    d.channels[1] = Box::new(Const(0.0));
    d.dist = Box::new(Op {
        name: "dist",
        f: |x| x + 1.0,
        log: log.clone(),
    });
    d.pcf = Box::new(Op {
        name: "pcf",
        f: |x| x * 2.0,
        log: log.clone(),
    });
    d.comp = Box::new(Op {
        name: "comp",
        f: |x| x + 3.0,
        log: log.clone(),
    });
    d.delay = Box::new(Recorder {
        seen: seen.clone(),
        log: log.clone(),
    });
    let mut e = engine_with(d, quiet_state());
    e.frame.devices.mixer.live.channels[1].dist = 1;
    e.frame.devices.mixer.live.pcf_id = 2; // channel 1
    e.frame.devices.mixer.live.comp_id = 3; // channel 1
    e.frame.mixer_update();
    assert_eq!(*log.borrow(), ["dist.update", "pcf.update", "comp.update"]);
    let r = e.frame.state.channels.map(|c| c.routing);
    assert_eq!(
        r[1],
        Routing {
            dist: true,
            pcf: true,
            comp: true,
            ..Routing::default()
        }
    );
    assert_eq!(r[0], Routing::default());
    assert!(!e.frame.state.master_comp);
    // the update set the send gains; give channel 1 a unit send and nothing else
    for ch in &mut e.frame.state.channels {
        ch.send = 0.0;
    }
    e.frame.state.channels[1].send = 1.0;
    log.borrow_mut().clear();
    block_once(&mut e);
    assert_eq!(*log.borrow(), ["dist", "pcf", "comp", "delay"]);
    // Dist, then PCF, then Comp: ((0 + 1) * 2) + 3
    assert_eq!(seen.borrow()[0][0], (5.0 + 1e-6f32 as f64) as f32);

    // the compressor on the master runs after the send effect
    e.frame.devices.mixer.live.comp_id = 1;
    e.frame.devices.mixer.live.channels[1].dist = 0;
    e.frame.mixer_update();
    assert!(e.frame.state.master_comp);
    assert!(!e.frame.state.channels[1].routing.comp);
    log.borrow_mut().clear();
    block_once(&mut e);
    assert_eq!(*log.borrow(), ["pcf", "delay", "comp.stereo"]);
}

#[test]
fn update_phase() {
    let mut e = Engine::new(tables(), Devices::placeholders());
    let before = e.frame.state.channels[0].target_l;
    e.frame.devices.mixer.live.channels[0].level = 127;
    let mut buf = vec![0i16; 2 * 1024];
    // counter 10..15: no update in the first six blocks
    e.render(&mut buf, 6 * BLOCK);
    assert_eq!(e.frame.state.counter, 16);
    assert_eq!(e.frame.state.channels[0].target_l, before);
    // the seventh block runs it first
    e.render(&mut buf, BLOCK);
    assert_eq!(e.frame.state.counter, 17);
    assert_eq!(
        e.frame.state.channels[0].target_l,
        (1e-6f32 as f64 + 1.0) as f32
    );
    // then every 16 blocks
    e.frame.devices.mixer.live.channels[0].level = 0;
    e.render(&mut buf, 15 * BLOCK);
    assert_eq!(
        e.frame.state.channels[0].target_l,
        (1e-6f32 as f64 + 1.0) as f32
    );
    e.render(&mut buf, BLOCK);
    assert_eq!(e.frame.state.channels[0].target_l, GUARD);
    assert_eq!(e.frame.state.counter, 33);
}

fn loud_engine() -> Engine {
    let mut d = Devices::placeholders();
    d.channels[0] = Box::new(Ramp(0));
    d.channels[3] = Box::new(Const(-0.3));
    let mut e = Engine::new(tables(), d);
    for ch in &mut e.frame.state.channels {
        ch.gain_l = ch.target_l;
        ch.gain_r = ch.target_r;
    }
    e
}

#[test]
fn leftover_buffer() {
    let mut whole = loud_engine();
    let mut a = vec![0i16; 2 * 228];
    whole.render(&mut a, 228);
    assert_eq!(whole.output.leftover_pos, 36);
    assert!(a.iter().any(|&x| x != 0));

    let mut split = loud_engine();
    let mut b = Vec::new();
    let mut buf = vec![0i16; 2 * 228];
    for (n, lp) in [(100, 36), (28, 0), (100, 36)] {
        split.render(&mut buf, n);
        assert_eq!(split.output.leftover_pos, lp);
        b.extend_from_slice(&buf[..2 * n]);
    }
    assert_eq!(a, b);
    // the same blocks were rendered: four in each
    assert_eq!(whole.frame.state.counter, split.frame.state.counter);

    // a 0-frame call drops the rest of the leftover block, and the position goes negative
    split.render(&mut [], 0);
    assert_eq!(split.output.leftover_pos, 36 - 64);
    let c0 = split.frame.state.counter;
    split.render(&mut buf, 64);
    assert_eq!(split.output.leftover_pos, 0);
    assert_eq!(split.frame.state.counter, c0 + 1);
    // the copied part of the leftover block is refilled with the guard
    split.render(&mut buf, 10);
    assert_eq!(split.output.leftover[0][..10], [GUARD; 10]);
    // the clamp
    let mut big = vec![0i16; 2 * (MAX_FRAMES + 64)];
    let c0 = split.frame.state.counter;
    split.render(&mut big, MAX_FRAMES + 64);
    assert_eq!(split.frame.state.counter, c0 + 256);
}

/// An insert that faults on every call from its `from`-th on (counted from 0), as the PCF with
/// a zero pattern length does on every call.
struct Faulty {
    log: Log,
    calls: u32,
    from: u32,
}
impl SongDevice for Faulty {}
impl InsertEffect for Faulty {
    fn process(&mut self, bus: &mut Block, _ctx: &BlockContext) {
        self.log.borrow_mut().push("faulty".into());
        bus.fill(9.0);
        self.calls += 1;
    }
    fn faulted(&mut self) -> Option<Fault> {
        (self.calls > self.from).then_some(Fault::PcfZeroLength { wave: 127 })
    }
}

#[test]
fn a_fault_ends_the_block_and_the_call() {
    let log: Log = Rc::default();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut e = loud_engine();
    e.frame.devices.pcf = Box::new(Faulty {
        log: log.clone(),
        calls: 0,
        from: 0,
    });
    e.frame.devices.comp = Box::new(Op {
        name: "comp",
        f: |x| x,
        log: log.clone(),
    });
    e.frame.devices.delay = Box::new(Recorder {
        seen: seen.clone(),
        log: log.clone(),
    });
    // the PCF and the compressor on channel 0
    e.frame.devices.mixer.live.pcf_id = 1;
    e.frame.devices.mixer.live.comp_id = 2;
    e.frame.mixer_update();
    log.borrow_mut().clear();
    let c0 = e.frame.state.counter;
    let lp = e.output.leftover_pos;
    // the int16 buffer keeps what the caller had in it, and the leftover position stands
    let mut buf = vec![7i16; 2 * 100];
    e.render(&mut buf, 100);
    assert_eq!(buf, vec![7i16; 2 * 100]);
    assert_eq!(e.output.leftover_pos, lp);
    // one block ran up to the fault: no compressor, no later channel, no send effect
    assert_eq!(*log.borrow(), ["faulty"]);
    assert_eq!(e.frame.state.counter, c0 + 1);
    assert!(seen.borrow().is_empty());
    // every later call faults the same way
    e.render(&mut buf, 100);
    assert_eq!(*log.borrow(), ["faulty", "faulty"]);
    assert_eq!(buf, vec![7i16; 2 * 100]);
    // the record: the first fault, its block, and both calls
    let b0 = e.frame.blocks - 2;
    assert_eq!(
        e.fault(),
        Some(&FaultRecord {
            first: Fault::PcfZeroLength { wave: 127 },
            block: b0,
            calls: 2,
        })
    );
    assert_eq!(
        Fault::PcfZeroLength { wave: 55 }.to_string(),
        "the PCF's wave 55 has pattern length 0 and divides by zero (0x10016811)"
    );
}

/// One int16 buffer serves the idle calls and every chunk, as in rbhost (`chunk_buf`) and the
/// app's export loop (the `new(0x2000)` buffer of 0x427C2F, reused by every call at
/// 0x4281C3): a chunk whose call faults keeps the previous chunk's samples.
#[test]
fn a_faulting_chunk_repeats_the_buffer() {
    let log: Log = Rc::default();
    let mut e = loud_engine();
    // fault from the first block of the second 2048-frame chunk on (32 blocks per chunk)
    e.frame.devices.pcf = Box::new(Faulty {
        log,
        calls: 0,
        from: 32,
    });
    e.frame.devices.mixer.live.pcf_id = 1;
    e.frame.mixer_update();
    let r = ExportRange {
        tempo: 120_000,
        loop_start: 0,
        loop_end: 768,
    };
    let s = export::export(&mut e, r, 0);
    let c = 2 * CHUNK_FRAMES as usize;
    assert!(s[..c].iter().any(|&x| x != 0));
    assert_eq!(s[c..2 * c], s[..c]);
    assert_eq!(e.fault().unwrap().calls as usize, chunk_sizes(r).len() - 1);
}

/// Marks the global delay line: a value that is not the guard and the gate cleared.
fn dirty_delay_line(p: &ProcessState) {
    let mut l = p.delay_line();
    l.buf.fill(0.5);
    l.gate = false;
    p.set_delay_line(l);
}

fn assert_refilled(p: &ProcessState) {
    let l = p.delay_line();
    assert!(l.gate);
    assert!(l.buf.iter().all(|&x| x.to_bits() == GUARD.to_bits()));
}

#[test]
fn msg53_replacing_a_song_refills_the_delay_line() {
    let stopped = Activation::ReplacesCurrent {
        current_playing: false,
    };
    for a in [stopped, Activation::NoneCurrent] {
        let mut e = Engine::new(tables(), Devices::implemented(None));
        let p = e.process_state().clone();
        dirty_delay_line(&p);
        e.activate(a);
        assert_refilled(&p);
    }
}

#[test]
fn msg53_over_a_playing_song_leaves_the_delay_line() {
    // 0x1001BDCC: the current song's play flag set, the routine returns before slot 6
    let mut e = Engine::new(tables(), Devices::implemented(None));
    let p = e.process_state().clone();
    dirty_delay_line(&p);
    let before = p.delay_line();
    e.activate(Activation::ReplacesCurrent {
        current_playing: true,
    });
    let l = p.delay_line();
    assert!(!l.gate);
    assert!(l.buf.iter().eq(before.buf.iter()));
}

#[test]
fn msg33_reset() {
    let log: Log = Rc::default();
    let mut d = Devices::placeholders();
    d.comp = Box::new(Op {
        name: "comp",
        f: |x| x,
        log: log.clone(),
    });
    let mut e = loud_engine();
    e.frame.devices.comp = d.comp;
    let p = e.process_state().clone();
    e.frame.devices.delay = Box::new(crate::delay::Delay::new(p.clone()));
    let mut buf = vec![0i16; 2 * 64];
    e.render(&mut buf, 40);
    e.frame.state.master_peak = [0.5, 0.5];
    e.frame.state.send = [0.25; BLOCK];
    dirty_delay_line(&p);
    let before = e.state();
    e.reset();
    // the delay's slot 5 (0x1000F4F0) runs slot 6: the global line refilled, the gate set
    assert_refilled(&p);
    let s = e.state();
    assert_eq!(s.mixer.master, 0.0);
    assert_eq!(s.mixer.master_target, before.mixer.master_target);
    assert_eq!(s.mixer.master_peak, [0.0; 2]);
    assert_eq!(s.mixer.send, [GUARD; BLOCK]);
    assert_eq!(s.output.leftover, [[0.0; BLOCK]; 2]);
    assert_eq!(s.output.leftover_pos, 40);
    assert_eq!(s.mixer.counter, before.mixer.counter);
    assert_eq!(s.mixer.channels, before.mixer.channels);
    assert_eq!(*log.borrow(), ["comp.reset"]);
}

#[test]
fn export_chunks() {
    // 120 BPM, two bars: 4 s
    let r = ExportRange {
        tempo: 120_000,
        loop_start: 768,
        loop_end: 3 * 768,
    };
    let c = chunk_sizes(r);
    assert!(c[..c.len() - 1].iter().all(|&n| n == 2048));
    assert!(*c.last().unwrap() < 2048);
    assert_eq!(frame_count(r), 176_400);
    // an empty loop still makes one 0-frame call
    let r = ExportRange {
        tempo: 120_000,
        loop_start: 768,
        loop_end: 768,
    };
    assert_eq!(chunk_sizes(r), [0]);
    // 140 BPM, one bar: 60/140*4*44100 = 75600 frames, less one when the accumulation
    // truncates below it
    let r = ExportRange {
        tempo: 140_000,
        loop_start: 0,
        loop_end: 768,
    };
    let n = frame_count(r);
    assert!(n == 75_600 || n == 75_599, "{n}");
}

#[test]
fn export_of_silence_is_zero() {
    let mut e = Engine::new(tables(), Devices::placeholders());
    let r = ExportRange {
        tempo: 120_000,
        loop_start: 0,
        loop_end: 768,
    };
    let s = export::export(&mut e, r, 160);
    assert_eq!(s.len(), 2 * frame_count(r));
    assert!(s.iter().all(|&x| x == 0));
    // the final 0-frame call dropped the last chunk's leftover
    assert!(e.output.leftover_pos <= 0);
    // msg 33 ran after the chunks
    assert_eq!(e.frame.state.master, 0.0);
}

#[test]
fn the_live_message_entry_points() {
    let mut e = Engine::new(tables(), Devices::implemented(None));
    // msg 145 {0,v} and msg 147 {c,0,v}: the master and a channel's enabled byte, live
    e.frame.devices.mixer.set_master(33);
    e.frame.devices.mixer.set_enabled(2, 0);
    e.frame.devices.mixer.set_enabled(9, 0);
    let live = e.frame.devices.mixer.live;
    assert_eq!(live.master, 33);
    assert_eq!(live.channels.map(|c| c.enabled), [1, 1, 0, 1]);
    assert_eq!(e.frame.devices.mixer.pattern, MixerParams::default());
    // slot 11 through Engine::set_param
    e.set_param(song::Device::Mixer, 7, 5);
    assert_eq!(e.frame.devices.mixer.live.channels[0].pan, 5);
    e.set_param(song::Device::Delay, 3, 99);
    assert_eq!(e.frame.devices.delay.live()[3], 99);
    // msg 35 id 21
    e.set_vintage(1);
    assert!(e.frame.vintage && e.frame.seq.globals.vintage == 1);
    e.set_vintage(0);
    assert!(!e.frame.vintage);
    // the msg 35 setters
    e.set_tempo(99_000);
    e.set_shuffle(7);
    e.set_loop_on(1);
    e.set_loop_start(768);
    e.set_loop_end(1536);
    let g = &e.frame.seq.globals;
    assert_eq!(
        (g.tempo, g.shuffle, g.loop_on, g.loop_start, g.loop_end),
        (99_000, 7, 1, 768, 1536)
    );
    // play, then halt: no bar locate
    e.set_song_mode(1);
    e.play();
    assert!(e.playing());
    let mut buf = vec![0i16; 2 * 1024];
    e.render(&mut buf, 1024);
    let t = e.frame.seq.tick_pos();
    assert!(t > 0);
    e.halt();
    assert!(!e.playing());
    assert_eq!(e.frame.seq.tick_pos(), t);
}

/// An engine with every device implemented (silent drum samples are enough: no audio is read).
fn edit_engine() -> Engine {
    use crate::samples::{COUNT, Sample, SampleSet};
    let s = Arc::new(SampleSet::from_samples(
        (0..COUNT).map(|_| Sample::from_audio(&[0; 16])).collect(),
    ));
    Engine::new(tables(), Devices::implemented(Some(s)))
}

fn patterns(e: &mut Engine, d: song::Device) -> Vec<u8> {
    let c = d as usize - 1;
    e.frame.devices.channels[c].patterns_mut().unwrap().to_vec()
}

#[test]
fn device_param_messages() {
    use song::Device as D;
    let mut e = edit_engine();
    // 303 msg 40: controllers 0..8 as slot 11, 9 nothing, 10 the unused byte 10
    for c in 0..=8 {
        e.device_param(D::Tb303_1, c, 100 + c);
    }
    e.device_param(D::Tb303_1, 9, 55);
    e.device_param(D::Tb303_1, 10, 66);
    e.device_param(D::Tb303_1, 11, 77);
    let live = e.frame.devices.channels[0].live().to_vec();
    assert_eq!(&live[..9], &[100, 101, 102, 103, 104, 105, 106, 107, 108]);
    assert_eq!(&live[9..], &[0, 66, 0]);
    // 808 msg 57: 0..28, 29 nothing, 30 the unused byte
    e.device_param(D::Tr808, 28, 9);
    e.device_param(D::Tr808, 29, 9);
    e.device_param(D::Tr808, 30, 9);
    e.device_param(D::Tr808, 31, 9);
    let l = e.frame.devices.channels[2].live();
    assert_eq!((l[28], l[29], l[30], l[31]), (9, 0, 9, 0));
    // 909 msg 74: 20..22 permuted as slot 11, 29 flam, 30 nothing, 31 the unused byte
    e.device_param(D::Tr909, 20, 1);
    e.device_param(D::Tr909, 21, 2);
    e.device_param(D::Tr909, 22, 3);
    e.device_param(D::Tr909, 29, 4);
    e.device_param(D::Tr909, 30, 5);
    e.device_param(D::Tr909, 31, 6);
    e.device_param(D::Tr909, 32, 7);
    let l = e.frame.devices.channels[3].live();
    assert_eq!(&l[20..], &[3, 1, 2, 32, 127, 100, 64, 100, 64, 4, 0, 6, 0]);
    // the effects: slot 11's controllers, nothing beyond
    for (d, n) in [
        (D::Delay, 5),
        (D::Distortion, 3),
        (D::Pcf, 7),
        (D::Compressor, 3),
    ] {
        let before = e.frame.devices.device_mut(d).live().to_vec();
        for c in 0..=n {
            e.device_param(d, c, 11 + c);
        }
        let after = e.frame.devices.device_mut(d).live().to_vec();
        for (c, (&b, &a)) in before.iter().zip(&after).enumerate() {
            let want = if c < n as usize { 11 + c as u8 } else { b };
            assert_eq!(a, want, "{d:?} byte {c}");
        }
    }
    // the mixer has no {ctrl, v} message
    let m = e.frame.devices.mixer;
    e.device_param(D::Mixer, 7, 1);
    assert_eq!(e.frame.devices.mixer, m);
}

#[test]
fn mixer_messages() {
    let mut e = edit_engine();
    let pattern = e.frame.devices.mixer.pattern;
    e.mixer_global(0, 90);
    e.mixer_global(1, 3);
    e.mixer_global(2, 4);
    e.mixer_global(3, 9);
    e.mixer_global(4, 9);
    let l = e.frame.devices.mixer.live;
    assert_eq!((l.master, l.comp_id, l.pcf_id), (90, 3, 4));
    for k in 0..=5 {
        e.mixer_channel(2, k, 10 + k);
    }
    e.mixer_channel(4, 1, 1);
    let ch = e.frame.devices.mixer.live.channels;
    assert_eq!(
        (
            ch[2].enabled,
            ch[2].level,
            ch[2].pan,
            ch[2].send,
            ch[2].dist
        ),
        (10, 11, 12, 13, 14)
    );
    assert_eq!(ch[0], MixerParams::default().channels[0]);
    assert_eq!(ch[3], MixerParams::default().channels[3]);
    // live only
    assert_eq!(e.frame.devices.mixer.pattern, pattern);
}

#[test]
fn pattern_messages_303() {
    use song::Device as D;
    let mut e = edit_engine();
    let base = patterns(&mut e, D::Tb303_2);
    let mut want = base.clone();
    // msg 42: field 0 shuffle, 1 length, other fields nothing
    e.pattern_field(D::Tb303_2, 3, 0, 1);
    e.pattern_field(D::Tb303_2, 3, 1, 7);
    e.pattern_field(D::Tb303_2, 3, 2, 9);
    want[98 * 3] = 1;
    want[98 * 3 + 1] = 7;
    // msg 43: both header bytes
    e.pattern_header(D::Tb303_2, 4, [1, 5]);
    want[98 * 4] = 1;
    want[98 * 4 + 1] = 5;
    // msg 45: fields 0..5 of a step; field 6 nothing
    for f in 0..=6 {
        e.step_field(D::Tb303_2, 5, 15, f, 20 + f);
    }
    for f in 0..6 {
        want[98 * 5 + 2 + 6 * 15 + f] = 20 + f as u8;
    }
    // msg 46: a whole step; a wrong length is ignored
    e.step(D::Tb303_2, 6, 0, &[1, 2, 3, 4, 5, 6]);
    e.step(D::Tb303_2, 6, 1, &[1, 2, 3]);
    want[98 * 6 + 2..98 * 6 + 8].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    // msg 44: a whole pattern
    let pat: Vec<u8> = (0..98).map(|i| i as u8).collect();
    e.pattern(D::Tb303_2, 31, &pat);
    e.pattern(D::Tb303_2, 30, &pat[..97]);
    want[98 * 31..].copy_from_slice(&pat);
    // unbounded steps land in the next pattern, beyond the array nothing
    e.step_field(D::Tb303_2, 7, 16, 0, 99);
    want[98 * 7 + 2 + 96] = 99;
    e.step_field(D::Tb303_2, 31, 16, 0, 99);
    e.pattern_field(D::Tb303_2, 32, 0, 99);
    e.pattern(D::Tb303_2, 32, &pat);
    assert_eq!(patterns(&mut e, D::Tb303_2), want);
    // the other 303 is untouched
    assert_eq!(patterns(&mut e, D::Tb303_1), base);
}

#[test]
fn pattern_messages_drums() {
    use song::Device as D;
    let mut e = edit_engine();
    for d in [D::Tr808, D::Tr909] {
        let mut want = patterns(&mut e, d);
        e.pattern_field(d, 2, 1, 12);
        want[194 * 2 + 1] = 12;
        e.pattern_header(d, 1, [1, 3]);
        want[194] = 1;
        want[195] = 3;
        // msgs 62 / 79: any instrument byte (12 is the next step's accent)
        e.step_field(d, 0, 3, 11, 3);
        e.step_field(d, 0, 3, 12, 2);
        want[2 + 12 * 3 + 11] = 3;
        want[2 + 12 * 4] = 2;
        // msgs 63 / 80
        let st: Vec<u8> = (1..=12).collect();
        e.step(d, 9, 15, &st);
        e.step(d, 9, 14, &st[..6]);
        want[194 * 9 + 2 + 12 * 15..194 * 10].copy_from_slice(&st);
        // msgs 61 / 78
        let pat = vec![1u8; 194];
        e.pattern(d, 0, &pat);
        e.pattern(d, 5, &pat[..98]);
        want[..194].copy_from_slice(&pat);
        assert_eq!(patterns(&mut e, d), want, "{d:?}");
    }
    // the mixer and the effects have no patterns
    let before = e.frame.devices.delay.live().to_vec();
    e.pattern_field(D::Delay, 0, 0, 1);
    e.step_field(D::Mixer, 0, 0, 0, 1);
    assert_eq!(e.frame.devices.delay.live(), &before[..]);
}

/// A pattern edit is heard the next time the step plays: the 303 plays the edited note.
#[test]
fn a_step_edit_is_heard() {
    use song::Device as D;
    let render = |edit: bool| {
        let mut e = edit_engine();
        e.set_song_mode(0);
        // pattern 0, step 0: a note (rest off) at note 0 or 12
        e.step_field(D::Tb303_1, 0, 0, 5, 0);
        if edit {
            e.step_field(D::Tb303_1, 0, 0, 0, 12);
        }
        e.play();
        let mut buf = vec![0i16; 2 * 4096];
        e.render(&mut buf, 4096);
        buf
    };
    let (a, b) = (render(false), render(true));
    assert!(a.iter().any(|&x| x != 0));
    assert_ne!(a, b);
}

/// A source that leaves `w` at its entry `esp - 0x1C` (as the 808 and 909 audio do), or nothing.
struct Word(Option<i32>);
impl SongDevice for Word {}
impl ChannelDevice for Word {
    fn process(&mut self, _bus: &mut Block, _ctx: &BlockContext) {}
    fn stack_word(&self) -> Option<i32> {
        self.0
    }
}

/// An insert that records the stack word it is called with and leaves `w` (or nothing).
struct SlotProbe {
    seen: Rc<RefCell<Vec<Option<i32>>>>,
    w: Option<i32>,
}
impl SongDevice for SlotProbe {}
impl InsertEffect for SlotProbe {
    fn process(&mut self, _bus: &mut Block, ctx: &BlockContext) {
        self.seen.borrow_mut().push(ctx.stack_word);
    }
    fn stack_word(&self, prev: Option<i32>) -> Option<i32> {
        self.w.or(prev)
    }
}

/// A send effect that leaves the output pointer, as the enabled delay does, or nothing.
struct OutSend(bool);
impl SongDevice for OutSend {}
impl SendEffect for OutSend {
    fn process(&mut self, send: &mut Block, _l: &mut Block, _r: &mut Block, _ctx: &BlockContext) {
        send.fill(GUARD);
    }
    fn stack_word(&self, out_end: Option<i32>, prev: Option<i32>) -> Option<i32> {
        if self.0 { out_end } else { prev }
    }
}

const W909: i32 = 0x1002_B8B4;

fn slot_engine(delay: bool) -> (Engine, Rc<RefCell<Vec<Option<i32>>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut d = Devices::placeholders();
    d.channels[3] = Box::new(Word(Some(W909)));
    d.pcf = Box::new(SlotProbe {
        seen: seen.clone(),
        w: None,
    });
    d.delay = Box::new(OutSend(delay));
    let mut e = engine_with(d, quiet_state());
    e.frame.devices.mixer.live.pcf_id = 1; // channel 0, a 303's
    e.frame.devices.mixer.live.comp_id = 1; // the master
    e.frame.mixer_update();
    (e, seen)
}

#[test]
fn the_stale_slot_through_render_calls() {
    let (mut e, seen) = slot_engine(true);
    // blocks 1..3 of the call see the delay's last output pointer of the block before; the
    // mixer update of the third (counter 16) leaves the compressor id
    e.frame.state.counter = 14;
    let mut buf = vec![0i16; 2 * 4 * BLOCK];
    e.render(&mut buf, 4 * BLOCK);
    let out = |k: i32| Some(OUT_RIGHT + 4 * (64 * k + 63));
    assert_eq!(*seen.borrow(), [Some(LOCK_WORD), out(0), Some(1), out(2)]);
    assert_eq!(e.frame.stack_word, out(3));
    // the partial block renders into the leftover block (a heap address): unknown after it
    seen.borrow_mut().clear();
    e.render(&mut buf, 32);
    assert_eq!(*seen.borrow(), [Some(LOCK_WORD)]);
    assert_eq!(e.frame.stack_word, None);
    // a 0-frame call takes the lock too
    e.render(&mut buf, 0);
    assert_eq!(e.frame.stack_word, Some(LOCK_WORD));
    // with the delay off, the 909 audio's return address is the block's last word
    let (mut e, seen) = slot_engine(false);
    e.render(&mut buf, 3 * BLOCK);
    assert_eq!(*seen.borrow(), [Some(LOCK_WORD), Some(W909), Some(W909)]);
}

#[test]
fn the_clock_and_an_earlier_insert_write_the_slot() {
    let (mut e, seen) = slot_engine(false);
    e.frame.seq.set_tempo(130_000);
    e.play();
    // a block whose clock reaches a new tick (the step loop) leaves the tempo
    let mut ticked = Vec::new();
    for _ in 0..12 {
        let t = e.frame.seq.tick_pos();
        e.frame.stack_word = Some(LOCK_WORD);
        block_once(&mut e);
        ticked.push(e.frame.seq.tick_pos() != t);
    }
    let want: Vec<_> = ticked
        .iter()
        .map(|&t| Some(if t { 130_000 } else { LOCK_WORD }))
        .collect();
    assert!(ticked.contains(&true) && ticked.contains(&false));
    assert_eq!(*seen.borrow(), want);
    // the compressor on channel 0 (enabled) writes its release factor before the PCF on
    // channel 1
    let seen = Rc::new(RefCell::new(Vec::new()));
    e.frame.devices.comp = Box::new(SlotProbe {
        seen: Rc::new(RefCell::new(Vec::new())),
        w: Some(0x3F7F_9724),
    });
    e.frame.devices.pcf = Box::new(SlotProbe {
        seen: seen.clone(),
        w: None,
    });
    e.frame.devices.mixer.live.comp_id = 2;
    e.frame.devices.mixer.live.pcf_id = 2;
    e.frame.mixer_update();
    block_once(&mut e);
    assert_eq!(*seen.borrow(), [Some(0x3F7F_9724)]);
}

/// The panel's controller messages reach the record hook for the controllers the DLL's
/// handlers hook (task-057), and record off merges them into the tracks.
#[test]
fn panel_messages_record() {
    use song::Device as D;
    let mut e = edit_engine();
    let recorded = |e: &Engine, d: D| -> Vec<(i32, u8, u8)> {
        e.frame
            .seq
            .recorded(d.id() as usize)
            .iter()
            .map(|x| (x.tick, x.ctrl, x.value))
            .collect()
    };
    e.set_song_mode(1);
    e.locate(1000);
    // record off: nothing
    e.device_param(D::Tb303_1, 3, 77);
    assert!(recorded(&e, D::Tb303_1).is_empty());
    e.set_record(1);
    assert!(e.recording());
    for c in [3, 8, 9, 10, 11] {
        e.device_param(D::Tb303_1, c, 70 + c);
    }
    for c in [28, 29, 30] {
        e.device_param(D::Tr808, c, 1);
        e.device_param(D::Tr909, c, 2);
    }
    e.device_param(D::Delay, 5, 9);
    e.device_param(D::Distortion, 3, 9);
    e.device_param(D::Mixer, 7, 9);
    e.mixer_global(0, 50);
    e.mixer_global(1, 1);
    e.mixer_channel(1, 2, 60);
    e.mixer_channel(1, 0, 0);
    // stopped at 1000: down to 984 (the 303's pattern would go to the bar, 768)
    assert_eq!(recorded(&e, D::Tb303_1), [(984, 8, 78), (984, 3, 73)]);
    assert_eq!(recorded(&e, D::Tr808), [(984, 28, 1)]);
    assert_eq!(recorded(&e, D::Tr909), [(984, 29, 2), (984, 28, 2)]);
    assert_eq!(recorded(&e, D::Delay), [(984, 5, 9)]);
    assert_eq!(recorded(&e, D::Distortion), [(984, 3, 9)]);
    assert_eq!(recorded(&e, D::Mixer), [(984, 13, 60), (984, 1, 1)]);
    // record off: merged into the tracks, and chased (the 303's controller 3 is 73 live)
    e.set_record(0);
    assert!(D::ALL.iter().all(|&d| recorded(&e, d).is_empty()));
    assert!(
        e.frame
            .seq
            .events(D::Tb303_1.id() as usize)
            .iter()
            .any(|x| (x.tick, x.ctrl, x.value) == (984, 3, 73))
    );
    assert_eq!(e.frame.devices.channels[0].live()[3], 73);
}

#[test]
fn the_303s_sub_is_recorded_with_its_start_value() {
    use crate::tb303::SUB;
    use song::Device as D;
    let mut e = edit_engine();
    let sub_events = |e: &Engine, rec: bool| -> Vec<(i32, u8)> {
        let i = D::Tb303_2.id() as usize;
        let list = if rec {
            e.frame.seq.recorded(i)
        } else {
            e.frame.seq.events(i)
        };
        list.iter()
            .filter(|x| x.ctrl == SUB)
            .map(|x| (x.tick, x.value))
            .collect()
    };
    // pattern mode: the level changes, nothing is recorded
    e.device_param(D::Tb303_2, SUB, 30);
    assert_eq!(e.frame.devices.channels[1].fork_param(SUB), Some(30));
    assert!(sub_events(&e, false).is_empty());
    e.set_song_mode(1);
    e.locate(1000);
    // song mode without record: still nothing
    e.device_param(D::Tb303_2, SUB, 40);
    assert!(sub_events(&e, false).is_empty());
    // recording: the level so far becomes the start value, the move is recorded
    e.set_record(1);
    e.device_param(D::Tb303_2, SUB, 100);
    assert_eq!(sub_events(&e, false), [(0, 40)]);
    assert_eq!(sub_events(&e, true), [(984, 100)]);
    e.device_param(D::Tb303_2, SUB, 110);
    assert_eq!(sub_events(&e, false), [(0, 40)]);
    e.set_record(0);
    // merged as every controller's recording: after the move, the value from before it
    assert_eq!(sub_events(&e, false), [(0, 40), (984, 110), (1008, 40)]);
    // a locate before the move chases the start value
    e.locate(0);
    assert_eq!(e.frame.devices.channels[1].fork_param(SUB), Some(40));
    // the other 303 has no sub events
    let i = D::Tb303_1.id() as usize;
    assert!(e.frame.seq.events(i).iter().all(|x| x.ctrl != SUB));
}

#[test]
fn the_master_limit_is_recorded_with_its_start_value() {
    use song::Device as D;
    let mut e = edit_engine();
    let amt_events = |e: &Engine| -> Vec<(i32, u8)> {
        e.frame
            .seq
            .events(D::Mixer.id() as usize)
            .iter()
            .filter(|x| x.ctrl == LIMIT_AMT)
            .map(|x| (x.tick, x.value))
            .collect()
    };
    // pattern mode: the key and the knob change, nothing is recorded
    e.mixer_global(LIMIT, 1);
    e.mixer_global(LIMIT_AMT, 30);
    let m = &e.frame.devices.mixer;
    assert_eq!((m.live.limit, m.live.limit_amt), (1, 30));
    assert_eq!(m.fork_param(LIMIT_AMT), Some(30));
    assert!(amt_events(&e).is_empty());
    // recording in song mode: the value so far becomes the start value
    e.set_song_mode(1);
    e.locate(1000);
    e.set_record(1);
    e.mixer_global(LIMIT_AMT, 100);
    e.set_record(0);
    assert_eq!(amt_events(&e), [(0, 30), (984, 100), (1008, 30)]);
    // a locate before the move chases the start value; the key was never recorded
    e.locate(0);
    assert_eq!(e.frame.devices.mixer.live.limit_amt, 30);
    let i = D::Mixer.id() as usize;
    assert!(e.frame.seq.events(i).iter().all(|x| x.ctrl != LIMIT));
    // an .rbsx's value and moves (`fork_track`): live, the pattern copy and the track
    let mut f = edit_engine();
    f.fork_track(D::Mixer, LIMIT_AMT, 90, &[(0, 90), (984, 20)]);
    f.fork_track(D::Mixer, LIMIT, 1, &[]);
    let m = &f.frame.devices.mixer;
    assert_eq!((m.live.limit, m.live.limit_amt), (1, 90));
    assert_eq!((m.pattern.limit, m.pattern.limit_amt), (1, 90));
    assert_eq!(amt_events(&f), [(0, 90), (984, 20)]);
    // a controller the device does not have: nothing
    f.fork_track(D::Tr808, LIMIT, 1, &[(0, 1)]);
    assert!(
        f.frame
            .seq
            .events(D::Tr808.id() as usize)
            .iter()
            .all(|x| x.ctrl != LIMIT)
    );
}

#[test]
fn the_limiter_caps_the_master_and_off_leaves_it() {
    let loud = |limit: u8| {
        let mut e = loud_engine();
        e.frame.devices.mixer.live.master = 127;
        e.frame.devices.mixer.live.limit = limit;
        e.frame.devices.mixer.live.limit_amt = 127;
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        buf[2 * 64 * 20..]
            .iter()
            .fold(0i32, |m, &x| m.max((x as i32).abs()))
    };
    let ceiling = (crate::limiter::CEILING * 32768.0) as i32;
    // AMT 127 drives the master 12 dB up, into the ceiling
    let off = loud(0);
    assert!(off * 3 > ceiling && off < ceiling, "{off}");
    let on = loud(1);
    assert!(on <= ceiling + 1 && on > ceiling - 400, "{on}");
    // off, the master is the original's, bit for bit
    let mut a = loud_engine();
    let mut b = loud_engine();
    b.frame.devices.mixer.live.limit_amt = 127;
    let (mut x, mut y) = (vec![0i16; 2 * 6400], vec![0i16; 2 * 6400]);
    a.render(&mut x, 6400);
    b.render(&mut y, 6400);
    assert_eq!(x, y);
}

/// This fork: a channel's insert at a wet amount below 127 is mixed with the channel's sound;
/// at 127 (the default) it is the original's insert, at 0 the channel as if it were off.
#[test]
fn an_insert_is_mixed_with_the_channel_by_its_wet_amount() {
    let render = |dist: u8, wet: Option<u8>| {
        let mut d = Devices::placeholders();
        d.channels[0] = Box::new(Ramp(0));
        d.dist = Box::new(Op {
            name: "dist",
            f: |x| x * 0.5,
            log: Log::default(),
        });
        let mut e = Engine::new(tables(), d);
        e.frame.devices.mixer.live.channels[0].dist = dist;
        if let Some(w) = wet {
            e.mixer_global(WET, w);
        }
        let mut buf = vec![0i16; 2 * 64 * 40];
        e.render(&mut buf, 64 * 40);
        buf
    };
    let sum = |b: &[i16]| b.iter().map(|&x| (x as i64).abs()).sum::<i64>();
    let full = render(1, None);
    assert_eq!(render(1, Some(127)), full);
    assert_eq!(render(1, Some(0)), render(0, None));
    let half = render(1, Some(64));
    let (lo, mid, hi) = (sum(&full), sum(&half), sum(&render(0, None)));
    assert!(lo < mid && mid < hi, "{lo} {mid} {hi}");
    // the mix itself: dry + (wet - dry) * amount / 127
    let mut wet = [0.5f32; BLOCK];
    mix_wet(&mut wet, &[1.0; BLOCK], 64);
    assert_eq!(wet[0], (1.0 - 0.5 * 64.0 / 127.0) as f32);
    // the controllers: WET + 3c + k, recorded and kept like the limiter's
    let mut e = edit_engine();
    assert_eq!(e.frame.devices.mixer.live.wet, [[WET_FULL; 3]; 4]);
    e.mixer_global(WET + 3 * 2 + 1, 40);
    assert_eq!(e.frame.devices.mixer.live.wet[2][1], 40);
    assert_eq!(e.frame.devices.mixer.fork_param(WET + 7), Some(40));
    assert!(is_fork_mixer(WET + 11) && WET + 12 == MORE);
}

/// This fork: the PCF and the compressor also on other channels ([`MORE`]), each such channel
/// with its own copy; none such, the original's routing as it was.
#[test]
fn the_pcf_and_the_compressor_run_on_more_channels_as_copies() {
    let render = |pcf_id: u8, comp_id: u8, more: [[u8; 2]; 4]| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        d.channels[1] = Box::new(Ramp(7));
        d.pcf.set_param(0, 1);
        d.comp.set_param(0, 1);
        d.comp.set_param(2, 20);
        let mut e = Engine::new(tables(), d);
        let m = &mut e.frame.devices.mixer.live;
        m.pcf_id = pcf_id;
        m.comp_id = comp_id;
        for (c, k) in more.iter().enumerate() {
            for (i, &v) in k.iter().enumerate() {
                e.mixer_global(MORE + 2 * c as u8 + i as u8, v);
            }
        }
        let mut buf = vec![0i16; 2 * 64 * 60];
        e.render(&mut buf, 64 * 60);
        (buf, e)
    };
    let none = [[0; 2]; 4];
    // a channel's copy plays as the song's insert would there
    let (song, _) = render(1, 2, none);
    let (copy, e) = render(0, 0, [[1, 1], [0, 0], [0, 0], [0, 0]]);
    assert_eq!(copy, song);
    let r = e.frame.state.channels[0].routing;
    assert!(r.pcf_copy && r.comp_copy && !r.pcf && !r.comp);
    // the song's channel keeps the song's insert, the other gets a copy: both filtered
    let (both, e) = render(1, 2, [[0, 0], [1, 1], [0, 0], [0, 0]]);
    assert_ne!(both, song);
    let r = e.frame.state.channels.map(|c| c.routing);
    assert!(r[0].pcf && !r[0].pcf_copy && r[1].pcf_copy && r[1].comp_copy);
    // MORE on the channel the id already chooses: no copy
    let (same, e) = render(1, 2, [[1, 1], [0, 0], [0, 0], [0, 0]]);
    assert_eq!(same, song);
    assert!(!e.frame.state.channels[0].routing.pcf_copy);
    assert!(is_fork_mixer(MORE + 7) && MORE + 8 == SLOT);
}

/// This fork: a channel runs its inserts in their slots' order ([`SLOT`]); the default slots
/// are the original's order.
#[test]
fn the_inserts_run_in_their_slots_order() {
    let log: Log = Rc::default();
    let mut d = Devices::placeholders();
    d.channels[1] = Box::new(Const(0.0));
    for (name, slot) in [("dist", 0), ("pcf", 1), ("comp", 2)] {
        let op = Box::new(Op {
            name,
            f: |x| x,
            log: log.clone(),
        });
        match slot {
            0 => d.dist = op,
            1 => d.pcf = op,
            _ => d.comp = op,
        }
    }
    let mut e = engine_with(d, quiet_state());
    let m = &mut e.frame.devices.mixer.live;
    assert_eq!(m.slots, DEFAULT_SLOTS);
    assert_eq!(m.insert_order(), [0, 1, 2]);
    m.channels[1].dist = 1;
    m.pcf_id = 2;
    m.comp_id = 3;
    e.frame.mixer_update();
    let run = |e: &mut Engine| {
        log.borrow_mut().clear();
        block_once(e);
        log.borrow().clone()
    };
    assert_eq!(run(&mut e), ["dist", "pcf", "comp"]);
    // the compressor on top, the PCF at the bottom
    for (s, fx) in [FX_COMP, FX_DIST, FX_DELAY, FX_PCF].into_iter().enumerate() {
        e.mixer_global(SLOT + s as u8, fx);
    }
    assert_eq!(e.frame.devices.mixer.live.insert_order(), [2, 0, 1]);
    assert_eq!(run(&mut e), ["comp", "dist", "pcf"]);
    assert_eq!(e.frame.devices.mixer.fork_param(SLOT), Some(FX_COMP));
    assert!(is_fork_mixer(SLOT + 3) && SLOT + 4 == DELAY_B);
}

/// This fork: two slots that hold the delay run a second delay, with its own line, its own
/// controllers and its own sends; an effect no slot holds is not heard.
#[test]
fn a_second_delay_and_effects_out_of_the_slots() {
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        e.frame.devices.mixer.live.channels[0].send = 100;
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 400];
        e.render(&mut buf, 64 * 400);
        (buf, e)
    };
    let (song, _) = render(&|_| {});
    // the second delay's sends at 0: as the song (its line adds 1e-6 guards: nearly)
    let two = |e: &mut Engine| e.mixer_global(SLOT + 1, FX_DELAY);
    let (quiet, e) = render(&two);
    assert!(e.frame.delay_b.is_some());
    let near = song
        .iter()
        .zip(&quiet)
        .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 1);
    assert!(near);
    // a send into it: heard
    let (sent, e) = render(&|e| {
        two(e);
        e.mixer_global(SEND_B, 120);
        e.mixer_global(DELAY_B + 1, 5);
    });
    assert_ne!(sent, song);
    assert_eq!(e.frame.devices.mixer.live.delay_b[1], 5);
    assert_eq!(e.frame.devices.mixer.fork_param(SEND_B), Some(120));
    // one delay slot again: the second delay goes
    let (_, e) = render(&|e| {
        two(e);
        e.mixer_global(SLOT + 1, FX_DIST);
    });
    assert!(e.frame.delay_b.is_none());
    // no slot holds the delay: no echo (as with no send)
    let (none, _) = render(&|e| {
        for s in 0..4 {
            e.mixer_global(SLOT + s, FX_DIST);
        }
    });
    let (dry, _) = render(&|e| e.frame.devices.mixer.live.channels[0].send = 0);
    let near = none
        .iter()
        .zip(&dry)
        .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 1);
    assert!(near);
    assert!(is_fork_mixer(SEND_B + 3) && SEND_B + 4 == WET_B);
}

/// This fork: two slots that hold the DIST, the PCF or the COMP run a second one, each channel
/// that hears it ([`MixerParams::wet_b`]) its own copy with the second one's controllers; its
/// meters sum the copies'.
#[test]
fn a_second_dist_pcf_and_comp() {
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        (buf, e)
    };
    let (song, _) = render(&|_| {});
    for (k, fx) in [FX_DIST, FX_PCF, FX_COMP].into_iter().enumerate() {
        // two slots hold it: no channel hears the second one, as the song
        let two = |e: &mut Engine| {
            if fx == FX_COMP {
                e.mixer_global(SLOT + 2, FX_COMP);
            } else {
                e.mixer_global(SLOT + 3, fx);
            }
        };
        let (quiet, e) = render(&two);
        assert_eq!(quiet, song, "fx {fx}");
        assert!(e.frame.copies_b[0][k][0].is_none());
        // channel 1 hears it: its own copy, heard, metered
        let on = |e: &mut Engine| {
            two(e);
            e.mixer_global(WET_B + k as u8, WET_FULL);
        };
        let (heard, e) = render(&on);
        assert!(e.frame.copies_b[0][k][0].is_some(), "fx {fx}");
        assert!(e.frame.copies_b[1][k][0].is_none());
        assert_ne!(heard, song, "fx {fx}");
        let meter = [0, 1, 3][k];
        assert!(
            e.frame.meters_b[meter] > 0,
            "fx {fx}: {:?}",
            e.frame.meters_b
        );
        // its own controllers
        let first = [DIST_B, PCF_B, COMP_B][k];
        let (other, e) = render(&|e| {
            on(e);
            e.mixer_global(first + 1, 127);
        });
        assert_ne!(other, heard, "fx {fx}");
        assert_eq!(e.frame.devices.mixer.live.ins_b(k)[1], 127);
        // one slot again: the copies go
        let (_, e) = render(&|e| {
            on(e);
            if fx == FX_COMP {
                e.mixer_global(SLOT + 2, FX_PCF);
            } else {
                e.mixer_global(SLOT + 3, FX_COMP);
            }
        });
        assert!(e.frame.copies_b[0][k][0].is_none(), "fx {fx}");
    }
    let m = MixerParams::default();
    assert_eq!(m.dist_b, crate::dist::DEFAULTS[..3]);
    assert_eq!(m.pcf_b, crate::pcf::DEFAULTS[..7]);
    assert_eq!(m.comp_b, crate::comp::DEFAULTS[..3]);
    assert!(is_fork_mixer(COMP_B + 2) && COMP_B + 3 == EQ);
    assert_eq!(COMP_B + 3, 124);
}

/// This fork: each channel's EQ (HI, MID, LO): all at 0 dB the original's sound; a band moved
/// changes only its channel, and back at 0 dB the sound is the original's again.
#[test]
fn a_channels_eq() {
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        e.frame.devices.mixer.live.channels[0].send = 100;
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        buf
    };
    let song = render(&|_| {});
    let n = crate::eq::NEUTRAL;
    assert_eq!(render(&|e| e.mixer_global(EQ, n)), song);
    // channel 1's LO up: changed; channel 2's (silent) LO up: as the song
    let lo = render(&|e| e.mixer_global(EQ, 127));
    assert_ne!(lo, song);
    assert_eq!(render(&|e| e.mixer_global(EQ + 3, 127)), song);
    // the HI cut is another sound
    assert_ne!(render(&|e| e.mixer_global(EQ + 2, 0)), lo);
    assert_eq!(MixerParams::default().eq, [[n; 3]; 4]);
    let mut m = MixerParams::default();
    m.set_fork(EQ + 7, 10);
    assert_eq!((m.fork(EQ + 7), m.eq[2][1]), (Some(10), 10));
    assert!(is_fork_mixer(EQ + 11));
    assert_eq!(DIST_KIND, EQ + 12);
}

/// (this fork) The DIST's kinds: the classic one is the original's; another is another sound
/// on the channels that hear the DIST only, with the classic's meter; the second DIST has its
/// own kind.
#[test]
fn a_dist_kind() {
    use crate::dist_kinds::{CLASSIC, KINDS, RECT};
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        e.frame.devices.mixer.live.channels[0].dist = 1;
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        (buf, e)
    };
    let (song, e) = render(&|_| {});
    let meter = e.frame.devices.dist.live()[crate::dist::METER];
    assert!(meter > 0);
    assert_eq!(render(&|e| e.mixer_global(DIST_KIND, CLASSIC)).0, song);
    let (rect, e) = render(&|e| e.mixer_global(DIST_KIND, RECT));
    assert_ne!(rect, song);
    assert_eq!(e.frame.devices.dist.live()[crate::dist::METER], meter);
    // every kind its own sound
    let mut all = vec![song.clone()];
    for k in 1..KINDS {
        let (b, _) = render(&|e| e.mixer_global(DIST_KIND, k));
        assert!(!all.contains(&b), "kind {k}");
        all.push(b);
    }
    // a channel without the DIST: as ever
    let off = |e: &mut Engine| e.frame.devices.mixer.live.channels[0].dist = 0;
    let (plain, _) = render(&off);
    let (plain_rect, _) = render(&|e| {
        off(e);
        e.mixer_global(DIST_KIND, RECT);
    });
    assert_eq!(plain_rect, plain);
    // the second DIST's kind
    let two = |e: &mut Engine| {
        e.mixer_global(SLOT + 3, FX_DIST);
        e.mixer_global(WET_B, WET_FULL);
    };
    let (b_classic, _) = render(&two);
    let (b_rect, _) = render(&|e| {
        two(e);
        e.mixer_global(DIST_KIND + 1, RECT);
    });
    assert_ne!(b_rect, b_classic);
    // a kind past the last is the last
    let mut m = MixerParams::default();
    assert_eq!(m.dist_kind, [CLASSIC; 2]);
    m.set_fork(DIST_KIND + 1, 200);
    assert_eq!(m.fork(DIST_KIND + 1), Some(KINDS - 1));
    assert!(is_fork_mixer(DIST_KIND + 1));
    assert_eq!(MOD, DIST_KIND + 2);
}

#[test]
fn the_master_comp_gives_way_to_the_limit_key() {
    let mut e = loud_engine();
    e.frame.devices.mixer.live.comp_id = 1;
    e.frame.mixer_update();
    assert!(e.frame.state.master_comp);
    e.frame.master_limit = true;
    e.frame.mixer_update();
    assert!(!e.frame.state.master_comp);
}

#[test]
fn songs_built_on_one_process_state_share_it() {
    let a = Devices::implemented(None);
    let b = Devices::implemented_with(None, a.process.clone());
    assert!(a.process.same(&b.process));
    assert!(!a.process.same(&Devices::implemented(None).process));
    b.process.refill_delay_line();
    assert!(a.process.delay_line().gate);
}

#[test]
fn the_saved_blocks_are_the_live_ones_in_pattern_mode_and_the_copies_in_song_mode() {
    let mut d = Devices::implemented(None);
    // pattern mode: the queries reply with the live block
    d.channels[0].set_param(3, 40);
    d.delay.set_param(3, 11);
    d.mixer.set_param(6, 33);
    assert_eq!(d.channels[0].saved_block(false), d.channels[0].live());
    assert_eq!(d.delay.saved_block(false), d.delay.live());
    assert_eq!(d.mixer.saved(false), d.mixer.live);
    // song mode (slot 9 copies the live blocks), then the tracks and the panel move the live
    // values: the queries reply with the copies
    d.channels[0].set_song_mode(true);
    d.delay.set_song_mode(true);
    d.mixer.set_song_mode(true);
    d.channels[0].set_param(3, 50);
    d.delay.set_param(3, 99);
    d.mixer.set_param(6, 90);
    d.mixer.live.master = 7;
    d.mixer.live.channels[1].enabled = 0;
    assert_eq!(d.channels[0].live()[3], 50);
    assert_eq!(d.channels[0].saved_block(true)[3], 40);
    assert_eq!(d.delay.saved_block(true)[3], 11);
    let m = d.mixer.saved(true);
    assert_eq!(m.channels[0].level, 33);
    // the master and the enables (no controller) are the live ones (0x10015635)
    assert_eq!((m.master, m.channels[1].enabled), (7, 0));
    // a placeholder has no block
    assert!(d.channels[2].saved_block(true).is_empty());
}

/// This fork: with one set of settings for both modes (`set_fork_shared_modes`), what the panel
/// set in pattern mode plays on in song mode, and what song mode had stays on in pattern mode;
/// without it (the original), song mode plays the song's start values and pattern mode gets its
/// own back.
#[test]
fn shared_modes_keep_the_settings_across_the_switch() {
    use song::Device as D;
    for shared in [false, true] {
        let mut e = Engine::new(tables(), Devices::implemented(None));
        e.frame.shared_modes = shared;
        let level = |e: &Engine| e.frame.devices.mixer.live.channels[2].level;
        let dist = |e: &Engine| e.frame.devices.mixer.live.channels[3].dist;
        let tune = |e: &mut Engine| e.frame.devices.device_mut(D::Tb303_1).live()[3];
        let (level0, tune0) = (level(&e), tune(&mut e));
        // pattern mode: the 808's level down, the 909's distortion on, a 303 knob turned
        e.mixer_channel(2, 1, 40);
        e.mixer_channel(3, 4, 1);
        e.device_param(D::Tb303_1, 3, 99);
        e.set_song_mode(1);
        if shared {
            assert_eq!(
                (level(&e), dist(&e), tune(&mut e)),
                (40, 1, 99),
                "song mode keeps them"
            );
        } else {
            assert_eq!(
                (level(&e), tune(&mut e)),
                (level0, tune0),
                "the song's start values"
            );
        }
        // song mode: the 808's level changed again
        e.mixer_channel(2, 1, 70);
        e.set_song_mode(0);
        if shared {
            assert_eq!(
                (level(&e), dist(&e), tune(&mut e)),
                (70, 1, 99),
                "pattern mode keeps them"
            );
            // and song mode again starts from them
            e.set_song_mode(1);
            e.locate(0);
            assert_eq!((level(&e), dist(&e), tune(&mut e)), (70, 1, 99));
        } else {
            assert_eq!(
                (level(&e), dist(&e), tune(&mut e)),
                (40, 1, 99),
                "pattern mode's own"
            );
        }
    }
}

/// (this fork) The chorus, the flanger and the phaser: not heard until a slot holds one and a
/// channel sends into it; each its own sound, metered; a second one with its own controllers.
#[test]
fn a_chorus_flanger_and_phaser() {
    use crate::modfx::{MODE, TYPE};
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        (buf, e)
    };
    let (song, _) = render(&|_| {});
    let mut heard = Vec::new();
    for k in 0..3u8 {
        let fx = FX_CHO + k;
        // in the PCF's slot (the PCF is then not heard: it had no channel), no send: the song
        let slot = |e: &mut Engine| e.mixer_global(SLOT + 2, fx);
        let (quiet, e) = render(&slot);
        assert_eq!(quiet, song, "fx {fx}");
        assert!(e.frame.mods[k as usize][0].is_none());
        // a send from channel 1, but no slot holds it: the song
        let send = |e: &mut Engine| e.mixer_global(MOD_SEND + 2 * k, 100);
        assert_eq!(render(&send).0, song, "fx {fx}");
        let on = |e: &mut Engine| {
            slot(e);
            send(e);
        };
        let (b, e) = render(&on);
        assert_ne!(b, song, "fx {fx}");
        assert!(e.frame.meters_mod[k as usize][0] > 0, "fx {fx}");
        assert!(!heard.contains(&b));
        heard.push(b.clone());
        // switched off: the song
        let first = MOD + 14 * k;
        assert_eq!(
            render(&|e| {
                on(e);
                e.mixer_global(first, 0);
            })
            .0,
            song
        );
        // mono, another TYPE: other sounds
        let (mono, _) = render(&|e| {
            on(e);
            e.mixer_global(first + MODE as u8, 1);
        });
        assert_ne!(mono, b, "fx {fx}");
        let (other, _) = render(&|e| {
            on(e);
            e.mixer_global(first + TYPE as u8, if k == 2 { 12 } else { 3 });
        });
        assert_ne!(other, b, "fx {fx}");
        // a second one in the DIST's slot, its own send
        let (two, e) = render(&|e| {
            on(e);
            e.mixer_global(SLOT + 1, fx);
            e.mixer_global(MOD_SEND + 2 * k + 1, 100);
        });
        assert!(e.frame.mods[k as usize][1].is_some());
        assert_ne!(two, b, "fx {fx}");
    }
    let mut m = MixerParams::default();
    assert_eq!(m.mods[2][1], crate::modfx::DEFAULTS[2]);
    m.set_fork(MOD_SEND + 23, 99);
    assert_eq!((m.fork(MOD_SEND + 23), m.mod_send[3][2][1]), (Some(99), 99));
    m.set_fork(MOD + 41, 7);
    assert_eq!(m.mods[2][1][6], 7);
    assert!(is_fork_mixer(FORK_END - 1) && !is_fork_mixer(FORK_END));
    assert_eq!(RVRB, MOD_SEND + 24);
}

/// (this fork) The reverb: not heard until a slot holds it and a channel sends into it; each
/// TYPE its own sound, metered; a second one with its own controllers.
#[test]
fn a_reverb() {
    use crate::reverb::{MODE, TYPE, TYPES};
    let render = |setup: &dyn Fn(&mut Engine)| {
        let mut d = Devices::implemented(None);
        d.channels[0] = Box::new(Ramp(0));
        let mut e = Engine::new(tables(), d);
        setup(&mut e);
        let mut buf = vec![0i16; 2 * 64 * 200];
        e.render(&mut buf, 64 * 200);
        (buf, e)
    };
    let (song, _) = render(&|_| {});
    // in the delay's slot (the delay is then not heard: no channel sent into it), no send
    let slot = |e: &mut Engine| e.mixer_global(SLOT, FX_RVRB);
    let (quiet, e) = render(&slot);
    assert_eq!(quiet, song);
    assert!(e.frame.rvrb[0].is_none());
    let send = |e: &mut Engine| e.mixer_global(RVRB_SEND, 100);
    assert_eq!(render(&send).0, song);
    let on = |e: &mut Engine| {
        slot(e);
        send(e);
    };
    let (b, e) = render(&on);
    assert_ne!(b, song);
    assert!(e.frame.meters_rvrb[0] > 0);
    // switched off: the song
    assert_eq!(
        render(&|e| {
            on(e);
            e.mixer_global(RVRB, 0);
        })
        .0,
        song
    );
    // every TYPE, mono: other sounds
    let mut all = vec![b.clone()];
    for t in 1..TYPES {
        let (x, _) = render(&|e| {
            on(e);
            e.mixer_global(RVRB + TYPE as u8, t);
        });
        assert!(!all.contains(&x), "type {t}");
        all.push(x);
    }
    let (mono, _) = render(&|e| {
        on(e);
        e.mixer_global(RVRB + MODE as u8, 1);
    });
    assert_ne!(mono, b);
    // a second one in the DIST's slot, its own send
    let (two, e) = render(&|e| {
        on(e);
        e.mixer_global(SLOT + 1, FX_RVRB);
        e.mixer_global(RVRB_SEND + 1, 100);
    });
    assert!(e.frame.rvrb[1].is_some());
    assert_ne!(two, b);
    let mut m = MixerParams::default();
    assert_eq!(m.rvrb[1], crate::reverb::DEFAULTS);
    m.set_fork(RVRB_SEND + 7, 99);
    assert_eq!((m.fork(RVRB_SEND + 7), m.rvrb_send[3][1]), (Some(99), 99));
    m.set_fork(RVRB + 9, 7);
    assert_eq!(m.rvrb[1][4], 7);
    assert!(is_fork_mixer(FORK_END - 1) && !is_fork_mixer(FORK_END));
    assert_eq!(FORK_END, RVRB_SEND + 8);
}
