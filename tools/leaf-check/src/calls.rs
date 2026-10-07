//! Ports checked against ssbm-rs from real game states: every saved call of a function in a
//! capture directory (`NAME-call*.call`, from ssbm-rs's `CAPTURE=DIR CAPTURE_FUNCS=NAME`) is
//! imported into melee-sim, run on both sides from the same state, and the whole state compared
//! afterwards. A call our port can't finish because it reaches a function no port covers yet
//! (`game::code` panics naming it) is counted as skipped, never as compared.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use gekko_fp::{FpMode, set_fp_mode};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps};
use melee_capture::{Capture, DataMap, DiscIndex, find_archives_after, find_archives_in};
use melee_data::archive::Archives;
use melee_sim::game::state::Running;
use ssbm_rt::capture::Call;

use crate::{MODES, disc_index, reference, run_capture};

/// What ran on our side: the imported game, the maps from ssbm-rs's addresses to our handles,
/// and the call (its registers hold the arguments). Returns what the function returns, which
/// is compared with the reference's r3 or f1.
pub type Ours<'a> = dyn Fn(&mut Running, &Maps, &Call) -> Ret + Sync + 'a;

/// A function's result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ret {
    Nothing,
    /// In r3.
    Int(u32),
    /// In f1, as the register holds it.
    Float(f64),
    /// An effect, a generator or null in r3 (`efSync_Spawn`'s `void*`), paired by the
    /// comparison.
    Spawned(melee_sim::game::ef::Spawned),
}

/// How a set of saved calls went.
#[derive(Debug, Default)]
pub struct Report {
    pub compared: usize,
    /// The calls that compared clean, with the function they ran (a prepare may change it).
    pub clean: Vec<(PathBuf, u32)>,
    /// Calls that reached an unported function, with what it was.
    pub skipped: Vec<(String, String)>,
    pub failed: Vec<String>,
}

/// The saved calls of `func` in `dir`, or none when the directory isn't there.
pub fn saved_calls(dir: &str, func: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let prefix = format!("{func}-call");
    let mut v: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".call"))
        })
        .collect();
    v.sort();
    v
}

/// Checks `ours` against the reference on every saved call of `func` in `dir`, in both float
/// modes. Fails on any difference, any panic other than reaching an unported function, and
/// when fewer than `min_compared` calls (per mode) were compared. Skips without `SSBM_DISC` or
/// without the directory.
pub fn check_calls(dir: &str, func: &str, min_compared: usize, ours: &Ours) {
    check_calls_with(dir, func, min_compared, &|_, _| {}, ours);
}

/// Changes a saved call before both sides run it: its function (`call.addr`, to run a callee
/// from its caller's state), registers or memory, so branches real calls rarely take run
/// from real states. Gets the call's index among the directory's calls, to vary inputs.
pub type Prepare<'a> = dyn Fn(&mut Call, usize) + Sync + 'a;

/// [`check_calls`], with each call changed by `prepare` first.
pub fn check_calls_with(
    dir: &str,
    func: &str,
    min_compared: usize,
    prepare: &Prepare,
    ours: &Ours,
) {
    quiet_unported_panics();
    let paths = saved_calls(dir, func);
    let Some(index) = disc_index() else {
        eprintln!("SSBM_DISC is unset: skipping {func}'s saved calls");
        return;
    };
    if paths.is_empty() {
        eprintln!("no saved calls of {func} in {dir}: skipping");
        return;
    }
    for mode in MODES {
        let report = run_calls(&paths, index, mode, prepare, ours);
        let mut skipped: std::collections::BTreeMap<&str, usize> = Default::default();
        for (_, why) in &report.skipped {
            *skipped.entry(why.as_str()).or_default() += 1;
        }
        eprintln!(
            "{func} ({mode:?}): {} of {} calls compared clean, {} skipped",
            report.compared,
            paths.len(),
            report.skipped.len()
        );
        for (why, n) in &skipped {
            eprintln!("  skipped x{n}: {why}");
        }
        for f in &report.failed {
            eprintln!("{f}");
        }
        // CALLS_CLEAN=DIR lists the clean calls per function, to see what they reach
        // (`reach`).
        if let Ok(out) = std::env::var("CALLS_CLEAN") {
            let list: Vec<String> = report
                .clean
                .iter()
                .map(|(p, addr)| format!("{}@{addr:x}", p.display()))
                .collect();
            let _ = std::fs::write(format!("{out}/{func}-{mode:?}.txt"), list.join("\n"));
        }
        assert!(
            report.failed.is_empty(),
            "{func} ({mode:?}): {} saved calls differ",
            report.failed.len()
        );
        assert!(
            report.compared >= min_compared,
            "{func} ({mode:?}): only {} saved calls compared, expected at least {min_compared}",
            report.compared
        );
    }
}

