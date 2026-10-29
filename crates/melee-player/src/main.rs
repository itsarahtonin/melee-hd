//! melee-hd's player: Melee from power-on in a window, or a VS match.
//!
//! ```text
//! melee-hd [--saves DIR] [--no-session-record]     boots Melee from power-on with the native save
//! melee-hd [--p1 fox] [--p2 marth] [--stage fd] [--stocks 4] [--minutes 8] [--disc ISO]
//!          [--replay FILE.slp] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread]
//!          [--no-adapter] [--mute] [--size WxH] [--fullscreen] [--disc-speed drive|fast] [--ticks N]
//! melee-hd --bench TICKS [--size WxH] [--resize-at TICK:WxH] [...]
//!                                               plays headless, timing each frame
//! melee-hd --settings-shot FILE.png [--size WxH] draws the settings panel to a PNG
//! latency: [--latency-log FILE.csv] [--latency-overlay] [--present mailbox|fifo|immediate]
//!          [--added-delay FRAMES] [--audio-exclusive] [--tick-timing timer|auto|refresh|vrr]
//!          [--jit-margin MS]
//! frames:  [--high-refresh off|interpolate|extrapolate] [--frames-per-tick N (with --bench)]
//! debug:   [--flicker-scan]
//! perf:    [--perf-report FILE] [--refresh HZ] [--seed N]
//! ```
//!
//! The disc comes from `--disc`, else `SSBM_DISC`, else the settings folder, where the disc
//! picker keeps the one it asks for the first time ([`settings`]). Characters and stages are melee-sim's
//! `game::roster`, what it has ported (`--help` lists them); a match the command line names is a
//! developer's. With no `--p1` to `--p4`, `--stage`, `--replay` or `--inputs`, Melee boots from
//! power-on with the native save ([`saves`]), its menus gated where they lead into unported
//! code ([`gates`]), each session recorded ([`session_rec`]); Esc opens the settings ([`front`]).
//! Controllers: GameCube controllers on a Wii U adapter play their own ports;
//! gamepads and the keyboard play the first ports left (arrows: stick, IJKL: C-stick, X/Z/C/S:
//! A/B/X/Y, D: Z, Q/W: L/R, Enter: start, left shift: half stick). F11 or Alt+Enter toggles
//! borderless fullscreen; F12 saves a bug capture ([`capture`]), and `--flicker-scan` saves one by
//! itself when a presented frame drops out for one frame ([`flicker`]). Escape opens the settings
//! over Melee, or quits a match the command line named.
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
//! the measures over the frame, with each frame's GPU time by stage.
//!
//! `--perf-report FILE` measures a run in the window as it plays ([`perf`]): with a replay, or a
//! match with a fixed `--seed` and no one at the controllers, and `--ticks`, a repeatable
//! benchmark. `--refresh HZ` draws Interpolate's and Extrapolate's frames at that
//! rate instead of the display's.
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
mod channel;
mod console;
#[cfg(test)]
mod cosmetic_tests;
mod dialogs;
mod flicker;
mod front;
mod gates;
mod holds;
mod jit;
mod latency;
mod paced;
#[cfg(test)]
mod paced_tests;
mod pacing;
mod perf;
mod pipelines;
mod power;
#[cfg(test)]
mod power_tests;
#[cfg(test)]
mod predict_tests;
mod present;
#[cfg(test)]
mod record_tests;
mod recording;
#[cfg(test)]
mod roster_tests;
mod saves;
mod session_rec;
mod settings;
mod sound;
mod timing;
mod ui;

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
use melee_sim::game::gm::vs_mode::{GamePrefs, GameRules, Special};
use melee_sim::game::roster;
use melee_sim::game::special_audit;
use melee_sim::game::vs_match::{DiscFile, Settings};
use melee_slippi::Replay;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

/// Ticks a second: the engine's pad-polling rate.
const TICK_RATE: f64 = 60.0;

/// The stack the game runs on: the simulation recurses as the original does (joint trees).
const GAME_STACK: usize = 512 << 20;

/// Runs a test's body on the game's stack ([`GAME_STACK`]), as every test that starts a
/// [`Session`] does: a test thread's is the standard library's 2 MiB.
#[cfg(test)]
fn on_game_stack(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(GAME_STACK)
        .spawn(body)
        .expect("the test's thread")
        .join()
        .unwrap_or_else(|p| std::panic::resume_unwind(p));
}

/// The engine tick's length: a 60th of a second, or shorter in a game sped up (Lightning
/// Melee's, whose tick the game's clock sets at its speed, `lb_80019628`).
fn engine_tick(game: &melee_sim::game::state::Game) -> Duration {
    game.pacing.short_tick_nanos().map_or(
        Duration::from_secs_f64(1.0 / TICK_RATE),
        Duration::from_nanos,
    )
}

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
    /// The console from power-on with the settings panel (no match on the command line).
    front: bool,
    /// Draws the settings panel to this PNG and quits.
    settings_shot: Option<String>,
    /// Borderless fullscreen on the window's monitor.
    fullscreen: bool,
    /// How fast the disc reads: a drive's pace unless `--disc-speed fast`.
    disc_speed: DiscSpeed,
    /// The settings the command line gave, which win over the kept ones.
    given: front::Given,
    /// Scans presented frames for one-frame dropouts and captures them.
    flicker_scan: bool,
    /// Writes the run's frame, GPU and CPU times here when the player quits.
    perf_report: Option<String>,
    /// Frames are drawn for a display of this rate, not the window's monitor's.
    refresh: Option<f64>,
    /// The match's random seed, instead of one from the clock.
    seed: Option<u32>,
    /// Plays a stage or character the command line doesn't offer (`--unfinished-stages`, for
    /// developers): one whose audit isn't done or found code a match can reach unported.
    unfinished_stages: bool,
    /// Saves each match of the player's own as a Slippi replay (`--no-record` turns it off).
    record: bool,
    /// Where the replays go: the replays folder beside the executable unless given.
    replays: Option<String>,
    /// The item frequency and switch a command line's match starts with: items off.
    items: GamePrefs,
    /// The Special Melee mode whose rules the match plays by, if any.
    special: Option<Special>,
    /// Boots the console from power-on into Melee's own menus.
    power_on: bool,
    /// Plays a recorded session (`SSBMINP1`) from power-on, its reads instead of the
    /// controllers.
    inputs: Option<String>,
    /// Where the console's session is recorded: `--record-session FILE`, else a new file in the
    /// sessions folder; none with `--no-session-record`.
    record_session: Option<String>,
    session_record: bool,
    /// The saves folder the console's card comes from (`--saves DIR`), else the settings
    /// folder's.
    saves: Option<String>,
    /// Scripted controllers by tick (`--holds`), for play-testing without anyone at them.
    holds: Option<holds::Holds>,
    /// Imports this save or card image into the saves folder and quits (`--import-save`).
    import_save: Option<String>,
    /// Exports the saves folder's save as a `.gci`, or its card as a raw image (`.raw`,
    /// `.card`), and quits (`--export-save`).
    export_save: Option<String>,
    /// Unlocks everything in the saves folder's save and quits (`--unlock-all`).
    unlock_all: bool,
    /// Writes the console's memory card as the session leaves it here (`--card-after`).
    card_after: Option<String>,
}

