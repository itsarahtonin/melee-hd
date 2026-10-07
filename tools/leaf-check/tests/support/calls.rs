//! Checks of game code from captured states (the machinery `vs.rs` grew, shared): each
//! capture is imported into melee-sim, a call runs on both sides from the same state in both
//! float modes, and the whole state (scheduler, stage, camera, the match, the HUD) and what
//! the call returned or wrote outside it are compared. Calls no capture saved run from other
//! functions' captured states, ssbm-rs's port called directly on its side.
//!
//! The captures are game memory, kept out of the repository; the directories in
//! [`calls_dirs`] each hold one directory per run, and the checks skip without them.

#![allow(dead_code)]

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex, OnceLock};

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{MODES, Rand, hsd_reference};
use melee_capture::compare::Comparer;
use melee_capture::import::{Importer, Maps, sym};
use melee_capture::{Capture, DataMap, DiscIndex, MEM1, disc_index, find_archives_in};
use melee_data::archive::Archives;
use melee_data::disc::Disc;
use melee_sim::game::code::Val;
use melee_sim::game::state::{Game, Running};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::Ctx;

/// Where the captures are: `MATCH_CALLS` (the VS match's, made as `vs.rs` says) and
/// `HUD_CALLS` (the HUD's, made the same way with the HUD's functions named; see `hud.rs`).
pub fn calls_dirs() -> Vec<PathBuf> {
    let var = |name: &str, default: &str| -> PathBuf {
        std::env::var(name)
            .unwrap_or_else(|_| default.into())
            .into()
    };
    vec![
        var(
            "MATCH_CALLS",
            "C:/Projects/ssbm-rs-publication/local/calls/match",
        ),
        var(
            "HUD_CALLS",
            "C:/Projects/ssbm-rs-publication/local/calls/hud",
        ),
    ]
}

/// The saved real calls of `func` in the captures' directories.
pub fn calls(func: &str) -> Vec<PathBuf> {
    let prefix = format!("{func}-call");
    let mut found: Vec<PathBuf> = calls_dirs()
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten())
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(".call")
        })
        .collect();
    found.sort();
    if found.is_empty() {
        eprintln!("no captures of {func} in {:?}", calls_dirs());
    }
    found
}

/// The disc's archives, read once for every test.
pub fn disc_archives() -> &'static DiscIndex {
    static ARCHIVES: OnceLock<DiscIndex> = OnceLock::new();
    ARCHIVES.get_or_init(|| disc_index(&Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap()))
}

/// The call's arguments, mapped to melee-sim's handles.
pub struct Args<'a> {
    pub cap: &'a Capture,
    pub maps: &'a Maps,
    pub data: &'a DataMap,
}

impl Args<'_> {
    pub fn r(&self, i: usize) -> u32 {
        self.cap.call.regs.gpr[i]
    }

    pub fn i(&self, i: usize) -> i32 {
        self.r(i) as i32
    }

    pub fn f(&self, i: usize) -> f64 {
        self.cap.call.regs.fpr[i].ps0
    }

    pub fn word(&self, a: u32) -> u32 {
        self.cap.u32(a)
    }

    pub fn u8(&self, a: u32) -> u8 {
        self.cap.call.mem1[(a - MEM1) as usize]
    }

    pub fn vec(&self, a: u32) -> Vec3 {
        let f = |o| f32::from_bits(self.word(a + o));
        Vec3::new(f(0), f(4), f(8))
    }

    pub fn gobj(&self, a: u32) -> GObjId {
        self.maps.gobjs[&a]
    }

    pub fn data_ref(&self, a: u32) -> melee_data::archive::DataRef {
        self.data.get(a).expect("a pointer into loaded data")
    }

    /// The archive loaded under a disc file name starting with `name`.
    pub fn archive(&self, name: &str) -> melee_data::archive::ArchiveId {
        self.data
            .loaded
            .iter()
            .find(|l| l.name.starts_with(name))
            .unwrap_or_else(|| panic!("{name} isn't loaded"))
            .id
    }

    pub fn mem(&self) -> melee_sim::game::mirror::Mem<'_> {
        melee_sim::game::mirror::Mem {
            bytes: &self.cap.call.mem1,
            base: MEM1,
        }
    }
}

