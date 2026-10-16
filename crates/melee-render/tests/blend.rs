//! Frames between two ticks (`melee_render::blend`) at their ends: at t = 1 the frame is the
//! tick's own frame command for command, and at t = 0 every draw sits where the tick before
//! drew it, but for what snaps to the later tick (the HUD, and point particles); midway the
//! moving draws lie between and the HUD stays put.
//!
//! Extrapolate's pair (`melee_render::predict`), the tick and the next predicted, the same way:
//! at t = 0 the frame is the tick's own, command for command, and at t = 1 every draw sits where
//! the next tick drew it (a replay's predictions are exact), but for what snaps to the tick.
//!
//! Plays the Fox ditto benchmark replay natively and decodes each frame's stream with
//! melee-gx, transforming every draw's vertices to the screen, as the frame checks do. Skips
//! without `SSBM_DISC`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_gx::xform::{self, Viewport};
use melee_gx::{Draw, Memory, Produced, State, vertex};
use melee_render::blend::{Blend, Capture};
use melee_render::frame::Frames;
use melee_render::mem::Space;
use melee_render::predict::Predictor;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_slippi::{Device, Replay};

const REPLAY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/bench/fox-fox-fd.slp"
);

/// The ticks whose frames are checked: both Foxes moving about, the HUD showing.
const FROM: u32 = 600;
const TO: u32 = 660;

/// GX's quads (the opcode's top five bits), which particles draw.
const QUADS: u8 = 0x80;

/// A draw as decoded: its primitive and its vertices on the screen.
#[derive(Clone, Debug, PartialEq)]
struct Drawn {
    primitive: u8,
    screen: Vec<[f32; 3]>,
}

/// What the GPU reads, recording each draw while `record` is set.
struct Host<'a> {
    space: &'a Space,
    data: &'a Archives,
    record: Cell<bool>,
    draws: RefCell<Vec<Drawn>>,
}

impl Memory for Host<'_> {
    fn read(&self, phys: u32, out: &mut [u8]) {
        self.space.read(self.data, phys, out);
    }

    fn view(&self, phys: u32, len: usize) -> Option<&[u8]> {
        self.space.view(self.data, phys, len)
    }

    fn draws(&self) -> bool {
        self.record.get()
    }

    fn draw(&self, state: &State, draw: &Draw<'_>) {
        let xf_index = [state.xf[xform::MATINDEX_A], state.xf[xform::MATINDEX_A + 1]];
        let mut inputs = Vec::new();
        let mut normals = Default::default();
        vertex::decode(
            &state.cp,
            xf_index,
            usize::from(draw.vat),
            draw.data,
            usize::from(draw.count),
            &mut normals,
            &mut inputs,
        );
        let vp = Viewport::of(&state.xf);
        let screen = inputs
            .iter()
            .map(|v| vp.screen(xform::transform(&state.xf, &state.bp, v).clip))
            .collect();
        self.draws.borrow_mut().push(Drawn {
            primitive: draw.primitive,
            screen,
        });
    }
}

/// One way of drawing the match: melee-render's frames and the GPU's state following them.
struct Side {
    frames: Frames,
    state: State,
    produced: Vec<Produced>,
}

impl Side {
    fn new(dol: &[u8]) -> Side {
        let mut frames = Frames::new();
        frames.space.load_dol(dol);
        Side {
            frames,
            state: State::new(),
            produced: Vec::new(),
        }
    }

    /// Runs a frame's stream through the GPU's state, returning its draws when `record`.
    fn run(&mut self, data: &Archives, fifo: &[u8], record: bool) -> Vec<Drawn> {
        let host = Host {
            space: &self.frames.space,
            data,
            record: Cell::new(record),
            draws: RefCell::new(Vec::new()),
        };
        self.state.feed(&host, fifo, &mut self.produced, false);
        self.produced.clear();
        host.draws.into_inner()
    }
}

