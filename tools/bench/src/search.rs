//! `melee-bench seed-search MATCH FROM COUNT [--jobs N] [--codes REPLAY.slp] [--record DIR]
//! [--watch KIND[:STATE]|caught]...`: plays an oracle set's scripted match (every player a walker or an
//! opener, so no CPU AI plays) with the seeds `FROM` to `FROM + COUNT - 1`, headless, and prints
//! how far each got: to the match's end, or the unported code it stopped at. It finds the seeds
//! whose matches reach something rare, such as a container's monster, which ssbm-rs then records
//! and traces as the oracle (`docs/oracle.md`, "Oracle sets"). Once the rare thing is ported,
//! `--watch` names it instead: an item of kind `KIND` (in state `STATE`, if given), or with
//! `caught` a container's monster a catch box took (Kirby's inhale or Yoshi's tongue holding it),
//! each match printing the frame each watched item first appears and stopping once all have.
//!
//! Each match is played as Slippi's playback plays its recording would be: the match's
//! `StartMeleeData` and seed as the generator's codes set them, the code list of a recording
//! `make-oracle.sh` made (`--codes`, by default the `items-held-ps` oracle match's), and each
//! frame's inputs as the recording run's controllers would leave them (`Match::scripted_pad`,
//! the processed sticks and buttons the game makes of them), but with no frame's seed restored:
//! the random seed runs free, as it does in the recording run. `--record DIR` writes each
//! match's recording (`DIR/SEED.slp`), which `melee-oracle compare` checks against ssbm-rs's.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::SystemTime;

use melee_data::disc::Disc;
use melee_sim::game::playback::{CharacterFrame, FIRST_FRAME, Frame, Source};
use melee_sim::game::vs_match::{DiscFile, Match as Sim, ReplayStart, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_slippi::Replay;

use crate::matches::{self, Match, Rules};

/// The simulation recurses as the original does (joint trees): it runs on a deep stack.
const STACK: usize = 512 << 20;

/// A scripted match's controllers as playback would inject them: nothing before "GO", then each
/// scripted player's pad, to past the time limit.
struct Script<'a> {
    m: &'a Match,
    last: i32,
}

/// The processed inputs the game makes of a scripted pad (buttons and the main stick, which the
/// scripts hold at 0 or past ±80 on each axis, where it reads ±1 and sets its direction's
/// button). Z is also A and a shoulder pressed at the common data's light press (0.35), as
/// `Fighter_procInput` makes it before playback records the inputs. R is a shoulder pressed all the
/// way (the trigger at 1.0, which a digital press means).
pub fn processed(button: u16, stick_x: i8, stick_y: i8) -> CharacterFrame {
    let axis = |v: i8, plus: u32, minus: u32| match v {
        0 => (0.0, 0),
        80..=127 => (1.0, plus),
        -128..=-80 => (-1.0, minus),
        _ => panic!("a script's stick at {v}, neither neutral nor past 80"),
    };
    let (x, right_left) = axis(stick_x, 0x8_0000, 0x4_0000);
    let (y, up_down) = axis(stick_y, 0x1_0000, 0x2_0000);
    let (z, trigger) = if button & 0x20 != 0 {
        (0x8000_0000, 1.0)
    } else if button & 0x10 != 0 {
        (0x8000_0100, 0.35)
    } else {
        (0, 0.0)
    };
    CharacterFrame {
        stick_x: x,
        stick_y: y,
        trigger,
        buttons: u32::from(button) | right_left | up_down | z,
        raw_stick_x: stick_x as u8,
        raw_stick_y: stick_y as u8,
        ..CharacterFrame::default()
    }
}

impl Source for Script<'_> {
    fn frame(&mut self, index: i32) -> Frame {
        if index > self.last {
            return Frame::terminate();
        }
        let chars = std::array::from_fn(|port| {
            if port >= self.m.players.len() {
                return [None, None];
            }
            let (button, stick_x, stick_y) = if index < 0 {
                (0, 0, 0)
            } else {
                self.m.scripted_pad(port, index).expect("a scripted player")
            };
            [Some(processed(button, stick_x, stick_y)), None]
        });
        Frame::new(None, chars)
    }

    fn has_player(&mut self, _index: i32, port: u8) -> bool {
        usize::from(port & 3) < self.m.players.len()
    }
}

/// How one seed's match went.
struct Outcome {
    seed: u32,
    frame: i32,
    stopped: Option<String>,
    /// The frame each watched item first appeared, if it did.
    seen: Vec<Option<i32>>,
}

/// An item to watch for: its kind, and its state if one is named; kind [`CAUGHT`] for a caught
/// monster.
type Watch = (i32, Option<i32>);

/// The watch for a container's monster a catch box took.
const CAUGHT: i32 = -1;