/// What a call returned or wrote outside the state, compared with ssbm-rs's.
pub enum Out {
    R3(u32),
    /// r3 and r4: a 64-bit result.
    R3R4(u64),
    /// f1, as a register holds it.
    F1(f64),
    /// Words at an address: an out parameter.
    Words(u32, Vec<u32>),
    /// The calls ssbm-rs's side made to a logging native, against ours.
    Logged(Vec<Logged>),
    /// A GObj in r3.
    Gobj(Option<GObjId>),
}

/// A call to a function a check logs on ssbm-rs's side (a draw call, say): its address, r3 to
/// r8 and f1 to f4.
#[derive(Clone, Debug, PartialEq)]
pub struct Logged {
    pub addr: u32,
    pub gpr: Vec<u32>,
    pub fpr: Vec<u64>,
}

pub fn vec_words(v: Vec3) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// Runs the capture in the reference and returns memory after it, r3, r4 and f1.
fn theirs(
    ctx: &Ctx,
    call: &ssbm_rt::capture::Call,
    run: Option<&Theirs<'_>>,
) -> (Vec<u8>, u32, u32, f64) {
    call.load(ctx);
    match run {
        Some(run) => {
            let (r3, r4) = run(ctx, call);
            ctx.regs.set_r(3, r3);
            ctx.regs.set_r(4, r4);
        }
        None => ctx.invoke(call.addr),
    }
    let mut out = vec![0; call.mem1.len()];
    let _ = ctx.mem.read_bytes(MEM1, &mut out);
    (out, ctx.regs.r(3), ctx.regs.r(4), ctx.regs.f(1))
}

pub fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<String>()
        .cloned()
        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "a panic".into())
}

/// Whether our side stopped at code no port covers yet: a boundary, not a difference.
fn unported(text: &str) -> bool {
    text.contains("isn't ported") || text.starts_with("not implemented")
}

/// A turn among the checks running at once, at most [`Turn::MAX`].
struct Turn;

static TURNS: (Mutex<usize>, Condvar) = (Mutex::new(0), Condvar::new());

impl Turn {
    const MAX: usize = 3;

    fn take() -> Turn {
        let (count, freed) = &TURNS;
        let mut n = count.lock().unwrap_or_else(|e| e.into_inner());
        while *n >= Self::MAX {
            n = freed.wait(n).unwrap_or_else(|e| e.into_inner());
        }
        *n += 1;
        Turn
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        let (count, freed) = &TURNS;
        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
        freed.notify_one();
    }
}

fn copy_capture(c: &Capture) -> Capture {
    let k = &c.call;
    Capture {
        call: ssbm_rt::capture::Call {
            addr: k.addr,
            mutated: k.mutated,
            stub: k.stub,
            regs: k.regs.clone(),
            mem1: k.mem1.clone(),
            locked: k.locked.clone(),
        },
    }
}

/// A capture's memory and registers being perturbed.
pub struct Fuzz<'a> {
    pub mem: &'a mut Vec<u8>,
    pub regs: &'a mut ssbm_rt::RegsSnapshot,
    pub rand: &'a mut Rand,
}

impl Fuzz<'_> {
    pub fn u8(&self, a: u32) -> u8 {
        self.mem[(a - MEM1) as usize]
    }

    pub fn u16(&self, a: u32) -> u16 {
        u16::from(self.u8(a)) << 8 | u16::from(self.u8(a + 1))
    }

    pub fn u32(&self, a: u32) -> u32 {
        u32::from(self.u16(a)) << 16 | u32::from(self.u16(a + 2))
    }

    pub fn put(&mut self, a: u32, v: u32) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 4].copy_from_slice(&v.to_be_bytes());
    }

    pub fn put8(&mut self, a: u32, v: u8) {
        self.mem[(a - MEM1) as usize] = v;
    }

    pub fn put16(&mut self, a: u32, v: u16) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 2].copy_from_slice(&v.to_be_bytes());
    }

    pub fn below(&mut self, n: u64) -> u32 {
        self.rand.below(n) as u32
    }
}