/// Runs the calls at `paths` on worker threads (each with its own reference) in `mode`.
pub fn run_calls(
    paths: &[PathBuf],
    index: &DiscIndex,
    mode: FpMode,
    prepare: &Prepare,
    ours: &Ours,
) -> Report {
    let next = AtomicUsize::new(0);
    let report = Mutex::new(Report::default());
    let workers = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(8)
        .min(paths.len());
    std::thread::scope(|s| {
        for _ in 0..workers {
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn_scoped(s, || {
                    set_fp_mode(mode);
                    let ctx = reference().expect("SSBM_DISC");
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(i) else { break };
                        let outcome = run_one(&ctx, path, i, index, prepare, ours);
                        let name = path.file_name().unwrap().to_string_lossy().into_owned();
                        let mut r = report.lock().unwrap();
                        match outcome {
                            Outcome::Clean(addr) => {
                                r.compared += 1;
                                r.clean.push((path.clone(), addr));
                            }
                            Outcome::Skipped(why) => r.skipped.push((name, why)),
                            Outcome::Failed(why) => r.failed.push(format!("{name}: {why}")),
                        }
                    }
                })
                .unwrap();
        }
    });
    report.into_inner().unwrap()
}

enum Outcome {
    /// Compared clean, running the function at this address.
    Clean(u32),
    Skipped(String),
    Failed(String),
}

/// The text of a panic's payload.
fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".to_owned())
}

fn run_one(
    ctx: &ssbm_rt::Ctx,
    path: &Path,
    i: usize,
    index: &DiscIndex,
    prepare: &Prepare,
    ours: &Ours,
) -> Outcome {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut cap = Capture::read(path);
    prepare(&mut cap.call, i);
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives_in(index, &cap.call.mem1, &mut archives),
    };
    let bank = melee_capture::audio::find_bank(&cap.call.mem1, &mut archives);
    let mut imp = Importer::new(&cap, &data).with_archives(&archives);
    imp.world();
    imp.stage();
    imp.ids();
    imp.audio(bank);
    let Importer { mut game, maps, .. } = imp;
    // The imported state must be memory before anything runs, or differences afterwards
    // wouldn't be the call's.
    let mut before = Comparer::new(&cap.call.mem1, &data, &game, maps.clone());
    before.world();
    before.stage();
    before.ids();
    before.audio();
    if !before.diffs.is_empty() {
        return Outcome::Failed(format!(
            "the import doesn't round-trip: {}",
            before.diffs[..before.diffs.len().min(10)].join("; ")
        ));
    }
    let after = match catch_unwind(AssertUnwindSafe(|| run_capture(ctx, &cap.call))) {
        Ok(after) => after,
        Err(p) => {
            let why = p
                .downcast_ref::<ssbm_rt::Fault>()
                .map(|f| format!("a fault at {:#x}", f.addr))
                .unwrap_or_else(|| panic_text(&*p));
            return Outcome::Skipped(format!("the reference stopped: {why}"));
        }
    };
    let (r3, f1) = (ctx.regs.r(3), ctx.regs.f(1));
    let run = catch_unwind(AssertUnwindSafe(|| {
        let mut running = Running {
            game: &mut game,
            data: &archives,
        };
        ours(&mut running, &maps, &cap.call)
    }));
    let ret = match run {
        Ok(ret) => ret,
        Err(p) => {
            let why = panic_text(&*p);
            return if why.contains("isn't ported") {
                Outcome::Skipped(why)
            } else {
                Outcome::Failed(format!("our port panicked: {why}"))
            };
        }
    };
    // A fighter that changed animation loaded another archive where its last one was.
    let data = DataMap {
        loaded: find_archives_after(index, &after, &mut archives, &data.loaded),
    };
    let mut cmp = Comparer::new(&after, &data, &game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.audio();
    match ret {
        Ret::Nothing => {}
        Ret::Int(v) if v != r3 => cmp
            .diffs
            .push(format!("returned r3: ours {v:#x}, theirs {r3:#x}")),
        Ret::Float(v) if v.to_bits() != f1.to_bits() => cmp
            .diffs
            .push(format!("returned f1: ours {v:?}, theirs {f1:?}")),
        Ret::Spawned(s) => {
            if let Some(d) = spawned_diff(&cmp.maps, s, r3) {
                cmp.diffs.push(d);
            }
        }
        _ => {}
    }
    if cmp.diffs.is_empty() {
        Outcome::Clean(cap.call.addr)
    } else {
        Outcome::Failed(format!(
            "{} differences:\n    {}",
            cmp.diffs.len(),
            cmp.diffs[..cmp.diffs.len().min(20)].join("\n    ")
        ))
    }
}

