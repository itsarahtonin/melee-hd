//! ssbm-rs's verified ports, ready to call one function at a time: melee-sim's leaf functions
//! are checked against them input by input (`docs/oracle.md`, leaf functions).
//!
//! The ports run in ssbm-rs's runtime with the disc's `main.dol` loaded, so they find the
//! constants and tables the original code reads. Tests need `SSBM_DISC` and skip without it.
//!
//! Most checks also read captures ssbm-rs's runner made, kept out of the repository in
//! ssbm-rs's `local/` folder, which `SSBM_RS_LOCAL` names: each test's default capture
//! directory is a path in it (its own variable overrides it), and a test skips when its
//! captures aren't there. Replays beyond the benchmark matches come from `SLIPPI_REPLAYS`.

pub mod calls;

/// The float crate the checks use: ssbm-rs's `gekko-fp`, which the reference computes with,
/// unchanged. melee-sim computes with melee-hd's faster copy (`melee_sim::gekko_fp`), so the
/// checks compare the copy against the original; setting the mode here sets both.
pub mod fp {
    pub use gekko_fp::*;

    /// Sets the floating-point mode for the current thread, in ssbm-rs's crate (the
    /// reference's) and in melee-sim's copy alike.
    pub fn set_fp_mode(mode: FpMode) {
        gekko_fp::set_fp_mode(mode);
        melee_sim::gekko_fp::set_fp_mode(match mode {
            FpMode::Hardware => melee_sim::gekko_fp::FpMode::Hardware,
            FpMode::Slippi => melee_sim::gekko_fp::FpMode::Slippi,
        });
    }
}

use fp::{FpMode, set_fp_mode};
use melee_sim::mtx::Mtx34;
use melee_sim::quat::Quat;
use melee_sim::vec::Vec3;
use ssbm_rt::Ctx;

/// How many worker threads a check runs its saved calls on: `LEAF_CHECK_WORKERS`, 2 by
/// default, so a suite run with `--test-threads=2` keeps to about four cores.
pub fn workers() -> usize {
    std::env::var("LEAF_CHECK_WORKERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(2)
}

/// How many sets of saved calls a check that runs many runs at once, each on its own
/// [`workers`]: `LEAF_CHECK_SETS`, 4 by default.
pub fn sets() -> usize {
    std::env::var("LEAF_CHECK_SETS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n > 0)
        .unwrap_or(4)
}

/// `f` of each of `items`, [`sets`] at a time, the heaviest by `weight` first: the results in
/// the order of `items`, as running them one after another would give them. A check that runs
/// many sets of saved calls one after another (each a capture set's calls of one function,
/// with what each runs) runs them this way, so a test of hundreds of small sets keeps several
/// cores busy. When some panic, the others still run, then the first of `items` to panic
/// panics again, as it would have stopped the check run in order.
pub fn each_set<T: Sync, R: Send>(
    items: &[T],
    weight: impl Fn(&T) -> usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(weight(&items[i])));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<std::thread::Result<R>>>> =
        Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..sets().min(items.len()) {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(s, || {
                    loop {
                        let k = next.fetch_add(1, Ordering::Relaxed);
                        let Some(&i) = order.get(k) else { break };
                        let r = catch_unwind(AssertUnwindSafe(|| f(&items[i])));
                        results.lock().unwrap()[i] = Some(r);
                    }
                })
                .unwrap();
        }
    });
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| match r.expect("every item ran") {
            Ok(r) => r,
            Err(p) => resume_unwind(p),
        })
        .collect()
}

/// Where the reference's stack starts: MEM1 above everything the DOL loads.
const STACK_TOP: u32 = 0x817F_0000;