/// Plays the replay to tick `to`, calling `each` with the match after each rendered tick, its
/// draws and the tick's number (rendered ticks from 0).
fn play(disc: &Disc, to: u32, mut each: impl FnMut(u32, &Match, &[melee_sim::hsd::display::Draw])) {
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let bytes = std::fs::read(REPLAY).expect("the benchmark replay");
    let replay = Replay::parse(&bytes).expect("a replay");
    let settings = Settings::from_replay(&melee_slippi::start(&replay)).expect("its settings");
    let mut cache: HashMap<String, (i32, Vec<u8>)> = HashMap::new();
    let mut load = |name: &str| {
        let (entrynum, bytes) = cache.entry(name.to_owned()).or_insert_with(|| {
            (
                disc.entrynum(name).expect("a file"),
                disc.read_file(name).expect("a file"),
            )
        });
        DiscFile {
            entrynum: *entrynum,
            bytes: bytes.clone(),
        }
    };
    let mut m = Match::start(&settings, &mut load);
    let mut n = 0;
    while n <= to {
        let f = m.step_replay(&mut Device(&replay));
        if !f.draws.is_empty() {
            each(n, &m, &f.draws);
            n += 1;
        }
        let ended = f.ended;
        m.recycle(f);
        assert!(!ended, "the match ended before tick {to}");
    }
}

fn disc() -> Option<Disc> {
    let Some(path) = std::env::var_os("SSBM_DISC") else {
        eprintln!("skipped: SSBM_DISC names no disc");
        return None;
    };
    Some(Disc::open(&path).expect("the disc"))
}

/// At t = 1 a blended frame is the tick's own, command for command, every tick of the stretch.
#[test]
fn a_frame_at_t_1_is_the_tick_s_own() {
    let Some(disc) = disc() else { return };
    let dol = disc.dol().expect("main.dol");
    let (mut off, mut blended) = (Side::new(&dol), Side::new(&dol));
    let mut capture = Capture::default();
    let mut checked = 0;
    play(&disc, TO, |n, m, draws| {
        let own = off.frames.frame(&m.game, &m.data, draws);
        let at_1 = if n > FROM {
            let blend = Blend {
                from: &capture,
                t: 1.0,
            };
            checked += 1;
            blended
                .frames
                .frame_blended(&m.game, &m.data, draws, Some(blend))
        } else {
            blended.frames.frame(&m.game, &m.data, draws)
        };
        assert!(
            own == at_1,
            "tick {n}: the frame at t = 1 differs from the tick's"
        );
        capture.fill(&m.game, draws);
    });
    assert_eq!(checked, TO - FROM);
}

