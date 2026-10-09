//! melee-sim's environment collision (`mp::coll`) against ssbm-rs's `mpcoll.c` port, on every
//! stage on the disc in both float modes. Each stage's geometry is loaded on both sides (as
//! the `mplib` test does) and moved by that test's random mutations between rounds, so the
//! remapping searches see moving lines. Each round places collision data around the stage's
//! lines (on floors, against walls, under ceilings, beside ledges, through platforms, with
//! random moves and ECB shapes) in ssbm-rs's memory, imports it, calls every mpcoll function
//! on both sides, and compares the full collision data, mpcoll's statics, the joints' bounding
//! marks, the return value and every stage callback, filter call and dynamic terrain lookup.
//! Fighter ECBs from joints run on the stages' own model joints.

#[path = "../support/mpstage.rs"]
#[allow(dead_code)]
mod mpstage;
#[allow(dead_code)]
#[path = "../mplib/mutations.rs"]
mod mutations;
#[allow(dead_code)]
#[path = "../mplib/queries.rs"]
mod queries;
#[allow(dead_code)]
#[path = "../mplib/state.rs"]
mod state;

mod cases;
mod joints;
mod place;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use leaf_check::fp::{FpMode, set_fp_mode};
use leaf_check::{SCRATCH, bits, hsd_reference, sym};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::game::handles::CodeRef;
use melee_sim::game::mirror::{Cx, Mem, Mirror};
use melee_sim::game::structs::CollData;
use melee_sim::hsd::Objects;
use melee_sim::hsd::gobj::GObjId;
use melee_sim::hsd::jobj::JObjId;
use melee_sim::mp::coll::{Coll, CollHooks, CollStatics};
use melee_sim::mp::{Geometry, JOINT_TOO_FAR};
use melee_sim::vec::Vec3;
#[allow(unused_imports)]
pub use mpstage::trace;
use mpstage::{FILE_MAX, Rng, Stage, T, load_theirs, release, reset_boxes, stage_files};
use ssbm_rt::Ctx;
use ssbm_types::fns;

/// The collision data both sides work on, in ssbm-rs's memory, and a second one for copies.
pub const COLL: u32 = SCRATCH + 0x5000;
pub const COLL2: u32 = SCRATCH + 0x5200;
/// Scratch for arguments by pointer: a vector or box, and an int.
pub const OUT_VEC: u32 = SCRATCH + 0x5400;
pub const OUT_INT: u32 = SCRATCH + 0x5480;
const COLL_SIZE: u32 = 0x19C;

/// How many times round `mpColl_80046904`'s loop ours goes before the test gives up on the
/// original (converging loops take a few).
const WATCHDOG: u32 = 5000;

/// Fake addresses for the stage joints' two callbacks and the fighter floor filter.
pub const LEDGE_CB: u32 = 0x817E_0700;
pub const CEILING_CB: u32 = 0x817E_0740;
pub const FLOOR_FILTER: u32 = 0x817E_0780;

/// Something the outside world was asked during a call, logged on both sides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// A joint callback: the callback, user data, joint, collision data, x50, kind, dy bits.
    Joint([u32; 7]),
    /// The floor filter: fighter and line.
    Filter(u32, i32),
    /// A dynamic terrain lookup: position bits and line.
    Terrain(u32, u32, i32),
}

thread_local! {
    static THEIRS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
    /// What our side logs for the collision data a joint callback is given: `COLL`, or 1 for
    /// one on the original's stack (`mpColl_8004D024`'s).
    pub static OURS_COLL: std::cell::Cell<u32> = const { std::cell::Cell::new(COLL) };
}

/// The collision data a callback was given, as logged: `COLL`/`COLL2`, or 1 for any other.
fn coll_id(addr: u32) -> u32 {
    if addr == COLL || addr == COLL2 {
        addr
    } else {
        1
    }
}

fn log_theirs(c: Call) {
    THEIRS.with(|l| l.borrow_mut().push(c));
}

