//! melee-hd's player: a VS match in a window.
//!
//! ```text
//! melee-hd [--p1 fox] [--p2 marth] [--stage fd] [--stocks 4] [--minutes 8] [--disc ISO]
//!          [--replay FILE.slp] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread]
//!          [--no-adapter] [--size WxH] [--fullscreen] [--ticks N]
//! melee-hd --bench TICKS [--size WxH] [...]     plays headless, timing each frame
//! melee-hd --launcher-shot FILE.png [--size WxH] draws the launcher to a PNG
//! ```
//!
//! The disc comes from `--disc` or `SSBM_DISC`. Characters and stages are melee-sim's
//! `game::roster`, what it has ported (`--help` lists them). With no `--p1` to `--p4`, `--stage`
//! or `--replay` (and no `--bench`), the window opens on the launcher, which picks the match
//! ([`launcher`]). Controllers: GameCube controllers on a Wii U adapter play their own ports;
//! gamepads and the keyboard play the first ports left (arrows: stick, IJKL: C-stick, X/Z/C/S:
//! A/B/X/Y, D: Z, Q/W: L/R, Enter: start, left shift: half stick). F11 or Alt+Enter toggles
//! borderless fullscreen. Escape quits, or from a match the launcher started, goes back to it.
//!
//! Frames are shown at the game's 73:60, pillarboxed; `--widescreen` (16:9) or `--aspect W:H`
//! widens the drawn cameras to a wider aspect, and `--aspect 4:3` stretches the picture to 4:3
//! ([`present::Aspect`]).
//!
//! The loop keeps latency short: each tick reads the controllers just before it runs, and the
//! frame it renders is drawn and presented at once, on a swap chain one frame deep (mailbox,
//! or immediate where the display has no mailbox mode).

mod latency;
mod launcher;
mod pipelines;
mod present;
mod sound;

use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_data::disc::Disc;
use melee_input::{Inputs, Key};
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_sim::game::roster;
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
#[derive(Clone)]
struct Args {
    disc: String,
    players: Vec<(u8, u8)>,
    stage: u16,
    stocks: i8,
    minutes: u32,
    replay: Option<String>,
    adapter: bool,
    /// Writes each tick's latency measures to this CSV, with a summary at exit.
    latency_log: Option<String>,
    /// How frames reach the display.
    present: present::Present,
    /// Ticks each controller read is held back before the simulation sees it (0: none).
    added_delay: u32,
    /// Takes the audio device in exclusive mode (Windows).
    audio_exclusive: bool,
    /// Starts with the latency overlay shown (F3 toggles it).
    latency_overlay: bool,
    bench: Option<u32>,
    size: (u32, u32),
    /// Quit after this many ticks.
    ticks: Option<u64>,
    /// The internal resolution, in multiples of the console's 640 by 480 (0: from the
    /// window's size).
    scale: u32,
    /// Saves the last frame the bench draws.
    shot: Option<String>,
    /// The aspect ratio frames are shown at: the game's 73:60, wider (seeing more at the
    /// sides), or 4:3 stretched.
    aspect: present::Aspect,
    /// Keeps the renderer on the game's thread instead of threads of its own.
    one_thread: bool,
    /// Samples a pixel: 4 for MSAA, 1 as the console draws.
    msaa: u32,
    /// No match on the command line: the launcher picks one.
    launch: bool,
    /// Draws the launcher to this PNG and quits.
    launcher_shot: Option<String>,
    /// Borderless fullscreen on the window's monitor.
    fullscreen: bool,
}