/// At t = 0 each draw is where the tick before drew it, but for what snaps to the tick (the
/// HUD's cameras, point particles) and what the tick before didn't draw (new particles, drawn
/// where the tick has them through the tick before's camera); at t = 0.5 what snapped is still
/// the tick's and the rest lies between.
#[test]
fn a_frame_at_t_0_draws_the_tick_before() {
    let Some(disc) = disc() else { return };
    let dol = disc.dol().expect("main.dol");
    // First, the ticks whose draws pair up with the tick before's one for one (the same
    // primitives and vertex counts), so draw i of one is draw i of the other.
    let mut off = Side::new(&dol);
    let mut drawn: Vec<(u32, Vec<Drawn>)> = Vec::new();
    play(&disc, TO, |n, m, draws| {
        let fifo = off.frames.frame(&m.game, &m.data, draws);
        let d = off.run(&m.data, &fifo, n >= FROM);
        if n >= FROM {
            drawn.push((n, d));
        }
    });
    let shape = |d: &[Drawn]| -> Vec<(u8, usize)> {
        d.iter().map(|x| (x.primitive, x.screen.len())).collect()
    };
    let pairs: Vec<u32> = drawn
        .windows(2)
        .filter(|w| shape(&w[0].1) == shape(&w[1].1))
        .filter(|w| {
            // Enough moved for the check to mean something.
            w[0].1.iter().zip(&w[1].1).filter(|(a, b)| a != b).count() >= 20
        })
        .map(|w| w[1].0)
        .take(3)
        .collect();
    assert!(!pairs.is_empty(), "no two ticks in {FROM}..{TO} pair up");
    let at = |n: u32| &drawn.iter().find(|(x, _)| *x == n).expect("drawn").1;
    let mut snapped_moving = 0;
    for &n in &pairs {
        // Again, drawing tick n at t = 0 and 0.5 with the same history as the ticks before,
        // and at t = 0 with only the cameras kept: everything where the tick has it, seen
        // through the tick before's cameras, as what the tick before didn't draw is drawn.
        let (mut zero, mut half, mut cams) = (Side::new(&dol), Side::new(&dol), Side::new(&dol));
        let mut capture = Capture::default();
        let (mut d0, mut d5, mut dc) = (Vec::new(), Vec::new(), Vec::new());
        play(&disc, n, |k, m, draws| {
            if k < n {
                for side in [&mut zero, &mut half, &mut cams] {
                    let fifo = side.frames.frame(&m.game, &m.data, draws);
                    side.run(&m.data, &fifo, false);
                }
                capture.fill(&m.game, draws);
                return;
            }
            fn blend(from: &Capture, t: f32) -> Option<Blend<'_>> {
                Some(Blend { from, t })
            }
            let fifo = zero
                .frames
                .frame_blended(&m.game, &m.data, draws, blend(&capture, 0.0));
            d0 = zero.run(&m.data, &fifo, true);
            let fifo = half
                .frames
                .frame_blended(&m.game, &m.data, draws, blend(&capture, 0.5));
            d5 = half.run(&m.data, &fifo, true);
            capture.keep_only_cameras();
            let fifo = cams
                .frames
                .frame_blended(&m.game, &m.data, draws, blend(&capture, 0.0));
            dc = cams.run(&m.data, &fifo, true);
        });
        let (a, b) = (at(n - 1), at(n));
        assert_eq!(shape(&d0), shape(b), "tick {n}: the draws at t = 0");
        assert_eq!(shape(&d5), shape(b), "tick {n}: the draws at t = 0.5");
        assert_eq!(
            shape(&dc),
            shape(b),
            "tick {n}: the draws with only the cameras"
        );
        // Particles are drawn in the order their links were last sorted, so one can draw at
        // another place in the list than the tick before drew it.
        let mut elsewhere: Vec<usize> = (0..a.len()).filter(|&i| d0[i] != a[i]).collect();
        let (mut from_a, mut moved, mut between, mut snapped) = (0, 0, 0, 0);
        let (mut theirs, mut reordered) = (0, 0);
        for i in 0..b.len() {
            if d0[i] == a[i] {
                from_a += 1;
                if a[i] != b[i] {
                    moved += 1;
                    if d5[i] != a[i] && d5[i] != b[i] {
                        between += 1;
                    }
                }
            } else if d0[i] == b[i] {
                // Snapped to the tick: the HUD, a point particle. So it stays at t = 0.5.
                assert_eq!(
                    d5[i], b[i],
                    "tick {n}: draw {i} snapped at t = 0, not at 0.5"
                );
                snapped += 1;
            } else if d0[i] == dc[i] {
                // Where the tick has it, through the tick before's camera: a new particle, a
                // point particle.
                theirs += 1;
            } else {
                assert_eq!(
                    b[i].primitive, QUADS,
                    "tick {n}: draw {i} at t = 0 is neither the tick before's nor the tick's: {:?}, the tick before's {:?}, the tick's {:?}",
                    d0[i], a[i], b[i]
                );
                let k = elsewhere.iter().position(|&k| a[k] == d0[i]);
                let k = k.unwrap_or_else(|| {
                    panic!("tick {n}: particle {i} at t = 0 is nowhere the tick before drew one")
                });
                elsewhere.swap_remove(k);
                reordered += 1;
            }
        }
        // Of what snapped, what moved between the ticks (else it counts as the tick before's).
        let snapped_here = (0..b.len())
            .filter(|&i| d0[i] == b[i] && a[i] != b[i])
            .count();
        snapped_moving += snapped_here;
        eprintln!(
            "tick {n}: {from_a} draws the tick before's ({moved} moved, {between} midway at t = 0.5), {snapped} the tick's ({snapped_here} moved), {theirs} where the tick has them through the tick before's camera, {reordered} particles from elsewhere in the list"
        );
        assert!(
            between * 10 >= moved * 9,
            "tick {n}: few moved draws lie between"
        );
    }
    assert!(snapped_moving > 0, "no draw that snaps to the tick moved");
}

