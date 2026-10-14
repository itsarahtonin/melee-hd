//! Ports checked against ssbm-rs from real game states: every saved call of a function in a
//! capture directory (`NAME-call*.call`, from ssbm-rs's `CAPTURE=DIR CAPTURE_FUNCS=NAME`) is
//! imported into melee-sim, run on both sides from the same state, and the whole state compared
//! afterwards. A call our port can't finish because it reaches a function no port covers yet
//! (`game::code` panics naming it) is counted as skipped, never as compared.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::fp::{FpMode, set_fp_mode};
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

/// How a call's run says it can't compare: a panic starting with this skips the call, as an
/// unported function does.
pub const SKIP: &str = "skip: ";

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

/// The saved calls of `func` in `dir` (files or its pack's), sorted, or none when the
/// directory isn't there.
pub fn saved_calls(dir: &str, func: &str) -> Vec<PathBuf> {
    let prefix = format!("{func}-call");
    let mut v = melee_capture::pack::call_files(dir);
    v.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(&prefix))
    });
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
    check_calls_in(dir, func, min_compared, prepare, &Setup::default(), ours);
}

/// What a check's reference needs besides ssbm-rs's ports: functions both sides stand
/// something in for (each address with what it returns in r3), and a Slippi replay whose
/// device answers what the calls ask (`ssbm_slippi::install`).
#[derive(Default)]
pub struct Setup<'a> {
    pub stubs: &'a [(u32, Option<u32>)],
    pub replay: Option<&'a ssbm_slippi::Replay>,
    /// The reference's hardware registers take writes and read 0, for code that draws: the
    /// GX commands it writes to the write-gather pipe are the presentation's.
    pub gx: bool,
}

/// Hardware registers that take writes and read 0.
struct NullHardware;

impl ssbm_rt::Mmio for NullHardware {
    fn read(&self, _: &ssbm_rt::Ctx, _: u32, _: u32) -> u32 {
        0
    }
    fn write(&self, _: &ssbm_rt::Ctx, _: u32, _: u32, _: u32) {}
}

/// [`check_calls_with`] with `setup`'s stand-ins and device.
pub fn check_calls_in(
    dir: &str,
    func: &str,
    min_compared: usize,
    prepare: &Prepare,
    setup: &Setup,
    ours: &Ours,
) {
    run_checks(dir, func, min_compared, &[], prepare, setup, ours);
}

/// A stand-in for an unported function, on both sides: its address and what it returns. The
/// calls each side makes to stand-ins are compared, in order, with their arguments.
pub type Stub = (u32, StubRet);

/// What a stand-in returns.
#[derive(Clone, Copy, Debug)]
pub enum StubRet {
    Nothing,
    Int(i32),
    Float(f32),
}

/// [`check_calls_with`], with `stubs` standing in for unported code on both sides and their
/// calls compared with their arguments; returns each float mode's report.
pub fn check_calls_stubbed(
    dir: &str,
    func: &str,
    min_compared: usize,
    stubs: &[Stub],
    prepare: &Prepare,
    ours: &Ours,
) -> Vec<Report> {
    run_checks(
        dir,
        func,
        min_compared,
        stubs,
        prepare,
        &Setup::default(),
        ours,
    )
}

fn run_checks(
    dir: &str,
    func: &str,
    min_compared: usize,
    stubs: &[Stub],
    prepare: &Prepare,
    setup: &Setup,
    ours: &Ours,
) -> Vec<Report> {
    quiet_unported_panics();
    let paths = saved_calls(dir, func);
    let Some(index) = disc_index() else {
        eprintln!("SSBM_DISC is unset: skipping {func}'s saved calls");
        return Vec::new();
    };
    if paths.is_empty() {
        eprintln!("no saved calls of {func} in {dir}: skipping");
        return Vec::new();
    }
    let mut reports = Vec::new();
    for mode in MODES {
        let report = run_calls(&paths, index, mode, stubs, prepare, setup, ours);
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
        reports.push(report);
    }
    reports
}

