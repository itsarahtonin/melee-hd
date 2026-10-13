//! melee-hd's player: a VS match in a window.
//!
//! ```text
//! melee-hd [--p1 fox] [--p2 marth] [--stage fd] [--stocks 4] [--minutes 8] [--disc ISO]
//!          [--replay FILE.slp] [--widescreen | --aspect W:H] [--scale N] [--no-adapter] [--size WxH] [--ticks N]
//! melee-hd --bench TICKS [--size WxH] [...]     plays headless, timing each frame
//! ```
//!
//! The disc comes from `--disc` or `SSBM_DISC`. Characters: fox, falco, marth, sheik, zelda;
//! stages: fd (Final Destination), bf (Battlefield), ps (Pokémon Stadium), as far as
//! melee-sim has them ported. Controllers: GameCube controllers on a Wii U adapter play their
//! own ports; gamepads and the keyboard play the first ports left (arrows: stick, IJKL:
//! C-stick, X/Z/C/S: A/B/X/Y, D: Z, Q/W: L/R, Enter: start, left shift: half stick). Escape
//! quits.
//!
//! The loop keeps latency short: each tick reads the controllers just before it runs, and the
//! frame it renders is drawn and presented at once, on a swap chain one frame deep (mailbox,
//! or immediate where the display has no mailbox mode).

mod pipelines;
mod present;
mod sound;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_data::disc::Disc;
use melee_input::{Inputs, Key};
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};
use melee_slippi::Replay;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

/// Ticks a second: the engine's pad-polling rate.
const TICK_RATE: f64 = 60.0;

/// The command line.
struct Args {
    disc: String,
    players: Vec<(u8, u8)>,
    stage: u16,
    stocks: i8,
    minutes: u32,
    replay: Option<String>,
    adapter: bool,
    bench: Option<u32>,
    size: (u32, u32),
    /// Quit after this many ticks.
    ticks: Option<u64>,
    /// The internal resolution, in multiples of the console's 640 by 480 (0: from the
    /// window's size).
    scale: u32,
    /// Saves the last frame the bench draws.
    shot: Option<String>,
    /// Shows frames this wide (width over height), seeing more at the sides; 4:3 otherwise.
    widescreen: Option<f32>,
}

/// A character's `CharacterKind` by name.
fn character(name: &str) -> Option<u8> {
    Some(match name {
        "fox" => 2,
        "marth" => 9,
        "zelda" => 18,
        "sheik" => 19,
        "falco" => 20,
        _ => return None,
    })
}

/// A stage's `StKind` by name.
fn stage(name: &str) -> Option<u16> {
    Some(match name {
        "fd" | "final-destination" => 32,
        "bf" | "battlefield" => 31,
        "ps" | "pokemon-stadium" => 3,
        _ => return None,
    })
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        disc: std::env::var("SSBM_DISC").unwrap_or_default(),
        players: vec![(2, 0), (2, 1)],
        stage: 32,
        stocks: 4,
        minutes: 8,
        replay: None,
        adapter: true,
        bench: None,
        size: (1280, 960),
        ticks: None,
        scale: 0,
        shot: None,
        widescreen: None,
    };
    let mut it = std::env::args().skip(1);
    let size = |s: &str| -> Result<(u32, u32), String> {
        let (w, h) = s.split_once('x').ok_or("a size is WxH")?;
        Ok((
            w.parse().map_err(|_| "a width")?,
            h.parse().map_err(|_| "a height")?,
        ))
    };
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--disc" => a.disc = value()?,
            "--p1" | "--p2" | "--p3" | "--p4" => {
                let port = usize::from(arg.as_bytes()[3] - b'1');
                let name = value()?;
                let c = character(&name).ok_or(format!("unknown character {name}"))?;
                if a.players.len() <= port {
                    a.players.resize(port + 1, (0xFF, 0));
                }
                a.players[port] = (c, 0);
            }
            "--stage" => {
                let name = value()?;
                a.stage = stage(&name).ok_or(format!("unknown stage {name}"))?;
            }
            "--stocks" => a.stocks = value()?.parse().map_err(|_| "--stocks N")?,
            "--minutes" => a.minutes = value()?.parse().map_err(|_| "--minutes N")?,
            "--replay" => a.replay = Some(value()?),
            "--no-adapter" => a.adapter = false,
            "--bench" => a.bench = Some(value()?.parse().map_err(|_| "--bench TICKS")?),
            "--size" | "--windowed" => a.size = size(&value()?)?,
            "--ticks" => a.ticks = Some(value()?.parse().map_err(|_| "--ticks N")?),
            "--scale" => a.scale = value()?.parse().map_err(|_| "--scale N")?,
            "--shot" => a.shot = Some(value()?),
            "--widescreen" => a.widescreen = Some(16.0 / 9.0),
            "--aspect" => {
                let v = value()?;
                let (w, h) = v.split_once(':').ok_or("--aspect W:H")?;
                let w: f32 = w.parse().map_err(|_| "--aspect W:H")?;
                let h: f32 = h.parse().map_err(|_| "--aspect W:H")?;
                a.widescreen = (w / h > 4.0 / 3.0 + 0.01).then_some(w / h);
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if a.disc.is_empty() {
        return Err("no disc: pass --disc ISO or set SSBM_DISC".into());
    }
    Ok(a)
}