/// The command line's usage, with the characters and stages melee-sim has ported.
fn usage() -> String {
    let names = |keys: Vec<&str>| keys.join("|");
    format!(
        "usage: melee-hd [--p1 NAME] [--p2 NAME] [--p3 NAME] [--p4 NAME] [--stage NAME] [--stocks N] [--minutes N] [--disc ISO] [--replay FILE] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread] [--no-adapter] [--size WxH] [--fullscreen] [--bench TICKS] [--launcher-shot FILE.png]\n\
         characters: {}\n\
         stages: {}\n\
         latency: [--latency-log FILE.csv] [--latency-overlay] [--present mailbox|fifo|immediate] [--added-delay FRAMES] [--audio-exclusive]\n\
         with no --p1..--p4, --stage or --replay, the window opens on a launcher that picks the match",
        names(roster::CHARACTERS.iter().map(|c| c.keys[0]).collect()),
        names(roster::STAGES.iter().map(|s| s.keys[0]).collect()),
    )
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
        latency_log: None,
        present: present::Present::Mailbox,
        added_delay: 0,
        audio_exclusive: false,
        latency_overlay: false,
        bench: None,
        size: (1280, 960),
        ticks: None,
        scale: 0,
        shot: None,
        aspect: present::Aspect::GAME,
        one_thread: false,
        msaa: 1,
        launch: false,
        launcher_shot: None,
        fullscreen: false,
    };
    // Whether the command line names a match.
    let mut chosen = false;
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
                let c = roster::character(&name)
                    .ok_or(format!("unknown character {name}"))?
                    .kind;
                chosen = true;
                if a.players.len() <= port {
                    a.players.resize(port + 1, (0xFF, 0));
                }
                a.players[port] = (c, 0);
            }
            "--stage" => {
                let name = value()?;
                a.stage = roster::stage(&name)
                    .ok_or(format!("unknown stage {name}"))?
                    .kind;
                chosen = true;
            }
            "--stocks" => a.stocks = value()?.parse().map_err(|_| "--stocks N")?,
            "--minutes" => a.minutes = value()?.parse().map_err(|_| "--minutes N")?,
            "--replay" => {
                a.replay = Some(value()?);
                chosen = true;
            }
            "--no-adapter" => a.adapter = false,
            "--latency-log" => a.latency_log = Some(value()?),
            "--present" => a.present = present::Present::parse(&value()?)?,
            "--added-delay" => {
                a.added_delay = value()?.parse().map_err(|_| "--added-delay FRAMES")?
            }
            "--audio-exclusive" => a.audio_exclusive = true,
            "--latency-overlay" => a.latency_overlay = true,
            "--one-thread" => a.one_thread = true,
            "--msaa" => {
                a.msaa = match value()?.as_str() {
                    "1" | "off" => 1,
                    "4" => 4,
                    _ => return Err("--msaa 4 (or 1, off)".into()),
                }
            }
            "--bench" => a.bench = Some(value()?.parse().map_err(|_| "--bench TICKS")?),
            "--size" | "--windowed" => a.size = size(&value()?)?,
            "--ticks" => a.ticks = Some(value()?.parse().map_err(|_| "--ticks N")?),
            "--scale" => a.scale = value()?.parse().map_err(|_| "--scale N")?,
            "--shot" => a.shot = Some(value()?),
            "--launcher-shot" => a.launcher_shot = Some(value()?),
            "--fullscreen" => a.fullscreen = true,
            "--widescreen" => a.aspect = present::Aspect::wide(16.0 / 9.0),
            "--aspect" => a.aspect = present::Aspect::parse(&value()?)?,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if a.disc.is_empty() && a.launcher_shot.is_none() {
        return Err("no disc: pass --disc ISO or set SSBM_DISC".into());
    }
    a.launch = !chosen && a.bench.is_none();
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

/// The launcher, when the player started without a match: its form, and its egui state once
/// the window exists.
struct Launcher {
    form: launcher::Form,
    window: Option<launcher::Window>,
}

struct App {
    /// The match playing; none while the launcher shows.
    session: Option<Session>,
    launcher: Option<Launcher>,
    /// The command line, which the launcher's matches start from with its choices.
    args: Args,
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
    /// How frames are shown, and the renderer's EFB width for it at `scale`.
    aspect: present::Aspect,
    efb_width: u32,
    one_thread: bool,
    msaa: u32,
    /// The window's size, which the internal resolution follows.
    window_size: (u32, u32),
    /// Alt is held (Alt+Enter toggles fullscreen).
    alt: bool,
    /// Why the match stopped, for the launcher to show.
    stopped: Option<String>,
    /// The latency log, with `--latency-log`.
    latency: Option<latency::Log>,
    /// The input latency added back on purpose.
    delay: melee_input::Delay,
    /// The last second's latency, and the overlay showing it (F3), while it's on.
    live: latency::Live,
    overlay: Option<launcher::Painter>,
}

