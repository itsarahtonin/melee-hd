//! The VS match (`melee/gm/gmvs.c` and the parts of `gm_1601.c`, `gm_16A2.c`, `gm_16F1.c`,
//! `gm_1A36.c`, `gmscene.c` and `gmpause.c` it runs) and the player blocks (`melee/pl`)
//! against ssbm-rs's ports, run from real game states: each capture of a function is imported
//! into melee-sim, run on both sides from the same state in both float modes, and the whole
//! state (scheduler, stage, camera, player blocks, pads and button maps, the scene loop, the
//! match's controller and the player library's counters) and what the call returned or wrote
//! outside it are compared. Calls no capture saves (those that load files or reach the audio
//! driver, which the runner doesn't save) run from other functions' captured states.
//!
//! The captures are game memory, kept out of the repository in `MATCH_CALLS` (default below),
//! one directory per run; the tests skip without them. ssbm-rs's runner (branch
//! `feat/state-trace`, `cargo build --release -p ssbm-run`) makes them in the debug VS mode's
//! random matches, whose code Slippi's playback codes don't patch:
//!
//! ```text
//! MATCH_STAGES=31,32 MATCH_FIGHTERS=2,19,20,9 MATCH_TIME=30 MATCH_PLAYERS=2 \
//!   CAPTURE=$MATCH_CALLS/dvs1 CAPTURE_FROM=0 CAPTURE_NESTED=1 CAPTURE_FUNCS=<names> \
//!   LOCKSTEP_DONE=<every function outside melee/gm and melee/pl> \
//!   ssbm-run "$SSBM_DISC" --mode e --matches 1 --monkey 1 --fields 6000 --port all --lockstep
//! ```
//!
//! and again with `MATCH_TEAMS=1 MATCH_PLAYERS=4` into `dvs-teams`, with the names this file's
//! tests use.

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
use melee_sim::game::gm::{input, pause};
use melee_sim::game::mirror::Mirror;
use melee_sim::game::state::Running;
use melee_sim::game::structs::{PlayerInitData, StartMeleeData};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

const DEFAULT_CALLS: &str = "C:/Projects/ssbm-rs-publication/local/calls/match";

fn calls_dir() -> PathBuf {
    std::env::var("MATCH_CALLS")
        .unwrap_or_else(|_| DEFAULT_CALLS.into())
        .into()
}

/// The saved real calls of `func` in the captures' directories.
fn calls(func: &str) -> Vec<PathBuf> {
    let dir = calls_dir();
    let prefix = format!("{func}-call");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
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
        eprintln!(
            "no captures of {func} in {}: make them with ssbm-rs's runner (see this file's docs)",
            dir.display()
        );
    }
    found
}

/// The disc's archives, read once for every test.
fn disc_archives() -> &'static DiscIndex {
    static ARCHIVES: OnceLock<DiscIndex> = OnceLock::new();
    ARCHIVES.get_or_init(|| disc_index(&Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap()))
}

/// The call's arguments, mapped to melee-sim's handles.
struct Args<'a> {
    cap: &'a Capture,
    maps: &'a Maps,
    data: &'a DataMap,
}

impl Args<'_> {
    fn r(&self, i: usize) -> u32 {
        self.cap.call.regs.gpr[i]
    }

    fn i(&self, i: usize) -> i32 {
        self.r(i) as i32
    }

    fn word(&self, a: u32) -> u32 {
        self.cap.u32(a)
    }

    fn vec(&self, a: u32) -> Vec3 {
        let f = |o| f32::from_bits(self.word(a + o));
        Vec3::new(f(0), f(4), f(8))
    }

    fn gobj(&self, a: u32) -> GObjId {
        self.maps.gobjs[&a]
    }

    fn data_ref(&self, a: u32) -> melee_data::archive::DataRef {
        self.data.get(a).expect("a pointer into loaded data")
    }

    fn mem(&self) -> melee_sim::game::mirror::Mem<'_> {
        melee_sim::game::mirror::Mem {
            bytes: &self.cap.call.mem1,
            base: MEM1,
        }
    }
}

/// What a call returned or wrote outside the state, compared with ssbm-rs's.
enum Out {
    R3(u32),
    /// r3 and r4: a 64-bit result.
    R3R4(u64),
    /// Words at an address: an out parameter.
    Words(u32, Vec<u32>),
}

fn vec_words(v: Vec3) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// Runs the capture in the reference and returns memory after it, r3 and r4.
fn theirs(
    ctx: &Ctx,
    call: &ssbm_rt::capture::Call,
    run: Option<&Theirs<'_>>,
) -> (Vec<u8>, u32, u32) {
    call.load(ctx);
    // A replay run's captures hold Slippi's playback codes: the functions they patch run as
    // patched.
    leaf_check::register_patched(ctx);
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
    (out, ctx.regs.r(3), ctx.regs.r(4))
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
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
struct Fuzz<'a> {
    mem: &'a mut Vec<u8>,
    regs: &'a mut ssbm_rt::RegsSnapshot,
    rand: &'a mut Rand,
}

impl Fuzz<'_> {
    fn u8(&self, a: u32) -> u8 {
        self.mem[(a - MEM1) as usize]
    }

    fn u32(&self, a: u32) -> u32 {
        let i = (a - MEM1) as usize;
        u32::from_be_bytes(self.mem[i..i + 4].try_into().unwrap())
    }

    fn put(&mut self, a: u32, v: u32) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn put8(&mut self, a: u32, v: u8) {
        self.mem[(a - MEM1) as usize] = v;
    }

    fn put16(&mut self, a: u32, v: u16) {
        let i = (a - MEM1) as usize;
        self.mem[i..i + 2].copy_from_slice(&v.to_be_bytes());
    }

    fn below(&mut self, n: u64) -> u32 {
        self.rand.below(n) as u32
    }

    /// Start, Z, L, R, A pressed or held on random ports, as the copy statuses and button
    /// maps show them.
    fn buttons(&mut self) {
        let pads = sym("HSD_PadCopyStatus");
        let map = sym("controller_map");
        let pick = [
            0x1000,
            0x10,
            0x40,
            0x20,
            0x100,
            0x1000 | 0x40 | 0x20 | 0x100,
        ];
        for i in 0..4 {
            let held = if self.below(3) == 0 {
                pick[self.below(6) as usize]
            } else {
                0
            };
            let pressed = if self.below(3) == 0 {
                pick[self.below(6) as usize] & (held | 0x1000 | 0x10)
            } else {
                0
            };
            self.put(pads + 0x44 * i, held | pressed);
            self.put(pads + 0x44 * i + 8, pressed);
            let err = if self.below(10) == 0 { 0xFF } else { 0 };
            self.put8(pads + 0x44 * i + 0x41, err);
            self.put(map + 0x30 * i + 4, held | pressed);
            self.put(map + 0x30 * i + 0xC, pressed);
        }
    }

    /// The match's controller near a decision: timer, frame counts, pause timers, pauser,
    /// stocks, respawn timers and debug pause flags changed now and then.
    fn match_state(&mut self) {
        let c = sym("controller");
        if self.below(3) == 0 {
            let secs = [0, 1, 5, 6, 59, u32::MAX][self.below(6) as usize];
            self.put(c + 0x28, secs);
            let frames = [0, 58, 59, 60][self.below(4) as usize];
            self.put16(c + 0x2C, frames);
        }
        if self.below(4) == 0 {
            let v = self.below(3) as u8;
            self.put8(c + 2, v);
            let v = self.below(3) as u8;
            self.put8(c + 4, v);
            let v = self.below(5) as u8;
            self.put8(c + 1, v);
        }
        if self.below(4) == 0 {
            let v = self.below(2) as u8;
            self.put8(c + 5, v);
        }
        let players = sym("player_slots");
        for s in 0..6 {
            if self.below(5) == 0 {
                let stocks = [0, 1, 2, 4][self.below(4) as usize];
                self.put8(players + 0xE90 * s + 0x8E, stocks);
            }
            if self.below(8) == 0 {
                let v = [0, 1, 0x51, 0x50][self.below(4) as usize];
                self.put8(c + 0x38 + 0xE * s + 1, v);
                let v = self.below(3) as u8;
                self.put8(c + 0x38 + 0xE * s + 8, v);
            }
        }
        if self.below(6) == 0 {
            let g = sym("gm_80479D58");
            let flags = [0, 1, 2, 3, 0x10][self.below(5) as usize];
            self.put8(g + 0x10, flags);
        }
        if self.below(6) == 0 {
            let start = c + 0x24C8;
            // disable_pausing and the pause-menu quit buttons.
            let b = self.u8(start + 2) ^ (1 << (7 - 4));
            self.put8(start + 2, b);
            let b3 = self.below(256) as u8;
            self.put8(start + 3, b3);
        }
    }
}