/// The match and what drives it: the controllers, or a replay's recorded inputs.
struct Session {
    m: Match,
    /// The game's executable, whose static data the renderer reads (the default font).
    dol: Vec<u8>,
    replay: Option<Replay>,
    ended: bool,
    sound: Option<sound::Sound>,
}

impl Session {
    fn start(a: &Args) -> Result<Session, String> {
        let disc = Disc::open(&a.disc).map_err(|e| format!("{}: {e}", a.disc))?;
        let mut load = |name: &str| DiscFile {
            entrynum: disc
                .entrynum(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
            bytes: disc
                .read_file(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
        };
        let (settings, replay) = match &a.replay {
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                let replay = Replay::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
                let s = Settings::from_replay(&melee_slippi::start(&replay))
                    .map_err(|e| format!("{path}: {e}"))?;
                (s, Some(replay))
            }
            None => {
                let players: Vec<Player> = a
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.0 != 0xFF)
                    .map(|(i, &(c, costume))| Player {
                        character: c,
                        costume,
                        port: i as u8,
                        kind: 0,
                        cpu_level: 0,
                        team: i as u8,
                    })
                    .collect();
                let mut s = Settings::vs(a.stage, &players, a.stocks, a.minutes * 60);
                // A seed from the clock: presentation-side, set once before the match.
                s.seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.subsec_nanos());
                (s, None)
            }
        };
        let (m, sound) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let play = a.bench.is_none();
            sound::start(&settings, play, &mut load)
        }))
        .map_err(|p| format!("the match couldn't start: {}", panic_text(&p)))??;
        Ok(Session {
            m,
            dol: disc.dol().map_err(|e| format!("main.dol: {e}"))?,
            replay,
            ended: false,
            sound,
        })
    }

    /// One engine tick with these controllers; the frame's draws if it renders.
    fn tick(
        &mut self,
        pads: [melee_sim::input::pad::PadStatus; 4],
    ) -> melee_sim::game::vs_match::Frame {
        let f = match &self.replay {
            Some(r) => self.m.step_replay(&mut melee_slippi::Device(r)),
            None => self.m.step(pads),
        };
        if let Some(s) = &mut self.sound {
            s.tick(&mut self.m);
        }
        self.ended |= f.ended;
        f
    }
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// The keyboard controller's keys.
fn key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::KeyJ => Key::CLeft,
        KeyCode::KeyL => Key::CRight,
        KeyCode::KeyI => Key::CUp,
        KeyCode::KeyK => Key::CDown,
        KeyCode::KeyX => Key::A,
        KeyCode::KeyZ => Key::B,
        KeyCode::KeyC => Key::X,
        KeyCode::KeyS => Key::Y,
        KeyCode::KeyD => Key::Z,
        KeyCode::KeyQ => Key::L,
        KeyCode::KeyW => Key::R,
        KeyCode::Enter => Key::Start,
        KeyCode::KeyT => Key::DUp,
        KeyCode::KeyG => Key::DDown,
        KeyCode::KeyF => Key::DLeft,
        KeyCode::KeyH => Key::DRight,
        KeyCode::ShiftLeft => Key::Modifier,
        _ => return None,
    })
}

/// The frame time statistics the loop logs.
#[derive(Default)]
struct Stats {
    ticks: u64,
    frames: u64,
    work: Duration,
    worst: Duration,
    since: Option<Instant>,
}

