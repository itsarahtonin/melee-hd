//! `melee-bench corpus [--rules slippi|own] [--heap off|on] [--fp slippi|hardware|own]
//! [--jobs N] [--list FILE]... [PATH...]`: plays every replay named (a `.slp` file, or every
//! `.slp` under a directory; `--list` files name one path a line, `#` starting a comment) from
//! its start through `vs_match`'s replay path with the display pass, as `native` does, and
//! reports how each run went: to the replay's end, the match's own end, or a stop (code no port
//! covers, or a word melee-hd doesn't follow, by name), and the first frame whose fighters
//! weren't the replay's.
//!
//! It is the Slippi ruleset's run of the replay corpus ([the plan](docs/slippi-ruleset.md),
//! design call 8): by default every replay plays under Slippi 3.19's online rules
//! (`Rules::slippi_online`, whatever rules it was recorded with), in Slippi Dolphin's floating
//! point, with the allocator model off (no `Settings::heap`), as the player's Slippi matches
//! would run. A replay recorded under other rules leaves its recording (its desync column
//! says where) and plays on, its recorded inputs driving the 3.19 match: what counts is that
//! nothing stops. `--rules own` plays each under its own codes' rules instead, `--heap on`
//! with the model as replays have it, and `--fp own` in the console's floating point for a
//! replay with no Slippi codes (the benchmark matches), Slippi's for the rest.
//!
//! One line per replay, tab-separated, as each finishes: the path, its own rules, its last
//! frame, how far it ran, the first frame out of sync (or `-`), whether its own rules are
//! the ones it ran under, and the seconds it took; then a summary of the stops by message.

use std::collections::{BTreeMap, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use melee_data::disc::Disc;
use melee_sim::game::slippi::{CodeSet, parse_codes};
use melee_sim::game::state::Rules;
use melee_sim::game::vs_match::{DiscFile, Match, Settings};
use melee_sim::gekko_fp::{FpMode, set_fp_mode};
use melee_slippi::Device;

use crate::native::{fighters, frame};
use crate::sync::{FIRST_FRAME, post_frames, replay_last_frame, same};

/// The simulation recurses as the original does (joint trees): each job runs on a deep stack.
const STACK: usize = 512 << 20;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Fp {
    Slippi,
    Hardware,
    /// The console's for a replay with no Slippi codes, Slippi Dolphin's for the rest.
    Own,
}

struct Options {
    /// Slippi 3.19's online rules for every replay; else each replay's own.
    slippi: bool,
    heap: bool,
    fp: Fp,
    jobs: usize,
    paths: Vec<PathBuf>,
}

fn parse(args: &[String]) -> Options {
    let mut o = Options {
        slippi: true,
        heap: false,
        fp: Fp::Slippi,
        jobs: 1,
        paths: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().map(String::as_str).unwrap_or("");
        match a.as_str() {
            "--rules" => {
                o.slippi = match value() {
                    "slippi" => true,
                    "own" => false,
                    v => panic!("--rules slippi or own, not {v:?}"),
                }
            }
            "--heap" => {
                o.heap = match value() {
                    "on" => true,
                    "off" => false,
                    v => panic!("--heap on or off, not {v:?}"),
                }
            }
            "--fp" => {
                o.fp = match value() {
                    "slippi" => Fp::Slippi,
                    "hardware" => Fp::Hardware,
                    "own" => Fp::Own,
                    v => panic!("--fp slippi, hardware or own, not {v:?}"),
                }
            }
            "--jobs" => o.jobs = value().parse().expect("--jobs N"),
            "--list" => {
                let file = value();
                let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{file}: {e}"));
                for l in text.lines() {
                    let l = l.split('#').next().unwrap_or("").trim();
                    if !l.is_empty() {
                        o.paths.push(PathBuf::from(l));
                    }
                }
            }
            _ => o.paths.push(PathBuf::from(a)),
        }
    }
    o.jobs = o.jobs.max(1);
    o
}

/// Every `.slp` file `path` names: itself, or those under it, in name order.
fn replays_in(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for e in entries {
            if e.is_dir() || e.extension().is_some_and(|x| x == "slp") {
                replays_in(&e, out);
            }
        }
    } else {
        out.push(path.to_owned());
    }
}