pub fn take_theirs() -> Vec<Call> {
    THEIRS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// The floor filter's decision: a fixed pseudo-random function of fighter and line.
pub fn decide(gobj: u32, line: i32) -> bool {
    !((gobj ^ (line as u32).wrapping_mul(0x9E37_79B9)) >> 5).is_multiple_of(3)
}

/// The dynamic terrain areas' answer: a fixed pseudo-random function of position and line,
/// mostly no area.
pub fn terrain(x: u32, y: u32, line: i32) -> i32 {
    let h =
        (x ^ y.rotate_left(13) ^ (line as u32).wrapping_mul(0x85EB_CA6B)).wrapping_mul(0x27D4_EB2F);
    if h >> 29 == 0 {
        ((h >> 8) % 20) as i32 + 0x100
    } else {
        0
    }
}

fn native_joint_cb(ctx: &Ctx) {
    let r = |i| ctx.regs.r(i);
    // The callback's own address isn't in a register; it's told apart by the user data.
    log_theirs(Call::Joint([
        0,
        r(3),
        r(4),
        coll_id(r(5)),
        r(6),
        r(7),
        bits(ctx.regs.f(1)),
    ]));
}

fn native_filter(ctx: &Ctx) {
    let (gobj, line) = (ctx.regs.r(3), ctx.regs.r(4) as i32);
    log_theirs(Call::Filter(gobj, line));
    ctx.regs.set_r(3, u32::from(decide(gobj, line)));
}

fn native_terrain(ctx: &Ctx) {
    let (pos, line) = (ctx.regs.r(3), ctx.regs.r(4) as i32);
    let (x, y) = (ctx.read_u32(pos), ctx.read_u32(pos + 4));
    log_theirs(Call::Terrain(x, y, line));
    ctx.regs.set_r(3, terrain(x, y, line) as u32);
}

pub fn register_natives(ctx: &Ctx) {
    queries::register_natives(ctx);
    ctx.register(LEDGE_CB, native_joint_cb);
    ctx.register(CEILING_CB, native_joint_cb);
    ctx.register(FLOOR_FILTER, native_filter);
    // The stage's dynamic attribute areas aren't ported (stage code); both sides ask the same
    // stand-in.
    ctx.register(sym("grDynamicAttr_801CA284"), native_terrain);
    // The original reports a floor or ceiling that doesn't reach its own end ("oioi...")
    // over the serial port, which isn't there; reports aren't simulation.
    ctx.register(sym("OSReport"), |_| {});
}

/// Our side's hooks: the same stand-ins, logged, and the joints for fighter ECBs.
pub struct Hooks<'a> {
    pub calls: Vec<Call>,
    pub objs: &'a mut Objects,
    pub archives: &'a Archives,
}

impl CollHooks<T> for Hooks<'_> {
    fn dynamic_terrain(&mut self, _g: &Geometry<T>, pos: Vec3, line_id: i32) -> i32 {
        let (x, y) = (pos.x.to_bits(), pos.y.to_bits());
        self.calls.push(Call::Terrain(x, y, line_id));
        terrain(x, y, line_id)
    }

    fn joint_callback(
        &mut self,
        _g: &mut Geometry<T>,
        _callback: u32,
        user_data: Option<u32>,
        joint_id: i32,
        _coll: &mut CollData,
        x50: i32,
        kind: i32,
        dy: f32,
    ) {
        self.calls.push(Call::Joint([
            0,
            user_data.unwrap_or(0),
            joint_id as u32,
            OURS_COLL.get(),
            x50 as u32,
            kind as u32,
            dy.to_bits(),
        ]));
    }

    fn floor_filter(&mut self, callback: CodeRef, gobj: Option<GObjId>, line_id: i32) -> bool {
        assert_eq!(callback.0, FLOOR_FILTER, "the floor filter called");
        let g = gobj.map_or(0, |g| g.0);
        self.calls.push(Call::Filter(g, line_id));
        decide(g, line_id)
    }

    fn joint_position(&mut self, jobj: Option<JObjId>) -> Vec3 {
        melee_sim::lb::joint::world_position(self.objs, self.archives, jobj, None)
    }
}

/// How ssbm-rs's addresses map to our handles: fighters' GObjs by address, joints through
/// the table the joint tests fill.
#[derive(Default)]
pub struct Handles {
    pub jobjs: HashMap<u32, u32>,
    pub jobjs_back: HashMap<u32, u32>,
    pub what: String,
}

impl Cx for Handles {
    fn import_handle(&mut self, kind: &'static str, addr: u32) -> Option<u32> {
        if addr == 0 {
            return None;
        }
        match kind {
            "GObjId" => Some(addr),
            "JObjId" => Some(
                *self
                    .jobjs
                    .get(&addr)
                    .unwrap_or_else(|| panic!("{}: unknown joint {addr:#x}", self.what)),
            ),
            _ => panic!("{}: a {kind} handle in collision data", self.what),
        }
    }