/// ssbm-rs's runtime with the game's executable loaded and every port registered, or `None`
/// when `SSBM_DISC` is unset.
pub fn reference() -> Option<Ctx> {
    let Ok(path) = std::env::var("SSBM_DISC") else {
        eprintln!("SSBM_DISC is unset: skipping the comparison with ssbm-rs");
        return None;
    };
    // The executable, read (and its hash checked) once: each check's workers make their own
    // references.
    static DOL: std::sync::OnceLock<ssbm_disc::Dol> = std::sync::OnceLock::new();
    let dol = DOL.get_or_init(|| {
        let disc = ssbm_disc::Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        disc.main_dol().unwrap_or_else(|e| panic!("{path}: {e}"))
    });
    let ctx = Ctx::new();
    dol.load_into(&ctx.mem).expect("main.dol fits in memory");
    ssbm_game::register(&ctx, |_| true);
    ctx.register(sym("lbArq_80014BD0"), aram_read);
    // The audio interface's stream volume is a register for the disc's audio stream, which
    // the game doesn't play: nothing in memory changes.
    ctx.register(sym("AISetStreamVolLeft"), |_| {});
    ctx.register(sym("AISetStreamVolRight"), |_| {});
    // The disc drive is outside memory too: a read the device command queue starts goes out
    // and nothing more (its end is the audio timeline's).
    ctx.register(sym("DVDReadAsyncPrio"), |ctx| ctx.regs.set_r(3, 1));
    // So is the ARAM's DMA: a transfer the ARAM queue starts goes out and nothing more.
    ctx.register(sym("ARStartDMA"), |_| {});
    ctx.regs.set_r(1, STACK_TOP);
    // r2 and r13 as `__init_registers` sets them: its lis/ori pairs give the small-data bases.
    let init = sym("__init_registers");
    let pair = |at: u32| (ctx.read_u32(at) & 0xFFFF) << 16 | (ctx.read_u32(at + 4) & 0xFFFF);
    ctx.regs.set_r(2, pair(init + 8));
    ctx.regs.set_r(13, pair(init + 16));
    ssbm_types::fns::__sinit_trigf_c(&ctx);
    Some(ctx)
}

/// The disc's index of archives, built once, or `None` when `SSBM_DISC` is unset.
pub fn disc_index() -> Option<&'static melee_capture::DiscIndex> {
    static INDEX: std::sync::OnceLock<Option<melee_capture::DiscIndex>> =
        std::sync::OnceLock::new();
    INDEX
        .get_or_init(|| {
            let path = std::env::var("SSBM_DISC").ok()?;
            let disc =
                melee_data::disc::Disc::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
            Some(melee_capture::disc_index(&disc))
        })
        .as_ref()
}

/// `lbArq_80014BD0`, reading ARAM, which the reference doesn't have: a fighter's animation
/// file is all the game keeps there, so the copy comes from the disc's file at the offset
/// the game put it (`ftData_Table_Unk0`). The request queue's bookkeeping isn't kept, nor
/// the completion callback run.
fn aram_read(ctx: &Ctx) {
    use melee_capture::{FT_ANIM_FILE_NAMES, FT_ANIM_FILES, FT_KINDS};
    let (src, dest, len, callback) = (ctx.regs.r(3), ctx.regs.r(4), ctx.regs.r(5), ctx.regs.r(6));
    assert_eq!(callback, 0, "lbArq_80014BD0 with a callback isn't modeled");
    let index = disc_index().expect("SSBM_DISC");
    let file = (0..FT_KINDS)
        .filter_map(|kind| {
            let base = ctx.read_u32(FT_ANIM_FILES + 8 * kind);
            if base == 0 || base > src {
                return None;
            }
            let mut name = Vec::new();
            let mut a = ctx.read_u32(FT_ANIM_FILE_NAMES + 4 * kind);
            while ctx.read_u8(a) != 0 {
                name.push(ctx.read_u8(a));
                a += 1;
            }
            let bytes = index.anim_file(&String::from_utf8_lossy(&name))?;
            ((src - base) < bytes.len() as u32).then_some((base, bytes))
        })
        .max_by_key(|(base, _)| *base);
    let (base, bytes) = file.unwrap_or_else(|| panic!("ARAM {src:#x} isn't an animation file"));
    for i in 0..len {
        let b = bytes.get((src - base + i) as usize).copied().unwrap_or(0);
        ctx.write_u8(dest + i, b);
    }
}