/// How ssbm-rs's side runs a call the captures didn't save: its port called on the captured
/// state, returning r3 and r4.
type Theirs<'a> = dyn Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, u32) + 'a;

/// How many of a function's captures the fuzzed checks also perturb.
const FUZZED_CAPTURES: usize = 6;

fn check(func: &str, ours: impl Fn(&mut Running, &Args) -> Vec<Out>) {
    check_runs(func, 0, false, &|_| {}, &ours, None, &[]);
}

/// A function of the original's no port covers, stood in for on both sides: its address and
/// what it returns.
type Stub = (u32, Ret);

/// What a stand-in returns.
#[derive(Clone, Copy, Debug)]
enum Ret {
    Nothing,
    /// A null GObj.
    Null,
}

/// `Fighter_Create`, which takes its arguments in a struct.
const FIGHTER_CREATE: u32 = 0x8006_8E98;

/// A stand-in that does nothing: [`Ret::Nothing`].
const fn nothing(addr: u32) -> Stub {
    (addr, Ret::Nothing)
}

/// A call ssbm-rs's side made to a stand-in: its address, r3 to r8 and f1 to f3.
type TheirCall = (u32, [u32; 6], [f64; 3]);

thread_local! {
    /// ssbm-rs's side's stand-ins and the calls they took: address, r3 to r8 and f1 to f3.
    static THEIR_STUBS: std::cell::RefCell<Vec<Stub>> = const { std::cell::RefCell::new(Vec::new()) };
    static THEIR_CALLS: std::cell::RefCell<Vec<TheirCall>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The stand-in ssbm-rs's side runs for every stubbed address: it logs the call and returns
/// what [`THEIR_STUBS`] says for the address it runs at.
fn their_stub(ctx: &Ctx) {
    let addr = *ctx.native_stack().last().expect("a stub runs as a native");
    let r = &ctx.regs;
    let mut gpr = [r.r(3), r.r(4), r.r(5), r.r(6), r.r(7), r.r(8)];
    if addr == FIGHTER_CREATE {
        // Its `plAllocInfo`, field by field, as our side passes it: fighter kind, slot, `x5`,
        // and the second-fighter and transformation bits.
        let a = gpr[0];
        let bits = ctx.read_u8(a + 6);
        gpr = [
            ctx.read_u32(a),
            u32::from(ctx.read_u8(a + 4)),
            ctx.read_u8(a + 5) as i8 as i32 as u32,
            u32::from(bits >> 7),
            u32::from(bits >> 6 & 1),
            0,
        ];
    }
    let fpr = [r.f(1), r.f(2), r.f(3)];
    THEIR_CALLS.with(|c| c.borrow_mut().push((addr, gpr, fpr)));
    let ret = THEIR_STUBS.with(|s| s.borrow().iter().find(|s| s.0 == addr).map(|s| s.1));
    match ret {
        Some(Ret::Null) => ctx.regs.set_r(3, 0),
        _ => {}
    }
}

/// Our stubbed calls against ssbm-rs's: the same functions in the same order with the same
/// integer, GObj and float arguments (vectors passed by pointer aside).
fn stub_call_diffs(maps: &Maps) -> Vec<String> {
    use melee_sim::game::code::Val;
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

/// [`check`] with `stubs` standing in on both sides for the unported code the call reaches.
fn check_stubbed(func: &str, stubs: &[Stub], ours: impl Fn(&mut Running, &Args) -> Vec<Out>) {
    check_runs(func, 0, false, &|_| {}, &ours, None, stubs);
}

fn check_fuzzed(
    func: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
) {
    check_runs(func, variants, false, &perturb, &ours, None, &[]);
}

/// Calls a function the captures didn't save from the states of another's captures (`base`).
fn check_calls(
    base: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
    theirs: impl Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, u32),
) {
    check_runs(base, variants, true, &perturb, &ours, Some(&theirs), &[]);
}

/// [`check_calls`] with stand-ins for unported code on both sides.
fn check_calls_stubbed(
    base: &str,
    variants: usize,
    stubs: &[Stub],
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
    theirs: impl Fn(&Ctx, &ssbm_rt::capture::Call) -> (u32, u32),
) {
    check_runs(base, variants, true, &perturb, &ours, Some(&theirs), stubs);
}

/// Imports a capture's whole state.
fn import(
    cap: &Capture,
    data: &DataMap,
    bank: Option<melee_data::archive::ArchiveId>,
) -> (melee_sim::game::state::Game, Maps, Vec<String>) {
    let mut imp = Importer::new(cap, data);
    imp.world();
    imp.stage();
    imp.ids();
    imp.match_state();
    imp.audio(bank);
    let Importer {
        game,
        maps,
        unhandled,
        ..
    } = imp;
    (game, maps, unhandled)
}

/// Compares a game with memory: everything [`import`] imports.
fn compare(
    mem: &[u8],
    data: &DataMap,
    game: &melee_sim::game::state::Game,
    maps: Maps,
) -> Vec<String> {
    let mut cmp = Comparer::new(mem, data, game, maps);
    cmp.world();
    cmp.stage();
    cmp.ids();
    cmp.match_state();
    cmp.audio();
    cmp.diffs
}

fn check_runs(
    func: &str,
    variants: usize,
    perturb_all: bool,
    perturb: &dyn Fn(&mut Fuzz),
    ours: &dyn Fn(&mut Running, &Args) -> Vec<Out>,
    run: Option<&Theirs<'_>>,
    stubs: &[Stub],
) {
    let paths = calls(func);
    if paths.is_empty() {
        return;
    }
    let _turn = Turn::take();
    let Some(ctx) = hsd_reference() else { return };
    melee_sim::game::code::stubs::clear();
    THEIR_STUBS.with(|s| *s.borrow_mut() = stubs.to_vec());
    for &(addr, ret) in stubs {
        let ours = match ret {
            Ret::Nothing => melee_sim::game::code::Val::None,
            Ret::Null => melee_sim::game::code::Val::GObj(None),
        };
        melee_sim::game::code::stubs::set(addr, ours);
        ctx.register(addr, their_stub);
    }
    let disc = disc_archives();
    let mut rand = Rand(0x5EED_CA3E_u64 ^ func.len() as u64);
    let (mut agreed, mut blocked, mut skipped) = (0, Vec::new(), Vec::new());
    for (n, path) in paths.iter().enumerate() {
        let original = Capture::read(path);
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(disc, &original.call.mem1, &mut archives),
        };
        let bank = melee_capture::audio::find_bank(&original.call.mem1, &mut archives);
        let copies = if n < FUZZED_CAPTURES { variants } else { 0 };
        for variant in 0..=copies {
            let perturbed;
            let cap = if variant == 0 && !perturb_all {
                &original
            } else {
                let mut c = copy_capture(&original);
                perturb(&mut Fuzz {
                    mem: &mut c.call.mem1,
                    regs: &mut c.call.regs,
                    rand: &mut rand,
                });
                perturbed = c;
                &perturbed
            };
            for mode in MODES {
                set_fp_mode(mode);
                // A state the importer can't take (a menu's animations copied into the heap,
                // a boot state's stale pointers) is skipped and counted.
                let imported = catch_unwind(AssertUnwindSafe(|| import(cap, &data, bank)));
                let Ok((mut game, maps, _)) = imported else {
                    if variant == 0 && mode == FpMode::Hardware {
                        skipped.push(path.file_name().unwrap().to_string_lossy().into_owned());
                    }
                    break;
                };
                let before: HashSet<String> = compare(&cap.call.mem1, &data, &game, maps.clone())
                    .into_iter()
                    .collect();
                let match_diffs: Vec<&String> = before
                    .iter()
                    .filter(|d| {
                        [
                            "player ",
                            "controller",
                            "gm_",
                            "HSD_Pad",
                            "rumble",
                            "lb_",
                            "pl ",
                            "pause",
                        ]
                        .iter()
                        .any(|k| d.starts_with(k))
                    })
                    .collect();
                assert!(
                    match_diffs.is_empty(),
                    "{}: the match's state doesn't round-trip: {match_diffs:?}",
                    path.display()
                );
                let _ = melee_sim::game::code::stubs::take();
                let ours = catch_unwind(AssertUnwindSafe(|| {
                    ours(
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
                        let t = catch_unwind(AssertUnwindSafe(|| theirs(&ctx, &cap.call, run)));
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
                let (after, r3, r4) = theirs(&ctx, &cap.call, run);
                let word = |a: u32| {
                    let i = (a - MEM1) as usize;
                    u32::from_be_bytes(after[i..i + 4].try_into().unwrap())
                };
                let mut diffs = stub_call_diffs(&maps);
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
                        Out::Words(a, ws) => {
                            let t: Vec<u32> =
                                (0..ws.len() as u32).map(|i| word(a + 4 * i)).collect();
                            if &t != ws {
                                diffs.push(format!("{a:#x}: ours {ws:x?}, theirs {t:x?}"));
                            }
                        }
                        _ => {}
                    }
                }
                // An audio call melee-sim hands the frontend unported (`Event::Audio`: the
                // music's pauses and stops, the crowd) changes the original's audio state as
                // an unported callee would; the audio state is compared where none ran.
                let unported_audio = game
                    .events
                    .iter()
                    .any(|e| matches!(e, melee_sim::game::state::Event::Audio { .. }));
                diffs.extend(
                    compare(&after, &data, &game, maps)
                        .into_iter()
                        .filter(|d| !before.contains(d))
                        .filter(|d| !(unported_audio && d.starts_with("audio: "))),
                );
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

/// The controller's address, which the match passes its functions.
fn controller() -> u32 {
    sym("controller")
}

/// The unported code the match's frame reaches, stood in for on both sides: the fighter's
/// rebirth (`ftCo_800D4FF4`).
const FRAME_STUBS: [Stub; 1] = [nothing(0x800D_4FF4)];

/// [`check_fuzzed`] with [`FRAME_STUBS`].
fn check_frame(
    func: &str,
    variants: usize,
    perturb: impl Fn(&mut Fuzz),
    ours: impl Fn(&mut Running, &Args) -> Vec<Out>,
) {
    check_runs(func, variants, false, &perturb, &ours, None, &FRAME_STUBS);
}

// The scene loop and the button maps.

#[test]
fn evaluate_controller_inputs() {
    check_fuzzed(
        "gm_EvaluateAllControllerInputs",
        6,
        |f| f.buttons(),
        |r, _| {
            r.game.evaluate_controller_inputs();
            vec![]
        },
    );
}

#[test]
fn map_any() {
    check("gm_801A3714", |r, a| {
        let from = u64::from(a.r(5)) << 32 | u64::from(a.r(6));
        let to = u64::from(a.r(7)) << 32 | u64::from(a.r(8));
        input::map_any(&mut r.game.controller_map, a.r(3) as usize, from, to);
        vec![]
    });
}

#[test]
fn map_all() {
    check("gm_801A3820", |r, a| {
        let from = u64::from(a.r(5)) << 32 | u64::from(a.r(6));
        let to = u64::from(a.r(7)) << 32 | u64::from(a.r(8));
        input::map_all(&mut r.game.controller_map, a.r(3) as usize, from, to);
        vec![]
    });
}

#[test]
fn key_repeat() {
    check_fuzzed(
        "fn_801A396C",
        6,
        |f| {
            let map = sym("controller_map");
            // Hold times about the map's own thresholds (`xF8`, `xFC`).
            let half =
                |f: &Fuzz, o: u32| u32::from(f.u8(map + o)) << 8 | u32::from(f.u8(map + o + 1));
            let (mid, fast) = (half(f, 0xF8), half(f, 0xFC));
            for i in 0..4 {
                let t = [0, 0, 1, f.below(30)][f.below(4) as usize];
                f.put(map + 0x30 * i + 0x28, t);
                let h = [
                    0,
                    19,
                    39,
                    40,
                    99,
                    100,
                    mid.wrapping_sub(1),
                    mid,
                    fast.wrapping_sub(1),
                ][f.below(9) as usize];
                f.put(map + 0x30 * i + 0x2C, h);
            }
        },
        |r, a| {
            input::repeat(&mut r.game.controller_map, a.r(3) as usize);
            vec![]
        },
    );
}

#[test]
fn reset_controller_map() {
    check("gm_801A3E88", |r, _| {
        r.game.reset_controller_map();
        vec![]
    });
}

#[test]
fn clear_db_pause() {
    check("gm_801677C0", |r, a| {
        assert_eq!(a.r(3), sym("gm_80479D58") + 0x10);
        r.game.clear_db_pause();
        vec![]
    });
}

#[test]
fn pause_links() {
    check("gm_801A48A4", |_, a| {
        vec![Out::R3R4(melee_sim::game::state::Game::pause_links(
            a.r(3) as u8
        ))]
    });
}

// The match's frame.

#[test]
fn vs_frame() {
    check_frame(
        "fn_8016CFE0",
        12,
        |f| {
            f.buttons();
            f.match_state();
        },
        |r, _| {
            r.vs_frame();
            vec![]
        },
    );
}

#[test]
fn vs_tick_clock() {
    check_frame(
        "fn_8016CD98",
        12,
        |f| f.match_state(),
        |r, a| {
            assert_eq!(a.r(3), controller());
            r.vs_tick_clock();
            vec![]
        },
    );
}

#[test]
fn vs_after_end() {
    check("fn_8016D634", |r, _| {
        r.vs_after_end();
        vec![]
    });
}

#[test]
fn vs_on_frame() {
    // No capture saves the frame function, which the VS scene's results reach through the
    // audio driver; it runs from the frame's own captured states.
    check_calls_stubbed(
        "fn_8016CFE0",
        6,
        &FRAME_STUBS,
        |f| {
            f.buttons();
            f.match_state();
            let c = sym("controller");
            let s = [0, 0, 0, 3][f.below(4) as usize];
            f.put8(c, s);
        },
        |r, _| {
            r.vs_on_frame();
            vec![]
        },
        |ctx, _| {
            fns::gm_Scene_Vs_OnFrame(ctx);
            (0, 0)
        },
    );
}

#[test]
fn match_outcome() {
    check_fuzzed(
        "gm_GetMatchOutcome",
        12,
        |f| f.match_state(),
        |r, _| vec![Out::R3(u32::from(r.game.match_outcome()))],
    );
}

#[test]
fn ffa_outcome() {
    check_fuzzed(
        "gm_GetFFAOutcome",
        12,
        |f| f.match_state(),
        |r, _| vec![Out::R3(u32::from(r.game.ffa_outcome()))],
    );
}

#[test]
fn team_outcome() {
    check_calls(
        "gm_GetMatchOutcome",
        12,
        |f| {
            f.match_state();
            let players = sym("player_slots");
            for s in 0..6 {
                let t = f.below(3) as u8;
                f.put8(players + 0xE90 * s + 0x47, t);
            }
            let start = sym("controller") + 0x24C8;
            let k = f.u8(start) & 0x1F | (f.below(2) as u8) << 5;
            f.put8(start, k);
        },
        |r, _| vec![Out::R3(u32::from(r.game.team_outcome()))],
        |ctx, _| (fns::gm_GetTeamBattleOutcome(ctx) as u32, 0),
    );
}

#[test]
fn vs_pauser() {
    check_fuzzed(
        "gm_DefaultVSGetPauser",
        12,
        |f| f.buttons(),
        |r, _| vec![Out::R3(r.game.vs_pauser() as u32)],
    );
}

#[test]
fn vs_check_pause() {
    check_frame(
        "gm_DoPauseChecksAndRoutine",
        12,
        |f| {
            f.buttons();
            f.match_state();
        },
        |r, a| {
            assert_eq!(a.r(3), controller());
            r.vs_check_pause(a.r(4));
            vec![]
        },
    );
}

#[test]
fn vs_check_unpause() {
    check_calls_stubbed(
        "gm_DoPauseChecksAndRoutine",
        12,
        &FRAME_STUBS,
        |f| {
            f.buttons();
            f.match_state();
            let c = sym("controller");
            f.put8(c + 2, 0);
            let v = f.below(4) as u8;
            f.put8(c + 1, v);
        },
        |r, _| {
            r.vs_check_unpause(1);
            vec![]
        },
        |ctx, _| {
            fns::gm_DoUnpauseChecksAndRoutine(
                ctx,
                Handle::from_at(ssbm_rt::At::new(ctx, controller())),
                1,
            );
            (0, 0)
        },
    );
}

#[test]
fn share_stocks() {
    check_fuzzed(
        "fn_8016B918",
        8,
        |f| {
            f.buttons();
            f.match_state();
        },
        |r, _| {
            r.share_stocks();
            vec![]
        },
    );
}

#[test]
fn tick_platforms() {
    check_fuzzed(
        "fn_8016758C",
        6,
        |f| f.match_state(),
        |r, _| {
            r.game.vs_tick_platforms();
            vec![]
        },
    );
}

#[test]
fn spawn_opponents() {
    check("fn_8016A4C8", |r, _| {
        r.spawn_opponents();
        vec![]
    });
}

#[test]
fn vs_proc() {
    check("fn_8016C7D0", |r, _| {
        r.vs_proc();
        vec![]
    });
}

#[test]
fn update_ranks() {
    check_fuzzed(
        "fn_80171DC4",
        8,
        |f| {
            let players = sym("player_slots");
            for s in 0..6 {
                for k in 0..6 {
                    if f.below(6) == 0 {
                        let v = f.below(4);
                        f.put(players + 0xE90 * s + 0x70 + 4 * k, v);
                    }
                }
            }
        },
        |r, _| {
            r.game.update_ranks();
            vec![]
        },
    );
}

#[test]
fn rank_players() {
    check("fn_80171BA4", |r, a| {
        let at = a.r(3);
        let teams = a.cap.call.mem1[(at + 6 - MEM1) as usize];
        let penalty = a.cap.call.mem1[(at + 0xC - MEM1) as usize] as i8;
        vec![Out::R3(r.game.rank_players(teams, penalty) as u32)]
    });
}

#[test]
fn reset_ranks() {
    check("fn_80171AD4", |r, _| {
        r.game.reset_ranks();
        vec![]
    });
}

#[test]
fn vs_end() {
    check_calls_stubbed(
        "fn_8016CFE0",
        6,
        &[],
        |f| {
            let c = sym("controller");
            let o = [1, 2, 3, 7][f.below(4) as usize];
            f.put8(c + 8, o);
        },
        |r, _| {
            r.vs_end();
            vec![]
        },
        |ctx, _| {
            fns::fn_8016C7F0(ctx);
            (0, 0)
        },
    );
}

#[test]
fn respawn() {
    // The fighter's rebirth (`ftCo_800D4FF4`) isn't ported.
    check_calls_stubbed(
        "fn_8016CFE0",
        4,
        &[nothing(0x800D_4FF4)],
        |f| {
            f.regs.gpr[3] = f.below(2);
            f.regs.gpr[4] = 0;
        },
        |r, a| {
            r.respawn(a.i(3), a.i(4));
            vec![]
        },
        |ctx, call| {
            fns::fn_8016719C(ctx, call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
            (0, 0)
        },
    );
}

#[test]
fn respawn_point() {
    check_fuzzed(
        "fn_80167638",
        8,
        |f| {
            // Which respawn platform places are taken.
            let fighters = sym("controller") + 0x38;
            for k in 0..6 {
                let taken = f.below(3) == 0;
                f.put8(fighters + 0xE * k + 8, u8::from(taken));
            }
            // Whether players respawn at their own entry points (`unk8C.b4`).
            let flags = sym("stage_info") + 0x8C;
            if f.below(2) == 0 {
                let b = f.u8(flags);
                f.put8(flags, b ^ 0x08);
            }
        },
        |r, a| {
            let (point, pos, off) = r.respawn_point(a.i(3));
            vec![
                Out::R3(point as u32),
                Out::Words(a.r(4), vec_words(pos)),
                Out::Words(a.r(5), vec_words(off)),
            ]
        },
    );
}

// The pause screen.

#[test]
fn pause_proc() {
    check_fuzzed(
        "fn_801A0E34",
        6,
        |f| {
            let p = sym("lbl_80479B10");
            let slot = [99, 0, 1, 2, 3][f.below(5) as usize];
            f.put(p + 0x14, slot);
            let pads = sym("HSD_PadMasterStatus");
            for i in 0..4 {
                let x = f.rand.range(-1.0, 1.0);
                let y = f.rand.range(-1.0, 1.0);
                f.put(pads + 0x44 * i + 0x20, x.to_bits());
                f.put(pads + 0x44 * i + 0x24, y.to_bits());
            }
        },
        |r, a| {
            pause::proc(r, a.gobj(a.r(3)));
            vec![]
        },
    );
}

#[test]
fn pause_show() {
    check_calls(
        "fn_801A0E34",
        8,
        |f| {
            f.regs.gpr[3] = f.below(4);
            f.regs.gpr[4] = f.below(8);
        },
        |r, a| {
            let g = &mut *r.game;
            pause::show(&mut g.objs, r.data, &mut g.gm.pause, a.i(3), a.r(4) as u8);
            vec![]
        },
        |ctx, call| {
            fns::gm_801A0FEC(ctx, call.regs.gpr[3] as i32, call.regs.gpr[4] as u8);
            (0, 0)
        },
    );
}

#[test]
fn pause_hide() {
    check_calls(
        "fn_801A0E34",
        2,
        |_| {},
        |r, _| {
            r.game.hide_pause(0);
            vec![]
        },
        |ctx, _| {
            fns::gm_801A10FC(ctx, 0);
            (0, 0)
        },
    );
}

// The match's start.

#[test]
fn init_vs() {
    check_fuzzed(
        "fn_8016DCC0",
        8,
        |f| {
            // The timer's kind (`timer_enabled`, `timer_counts_up`) and its starting second.
            let d = f.regs.gpr[3];
            let b = f.u8(d) & !3 | f.below(4) as u8;
            f.put8(d, b);
            let second = [0, 1, 30, 60][f.below(4) as usize];
            f.put8(d + 0x14, second);
        },
        |r, a| {
            let d = StartMeleeData::import(&a.mem(), a.r(3), &mut NoPointers);
            r.game.init_vs(&d);
            vec![]
        },
    );
}

#[test]
fn init_vs_player() {
    check_fuzzed(
        "fn_8016D8AC",
        6,
        |f| {
            // The slot a player's data names (0 for its own).
            let d = f.regs.gpr[4];
            let slot = f.below(5) as u8;
            f.put8(d + 4, slot);
        },
        |r, a| {
            let d = PlayerInitData::import(&a.mem(), a.r(4), &mut NoPointers);
            r.game.init_vs_player(a.i(3), &d);
            vec![]
        },
    );
}

#[test]
fn face_opponents() {
    check_fuzzed(
        "fn_8016DEEC",
        8,
        |f| {
            let players = sym("player_slots");
            for s in 0..6 {
                let facing: f32 = [0.0, 0.0, 1.0, -1.0, -0.0][f.below(5) as usize];
                f.put(players + 0xE90 * s + 0x40, facing.to_bits());
                if f.below(4) == 0 {
                    let t = f.below(3) as u8;
                    f.put8(players + 0xE90 * s + 0x47, t);
                }
            }
            // The distance a first opponent inherits from the caller.
            f.regs.fpr[26].ps0 = [0.0, 20.0, 38.8, 100.0, -1.0][f.below(5) as usize];
        },
        |r, a| {
            r.face_opponents(a.cap.call.regs.fpr[26].ps0);
            vec![]
        },
    );
}

#[test]
fn kirby_costumes() {
    check("fn_8016E124", |r, _| {
        r.kirby_costumes();
        vec![]
    });
}

#[test]
fn alternate_stage() {
    check_fuzzed(
        "fn_8016E5C0",
        6,
        |f| f.buttons(),
        |r, a| {
            let d = StartMeleeData::import(&a.mem(), a.r(3), &mut NoPointers);
            vec![Out::R3(r.game.alternate_stage(&d) as u32)]
        },
    );
}

#[test]
fn player_color() {
    check("gm_80160854", |_, a| {
        let c = melee_sim::game::gm::vs::player_color(
            a.r(3) as u8,
            a.r(4) as u8,
            a.r(5) as u8,
            a.r(6) as u8,
        );
        vec![Out::R3(c)]
    });
}

#[test]
fn cap_color() {
    check("fn_80160840", |_, a| {
        vec![Out::R3(u32::from(melee_sim::game::gm::vs::cap_color(
            a.r(3) as u8,
        )))]
    });
}

#[test]
fn add_model_anim() {
    check("gm_8016895C", |r, a| {
        let j = (a.r(3) != 0).then(|| a.maps.jobjs[&a.r(3)]);
        let model = a.data_ref(a.r(4));
        melee_sim::game::gm::vs::add_model_anim(&mut r.game.objs, r.data, j, model, a.r(5));
        vec![]
    });
}

/// The rules hold no pointers into loaded data; their callbacks and records stay addresses.
struct NoPointers;

impl melee_sim::game::mirror::Cx for NoPointers {
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
        assert_eq!(addr, 0, "a {kind} in the match's rules");
        None
    }
    fn import_data(&mut self, addr: u32) -> Option<melee_data::archive::DataRef> {
        assert_eq!(addr, 0, "a data pointer in the match's rules");
        None
    }
    fn compare_handle(&mut self, _: &str, _: &'static str, _: Option<u32>, _: u32) {
        unreachable!()
    }
    fn compare_data(&mut self, _: &str, _: Option<melee_data::archive::DataRef>, _: u32) {
        unreachable!()
    }
    fn mismatch(&mut self, _: &str, _: String, _: String) {
        unreachable!()
    }
}

// The player library's resets.

#[test]
fn init_all_players() {
    check("Player_InitAllPlayers", |r, _| {
        r.game.init_all_players();
        vec![]
    });
}

#[test]
fn reset_player() {
    check("Player_InitOrResetPlayer", |r, a| {
        r.game.reset_player(a.i(3));
        vec![]
    });
}

#[test]
fn reset_player_records() {
    check("pl_8003891C", |r, a| {
        r.game.reset_player_records(a.i(3));
        vec![]
    });
}

#[test]
fn reset_action_stats() {
    check("plAttack_8003759C", |r, a| {
        r.game.reset_action_stats(a.i(3));
        vec![]
    });
}

#[test]
fn reset_stale_moves() {
    check("plStale_ResetStaleMoveTableForPlayer", |r, a| {
        r.game.reset_stale_moves(a.i(3));
        vec![]
    });
}

#[test]
fn reset_bonus_counts() {
    check("fn_80038FB8", |r, a| {
        r.game.reset_bonus_counts(a.i(3));
        vec![]
    });
}

#[test]
fn reset_frame_counters() {
    check("pl_80040DDC", |r, a| {
        r.game.reset_frame_counters(a.i(3));
        vec![]
    });
}

#[test]
fn reset_x8c0() {
    check("pl_80041524", |r, a| {
        r.game.reset_x8c0(a.i(3));
        vec![]
    });
}

#[test]
fn reset_x8d8() {
    check("pl_800418F4", |r, a| {
        r.game.reset_x8d8(a.i(3));
        vec![]
    });
}

#[test]
fn forget_player_records() {
    check_calls(
        "pl_8003891C",
        6,
        |f| f.regs.gpr[3] = f.below(6),
        |r, a| {
            r.game.forget_player_records(a.i(3));
            vec![]
        },
        |ctx, call| {
            fns::pl_80038F10(ctx, call.regs.gpr[3] as i32);
            (0, 0)
        },
    );
}

// The player blocks' accessors that reach fighters or other blocks.

#[test]
fn player_set_damage() {
    check("Player_SetHUDDamage", |r, a| {
        r.set_player_damage(a.i(3), a.i(4));
        vec![]
    });
}

#[test]
fn player_set_cpu_level() {
    check("Player_SetPlayerAndEntityCpuLevel", |r, a| {
        r.set_player_cpu_level(a.i(3), a.i(4));
        vec![]
    });
}

#[test]
fn player_set_cpu_type() {
    check("Player_SetPlayerAndEntityCpuType", |r, a| {
        r.set_player_cpu_type(a.i(3), a.i(4));
        vec![]
    });
}

/// The CPU setters from a running match's states, whose slots have fighters
/// (`ft_80087BEC`, `ft_80087BC0`: the fighters' computer player state started afresh).
#[test]
fn player_set_cpu_on_fighters() {
    check_calls(
        "fn_8016CFE0",
        6,
        |f| {
            f.regs.gpr[3] = f.below(4);
            f.regs.gpr[4] = f.below(10);
            f.regs.gpr[5] = f.below(2);
        },
        |r, a| {
            if a.r(5) == 0 {
                r.set_player_cpu_level(a.i(3), a.i(4));
            } else {
                r.set_player_cpu_type(a.i(3), a.i(4));
            }
            vec![]
        },
        |ctx, call| {
            let (s, v) = (call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
            if call.regs.gpr[5] == 0 {
                fns::Player_SetPlayerAndEntityCpuLevel(ctx, s, v);
            } else {
                fns::Player_SetPlayerAndEntityCpuType(ctx, s, v);
            }
            (0, 0)
        },
    );
}

#[test]
fn player_set_pos() {
    check("Player_80032768", |r, a| {
        let v = a.vec(a.r(4));
        r.game.set_player_pos(a.i(3), v);
        vec![]
    });
}

#[test]
fn player_set_entity_pos() {
    check("Player_80032828", |r, a| {
        let v = a.vec(a.r(5));
        r.game.set_player_entity_pos(a.i(3), a.i(4), v);
        vec![]
    });
}

#[test]
fn player_falls() {
    check("Player_GetFalls", |r, a| {
        vec![Out::R3(r.game.player_falls(a.i(3)) as u32)]
    });
}

#[test]
fn player_remaining_hp() {
    check("Player_GetRemainingHP", |r, a| {
        vec![Out::R3(r.game.player_remaining_hp(a.i(3)) as u32)]
    });
}

#[test]
fn player_awake() {
    check("Player_8003221C", |r, a| {
        vec![Out::R3(u32::from(r.game.player_awake(a.i(3))))]
    });
}

#[test]
fn player_kind() {
    check("Player_8003248C", |r, a| {
        vec![Out::R3(r.game.player_kind(a.i(3), a.i(4)) as u32)]
    });
}

#[test]
fn player_rumbles() {
    check("Player_800353BC", |r, a| {
        vec![Out::R3(u32::from(r.game.player_rumbles(a.i(3))))]
    });
}

#[test]
fn rumble_enabled() {
    check("Player_8003544C", |r, a| {
        vec![Out::R3(u32::from(r.game.rumble_enabled(a.i(3), a.i(4))))]
    });
}

#[test]
fn player_fighter_kind() {
    check("Player_80036394", |r, a| {
        vec![Out::R3(r.game.player_fighter_kind(a.i(3)) as u32)]
    });
}

#[test]
fn player_count_ko() {
    check_stubbed("Player_UpdateKOsBySlot", &[], |r, a| {
        r.player_count_ko(a.i(3), a.r(4) != 0, a.i(5));
        vec![]
    });
}

#[test]
fn player_count_self_destruct() {
    check_stubbed("Player_IncSelfDestructs", &[], |r, a| {
        r.player_count_self_destruct(a.i(3), a.i(4));
        vec![]
    });
}

#[test]
fn player_lose_stock() {
    check_calls(
        "fn_8016B918",
        6,
        |f| f.regs.gpr[3] = f.below(6),
        |r, a| {
            r.game.player_lose_stock(a.i(3));
            vec![]
        },
        |ctx, call| {
            fns::Player_LoseStock(ctx, call.regs.gpr[3] as i32);
            (0, 0)
        },
    );
}

// The player blocks' seams into the fighters, with the fighters' functions stood in for.

/// A random slot in r3, and a second argument in r4.
fn slot_args(f: &mut Fuzz) {
    f.regs.gpr[3] = f.below(6);
    f.regs.gpr[4] = f.below(3);
}

#[test]
fn player_disable_input() {
    // With `ftLib_DisableInput` (and `Fighter_ResetInputData_80068854`) on the fighters.
    check_calls(
        "fn_8016CFE0",
        6,
        slot_args,
        |r, a| {
            r.player_disable_input(a.i(3));
            vec![]
        },
        |ctx, call| {
            fns::Player_80031790(ctx, call.regs.gpr[3] as i32);
            (0, 0)
        },
    );
}

#[test]
fn players_enable_input() {
    check_calls(
        "fn_8016CFE0",
        2,
        |_| {},
        |r, _| {
            r.players_enable_input();
            vec![]
        },
        |ctx, _| {
            fns::Player_80031900(ctx);
            (0, 0)
        },
    );
}

#[test]
fn player_create_fighters() {
    // Fighter_Create stood in for, returning no fighter.
    check_calls_stubbed(
        "fn_8016DEEC",
        6,
        &[(0x8006_8E98, Ret::Null)],
        |f| {
            f.regs.gpr[3] = f.below(6);
            let players = sym("player_slots");
            let s = f.regs.gpr[3];
            let ckind = [2, 14, 18, 19, 20][f.below(5) as usize];
            f.put(players + 0xE90 * s + 4, ckind);
            f.put(players + 0xE90 * s + 0xB8, 0);
        },
        |r, a| {
            r.player_create_fighters(a.i(3));
            vec![]
        },
        |ctx, call| {
            fns::Player_80031AD0(ctx, call.regs.gpr[3] as i32);
            (0, 0)
        },
    );
}

#[test]
fn player_costume_data() {
    check_calls_stubbed(
        "fn_8016CFE0",
        8,
        &[
            nothing(0x8008_55C8),
            nothing(0x8008_578C),
            nothing(0x800E_ED50),
        ],
        |f| {
            f.regs.gpr[3] = f.below(33);
            f.regs.gpr[4] = f.below(6);
            f.regs.gpr[5] = f.below(3);
        },
        |r, a| {
            match a.r(5) {
                0 => r.player_load_costume(a.i(3), a.r(4) as u8),
                1 => r.player_80031d2c(a.i(3), a.r(4) as u8),
                _ => r.player_80031da8(a.i(3), a.i(4)),
            }
            vec![]
        },
        |ctx, call| {
            let (k, c) = (call.regs.gpr[3] as i32, call.regs.gpr[4]);
            match call.regs.gpr[5] {
                0 => fns::Player_80031CB0(ctx, k, c as u8),
                1 => fns::Player_80031D2C(ctx, k, c as u8),
                _ => fns::Player_80031DA8(ctx, k, c as i32),
            }
            (0, 0)
        },
    );
}

#[test]
fn player_first_init() {
    check_calls_stubbed(
        "fn_8016DEEC",
        1,
        &[nothing(0x8006_7A84)],
        |_| {},
        |r, _| {
            r.player_80036da4();
            vec![]
        },
        |ctx, _| {
            fns::Player_80036DA4(ctx);
            (0, 0)
        },
    );
}

#[test]
fn player_nametag_height() {
    // With `ftLib_GetNameTagHeight` on the fighter, visible or not.
    check_calls(
        "fn_8016CFE0",
        4,
        |f| {
            f.regs.gpr[3] = f.below(2);
            let players = sym("player_slots");
            let g = f.u32(players + 0xE90 * f.regs.gpr[3] + 0xB0);
            if g != 0 && f.below(2) == 0 {
                let fp = f.u32(g + 0x2C);
                let b = f.u8(fp + 0x221E);
                f.put8(fp + 0x221E, b | 0x80);
            }
        },
        |r, a| {
            let h = r.player_nametag_height(a.i(3));
            vec![Out::R3(gekko_fp::stfs(h))]
        },
        |ctx, call| {
            let h = fns::Player_800360D8(ctx, call.regs.gpr[3] as i32);
            (gekko_fp::stfs(h), 0)
        },
    );
}

#[test]
fn player_reinit() {
    check_calls(
        "fn_8016CFE0",
        6,
        slot_args,
        |r, a| {
            if a.r(4) == 0 {
                r.game.reinit_player(a.i(3));
            } else {
                r.game.replace_player(a.i(3));
            }
            vec![]
        },
        |ctx, call| {
            let s = call.regs.gpr[3] as i32;
            if call.regs.gpr[4] == 0 {
                fns::Player_80036CF0(ctx, s);
            } else {
                fns::Player_80036D24(ctx, s);
            }
            (0, 0)
        },
    );
}

#[test]
fn enable_match_camera() {
    check_calls(
        "fn_8016CFE0",
        1,
        |_| {},
        |r, _| {
            r.enable_match_camera();
            vec![]
        },
        |ctx, _| {
            fns::Ground_EnableMatchCamera(ctx);
            (0, 0)
        },
    );
}

#[test]
fn spawn_point() {
    check_calls(
        "fn_8016DEEC",
        8,
        |f| f.regs.gpr[3] = f.below(5),
        |r, a| {
            let v = r.spawn_point(a.i(3));
            vec![Out::Words(leaf_check::SCRATCH, vec_words(v))]
        },
        |ctx, call| {
            fns::Stage_80224E64(
                ctx,
                call.regs.gpr[3] as i32,
                leaf_check::handle(ctx, leaf_check::SCRATCH),
            );
            (0, 0)
        },
    );
}

#[test]
fn pause_tree_nodes() {
    check_calls(
        "fn_801A0E34",
        8,
        |f| f.regs.gpr[3] = f.below(14),
        |r, a| {
            let root = r.game.gm.pause.background;
            let n = pause::tree_nodes(&r.game.objs.jobjs, root, &[a.i(3)])[0];
            let addr = n.map_or(0, |j| {
                *a.maps
                    .jobjs
                    .iter()
                    .find(|(_, v)| **v == j)
                    .expect("a captured joint")
                    .0
            });
            vec![
                Out::R3(u32::from(n.is_some())),
                Out::Words(leaf_check::SCRATCH, vec![addr]),
            ]
        },
        |ctx, call| {
            let root = ctx.read_u32(sym("lbl_80479B10"));
            let out = leaf_check::SCRATCH;
            ctx.write_u32(out, 0);
            // `lb_80011E24(root, &out, index, -1)`, its indices passed as variadic words.
            ctx.regs.set_r(3, root);
            ctx.regs.set_r(4, out);
            ctx.regs.set_r(5, call.regs.gpr[3]);
            ctx.regs.set_r(6, u32::MAX);
            ctx.invoke(sym("lb_80011E24"));
            (ctx.regs.r(3), 0)
        },
    );
}

#[test]
fn vs_quit() {
    check_calls_stubbed(
        "fn_8016CFE0",
        6,
        &FRAME_STUBS,
        |f| {
            f.regs.gpr[3] = f.below(4);
            f.regs.gpr[4] = [7, 8][f.below(2) as usize];
        },
        |r, a| {
            r.vs_quit(a.i(3), a.r(4) as u8);
            vec![]
        },
        |ctx, call| {
            fns::fn_8016CF4C(ctx, call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
            (0, 0)
        },
    );
}

#[test]
fn place_players() {
    // Fighter_Create returns no fighter; the fighters' first setup and Kirby's copies are
    // stood in for.
    check_calls_stubbed(
        "fn_8016DEEC",
        6,
        &[
            (0x8006_8E98, Ret::Null),
            nothing(0x8006_7A84),
            nothing(0x800E_ED50),
        ],
        |f| {
            let players = sym("player_slots");
            for s in 0..6 {
                if f.below(4) == 0 {
                    let ckind = [2, 4, 14, 19][f.below(4) as usize];
                    f.put(players + 0xE90 * s + 4, ckind);
                }
                let facing: f32 = [0.0, 0.0, 1.0, -1.0][f.below(4) as usize];
                f.put(players + 0xE90 * s + 0x40, facing.to_bits());
                f.put(players + 0xE90 * s + 0xB8, 0);
            }
        },
        |r, _| {
            r.place_players();
            vec![]
        },
        |ctx, _| {
            fns::fn_8016E2BC(ctx);
            (0, 0)
        },
    );
}

#[test]
fn player_common_data() {
    // `Player_80036DD8` loads `PdPm.dat` and keeps its common data's pointer; from a match's
    // state, ours must find the pointer the original kept, in the archive loaded there.
    let paths = calls("fn_8016CFE0");
    let Some(path) = paths.first() else { return };
    if std::env::var("SSBM_DISC").is_err() {
        return;
    }
    let cap = Capture::read(path);
    let mut archives = Archives::default();
    let data = DataMap {
        loaded: find_archives_in(disc_archives(), &cap.call.mem1, &mut archives),
    };
    let pdpm = data
        .loaded
        .iter()
        .find(|l| l.name == "PdPm.dat")
        .expect("PdPm.dat loaded")
        .id;
    let mut game = melee_sim::game::state::Game::default();
    game.load_player_common(&archives, pdpm);
    let root = game.pl.load_common.expect("the common data's root");
    let first = archives.ptr(root).map(|r| data.addr(r));
    assert_eq!(first, Some(cap.u32(sym("pl_804D6470"))));
}

#[test]
fn vs_announce_entry() {
    // grStadium_801D3B4C, Pokémon Stadium's screen, isn't ported.
    check_calls_stubbed(
        "fn_8016CFE0",
        4,
        &[nothing(0x801D_3B4C)],
        |f| {
            f.regs.gpr[3] = f.below(4);
            f.regs.gpr[4] = f.below(4);
        },
        |r, a| {
            r.vs_announce_entry(a.i(3), a.r(4) as u8);
            vec![]
        },
        |ctx, call| {
            fns::gm_8016B8D4(ctx, call.regs.gpr[3] as i32, call.regs.gpr[4] as u8);
            (0, 0)
        },
    );
}

#[test]
fn enable_pause_camera() {
    check_calls(
        "fn_8016CFE0",
        8,
        |f| {
            f.regs.gpr[3] = [0xFFFF_FFFF, 0, 1, 2, 3][f.below(5) as usize];
            f.regs.gpr[4] = f.below(4);
        },
        |r, a| {
            r.game.enable_pause_camera(a.i(3) as i8, a.i(4));
            vec![]
        },
        |ctx, call| {
            fns::gm_EnablePlayerPauseCamera(ctx, call.regs.gpr[3] as i32, call.regs.gpr[4] as i32);
            (0, 0)
        },
    );
}

#[test]
fn pause_create() {
    // `fn_801A1134` loads `GmPause.dat`, so no capture saves it; the screen it builds stays
    // hidden and unchanged until a pause, so a match's captured screen is what it built. Ours,
    // built from the loaded archive on an empty game, must be that tree.
    let paths = calls("fn_801A0E34");
    if paths.is_empty() || std::env::var("SSBM_DISC").is_err() {
        return;
    }
    let mut checked = 0;
    for path in paths.iter().take(4) {
        let cap = Capture::read(path);
        if cap.u32(sym("lbl_80479B10") + 0x14) != 99 {
            continue;
        }
        let mut archives = Archives::default();
        let data = DataMap {
            loaded: find_archives_in(disc_archives(), &cap.call.mem1, &mut archives),
        };
        let gm_pause = data
            .loaded
            .iter()
            .find(|l| l.name.starts_with("GmPause"))
            .expect("GmPause.dat loaded")
            .id;
        let mut game = melee_sim::game::state::Game::default();
        pause::create(
            &mut Running {
                game: &mut game,
                data: &archives,
            },
            gm_pause,
        );
        let p = &game.gm.pause;
        let theirs = sym("lbl_80479B10");
        let mut cmp = Comparer::new(&cap.call.mem1, &data, &game, Maps::default());
        cmp.jobj_tree("pause screen", p.background, cap.u32(theirs));
        for (i, j) in [p.analog_stick, p.lras, p.z, p.analog_stick_outline]
            .into_iter()
            .enumerate()
        {
            let t = cap.u32(theirs + 4 * (i as u32 + 1));
            let ours = j.map(|j| {
                cmp.maps
                    .jobjs
                    .iter()
                    .find(|(_, v)| **v == j)
                    .map(|(a, _)| *a)
            });
            if ours != Some(Some(t)) {
                cmp.diffs
                    .push(format!("pause part {i}: ours {ours:?}, theirs {t:#x}"));
            }
        }
        let g = game.gm.pause.gobj.expect("the pause screen's GObj");
        let o = game.world.gobj(g);
        let tg = cap.u32(sym("lbl_804D6704"));
        let theirs_g = (
            cap.call.mem1[(tg + 2 - MEM1) as usize],
            cap.call.mem1[(tg + 3 - MEM1) as usize],
            cap.u32(cap.u32(tg + 0x18) + 0x14),
        );
        if (
            o.p_link,
            o.gx_link,
            o.proc.map(|p| game.world.proc(p).on_invoke.0),
        ) != (theirs_g.0, theirs_g.1, Some(theirs_g.2))
        {
            cmp.diffs.push(format!(
                "pause GObj: ours {:?}, theirs {theirs_g:?}",
                (o.p_link, o.gx_link)
            ));
        }
        for d in cmp.diffs.iter().take(20) {
            eprintln!("  {d}");
        }
        assert!(
            cmp.diffs.is_empty(),
            "{}: {} differences",
            path.display(),
            cmp.diffs.len()
        );
        checked += 1;
    }
    eprintln!("fn_801A1134: {checked} captured pause screens match");
    assert!(checked > 0);
}

fn nothing_native(_: &Ctx) {}

#[test]
fn pacing_init() {
    // `lb_80019AAC` runs its argument (the pads' setup) first: stood in for by nothing.
    let paths = calls("fn_8016CFE0");
    let Some(path) = paths.first() else { return };
    let Some(ctx) = hsd_reference() else { return };
    for name in [
        "PADSetSamplingRate",
        "OSCancelAlarm",
        "OSCreateAlarm",
        "OSSetPeriodicAlarm",
    ] {
        ctx.register(sym(name), nothing_native);
    }
    let cap = Capture::read(path);
    cap.call.load(&ctx);
    let f = leaf_check::SCRATCH;
    ctx.register(f, nothing_native);
    fns::lb_80019AAC(&ctx, leaf_check::handle(&ctx, f));
    let ours = melee_sim::lb::pacing::Pacing::new();
    let at = sym("lb_804329F0");
    let w64 = |a: u32| u64::from(ctx.read_u32(a)) << 32 | u64::from(ctx.read_u32(a + 4));
    let theirs = (
        [
            w64(at) as i64,
            w64(at + 8) as i64,
            ctx.read_u32(at + 0x10).into(),
        ],
        [
            w64(at + 0x18) as i64,
            w64(at + 0x20) as i64,
            ctx.read_u32(at + 0x28).into(),
        ],
        ctx.read_u32(at + 0x30),
        w64(at + 0x38),
        w64(at + 0x40) as i64,
        ctx.read_u32(at + 0x48),
    );
    let c = |k: usize| {
        let c = &ours.clocks[k];
        [c.period, c.elapsed, i64::from(c.due)]
    };
    let mine = (
        c(0),
        c(1),
        ours.sampling_rate,
        ours.requested,
        ours.tick,
        u32::from(ours.alarm),
    );
    assert_eq!(mine, theirs);
}
