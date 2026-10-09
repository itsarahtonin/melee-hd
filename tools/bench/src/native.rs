//! melee-hd's own speed on the benchmark matches, against Dolphin's (`dolphin.rs`), and rollback
//! snapshots of a real match's states.
//!
//! `melee-bench native [--dir DIR] [--runs N] [MATCH...]` plays each match (`DIR/MATCH.slp`)
//! natively and headless on one core: from the replay's start through `vs_match`'s replay path
//! (Slippi's playback codes, `Match::step_replay`), with the display pass, in the console's
//! floating point (`FpMode::Hardware`, as the matches were made). The match's files are read
//! from the disc `SSBM_DISC` before the clock starts; the fighters' addresses come from the
//! match's start trace (`DIR/MATCH.start.trace`), as the oracle takes them. Each run is timed
//! from the match's first tick to its last replay frame, and every frame's fighters (action
//! state, position, facing, percent and stocks) are checked against the replay's post-frame
//! updates afterwards. It reports the median frames per second over `N` runs (default 5) in
//! `melee-bench dolphin`'s table. A run that stops before the match's last frame (code no port
//! covers yet) is no result: the table says how far it got.
//!
//! `melee-bench snapshot-match [--dir DIR] [--every TICKS] [MATCH]` plays the match (default
//! `fox-fox-fd`) as far as it runs, keeping its state every `TICKS` ticks (default 1000), and
//! measures each as `melee-bench snapshot` measures a captured state, plus a restore followed
//! by 8 ticks, and the allocations those 8 ticks make.