/// The command line's usage, with the characters and stages melee-sim has ported.
fn usage() -> String {
    let names = |keys: Vec<&str>| keys.join("|");
    format!(
        "usage: melee-hd [--p1 NAME] [--p2 NAME] [--p3 NAME] [--p4 NAME] [--stage NAME] [--stocks N] [--minutes N] [--disc ISO] [--replay FILE] [--widescreen | --aspect W:H] [--scale N] [--msaa 4] [--one-thread] [--no-adapter] [--mute] [--size WxH] [--fullscreen] [--disc-speed drive|fast] [--bench TICKS [--resize-at TICK:WxH]] [--settings-shot FILE.png]\n\
         characters: {}\n\
         stages: {}\n\
         latency: [--latency-log FILE.csv] [--latency-overlay] [--present mailbox|fifo|immediate] [--added-delay FRAMES] [--audio-exclusive] [--tick-timing timer|auto|refresh|vrr] [--jit-margin MS]\n\
         frames: [--high-refresh off|interpolate|extrapolate] [--frames-per-tick N (with --bench)]\n\
         debug: [--flicker-scan] captures a presented frame that drops out for one frame\n\
         perf: [--perf-report FILE] [--refresh HZ] [--seed N]\n\
         replays: [--no-record] [--replays DIR] each match is saved to DIR, else to replays beside the program\n\
         special melee: [--special MODE] plays by a mode's rules: {}\n\
         developers: [--unfinished-stages] plays a stage or character not offered yet, which can stop mid-match\n\
         console: [--power-on] boots Melee from power-on into its menus; [--inputs FILE.inputs] plays a recorded session from power-on (with --bench, headless); [--record-session FILE] [--no-session-record] [--saves DIR] [--holds SCRIPT] (scripted controllers by tick, MONKEY_HOLD's form) [--card-after FILE] (the card as the session leaves it)\n\
         saves: [--import-save FILE.gci|.raw] [--export-save FILE.gci|.raw] [--unlock-all], each with [--saves DIR], then quit\n\
         with no --p1..--p4, --stage, --replay or --inputs, Melee boots from power-on with the native save; Esc opens the settings\n\
         the disc is --disc's, else SSBM_DISC's, else the one the disc picker asked for and remembered",
        names(roster::offered_characters().map(|c| c.keys[0]).collect()),
        names(roster::offered_stages().map(|s| s.keys[0]).collect()),
        names(
            Special::ALL
                .into_iter()
                .filter(|&m| special_audit::offered(m))
                .map(Special::key)
                .collect()
        ),
    )
}

fn parse_args() -> Result<Args, String> {
    parse_args_from(std::env::args().skip(1).collect())
}

/// [`parse_args`] for these arguments.
fn parse_args_from(args: Vec<String>) -> Result<Args, String> {
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
        front: false,
        settings_shot: None,
        fullscreen: false,
        disc_speed: DiscSpeed::Drive,
        given: front::Given::default(),
        flicker_scan: false,
        perf_report: None,
        refresh: None,
        seed: None,
        unfinished_stages: false,
        record: true,
        replays: None,
        items: GamePrefs::items_off(),
        special: None,
        power_on: false,
        inputs: None,
        record_session: None,
        session_record: true,
        saves: None,
        holds: None,
        import_save: None,
        export_save: None,
        unlock_all: false,
        card_after: None,
    };
    // Whether the command line names a match.
    let mut chosen = false;
    let mut it = args.into_iter();
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
            }
            "--minutes" => {
                a.minutes = value()?.parse().map_err(|_| "--minutes N")?;
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
                let ms = front::parse_margin(&value()?)
                    .ok_or(format!("--jit-margin MS (0 to {})", front::MAX_MARGIN_MS))?;
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
                };
                a.given.msaa = true;
            }
            "--bench" => a.bench = Some(value()?.parse().map_err(|_| "--bench TICKS")?),
            "--size" | "--windowed" => a.size = size(&value()?)?,
            "--ticks" => a.ticks = Some(value()?.parse().map_err(|_| "--ticks N")?),
            "--scale" => {
                a.scale = value()?.parse().map_err(|_| "--scale N")?;
                a.given.scale = true;
            }
            "--shot" => a.shot = Some(value()?),
            "--resize-at" => {
                let v = value()?;
                let (tick, s) = v.split_once(':').ok_or("--resize-at TICK:WxH")?;
                let tick = tick.parse().map_err(|_| "--resize-at TICK:WxH")?;
                a.resize_at.push((tick, size(s)?));
            }
            "--settings-shot" => a.settings_shot = Some(value()?),
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
            "--flicker-scan" => a.flicker_scan = true,
            "--perf-report" => a.perf_report = Some(value()?),
            "--refresh" => {
                a.refresh = Some(
                    value()?
                        .parse()
                        .ok()
                        .filter(|&hz: &f64| hz >= 1.0)
                        .ok_or("--refresh HZ")?,
                )
            }
            "--seed" => a.seed = Some(value()?.parse().map_err(|_| "--seed N")?),
            "--no-record" => {
                a.record = false;
                a.given.record = true;
            }
            "--replays" => {
                a.replays = Some(value()?);
                a.given.replays = true;
            }
            "--unfinished-stages" => a.unfinished_stages = true,
            "--power-on" => {
                a.power_on = true;
                chosen = true;
            }
            "--inputs" => {
                a.inputs = Some(value()?);
                chosen = true;
            }
            "--record-session" => a.record_session = Some(value()?),
            "--no-session-record" => {
                a.session_record = false;
                a.given.record_sessions = true;
            }
            "--saves" => a.saves = Some(value()?),
            "--holds" => a.holds = Some(holds::Holds::parse(&value()?)?),
            "--import-save" => {
                a.import_save = Some(value()?);
                chosen = true;
            }
            "--export-save" => {
                a.export_save = Some(value()?);
                chosen = true;
            }
            "--card-after" => a.card_after = Some(value()?),
            "--unlock-all" => {
                a.unlock_all = true;
                chosen = true;
            }
            "--special" => {
                let v = value()?;
                a.special = Some(Special::from_key(&v).ok_or("--special MODE")?);
            }
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    a.front = !chosen && a.bench.is_none();
    a.power_on |= a.front;
    let console = a.power_on || a.inputs.is_some();
    if a.replay.is_none() && !a.front && !a.unfinished_stages && !console {
        let characters: Vec<u8> = a.players.iter().map(|p| p.0).collect();
        if let Some(why) = roster::characters_refusal(&characters) {
            return Err(format!("{why} (--unfinished-stages plays it anyway)"));
        }
        let why = roster::stage_by_kind(a.stage).and_then(|s| s.refusal(&characters));
        if let Some(why) = why {
            return Err(format!("{why} (--unfinished-stages plays it anyway)"));
        }
        if let Some(why) = a.special.and_then(special_refusal) {
            return Err(format!("{why} (--unfinished-stages plays it anyway)"));
        }
    }
    Ok(a)
}

/// Why a Special Melee mode isn't offered: what its audit finds a match with its rules reaches
/// unported (`special_audit::missing`).
fn special_refusal(mode: Special) -> Option<String> {
    let gaps = special_audit::missing(mode);
    if gaps.is_empty() {
        return None;
    }
    let whats: Vec<&str> = gaps.iter().map(|g| g.what).collect();
    Some(format!(
        "{} isn't ported yet: {}.",
        mode.name(),
        whats.join("; ")
    ))
}

/// What the player runs and what drives it: the console from power-on with the controllers
/// ([`power`]), or a match (a replay's recorded inputs, or the command line's for developers).
struct Session {
    run: power::Run,
    /// The game's executable, whose static data the renderer reads (the default font).
    dol: Vec<u8>,
    replay: Option<Replay>,
    ended: bool,
    sound: Option<sound::Sound>,
    /// Where the match's recording goes when the session ends, when it's recorded.
    recording: Option<recording::Recording>,
    /// The console's platform from power-on: the disc, the saves, the session's recording and
    /// the menus' gates.
    console: Option<Box<Platform>>,
}

impl Platform {
    /// The reads `live` handed the game, recorded, and the feed moved on.
    fn after(&mut self, next: usize) {
        if let Some((_, n)) = &mut self.feed {
            *n = next;
        }
    }

    /// The VS matches' Slippi replays: a recorder waits in the game for the next match's start
    /// (it records nothing before it), and the match's replay is saved at its end.
    fn replay(&mut self, g: &mut melee_sim::game::state::Game) {
        use melee_sim::game::gm::machine::{GM_OPENING_MV, GM_TITLE, Phase};
        use melee_sim::game::record::{Recorder, Setup};
        let Some(dir) = &self.replays else {
            if let Some(r) = g.recorder.take() {
                self.save_replay(&r);
            }
            return;
        };
        match &g.recorder {
            None => g.recorder = Some(Box::new(Recorder::new(Setup::vanilla()))),
            Some(r) => {
                if self.slp.is_none() && r.last_frame().is_some() && g.vs.start.match_kind == 3 {
                    // A Bonus match's placements at its end take its bonuses' tally
                    // (`fn_8016588C`), which the recorder, only reading, can't run: it isn't
                    // recorded (a fresh recorder waits for the next match's start).
                    eprintln!("a Bonus match isn't saved as a replay");
                    g.recorder = Some(Box::new(Recorder::new(Setup::vanilla())));
                    return;
                }
                if self.slp.is_none() && r.last_frame().is_some() {
                    // The title's attract match plays itself: not the player's.
                    let attract = matches!(g.machine.phase, Phase::Scene { mode, .. }
                        if mode == GM_TITLE || mode == GM_OPENING_MV);
                    self.slp = Some((recording::Recording::new(dir.as_deref()), !attract));
                }
                if r.ended() {
                    let r = g.recorder.take().expect("the recorder");
                    self.save_replay(&r);
                    g.recorder = Some(Box::new(Recorder::new(Setup::vanilla())));
                }
            }
        }
    }