/// How ssbm-rs's side runs a call the captures didn't save: its port called on the captured
/// state, returning r3 and r4.
pub type Theirs<'a> = dyn Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, u32) + 'a;

/// How many of a function's captures the fuzzed checks also perturb.
const FUZZED_CAPTURES: usize = 6;

/// A function of the original's no port covers, stood in for on both sides: its address and
/// what it returns.
pub type Stub = (u32, Ret);

/// What a stand-in returns.
#[derive(Clone, Copy, Debug)]
pub enum Ret {
    Nothing,
    Int(i32),
    Float(f32),
    /// A null GObj.
    Null,
}

/// A stand-in that does nothing: [`Ret::Nothing`].
pub const fn nothing(addr: u32) -> Stub {
    (addr, Ret::Nothing)
}

/// A call ssbm-rs's side made to a stand-in: its address, r3 to r8 and f1 to f3.
type TheirCall = (u32, [u32; 6], [f64; 3]);

thread_local! {
    static THEIR_STUBS: std::cell::RefCell<Vec<Stub>> = const { std::cell::RefCell::new(Vec::new()) };
    static THEIR_CALLS: std::cell::RefCell<Vec<TheirCall>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static LOGGED: std::cell::RefCell<Vec<Logged>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The stand-in ssbm-rs's side runs for every stubbed address: it logs the call and returns
/// what [`THEIR_STUBS`] says for the address it runs at.
fn their_stub(ctx: &Ctx) {
    let addr = *ctx.native_stack().last().expect("a stub runs as a native");
    let r = &ctx.regs;
    let gpr = [r.r(3), r.r(4), r.r(5), r.r(6), r.r(7), r.r(8)];
    let fpr = [r.f(1), r.f(2), r.f(3)];
    THEIR_CALLS.with(|c| c.borrow_mut().push((addr, gpr, fpr)));
    let ret = THEIR_STUBS.with(|s| s.borrow().iter().find(|s| s.0 == addr).map(|s| s.1));
    match ret {
        Some(Ret::Int(v)) => ctx.regs.set_r(3, v as u32),
        Some(Ret::Float(v)) => ctx.regs.set_f(1, leaf_check::arg(v)),
        Some(Ret::Null) => ctx.regs.set_r(3, 0),
        _ => {}
    }
}

/// Logs a call for [`Out::Logged`] (from a check's own native).
pub fn log(l: Logged) {
    LOGGED.with(|v| v.borrow_mut().push(l));
}

/// A native that logs its call (r3 to r8, f1 to f4) and does nothing else.
pub fn log_native(ctx: &Ctx) {
    let addr = *ctx
        .native_stack()
        .last()
        .expect("a logger runs as a native");
    let r = &ctx.regs;
    LOGGED.with(|l| {
        l.borrow_mut().push(Logged {
            addr,
            gpr: (3..9).map(|i| r.r(i)).collect(),
            fpr: (1..5).map(|i| r.f(i).to_bits()).collect(),
        })
    });
}

/// Our stubbed calls against ssbm-rs's: the same functions in the same order with the same
/// integer, GObj and float arguments (vectors passed by pointer aside).
fn stub_call_diffs(maps: &Maps) -> Vec<String> {
    let ours = melee_sim::game::code::stubs::take();
    let theirs = THEIR_CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()));
    let addr_of: std::collections::HashMap<GObjId, u32> =
        maps.gobjs.iter().map(|(&a, &g)| (g, a)).collect();
    let mut diffs = Vec::new();
    if ours.len() != theirs.len() {
        diffs.push(format!(
            "stubbed calls: ours {:x?}, theirs {:x?}",
            ours.iter().map(|c| c.0).collect::<Vec<_>>(),
            theirs.iter().map(|c| c.0).collect::<Vec<_>>()
        ));
        return diffs;
    }
    for (k, ((oa, args), (ta, gpr, fpr))) in ours.iter().zip(&theirs).enumerate() {
        if oa != ta {
            diffs.push(format!("stubbed call {k}: ours {oa:#x}, theirs {ta:#x}"));
            continue;
        }
        let (mut gi, mut fi) = (0, 0);
        for a in args {
            match *a {
                Val::Int(v) => {
                    if gpr[gi] != v as u32 {
                        diffs.push(format!(
                            "stubbed call {k} ({oa:#x}): r{} ours {v:#x}, theirs {:#x}",
                            3 + gi,
                            gpr[gi]
                        ));
                    }
                    gi += 1;
                }
                Val::GObj(g) => {
                    let o = g.map_or(Some(0), |g| addr_of.get(&g).copied());
                    if o.is_some_and(|o| o != gpr[gi]) {
                        diffs.push(format!(
                            "stubbed call {k} ({oa:#x}): r{} ours {g:?}, theirs {:#x}",
                            3 + gi,
                            gpr[gi]
                        ));
                    }
                    gi += 1;
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
                }
                Val::Vec(_) | Val::None | Val::JObj(_) | Val::Spawned(_) => gi += 1,
            }
        }
    }
    diffs
}