use std::collections::{BTreeMap, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::Instant;

use gekko_fp::{FpMode, set_fp_mode};
use melee_data::disc::Disc;
use melee_replay::Device;
use melee_sim::game::state::Game;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use ssbm_slippi::replay::Replay;

use crate::sync::{FIRST_FRAME, Fighter, post_frames, replay_last_frame, same};
use crate::{counted, matches, median_us};

/// The simulation recurses as the original does (joint trees): it runs on a deep stack.
const STACK: usize = 512 << 20;

/// The disc's files, read once, so a run's setup and timing never wait on it.
struct Files {
    disc: Disc,
    cache: HashMap<String, DiscFile>,
}

impl Files {
    fn load(&mut self, name: &str) -> DiscFile {
        let disc = &self.disc;
        let _ = self
            .cache
            .entry(name.to_owned())
            .or_insert_with(|| DiscFile {
                entrynum: disc
                    .entrynum(name)
                    .unwrap_or_else(|e| panic!("{name}: {e}")),
                bytes: disc
                    .read_file(name)
                    .unwrap_or_else(|e| panic!("{name}: {e}")),
            });
        let f = &self.cache[name];
        DiscFile {
            entrynum: f.entrynum,
            bytes: f.bytes.clone(),
        }
    }
}

/// A benchmark match: its replay, the settings it starts from and its frames' fighters.
struct Bench {
    name: String,
    replay: Replay,
    settings: Settings,
    want: BTreeMap<i32, [Option<Fighter>; 8]>,
    last: i32,
}

impl Bench {
    /// The match, or why melee-hd can't start it.
    fn open(dir: &Path, name: &str) -> Result<Bench, String> {
        let path = dir.join(format!("{name}.slp"));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let replay = Replay::parse(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut settings = Settings::from_replay(&melee_replay::start(&replay))
            .map_err(|e| format!("the replay's codes: {e}"))?;
        let trace = dir.join(format!("{name}.start.trace"));
        settings.fighter_addresses = crate::sync::fighter_addresses(&trace).ok_or_else(|| {
            format!(
                "the fighters' addresses come from ssbm-rs's trace of the match's start, {}                  (make-matches.sh writes it)",
                trace.display()
            )
        })?;
        let want = post_frames(&replay);
        let last = replay_last_frame(&replay);
        Ok(Bench {
            name: name.to_owned(),
            replay,
            settings,
            want,
            last,
        })
    }

    fn start(&self, files: &mut Files) -> Match {
        Match::start(&self.settings, &mut |name| files.load(name))
    }
}

/// The frame a match's playback last counted.
fn frame(m: &Match) -> i32 {
    m.game
        .playback
        .as_ref()
        .map_or(FIRST_FRAME - 1, |p| p.frame_index)
}

/// The fighters at the end of the frame, as `dolphin.rs` samples Dolphin's: leaders in 0 to 3,
/// followers in 4 to 7.
fn fighters(g: &Game) -> [Option<Fighter>; 8] {
    let mut out = [None; 8];
    for (i, f) in out.iter_mut().enumerate() {
        let slot = (i % 4) as i32;
        let Some(gobj) = g.player_entity_at(slot, (i / 4) as i32) else {
            continue;
        };
        let fp = g.fighter(gobj);
        *f = Some(Fighter {
            action: fp.motion_id as u16,
            x: fp.cur_pos.x.to_bits(),
            y: fp.cur_pos.y.to_bits(),
            facing: fp.facing_dir.to_bits(),
            percent: fp.dmg.x1830_percent.to_bits(),
            stocks: g.player_stocks(slot) as u8,
        });
    }
    out
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// How a run went.
struct Run {
    /// Frames per second from the first tick to the last frame reached.
    fps: f64,
    /// The last replay frame the run reached, and the ticks it ran.
    reached: i32,
    ticks: u64,
    /// Why it stopped short of the match's last frame, if it did.
    stopped: Option<String>,
    /// The first frame whose fighters weren't the replay's, and how many frames were checked.
    desync: Option<i32>,
    checked: usize,
}

impl Run {
    fn complete(&self) -> bool {
        self.stopped.is_none()
    }
}

/// Plays the match once: timed ticks to its last frame, then the fighters checked.
fn run_once(b: &Bench, files: &mut Files) -> Run {
    let mut m = match catch_unwind(AssertUnwindSafe(|| b.start(files))) {
        Ok(m) => m,
        Err(p) => {
            return Run {
                fps: 0.0,
                reached: FIRST_FRAME - 1,
                ticks: 0,
                stopped: Some(format!("starting the match: {}", panic_text(&p))),
                desync: None,
                checked: 0,
            };
        }
    };
    let mut device = Device(&b.replay);
    // A sample per frame, kept cheap inside the timed loop: compared afterwards.
    let frames = (b.last - FIRST_FRAME + 1) as usize;
    let mut samples: Vec<(i32, [Option<Fighter>; 8])> = Vec::with_capacity(frames + 16);
    let mut ticks = 0u64;
    let mut stopped = None;
    let t = Instant::now();
    loop {
        match catch_unwind(AssertUnwindSafe(|| m.step_replay(&mut device))) {
            Ok(f) => {
                ticks += 1;
                let now = frame(&m);
                if samples.last().is_none_or(|s| s.0 != now) {
                    samples.push((now, fighters(&m.game)));
                }
                if now >= b.last {
                    break;
                }
                if f.ended {
                    stopped = Some(format!("the match ended at frame {now}"));
                    break;
                }
            }
            Err(p) => {
                stopped = Some(panic_text(&p));
                break;
            }
        }
    }
    let secs = t.elapsed().as_secs_f64();
    let reached = frame(&m);
    let mut desync = None;
    let mut checked = 0;
    for (f, got) in &samples {
        if let Some(want) = b.want.get(f) {
            checked += 1;
            if desync.is_none() && !same(got, want) {
                desync = Some(*f);
            }
        }
    }
    Run {
        fps: f64::from(reached - FIRST_FRAME + 1) / secs,
        reached,
        ticks,
        stopped,
        desync,
        checked,
    }
}

fn args_dir(args: &[String]) -> (PathBuf, usize, Vec<String>, Option<u64>) {
    let mut dir = std::env::var("BENCH_DIR").ok();
    let (mut runs, mut every) = (5usize, None);
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().cloned(),
            "--runs" => runs = it.next().and_then(|n| n.parse().ok()).expect("--runs N"),
            "--every" => every = it.next().and_then(|n| n.parse().ok()),
            _ => names.push(a.clone()),
        }
    }
    let dir = PathBuf::from(dir.expect("--dir DIR (or BENCH_DIR)"));
    (dir, runs, names, every)
}

fn files() -> Files {
    let path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    Files {
        disc: Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}")),
        cache: HashMap::new(),
    }
}

