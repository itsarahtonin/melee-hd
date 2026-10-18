//! melee-hd's player: a VS match in a window.
//!
//! ```text
//! melee-hd [--p1 fox] [--p2 marth] [--stage fd] [--stocks 4] [--minutes 8] [--disc ISO]
//!          [--replay FILE.slp] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread]
//!          [--no-adapter] [--mute] [--size WxH] [--fullscreen] [--disc-speed drive|fast] [--ticks N]
//! melee-hd --bench TICKS [--size WxH] [--resize-at TICK:WxH] [...]
//!                                               plays headless, timing each frame
//! melee-hd --launcher-shot FILE.png [--size WxH] draws the launcher to a PNG
//! latency: [--latency-log FILE.csv] [--latency-overlay] [--present mailbox|fifo|immediate]
//!          [--added-delay FRAMES] [--audio-exclusive] [--tick-timing timer|auto|refresh|vrr]
//!          [--jit-margin MS]
//! frames:  [--high-refresh off|interpolate|extrapolate] [--frames-per-tick N (with --bench)]
//! ```
//!
//! The disc comes from `--disc`, else `SSBM_DISC`, else the settings folder, where the disc
//! picker keeps the one it asks for the first time ([`settings`]). Characters and stages are melee-sim's
//! `game::roster`, what it has ported (`--help` lists them). With no `--p1` to `--p4`, `--stage`
//! or `--replay` (and no `--bench`), the window opens on the launcher, which picks the match
//! ([`launcher`]). Controllers: GameCube controllers on a Wii U adapter play their own ports;
//! gamepads and the keyboard play the first ports left (arrows: stick, IJKL: C-stick, X/Z/C/S:
//! A/B/X/Y, D: Z, Q/W: L/R, Enter: start, left shift: half stick). F11 or Alt+Enter toggles
//! borderless fullscreen; F12 saves a bug capture ([`capture`]). Escape quits, or from a match the launcher started, goes back to it.
//!
//! The disc reads at a drive's pace, as ssbm-rs's window reads it, which the music needs
//! (`melee_sim::audio::clock::DiscSpeed`); `--disc-speed fast` reads at Slippi's fast disc
//! speed, as the oracle's traces and the replay checks do.
//!
//! Frames are shown at the game's 73:60, pillarboxed; `--widescreen` (16:9) or `--aspect W:H`
//! widens the drawn cameras to a wider aspect, and `--aspect 4:3` stretches the picture to 4:3
//! ([`present::Aspect`]).
//!
//! The loop keeps latency as short as it can: each tick reads the controllers just before it
//! runs, and the frame it renders is drawn and presented at once, on a swap chain one frame deep
//! (mailbox by default, `--present` picks; [`present::Present`]). Ticks run on a fixed 60 Hz timer,
//! or, experimentally, just in time for the refresh that shows their frame ([`jit`],
//! `--tick-timing`), waited for precisely ([`timing`]). `--added-delay N` adds input
//! latency back on purpose, `--latency-log FILE` measures it all ([`latency`]), and F3 shows
//! the measures over the frame.
//!
//! `--high-refresh interpolate` draws frames at the display's rate between the last two ticks,
//! each placed by when it reaches the screen ([`pacing`]); `extrapolate` draws them between the
//! latest tick and a prediction of the next (`melee_render::predict`); off, the default, draws
//! a frame a tick.
//!
//! The release build on Windows opens no console window; started from a terminal it prints to
//! the terminal, and double-clicked it logs to the settings folder ([`console`]).

// No console window for the release build on Windows (`console` attaches to a terminal's).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod capture;
mod console;
mod jit;
mod latency;
mod launcher;
mod pacing;
mod pipelines;
#[cfg(test)]
mod predict_tests;
mod present;
#[cfg(test)]
mod roster_tests;
mod settings;
mod sound;
mod timing;

use std::collections::VecDeque;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use melee_data::disc::Disc;
use melee_input::{Inputs, Key};
use melee_render::blend::{Blend, Capture};
use melee_render::frame::Frames;
use melee_render::gpu::Gpu;
use melee_render::predict::{Cost, Predictor};
use melee_sim::audio::clock::DiscSpeed;
use melee_sim::game::roster;
use melee_sim::game::vs_match::{DiscFile, Match, Player, Settings};
use melee_slippi::Replay;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
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
    stocks: u8,
    minutes: u8,
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
    /// Mixes the sound but opens no audio device: nothing is heard.
    mute: bool,
    /// Starts with the latency overlay shown (F3 toggles it).
    latency_overlay: bool,
    /// How frames are drawn between ticks.
    high_refresh: pacing::Mode,
    /// How ticks are timed, and the just-in-time tick's margin over its costs.
    tick_timing: jit::Timing,
    jit_margin: Duration,
    /// With `--bench` and Interpolate or Extrapolate, frames drawn a tick (4 for a 240 Hz
    /// display).
    frames_per_tick: u32,
    bench: Option<u32>,
    size: (u32, u32),
    /// Quit after this many ticks.
    ticks: Option<u64>,
    /// The internal resolution, in multiples of the console's 640 by 480 (0: from the
    /// window's size).
    scale: u32,
    /// Saves the last frame the bench draws.
    shot: Option<String>,
    /// The bench's display changes size before these ticks, as a window resized or made
    /// fullscreen does.
    resize_at: Vec<(u32, (u32, u32))>,
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
    /// How fast the disc reads: a drive's pace unless `--disc-speed fast`.
    disc_speed: DiscSpeed,
    /// The launcher's choices the command line gave, which win over the last match's.
    given: launcher::Given,
}