/// The reference with HAL's heap and object allocators set up as `HSD_InitComponent` does
/// (an OS heap from 0x80F00000 to 0x81500000, then `HSD_ObjInit`'s allocators), so ports
/// that allocate HSD objects (joints, display objects, animations, ID-table entries) run.
pub fn hsd_reference() -> Option<Ctx> {
    use ssbm_rt::{Addr, Handle};
    let ctx = reference()?;
    // The original reports a failed assertion over the serial port, which isn't there:
    // surface it as a panic with its message instead.
    ctx.register(sym("__assert"), |ctx| {
        let s = |r: usize| {
            let mut out = Vec::new();
            let mut a = ctx.regs.r(r);
            while a != 0 && out.len() < 256 {
                let b = ctx.read_u8(a);
                if b == 0 {
                    break;
                }
                out.push(b);
                a += 1;
            }
            String::from_utf8_lossy(&out).into_owned()
        };
        panic!(
            "the original's assertion failed: {}:{}: {}",
            s(3),
            ctx.regs.r(4),
            s(5)
        );
    });
    let (lo, hi) = (HEAP_LO, HEAP_HI);
    let lo =
        ssbm_types::fns::OSInitAlloc(&ctx, handle::<Addr>(&ctx, lo), handle::<Addr>(&ctx, hi), 1)
            .addr();
    let lo = (lo + 31) & !31;
    let heap =
        ssbm_types::fns::OSCreateHeap(&ctx, handle::<Addr>(&ctx, lo), handle::<Addr>(&ctx, hi));
    ssbm_types::fns::OSSetCurrentHeap(&ctx, heap);
    ctx.write_u32(sym("current_heap"), heap as u32);
    ssbm_types::fns::HSD_ObjSetHeap(&ctx, hi - lo, handle::<Addr>(&ctx, 0));
    use ssbm_types::fns::*;
    for init in [
        HSD_ListInitAllocData,
        HSD_AObjInitAllocData,
        HSD_FObjInitAllocData,
        HSD_IDInitAllocData,
        HSD_VecInitAllocData,
        HSD_MtxInitAllocData,
        HSD_RObjInitAllocData,
        HSD_RenderInitAllocData,
        HSD_ShadowInitAllocData,
        HSD_ZListInitAllocData,
    ] {
        init(&ctx);
    }
    Some(ctx)
}

/// The OS heap `hsd_reference` sets up.
pub const HEAP_LO: u32 = 0x80F0_0000;
pub const HEAP_HI: u32 = 0x8150_0000;

/// A symbol's address.
pub fn sym(name: &str) -> u32 {
    // By name, the first of a name as the table lists them, from a map made once: the
    // checks look symbols up in their inner loops.
    static BY_NAME: std::sync::OnceLock<std::collections::HashMap<&str, u32>> =
        std::sync::OnceLock::new();
    let by_name = BY_NAME.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for s in ssbm_types::symbols::SYMBOLS {
            m.entry(s.2).or_insert(s.0);
        }
        m
    });
    *by_name
        .get(name)
        .unwrap_or_else(|| panic!("no symbol named {name}"))
}

/// A float as the original loads it into a register.
pub fn arg(x: f32) -> f64 {
    gekko_fp::lfs(x.to_bits())
}

/// A returned register's bits, as the original stores it.
pub fn bits(r: f64) -> u32 {
    gekko_fp::stfs(r)
}

/// Inputs that exercise a float function: special values, then pseudo-random bit patterns
/// and values spread over `range`, from a fixed seed so failures reproduce.
pub fn inputs(range: f32, count: usize) -> Vec<f32> {
    let mut v: Vec<f32> = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        std::f32::consts::PI,
        std::f32::consts::FRAC_PI_2,
        -std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_4,
    ]
    .to_vec();
    let mut s: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for _ in 0..count {
        v.push(f32::from_bits(next() as u32));
        let unit = (next() >> 40) as f32 / (1u64 << 24) as f32;
        v.push((unit * 2.0 - 1.0) * range);
    }
    v
}

/// Scratch memory for arguments passed by pointer, clear of the DOL and the stack.
pub const SCRATCH: u32 = 0x8160_0000;