impl Stats {
    fn frame(&mut self, work: Duration) {
        self.frames += 1;
        self.work += work;
        self.worst = self.worst.max(work);
        let since = *self.since.get_or_insert_with(Instant::now);
        let t = since.elapsed();
        if t >= Duration::from_secs(5) {
            let s = t.as_secs_f64();
            eprintln!(
                "{:.2} ticks/s, {:.2} frames/s, tick+frame {:.2} ms average, {:.2} ms worst",
                self.ticks as f64 / s,
                self.frames as f64 / s,
                self.work.as_secs_f64() * 1000.0 / self.frames.max(1) as f64,
                self.worst.as_secs_f64() * 1000.0
            );
            *self = Stats {
                since: Some(Instant::now()),
                ..Stats::default()
            };
        }
    }
}

struct App {
    session: Session,
    frames: Frames,
    gpu: present::Gpu,
    renderer: Option<Gpu>,
    shown: Option<present::Shown>,
    inputs: Inputs,
    next_tick: Instant,
    stats: Stats,
    quit: bool,
    ticks: u64,
    max_ticks: Option<u64>,
    scale: u32,
}

impl App {
    /// Runs the ticks that are due: controllers read, tick, frame drawn and presented.
    fn run_due(&mut self) {
        let period = Duration::from_secs_f64(1.0 / TICK_RATE);
        let now = Instant::now();
        if now < self.next_tick {
            return;
        }
        // Far behind (a stall, a dragged window): drop the backlog rather than race.
        if now - self.next_tick > 4 * period {
            self.next_tick = now;
        }
        while Instant::now() >= self.next_tick && !self.session.ended {
            self.next_tick += period;
            let start = Instant::now();
            let pads = self.inputs.read();
            let f = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.session.tick(pads)
            })) {
                Ok(f) => f,
                Err(p) => {
                    eprintln!("the match stopped: {}", panic_text(&p));
                    self.session.ended = true;
                    return;
                }
            };
            self.stats.ticks += 1;
            self.ticks += 1;
            if self.max_ticks.is_some_and(|n| self.ticks >= n) {
                self.session.ended = true;
            }
            if !f.draws.is_empty() {
                self.draw_frame(&f.draws);
                self.stats.frame(start.elapsed());
            }
            self.session.m.recycle(f);
        }
    }

    fn draw_frame(&mut self, draws: &[melee_sim::hsd::display::Draw]) {
        let m = &self.session.m;
        let fifo = self.frames.frame(&m.game, &m.data, draws);
        let gpu = self
            .renderer
            .get_or_insert_with(|| Gpu::new(pipelines::renderer(&self.gpu, self.scale)));
        let images = gpu.run(&self.frames.space, &m.data, &fifo, None);
        if let (Some(shown), Some(frame)) = (&mut self.shown, images.into_iter().last()) {
            shown.present(&self.gpu, &frame);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_none() {
            self.shown = Some(present::Shown::new(&self.gpu, event_loop));
            self.next_tick = Instant::now();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.quit = true;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(s) = &mut self.shown {
                    s.resize(&self.gpu, size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        repeat: false,
                        ..
                    },
                ..
            } => {
                if code == KeyCode::Escape {
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
                if let Some(k) = key(code) {
                    self.inputs.keyboard.set(k, state == ElementState::Pressed);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit {
            return;
        }
        self.run_due();
        if self.session.ended {
            eprintln!("the match ended");
            self.quit = true;
            event_loop.exit();
            return;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
    }
}

/// The internal resolution's scale: as asked, or the smallest whose 480 lines cover the
/// window's height.
fn scale_for(asked: u32, size: (u32, u32)) -> u32 {
    if asked != 0 {
        return asked.clamp(1, 8);
    }
    size.1.div_ceil(480).clamp(1, 8)
}

/// Plays `ticks` ticks headless, drawing each frame and presenting it to an offscreen target
/// of `size`, and reports the time a tick with its frame takes.
fn bench(args: &Args, ticks: u32) -> ExitCode {
    let mut session = match Session::start(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let gpu = present::Gpu::new();
    let mut target = present::Offscreen::new(&gpu, args.size.0, args.size.1);
    let mut r = pipelines::renderer(&gpu, scale_for(args.scale, args.size));
    r.xfb_readback = args.shot.is_some();
    let mut renderer = Gpu::new(r);
    let mut last = None;
    let mut frames = Frames::new();
    frames.space.load_dol(&session.dol);
    frames.hsd.widescreen = args.widescreen;
    present::set_aspect(args.widescreen.unwrap_or(4.0 / 3.0));
    let pads = [melee_sim::input::pad::PadStatus::default(); 4];
    let (mut sim, mut draw, mut n) = (Duration::ZERO, Duration::ZERO, 0u32);
    let mut worst = Duration::ZERO;
    let mut parts = [Duration::ZERO; 3];
    for _ in 0..ticks {
        let t0 = Instant::now();
        let f = session.tick(pads);
        let t1 = Instant::now();
        if !f.draws.is_empty() {
            let m = &session.m;
            let fifo = frames.frame(&m.game, &m.data, &f.draws);
            let ta = Instant::now();
            let images = renderer.run(&frames.space, &m.data, &fifo, None);
            let tb = Instant::now();
            if let Some(frame) = images.into_iter().last() {
                target.present(&gpu, &frame);
                last = Some(frame);
            }
            let t2 = Instant::now();
            parts[0] += ta - t1;
            parts[1] += tb - ta;
            parts[2] += t2 - tb;
            draw += t2 - t1;
            worst = worst.max(t2 - t0);
            n += 1;
        }
        sim += t1 - t0;
        session.m.recycle(f);
        if session.ended {
            break;
        }
    }
    println!(
        "{n} frames at {}x{} (internal {}x): simulation {:.3} ms a tick, drawing {:.3} ms a frame, worst tick+frame {:.3} ms",
        args.size.0,
        args.size.1,
        scale_for(args.scale, args.size),
        sim.as_secs_f64() * 1000.0 / f64::from(ticks.max(1)),
        draw.as_secs_f64() * 1000.0 / f64::from(n.max(1)),
        worst.as_secs_f64() * 1000.0
    );
    if let (Some(path), Some(f)) = (&args.shot, &last) {
        f.save_png(std::path::Path::new(path))
            .unwrap_or_else(|e| eprintln!("{path}: {e}"));
    }
    pipelines::save(&renderer.renderer.borrow());
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / f64::from(n.max(1));
    println!(
        "a frame: GX stream {:.3} ms, renderer {:.3} ms, presenting (with the GPU's wait) {:.3} ms",
        ms(parts[0]),
        ms(parts[1]),
        ms(parts[2])
    );
    for (what, count) in &frames.hsd.notes {
        println!("not drawn exactly: {what} ({count})");
    }
    ExitCode::SUCCESS
}

fn run() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!(
                "usage: melee-hd [--p1 fox] [--p2 marth] [--stage fd|bf|ps] [--stocks N] [--minutes N] [--disc ISO] [--replay FILE] [--no-adapter] [--size WxH] [--bench TICKS]"
            );
            return ExitCode::from(2);
        }
    };
    if let Some(ticks) = args.bench {
        return bench(&args, ticks);
    }
    let session = match Session::start(&args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let event_loop = {
        let mut b = EventLoop::builder();
        // The player runs on a thread with a deep stack (see `main`), not the process's first.
        #[cfg(windows)]
        winit::platform::windows::EventLoopBuilderExtWindows::with_any_thread(&mut b, true);
        b.build().expect("an event loop")
    };
    let gpu = present::Gpu::new();
    let mut frames = Frames::new();
    frames.space.load_dol(&session.dol);
    frames.hsd.widescreen = args.widescreen;
    present::set_aspect(args.widescreen.unwrap_or(4.0 / 3.0));
    let mut app = App {
        session,
        frames,
        gpu,
        renderer: None,
        shown: None,
        inputs: Inputs::new(args.adapter),
        next_tick: Instant::now(),
        stats: Stats::default(),
        quit: false,
        ticks: 0,
        max_ticks: args.ticks,
        scale: scale_for(args.scale, args.size),
    };
    present::set_size(args.size);
    // The pipelines the last runs needed, made before the first tick.
    app.renderer = Some(Gpu::new(pipelines::renderer(&app.gpu, app.scale)));
    let r = event_loop.run_app(&mut app);
    app.inputs.close();
    if let Some(g) = &app.renderer {
        pipelines::save(&g.renderer.borrow());
    }
    if let Err(e) = r {
        eprintln!("the window failed: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    // The simulation recurses as the original does (joint trees): a deep stack. The window
    // must live on the main thread on some systems, so the main thread gets the stack by
    // running everything on a thread that is itself the event loop's only user.
    let child = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(run)
        .expect("the player's thread");
    child.join().unwrap_or(ExitCode::FAILURE)
}