    /// Saves the match's replay, if one started and is kept.
    fn save_replay(&mut self, r: &melee_sim::game::record::Recorder) {
        if let Some((to, true)) = self.slp.take() {
            match to.save(r) {
                Ok(Some(path)) => eprintln!("saved the replay {}", path.display()),
                Ok(None) => {}
                Err(e) => eprintln!("the replay couldn't be saved: {e}"),
            }
        }
    }

    /// Power-on: the boot to the first scene.
    fn boot(&mut self, s: &mut melee_sim::game::session::Session) {
        let pads =
            [melee_sim::input::pad::PadStatus::error(melee_sim::input::pad::ERR_NO_CONTROLLER); 4];
        let feed = self.feed.take();
        let next = {
            let source = match &feed {
                Some((reads, next)) => power::Source::Reads { reads, next: *next },
                None => power::Source::Now(pads),
            };
            let mut live = power::Live::new(&self.disc, source);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.boot(&mut live)));
            let next = live.record(self.rec.as_mut());
            if let Err(p) = r {
                std::panic::resume_unwind(p);
            }
            next
        };
        self.feed = feed;
        self.after(next);
    }

    /// One tick with the controllers `pads`; the card's changes written to the saves folder.
    fn step(
        &mut self,
        s: &mut melee_sim::game::session::Session,
        pads: melee_sim::input::controller::PadData,
    ) -> melee_sim::game::vs_match::Frame {
        let feed = self.feed.take();
        let gated = match &feed {
            Some(_) => pads,
            None => self.gates.apply(&s.game, pads),
        };
        let (f, next) = {
            let source = match &feed {
                Some((reads, next)) => power::Source::Reads { reads, next: *next },
                None => power::Source::Now(gated),
            };
            let mut live = power::Live::new(&self.disc, source);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                power::step(s, &mut live)
            }));
            let next = live.record(self.rec.as_mut());
            match r {
                Ok(f) => (f, next),
                Err(p) => {
                    self.feed = feed;
                    self.after(next);
                    std::panic::resume_unwind(p);
                }
            }
        };
        self.feed = feed;
        self.after(next);
        self.replay(&mut s.game);
        if let (Some(k), Some(card)) = (
            &mut self.kept,
            s.card.as_ref().and_then(|c| c.card.as_ref()),
        ) {
            for name in k.update(card.image()) {
                eprintln!("saved {name}");
            }
        }
        f
    }
}

/// What the console from power-on reads from and writes to besides the game's state.
struct Platform {
    disc: Disc,
    /// The reads played back (`--inputs`), and the next; none: the controllers.
    feed: Option<(Vec<melee_sim::input::controller::PadData>, usize)>,
    /// The session's recording.
    rec: Option<session_rec::Writer>,
    /// The saves folder, kept as the card changes.
    kept: Option<saves::Kept>,
    /// Where the card goes as the session ends (`--card-after`).
    card_after: Option<std::path::PathBuf>,
    /// Each VS match saved as a Slippi replay: to this folder (none: the replays folder beside
    /// the program), when on.
    replays: Option<Option<String>>,
    /// The replay of the match playing, once it started, and whether it's kept (the title's
    /// attract match isn't).
    slp: Option<(recording::Recording, bool)>,
    gates: gates::Gates,
}

impl Drop for Session {
    /// Saves the match's recording, however it ended, and with `--card-after` the console's
    /// card.
    fn drop(&mut self) {
        if let (Some(p), power::Run::Console(s)) = (&mut self.console, &mut self.run)
            && let Some(r) = s.game.recorder.take()
        {
            p.save_replay(&r);
        }
        if let (Some(p), power::Run::Console(s)) = (&self.console, &self.run)
            && let Some(path) = &p.card_after
            && let Some(card) = s.card.as_ref().and_then(|c| c.card.as_ref())
        {
            match std::fs::write(path, card.image()) {
                Ok(()) => eprintln!("the card after the session: {}", path.display()),
                Err(e) => eprintln!("{}: {e}", path.display()),
            }
        }
        let (Some(to), Some(r)) = (&self.recording, &self.game().recorder) else {
            return;
        };
        match to.save(r) {
            Ok(Some(path)) => eprintln!("saved the replay {}", path.display()),
            Ok(None) => {}
            Err(e) => eprintln!("the replay couldn't be saved: {e}"),
        }
    }
}

impl Session {
    fn game(&self) -> &melee_sim::game::state::Game {
        self.run.game()
    }

    fn game_mut(&mut self) -> &mut melee_sim::game::state::Game {
        self.run.game_mut()
    }

    fn data(&self) -> &melee_data::archive::Archives {
        self.run.data()
    }

    fn recycle(&mut self, f: melee_sim::game::vs_match::Frame) {
        self.run.recycle(f);
    }