/// The command line's usage, with the characters and stages melee-sim has ported.
fn usage() -> String {
    let names = |keys: Vec<&str>| keys.join("|");
    format!(
        "usage: melee-hd [--p1 NAME] [--p2 NAME] [--p3 NAME] [--p4 NAME] [--stage NAME] [--stocks N] [--minutes N] [--disc ISO] [--replay FILE] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread] [--no-adapter] [--mute] [--size WxH] [--fullscreen] [--disc-speed drive|fast] [--bench TICKS [--resize-at TICK:WxH]] [--launcher-shot FILE.png]\n\
         characters: {}\n\
         stages: {}\n\
         latency: [--latency-log FILE.csv] [--latency-overlay] [--present mailbox|fifo|immediate] [--added-delay FRAMES] [--audio-exclusive] [--tick-timing timer|auto|refresh|vrr] [--jit-margin MS]\n\
         frames: [--high-refresh off|interpolate|extrapolate] [--frames-per-tick N (with --bench)]\n\
         with no --p1..--p4, --stage or --replay, the window opens on a launcher that picks the match\n\
         the disc is --disc's, else SSBM_DISC's, else the one the disc picker asked for and remembered",
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
        mute: false,
        latency_overlay: false,
        high_refresh: pacing::Mode::Off,
        tick_timing: jit::Timing::Timer,
        jit_margin: jit::DEFAULT_MARGIN,
        frames_per_tick: 4,
        bench: None,
        size: (1280, 960),
        ticks: None,
        scale: 0,
        shot: None,
        resize_at: Vec::new(),
        aspect: present::Aspect::GAME,
        one_thread: false,
        msaa: 1,
        launch: false,
        launcher_shot: None,
        fullscreen: false,
        disc_speed: DiscSpeed::Drive,
        given: launcher::Given::default(),
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
            "--stocks" => {
                a.stocks = value()?.parse().map_err(|_| "--stocks N")?;
                a.given.stocks = true;
            }
            "--minutes" => {
                a.minutes = value()?.parse().map_err(|_| "--minutes N")?;
                a.given.minutes = true;
            }
            "--replay" => {
                a.replay = Some(value()?);
                chosen = true;
            }
            "--no-adapter" => a.adapter = false,
            "--latency-log" => a.latency_log = Some(value()?),
            "--present" => {
                a.present = present::Present::parse(&value()?)?;
                a.given.present = true;
            }
            "--added-delay" => {
                a.added_delay = value()?.parse().map_err(|_| "--added-delay FRAMES")?;
                a.given.added_delay = true;
            }
            "--audio-exclusive" => a.audio_exclusive = true,
            "--mute" => a.mute = true,
            "--latency-overlay" => a.latency_overlay = true,
            "--high-refresh" => {
                a.high_refresh = pacing::Mode::parse(&value()?)?;
                a.given.high_refresh = true;
            }
            "--tick-timing" => {
                a.tick_timing = jit::Timing::parse(&value()?)?;
                a.given.tick_timing = true;
            }
            "--jit-margin" => {
                let ms = launcher::parse_margin(&value()?).ok_or(format!(
                    "--jit-margin MS (0 to {})",
                    launcher::MAX_MARGIN_MS
                ))?;
                a.jit_margin = Duration::from_secs_f64(f64::from(ms) / 1000.0);
                a.given.jit_margin = true;
            }
            "--frames-per-tick" => {
                a.frames_per_tick = value()?
                    .parse()
                    .ok()
                    .filter(|&n| n >= 1)
                    .ok_or("--frames-per-tick N")?
            }
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
            "--resize-at" => {
                let v = value()?;
                let (tick, s) = v.split_once(':').ok_or("--resize-at TICK:WxH")?;
                let tick = tick.parse().map_err(|_| "--resize-at TICK:WxH")?;
                a.resize_at.push((tick, size(s)?));
            }
            "--launcher-shot" => a.launcher_shot = Some(value()?),
            "--fullscreen" => {
                a.fullscreen = true;
                a.given.fullscreen = true;
            }
            "--disc-speed" => {
                a.disc_speed = DiscSpeed::parse(&value()?).ok_or("--disc-speed drive|fast")?
            }
            "--widescreen" => {
                a.aspect = present::Aspect::wide(16.0 / 9.0);
                a.given.aspect = true;
            }
            "--aspect" => {
                a.aspect = present::Aspect::parse(&value()?)?;
                a.given.aspect = true;
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
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
        let disc = Disc::open(&a.disc).map_err(|e| format!("{}: {}", a.disc, settings::why(&e)))?;
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
                let mut s = Settings::vs(a.stage, &players, a.stocks, a.minutes);
                // A seed from the clock: presentation-side, set once before the match.
                s.seed = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.subsec_nanos());
                (s, None)
            }
        };
        // The replay's match too reads at the player's pace, as ssbm-rs's window plays one.
        let settings = Settings {
            disc: a.disc_speed,
            ..settings
        };
        Session::begin(&disc, &settings, replay, a.bench.is_none() && !a.mute)
    }

    /// Starts the match `settings` set from `disc`, with its sound mixed (and played when
    /// `play`), driven by `replay` if there is one.
    fn begin(
        disc: &Disc,
        settings: &Settings,
        replay: Option<Replay>,
        play: bool,
    ) -> Result<Session, String> {
        let mut load = |name: &str| DiscFile {
            entrynum: disc
                .entrynum(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
            bytes: disc
                .read_file(name)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
        };
        let (m, sound) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sound::start(settings, play, &mut load)
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

    /// Extrapolate's prediction of the next tick, on the predictor's copy of the match: with
    /// `pads` held (a replay's next frame for a replay). The session itself, its sound
    /// included, is only read.
    fn predict(
        &self,
        predictor: &mut Predictor,
        pads: [melee_sim::input::pad::PadStatus; 4],
    ) -> Result<Cost, String> {
        match &self.replay {
            Some(r) => predictor.predict(&self.m, |c| c.step_replay(&mut melee_slippi::Device(r))),
            None => predictor.predict(&self.m, |c| c.step(pads)),
        }
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
    /// Extrapolate's predictions and their time.
    predictions: u64,
    predicting: Duration,
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
            let predicting = if self.predictions > 0 {
                format!(
                    ", predicting a tick {:.2} ms average",
                    self.predicting.as_secs_f64() * 1000.0 / self.predictions as f64
                )
            } else {
                String::new()
            };
            eprintln!(
                "{:.2} ticks/s, {:.2} frames/s, tick+frame {:.2} ms average, {:.2} ms worst{predicting}",
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

/// A frame the GPU is drawing, until the GPU's timing thread says it finished.
struct InFlight {
    id: u64,
    /// When its tick started (Off), or the frame (Interpolate, Extrapolate).
    start: Instant,
    /// The refresh it's meant for.
    target: Option<Instant>,
    /// Drawn on the frames' own schedule (Interpolate, Extrapolate).
    paced: bool,
}

/// A latency log row, waiting for its frame's GPU time.
struct Row {
    id: Option<u64>,
    tick: latency::Tick,
    done: bool,
    queued: Instant,
}

/// How long a row waits for its frame's GPU time before it's written without it.
const ROW_WAIT: Duration = Duration::from_millis(500);

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
    /// Places the ticks on Melee's clock, just in time for their refreshes.
    jit: jit::Scheduler,
    /// How ticks are timed and its margin, how they're placed now, and whether frames wait for
    /// refreshes.
    timing: jit::Timing,
    margin: Duration,
    policy: Option<jit::Policy>,
    waits: bool,
    /// The refreshes when known, as last read.
    grid: Option<jit::Grid>,
    /// The window's monitor's refresh rate, which the refresh times must match.
    monitor_hz: Option<u32>,
    /// How early the event loop's wait ends for the precise wait.
    guard: timing::Guard,
    /// When the GPU finishes frames, and the frames it hasn't yet.
    gpu_done: Option<present::GpuDone>,
    inflight: VecDeque<InFlight>,
    /// With Interpolate or Extrapolate: a frame's lead before its refresh.
    frame_lead: jit::Lead,
    /// Latency log rows waiting for their frames' GPU times.
    rows: VecDeque<Row>,
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
    /// F12 was pressed: the next frame drawn is captured ([`capture`]).
    capture_next: bool,
    /// How frames are drawn between ticks, and with Interpolate or Extrapolate, when.
    mode: pacing::Mode,
    pacer: Option<pacing::Pacer>,
    /// With Interpolate or Extrapolate: the latest tick's frame and when the tick was due.
    latest: Option<(melee_sim::game::vs_match::Frame, Instant)>,
    /// With Interpolate: the tick before's transforms (A) and the latest's (B), kept for the
    /// next tick.
    before: Capture,
    after: Capture,
    /// With Extrapolate: the next tick, predicted.
    predictor: Predictor,
    /// With Extrapolate: a prediction stopped, which is said once.
    prediction_failed: bool,
    /// With Interpolate or Extrapolate: a tick's latency measures, waiting for the first frame
    /// after it.
    pending: Option<latency::Tick>,
}

impl App {
    fn ended(&self) -> bool {
        self.session.as_ref().is_none_or(|s| s.ended)
    }

    /// How ticks are placed now, with the display's refreshes where they're known; says so when
    /// it changes.
    fn sync(&mut self) -> (jit::Policy, Option<jit::Grid>) {
        let expected = self.monitor_hz.map(|m| pacing::refresh_period(Some(m)));
        let grid = timing::refresh_grid(expected);
        let immediate = self
            .shown
            .as_ref()
            .is_some_and(|s| s.present_mode() == wgpu::PresentMode::Immediate);
        let tick = self.jit.tick();
        let mut policy = self.timing.policy(grid, immediate, tick);
        // The automatic choice aligns only where it pays: when the lead is shorter than the
        // median cost and the half refresh a frame waits on average.
        let lead = if self.mode.paced() {
            &self.frame_lead
        } else {
            &self.jit.lead
        };
        if let (jit::Timing::Auto, jit::Policy::Aligned, Some(g)) = (self.timing, policy, grid)
            && !lead.pays(g.period, self.policy == Some(jit::Policy::Aligned))
        {
            policy = jit::Policy::Cadence;
        }
        // Frames wait for a refresh unless they're presented at once or the display waits for
        // them (variable refresh, which the automatic choice takes a display to be when its rate
        // isn't a multiple of the ticks').
        self.waits = grid.is_some_and(|g| {
            !immediate
                && match self.timing {
                    jit::Timing::Vrr => false,
                    jit::Timing::Auto => jit::multiple(g.period, tick),
                    jit::Timing::Refresh | jit::Timing::Timer => true,
                }
        });
        self.grid = grid;
        if self.policy != Some(policy) {
            let at = grid.map_or(String::new(), |g| {
                format!(
                    ", refreshes every {:.3} ms",
                    g.period.as_secs_f64() * 1000.0
                )
            });
            let costs = lead.median().map_or(String::new(), |m| {
                format!(
                    " (lead {:.2} ms, median cost {:.2} ms)",
                    lead.get().as_secs_f64() * 1000.0,
                    m.as_secs_f64() * 1000.0
                )
            });
            eprintln!("tick timing: {}{at}{costs}", policy.name());
            self.policy = Some(policy);
        }
        (policy, grid)
    }

    /// Plans the next tick: just in time for its refresh, or with Interpolate and Extrapolate
    /// for the first frame that will show it.
    fn plan_tick(&mut self, now: Instant) -> jit::Plan {
        let (policy, grid) = self.sync();
        match &self.pacer {
            Some(p) if self.mode.paced() => {
                let period = match (policy, grid) {
                    (jit::Policy::Aligned, Some(g)) => g.period,
                    _ => p.refresh(),
                };
                self.jit.plan_before_frames(now, policy, p.due(), period)
            }
            _ => self.jit.plan(now, policy, grid),
        }
    }

    /// Runs the ticks that are due: controllers read, tick, and with Off its frame drawn and
    /// presented. Returns when the next tick starts.
    fn run_due(&mut self) -> Instant {
        loop {
            self.collect_done();
            let now = Instant::now();
            let plan = self.plan_tick(now);
            if self.ended() || now < plan.start || !self.run_tick(plan) {
                return plan.start;
            }
        }
    }

    /// Runs a tick as planned; false when the match stopped.
    fn run_tick(&mut self, plan: jit::Plan) -> bool {
        let start = Instant::now();
        let pads = self.delay.apply(self.inputs.read());
        let session = self.session.as_mut().expect("a match");
        let f = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.tick(pads)))
        {
            Ok(f) => f,
            Err(p) => {
                let text = format!("the match stopped: {}", panic_text(&p));
                eprintln!("{text}");
                session.ended = true;
                self.stopped = Some(text);
                return false;
            }
        };
        let simulated = start.elapsed();
        let audio = session.sound.as_ref().and_then(sound::Sound::stats);
        self.jit.ran(&plan);
        self.stats.ticks += 1;
        self.ticks += 1;
        if self.max_ticks.is_some_and(|n| self.ticks >= n) {
            session.ended = true;
        }
        let mut frame = None;
        let mut id = None;
        if self.mode.paced() {
            let rendered = !f.draws.is_empty();
            if rendered {
                if self.mode == pacing::Mode::Interpolate {
                    // B is this tick; A, the tick before, whose transforms were kept then.
                    std::mem::swap(&mut self.before, &mut self.after);
                    let m = &self.session.as_ref().expect("a match").m;
                    self.after.fill(&m.game, &f.draws);
                }
                if let Some((old, _)) = self.latest.replace((f, plan.due)) {
                    self.session.as_mut().expect("a match").m.recycle(old);
                }
            } else {
                self.session.as_mut().expect("a match").m.recycle(f);
            }
            if self.mode == pacing::Mode::Extrapolate {
                self.predict(rendered, pads);
            }
            self.jit.tick_done(start, Instant::now(), plan.target);
        } else {
            if !f.draws.is_empty() && plan.render {
                if let Some((mut times, shown)) = self.draw_frame(&f.draws, start, None) {
                    times.target = plan.target.map(|t| t.saturating_duration_since(start));
                    id = Some(self.watch(shown.submission, start, plan.target, false));
                    frame = Some(times);
                }
                self.stats.frame(start.elapsed());
            }
            self.session.as_mut().expect("a match").m.recycle(f);
        }
        let t = latency::Tick {
            at: Some(start),
            late: start.saturating_duration_since(plan.start),
            ages: self.inputs.ages,
            adapter_reports: self.inputs.adapter.as_ref().map(|a| a.reports()),
            sim: simulated,
            frame,
            audio,
        };
        if self.mode.paced() {
            // Logged with the first frame drawn after it.
            if let Some(p) = self.pending.replace(t) {
                self.queue_row(None, p);
            }
        } else {
            self.queue_row(id, t);
        }
        true
    }

    /// Has the GPU's timing thread note when the frame presented with `submission` is done: a
    /// frame started at `start` (its tick's start with Off) and meant for `target`.
    fn watch(
        &mut self,
        submission: wgpu::SubmissionIndex,
        start: Instant,
        target: Option<Instant>,
        paced: bool,
    ) -> u64 {
        let device = &self.gpu.device;
        let g = self
            .gpu_done
            .get_or_insert_with(|| present::GpuDone::new(device));
        let id = g.watch(submission);
        if self.inflight.len() >= 64 {
            self.inflight.pop_front();
        }
        self.inflight.push_back(InFlight {
            id,
            start,
            target,
            paced,
        });
        id
    }

    /// Takes in the frames the GPU finished: their costs and missed refreshes, and their log
    /// rows' times; writes the rows that are complete.
    fn collect_done(&mut self) {
        let done = self
            .gpu_done
            .as_ref()
            .map_or_else(Vec::new, present::GpuDone::finished);
        for (id, at) in done {
            let Some(i) = self.inflight.iter().position(|f| f.id == id) else {
                continue;
            };
            // Frames before it that never reported won't now.
            let f = self.inflight.drain(..=i).next_back().expect("the frame");
            let shown = match (self.waits, self.grid, f.target) {
                (true, Some(g), Some(t)) => jit::shown_at(g, t, at),
                (true, Some(g), None) => g.at_or_after(at + jit::LATCH),
                _ => at,
            };
            let missed = if f.paced {
                self.frame_lead.add(at.saturating_duration_since(f.start));
                f.target.map(|t| {
                    let missed = self
                        .grid
                        .map_or(at + jit::LATCH > t, |g| jit::shown_at(g, t, at) > t);
                    self.jit.counts.targeted += 1;
                    if missed {
                        self.jit.counts.missed += 1;
                        self.frame_lead.missed();
                    } else {
                        self.frame_lead.met();
                    }
                    missed
                })
            } else {
                self.jit.ready(f.start, f.target, at, self.grid)
            };
            if let Some(r) = self.rows.iter_mut().find(|r| r.id == Some(id)) {
                let from = r.tick.at.unwrap_or(f.start);
                if let Some(t) = &mut r.tick.frame {
                    t.ready = Some(at.saturating_duration_since(from));
                    t.shown = Some(shown.saturating_duration_since(from));
                    t.missed = missed;
                }
                r.done = true;
            }
        }
        let now = Instant::now();
        while let Some(r) = self.rows.front() {
            if !(r.id.is_none() || r.done || now - r.queued > ROW_WAIT) {
                break;
            }
            let r = self.rows.pop_front().expect("a row");
            self.log_tick(r.tick);
        }
    }

    /// Queues a tick's row, to be written once its frame's GPU time is in.
    fn queue_row(&mut self, id: Option<u64>, tick: latency::Tick) {
        self.rows.push_back(Row {
            id,
            tick,
            done: false,
            queued: Instant::now(),
        });
    }

    /// With Extrapolate, after a tick that ran with `pads`: the next tick predicted with the
    /// controllers it will see if they're known (an added delay has read them), else `pads`
    /// held. After a tick without a display pass the frames draw the latest as it is.
    fn predict(&mut self, rendered: bool, pads: [melee_sim::input::pad::PadStatus; 4]) {
        if !rendered {
            self.predictor.forget();
            return;
        }
        let next = self.delay.next().unwrap_or(pads);
        let session = self.session.as_ref().expect("a match");
        match session.predict(&mut self.predictor, next) {
            Ok(cost) => {
                self.stats.predictions += 1;
                self.stats.predicting += cost.total();
            }
            Err(why) if !self.prediction_failed => {
                eprintln!("the next tick's prediction stopped (frames draw the latest): {why}");
                self.prediction_failed = true;
            }
            Err(_) => {}
        }
    }

    fn log_tick(&mut self, t: latency::Tick) {
        self.live.tick(t);
        if let Some(log) = &mut self.latency {
            log.tick(t);
        }
    }

    /// With Interpolate or Extrapolate, draws the frame that's due, if one is: between the last
    /// two ticks, or the latest and the next predicted, at the fraction its display time gives
    /// ([`pacing`]). Aligned to the refreshes, each frame starts its lead before the refresh
    /// it's meant for. Returns when the next frame is due.
    fn draw_due(&mut self) -> Option<Instant> {
        let aligned = self
            .grid
            .filter(|_| self.policy == Some(jit::Policy::Aligned));
        let lead = self.frame_lead.get();
        let now = Instant::now();
        let (Some(pacer), Some((_, due))) = (&mut self.pacer, &self.latest) else {
            return None;
        };
        if aligned.is_some() != pacer.target().is_some() {
            pacer.align(now, aligned, lead);
        }
        if now < pacer.due() {
            return Some(pacer.due());
        }
        let target = pacer.target();
        let t = pacer.start(now, *due);
        let (f, due) = self.latest.take().expect("a frame");
        let drawn = if self.mode == pacing::Mode::Extrapolate {
            let predictor = std::mem::take(&mut self.predictor);
            let drawn = self.draw_frame(&f.draws, now, Some(predictor.blend(t)));
            self.predictor = predictor;
            drawn
        } else {
            let before = std::mem::take(&mut self.before);
            let blend = Blend { from: &before, t };
            let drawn = self.draw_frame(&f.draws, now, Some(blend));
            self.before = before;
            drawn
        };
        self.latest = Some((f, due));
        self.stats.frame(now.elapsed());
        if let Some((mut times, shown)) = drawn {
            if let Some(p) = &mut self.pacer {
                p.presented(now, times.acquire, shown.acquired, shown.presented);
            }
            times.target = target.map(|t| t.saturating_duration_since(now));
            let id = self.watch(shown.submission, now, target, true);
            if let Some(mut p) = self.pending.take() {
                // Timed from the tick's start, so the row measures from its controller read.
                let since =
                    p.at.map_or(Duration::ZERO, |at| now.saturating_duration_since(at));
                times.rendered += since;
                times.presented += since;
                times.target = times.target.map(|t| t + since);
                p.frame = Some(times);
                self.queue_row(Some(id), p);
            }
        }
        let pacer = self.pacer.as_mut()?;
        pacer.align(Instant::now(), aligned, self.frame_lead.get());
        Some(pacer.due())
    }

    /// The display's refresh period and whether frames wait for a refresh, for the pacer; the
    /// monitor's rate is kept for the refresh times.
    fn display_refresh(&mut self) -> (Duration, bool) {
        let Some(s) = &self.shown else {
            return (pacing::refresh_period(None), false);
        };
        self.monitor_hz = s
            .window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz());
        (
            pacing::refresh_period(self.monitor_hz),
            s.present_mode() == wgpu::PresentMode::Fifo,
        )
    }

    /// Starts Melee's clock at the next tick, now, with the timing and margin chosen.
    fn start_clock(&mut self) {
        self.display_refresh();
        self.jit = jit::Scheduler::new(
            Duration::from_secs_f64(1.0 / TICK_RATE),
            self.margin,
            Instant::now(),
        );
        self.frame_lead = jit::Lead::new(self.margin, self.jit.tick() / 2);
        self.policy = None;
        self.inflight.clear();
    }

    /// Starts drawing frames as `mode` says, from the next tick.
    fn start_pacing(&mut self, mode: pacing::Mode) {
        self.mode = mode;
        self.latest = None;
        self.pending = None;
        self.before.clear();
        self.after.clear();
        self.predictor.clear();
        self.prediction_failed = false;
        let (refresh, fifo) = self.display_refresh();
        self.pacer = mode.paced().then(|| {
            eprintln!(
                "high refresh: {}, display at {:.2} Hz",
                mode.key(),
                1.0 / refresh.as_secs_f64()
            );
            pacing::Pacer::new(
                Duration::from_secs_f64(1.0 / TICK_RATE),
                refresh,
                fifo,
                Instant::now(),
            )
        });
    }

    /// The display may have changed its rate (another monitor, fullscreen).
    fn update_refresh(&mut self) {
        let (refresh, fifo) = self.display_refresh();
        if let Some(p) = &mut self.pacer {
            if p.refresh() != refresh {
                eprintln!("display at {:.2} Hz", 1.0 / refresh.as_secs_f64());
            }
            p.set_refresh(refresh, fifo);
        }
    }

    /// Draws and presents a frame, between two ticks with a blend: when its stages finished,
    /// from `start` (its tick's start, or the frame's), and how it was presented.
    fn draw_frame(
        &mut self,
        draws: &[melee_sim::hsd::display::Draw],
        start: Instant,
        blend: Option<Blend<'_>>,
    ) -> Option<(latency::FrameTimes, present::Presented)> {
        // The internal resolution follows the window's size unless `--scale` fixed it, and
        // the EFB's width the aspect: the renderer's EFB is made again at a new one, in place.
        let scale = scale_for(self.args.scale, self.window_size, self.aspect);
        let width = self.aspect.efb_width(scale);
        if (scale, width) != (self.scale, self.efb_width) {
            eprintln!("internal resolution: {scale}x ({width}x{})", 480 * scale);
            self.scale = scale;
            self.efb_width = width;
            if let Some(g) = &mut self.renderer {
                g.resize_efb(scale, width);
            }
        }
        let m = &self.session.as_ref().expect("a match").m;
        let fifo = self.frames.frame_blended(&m.game, &m.data, draws, blend);
        let gpu = self.renderer.get_or_insert_with(|| {
            gpu_for(
                pipelines::renderer(&self.gpu, self.scale, self.msaa, self.aspect),
                self.one_thread,
            )
        });
        let before = self.capture_next.then(|| capture::Regs::of(&gpu.state));
        let images = gpu.run(&self.frames.space, &m.data, &fifo, None);
        let after = before.as_ref().map(|_| capture::Regs::of(&gpu.state));
        if std::mem::take(&mut self.capture_next)
            && let (Some(before), Some(after)) = (before, after)
            && let (Some(tex), Some(s)) = (
                images.last().and_then(|f| f.texture.as_ref()),
                &self.session,
            )
        {
            let about = capture::About {
                mode: self.mode.key(),
                tick: self.ticks,
                scale: self.scale,
                msaa: self.msaa,
                aspect: self.aspect,
                widescreen: self.frames.hsd.widescreen,
                notes: format!("{:?}", self.frames.hsd.notes),
            };
            let live = capture::Live {
                before,
                after,
                fifo: &fifo,
                space: &self.frames.space,
                hsd: format!("{:?}", self.frames.hsd.state),
            };
            match capture::save(&self.gpu, tex, &s.m, &s.dol, draws, blend, &about, &live) {
                Ok(dir) => eprintln!("captured to {}", dir.display()),
                Err(e) => eprintln!("the capture failed: {e}"),
            }
        }
        let rendered = start.elapsed();
        let (shown, frame) = (self.shown.as_mut()?, images.into_iter().last()?);
        let text = self.overlay.is_some().then(|| {
            self.live
                .text(&format!("{:?}", shown.present_mode()), self.delay.frames())
        });
        let (gpu, overlay) = (&self.gpu, &mut self.overlay);
        let presented = shown.present(gpu, &frame, |view, w, h| {
            if let (Some(p), Some(text)) = (overlay, text) {
                latency::overlay(p, gpu, &text, view, w, h);
            }
        })?;
        Some((
            latency::FrameTimes {
                rendered,
                acquire: presented.acquire,
                presented: presented.presented.saturating_duration_since(start),
                ..Default::default()
            },
            presented,
        ))
    }

    /// Saves the renderer's pipelines and lets it go, with the match.
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
        let high_refresh = l.form.high_refresh;
        let (tick_timing, margin) = (l.form.tick_timing, l.form.jit_margin);
        settings::write(settings::LAST_MATCH, &l.form.save());
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
                self.timing = tick_timing;
                self.margin = Duration::from_secs_f64(f64::from(margin) / 1000.0);
                self.start_clock();
                self.start_pacing(high_refresh);
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
        self.latest = None;
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
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        // How late the event loop's timed wake-ups come, for the precise wait's guard.
        if let StartCause::ResumeTimeReached {
            start,
            requested_resume,
        } = cause
        {
            self.guard.woke(requested_resume, start);
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_none() {
            let shown = present::Shown::new(&self.gpu, event_loop, self.args.fullscreen);
            if let Some(l) = &mut self.launcher {
                l.window = Some(launcher::Window::new(&self.gpu, &shown));
            }
            self.shown = Some(shown);
            self.start_clock();
            if self.args.latency_overlay {
                self.toggle_overlay();
            }
            if self.session.is_some() {
                self.start_pacing(self.args.high_refresh);
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
        // F12 captures the next frame drawn, for a bug report.
        if let WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(KeyCode::F12),
                    state: ElementState::Pressed,
                    repeat: false,
                    ..
                },
            ..
        } = &event
        {
            if self.session.is_some() {
                self.capture_next = true;
            }
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
            WindowEvent::Resized(size) => {
                self.resized(size.width, size.height);
                self.update_refresh();
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => self.update_refresh(),
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
            // Ticks and frames that are due run; the event loop sleeps until shortly before the
            // next (it wakes for window events too), and a precise wait takes it the rest of the
            // way. The fixed timer only sleeps.
            for _ in 0..32 {
                let next_tick = self.run_due();
                if self.ended() {
                    break;
                }
                let next = self.draw_due().map_or(next_tick, |f| f.min(next_tick));
                if self.policy == Some(jit::Policy::Timer) {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next));
                    return;
                }
                let guard = self.guard.get();
                if next > Instant::now() + guard {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(next - guard));
                    return;
                }
                timing::sleep_until(next);
            }
            if !self.ended() {
                event_loop.set_control_flow(ControlFlow::Poll);
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
    if args.high_refresh.paced() {
        return bench_paced(args, ticks);
    }
    let mut session = match Session::start(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let gpu = present::Gpu::new();
    let mut target = present::Offscreen::new(&gpu, args.size.0, args.size.1);
    let mut size = args.size;
    let mut scale = scale_for(args.scale, size, args.aspect);
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
    // The last resize's tick and how long the renderer took to follow it, until the frame
    // after is drawn.
    let mut resized: Option<(u32, Duration)> = None;
    // The last frame's tick and drawing with its presenting, and the resize whose frame was
    // the frame before, to say what the frame after it took.
    let mut previous = Duration::ZERO;
    let mut after_resize: Option<u32> = None;
    for tick in 0..ticks {
        // A resize: the display's new size at once, the renderer's at the frame after, as the
        // window's.
        for &(_, to) in args.resize_at.iter().filter(|(at, _)| *at == tick) {
            size = to;
            target = present::Offscreen::new(&gpu, size.0, size.1);
            let s = scale_for(args.scale, size, args.aspect);
            if s != scale {
                scale = s;
                eprintln!(
                    "tick {tick}: {}x{}, internal resolution {scale}x",
                    size.0, size.1
                );
                let t = Instant::now();
                renderer.resize_efb(scale, args.aspect.efb_width(scale));
                resized = Some((tick, t.elapsed()));
            }
        }
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
            let ms = |d: Duration| d.as_secs_f64() * 1000.0;
            if let Some(at) = after_resize.take() {
                eprintln!(
                    "tick {at}: the frame after the resize's took {:.3} ms",
                    ms(t2 - t0)
                );
            }
            if let Some((at, took)) = resized.take() {
                eprintln!(
                    "tick {at}: the resize's frame took {:.3} ms (the frame before {:.3} ms): the renderer following it {:.3} ms, then the tick {:.3} ms, the GX stream {:.3} ms, the renderer {:.3} ms, presenting {:.3} ms",
                    ms(took + (t2 - t0)),
                    ms(previous),
                    ms(took),
                    ms(t1 - t0),
                    ms(ta - t1),
                    ms(tb - ta),
                    ms(t2 - tb)
                );
                after_resize = Some(at);
            }
            previous = t2 - t0;
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
        size.0,
        size.1,
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

/// `--bench` with Interpolate or Extrapolate: plays `ticks` ticks headless and after each draws
/// `--frames-per-tick` frames between it and the tick before (Interpolate) or the next tick
/// predicted (Extrapolate), at t = 0, 1/N ... (N-1)/N, as a display N times faster than the
/// ticks shows them, and reports what keeping a tick's transforms (and predicting the next) and
/// drawing a frame take.
fn bench_paced(args: &Args, ticks: u32) -> ExitCode {
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
    let mut frames = Frames::new();
    frames.space.load_dol(&session.dol);
    frames.hsd.widescreen = args.aspect.widescreen();
    present::set_aspect(args.aspect.shown);
    let pads = [melee_sim::input::pad::PadStatus::default(); 4];
    let per_tick = args.frames_per_tick;
    let extrapolate = args.high_refresh == pacing::Mode::Extrapolate;
    let (mut before, mut after) = (Capture::default(), Capture::default());
    let mut predictor = Predictor::default();
    // Each prediction's parts, and a tick with its prediction.
    let mut costs: Vec<Cost> = Vec::with_capacity(ticks as usize);
    let mut ticked: Vec<Duration> = Vec::with_capacity(ticks as usize);
    let (mut sim, mut kept) = (Duration::ZERO, Duration::ZERO);
    let mut parts = [Duration::ZERO; 3];
    let mut drawing: Vec<Duration> = Vec::with_capacity((ticks * per_tick) as usize);
    let mut presented: Vec<Duration> = Vec::with_capacity((ticks * per_tick) as usize);
    let mut last = None;
    let mut rendered = 0u32;
    for _ in 0..ticks {
        let t0 = Instant::now();
        let f = session.tick(pads);
        let t1 = Instant::now();
        sim += t1 - t0;
        if !f.draws.is_empty() {
            if extrapolate {
                match session.predict(&mut predictor, pads) {
                    Ok(cost) => {
                        costs.push(cost);
                        ticked.push(t1 - t0 + cost.total());
                    }
                    Err(why) => eprintln!("the prediction stopped: {why}"),
                }
            } else {
                std::mem::swap(&mut before, &mut after);
                after.fill(&session.m.game, &f.draws);
            }
            let m = &session.m;
            let t2 = Instant::now();
            kept += t2 - t1;
            rendered += 1;
            for k in 0..per_tick {
                let t = k as f32 / per_tick as f32;
                let blend = if extrapolate {
                    predictor.blend(t)
                } else {
                    Blend { from: &before, t }
                };
                let ta = Instant::now();
                let fifo = frames.frame_blended(&m.game, &m.data, &f.draws, Some(blend));
                let tb = Instant::now();
                let images = renderer.run(&frames.space, &m.data, &fifo, None);
                let tc = Instant::now();
                if let Some(frame) = images.into_iter().last() {
                    target.present(&gpu, &frame);
                    last = Some(frame);
                }
                let td = Instant::now();
                parts[0] += tb - ta;
                parts[1] += tc - tb;
                parts[2] += td - tc;
                drawing.push(tc - ta);
                presented.push(td - ta);
            }
        }
        session.m.recycle(f);
        if session.ended {
            break;
        }
    }
    let n = drawing.len();
    let ms = |d: Duration, k: usize| d.as_secs_f64() * 1000.0 / k.max(1) as f64;
    println!(
        "{n} frames, {per_tick} a tick, at {}x{} (internal {scale}x, EFB {}x{}): simulation {:.3} ms a tick, {} {:.3} ms a tick",
        args.size.0,
        args.size.1,
        args.aspect.efb_width(scale),
        480 * scale,
        ms(sim, ticks as usize),
        if extrapolate {
            "predicting the next"
        } else {
            "keeping its transforms"
        },
        ms(kept, rendered as usize)
    );
    println!(
        "a frame: GX stream (with the blend) {:.3} ms, renderer {:.3} ms, presenting (with the GPU's wait) {:.3} ms",
        ms(parts[0], n),
        ms(parts[1], n),
        ms(parts[2], n)
    );
    let pct = |v: &mut Vec<Duration>, p: f64| {
        v.sort();
        let i = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len().max(1)) - 1;
        v.get(i).map_or(0.0, |d| d.as_secs_f64() * 1000.0)
    };
    println!(
        "a frame's drawing: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms; with presenting: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        pct(&mut drawing, 0.5),
        pct(&mut drawing, 0.99),
        pct(&mut drawing, 1.0),
        pct(&mut presented, 0.5),
        pct(&mut presented, 0.99),
        pct(&mut presented, 1.0)
    );
    if extrapolate {
        let k = costs.len();
        let sum = |f: fn(&Cost) -> Duration| costs.iter().map(f).sum::<Duration>();
        let mut total: Vec<Duration> = costs.iter().map(Cost::total).collect();
        println!(
            "predicting a tick ({k}): restoring the copy {:.3} ms, the tick {:.3} ms, keeping its transforms {:.3} ms; in all p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
            ms(sum(|c| c.clone), k),
            ms(sum(|c| c.tick), k),
            ms(sum(|c| c.capture), k),
            pct(&mut total, 0.5),
            pct(&mut total, 0.99),
            pct(&mut total, 1.0)
        );
        println!(
            "a tick with its prediction: p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
            pct(&mut ticked, 0.5),
            pct(&mut ticked, 0.99),
            pct(&mut ticked, 1.0)
        );
    }
    if let (Some(path), Some(f)) = (&args.shot, &last) {
        f.save_png(std::path::Path::new(path))
            .unwrap_or_else(|e| eprintln!("{path}: {e}"));
    }
    pipelines::save(&renderer.into_renderer());
    ExitCode::SUCCESS
}

fn run(args: Args) -> ExitCode {
    if let Some(path) = &args.launcher_shot {
        let gpu = present::Gpu::new();
        let mut form = launcher::Form::new(args.stocks, args.minutes, args.fullscreen, args.aspect);
        form.present = args.present;
        form.added_delay = args.added_delay;
        form.high_refresh = args.high_refresh;
        return match launcher::shot(&gpu, &mut form, args.size, std::path::Path::new(path)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                console::fatal(&format!("{path}: {e}"));
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
                console::fatal(&e);
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
        form.high_refresh = args.high_refresh;
        form.tick_timing = args.tick_timing;
        form.jit_margin = (args.jit_margin.as_secs_f64() * 1000.0) as f32;
        if let Some(text) = settings::read(settings::LAST_MATCH) {
            form.restore(&text, &args.given);
        }
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
        jit: jit::Scheduler::new(
            Duration::from_secs_f64(1.0 / TICK_RATE),
            args.jit_margin,
            Instant::now(),
        ),
        timing: args.tick_timing,
        margin: args.jit_margin,
        policy: None,
        waits: false,
        grid: None,
        monitor_hz: None,
        guard: timing::Guard::default(),
        gpu_done: None,
        inflight: VecDeque::new(),
        frame_lead: jit::Lead::new(args.jit_margin, Duration::from_secs_f64(0.5 / TICK_RATE)),
        rows: VecDeque::new(),
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
        capture_next: false,
        mode: pacing::Mode::Off,
        pacer: None,
        latest: None,
        before: Capture::default(),
        after: Capture::default(),
        predictor: Predictor::default(),
        prediction_failed: false,
        pending: None,
        args,
    };
    if let Some(path) = &app.args.latency_log {
        match latency::Log::create(path) {
            Ok(log) => app.latency = Some(log),
            Err(e) => {
                console::fatal(&e);
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
    // The rows still waiting for their frames' GPU times.
    app.collect_done();
    for r in std::mem::take(&mut app.rows) {
        app.log_tick(r.tick);
    }
    if let Some(mut log) = app.latency.take() {
        if let Some(s) = &app.shown {
            log.present_mode = format!("{:?}", s.present_mode());
        }
        log.added_delay = app.delay.frames();
        log.timing = format!(
            "tick timing {}: {}{}, margin {:.2} ms at the end",
            app.timing.key(),
            app.policy.map_or("no ticks", jit::Policy::name),
            app.grid.map_or(String::new(), |g| format!(
                ", refreshes every {:.3} ms",
                g.period.as_secs_f64() * 1000.0
            )),
            if app.mode.paced() {
                app.frame_lead.margin()
            } else {
                app.jit.lead.margin()
            }
            .as_secs_f64()
                * 1000.0
        );
        log.counts = app.jit.counts;
        log.finish();
    }
    if let Err(e) = r {
        console::fatal(&format!("the window failed: {e}"));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    console::attach();
    let mut args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            let usage = usage();
            console::fatal(&if e.is_empty() {
                usage
            } else {
                format!("{e}\n{usage}")
            });
            return ExitCode::from(2);
        }
    };
    // The disc picker on the main thread, where macOS wants its dialogs.
    if args.disc.is_empty() && args.launcher_shot.is_none() {
        match settings::disc() {
            Some(disc) => args.disc = disc,
            None => return ExitCode::SUCCESS,
        }
    }
    // The simulation recurses as the original does (joint trees): a deep stack. The window
    // must live on the main thread on some systems, so the main thread gets the stack by
    // running everything on a thread that is itself the event loop's only user.
    let child = std::thread::Builder::new()
        .stack_size(512 << 20)
        .spawn(move || run(args))
        .expect("the player's thread");
    child.join().unwrap_or_else(|p| {
        console::fatal(&format!("melee-hd stopped: {}", panic_text(&p)));
        ExitCode::FAILURE
    })
}