    fn import_data(&mut self, addr: u32) -> Option<DataRef> {
        panic!("{}: game data at {addr:#x} in collision data", self.what)
    }

    fn compare_handle(&mut self, path: &str, kind: &'static str, ours: Option<u32>, theirs: u32) {
        let mapped = match (kind, ours) {
            (_, None) => 0,
            ("GObjId", Some(g)) => g,
            ("JObjId", Some(j)) => *self.jobjs_back.get(&j).unwrap_or(&u32::MAX),
            (k, _) => panic!("{}: a {k} handle in collision data", self.what),
        };
        assert_eq!(mapped, theirs, "{}: {path}", self.what);
    }

    fn compare_data(&mut self, path: &str, _ours: Option<DataRef>, theirs: u32) {
        panic!("{}: {path}: game data at {theirs:#x}", self.what)
    }

    fn mismatch(&mut self, path: &str, ours: String, theirs: String) {
        panic!("{}: {path}: ours {ours}, theirs {theirs}", self.what)
    }
}

fn coll_bytes(ctx: &Ctx, at: u32) -> Vec<u8> {
    (0..COLL_SIZE).map(|i| ctx.read_u8(at + i)).collect()
}

/// The collision data at `at` in ssbm-rs's memory, as ours.
pub fn import(ctx: &Ctx, at: u32, h: &mut Handles) -> CollData {
    let bytes = coll_bytes(ctx, at);
    CollData::import(
        &Mem {
            bytes: &bytes,
            base: at,
        },
        at,
        h,
    )
}

pub fn compare(ctx: &Ctx, at: u32, ours: &CollData, h: &mut Handles, path: &str) {
    let bytes = coll_bytes(ctx, at);
    ours.compare(
        &Mem {
            bytes: &bytes,
            base: at,
        },
        at,
        path,
        h,
    );
}

/// mpcoll's statics in ssbm-rs's memory.
pub fn statics_theirs(ctx: &Ctx) -> CollStatics {
    let r = |n: &str| ctx.read_u32(sym(n));
    let walls = sym("mpColl_80458810");
    let f = |a: u32| f32::from_bits(ctx.read_u32(a));
    let code = |a: u32| (a != 0).then_some(CodeRef(a));
    CollStatics {
        right_walls: std::array::from_fn(|i| ctx.read_u32(walls + 4 * i as u32) as i32),
        right_count: r("mpColl_804D6488") as i32,
        left_walls: std::array::from_fn(|i| ctx.read_u32(walls + 0x24 + 4 * i as u32) as i32),
        left_count: r("mpColl_804D648C") as i32,
        max_x: f(sym("mpColl_804D6490_max_x")),
        line_id: r("mpColl_804D6494_line_id") as i32,
        flags: r("mpColl_804D6498_flags"),
        normal: Vec3::new(f(walls + 0x48), f(walls + 0x4C), f(walls + 0x50)),
        ecb_tiny: r("mpColl_IsEcbTiny") != 0,
        floor_filter: code(r("mpColl_804D64A0")),
        floor_filter_gobj: (r("mpColl_804D64A4") != 0).then(|| GObjId(r("mpColl_804D64A4"))),
        x64a8: code(r("mpColl_804D64A8")),
        stale_reads: 0,
        watchdog: None,
    }
}

/// The statics as bits, so floats compare exactly.
fn statics_key(s: &CollStatics) -> String {
    let v = |v: Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
    format!(
        "right {:?}/{} left {:?}/{} max_x {:#x} line {} flags {:#x} normal {:x?} tiny {} filter {:?} {:?} {:?}",
        s.right_walls,
        s.right_count,
        s.left_walls,
        s.left_count,
        s.max_x.to_bits(),
        s.line_id,
        s.flags,
        v(s.normal),
        s.ecb_tiny,
        s.floor_filter,
        s.floor_filter_gobj,
        s.x64a8
    )
}

/// One stage on both sides, with mpcoll's statics and the round's state.
pub struct World<'a, 'c> {
    pub st: &'a mut Stage<'c>,
    pub statics: CollStatics,
    pub objs: Objects,
    pub archives: Archives,
    pub handles: Handles,
    pub counts: BTreeMap<String, u64>,
    /// How often each function returned nonzero.
    pub hits: BTreeMap<String, u64>,
    pub halts: u64,
    pub hangs: u64,
    pub stale: u64,
}