/// The replay, for the predictions' ticks.
fn replay() -> Replay {
    Replay::parse(&std::fs::read(REPLAY).expect("the benchmark replay")).expect("a replay")
}

/// Extrapolate at t = 0: the frame between a tick and the next predicted is the tick's own,
/// command for command, every tick of the stretch.
#[test]
fn a_predicted_frame_at_t_0_is_the_tick_s_own() {
    let Some(disc) = disc() else { return };
    let dol = disc.dol().expect("main.dol");
    let replay = replay();
    let (mut off, mut predicted) = (Side::new(&dol), Side::new(&dol));
    let mut predictor = Predictor::default();
    let mut checked = 0;
    play(&disc, TO, |n, m, draws| {
        let own = off.frames.frame(&m.game, &m.data, draws);
        let at_0 = if n >= FROM {
            predictor
                .predict(m, |c| c.step_replay(&mut Device(&replay)))
                .expect("the prediction");
            assert!(predictor.is_filled(), "tick {n}: no prediction");
            checked += 1;
            predicted
                .frames
                .frame_blended(&m.game, &m.data, draws, Some(predictor.blend(0.0)))
        } else {
            predicted.frames.frame(&m.game, &m.data, draws)
        };
        assert!(
            own == at_0,
            "tick {n}: the predicted frame at t = 0 differs from the tick's"
        );
    });
    assert_eq!(checked, TO - FROM + 1);
}