/// A handle to `addr` for passing to a port.
pub fn handle<'a, H: ssbm_rt::Handle<'a>>(ctx: &'a Ctx, addr: u32) -> H {
    H::from_at(ssbm_rt::At::new(ctx, addr))
}

/// Writes floats to memory, as the game stores them.
pub fn write_f32s(ctx: &Ctx, addr: u32, values: &[f32]) {
    for (i, v) in values.iter().enumerate() {
        ctx.write_u32(addr + 4 * i as u32, v.to_bits());
    }
}

pub fn read_f32s(ctx: &Ctx, addr: u32, n: usize) -> Vec<u32> {
    (0..n).map(|i| ctx.read_u32(addr + 4 * i as u32)).collect()
}

/// A stream of floats for vectors, matrices and quaternions: mostly values in a game-like
/// range, with special values and random bit patterns mixed in.
pub struct Floats {
    values: Vec<f32>,
    at: usize,
}

impl Floats {
    pub fn new(range: f32) -> Self {
        Floats {
            values: inputs(range, 200_000),
            at: 0,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f32 {
        self.at = (self.at + 7) % self.values.len();
        self.values[self.at]
    }

    pub fn vec(&mut self) -> Vec3 {
        Vec3::new(self.next(), self.next(), self.next())
    }

    pub fn mtx(&mut self) -> Mtx34 {
        let mut m = Mtx34::default();
        for row in &mut m.m {
            for v in row {
                *v = self.next();
            }
        }
        m
    }

    pub fn quat(&mut self) -> Quat {
        Quat::new(self.next(), self.next(), self.next(), self.next())
    }
}

pub fn put_vec(ctx: &Ctx, addr: u32, v: Vec3) {
    write_f32s(ctx, addr, &[v.x, v.y, v.z]);
}

pub fn put_mtx(ctx: &Ctx, addr: u32, m: &Mtx34) {
    write_f32s(ctx, addr, m.m.as_flattened());
}

pub fn put_quat(ctx: &Ctx, addr: u32, q: Quat) {
    write_f32s(ctx, addr, &[q.x, q.y, q.z, q.w]);
}

pub fn vec_bits(v: Vec3) -> Vec<u32> {
    vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

pub fn mtx_bits(m: &Mtx34) -> Vec<u32> {
    m.m.as_flattened().iter().map(|v| v.to_bits()).collect()
}

pub fn quat_bits(q: Quat) -> Vec<u32> {
    vec![q.x.to_bits(), q.y.to_bits(), q.z.to_bits(), q.w.to_bits()]
}

/// Both float modes the comparisons run in.
pub const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];

/// Runs `check` over `n` cases in each float mode, with a fresh stream of floats in `range`.
pub fn each_mode(n: usize, range: f32, mut check: impl FnMut(&Ctx, &mut Floats, FpMode)) {
    let Some(ctx) = reference() else { return };
    for mode in MODES {
        set_fp_mode(mode);
        let mut floats = Floats::new(range);
        for _ in 0..n {
            check(&ctx, &mut floats, mode);
        }
    }
    set_fp_mode(FpMode::Hardware);
}
/// Where [`load_archive`] keeps the reference's `HSD_Archive` and the file it parses.
const ARCHIVE: u32 = 0x8070_0000;
const FILE: u32 = 0x8080_0000;
/// The largest file [`load_archive`] takes, clear of the memory above it.
pub const FILE_MAX: usize = 0x0070_0000;

/// Loads a DAT file into the reference and parses it as the game does (`HSD_ArchiveParse`
/// relocates its pointers), returning the address of its data block. Each call replaces the
/// previous archive.
pub fn load_archive(ctx: &Ctx, file: &[u8]) -> u32 {
    assert!(file.len() <= FILE_MAX, "a {:#x}-byte archive", file.len());
    ctx.write_bytes(FILE, file);
    let parsed = ssbm_types::fns::HSD_ArchiveParse(
        ctx,
        handle(ctx, ARCHIVE),
        handle(ctx, FILE),
        file.len() as u32,
    );
    assert_eq!(parsed, 0, "HSD_ArchiveParse");
    ctx.read_u32(ARCHIVE + 0x20)
}

/// A DAT file holding `data`, with the words at `relocs` marked as pointers (offsets into
/// `data`) and no symbols: test data both melee-data and the reference can load.
pub fn dat(data: &[u8], relocs: &[u32]) -> Vec<u8> {
    let mut f = Vec::new();
    let size = 0x20 + data.len() + 4 * relocs.len();
    for w in [size as u32, data.len() as u32, relocs.len() as u32, 0, 0] {
        f.extend_from_slice(&w.to_be_bytes());
    }
    f.resize(0x20, 0);
    f.extend_from_slice(data);
    for r in relocs {
        f.extend_from_slice(&r.to_be_bytes());
    }
    f
}

/// A xorshift generator for test inputs, from a fixed seed so failures reproduce.
pub struct Rand(pub u64);

impl Rand {
    pub fn bits(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in [0, n).
    pub fn below(&mut self, n: u64) -> u64 {
        self.bits() % n
    }

    /// Uniform in [lo, hi].
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        let unit = (self.bits() >> 40) as f32 / (1u64 << 24) as f32;
        lo + (hi - lo) * unit
    }
}

/// An update function's value as the tests log it: the float's bits, or `None` for
/// `Value::Stale`.
pub fn logged(v: melee_sim::hsd::fobj::Value) -> Option<u32> {
    match v {
        melee_sim::hsd::fobj::Value::Float(f) => Some(f.to_bits()),
        melee_sim::hsd::fobj::Value::Stale => None,
        other => panic!("an FObj handed over {other:?}"),
    }
}

/// Whether our updates are theirs: the same channel types and values, a stale value standing
/// for whatever the original's stack held (see `Value::Stale`).
pub fn same_updates(ours: &[(u8, Option<u32>)], theirs: &[(u8, u32)]) -> bool {
    ours.len() == theirs.len()
        && ours
            .iter()
            .zip(theirs)
            .all(|(&(ot, ov), &(tt, tv))| ot == tt && ov.is_none_or(|v| v == tv))
}

/// The AnimJoint and MatAnimJoint roots of a stage's models (`map_head`'s model groups:
/// joint, then null-terminated lists of AnimJoints and MatAnimJoints).
pub fn stage_anim_roots(a: &melee_data::archive::Archive) -> (Vec<u32>, Vec<u32>) {
    let (mut anim, mut matanim) = (Vec::new(), Vec::new());
    let Some(head) = a.public("map_head") else {
        return (anim, matanim);
    };
    let Some(groups) = a.ptr(head + 8) else {
        return (anim, matanim);
    };
    for g in 0..a.u32(head + 0xC) {
        let group = groups + g * 0x34;
        for (at, out) in [(4, &mut anim), (8, &mut matanim)] {
            let mut list = a.ptr(group + at);
            while let Some(entry) = list {
                let Some(root) = a.ptr(entry) else { break };
                out.push(root);
                list = Some(entry + 4);
            }
        }
    }
    (anim, matanim)
}

/// Whether the reference runs the original's machine code for every function instead of
/// ssbm-rs's ports ([`run_as_machine_code`]).
static MACHINE_CODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Makes every captured call in this process run the original's machine code in ssbm-rs's
/// interpreter (`ssbm_ppc`), its ports set aside: for a check whose reference must be the console itself, where ssbm-rs's
/// ports at the tag leave a register otherwise (Nana's DI, `docs/engine.md`). A test binary
/// sets it once, for all its tests.
pub fn run_as_machine_code() {
    MACHINE_CODE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Runs a captured call in the reference from the state it was saved in, and returns main
/// memory afterwards.
pub fn run_capture(ctx: &Ctx, call: &ssbm_rt::capture::Call) -> Vec<u8> {
    call.load(ctx);
    register_patched(ctx);
    // Functions a player's code patches run as their machine code (`calls::Setup::original`),
    // even where the ruleset's codes gave them a port.
    ORIGINAL.with(|o| {
        for &f in o.borrow().iter() {
            ctx.set_mode(f, ssbm_rt::Mode::Original);
        }
    });
    if MACHINE_CODE.load(std::sync::atomic::Ordering::Relaxed) {
        if !ctx.has_backend() {
            ctx.set_backend(Box::new(ssbm_ppc::Interpreter::default()));
        }
        ctx.set_all_modes(ssbm_rt::Mode::Original);
    }
    ctx.invoke(call.addr);
    let mut out = vec![0; call.mem1.len()];
    let _ = ctx.mem.read_bytes(0x8000_0000, &mut out);
    out
}

thread_local! {
    /// The functions this thread's reference runs as machine code (`calls::Setup::original`).
    pub(crate) static ORIGINAL: std::cell::RefCell<Vec<u32>> = std::cell::RefCell::default();
}

/// ssbm-rs's own ports by address, to tell them from a check's stand-ins: made once for every
/// thread.
static PORTS: std::sync::LazyLock<std::collections::HashMap<u32, ssbm_rt::Native>> =
    std::sync::LazyLock::new(|| {
        let ctx = Ctx::new();
        ssbm_game::register(&ctx, |_| true);
        ctx.registered()
            .into_iter()
            .filter_map(|a| ctx.entry(a).map(|e| (a, e.native)))
            .collect()
    });

thread_local! {
    /// The ports [`register_patched`] replaced, to put back for a state without the codes.
    static REPLACED: std::cell::RefCell<std::collections::HashMap<u32, ssbm_rt::Entry>> =
        std::cell::RefCell::default();
}

/// Registers ssbm-rs's ports made with Slippi's playback codes (`ssbm_game::playback`) for
/// the functions those codes patch in the memory `ctx` holds, as ssbm-rs's runner does for a
/// replay run: a captured replay run's patched functions run as the run ran them. Each
/// function takes the port made for the most of its codes that memory has; a function whose
/// codes aren't there gets its own port back. A function a check stands something else in for
/// keeps its stand-in.
pub fn register_patched(ctx: &Ctx) {
    use ssbm_game::patched::{Code, PATCHED};
    // `Code::Inject`'s words are offsets into the injected instructions and what they hold.
    let read = |a: u32| ctx.mem.read_u32(a).unwrap_or(0);
    let applied = |site: u32, code: &Code| match *code {
        Code::Word(w) => read(site) == w,
        Code::Inject { len, words } => melee_sim::game::slippi::placed(site, read(site))
            .is_some_and(|body| {
                let _ = len;
                words.iter().all(|&(off, w)| read(body + off) == w)
            }),
    };
    let mut best: std::collections::HashMap<u32, &'static ssbm_game::patched::Patched> =
        std::collections::HashMap::new();
    for p in PATCHED {
        if p.sites.iter().all(|s| applied(s.addr, &s.code))
            && best
                .get(&p.function)
                .is_none_or(|b| b.sites.len() < p.sites.len())
        {
            best.insert(p.function, p);
        }
    }
    // The places in injected code that other code calls (a playback GObj's process), where
    // the codes sit in this memory.
    for e in ssbm_game::patched::ENTRIES {
        if applied(e.site, &e.code)
            && let Some(body) = melee_sim::game::slippi::placed(e.site, read(e.site))
        {
            ctx.register_port(body + e.offset, e.port, ssbm_rt::Returns::Unknown);
        }
    }
    REPLACED.with(|r| {
        let mut r = r.borrow_mut();
        let back: Vec<u32> = r
            .keys()
            .copied()
            .filter(|f| !best.contains_key(f))
            .collect();
        for f in back {
            let e = r.remove(&f).unwrap();
            ctx.register_port(f, e.native, e.returns);
        }
        for (&f, p) in &best {
            let original = ctx.entry(f);
            let stood_in =
                !r.contains_key(&f) && original.is_some_and(|e| PORTS.get(&f) != Some(&e.native));
            if stood_in {
                continue;
            }
            if let Some(e) = original {
                r.entry(f).or_insert(e);
            }
            let returns = original.map_or(ssbm_rt::Returns::Unknown, |e| e.returns);
            ctx.register_port(f, p.port, returns);
        }
    });
}