impl<'c> World<'_, 'c> {
    pub fn ctx(&self) -> &'c Ctx {
        self.st.ctx
    }

    /// Runs `ours` with a collision world borrowed from this one.
    pub fn with_coll<R>(
        &mut self,
        f: impl FnOnce(&mut Coll<'_, T, Hooks<'_>>) -> R,
    ) -> (R, Vec<Call>) {
        let mut hooks = Hooks {
            calls: Vec::new(),
            objs: &mut self.objs,
            archives: &self.archives,
        };
        let mut c = Coll {
            g: &mut self.st.g,
            s: &mut self.statics,
            hooks: &mut hooks,
        };
        let r = f(&mut c);
        (r, hooks.calls)
    }

    /// Checks everything a call may change: the collision data at `at`, the statics, the
    /// joints' bounding marks and whether they're checked, and the calls made outside.
    pub fn check(&mut self, ours: &CollData, at: u32, ours_calls: &[Call], what: &str) {
        let ctx = self.ctx();
        self.handles.what = format!("{}: {what}", self.st.what());
        compare(ctx, at, ours, &mut self.handles, "coll");
        let theirs = statics_theirs(ctx);
        assert_eq!(
            statics_key(&self.statics),
            statics_key(&theirs),
            "{}: statics",
            self.handles.what
        );
        let joints = ctx.read_u32(sym("groundCollJoint"));
        for (j, joint) in self.st.g.joints.iter().enumerate() {
            let flags = ctx.read_u32(joints + 0x34 * j as u32 + 8);
            assert_eq!(
                joint.flags & JOINT_TOO_FAR,
                flags & JOINT_TOO_FAR,
                "{}: joint {j} bounding mark",
                self.handles.what
            );
        }
        assert_eq!(
            self.st.g.checked_bounding,
            ctx.read_u32(sym("didCheckBounding")) != 0,
            "{}: bounding checked",
            self.handles.what
        );
        assert_eq!(
            self.st.g.generation,
            ctx.read_u32(sym("mpColl_804D64AC")) as i32
        );
        let theirs_calls = take_theirs();
        // ssbm-rs's joint callbacks don't see which callback they are; ours log 0 too.
        assert_eq!(
            ours_calls,
            &theirs_calls[..],
            "{}: calls",
            self.handles.what
        );
    }

    pub fn count(&mut self, name: &str) {
        *self.counts.entry(name.to_owned()).or_default() += 1;
    }
}

thread_local! {
    static HUNG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether the last [`both`] skipped the original because it would hang.
pub fn result_hung() -> bool {
    HUNG.get()
}

/// Runs ours, then the original. Where ours halts, the original must too (an assertion or a
/// fault), unless ours halted where the original hangs forever, which isn't run. Returns both
/// results when both carried on.
pub fn both<A, B>(
    ctx: &Ctx,
    what: &str,
    theirs: impl FnOnce() -> A,
    ours: impl FnOnce() -> B,
) -> Option<(A, B)> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    HUNG.set(false);
    let sp = ctx.regs.r(1);
    match catch_unwind(AssertUnwindSafe(ours)) {
        Ok(b) => Some((theirs(), b)),
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            if msg.contains("the original hangs") {
                HUNG.set(true);
                return None;
            }
            assert!(
                catch_unwind(AssertUnwindSafe(theirs)).is_err(),
                "{what}: ours halts here ({msg}), the original carries on"
            );
            ctx.regs.set_r(1, sp);
            None
        }
    }
}

#[test]
fn coll_matches() {
    std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(check_all)
        .unwrap()
        .join()
        .unwrap();
}