/// A short name for a replay's own rules.
fn rules_name(r: &Rules) -> String {
    if *r == Rules::default() {
        return "vanilla".into();
    }
    let mut s = if r.slippi {
        match r.codes {
            CodeSet::V3_19 => "3.19",
            CodeSet::V3_16 => "3.16",
            CodeSet::V3_14 => "3.14",
        }
        .to_owned()
    } else {
        "codes".to_owned()
    };
    s.push_str(&format!(" {:?}", r.ucf));
    if r.player != Default::default() {
        s.push_str(" +players' codes");
    }
    s
}

/// What happened to one replay.
struct Outcome {
    own: String,
    last: i32,
    /// "end", "match end at frame N" or "stop at frame N (tick T): MESSAGE".
    result: String,
    /// The stop's message alone, for the summary.
    stop: Option<String>,
    desync: Option<i32>,
    same_rules: bool,
    secs: f64,
}

/// The disc's files, read once per job.
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

thread_local! {
    /// Where the last panic on this thread was raised, for the report.
    static WHERE: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    let text = p
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into());
    let at = WHERE.with(|w| w.borrow().clone());
    format!("{} [{at}]", text.replace(['\t', '\n'], " "))
}

fn play(o: &Options, files: &mut Files, path: &Path) -> Outcome {
    let t = Instant::now();
    let fail = |own: String, why: String| Outcome {
        own,
        last: FIRST_FRAME,
        result: format!("not started: {why}"),
        stop: Some(why),
        desync: None,
        same_rules: false,
        secs: t.elapsed().as_secs_f64(),
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return fail("-".into(), format!("reading it: {e}")),
    };
    let (original, replay) = match (
        ssbm_slippi::replay::Replay::parse(&bytes),
        melee_slippi::Replay::parse(&bytes),
    ) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) => return fail("-".into(), format!("parsing it: {e}")),
        (_, Err(e)) => return fail("-".into(), format!("parsing it: {e}")),
    };
    let mut start = melee_slippi::start(&replay);
    let own = Rules::from_codes(&parse_codes(&start.codes));
    let own_name = match &own {
        Ok(r) => rules_name(r),
        Err(e) => format!("refused ({e})"),
    };
    // A replay whose codes its own rules refuse plays under the forced rules without them.
    if own.is_err() {
        if !o.slippi {
            return fail(own_name, "its codes are refused".into());
        }
        start.codes.clear();
    }
    let mut settings = match Settings::from_replay(&start) {
        Ok(s) => s,
        Err(e) => return fail(own_name, format!("its settings: {e}")),
    };
    if o.slippi {
        settings.rules = Rules::slippi_online();
    }
    if !o.heap {
        settings.heap = None;
        settings.playback_heap = None;
    }
    let same_rules = own.as_ref().is_ok_and(|r| *r == settings.rules);
    let slippi_fp = match o.fp {
        Fp::Slippi => true,
        Fp::Hardware => false,
        Fp::Own => own.as_ref().map_or(true, |r| *r != Rules::default()),
    };
    set_fp_mode(if slippi_fp {
        FpMode::Slippi
    } else {
        FpMode::Hardware
    });
    let want = post_frames(&original);
    let last = replay_last_frame(&original);
    let mut m = match catch_unwind(AssertUnwindSafe(|| {
        Match::start(&settings, &mut |name| files.load(name))
    })) {
        Ok(m) => m,
        Err(p) => {
            let why = panic_text(&p);
            return Outcome {
                result: format!("stop starting the match: {why}"),
                ..fail(own_name, why)
            };
        }
    };
    let mut device = Device(&replay);
    // A replay that never reaches its last frame (the match ended under other rules) stops by
    // the match's end; a cap on ticks catches anything else.
    let cap = 2 * (last - FIRST_FRAME + 1) as u64 + 2000;
    let mut ticks = 0u64;
    let mut desync = None;
    let mut sampled = FIRST_FRAME - 1;
    let (result, stop) = loop {
        let step = catch_unwind(AssertUnwindSafe(|| {
            let (mut f, render) = m.step_replay_tick(&mut device);
            let now = frame(&m);
            let sample = (now != sampled).then(|| fighters(&m.game));
            if render {
                f.draws = m.render();
            }
            (f, now, sample)
        }));
        match step {
            Ok((f, now, sample)) => {
                ticks += 1;
                if let Some(got) = sample {
                    sampled = now;
                    if desync.is_none() && want.get(&now).is_some_and(|w| !same(&got, w)) {
                        desync = Some(now);
                    }
                }
                let ended = f.ended;
                m.recycle(f);
                if now >= last {
                    break ("end".to_owned(), None);
                }
                if ended {
                    break (format!("match end at frame {now}"), None);
                }
                if ticks > cap {
                    let why = format!("ran {ticks} ticks without reaching frame {last}");
                    break (format!("stop at frame {now}: {why}"), Some(why));
                }
            }
            Err(p) => {
                let why = panic_text(&p);
                break (
                    format!("stop at frame {} (tick {ticks}): {why}", frame(&m)),
                    Some(why),
                );
            }
        }
    };
    Outcome {
        own: own_name,
        last,
        result,
        stop,
        desync,
        same_rules,
        secs: t.elapsed().as_secs_f64(),
    }
}