/// Imports a capture's whole state.
pub fn import(cap: &Capture, data: &DataMap) -> (Game, Maps, Vec<String>) {
    let mut imp = Importer::new(cap, data);
    imp.world();
    imp.stage();
    imp.ids();
    imp.match_state();
    let Importer {
        game,
        maps,
        unhandled,
        ..
    } = imp;
    (game, maps, unhandled)
}

/// Compares a game with memory: everything [`import`] imports.
pub fn compare(mem: &[u8], data: &DataMap, game: &Game, maps: Maps) -> Vec<String> {
    let mut cmp = Comparer::new(mem, data, game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.match_state();
    cmp.diffs
}

/// A check: which captures, what to change in them, how each side runs, and what stands in
/// on both sides.
pub struct Check<'a> {
    /// The function whose captures the check runs from.
    pub base: &'a str,
    /// Perturbed copies of each of the first captures, besides the captures themselves.
    pub variants: usize,
    /// Perturb the captures themselves too (a call no capture saved).
    pub perturb_all: bool,
    pub perturb: &'a dyn Fn(&mut Fuzz),
    pub ours: &'a dyn Fn(&mut Running, &Args) -> Vec<Out>,
    /// ssbm-rs's side, if not the captured call itself.
    pub theirs: Option<&'a Theirs<'a>>,
    pub stubs: &'a [Stub],
    /// Natives on ssbm-rs's side only (stand-ins for file loads whose files are loaded
    /// already, and loggers for [`Out::Logged`]).
    pub natives: &'a [(u32, fn(&Ctx))],
    /// Diffs whose path starts with one of these must round-trip on import.
    pub round_trip: &'a [&'a str],
    /// At most this many captures.
    pub max_captures: usize,
}

impl<'a> Check<'a> {
    pub fn new(base: &'a str, ours: &'a dyn Fn(&mut Running, &Args) -> Vec<Out>) -> Self {
        Check {
            base,
            variants: 0,
            perturb_all: false,
            perturb: &|_| {},
            ours,
            theirs: None,
            stubs: &[],
            natives: &[],
            round_trip: &["hud"],
            max_captures: usize::MAX,
        }
    }

    /// ssbm-rs's side runs `theirs` on the captured state instead of the captured call.
    pub fn with_theirs(mut self, theirs: &'a Theirs<'a>) -> Self {
        self.theirs = Some(theirs);
        self
    }