impl App {
    fn ended(&self) -> bool {
        self.session.as_ref().is_none_or(|s| s.ended)
    }

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
        while Instant::now() >= self.next_tick && !self.ended() {
            let due = self.next_tick;
            self.next_tick += period;
            let start = Instant::now();
            let pads = self.delay.apply(self.inputs.read());
            let session = self.session.as_mut().expect("a match");
            let f =
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.tick(pads)))
                {
                    Ok(f) => f,
                    Err(p) => {
                        let text = format!("the match stopped: {}", panic_text(&p));
                        eprintln!("{text}");
                        session.ended = true;
                        self.stopped = Some(text);
                        return;
                    }
                };
            let simulated = start.elapsed();
            let audio = session.sound.as_ref().and_then(sound::Sound::stats);
            self.stats.ticks += 1;
            self.ticks += 1;
            if self.max_ticks.is_some_and(|n| self.ticks >= n) {
                session.ended = true;
            }
            let mut frame = None;
            if !f.draws.is_empty() {
                frame = self.draw_frame(&f.draws, start);
                self.stats.frame(start.elapsed());
            }
            self.session.as_mut().expect("a match").m.recycle(f);
            let t = latency::Tick {
                at: Some(start),
                late: start.saturating_duration_since(due),
                ages: self.inputs.ages,
                adapter_reports: self.inputs.adapter.as_ref().map(|a| a.reports()),
                sim: simulated,
                frame,
                audio,
            };
            self.live.tick(t);
            if let Some(log) = &mut self.latency {
                log.tick(t);
            }
        }
    }

    /// Draws and presents a frame: when its stages finished, from `start`, its tick's start.
    fn draw_frame(
        &mut self,
        draws: &[melee_sim::hsd::display::Draw],
        start: Instant,
    ) -> Option<latency::FrameTimes> {
        // The internal resolution follows the window's size unless `--scale` fixed it, and
        // the EFB's width the aspect: the renderer is made again for a new one.
        let scale = scale_for(self.args.scale, self.window_size, self.aspect);
        let width = self.aspect.efb_width(scale);
        if (scale, width) != (self.scale, self.efb_width) {
            eprintln!("internal resolution: {scale}x ({width}x{})", 480 * scale);
            self.scale = scale;
            self.efb_width = width;
            self.drop_renderer();
        }
        let m = &self.session.as_ref().expect("a match").m;
        let fifo = self.frames.frame(&m.game, &m.data, draws);
        let gpu = self.renderer.get_or_insert_with(|| {
            gpu_for(
                pipelines::renderer(&self.gpu, self.scale, self.msaa, self.aspect),
                self.one_thread,
            )
        });
        let images = gpu.run(&self.frames.space, &m.data, &fifo, None);
        let rendered = start.elapsed();
        let (shown, frame) = (self.shown.as_mut()?, images.into_iter().last()?);
        let text = self.overlay.is_some().then(|| {
            self.live
                .text(&format!("{:?}", shown.present_mode()), self.delay.frames())
        });
        let (gpu, overlay) = (&self.gpu, &mut self.overlay);
        let (acquire, presented) = shown.present(gpu, &frame, |view, w, h| {
            if let (Some(p), Some(text)) = (overlay, text) {
                latency::overlay(p, gpu, &text, view, w, h);
            }
        })?;
        Some(latency::FrameTimes {
            rendered,
            acquire,
            presented: presented.saturating_duration_since(start),
        })
    }

    /// Saves the renderer's pipelines and lets it go; the next frame makes a new one.
    fn drop_renderer(&mut self) {
        if let Some(g) = self.renderer.take() {
            pipelines::save(&g.into_renderer());
        }
    }

    /// The window's new size: the surface follows it at once, and the internal resolution at
    /// the next frame.
    fn resized(&mut self, w: u32, h: u32) {
        if let Some(s) = &mut self.shown {
            s.resize(&self.gpu, w, h);
        }
        if w > 0 && h > 0 {
            self.window_size = (w, h);
        }
    }

    /// Shows the latency overlay, or hides it.
    fn toggle_overlay(&mut self) {
        self.overlay = match (self.overlay.take(), &self.shown) {
            (None, Some(s)) => {
                let mut p = launcher::Painter::new(&self.gpu, s.format());
                p.clear = false;
                Some(p)
            }
            _ => None,
        };
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(s) = &mut self.shown {
            s.set_fullscreen(!s.fullscreen());
            if let Some(l) = &mut self.launcher {
                l.form.fullscreen = s.fullscreen();
                s.window.request_redraw();
            }
        }
    }

    /// Shows frames at `aspect` from the next frame on.
    fn set_aspect(&mut self, aspect: present::Aspect) {
        self.aspect = aspect;
        self.frames.hsd.widescreen = aspect.widescreen();
        present::set_aspect(aspect.shown);
    }

    /// Starts the launcher's match in the window, or says why it can't.
    fn start_match(&mut self) {
        let Some(l) = &mut self.launcher else { return };
        let aspect = l.form.aspect;
        let (present, added_delay) = (l.form.present, l.form.added_delay);
        let c = &l.form.choice;
        let mut args = self.args.clone();
        args.players = c.ports();
        args.stage = c.stage;
        args.stocks = c.stocks;
        args.minutes = c.minutes;
        let names: Vec<&str> = c
            .players
            .iter()
            .flatten()
            .filter_map(|&k| roster::character_by_kind(k).map(|c| c.name))
            .collect();
        let stage = roster::stage_by_kind(c.stage).map_or("?", |s| s.name);
        eprintln!("the launcher starts {} on {stage}", names.join(" vs "));
        match Session::start(&args) {
            Ok(session) => {
                l.form.message = None;
                self.frames = Frames::new();
                self.frames.space.load_dol(&session.dol);
                self.set_aspect(aspect);
                self.efb_width = aspect.efb_width(self.scale);
                self.renderer = Some(gpu_for(
                    pipelines::renderer(&self.gpu, self.scale, self.msaa, aspect),
                    self.one_thread,
                ));
                if let Some(s) = &mut self.shown {
                    s.set_present(&self.gpu, present);
                }
                self.delay = melee_input::Delay::new(added_delay);
                // What the controllers did in the launcher doesn't reach the match.
                self.inputs.read();
                self.session = Some(session);
                self.ticks = 0;
                self.stats = Stats::default();
                self.next_tick = Instant::now();
            }
            Err(e) => {
                eprintln!("{e}");
                l.form.message = Some(e);
            }
        }
    }

    /// Leaves the match (its sound stops with it) for the launcher.
    fn back_to_launcher(&mut self) {
        eprintln!("back to the launcher");
        self.session = None;
        self.drop_renderer();
        if let Some(l) = &mut self.launcher {
            l.form.message = self.stopped.take();
        }
        if let Some(s) = &self.shown {
            s.window.request_redraw();
        }
    }

    /// Draws the launcher, starting its match if Start was pressed.
    fn redraw_launcher(&mut self) {
        let (Some(l), Some(shown)) = (&mut self.launcher, &mut self.shown) else {
            return;
        };
        let Some(w) = &mut l.window else { return };
        let start = w.redraw(&self.gpu, shown, &mut l.form);
        if l.form.fullscreen != shown.fullscreen() {
            shown.set_fullscreen(l.form.fullscreen);
        }
        if start {
            self.start_match();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_none() {
            let shown = present::Shown::new(&self.gpu, event_loop, self.args.fullscreen);
            if let Some(l) = &mut self.launcher {
                l.window = Some(launcher::Window::new(&self.gpu, &shown));
            }
            self.shown = Some(shown);
            self.next_tick = Instant::now();
            if self.args.latency_overlay {
                self.toggle_overlay();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // F11 and Alt+Enter toggle fullscreen, in a match and in the launcher alike.
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(code),
                    state: ElementState::Pressed,
                    repeat: false,
                    ..
                },
            ..
        } = &event
            && (*code == KeyCode::F11 || (*code == KeyCode::Enter && self.alt))
        {
            self.toggle_fullscreen();
            return;
        }
        // F3 shows or hides the latency overlay.
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::F3),
                    state: ElementState::Pressed,
                    repeat: false,
                    ..
                },
            ..
        } = &event
        {
            self.toggle_overlay();
            return;
        }
        let in_launcher = self.session.is_none();
        if in_launcher
            && let (
                Some(Launcher {
                    window: Some(w), ..
                }),
                Some(shown),
            ) = (&mut self.launcher, &self.shown)
        {
            w.event(shown, &event);
        }
        match event {
            WindowEvent::CloseRequested => {
                self.quit = true;
                event_loop.exit();
            }
            WindowEvent::Resized(size) => self.resized(size.width, size.height),
            WindowEvent::ModifiersChanged(m) => self.alt = m.state().alt_key(),
            WindowEvent::RedrawRequested if in_launcher => self.redraw_launcher(),
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
                if code == KeyCode::Escape && state == ElementState::Pressed {
                    if !in_launcher && self.launcher.is_some() {
                        self.back_to_launcher();
                        return;
                    }
                    let popup = self
                        .launcher
                        .as_ref()
                        .and_then(|l| l.window.as_ref())
                        .is_some_and(|w| w.popup_open());
                    if !(in_launcher && popup) {
                        self.quit = true;
                        event_loop.exit();
                    }
                    return;
                }
                // The launcher's key presses aren't the keyboard controller's; releases
                // always are, so no key stays held into a match.
                let pressed = state == ElementState::Pressed;
                if let Some(k) = key(code)
                    && !(in_launcher && pressed)
                {
                    self.inputs.keyboard.set(k, pressed);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit {
            return;
        }
        if self.session.is_some() {
            self.run_due();
            if !self.ended() {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
                return;
            }
            eprintln!("the match ended");
            if self.launcher.is_none() {
                self.quit = true;
                event_loop.exit();
                return;
            }
            self.back_to_launcher();
        }
        // The launcher: woken by events, or when egui asked for its next frame.
        let flow = match self.launcher.as_mut().and_then(|l| l.window.as_mut()) {
            Some(w) => match w.repaint_at {
                Some(t) if t <= Instant::now() => {
                    w.repaint_at = None;
                    if let Some(s) = &self.shown {
                        s.window.request_redraw();
                    }
                    ControlFlow::Wait
                }
                Some(t) => ControlFlow::WaitUntil(t),
                None => ControlFlow::Wait,
            },
            None => ControlFlow::Wait,
        };
        event_loop.set_control_flow(flow);
    }
}