/// A stop's message without what varies between replays (frame numbers, addresses), so the
/// summary groups like stops.
fn stop_kind(why: &str) -> String {
    why.split(" [").next().unwrap_or(why).to_owned()
}

pub fn main(args: &[String]) {
    let o = parse(args);
    let mut paths = Vec::new();
    for p in &o.paths {
        replays_in(p, &mut paths);
    }
    eprintln!(
        "{} replays, rules {}, allocator model {}, float mode {}, {} jobs",
        paths.len(),
        if o.slippi { "3.19 online" } else { "their own" },
        if o.heap { "on" } else { "off" },
        match o.fp {
            Fp::Slippi => "Slippi's",
            Fp::Hardware => "the console's",
            Fp::Own => "each replay's own",
        },
        o.jobs
    );
    std::panic::set_hook(Box::new(|info| {
        let at = info
            .location()
            .map_or(String::new(), |l| format!("{}:{}", l.file(), l.line()));
        WHERE.with(|w| *w.borrow_mut() = at);
    }));
    let disc_path = std::env::var("SSBM_DISC").expect("SSBM_DISC names the disc");
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Outcome)>> = Mutex::new(Vec::new());
    let out = Mutex::new(());
    let t = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..o.jobs {
            std::thread::Builder::new()
                .stack_size(STACK)
                .spawn_scoped(s, || {
                    let mut files = Files {
                        disc: Disc::open(&disc_path).unwrap_or_else(|e| panic!("{disc_path}: {e}")),
                        cache: HashMap::new(),
                    };
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(i) else { break };
                        let r = play(&o, &mut files, path);
                        {
                            let _l = out.lock().unwrap();
                            println!(
                                "{}\t{}\t{}\t{}\t{}\t{}\t{:.1}",
                                path.display(),
                                r.own,
                                r.last,
                                r.result,
                                r.desync.map_or("-".into(), |f| f.to_string()),
                                if r.same_rules { "own" } else { "forced" },
                                r.secs
                            );
                        }
                        results.lock().unwrap().push((i, r));
                    }
                    set_fp_mode(FpMode::Hardware);
                })
                .unwrap();
        }
    });
    let results = results.into_inner().unwrap();
    let ended = results
        .iter()
        .filter(|(_, r)| r.result == "end" || r.result.starts_with("match end"))
        .count();
    let in_sync = results
        .iter()
        .filter(|(_, r)| r.same_rules && r.stop.is_none() && r.desync.is_none())
        .count();
    let same = results.iter().filter(|(_, r)| r.same_rules).count();
    let mut stops: BTreeMap<String, Vec<&Path>> = BTreeMap::new();
    for (i, r) in &results {
        if let Some(why) = &r.stop {
            stops.entry(stop_kind(why)).or_default().push(&paths[*i]);
        }
    }
    println!();
    println!(
        "# {} replays in {:.0} s: {ended} ran to their ends, {} stopped; {in_sync} of the {same} run under their own rules stayed in sync",
        results.len(),
        t.elapsed().as_secs_f64(),
        results.len() - ended,
    );
    for (why, which) in &stops {
        println!("# {} x {why}", which.len());
        for p in which.iter().take(5) {
            println!("#     {}", p.display());
        }
    }
}