fn check_all() {
    let Some(ctx) = hsd_reference() else { return };
    let ctx = &ctx;
    register_natives(ctx);
    // Retail: mpCollPrev's and mpCollEnd's range checks are the debug ROM's.
    assert!(
        fns::DbLevel(ctx).get() < 2,
        "the reference runs as a debug ROM"
    );
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let files = disc.files().unwrap();
    let rounds: usize = std::env::var("MPCOLL_ROUNDS")
        .ok()
        .and_then(|r| r.parse().ok())
        .unwrap_or(6);
    let mut totals: BTreeMap<String, u64> = BTreeMap::new();
    let mut hits: BTreeMap<String, u64> = BTreeMap::new();
    let mut halts = 0;
    let mut stale = 0;
    let mut hangs = 0;
    let mut stages = 0;
    for (grkind, file) in stage_files(ctx) {
        let mut file = file.trim_start_matches('/').to_owned();
        if !file.contains('.') {
            file.push_str(".dat");
        }
        let Some(path) = files
            .iter()
            .find(|f| f.trim_start_matches('/').eq_ignore_ascii_case(&file))
        else {
            continue;
        };
        if let Ok(only) = std::env::var("MPCOLL_ONLY")
            && !file.contains(&only)
        {
            continue;
        }
        let bytes = disc.read_file(path).unwrap();
        let archive = Archive::parse(&bytes).unwrap();
        let Some(coll_off) = archive.public("coll_data") else {
            continue;
        };
        assert!(
            bytes.len() <= FILE_MAX,
            "{file} is too big for the test's buffer"
        );
        let scale = archive
            .public("grGroundParam")
            .map_or(1.0, |o| archive.f32(o));
        for mode in [FpMode::Hardware, FpMode::Slippi] {
            set_fp_mode(mode);
            let seed = file
                .bytes()
                .fold(0xC0FF_EE00_D15E_A5E5u64 ^ mode as u64, |h, b| {
                    (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01B3)
                });
            // Both sides count stage updates from zero.
            fns::mpColl_804D64AC(ctx).set(0);
            let coll = load_theirs(ctx, &bytes, &archive, grkind, true);
            let mut archives = Archives::default();
            let id: ArchiveId = archives.add(archive.clone());
            let g = Geometry::<T>::load(
                &archives,
                Some(DataRef {
                    archive: id,
                    offset: coll_off,
                }),
                scale,
                grkind,
            );
            let mut stage = Stage {
                ctx,
                name: format!("{file} (kind {grkind})"),
                grkind,
                g,
                coll,
                gen0: 0,
                islands: HashMap::new(),
                islands_back: HashMap::new(),
                rng: Rng(seed | 1),
                mode,
            };
            eprintln!("{}", stage.what());
            state::check(&mut stage, "after loading");
            reset_boxes(&mut stage);
            // The stage's first update gives dynamic lines their kinds; before it, Pokémon
            // Stadium's form closed runs of one kind, which the original's walks never leave.
            for j in 0..stage.g.joints.len() as i32 {
                if mutations::dynamics_safe(&stage, j as usize) {
                    fns::mpJointUpdateDynamics(ctx, j);
                    stage.g.update_dynamics(j);
                }
            }
            let mut w = World {
                st: &mut stage,
                // A loop running this long the original would run for ages (or for ever).
                statics: CollStatics {
                    watchdog: Some(WATCHDOG),
                    ..statics_theirs(ctx)
                },
                objs: Objects::default(),
                archives,
                handles: Handles::default(),
                counts: BTreeMap::new(),
                hits: BTreeMap::new(),
                halts: 0,
                hangs: 0,
                stale: 0,
            };
            let mut qcounts = queries::Counts::default();
            let base = ctx.read_u32(mpstage::ARCHIVE + 0x20);
            let tree = joints::load(&mut w, &archive, id, base);
            for round in 0..rounds {
                if round > 0 {
                    mutations::steps_with(w.st, &mut qcounts, 4, false);
                }
                cases::round(&mut w, tree.as_ref());
                state::check(w.st, &format!("after round {round}"));
            }
            if let Some(t) = &tree {
                joints::unload(&mut w, t);
            }
            for (k, v) in &w.counts {
                *totals.entry(k.clone()).or_default() += v;
            }
            for (k, v) in &w.hits {
                *hits.entry(k.clone()).or_default() += v;
            }
            halts += w.halts;
            stale += w.stale;
            hangs += w.hangs;
            release(w.st);
        }
        stages += 1;
    }
    set_fp_mode(FpMode::Hardware);
    let total: u64 = totals.values().sum();
    eprintln!(
        "{stages} stages; {total} comparisons, {halts} halting on both sides, {hangs} where the original hangs, {stale} reading an uninitialized surface:"
    );
    for (k, v) in &totals {
        eprintln!("  {k} {v} ({} nonzero)", hits.get(k).copied().unwrap_or(0));
    }
    for (k, v) in hits.iter().filter(|(k, _)| k.ends_with("(with calls)")) {
        eprintln!("  {k} {v}");
    }
    assert!(stages >= 25, "only {stages} stages have collision data");
}