/// The GPU's side of the frames: the renderer on threads of its own (each frame done before
/// it's presented), or on this one with `--one-thread`.
fn gpu_for(renderer: melee_gpu::Renderer, one_thread: bool) -> Gpu {
    if one_thread {
        Gpu::new(renderer)
    } else {
        Gpu::threaded(renderer)
    }
}

/// The internal resolution's scale: as asked, or the smallest whose 480 lines cover the
/// window's height, at most what the aspect's EFB width allows.
fn scale_for(asked: u32, size: (u32, u32), aspect: present::Aspect) -> u32 {
    let scale = if asked != 0 {
        asked
    } else {
        size.1.div_ceil(480)
    };
    scale.clamp(1, aspect.max_scale())
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
    let scale = scale_for(args.scale, args.size, args.aspect);
    let mut r = pipelines::renderer(&gpu, scale, args.msaa, args.aspect);
    r.xfb_readback = args.shot.is_some();
    let mut renderer = gpu_for(r, args.one_thread);
    let mut last = None;
    let mut frames = Frames::new();
    frames.space.load_dol(&session.dol);
    frames.hsd.widescreen = args.aspect.widescreen();
    present::set_aspect(args.aspect.shown);
    let pads = [melee_sim::input::pad::PadStatus::default(); 4];
    let (mut sim, mut draw, mut n) = (Duration::ZERO, Duration::ZERO, 0u32);
    let mut worst = Duration::ZERO;
    let mut parts = [Duration::ZERO; 3];
    // Each drawn frame's ticks and drawing (all the ticks of its round), and with its
    // presenting.
    let mut work: Vec<Duration> = Vec::with_capacity(ticks as usize);
    let mut presented: Vec<Duration> = Vec::with_capacity(ticks as usize);
    let mut round = Duration::ZERO;
    for _ in 0..ticks {
        let t0 = Instant::now();
        let f = session.tick(pads);
        let t1 = Instant::now();
        round += t1 - t0;
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
            work.push(round + (tb - t1));
            presented.push(round + (t2 - t1));
            round = Duration::ZERO;
            n += 1;
        }
        sim += t1 - t0;
        session.m.recycle(f);
        if session.ended {
            break;
        }
    }
    println!(
        "{n} frames at {}x{} (internal {scale}x, EFB {}x{}): simulation {:.3} ms a tick, drawing {:.3} ms a frame, worst tick+frame {:.3} ms",
        args.size.0,
        args.size.1,
        args.aspect.efb_width(scale),
        480 * scale,
        sim.as_secs_f64() * 1000.0 / f64::from(ticks.max(1)),
        draw.as_secs_f64() * 1000.0 / f64::from(n.max(1)),
        worst.as_secs_f64() * 1000.0
    );
    if let (Some(path), Some(f)) = (&args.shot, &last) {
        f.save_png(std::path::Path::new(path))
            .unwrap_or_else(|e| eprintln!("{path}: {e}"));
    }
    pipelines::save(&renderer.into_renderer());
    let ms = |d: Duration| d.as_secs_f64() * 1000.0 / f64::from(n.max(1));
    println!(
        "a frame: GX stream {:.3} ms, renderer {:.3} ms, presenting (with the GPU's wait) {:.3} ms",
        ms(parts[0]),
        ms(parts[1]),
        ms(parts[2])
    );
    let pct = |v: &mut Vec<Duration>, p: f64| {
        v.sort();
        let i = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len().max(1)) - 1;
        v.get(i).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    };
    println!(
        "a frame's ticks and drawing: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms; with presenting: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        pct(&mut work, 0.5),
        pct(&mut work, 0.99),
        pct(&mut work, 1.0),
        pct(&mut presented, 0.5),
        pct(&mut presented, 0.99),
        pct(&mut presented, 1.0)
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
            eprintln!("{}", usage());
            return ExitCode::from(2);
        }
    };
    if let Some(path) = &args.launcher_shot {
        let gpu = present::Gpu::new();
        let mut form = launcher::Form::new(args.stocks, args.minutes, args.fullscreen, args.aspect);
        form.present = args.present;
        form.added_delay = args.added_delay;
        return match launcher::shot(&gpu, &mut form, args.size, std::path::Path::new(path)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{path}: {e}");
                ExitCode::FAILURE
            }
        };
    }
    sound::set_output(melee_audio::OutputOptions {
        exclusive: args.audio_exclusive,
    });
    present::set_present(args.present);
    if let Some(ticks) = args.bench {
        return bench(&args, ticks);
    }
    let session = if args.launch {
        None
    } else {
        match Session::start(&args) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
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
    if let Some(s) = &session {
        frames.space.load_dol(&s.dol);
    }
    frames.hsd.widescreen = args.aspect.widescreen();
    present::set_aspect(args.aspect.shown);
    let launcher = args.launch.then(|| {
        let mut form = launcher::Form::new(args.stocks, args.minutes, args.fullscreen, args.aspect);
        form.present = args.present;
        form.added_delay = args.added_delay;
        Launcher { form, window: None }
    });
    let mut app = App {
        session,
        launcher,
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
        scale: scale_for(args.scale, args.size, args.aspect),
        aspect: args.aspect,
        efb_width: args
            .aspect
            .efb_width(scale_for(args.scale, args.size, args.aspect)),
        one_thread: args.one_thread,
        msaa: args.msaa,
        window_size: args.size,
        alt: false,
        stopped: None,
        latency: None,
        delay: melee_input::Delay::new(args.added_delay),
        live: latency::Live::default(),
        overlay: None,
        args,
    };
    if let Some(path) = &app.args.latency_log {
        match latency::Log::create(path) {
            Ok(log) => app.latency = Some(log),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    }
    present::set_size(app.args.size);
    // The pipelines the last runs needed, made before the first tick.
    if app.session.is_some() {
        app.renderer = Some(gpu_for(
            pipelines::renderer(&app.gpu, app.scale, app.msaa, app.aspect),
            app.one_thread,
        ));
    }
    let r = event_loop.run_app(&mut app);
    app.inputs.close();
    app.drop_renderer();
    if let Some(mut log) = app.latency.take() {
        if let Some(s) = &app.shown {
            log.present_mode = format!("{:?}", s.present_mode());
        }
        log.added_delay = app.delay.frames();
        log.finish();
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
