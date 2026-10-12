//! A replay played natively in melee-sim, its frames drawn through melee-render.

use std::collections::HashMap;

use melee_data::disc::Disc;
use melee_render::frame::Frames;
use melee_render::gpu::{Frame, Gpu, Observer};
use melee_replay::Device;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use ssbm_gx::{Draw, Memory, State};
use ssbm_slippi::replay::Replay;

use crate::snap::{Event, Recorder};

struct Watch<'a>(&'a Recorder);

impl Observer for Watch<'_> {
    fn draw(&self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        self.0.draw(state, draw, mem);
    }
    fn copy(&self, state: &State, value: u32) {
        self.0.copy(state, value);
    }
}

/// A frame drawn: the replay frame it ends, its GX events and the image.
pub struct Drawn {
    pub replay_frame: i32,
    pub events: Vec<Event>,
    pub image: Option<Frame>,
    pub notes: Vec<(String, u32)>,
}

/// The replay's match, drawing each frame from `from` on to `to` (match frames, counted from
/// its first rendered frame, 0), and calling `each` with each.
pub fn play(
    disc_path: &str,
    replay_path: &str,
    from: u32,
    to: u32,
    mut each: impl FnMut(u32, Drawn) -> bool,
) {
    let disc = Disc::open(disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let settings = Settings::from_replay(&melee_replay::start(&replay))
        .unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let mut cache: HashMap<String, (i32, Vec<u8>)> = HashMap::new();
    let mut load = |name: &str| {
        let (entrynum, bytes) = cache.entry(name.to_owned()).or_insert_with(|| {
            (
                disc.entrynum(name)
                    .unwrap_or_else(|e| panic!("{name}: {e}")),
                disc.read_file(name)
                    .unwrap_or_else(|e| panic!("{name}: {e}")),
            )
        });
        DiscFile {
            entrynum: *entrynum,
            bytes: bytes.clone(),
        }
    };
    let mut m = Match::start(&settings, &mut load);
    let mut device = Device(&replay);
    let mut frames = Frames::new();
    let renderer = ssbm_render::Renderer::new().expect("a GPU");
    let mut gpu = Gpu::new(renderer);
    let mut n = 0u32;
    loop {
        let (mut f, render) = m.step_replay_tick(&mut device);
        let ended = f.ended;
        if render {
            f.draws = m.render();
            let fifo = frames.frame(&m.game, &m.data, &f.draws);
            if n >= from {
                let rec = Recorder::default();
                let images = gpu.run(&frames.space, &m.data, &fifo, Some(&Watch(&rec)));
                let notes = std::mem::take(&mut frames.hsd.notes).into_iter().collect();
                let drawn = Drawn {
                    replay_frame: m.game.playback.as_ref().map_or(-124, |p| p.frame_index),
                    events: rec.events.into_inner(),
                    image: images.into_iter().last(),
                    notes,
                };
                if !each(n, drawn) {
                    return;
                }
            } else {
                gpu.run(&frames.space, &m.data, &fifo, None);
                frames.hsd.notes.clear();
            }
            n += 1;
            if n > to {
                return;
            }
        }
        m.recycle(f);
        if ended {
            return;
        }
    }
}

/// How far two frames' draws are apart, for lining frames up: the summed screen distance of
/// the vertices of draws whose primitive and count match, position by position.
pub fn distance(a: &[Event], b: &[Event]) -> (f64, usize) {
    let (mut d, mut matched) = (0f64, 0usize);
    fn draws(e: &[Event]) -> Vec<&crate::snap::DrawSnap> {
        e.iter()
            .filter_map(|x| match x {
                Event::Draw(d) => Some(&**d),
                _ => None,
            })
            .collect()
    }
    for (x, y) in draws(a).iter().zip(draws(b)) {
        if (x.primitive, x.count) != (y.primitive, y.count) {
            continue;
        }
        matched += 1;
        for (p, q) in x.verts.iter().zip(&y.verts) {
            for k in 0..2 {
                let e = f64::from(p.screen[k] - q.screen[k]).abs();
                if e.is_finite() {
                    d += e.min(100.0);
                }
            }
        }
    }
    (d, matched)
}