/// Extrapolate at t = 1: each draw is where the next tick draws it, but for what snaps to the
/// tick (the HUD's cameras, point particles) and what the prediction didn't draw (drawn where
/// the tick has it, through the predicted cameras); at t = 0.5 what snapped is still the tick's
/// and the rest lies between.
#[test]
fn a_predicted_frame_at_t_1_draws_the_next_tick() {
    let Some(disc) = disc() else { return };
    let dol = disc.dol().expect("main.dol");
    let replay = replay();
    let mut off = Side::new(&dol);
    let mut drawn: Vec<(u32, Vec<Drawn>)> = Vec::new();
    play(&disc, TO, |n, m, draws| {
        let fifo = off.frames.frame(&m.game, &m.data, draws);
        let d = off.run(&m.data, &fifo, n >= FROM);
        if n >= FROM {
            drawn.push((n, d));
        }
    });
    let shape = |d: &[Drawn]| -> Vec<(u8, usize)> {
        d.iter().map(|x| (x.primitive, x.screen.len())).collect()
    };
    // Ticks whose draws pair up with the next tick's one for one.
    let pairs: Vec<u32> = drawn
        .windows(2)
        .filter(|w| shape(&w[0].1) == shape(&w[1].1))
        .filter(|w| w[0].1.iter().zip(&w[1].1).filter(|(a, b)| a != b).count() >= 20)
        .map(|w| w[0].0)
        .take(3)
        .collect();
    assert!(!pairs.is_empty(), "no two ticks in {FROM}..{TO} pair up");
    let at = |n: u32| &drawn.iter().find(|(x, _)| *x == n).expect("drawn").1;
    let mut snapped_moving = 0;
    for &n in &pairs {
        let (mut one, mut half, mut cams) = (Side::new(&dol), Side::new(&dol), Side::new(&dol));
        let mut predictor = Predictor::default();
        let (mut d1, mut d5, mut dc) = (Vec::new(), Vec::new(), Vec::new());
        play(&disc, n, |k, m, draws| {
            if k < n {
                for side in [&mut one, &mut half, &mut cams] {
                    let fifo = side.frames.frame(&m.game, &m.data, draws);
                    side.run(&m.data, &fifo, false);
                }
                return;
            }
            predictor
                .predict(m, |c| c.step_replay(&mut Device(&replay)))
                .expect("the prediction");
            let fifo =
                one.frames
                    .frame_blended(&m.game, &m.data, draws, Some(predictor.blend(1.0)));
            d1 = one.run(&m.data, &fifo, true);
            let fifo =
                half.frames
                    .frame_blended(&m.game, &m.data, draws, Some(predictor.blend(0.5)));
            d5 = half.run(&m.data, &fifo, true);
            // The prediction's cameras alone: the tick's draws through the predicted cameras.
            let mut copy = m.clone();
            let f = copy.step_replay(&mut Device(&replay));
            let mut capture = Capture::default();
            capture.fill(&copy.game, &f.draws);
            capture.keep_only_cameras();
            let blend = Blend {
                from: &capture,
                t: 0.0,
            };
            let fifo = cams
                .frames
                .frame_blended(&m.game, &m.data, draws, Some(blend));
            dc = cams.run(&m.data, &fifo, true);
        });
        let (a, b) = (at(n), at(n + 1));
        assert_eq!(shape(&d1), shape(a), "tick {n}: the draws at t = 1");
        assert_eq!(shape(&d5), shape(a), "tick {n}: the draws at t = 0.5");
        assert_eq!(
            shape(&dc),
            shape(a),
            "tick {n}: the draws with only the cameras"
        );
        let mut elsewhere: Vec<usize> = (0..b.len()).filter(|&i| d1[i] != b[i]).collect();
        let (mut from_b, mut moved, mut between, mut snapped) = (0, 0, 0, 0);
        let (mut theirs, mut reordered) = (0, 0);
        for i in 0..a.len() {
            if d1[i] == b[i] {
                from_b += 1;
                if a[i] != b[i] {
                    moved += 1;
                    if d5[i] != a[i] && d5[i] != b[i] {
                        between += 1;
                    }
                }
            } else if d1[i] == a[i] {
                // Snapped to the tick: the HUD, a point particle. So it stays at t = 0.5.
                assert_eq!(
                    d5[i], a[i],
                    "tick {n}: draw {i} snapped at t = 1, not at 0.5"
                );
                snapped += 1;
            } else if d1[i] == dc[i] {
                // Where the tick has it, through the predicted camera.
                theirs += 1;
            } else {
                assert_eq!(
                    a[i].primitive, QUADS,
                    "tick {n}: draw {i} at t = 1 is neither the tick's nor the next's: {:?}, the tick's {:?}, the next's {:?}",
                    d1[i], a[i], b[i]
                );
                let k = elsewhere.iter().position(|&k| b[k] == d1[i]);
                let k = k.unwrap_or_else(|| {
                    panic!("tick {n}: particle {i} at t = 1 is nowhere the next tick drew one")
                });
                elsewhere.swap_remove(k);
                reordered += 1;
            }
        }
        let snapped_here = (0..a.len())
            .filter(|&i| d1[i] == a[i] && a[i] != b[i])
            .count();
        snapped_moving += snapped_here;
        eprintln!(
            "tick {n}: {from_b} draws the next tick's ({moved} moved, {between} midway at t = 0.5), {snapped} the tick's ({snapped_here} moved), {theirs} where the tick has them through the predicted camera, {reordered} particles from elsewhere in the list"
        );
        assert!(
            between * 10 >= moved * 9,
            "tick {n}: few moved draws lie between"
        );
        assert!(from_b * 10 >= a.len() * 9, "tick {n}: few draws predicted");
    }
    assert!(snapped_moving > 0, "no draw that snaps to the tick moved");
}