/// Runs the calls at `paths` on worker threads (each with its own reference) in `mode`.
pub fn run_calls(
    paths: &[PathBuf],
    index: &DiscIndex,
    mode: FpMode,
    stubs: &[Stub],
    prepare: &Prepare,
    setup: &Setup,
    ours: &Ours,
) -> Report {
    let next = AtomicUsize::new(0);
    let report = Mutex::new(Report::default());
    let workers = crate::workers().min(paths.len());
    std::thread::scope(|s| {
        for _ in 0..workers {
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn_scoped(s, || {
                    set_fp_mode(mode);
                    let ctx = reference().expect("SSBM_DISC");
                    if setup.gx {
                        ctx.set_mmio(Box::new(NullHardware));
                    }
                    if let Some(r) = setup.replay {
                        let _ = ssbm_slippi::install(&ctx, r.clone());
                    }
                    STUBS.with(|s| *s.borrow_mut() = setup.stubs.to_vec());
                    melee_sim::game::code::stubs::clear();
                    for &(addr, ret) in setup.stubs {
                        ctx.register(addr, their_stub);
                        let v = ret.map_or(melee_sim::game::code::Val::None, |v| {
                            melee_sim::game::code::Val::Int(v as i32)
                        });
                        melee_sim::game::code::stubs::set(addr, v);
                    }
                    install_logged_stubs(&ctx, stubs);
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = paths.get(i) else { break };
                        let outcome =
                            run_one(&ctx, path, i, index, prepare, ours, !stubs.is_empty());
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
    logged: bool,
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
    // The player blocks, pads, scene and match state, and in a replay run Slippi's playback
    // with the ruleset's state: playback's codes patch functions the reference runs patched.
    imp.match_state();
    // The item code's globals: what a fighter's items count and track.
    imp.items();
    imp.display();
    let Importer { mut game, maps, .. } = imp;
    // The imported state must be memory before anything runs, or differences afterwards
    // wouldn't be the call's.
    let mut before = Comparer::new(&cap.call.mem1, &data, &game, maps.clone());
    before.world();
    before.stage();
    before.ids();
    before.audio();
    before.match_state();
    before.items();
    before.display();
    if !before.diffs.is_empty() {
        return Outcome::Failed(format!(
            "the import doesn't round-trip: {}",
            before.diffs[..before.diffs.len().min(10)].join("; ")
        ));
    }
    CALLED.with(|c| c.borrow_mut().clear());
    THEIR_CALLS.with(|c| c.borrow_mut().clear());
    let _ = melee_sim::game::code::stubs::take();
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
            return if why.contains("isn't ported") || why.starts_with(SKIP) {
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
    // The stand-ins each side called, in order: with their arguments for logged stand-ins.
    let ours_log = melee_sim::game::code::stubs::take();
    let theirs_logged = THEIR_CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if logged {
        cmp.diffs
            .extend(stub_call_diffs(&cmp.maps, &ours_log, &theirs_logged));
    }
    let ours_called: Vec<u32> = ours_log.iter().map(|c| c.0).collect();
    let theirs_called = CALLED.with(|c| std::mem::take(&mut *c.borrow_mut()));
    if !logged && ours_called != theirs_called {
        cmp.diffs.push(format!(
            "stand-ins called: ours {ours_called:x?}, theirs {theirs_called:x?}"
        ));
    }
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.audio();
    cmp.match_state();
    cmp.items();
    cmp.display();
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
            cmp.diffs[..cmp
                .diffs
                .len()
                .min(std::env::var("CALLS_DIFFS").map_or(20, |v| v.parse().unwrap_or(20)))]
                .join("\n    ")
        ))
    }
}

thread_local! {
    /// The reference's stand-ins on this thread: address and what it returns in r3.
    static STUBS: std::cell::RefCell<Vec<(u32, Option<u32>)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// The stand-ins the reference called, in order.
    static CALLED: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The reference's stand-in at every stubbed address: returns what [`STUBS`] says.
fn their_stub(ctx: &ssbm_rt::Ctx) {
    let addr = *ctx.native_stack().last().expect("a stub runs as a native");
    CALLED.with(|c| c.borrow_mut().push(addr));
    if let Some(Some(v)) = STUBS.with(|s| s.borrow().iter().find(|s| s.0 == addr).map(|s| s.1)) {
        ctx.regs.set_r(3, v);
    }
}

/// A call the reference made to a stand-in: its address, r3 to r10, f1 to f4, and the three
/// floats each of r3 to r10 points to when it holds an address in main memory (a vector passed
/// by pointer).
type TheirCall = (u32, [u32; 8], [f64; 4], [Option<[u32; 3]>; 8]);

thread_local! {
    static THEIR_LOGGED_STUBS: std::cell::RefCell<Vec<Stub>> = const { std::cell::RefCell::new(Vec::new()) };
    static THEIR_CALLS: std::cell::RefCell<Vec<TheirCall>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Registers logged stand-ins on the reference and on our side, for this thread (after
/// [`run_calls`] has cleared ours).
fn install_logged_stubs(ctx: &ssbm_rt::Ctx, stubs: &[Stub]) {
    use melee_sim::game::code::{Val, stubs as ours};
    THEIR_LOGGED_STUBS.with(|s| *s.borrow_mut() = stubs.to_vec());
    for &(addr, ret) in stubs {
        ours::set(
            addr,
            match ret {
                StubRet::Nothing => Val::None,
                StubRet::Int(v) => Val::Int(v),
                StubRet::Float(v) => Val::Float(crate::arg(v)),
            },
        );
        ctx.register(addr, their_logged_stub);
    }
}

/// The stand-in the reference runs for every stubbed address: logs the call and returns what
/// [`THEIR_LOGGED_STUBS`] says.
fn their_logged_stub(ctx: &ssbm_rt::Ctx) {
    let addr = *ctx.native_stack().last().expect("a stub runs as a native");
    let r = &ctx.regs;
    let gpr: [u32; 8] = std::array::from_fn(|i| r.r(3 + i));
    let fpr: [f64; 4] = std::array::from_fn(|i| r.f(1 + i));
    let pointed = gpr.map(|a| {
        (0x8000_0000..0x817F_FFF4)
            .contains(&a)
            .then(|| [ctx.read_u32(a), ctx.read_u32(a + 4), ctx.read_u32(a + 8)])
    });
    THEIR_CALLS.with(|c| c.borrow_mut().push((addr, gpr, fpr, pointed)));
    let ret = THEIR_LOGGED_STUBS.with(|s| s.borrow().iter().find(|s| s.0 == addr).map(|s| s.1));
    match ret {
        Some(StubRet::Int(v)) => ctx.regs.set_r(3, v as u32),
        Some(StubRet::Float(v)) => ctx.regs.set_f(1, crate::arg(v)),
        _ => {}
    }
}

/// Our calls to stand-ins against the reference's: the same functions in the same order with
/// the same arguments.
fn stub_call_diffs(
    maps: &Maps,
    ours: &[(u32, Vec<melee_sim::game::code::Val>)],
    theirs: &[TheirCall],
) -> Vec<String> {
    use melee_sim::game::code::Val;
    let gobj_addr: std::collections::HashMap<_, u32> =
        maps.gobjs.iter().map(|(&a, &g)| (g, a)).collect();
    let fighter_addr: std::collections::HashMap<_, u32> =
        maps.fighters.iter().map(|(&a, &f)| (f, a)).collect();
    let jobj_addr: std::collections::HashMap<_, u32> =
        maps.jobjs.iter().map(|(&a, &j)| (j, a)).collect();
    let mut diffs = Vec::new();
    if ours.len() != theirs.len() {
        diffs.push(format!(
            "stubbed calls: ours {:x?}, theirs {:x?}",
            ours.iter().map(|c| c.0).collect::<Vec<_>>(),
            theirs.iter().map(|c| c.0).collect::<Vec<_>>()
        ));
        return diffs;
    }
    for (k, ((oa, args), (ta, gpr, fpr, pointed))) in ours.iter().zip(theirs).enumerate() {
        if oa != ta {
            diffs.push(format!("stubbed call {k}: ours {oa:#x}, theirs {ta:#x}"));
            continue;
        }
        let (mut gi, mut fi) = (0, 0);
        for a in args {
            let mut int = |what: String, ours: Option<u32>| {
                if ours.is_some_and(|o| o != gpr[gi]) {
                    diffs.push(format!(
                        "stubbed call {k} ({oa:#x}): r{} ours {what}, theirs {:#x}",
                        3 + gi,
                        gpr[gi]
                    ));
                }
            };
            match *a {
                Val::Int(v) => int(format!("{v:#x}"), Some(v as u32)),
                Val::GObj(g) => int(
                    format!("{g:?}"),
                    g.map_or(Some(0), |g| gobj_addr.get(&g).copied()),
                ),
                Val::Fighter(f) => int(format!("{f:?}"), fighter_addr.get(&f).copied()),
                Val::JObj(j) => int(
                    format!("{j:?}"),
                    j.map_or(Some(0), |j| jobj_addr.get(&j).copied()),
                ),
                Val::F32Ref(v) => {
                    if pointed[gi].map(|p| p[0]) != Some(v.to_bits()) {
                        diffs.push(format!(
                            "stubbed call {k} ({oa:#x}): r{} ours *{v:?}, theirs {:x?}",
                            3 + gi,
                            pointed[gi]
                        ));
                    }
                }
                Val::Vec(v) => {
                    let o = [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
                    if pointed[gi] != Some(o) {
                        diffs.push(format!(
                            "stubbed call {k} ({oa:#x}): r{} ours {v:?}, theirs {:x?}",
                            3 + gi,
                            pointed[gi]
                        ));
                    }
                }
                Val::Float(v) => {
                    if v.to_bits() != fpr[fi].to_bits() {
                        diffs.push(format!(
                            "stubbed call {k} ({oa:#x}): f{} ours {v:?}, theirs {:?}",
                            1 + fi,
                            fpr[fi]
                        ));
                    }
                    fi += 1;
                    continue;
                }
                // A spawn's result isn't an argument any stand-in takes.
                Val::Spawned(_) | Val::None => {}
            }
            gi += 1;
        }
    }
    diffs
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
            if !text.contains("isn't ported") && !text.starts_with(SKIP) {
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

/// The wind sources' pool (`lb_804D63A8`), given `n` sources by the call's index: boxes over
/// the stage and points near the fighters, fading, pulsing, some for ever; the rest free.
pub fn give_winds(call: &mut Call, i: usize, n: usize) {
    let base = peek_u32(call, crate::sym("lb_804D63A8"));
    let mut active = 0;
    let mut free = 0;
    for k in (0..8).rev() {
        let a = base + 0x38 * k as u32;
        if k < n {
            let point = (i + k) % 2 == 1;
            poke_u32(call, a, if point { 0x0264_0000 } else { 0x0100_0000 });
            let dir = if point {
                [(k as f32 - 3.0) * 7.0, 12.0 + k as f32, 0.0]
            } else {
                [0.6 - 0.3 * k as f32, -0.4, 0.2]
            };
            for (w, v) in dir.iter().enumerate() {
                poke_f32(call, a + 4 + 4 * w as u32, *v);
            }
            let bounds = [-200.0, 150.0 - 40.0 * (i % 3) as f32, 200.0, -50.0];
            for (w, v) in bounds.iter().enumerate() {
                poke_f32(call, a + 0x10 + 4 * w as u32, *v);
            }
            poke_f32(call, a + 0x20, 0.5 + 0.7 * k as f32);
            poke_f32(call, a + 0x24, [0.0, 0.02, 0.4][(i + k) % 3]);
            poke_u32(call, a + 0x28, [u32::MAX, 1, 30][(i + 2 * k) % 3]);
            poke_f32(call, a + 0x2C, std::f32::consts::FRAC_PI_3);
            poke_u32(call, a + 0x30, (i * 3 + k) as u32 % 50);
            poke_u32(call, a + 0x34, active);
            active = a;
        } else {
            poke_u32(call, a, 0);
            poke_u32(call, a + 0x34, free);
            free = a;
        }
    }
    poke_u32(call, crate::sym("lb_804D63B0"), active);
    poke_u32(call, crate::sym("lb_804D63AC"), free);
    // Whether a box's wind blew last frame (`lb_804D63B4`).
    poke_u32(call, crate::sym("lb_804D63B4"), [0, 1, 2, u32::MAX][i % 4]);
}