fn parse_watch(s: &str) -> Option<Watch> {
    if s == "caught" {
        return Some((CAUGHT, None));
    }
    match s.split_once(':') {
        Some((k, st)) => Some((k.parse().ok()?, Some(st.parse().ok()?))),
        None => Some((s.parse().ok()?, None)),
    }
}

/// The disc's files, read once per thread.
struct Files {
    disc: Disc,
    cache: HashMap<String, (i32, Vec<u8>)>,
}

impl Files {
    fn load(&mut self, name: &str) -> DiscFile {
        let disc = &self.disc;
        let (entrynum, bytes) = self.cache.entry(name.to_owned()).or_insert_with(|| {
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
    }
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// Plays `m` to its end or its first stop, or until every item in `watch` has appeared.
fn play(
    m: &Match,
    codes: &Replay,
    files: &mut Files,
    record: Option<&Path>,
    watch: &[Watch],
) -> Outcome {
    let base = melee_slippi::start(codes);
    let start = ReplayStart {
        info: m.start_data(),
        seed: m.seed,
        nametags: base.nametags,
        codes: base.codes,
    };
    let mut settings = Settings::from_replay(&start).expect("the oracle codes' rules");
    if record.is_some() {
        settings.record = Some(melee_slippi::write::setup(codes));
    }
    let limit = match m.rules {
        Rules::Stock { time_limit, .. }
        | Rules::Time { time_limit }
        | Rules::Coin { time_limit } => time_limit,
    };
    let mut source = Script {
        m,
        last: 60 * limit as i32 + 600,
    };
    let frame = |s: &Sim| {
        s.game
            .playback
            .as_ref()
            .map_or(FIRST_FRAME - 1, |p| p.frame_index)
    };
    let started_at = SystemTime::now();
    let mut sim = match catch_unwind(AssertUnwindSafe(|| {
        Sim::start(&settings, &mut |name| files.load(name))
    })) {
        Ok(s) => s,
        Err(p) => {
            return Outcome {
                seed: m.seed,
                frame: FIRST_FRAME - 1,
                stopped: Some(format!("starting the match: {}", panic_text(&p))),
                seen: vec![None; watch.len()],
            };
        }
    };
    let mut stopped = None;
    let mut seen = vec![None; watch.len()];
    loop {
        match catch_unwind(AssertUnwindSafe(|| sim.step_replay(&mut source))) {
            Ok(out) => {
                let ended = out.ended;
                sim.recycle(out);
                let f = frame(&sim);
                for (w, at) in watch.iter().zip(seen.iter_mut()) {
                    if at.is_none()
                        && sim.game.items.iter().any(|(_, ip)| {
                            ip.entity.is_some()
                                && if w.0 == CAUGHT {
                                    (43..=46).contains(&ip.kind) && ip.grab_victim.is_some()
                                } else {
                                    ip.kind == w.0 && w.1.is_none_or(|st| ip.msid == st)
                                }
                        })
                    {
                        *at = Some(f);
                    }
                }
                let all_seen = !watch.is_empty() && seen.iter().all(Option::is_some);
                if ended || all_seen || f > source.last {
                    break;
                }
            }
            Err(p) => {
                stopped = Some(panic_text(&p));
                break;
            }
        }
    }
    if let (Some(path), Some(rec)) = (record, sim.game.recorder.as_ref()) {
        let file = melee_slippi::write::recording(rec, started_at);
        std::fs::write(path, file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    Outcome {
        seed: m.seed,
        frame: frame(&sim),
        stopped,
        seen,
    }
}

/// `melee-bench record-match MATCH OUT.slp [--codes REPLAY.slp]`: records an oracle set's
/// scripted match with its own seed, as [`main`] plays it, to `OUT.slp`; it fails if the match
/// stops short of its end. ssbm-rs's playback of the recording is then the oracle
/// (`make-oracle.sh` with `RECORDER=melee-hd`).
pub fn record(args: &[String]) -> ExitCode {
    let mut codes = None;
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--codes" => codes = it.next().map(PathBuf::from),
            _ => pos.push(a.clone()),
        }
    }
    let [name, out] = &pos[..] else {
        eprintln!("usage: melee-bench record-match MATCH OUT.slp [--codes REPLAY.slp]");
        return ExitCode::from(2);
    };
    let m = match scripted(name) {
        Ok(m) => m,
        Err(code) => return code,
    };
    let codes = load_codes(codes);
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let out = PathBuf::from(out);
    std::panic::set_hook(Box::new(|_| {}));
    let o = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK)
            .spawn_scoped(s, || {
                set_fp_mode(FpMode::Hardware);
                let mut files = Files {
                    disc: Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}")),
                    cache: HashMap::new(),
                };
                play(&m, &codes, &mut files, Some(&out), &[])
            })
            .unwrap()
            .join()
            .unwrap()
    });
    match o.stopped {
        None => {
            println!("{name}: recorded to frame {} in {}", o.frame, out.display());
            ExitCode::SUCCESS
        }
        Some(why) => {
            println!("{name}: stopped at frame {}: {why}", o.frame);
            ExitCode::FAILURE
        }
    }
}