    /// The console from power-on, on `args`' disc: the card from the saves folder (none with
    /// `saves` none: a blank card's), the session recorded into `rec` when given, the reads
    /// played from `feed` when given.
    fn power_on(
        args: &Args,
        saves: Option<&std::path::Path>,
        feed: Option<melee_sim::game::inputs::Recording>,
        rec: Option<&std::path::Path>,
    ) -> Result<Session, String> {
        let disc =
            Disc::open(&args.disc).map_err(|e| format!("{}: {}", args.disc, settings::why(&e)))?;
        let (setup, kept, feed) = match feed {
            Some(r) => {
                let card = match &r.card {
                    Some(p) => Some(std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?),
                    None => None,
                };
                let setup = melee_sim::game::session::Setup {
                    disc: r.disc()?,
                    dsp: r.dsp(),
                    card,
                    power_on: r.power_on,
                    dtv: r.dtv,
                    sram_flags: r.sram_flags,
                    card_pull: None,
                };
                (setup, None, Some((r.reads, 0)))
            }
            None => {
                let (card, kept) = match saves {
                    Some(dir) => {
                        let (card, kept) = saves::Kept::open(dir);
                        (card, Some(kept))
                    }
                    None => (saves::card_of(&Default::default()).0.into_image(), None),
                };
                (power::setup(card, power::power_on_clock()), kept, None)
            }
        };
        let rec = match rec {
            Some(p) => match session_rec::Writer::create(p, &setup) {
                Ok(w) => {
                    eprintln!("recording the session in {}", p.display());
                    Some(w)
                }
                Err(e) => {
                    eprintln!("the session isn't recorded ({}): {e}", p.display());
                    None
                }
            },
            None => None,
        };
        let play = args.bench.is_none() && !args.mute;
        let (s, sound) = sound::power_on(setup, play)?;
        if feed.as_ref().is_some_and(|f| f.0.is_empty()) {
            return Err("the recording has no reads".into());
        }
        let mut platform = Box::new(Platform {
            disc,
            feed,
            rec,
            kept,
            card_after: args.card_after.as_ref().map(std::path::PathBuf::from),
            replays: (args.record && args.bench.is_none() && args.inputs.is_none())
                .then(|| args.replays.clone()),
            slp: None,
            gates: gates::Gates::default(),
        });
        let mut s = s;
        let dol = platform.disc.dol().map_err(|e| format!("main.dol: {e}"))?;
        let booted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            platform.boot(&mut s);
        }));
        if let Err(p) = booted {
            return Err(format!("Melee stopped at power-on: {}", panic_text(&p)));
        }
        let mut session = Session {
            run: power::Run::Console(Box::new(s)),
            dol,
            replay: None,
            ended: false,
            sound,
            recording: None,
            console: Some(platform),
        };
        if let Some(snd) = &mut session.sound {
            let power::Run::Console(s) = &mut session.run else {
                unreachable!()
            };
            snd.mix(&mut s.game.audio, &s.data);
        }
        Ok(session)
    }

    /// What the command line asks for: the console from power-on, a replay's match or the
    /// command line's.
    fn open(a: &Args) -> Result<Session, String> {
        if !(a.power_on || a.inputs.is_some()) {
            return Session::start(a);
        }
        let feed = match &a.inputs {
            Some(p) => Some(melee_sim::game::inputs::Recording::read(
                std::path::Path::new(p),
            )?),
            None => None,
        };
        let rec = match (&a.record_session, a.session_record) {
            (Some(p), _) => Some(std::path::PathBuf::from(p)),
            // A benchmark or a played-back session isn't recorded unless asked.
            (None, true) if a.bench.is_none() && feed.is_none() => {
                let dir = session_rec::dir();
                Some(session_rec::new_path(&dir, recording::local_now()))
            }
            (None, _) => None,
        };
        let saves = a
            .saves
            .as_ref()
            .map_or_else(saves::dir, std::path::PathBuf::from);
        let mut s = Session::power_on(a, Some(&saves), feed, rec.as_deref())?;
        s.set_aspect(a.aspect);
        Ok(s)
    }

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
                let players: Vec<melee_sim::game::vs_match::Player> = (a.players.iter())
                    .enumerate()
                    .filter(|(_, p)| p.0 != 0xFF)
                    .map(
                        |(i, &(character, costume))| melee_sim::game::vs_match::Player {
                            character,
                            costume,
                            port: i as u8,
                            kind: 0,
                            cpu_level: 0,
                            team: i as u8,
                        },
                    )
                    .collect();
                let rules = GameRules::vs(a.stocks, a.minutes);
                let mut s = Settings::vs_special(a.stage, &players, &rules, &a.items, a.special);
                // A seed from the clock unless given: presentation-side, set once before the
                // match.
                s.seed = a.seed.unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.subsec_nanos())
                });
                (s, None)
            }
        };
        // A match of the player's own is recorded (a benchmark's isn't), as melee-hd's Vanilla
        // ruleset plays it.
        let record = a.record && replay.is_none() && a.bench.is_none();
        // The replay's match too reads at the player's pace, as ssbm-rs's window plays one.
        let settings = Settings {
            disc: a.disc_speed,
            record: record.then(melee_sim::game::record::Setup::vanilla),
            ..settings
        };
        let mut s = Session::begin(&disc, &settings, replay, a.bench.is_none() && !a.mute)?;
        s.set_aspect(a.aspect);
        s.recording = record.then(|| recording::Recording::new(a.replays.as_deref()));
        Ok(s)
    }

    /// Draws for frames shown at `aspect`: in widescreen, whose added sides show what the
    /// game camera doesn't reach, every fighter it marks off screen draws, as Slippi's
    /// widescreen code draws them (a cosmetic toggle, `Cosmetic::draw_offscreen_fighters`).
    fn set_aspect(&mut self, aspect: present::Aspect) {
        self.game_mut().cosmetic.draw_offscreen_fighters = aspect.widescreen().is_some();
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
            run: power::Run::Match(Box::new(m)),
            dol: disc.dol().map_err(|e| format!("main.dol: {e}"))?,
            replay,
            ended: false,
            sound,
            recording: None,
            console: None,
        })
    }

    /// One engine tick with these controllers; the frame's draws if it renders.
    fn tick(
        &mut self,
        pads: [melee_sim::input::pad::PadStatus; 4],
    ) -> melee_sim::game::vs_match::Frame {
        let f = match (&mut self.run, &mut self.console) {
            (power::Run::Console(s), Some(p)) => {
                let f = p.step(s, pads);
                if let Some(snd) = &mut self.sound {
                    snd.mix(&mut s.game.audio, &s.data);
                }
                // A scene's end isn't the session's; a played-back session's is its last read.
                if p.feed.as_ref().is_some_and(|(r, n)| *n >= r.len()) {
                    self.ended = true;
                }
                return f;
            }
            (power::Run::Match(m), _) => match &self.replay {
                Some(r) => m.step_replay(&mut melee_slippi::Device(r)),
                None => m.step(pads),
            },
            (power::Run::Console(_), None) => unreachable!("a console without its platform"),
        };
        if let (Some(snd), power::Run::Match(m)) = (&mut self.sound, &mut self.run) {
            snd.mix(&mut m.game.audio, &m.data);
        }
        self.ended |= f.ended;
        f
    }

    /// Extrapolate's prediction of the next tick, on the predictor's copy of the match: with
    /// `pads` held (a replay's next frame for a replay). The session itself, its sound
    /// included, is only read.
    fn predict(
        &self,
        predictor: &mut Predictor<power::Run>,
        pads: [melee_sim::input::pad::PadStatus; 4],
    ) -> Result<Cost, String> {
        let replay = self.replay.as_ref();
        predictor.predict(&self.run, |c| match c {
            power::Run::Match(m) => match replay {
                Some(r) => m.step_replay(&mut melee_slippi::Device(r)),
                None => m.step(pads),
            },
            power::Run::Console(s) => power::step_predicted(s, pads),
        })
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

/// The settings panel, when the player boots the console: its settings, and its egui state
/// once the window exists.
struct Front {
    panel: front::Panel,
    window: Option<ui::Window>,
}

struct App {
    /// The console (or a command line's match) playing; none while it powers on.
    session: Option<Session>,
    /// The power-on running in the background, which makes the next session.
    booting: Option<std::thread::JoinHandle<Result<Session, String>>>,
    /// The settings panel, when the player boots the console.
    front: Option<Front>,
    /// The command line, with the settings applied.
    args: Args,
    /// The last frame shown, which the panel draws over.
    last_image: Option<melee_gpu::Frame>,
    /// The menus' gate note's painter, once it's shown.
    note_painter: Option<ui::Painter>,
    frames: Frames,
    gpu: present::Gpu,
    renderer: Option<Gpu>,
    shown: Option<present::Shown>,
    inputs: Inputs,
    /// Each port's rumble motor as the match last set it (`PADControlMotor`'s commands):
    /// the controllers' rumble output, which the save's rumble options decide.
    motors: [bool; 4],
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
    /// Why the match stopped, for the panel to show.
    stopped: Option<String>,
    /// The latency log, with `--latency-log`.
    latency: Option<latency::Log>,
    /// The input latency added back on purpose.
    delay: melee_input::Delay,
    /// The last second's latency, and the overlay showing it (F3), while it's on.
    live: latency::Live,
    overlay: Option<ui::Painter>,
    /// F12 was pressed, or the flicker scan hit: the next frame drawn is captured into this
    /// folder ([`capture`]).
    capture_next: Option<std::path::PathBuf>,
    /// `--flicker-scan`'s scan of the presented frames.
    scan: Option<flicker::Scan>,
    /// How frames are drawn between ticks, and with Interpolate or Extrapolate, when.
    mode: pacing::Mode,
    pacer: Option<pacing::Pacer>,
    /// With Interpolate or Extrapolate: the ticks the frames draw from.
    paced: paced::Paced,
    /// With Extrapolate: a prediction stopped, which is said once.
    prediction_failed: bool,
    /// With Interpolate or Extrapolate: a tick's latency measures, waiting for the first frame
    /// after it.
    pending: Option<latency::Tick>,
    /// The run's measures, with `--perf-report`.
    perf: Option<perf::Perf>,
    /// What the specialized shaders did, when the renderer last went.
    specialized: Option<String>,
    /// Camera Mode's frames drawn as the console draws them, for its snapshots
    /// ([`App::give_snapshot`]).
    console_view: Option<ConsoleView>,
}

/// [`App::give_snapshot`], with the console's view in `view` and the GPU `gpu`.
fn give_snapshot(
    view: &mut Option<ConsoleView>,
    gpu: &present::Gpu,
    s: &Session,
    draws: &[melee_sim::hsd::display::Draw],
) {
    use melee_sim::game::gm::camera_mode::GM_CAMERA_MODE;
    use melee_sim::game::gm::machine::Phase;
    use melee_sim::hsd::display::Draw;
    use melee_sim::lb::snap_write::{IMAGE_SIZE, give_frame};
    let camera =
        matches!(s.game().machine.phase, Phase::Scene { mode, .. } if mode == GM_CAMERA_MODE);
    if !camera {
        *view = None;
        return;
    }
    if draws.is_empty() {
        return;
    }
    let view = view.get_or_insert_with(|| {
        let mut frames = Frames::new();
        frames.space.load_dol(&s.dol);
        ConsoleView {
            frames,
            gpu: Gpu::new(pipelines::renderer(gpu, 1, 1, present::Aspect::STRETCHED)),
        }
    });
    let snapshot =
        s.game().camera_mode.snap.image != 0 && draws.iter().any(|d| matches!(d, Draw::SnapCopy));
    let fifo = view.frames.frame(s.game(), s.data(), draws);
    if snapshot {
        // RGB565, the snapshot copy's format (`snap_copy`).
        view.gpu.set_copies_to_memory(vec![4]);
    }
    let _ = view.gpu.run(&view.frames.space, s.data(), &fifo, None);
    if snapshot {
        view.gpu.set_copies_to_memory(Vec::new());
        let space = &view.frames.space;
        let at = space.dynamic(melee_render::mem::Dynamic::Snapshot, IMAGE_SIZE);
        let mut pixels = vec![0; IMAGE_SIZE as usize];
        space.read(s.data(), at, &mut pixels);
        give_frame(pixels);
        eprintln!("Camera Mode: the snapshot's frame handed to the game");
    }
}

/// A renderer drawing the frames as the console does (640 by 480, unwidened), never shown.
struct ConsoleView {
    frames: Frames,
    gpu: Gpu,
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
        let mut pads = self.delay.apply(self.inputs.read());
        if let Some(h) = &self.args.holds {
            pads = h.at(self.ticks);
        }
        let session = self.session.as_mut().expect("a match");
        let f = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.tick(pads)))
        {
            Ok(f) => f,
            Err(p) => {
                let what = if self.front.is_some() {
                    "Melee"
                } else {
                    "the match"
                };
                let text = format!("{what} stopped: {}", panic_text(&p));
                eprintln!("{text}");
                session.ended = true;
                self.stopped = Some(text);
                return false;
            }
        };
        let simulated = start.elapsed();
        // The pad library's motor commands this tick, to the controllers.
        if f.pads.motors.iter().any(Option::is_some) {
            for (on, m) in self.motors.iter_mut().zip(f.pads.motors) {
                if let Some(m) = m {
                    *on = m == melee_sim::input::rumble::Motor::Rumble;
                }
            }
            self.inputs.rumble(self.motors);
        }
        let audio = session.sound.as_ref().and_then(sound::Sound::stats);
        let tick = engine_tick(session.game());
        self.give_snapshot(&f.draws);
        let session = self.session.as_mut().expect("a match");
        self.jit.ran(&plan);
        self.stats.ticks += 1;
        self.ticks += 1;
        if self.max_ticks.is_some_and(|n| self.ticks >= n) {
            session.ended = true;
        }
        if tick != self.jit.tick() {
            // A game sped up (Lightning Melee) runs its engine ticks more often: Melee's clock
            // follows from the next tick, and the frames between ticks too.
            self.jit.set_tick(tick);
            if self.mode.paced() {
                self.start_pacing(self.mode);
            }
        }
        let mut frame = None;
        let mut id = None;
        if self.mode.paced() {
            // A tick that draws nothing is kept too: the frames hold the image shown until a
            // tick draws again, rather than draw the last tick's draws on the new state
            // ([`paced`]).
            let session = self.session.as_mut().expect("a match");
            if let Some(old) = self.paced.ran(self.mode, session.game(), f, plan.due) {
                session.recycle(old);
            }
            if self.mode == pacing::Mode::Extrapolate {
                self.predict(pads);
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
            self.session.as_mut().expect("a match").recycle(f);
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
    /// held. After a tick without a display pass the frames hold the image shown, and nothing
    /// is predicted.
    fn predict(&mut self, pads: [melee_sim::input::pad::PadStatus; 4]) {
        let next = self.delay.next().unwrap_or(pads);
        let session = self.session.as_ref().expect("a match");
        match self.paced.predict(|p| session.predict(p, next)) {
            Some(Ok(cost)) => {
                self.stats.predictions += 1;
                self.stats.predicting += cost.total();
            }
            Some(Err(why)) if !self.prediction_failed => {
                eprintln!("the next tick's prediction stopped (frames draw the latest): {why}");
                self.prediction_failed = true;
            }
            Some(Err(_)) | None => {}
        }
    }

    fn log_tick(&mut self, t: latency::Tick) {
        if let Some(p) = &mut self.perf {
            p.sim(t.sim);
        }
        self.live.tick(t);
        if let Some(log) = &mut self.latency {
            log.tick(t);
        }
    }

    /// With Interpolate or Extrapolate, draws the frame that's due, if one is: between the last
    /// two ticks, or the latest and the next predicted, at the fraction its display time gives
    /// ([`pacing`]). Aligned to the refreshes, each frame starts its lead before the refresh
    /// it's meant for. After a tick that drew nothing its refreshes keep the image shown.
    /// Returns when the next frame is due.
    fn draw_due(&mut self) -> Option<Instant> {
        let aligned = self
            .grid
            .filter(|_| self.policy == Some(jit::Policy::Aligned));
        let lead = self.frame_lead.get();
        let now = Instant::now();
        let (Some(pacer), Some(due)) = (&mut self.pacer, self.paced.due()) else {
            return None;
        };
        if aligned.is_some() != pacer.target().is_some() {
            pacer.align(now, aligned, lead);
        }
        if now < pacer.due() {
            return Some(pacer.due());
        }
        let target = pacer.target();
        let t = pacer.start(now, due);
        let paced = std::mem::take(&mut self.paced);
        // None after a tick that drew nothing: the refresh shows the image shown before.
        let drawn = paced
            .frame(self.mode, t)
            .map(|(draws, blend)| self.draw_frame(draws, now, Some(blend)));
        self.paced = paced;
        if drawn.is_some() {
            self.stats.frame(now.elapsed());
        }
        if let Some((mut times, shown)) = drawn.flatten() {
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
        self.monitor_hz = match self.args.refresh {
            Some(hz) => Some((hz * 1000.0).round() as u32),
            None => s
                .window
                .current_monitor()
                .and_then(|m| m.refresh_rate_millihertz()),
        };
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
        self.paced.clear();
        self.pending = None;
        self.prediction_failed = false;
        let (refresh, fifo) = self.display_refresh();
        self.pacer = mode.paced().then(|| {
            eprintln!(
                "high refresh: {}, display at {:.2} Hz",
                mode.key(),
                1.0 / refresh.as_secs_f64()
            );
            pacing::Pacer::new(self.jit.tick(), refresh, fifo, Instant::now())
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
        let entered = Instant::now();
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
        let m = self.session.as_ref().expect("a match");
        let fifo = self.frames.frame_blended(m.game(), m.data(), draws, blend);
        let gpu = self.renderer.get_or_insert_with(|| {
            gpu_for(
                pipelines::renderer(&self.gpu, self.scale, self.msaa, self.aspect),
                self.one_thread,
            )
        });
        let before = self
            .capture_next
            .is_some()
            .then(|| capture::Regs::of(&gpu.state));
        let images = gpu.run(&self.frames.space, m.data(), &fifo, None);
        let after = before.as_ref().map(|_| capture::Regs::of(&gpu.state));
        if let Some(dir) = self.capture_next.take()
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
            match capture::save(
                &dir,
                &self.gpu,
                tex,
                s.game(),
                s.data(),
                &s.dol,
                draws,
                blend,
                &about,
                &live,
            ) {
                Ok(()) => eprintln!("captured to {}", dir.display()),
                Err(e) => eprintln!("the capture failed: {e}"),
            }
        }
        let rendered = start.elapsed();
        let built = Instant::now();
        let (shown, frame) = (self.shown.as_mut()?, images.into_iter().last()?);
        let text = self.overlay.is_some().then(|| {
            self.live
                .text(&format!("{:?}", shown.present_mode()), self.delay.frames())
        });
        // The menus' gate note, for three seconds after a refusal.
        let note = (self.session.as_ref())
            .and_then(|s| s.console.as_ref())
            .and_then(|p| p.gates.refused)
            .filter(|(_, at)| at.elapsed() < gates::NOTE)
            .map(|(g, _)| gates::note_text(g));
        if note.is_some() && self.note_painter.is_none() {
            let mut p = ui::Painter::new(&self.gpu, shown.format());
            p.clear = false;
            self.note_painter = Some(p);
        }
        let (gpu, overlay, note_painter) = (&self.gpu, &mut self.overlay, &mut self.note_painter);
        let meta = flicker::Meta {
            tick: self.ticks,
            t: blend.map(|b| b.t),
            draws: draws.len(),
        };
        let scan = self.scan.as_mut().map(|s| (s, meta));
        let presented = shown.present(gpu, &frame, scan, |view, w, h| {
            if let (Some(p), Some(text)) = (overlay, text) {
                latency::overlay(p, gpu, &text, view, w, h);
            }
            if let (Some(p), Some(text)) = (note_painter, note) {
                front::note(p, gpu, &text, view, w, h);
            }
        });
        // Kept to draw the settings panel over.
        self.last_image = Some(frame);
        if let Some(s) = &mut self.scan
            && let Some(dir) = s.collect(gpu, self.mode.key()).pop()
        {
            self.capture_next = Some(dir.join("after"));
        }
        let presented = presented?;
        self.live.frame(presented.presented);
        self.measure(
            presented.presented,
            latency::CpuTimes {
                build: built - entered,
                acquire: presented.acquire,
                present: presented.presented - presented.acquired,
            },
        );
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

    /// Camera Mode's snapshot (`snap_write::give_frame`): its pixels are what a console's
    /// renderer leaves in the buffer, so they're drawn as the console draws, whatever the
    /// player shows: while a Camera Mode match runs, each tick's frame is drawn again by a
    /// renderer of its own at the console's 640 by 480 (scale 1, no MSAA, unwidened, as the
    /// frames checks draw), never presented; a frame that copies the snapshot
    /// (`Draw::SnapCopy`) while the game waits for one has that copy read back to memory and
    /// handed to the game. Nothing of the player's settings reaches the snapshot.
    fn give_snapshot(&mut self, draws: &[melee_sim::hsd::display::Draw]) {
        if let Some(s) = &self.session {
            give_snapshot(&mut self.console_view, &self.gpu, s, draws);
        }
    }

    /// A frame presented `at` with its CPU times, and the GPU times read back since the last:
    /// for the overlay and the perf report.
    fn measure(&mut self, at: Instant, cpu: latency::CpuTimes) {
        self.live.cpu(at, cpu);
        let gpu = self.gpu.gpu_times();
        for f in &gpu {
            self.live.gpu(at, *f);
        }
        if let Some(p) = &mut self.perf {
            p.ticks(self.ticks);
            p.frame(at, cpu);
            for f in gpu {
                p.gpu(f);
            }
        }
    }

    /// The run's settings, for the perf report.
    fn perf_config(&self) -> String {
        let window = self.shown.as_ref().map_or("no window".to_owned(), |s| {
            let size = s.window.inner_size();
            format!(
                "{}x{}{}, {:?}",
                size.width,
                size.height,
                if s.fullscreen() { " fullscreen" } else { "" },
                s.present_mode()
            )
        });
        let what = match &self.args.replay {
            Some(r) => format!("replay {r}"),
            None => {
                let names: Vec<&str> = self
                    .args
                    .players
                    .iter()
                    .filter_map(|&(c, _)| roster::character_by_kind(c).map(|c| c.keys[0]))
                    .collect();
                let stage = roster::stage_by_kind(self.args.stage).map_or("?", |s| s.keys[0]);
                format!(
                    "{} on {stage}, seed {}",
                    names.join(" vs "),
                    self.args
                        .seed
                        .map_or("from the clock".to_owned(), |s| s.to_string())
                )
            }
        };
        format!(
            "{what}; {} ticks; window {window}; internal {}x (EFB {}x{}), MSAA {}; high refresh {}, display {:.2} Hz; {}",
            self.ticks,
            self.scale,
            self.efb_width,
            528 * self.scale,
            self.msaa,
            self.mode.key(),
            self.monitor_hz.map_or(60.0, |m| f64::from(m) / 1000.0),
            present::describe(&self.gpu.adapter)
        )
    }

    /// Saves the renderer's pipelines and lets it go, with the match.
    fn drop_renderer(&mut self) {
        if let Some(g) = self.renderer.take() {
            self.specialized = pipelines::save(&g.into_renderer());
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
                let mut p = ui::Painter::new(&self.gpu, s.format());
                p.clear = false;
                Some(p)
            }
            _ => None,
        };
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(s) = &mut self.shown {
            s.set_fullscreen(!s.fullscreen());
            if let Some(f) = &mut self.front {
                f.panel.settings.fullscreen = s.fullscreen();
                f.panel.settings.keep();
                s.window.request_redraw();
            }
        }
    }

    /// Shows frames at `aspect` from the next frame on.
    fn set_aspect(&mut self, aspect: present::Aspect) {
        self.aspect = aspect;
        self.frames.hsd.widescreen = aspect.widescreen();
        if let Some(s) = &mut self.session {
            s.set_aspect(aspect);
        }
        present::set_aspect(aspect.shown);
    }

    /// Powers the console on in the background (its disc files read, the boot run), the
    /// window black meanwhile, as a console's is.
    fn power_on(&mut self) {
        let mut args = self.args.clone();
        if let Some(f) = &self.front {
            let s = &f.panel.settings;
            args.session_record = s.record_sessions;
            args.record = s.record;
            args.replays = Some(s.replays.clone()).filter(|d| !d.is_empty());
        }
        eprintln!("power on");
        self.booting = Some(
            std::thread::Builder::new()
                .stack_size(GAME_STACK)
                .spawn(move || {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Session::open(&args)))
                        .unwrap_or_else(|p| Err(format!("Melee stopped: {}", panic_text(&p))))
                })
                .expect("the power-on's thread"),
        );
    }

    /// Takes up the console the power-on made.
    fn powered_on(&mut self, session: Session) {
        self.frames = Frames::new();
        self.frames.space.load_dol(&session.dol);
        self.session = Some(session);
        let aspect = self.aspect;
        self.set_aspect(aspect);
        self.renderer = Some(gpu_for(
            pipelines::renderer(&self.gpu, self.scale, self.msaa, aspect),
            self.one_thread,
        ));
        // What the controllers did meanwhile doesn't reach the game.
        self.inputs.read();
        self.ticks = 0;
        self.stats = Stats::default();
        self.start_clock();
        self.start_pacing(self.mode);
    }

    /// Powers the console off: the session ends (its recordings saved), its renderer goes.
    fn power_off(&mut self) {
        self.paced.clear();
        self.session = None;
        self.drop_renderer();
        self.console_view = None;
        self.last_image = None;
    }

    /// Opens the settings panel over the game, which pauses.
    fn open_panel(&mut self) {
        let Some(f) = &mut self.front else { return };
        f.panel.open = true;
        f.panel.look_at_save();
        // Every key goes up: none stays held into the game when it resumes.
        self.inputs.keyboard = Default::default();
        if let Some(s) = &self.shown {
            s.window.request_redraw();
        }
    }

    /// Closes the panel: its settings kept and applied, and the game resumes on Melee's clock
    /// from now.
    fn close_panel(&mut self) {
        let Some(f) = &mut self.front else { return };
        f.panel.open = false;
        f.panel.message = None;
        let s = f.panel.settings.clone();
        s.keep();
        self.apply(&s);
        self.start_clock();
        self.start_pacing(s.high_refresh);
    }

    /// The settings to the frames, the latency and the recordings.
    fn apply(&mut self, s: &front::Settings) {
        let msaa = if s.msaa { 4 } else { 1 };
        self.args.scale = s.scale;
        self.set_aspect(s.aspect);
        self.scale = scale_for(s.scale, self.window_size, s.aspect);
        let width = s.aspect.efb_width(self.scale);
        if msaa != self.msaa {
            // Its pipelines take the sample count: a renderer made anew.
            self.msaa = msaa;
            if self.renderer.is_some() {
                self.drop_renderer();
                self.renderer = Some(gpu_for(
                    pipelines::renderer(&self.gpu, self.scale, msaa, s.aspect),
                    self.one_thread,
                ));
            }
        } else if let Some(g) = &mut self.renderer {
            g.resize_efb(self.scale, width);
        }
        self.efb_width = width;
        if let Some(sh) = &mut self.shown {
            sh.set_present(&self.gpu, s.present);
            if sh.fullscreen() != s.fullscreen {
                sh.set_fullscreen(s.fullscreen);
            }
        }
        self.delay = melee_input::Delay::new(s.added_delay);
        self.timing = s.tick_timing;
        self.margin = Duration::from_secs_f64(f64::from(s.jit_margin) / 1000.0);
        self.mode = s.high_refresh;
        self.args.record = s.record;
        self.args.replays = Some(s.replays.clone()).filter(|d| !d.is_empty());
        let replays = s.record.then(|| self.args.replays.clone());
        if let Some(p) = self.session.as_mut().and_then(|s| s.console.as_mut()) {
            p.replays = replays;
        }
    }

    /// Melee stopped: the panel shows why, and it can be restarted.
    fn halted(&mut self, why: String) {
        if let Some(f) = &mut self.front {
            f.panel.message = Some(why);
            f.panel.halted = true;
        }
        self.open_panel();
    }

    /// Draws the panel over the last frame, doing what it was asked.
    fn redraw_panel(&mut self, event_loop: &ActiveEventLoop) {
        let (Some(f), Some(shown)) = (&mut self.front, &mut self.shown) else {
            return;
        };
        let Some(w) = &mut f.window else { return };
        let action = w.redraw(&self.gpu, shown, &mut f.panel, self.last_image.as_ref());
        if f.panel.settings.fullscreen != shown.fullscreen() {
            shown.set_fullscreen(f.panel.settings.fullscreen);
        }
        if let Some(a) = action {
            self.act(a, event_loop);
        }
    }

    /// What the panel asked for: resume, restart, quit, or a change to the save or the disc
    /// (each confirmed first, each restarting Melee).
    fn act(&mut self, action: front::Action, event_loop: &ActiveEventLoop) {
        use front::Action;
        match action {
            Action::Resume => self.close_panel(),
            Action::Quit => {
                self.quit = true;
                event_loop.exit();
            }
            Action::Restart => {
                if dialogs::confirm(
                    "Restart Melee from power-on? Anything the game hasn't saved is lost.",
                    "Restart",
                ) {
                    self.restart(None);
                }
            }
            Action::ChangeDisc => {
                if let Some(disc) = dialogs::change_disc() {
                    self.args.disc = disc.clone();
                    if let Some(f) = &mut self.front {
                        f.panel.disc = disc;
                    }
                    self.restart(None);
                }
            }
            Action::Import => {
                if dialogs::import() {
                    self.restart(None);
                }
            }
            Action::Export => {
                if let Some(f) = &mut self.front {
                    f.panel.message = dialogs::export();
                }
            }
            Action::UnlockAll => {
                if dialogs::unlock_all() {
                    self.restart(None);
                }
            }
        }
        if let Some(s) = &self.shown {
            s.window.request_redraw();
        }
    }

    /// Powers off and on again, the panel closed.
    fn restart(&mut self, _said: Option<String>) {
        self.power_off();
        if let Some(f) = &mut self.front {
            f.panel.halted = false;
            f.panel.message = None;
            f.panel.look_at_save();
            f.panel.open = false;
            let s = f.panel.settings.clone();
            s.keep();
            self.apply(&s);
        }
        self.power_on();
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
            if let Some(f) = &mut self.front {
                f.window = Some(ui::Window::new(&self.gpu, &shown));
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
        // F11 and Alt+Enter toggle fullscreen, in Melee and with the settings open alike.
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
                self.capture_next = Some(capture::folder(""));
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
        let paneled = self.front.as_ref().is_some_and(|f| f.panel.open);
        if paneled
            && let (
                Some(Front {
                    window: Some(w), ..
                }),
                Some(shown),
            ) = (&mut self.front, &self.shown)
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
                if paneled && let Some(s) = &self.shown {
                    s.window.request_redraw();
                }
            }
            WindowEvent::Moved(_) | WindowEvent::ScaleFactorChanged { .. } => self.update_refresh(),
            WindowEvent::ModifiersChanged(m) => self.alt = m.state().alt_key(),
            WindowEvent::RedrawRequested if paneled => self.redraw_panel(event_loop),
            WindowEvent::RedrawRequested if self.session.is_none() => {
                if let (
                    Some(Front {
                        window: Some(w), ..
                    }),
                    Some(shown),
                ) = (&mut self.front, &mut self.shown)
                {
                    w.black(&self.gpu, shown);
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
                if code == KeyCode::Escape && state == ElementState::Pressed {
                    // Escape opens the settings panel over the console, and closes it again (a
                    // drop-down open in it closes first); without the panel, it quits.
                    match &self.front {
                        Some(f) if f.panel.open => {
                            let popup = f.window.as_ref().is_some_and(ui::Window::popup_open);
                            if !popup && !f.panel.halted {
                                self.close_panel();
                            }
                        }
                        Some(_) => self.open_panel(),
                        None => {
                            self.quit = true;
                            event_loop.exit();
                        }
                    }
                    return;
                }
                // The panel's key presses aren't the keyboard controller's; releases always
                // are, so no key stays held.
                let pressed = state == ElementState::Pressed;
                if let Some(k) = key(code)
                    && !(paneled && pressed)
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
        // The power-on's console, once its boot ran.
        if self
            .booting
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            let r = (self.booting.take().expect("the power-on").join())
                .unwrap_or_else(|p| Err(format!("Melee stopped: {}", panic_text(&p))));
            match r {
                Ok(s) => self.powered_on(s),
                Err(e) if self.front.is_some() => {
                    eprintln!("{e}");
                    self.halted(e);
                }
                Err(e) => {
                    console::fatal(&e);
                    self.quit = true;
                    event_loop.exit();
                    return;
                }
            }
        }
        let paneled = self.front.as_ref().is_some_and(|f| f.panel.open);
        if self.session.is_some() && !paneled {
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
            if self.front.is_none() {
                eprintln!("the match ended");
                self.quit = true;
                event_loop.exit();
                return;
            }
            let why = self
                .stopped
                .take()
                .unwrap_or_else(|| "Melee stopped.".to_owned());
            self.power_off();
            self.halted(why);
        }
        if self.booting.is_some() {
            // The window stays black while the console powers on.
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(10),
            ));
            return;
        }
        // The panel: woken by events, or when egui asked for its next frame.
        let flow = match self.front.as_mut().and_then(|f| f.window.as_mut()) {
            Some(w) if paneled => match w.repaint_at {
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
            _ => ControlFlow::Wait,
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
    let mut session = match Session::open(args) {
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
    let mut console_view = None;
    let mut last = None;
    let mut frames = Frames::new();
    frames.space.load_dol(&session.dol);
    frames.hsd.widescreen = args.aspect.widescreen();
    present::set_aspect(args.aspect.shown);
    let rest = [melee_sim::input::pad::PadStatus::default(); 4];
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
        let pads = args.holds.as_ref().map_or(rest, |h| h.at(u64::from(tick)));
        let f = session.tick(pads);
        let t1 = Instant::now();
        give_snapshot(&mut console_view, &gpu, &session, &f.draws);
        round += t1 - t0;
        if !f.draws.is_empty() {
            let fifo = frames.frame(session.game(), session.data(), &f.draws);
            let ta = Instant::now();
            let images = renderer.run(&frames.space, session.data(), &fifo, None);
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
        session.recycle(f);
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
    let mut session = match Session::open(args) {
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
    let rest = [melee_sim::input::pad::PadStatus::default(); 4];
    let per_tick = args.frames_per_tick;
    let extrapolate = args.high_refresh == pacing::Mode::Extrapolate;
    let (mut before, mut after) = (Capture::default(), Capture::default());
    let mut predictor = Predictor::<power::Run>::default();
    // Each prediction's parts, and a tick with its prediction.
    let mut costs: Vec<Cost> = Vec::with_capacity(ticks as usize);
    let mut ticked: Vec<Duration> = Vec::with_capacity(ticks as usize);
    let (mut sim, mut kept) = (Duration::ZERO, Duration::ZERO);
    let mut parts = [Duration::ZERO; 3];
    let mut drawing: Vec<Duration> = Vec::with_capacity((ticks * per_tick) as usize);
    let mut presented: Vec<Duration> = Vec::with_capacity((ticks * per_tick) as usize);
    let mut last = None;
    let mut rendered = 0u32;
    for tick in 0..ticks {
        let t0 = Instant::now();
        let pads = args.holds.as_ref().map_or(rest, |h| h.at(u64::from(tick)));
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
                after.fill(session.game(), &f.draws);
            }
            let (game, data) = (session.game(), session.data());
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
                let fifo = frames.frame_blended(game, data, &f.draws, Some(blend));
                let tb = Instant::now();
                let images = renderer.run(&frames.space, data, &fifo, None);
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
        session.recycle(f);
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

/// The settings: the kept ones, and over them what the command line gave.
fn settings_of(args: &Args) -> front::Settings {
    let mut s = front::Settings::default();
    s.load(&args.given);
    let g = &args.given;
    if g.aspect {
        s.aspect = args.aspect;
    }
    if g.fullscreen {
        s.fullscreen = args.fullscreen;
    }
    if g.scale {
        s.scale = args.scale;
    }
    if g.msaa {
        s.msaa = args.msaa == 4;
    }
    if g.present {
        s.present = args.present;
    }
    if g.added_delay {
        s.added_delay = args.added_delay;
    }
    if g.high_refresh {
        s.high_refresh = args.high_refresh;
    }
    if g.tick_timing {
        s.tick_timing = args.tick_timing;
    }
    if g.jit_margin {
        s.jit_margin = (args.jit_margin.as_secs_f64() * 1000.0) as f32;
    }
    if g.record {
        s.record = args.record;
    }
    if g.replays {
        s.replays = args.replays.clone().unwrap_or_default();
    }
    if g.record_sessions {
        s.record_sessions = args.session_record;
    }
    s
}

/// The command line with the settings in it.
fn with_settings(mut a: Args, s: &front::Settings) -> Args {
    a.aspect = s.aspect;
    a.fullscreen = s.fullscreen;
    a.scale = s.scale;
    a.msaa = if s.msaa { 4 } else { 1 };
    a.present = s.present;
    a.added_delay = s.added_delay;
    a.high_refresh = s.high_refresh;
    a.tick_timing = s.tick_timing;
    a.jit_margin = Duration::from_secs_f64(f64::from(s.jit_margin) / 1000.0);
    a.record = s.record;
    a.replays = Some(s.replays.clone()).filter(|d| !d.is_empty());
    a.session_record = s.record_sessions;
    a
}

fn run(args: Args) -> ExitCode {
    if let Some(path) = &args.settings_shot {
        let gpu = present::Gpu::new();
        let mut panel = front::Panel::new(settings_of(&args), args.disc.clone());
        panel.open = true;
        return match ui::shot(&gpu, &mut panel, args.size, std::path::Path::new(path)) {
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
    present::set_scanned(args.flicker_scan);
    if let Some(ticks) = args.bench {
        return bench(&args, ticks);
    }
    // The console from power-on, with the settings panel: its settings in the command line,
    // and its power-on in the background once the window is up.
    let front = args.front.then(|| {
        let settings = settings_of(&args);
        Front {
            panel: front::Panel::new(settings, args.disc.clone()),
            window: None,
        }
    });
    let args = match &front {
        Some(f) => with_settings(args, &f.panel.settings),
        None => args,
    };
    let session = if args.front {
        None
    } else {
        match Session::open(&args) {
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
    let scan = args.flicker_scan.then(|| flicker::Scan::new(&gpu));
    let mut frames = Frames::new();
    if let Some(s) = &session {
        frames.space.load_dol(&s.dol);
    }
    frames.hsd.widescreen = args.aspect.widescreen();
    present::set_aspect(args.aspect.shown);
    let mut app = App {
        session,
        booting: None,
        front,
        last_image: None,
        note_painter: None,
        frames,
        gpu,
        renderer: None,
        shown: None,
        inputs: Inputs::new(args.adapter),
        motors: [false; 4],
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
        capture_next: None,
        scan,
        mode: if args.front {
            args.high_refresh
        } else {
            pacing::Mode::Off
        },
        pacer: None,
        paced: paced::Paced::default(),
        prediction_failed: false,
        pending: None,
        perf: args.perf_report.as_deref().map(perf::Perf::new),
        specialized: None,
        console_view: None,
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
    if app.front.is_some() {
        app.power_on();
    }
    // The pipelines the last runs needed, made before the first tick.
    if app.session.is_some() {
        app.renderer = Some(gpu_for(
            pipelines::renderer(&app.gpu, app.scale, app.msaa, app.aspect),
            app.one_thread,
        ));
    }
    let r = event_loop.run_app(&mut app);
    app.inputs.close();
    // A power-on still running ends, its session's recordings saved as it goes.
    if let Some(h) = app.booting.take() {
        let _ = h.join();
    }
    app.session = None;
    app.drop_renderer();
    // The rows still waiting for their frames' GPU times.
    app.collect_done();
    for r in std::mem::take(&mut app.rows) {
        app.log_tick(r.tick);
    }
    if let Some(p) = &app.perf {
        let untimed = app.gpu.timer.as_ref().map(melee_gpu::GpuTimer::untimed);
        p.finish(&app.perf_config(), untimed, app.specialized.as_deref());
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
    // Which build this is, and the folders it keeps (a dev build's are its own).
    eprintln!(
        "{}; settings in {}, caches in {}",
        channel::stamp(),
        settings::folder().display(),
        pipelines::dir().map_or("none".into(), |d| d.display().to_string())
    );
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
    let dir = args
        .saves
        .as_ref()
        .map_or_else(saves::dir, std::path::PathBuf::from);
    if let Some(path) = &args.export_save {
        let raw = [".raw", ".card"]
            .iter()
            .any(|x| path.to_lowercase().ends_with(x));
        let bytes = if raw {
            Some(saves::export_raw(&dir))
        } else {
            saves::export_gci(&dir)
        };
        let Some(bytes) = bytes else {
            console::fatal(&format!("{} has no save to export", dir.display()));
            return ExitCode::FAILURE;
        };
        return match std::fs::write(path, bytes) {
            Ok(()) => {
                eprintln!("exported to {path}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                console::fatal(&format!("{path}: {e}"));
                ExitCode::FAILURE
            }
        };
    }
    if args.unlock_all {
        let r = match saves::unlocked(&dir) {
            Some(true) => Ok("everything is unlocked already".to_owned()),
            Some(false) => {
                let now = (power::power_on_clock() / melee_sim::audio::clock::TB_HZ) as u32;
                saves::unlock_all(&dir, now).map(|()| "unlocked everything".to_owned())
            }
            None => Err(format!("{} has no save to unlock", dir.display())),
        };
        return match r {
            Ok(m) => {
                eprintln!("{m}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                console::fatal(&e);
                ExitCode::FAILURE
            }
        };
    }
    if let Some(path) = &args.import_save {
        let r = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| saves::import(&b))
            .and_then(|i| {
                saves::replace(&dir, &i)
                    .map(|()| i)
                    .map_err(|e| e.to_string())
            });
        return match r {
            Ok(i) => {
                eprintln!(
                    "imported {path} into {}: the save and {} snapshots",
                    dir.display(),
                    i.snapshots()
                );
                ExitCode::SUCCESS
            }
            Err(e) => {
                console::fatal(&format!("{path}: {e}"));
                ExitCode::FAILURE
            }
        };
    }
    // The disc picker on the main thread, where macOS wants its dialogs.
    if args.disc.is_empty() && args.settings_shot.is_none() {
        match settings::disc() {
            Some(disc) => args.disc = disc,
            None => return ExitCode::SUCCESS,
        }
    }
    // The window must live on the main thread on some systems, so the game gets its stack by
    // running everything on a thread that is itself the event loop's only user.
    let child = std::thread::Builder::new()
        .stack_size(GAME_STACK)
        .spawn(move || run(args))
        .expect("the player's thread");
    child.join().unwrap_or_else(|p| {
        console::fatal(&format!("melee-hd stopped: {}", panic_text(&p)));
        ExitCode::FAILURE
    })
}
