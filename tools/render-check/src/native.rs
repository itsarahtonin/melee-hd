//! A replay played natively in melee-sim, its frames drawn through melee-render.

use std::collections::HashMap;

use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_gx::{Draw, Memory, State};
use melee_render::frame::Frames;
use melee_render::gpu::{Frame, Gpu, Observer};
use melee_sim::game::state::{Cosmetic, Game};
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_slippi::Device;
use melee_slippi::Replay;

use crate::snap::{Event, Recorder};

struct Watch<'a>(&'a Recorder);

impl Observer for Watch<'_> {
    fn draw(&self, state: &State, draw: &Draw<'_>, mem: &dyn Memory) {
        self.0.draw(state, draw, mem);
    }
    fn copy(&self, state: &State, value: u32) {
        self.0.copy(state, value);
    }
    fn command(&self, bytes: &[u8]) {
        self.0.command(bytes);
    }
}

/// A frame drawn: the replay frame it ends, its GX events and the image.
pub struct Drawn {
    pub replay_frame: i32,
    pub events: Vec<Event>,
    pub image: Option<Frame>,
    pub notes: Vec<(String, u32)>,
}

/// A rendered frame of the match, as [`drive`] hands it on.
pub struct Tick<'a> {
    /// Match frames counted from its first rendered frame, 0.
    pub n: u32,
    pub replay_frame: i32,
    pub game: &'a Game,
    pub data: &'a Archives,
    pub draws: &'a [melee_sim::hsd::display::Draw],
    /// The disc's main.dol, which melee-render's address space loads.
    pub dol: &'a [u8],
}