/// The oracle set's match `name`, if every player follows a script.
fn scripted(name: &str) -> Result<Match, ExitCode> {
    let m = match matches::select(&[name.to_owned()]) {
        Ok(mut list) if list.len() == 1 => list.remove(0),
        Ok(_) => {
            eprintln!("{name}: name one match, not a set");
            return Err(ExitCode::from(2));
        }
        Err(e) => {
            eprintln!("{e}");
            return Err(ExitCode::from(2));
        }
    };
    if (0..m.players.len()).any(|i| m.scripted_pad(i, 0).is_none()) {
        eprintln!("{name}: a player no script plays (the CPU AI isn't ported)");
        return Err(ExitCode::from(2));
    }
    Ok(m)
}

/// The recording whose code list the matches run with: `--codes`, or the `items-held-ps`
/// oracle match's.
fn load_codes(codes: Option<PathBuf>) -> Replay {
    let codes = codes.unwrap_or_else(|| {
        melee_local_data::ssbm_rs_local("bench/oracle-m4/items-held-ps.slp").into()
    });
    let bytes = std::fs::read(&codes).unwrap_or_else(|e| panic!("{}: {e}", codes.display()));
    Replay::parse(&bytes).unwrap_or_else(|e| panic!("{e}"))
}

fn parse_seed(s: &str) -> Option<u32> {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(&h.replace('_', ""), 16).ok(),
        None => s.parse().ok(),
    }
}

pub fn main(args: &[String]) -> ExitCode {
    let mut jobs = 4usize;
    let mut codes = None;
    let mut record = None;
    let mut watch = Vec::new();
    let mut pos = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--jobs" => jobs = it.next().and_then(|n| n.parse().ok()).expect("--jobs N"),
            "--codes" => codes = it.next().map(PathBuf::from),
            "--record" => record = it.next().map(PathBuf::from),
            "--watch" => watch.push(
                it.next()
                    .and_then(|w| parse_watch(w))
                    .expect("--watch KIND[:STATE]"),
            ),
            _ => pos.push(a.clone()),
        }
    }
    let (Some(name), Some(from), Some(count)) = (
        pos.first(),
        pos.get(1).and_then(|s| parse_seed(s)),
        pos.get(2).and_then(|s| s.parse::<u32>().ok()),
    ) else {
        eprintln!(
            "usage: melee-bench seed-search MATCH FROM COUNT [--jobs N] [--codes REPLAY.slp] [--record DIR] [--watch KIND[:STATE]|caught]..."
        );
        return ExitCode::from(2);
    };
    let template = match scripted(name) {
        Ok(m) => m,
        Err(code) => return code,
    };
    let codes = load_codes(codes);
    if let Some(dir) = &record {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    }
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    // The stops are the results: keep the panics' own reports quiet.
    std::panic::set_hook(Box::new(|_| {}));
    let next = AtomicU32::new(0);
    let results = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            let (template, codes, record, next, results, disc_path, watch) = (
                &template, &codes, &record, &next, &results, &disc_path, &watch,
            );
            std::thread::Builder::new()
                .stack_size(STACK)
                .spawn_scoped(s, move || {
                    set_fp_mode(FpMode::Hardware);
                    let mut files = Files {
                        disc: Disc::open(disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}")),
                        cache: HashMap::new(),
                    };
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= count {
                            break;
                        }
                        let m = Match {
                            seed: from.wrapping_add(i),
                            name: template.name.clone(),
                            players: template.players.clone(),
                            ..*template
                        };
                        let path = record
                            .as_ref()
                            .map(|d| d.join(format!("{:08x}.slp", m.seed)));
                        let o = play(&m, codes, &mut files, path.as_deref(), watch);
                        let seen: String = watch
                            .iter()
                            .zip(&o.seen)
                            .filter_map(|(w, at)| {
                                let st = w.1.map_or(String::new(), |s| format!(":{s}"));
                                at.map(|f| format!("\t{}{st} at {f}", w.0))
                            })
                            .collect();
                        println!(
                            "{:#010x}\t{}\t{}{seen}",
                            o.seed,
                            o.frame,
                            o.stopped.as_deref().unwrap_or("played to its end")
                        );
                        results.lock().unwrap().push(o);
                    }
                })
                .unwrap();
        }
    });
    let results = results.into_inner().unwrap();
    let ended = results.iter().filter(|o| o.stopped.is_none()).count();
    println!(
        "{name}: {} seeds, {ended} played to their end, {} stopped",
        results.len(),
        results.len() - ended
    );
    ExitCode::SUCCESS
}