/// The simulation's thread: a deep stack, the console's floating point, raised priority.
fn on_sim_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || {
            set_fp_mode(FpMode::Hardware);
            #[cfg(windows)]
            crate::win::raise_thread_priority();
            f()
        })
        .unwrap()
        .join()
        .unwrap()
}

/// What this build is: melee-hd's commit.
fn version() -> String {
    let rev = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown commit".into());
    format!("melee-hd {rev} (release, one core, FpMode::Hardware)")
}

pub fn main(args: &[String]) {
    let (dir, runs, mut names, _) = args_dir(args);
    if names.is_empty() {
        names = matches::MATCHES.iter().map(|m| m.name.to_owned()).collect();
    }
    println!("{}", crate::machine());
    println!("{}", version());
    println!();
    println!("| match | frames | runs (frames/s) | median frames/s | in sync |");
    println!("| --- | --- | --- | --- | --- |");
    on_sim_thread(move || {
        let mut files = files();
        for name in &names {
            let b = match Bench::open(&dir, name) {
                Ok(b) => b,
                Err(e) => {
                    println!(
                        "| {name} | — | — | — | no result: {} |",
                        e.replace('|', "/")
                    );
                    continue;
                }
            };
            let frames = b.last - FIRST_FRAME + 1;
            let mut speeds = Vec::new();
            let mut sync = Vec::new();
            let mut partial = None;
            for i in 0..runs {
                let r = run_once(&b, &mut files);
                eprintln!(
                    "{name} run {}: {:.1} frames/s over frames {FIRST_FRAME}..{} ({} ticks), {} frames checked, {}",
                    i + 1,
                    r.fps,
                    r.reached,
                    r.ticks,
                    r.checked,
                    match (&r.stopped, r.desync) {
                        (Some(s), _) => format!("stopped: {s}"),
                        (None, Some(f)) => format!("OUT OF SYNC from frame {f}"),
                        (None, None) => "in sync".into(),
                    }
                );
                if !r.complete() {
                    partial = Some(r);
                    break;
                }
                speeds.push(r.fps);
                sync.push(r.desync.is_none());
            }
            if let Some(r) = partial {
                // Not a result: how far it got.
                let why = r.stopped.clone().unwrap_or_default().replace('|', "/");
                println!(
                    "| {} | {frames} | — | — | no result: stopped at frame {} of {} ({} ticks{}): {} |",
                    b.name,
                    r.reached,
                    b.last,
                    r.ticks,
                    r.desync
                        .map_or(String::new(), |f| format!(", out of sync from frame {f}")),
                    why
                );
                continue;
            }
            let mut sorted = speeds.clone();
            sorted.sort_by(f64::total_cmp);
            let median = sorted[sorted.len() / 2];
            let all: Vec<String> = speeds.iter().map(|s| format!("{s:.0}")).collect();
            let in_sync = if sync.iter().all(|&s| s) {
                "yes".to_owned()
            } else {
                format!(
                    "no ({} of {runs} runs differ)",
                    sync.iter().filter(|&&s| !s).count()
                )
            };
            println!(
                "| {} | {frames} | {} | {median:.0} | {in_sync} |",
                b.name,
                all.join(", ")
            );
        }
    });
}