/// Plays the replay's match to its rendered frame `to`, calling `each` with each rendered
/// frame until it returns false; with `cosmetic`'s toggles, as the player sets them for the
/// frames it shows.
fn drive(
    disc_path: &str,
    replay_path: &str,
    to: u32,
    cosmetic: Cosmetic,
    mut each: impl FnMut(&Tick<'_>) -> bool,
) {
    // The float mode the replay was recorded in: the console's for the benchmark matches
    // (the default), Slippi Dolphin's for a Slippi replay (`CHECK_FP=slippi`).
    let fp = match std::env::var("CHECK_FP").as_deref() {
        Ok("slippi") => melee_sim::gekko_fp::FpMode::Slippi,
        _ => melee_sim::gekko_fp::FpMode::Hardware,
    };
    melee_sim::gekko_fp::set_fp_mode(fp);
    let disc = Disc::open(disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let bytes = std::fs::read(replay_path).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let replay = Replay::parse(&bytes).unwrap_or_else(|e| panic!("{replay_path}: {e}"));
    let settings = Settings::from_replay(&melee_slippi::start(&replay))
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
    m.game.cosmetic = cosmetic;
    let mut device = Device(&replay);
    let dol = disc.dol().unwrap_or_else(|e| panic!("main.dol: {e}"));
    let mut n = 0u32;
    loop {
        let (mut f, render) = m.step_replay_tick(&mut device);
        let ended = f.ended;
        if render {
            f.draws = m.render();
            let tick = Tick {
                n,
                replay_frame: m.game.playback.as_ref().map_or(-124, |p| p.frame_index),
                game: &m.game,
                data: &m.data,
                draws: &f.draws,
                dol: &dol,
            };
            if !each(&tick) {
                return;
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

/// A frame's stream and its drawing: melee-render's frames and a GPU for them.
struct Drawer {
    frames: Frames,
    gpu: Gpu,
    loaded: bool,
}

impl Drawer {
    fn new(renderer: melee_gpu::Renderer, widescreen: Option<f32>) -> Drawer {
        let mut frames = Frames::new();
        frames.hsd.widescreen = widescreen;
        // `CHECK_THREADED=1` draws on the renderer's own threads, as the player does.
        let gpu = if std::env::var_os("CHECK_THREADED").is_some() {
            Gpu::threaded(renderer)
        } else {
            Gpu::new(renderer)
        };
        Drawer {
            frames,
            gpu,
            loaded: false,
        }
    }

    /// Runs the tick's frame: drawn (watched by `observer`), or only followed, as the GPU's
    /// registers follow the stream all the same. Returns the frames it copied to the XFB and
    /// what melee-render noted it didn't draw exactly.
    fn frame(
        &mut self,
        t: &Tick<'_>,
        draw: bool,
        observer: Option<&dyn Observer>,
    ) -> (Vec<Frame>, Vec<(String, u32)>) {
        if !self.loaded {
            self.frames.space.load_dol(t.dol);
            self.loaded = true;
        }
        let fifo = self.frames.frame(t.game, t.data, t.draws);
        let images = if draw {
            self.gpu.run(&self.frames.space, t.data, &fifo, observer)
        } else {
            self.gpu.follow(&self.frames.space, t.data, &fifo);
            Vec::new()
        };
        let notes = std::mem::take(&mut self.frames.hsd.notes)
            .into_iter()
            .collect();
        (images, notes)
    }
}

/// Frames drawn before a frame that's wanted: their copies are what it may sample. Two
/// cover the shadows' and the magnifier's copies, made the frame before; Pokémon Stadium's
/// screens show a copy of the screen from the frame before, which shows the copy from the
/// frame before that, each smaller, so a frame of its feed needs four (fewer left a few of
/// the feed's pixels different at frame 9000, `fox_marth_ps_9000`), and eight leave margin.
const WARM_UP: u32 = 8;

/// Whether frame `n` is drawn for a frame `at` wanted: it is `at` or one of the
/// [`WARM_UP`] frames before.
fn drawn_for(n: u32, at: u32) -> bool {
    n <= at && n + WARM_UP >= at
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
    let mut drawer = Drawer::new(renderer(), None);
    drive(disc_path, replay_path, to, Cosmetic::default(), |t| {
        if t.n < from {
            drawer.frame(t, drawn_for(t.n, from), None);
            return true;
        }
        let rec = Recorder::default();
        let (images, notes) = drawer.frame(t, true, Some(&Watch(&rec)));
        let drawn = Drawn {
            replay_frame: t.replay_frame,
            events: rec.events.into_inner(),
            image: images.into_iter().last(),
            notes,
        };
        each(t.n, drawn)
    });
}

/// How a frame is drawn: the EFB's scale and width (for the console's 640 columns), the aspect
/// ratio the cameras widen to, as the player's aspect sets them, and whether the copy to the
/// XFB runs on the GPU at the EFB's size, as the player's does, or on the CPU at the console's,
/// as Dolphin's software renderer and ssbm-rs's do (only at 1x).
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub scale: u32,
    pub width: u32,
    pub widescreen: Option<f32>,
    pub gpu_xfb: bool,
}

impl View {
    /// The frame `compare` checks against ssbm-rs's: 640 by 480, copied on the CPU.
    pub const REFERENCE: View = View {
        scale: 1,
        width: 640,
        widescreen: None,
        gpu_xfb: false,
    };

    /// The console's 640 by 480 times `scale`, copied to the XFB on the GPU.
    pub const fn console(scale: u32) -> View {
        View {
            scale,
            width: 640 * scale,
            widescreen: None,
            gpu_xfb: true,
        }
    }

    /// The player's frame at `scale` for an aspect ratio: the game's 73:60 as it is, or wider
    /// with the cameras widened (the player's `Aspect::efb_width`).
    pub fn shown(scale: u32, aspect: f32) -> View {
        let width = (480.0 * scale as f32 * aspect / 2.0).round() as u32 * 2;
        let widescreen = (aspect > melee_render::frame::GAME_ASPECT + 0.001).then_some(aspect);
        View {
            scale,
            width,
            widescreen,
            gpu_xfb: true,
        }
    }
}

/// Each view's image of each of the replay's native frames `at`, each view drawn by a renderer
/// of its own: for each frame, its number and the views' images. With a widescreen view the
/// fighters off the game camera's view draw, as the player draws them in widescreen
/// (`Cosmetic::draw_offscreen_fighters`), in every view: the narrower ones clip them.
pub fn draw_views(
    disc_path: &str,
    replay_path: &str,
    at: &[u32],
    views: &[View],
) -> Vec<(u32, Vec<Frame>)> {
    let mut drawers: Vec<Drawer> = views
        .iter()
        .map(|v| {
            let mut r = renderer();
            r.resize_efb(v.scale, v.width);
            (r.xfb_on_gpu, r.xfb_readback) = (v.gpu_xfb, v.gpu_xfb);
            Drawer::new(r, v.widescreen)
        })
        .collect();
    let last = at.iter().copied().max().unwrap_or(0);
    let mut out = Vec::new();
    let cosmetic = Cosmetic {
        draw_offscreen_fighters: views.iter().any(|v| v.widescreen.is_some()),
    };
    drive(disc_path, replay_path, last, cosmetic, |t| {
        let draw = at.iter().any(|&a| drawn_for(t.n, a));
        let images: Vec<Option<Frame>> = drawers
            .iter_mut()
            .map(|d| d.frame(t, draw, None).0.into_iter().last())
            .collect();
        if at.contains(&t.n) {
            let images = images
                .into_iter()
                .zip(views)
                .map(|(f, v)| {
                    f.unwrap_or_else(|| panic!("frame {} at {v:?}: nothing copied to the XFB", t.n))
                })
                .collect();
            out.push((t.n, images));
        }
        t.n < last
    });
    out
}

/// How far two frames' draws are apart, for lining frames up: the share of `b`'s draws that
/// don't line up (by signature, as `compare` lines events up) with a draw of `a` whose
/// vertices sit where theirs do, and how many lined up at all.
pub fn distance(a: &[Event], b: &[Event]) -> (f64, usize) {
    let mut exact = 0usize;
    let mut matched = 0usize;
    for (i, j) in crate::snap::align(a, b) {
        let (Some(i), Some(j)) = (i, j) else { continue };
        let (Event::Draw(x), Event::Draw(y)) = (&a[i], &b[j]) else {
            continue;
        };
        matched += 1;
        let same = x.verts.iter().zip(&y.verts).all(|(p, q)| {
            (0..2).all(|k| {
                let e = (p.screen[k] - q.screen[k]).abs();
                e < 0.05 || !e.is_finite()
            })
        });
        exact += usize::from(same);
    }
    (1.0 - exact as f64 / b.len().max(1) as f64, matched)
}

/// A renderer for checks: `CHECK_DRAW_LIMIT=N` draws only each frame's first N draws, to find
/// the draw where two frames' images part.
pub fn renderer() -> melee_gpu::Renderer {
    let mut r = melee_gpu::Renderer::new().expect("a GPU");
    r.draw_limit = std::env::var("CHECK_DRAW_LIMIT")
        .ok()
        .map(|n| n.parse().expect("CHECK_DRAW_LIMIT=N"));
    r
}