    /// Runs on at most `n` captures.
    pub fn run_n(mut self, n: usize) {
        self.max_captures = n;
        self.run();
    }

    pub fn run(&self) {
        let func = self.base;
        let paths = calls(func);
        if paths.is_empty() {
            return;
        }
        let _turn = Turn::take();
        let Some(ctx) = hsd_reference() else { return };
        melee_sim::game::code::stubs::clear();
        THEIR_STUBS.with(|s| *s.borrow_mut() = self.stubs.to_vec());
        for &(addr, ret) in self.stubs {
            let ours = match ret {
                Ret::Nothing => Val::None,
                Ret::Int(v) => Val::Int(v),
                Ret::Float(v) => Val::Float(leaf_check::arg(v)),
                Ret::Null => Val::GObj(None),
            };
            melee_sim::game::code::stubs::set(addr, ours);
            ctx.register(addr, their_stub);
        }
        for &(addr, f) in self.natives {
            ctx.register(addr, f);
        }
        let disc = disc_archives();
        let mut rand = Rand(0x5EED_CA3E_u64 ^ func.len() as u64);
        let (mut agreed, mut blocked, mut skipped) = (0, Vec::new(), Vec::new());
        for (n, path) in paths.iter().enumerate().take(self.max_captures) {
            let original = Capture::read(path);
            let mut archives = Archives::default();
            let data = DataMap {
                loaded: find_archives_in(disc, &original.call.mem1, &mut archives),
            };
            let copies = if n < FUZZED_CAPTURES {
                self.variants
            } else {
                0
            };
            for variant in 0..=copies {
                let perturbed;
                let cap = if variant == 0 && !self.perturb_all {
                    &original
                } else {
                    let mut c = copy_capture(&original);
                    (self.perturb)(&mut Fuzz {
                        mem: &mut c.call.mem1,
                        regs: &mut c.call.regs,
                        rand: &mut rand,
                    });
                    perturbed = c;
                    &perturbed
                };
                for mode in MODES {
                    set_fp_mode(mode);
                    let imported = catch_unwind(AssertUnwindSafe(|| import(cap, &data)));
                    let Ok((mut game, maps, _)) = imported else {
                        if variant == 0 && mode == FpMode::Hardware {
                            skipped.push(path.file_name().unwrap().to_string_lossy().into_owned());
                        }
                        break;
                    };
                    let before: HashSet<String> =
                        compare(&cap.call.mem1, &data, &game, maps.clone())
                            .into_iter()
                            .collect();
                    let bad: Vec<&String> = before
                        .iter()
                        .filter(|d| self.round_trip.iter().any(|k| d.starts_with(k)))
                        .collect();
                    assert!(
                        bad.is_empty(),
                        "{}: the state doesn't round-trip: {bad:?}",
                        path.display()
                    );
                    let _ = melee_sim::game::code::stubs::take();
                    LOGGED.with(|l| l.borrow_mut().clear());
                    let ours = catch_unwind(AssertUnwindSafe(|| {
                        (self.ours)(
                            &mut Running {
                                game: &mut game,
                                data: &archives,
                            },
                            &Args {
                                cap,
                                maps: &maps,
                                data: &data,
                            },
                        )
                    }));
                    let outs = match ours {
                        Ok(o) => o,
                        Err(p) if unported(&panic_text(&p)) => {
                            if variant == 0 && mode == FpMode::Hardware {
                                blocked.push(format!(
                                    "{}: {}",
                                    path.file_name().unwrap().to_string_lossy(),
                                    panic_text(&p)
                                ));
                            }
                            break;
                        }
                        Err(p) => {
                            let t = catch_unwind(AssertUnwindSafe(|| {
                                theirs(&ctx, &cap.call, self.theirs)
                            }));
                            match t {
                                Err(t) => {
                                    eprintln!(
                                        "{} (variant {variant}): both stop: ours {}, theirs {}",
                                        path.display(),
                                        panic_text(&p),
                                        panic_text(&t)
                                    );
                                    continue;
                                }
                                Ok(_) => panic!(
                                    "{} (variant {variant}): ours panicked: {}",
                                    path.display(),
                                    panic_text(&p)
                                ),
                            }
                        }
                    };
                    THEIR_CALLS.with(|c| c.borrow_mut().clear());
                    LOGGED.with(|l| l.borrow_mut().clear());
                    let (after, r3, r4, f1) = theirs(&ctx, &cap.call, self.theirs);
                    let logged = LOGGED.with(|l| std::mem::take(&mut *l.borrow_mut()));
                    let word = |a: u32| {
                        let i = (a - MEM1) as usize;
                        u32::from_be_bytes(after[i..i + 4].try_into().unwrap())
                    };
                    let mut diffs = stub_call_diffs(&maps);
                    let mut cmp = Comparer::new(&after, &data, &game, maps);
                    cmp.world();
                    cmp.stage();
                    cmp.ids();
                    cmp.match_state();
                    let state_diffs: Vec<String> = std::mem::take(&mut cmp.diffs)
                        .into_iter()
                        .filter(|d| !before.contains(d))
                        .collect();
                    let maps = cmp.maps;
                    for o in &outs {
                        match o {
                            Out::R3(v) if *v != r3 => {
                                diffs.push(format!("r3: ours {v:#x}, theirs {r3:#x}"))
                            }
                            Out::R3R4(v) => {
                                let t = u64::from(r3) << 32 | u64::from(r4);
                                if *v != t {
                                    diffs.push(format!("r3:r4: ours {v:#x}, theirs {t:#x}"));
                                }
                            }
                            Out::Gobj(g) => {
                                let ours = match g {
                                    None => Some(0),
                                    Some(g) => {
                                        maps.gobjs.iter().find(|(_, v)| *v == g).map(|(a, _)| *a)
                                    }
                                };
                                if ours != Some(r3) {
                                    diffs.push(format!("r3: ours {g:?}, theirs {r3:#x}"));
                                }
                            }
                            Out::F1(v) if v.to_bits() != f1.to_bits() => {
                                diffs.push(format!("f1: ours {v:?}, theirs {f1:?}"))
                            }
                            Out::Words(a, ws) => {
                                let t: Vec<u32> =
                                    (0..ws.len() as u32).map(|i| word(a + 4 * i)).collect();
                                if &t != ws {
                                    diffs.push(format!("{a:#x}: ours {ws:x?}, theirs {t:x?}"));
                                }
                            }
                            Out::Logged(ours) => {
                                if ours != &logged {
                                    let k = ours
                                        .iter()
                                        .zip(&logged)
                                        .position(|(a, b)| a != b)
                                        .unwrap_or(ours.len().min(logged.len()));
                                    diffs.push(format!(
                                        "logged calls ({} ours, {} theirs) differ at {k}: ours {:x?}, theirs {:x?}",
                                        ours.len(),
                                        logged.len(),
                                        ours.get(k),
                                        logged.get(k)
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                    diffs.extend(state_diffs);
                    for d in diffs.iter().take(20) {
                        eprintln!("  {d}");
                    }
                    assert!(
                        diffs.is_empty(),
                        "{} (variant {variant}) in {mode:?}: {} differences",
                        path.display(),
                        diffs.len()
                    );
                    if variant == 0 && mode == FpMode::Hardware {
                        agreed += 1;
                    }
                }
            }
        }
        set_fp_mode(FpMode::Hardware);
        eprintln!(
            "{func}: {agreed} captures agree, {} reach unported code, {} don't import",
            blocked.len(),
            skipped.len()
        );
        for s in skipped.iter().take(5) {
            eprintln!("  skipped (the importer can't take it): {s}");
        }
        for b in blocked.iter().take(5) {
            eprintln!("  {b}");
        }
        assert!(agreed > 0, "{func}: every capture reaches unported code");
    }
}

/// A symbol's address.
pub fn at(name: &str) -> u32 {
    sym(name)
}
