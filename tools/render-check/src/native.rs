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

/// The console a recorded session plays on: the recording's reads in order (the last one
/// again past its end, as ssbm-rs's playback does) and the disc.
struct SessionConsole<'a> {
    reads: &'a [melee_sim::input::controller::PadData],
    next: usize,
    disc: &'a Disc,
}

impl melee_sim::game::session::Console for SessionConsole<'_> {
    fn read_pads(&mut self) -> melee_sim::input::controller::PadData {
        let pads = self.reads[self.next.min(self.reads.len() - 1)];
        self.next += 1;
        pads
    }

    fn unread(&mut self, n: usize) {
        self.next -= n;
    }

    fn entrynum(&mut self, path: &str) -> Option<i32> {
        self.disc.entrynum(path.trim_start_matches('/')).ok()
    }

    fn file_info(&mut self, path: &str) -> (i32, u32) {
        let path = path.trim_start_matches('/');
        self.disc
            .file_info(path)
            .unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn file(&mut self, path: &str) -> DiscFile {
        let path = path.trim_start_matches('/');
        DiscFile {
            entrynum: self
                .disc
                .entrynum(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
            bytes: self
                .disc
                .read_file(path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
        }
    }
}

/// Plays a recorded console session (ssbm-rs's `SSBMINP1`, a `.inputs` file) from power-on to
/// its rendered frame `to`, as [`drive`] plays a replay; `replay_frame` counts the session's
/// engine ticks. In the console's float mode.
fn drive_session(
    disc_path: &str,
    inputs_path: &str,
    to: u32,
    mut each: impl FnMut(&Tick<'_>) -> bool,
) {
    use melee_sim::game::session::{Session, Setup};
    melee_sim::gekko_fp::set_fp_mode(melee_sim::gekko_fp::FpMode::Hardware);
    let disc = Disc::open(disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}"));
    let rec = melee_sim::game::inputs::Recording::read(std::path::Path::new(inputs_path))
        .unwrap_or_else(|e| panic!("{e}"));
    let card = rec.card.as_ref().map(|p| {
        std::fs::read(p).unwrap_or_else(|e| panic!("the memory card {}: {e}", p.display()))
    });
    let setup = Setup {
        disc: rec.disc().unwrap_or_else(|e| panic!("{inputs_path}: {e}")),
        dsp: rec.dsp(),
        card,
        power_on: rec.power_on,
        dtv: rec.dtv,
        sram_flags: rec.sram_flags,
        card_pull: None,
    };
    let mut console = SessionConsole {
        reads: &rec.reads,
        next: 0,
        disc: &disc,
    };
    let dol = disc.dol().unwrap_or_else(|e| panic!("main.dol: {e}"));
    let mut s = Session::new(setup);
    s.boot(&mut console);
    let (mut n, mut ticks) = (0u32, 0i32);
    loop {
        let (_, render) = s.step_tick(&mut console);
        ticks += 1;
        if render {
            let draws = s.render();
            let tick = Tick {
                n,
                replay_frame: ticks,
                game: &s.game,
                data: &s.data,
                draws: &draws,
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
    }
}

/// Plays the replay's match to its rendered frame `to`, calling `each` with each rendered
/// frame until it returns false; with `cosmetic`'s toggles, as the player sets them for the
/// frames it shows. A `.inputs` file is a recorded console session instead
/// ([`drive_session`]).
fn drive(
    disc_path: &str,
    replay_path: &str,
    to: u32,
    cosmetic: Cosmetic,
    mut each: impl FnMut(&Tick<'_>) -> bool,
) {
    if replay_path.ends_with(".inputs") {
        drive_session(disc_path, replay_path, to, each);
        return;
    }
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

/// What melee-render noted it didn't draw exactly over the replay's match, up to its rendered
/// frame `to`, building each frame's stream without drawing it: each note with how often it
/// came and the first frame it came in.
pub fn notes(
    disc_path: &str,
    replay_path: &str,
    to: u32,
) -> std::collections::BTreeMap<String, (u32, u32)> {
    let mut frames = Frames::new();
    let mut loaded = false;
    let mut out = std::collections::BTreeMap::new();
    drive(disc_path, replay_path, to, Cosmetic::default(), |t| {
        if !loaded {
            frames.space.load_dol(t.dol);
            loaded = true;
        }
        frames.frame(t.game, t.data, t.draws);
        for (what, n) in std::mem::take(&mut frames.hsd.notes) {
            out.entry(what).or_insert((0, t.n)).0 += n;
        }
        true
    });
    out
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

/// The replay's native frame `at` drawn as `view` draws it (and the [`WARM_UP`] frames before,
/// whose copies it may sample), with its events recorded, and the address of Fountain of
/// Dreams' reflection image, which its draws sample, if the frame has one.
pub fn record(disc_path: &str, replay_path: &str, at: u32, view: View) -> (Drawn, Option<u32>) {
    let mut r = renderer();
    r.resize_efb(view.scale, view.width);
    (r.xfb_on_gpu, r.xfb_readback) = (view.gpu_xfb, view.gpu_xfb);
    let mut drawer = Drawer::new(r, view.widescreen);
    let cosmetic = Cosmetic {
        draw_offscreen_fighters: view.widescreen.is_some(),
    };
    let mut out = None;
    drive(disc_path, replay_path, at, cosmetic, |t| {
        if t.n < at {
            drawer.frame(t, drawn_for(t.n, at), None);
            return true;
        }
        let rec = Recorder::default();
        let (images, notes) = drawer.frame(t, true, Some(&Watch(&rec)));
        let reflection = t.game.gr.izumi.reflection.is_some().then(|| {
            drawer
                .frames
                .space
                .dynamic(melee_render::mem::Dynamic::Reflection, 0)
        });
        let drawn = Drawn {
            replay_frame: t.replay_frame,
            events: rec.events.into_inner(),
            image: images.into_iter().last(),
            notes,
        };
        out = Some((drawn, reflection));
        false
    });
    out.unwrap_or_else(|| panic!("{replay_path}: the match ends before frame {at}"))
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
/// the draw where two frames' images part; `CHECK_COPIES=memory` writes the copies to memory
/// that a log captured with ssbm-rs's renderer running has there (`Capture::Rendered`), so the
/// draws that sample them compare their texels: ssbm-render encodes every copy into memory
/// but those it keeps on the GPU (I4 and RGB5A3), and melee-gpu keeps RGB565 copies there too
/// (refraction's screen copy, Pokémon Stadium's screens, Fountain of Dreams' reflection).
pub fn renderer() -> melee_gpu::Renderer {
    let mut r = melee_gpu::Renderer::new().expect("a GPU");
    r.draw_limit = std::env::var("CHECK_DRAW_LIMIT")
        .ok()
        .map(|n| n.parse().expect("CHECK_DRAW_LIMIT=N"));
    if std::env::var("CHECK_COPIES").as_deref() == Ok("memory") {
        r.copies_to_memory = vec![4];
    }
    r
}