/// Ticks after a restore that the restore-and-ticks measurement runs (rollback's window).
const ROLLBACK_TICKS: usize = 8;

pub fn snapshots(args: &[String]) {
    let (dir, _, names, every) = args_dir(args);
    let name = names
        .first()
        .cloned()
        .unwrap_or_else(|| "fox-fox-fd".into());
    let every = every.unwrap_or(1000).max(1);
    println!("{}", crate::machine());
    println!("{}", version());
    println!();
    on_sim_thread(move || {
        let mut files = files();
        let b = match Bench::open(&dir, &name) {
            Ok(b) => b,
            Err(e) => {
                println!("{name}: {e}");
                return;
            }
        };
        let mut m = match catch_unwind(AssertUnwindSafe(|| b.start(&mut files))) {
            Ok(m) => m,
            Err(p) => {
                println!("{name}: stopped starting the match: {}", panic_text(&p));
                return;
            }
        };
        let mut device = Device(&b.replay);
        // The match's state every `every` ticks, as far as it runs.
        let mut kept: Vec<(u64, i32, Match)> = Vec::new();
        let mut ticks = 0u64;
        let stop = loop {
            if ticks > 0 && ticks.is_multiple_of(every) {
                kept.push((ticks, frame(&m), m.clone()));
            }
            match catch_unwind(AssertUnwindSafe(|| m.step_replay(&mut device))) {
                Ok(f) => {
                    ticks += 1;
                    if f.ended || frame(&m) >= b.last {
                        break format!("the match's end at frame {}", frame(&m));
                    }
                }
                Err(p) => break format!("frame {} (tick {ticks}): {}", frame(&m), panic_text(&p)),
            }
        };
        println!("{name}: ran {ticks} ticks, to {stop}");
        println!();
        println!(
            "| tick (frame) | GObjs | joints | snapshot | snapshot allocs | clone | restore | restore allocs | restore + {ROLLBACK_TICKS} ticks | allocs per tick |"
        );
        println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
        for (tick, f, snap) in &kept {
            let game = &snap.game;
            let (copy, allocs, bytes) = counted(|| game.clone());
            let size = std::mem::size_of::<Game>() + bytes;
            let clone_us = median_us(200, || {
                std::hint::black_box(game.clone());
            });
            let mut target = copy.clone();
            let restore_us = median_us(200, || {
                target.clone_from(std::hint::black_box(game));
            });
            let ((), restore_allocs, _) = counted(|| target.clone_from(game));
            // A restore, then the ticks after it: the rollback's work.
            let mut work = snap.clone();
            let mut tick_allocs = Vec::new();
            let rollback = catch_unwind(AssertUnwindSafe(|| {
                let us = median_us(5, || {
                    work.game.clone_from(game);
                    for _ in 0..ROLLBACK_TICKS {
                        std::hint::black_box(work.step_replay(&mut device));
                    }
                });
                work.game.clone_from(game);
                for _ in 0..ROLLBACK_TICKS {
                    let (_, n, _) = counted(|| work.step_replay(&mut device));
                    tick_allocs.push(n);
                }
                us
            }));
            let (rollback, per_tick) = match rollback {
                Ok(us) => (
                    format!("{:.2} ms", us / 1e3),
                    format!(
                        "{} (max {})",
                        tick_allocs.iter().sum::<usize>() / ROLLBACK_TICKS,
                        tick_allocs.iter().max().unwrap_or(&0)
                    ),
                ),
                Err(p) => (format!("stopped: {}", panic_text(&p)), "—".into()),
            };
            println!(
                "| {tick} ({f}) | {} | {} | {:.2} MB | {allocs} | {clone_us:.1} µs | {restore_us:.1} µs | {restore_allocs} | {rollback} | {per_tick} |",
                game.world.gobjs.used(),
                game.objs.jobjs.pool.used(),
                size as f64 / 1e6,
            );
        }
    });
}