/// The GObj argument in r3, as our handle.
pub fn gobj_arg(maps: &Maps, call: &Call) -> melee_sim::hsd::gobj::GObjId {
    let addr = call.regs.gpr[3];
    *maps
        .gobjs
        .get(&addr)
        .unwrap_or_else(|| panic!("r3 {addr:#x} isn't a GObj the import knows"))
}

/// Leaves panics from reaching unported functions out of the test output: they're counted
/// as skipped calls instead.
fn quiet_unported_panics() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let text = info
                .payload()
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| info.payload().downcast_ref::<&str>().copied())
                .unwrap_or("");
            if !text.contains("isn't ported") {
                default(info);
            }
        }));
    });
}

fn at(addr: u32) -> usize {
    (addr - 0x8000_0000) as usize
}

pub fn peek_u32(call: &Call, addr: u32) -> u32 {
    let i = at(addr);
    u32::from_be_bytes(call.mem1[i..i + 4].try_into().unwrap())
}

pub fn poke_u32(call: &mut Call, addr: u32, v: u32) {
    let i = at(addr);
    call.mem1[i..i + 4].copy_from_slice(&v.to_be_bytes());
}

pub fn poke_f32(call: &mut Call, addr: u32, v: f32) {
    poke_u32(call, addr, v.to_bits());
}

/// Sets bit `bit` (0 the most significant, as the compiler allocates bitfields) of the byte
/// at `addr`.
pub fn poke_bit(call: &mut Call, addr: u32, bit: u32, v: bool) {
    let i = at(addr);
    let mask = 0x80u8 >> bit;
    if v {
        call.mem1[i] |= mask;
    } else {
        call.mem1[i] &= !mask;
    }
}

/// The user data of the GObj in r3: the fighter (or item) a process runs on.
pub fn user_data_arg(call: &Call) -> u32 {
    peek_u32(call, call.regs.gpr[3] + 0x2C)
}

/// Whether a spawn's result pairs with the pointer ssbm-rs's returned, after a comparison.
pub fn spawned_diff(maps: &Maps, s: melee_sim::game::ef::Spawned, r3: u32) -> Option<String> {
    use melee_sim::game::ef::Spawned;
    let same = match s {
        Spawned::None => r3 == 0,
        Spawned::Effect(e) => maps.effects.get(&r3) == Some(&e),
        Spawned::Generator(g) => maps.generators.get(&r3) == Some(&g),
    };
    (!same).then(|| format!("returned: ours {s:?}, theirs {r3:#x}"))
}
