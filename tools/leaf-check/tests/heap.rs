//! The original's allocators (`melee_sim::hsd::heap`) against ssbm-rs's ports, from real
//! match states: a debug VS match's (vanilla) and a Slippi replay's, whose layouts differ.
//! Random allocations and frees run on both sides; every address handed out and the whole
//! heap afterwards (the OS heap's blocks, the memory pieces, every pool) are compared.

#[path = "support/calls.rs"]
mod calls;

use calls::{Args, Check, Out};
use leaf_check::{Rand, SCRATCH, handle};
use melee_capture::heap::{alloc_data, class_info};
use melee_sim::hsd::heap::{Class, Kind};
use ssbm_rt::{Ctx, Handle};
use ssbm_types::fns;

const RESULTS: u32 = SCRATCH + 0x8000;

/// The debug VS match's captures and a Slippi replay's.
const VANILLA: &[&str] = &["C:/Projects/ssbm-rs-publication/local/calls/match"];
const SLIPPI: &[&str] = &["C:/Projects/ssbm-rs-publication/local/calls/replay"];

/// One step of a random run.
#[derive(Clone, Copy, Debug)]
enum Op {
    MemAlloc(u32),
    MemFree(usize),
    PieceAlloc(u32),
    PieceFree(usize),
    ObjAlloc(Kind),
    ObjFree(usize),
    ClassAlloc(Class),
    ClassFree(usize),
    /// A live class object becomes one of the classes sharing its root (`hsdChangeClass`).
    ChangeClass(usize, usize),
}

/// The pools the runs allocate from. Not the particles', generators' and effects': their
/// comparisons check those pools' counts against melee-sim's own pools, which these
/// runs leave alone.
const KINDS: [Kind; 11] = [
    Kind::GObj,
    Kind::GObjProc,
    Kind::AObj,
    Kind::FObj,
    Kind::RObj,
    Kind::IdEntry,
    Kind::Mtx,
    Kind::Item,
    Kind::SList,
    Kind::Vec,
    Kind::Fighter,
];

fn ops(seed: u32, mix: u32) -> Vec<Op> {
    let mut r = Rand(u64::from(seed) | 1);
    let sizes = [
        0x14, 0x20, 0x38, 0x9C, 0x100, 0x3E0, 0x1000, 0x8000, 0x2_0000,
    ];
    let pieces = [0x14, 0x18, 0x20, 0x21, 0x88, 0xAC, 0xD4, 0x100, 0x3E0];
    (0..120)
        .map(|_| {
            let k = match mix {
                0 => r.below(2),
                1 => 2 + r.below(2),
                2 => 4 + r.below(2),
                3 => 6 + r.below(3),
                _ => r.below(9),
            };
            let pick = r.below(1 << 16) as usize;
            match k {
                0 => Op::MemAlloc(sizes[r.below(9) as usize]),
                1 => Op::MemFree(pick),
                2 => Op::PieceAlloc(pieces[r.below(9) as usize]),
                3 => Op::PieceFree(pick),
                4 => Op::ObjAlloc(KINDS[r.below(11) as usize]),
                5 => Op::ObjFree(pick),
                6 => Op::ClassAlloc(Class::ALL[r.below(9) as usize]),
                7 => Op::ClassFree(pick),
                _ => Op::ChangeClass(pick, r.below(4) as usize),
            }
        })
        .collect()
}

/// What each side keeps live: blocks, pieces with their sizes, objects with their pools,
/// class objects with their classes.
#[derive(Default)]
struct Live {
    mem: Vec<u32>,
    pieces: Vec<(u32, u32)>,
    objs: Vec<(u32, Kind)>,
    class_objs: Vec<(u32, Class)>,
}

fn ours(r: &mut melee_sim::game::state::Running, a: &Args) -> Vec<Out> {
    let h = &mut r.game.objs.heap;
    let mut live = Live::default();
    let mut got = Vec::new();
    for op in ops(a.r(20), a.r(21)) {
        match op {
            Op::MemAlloc(n) => {
                let p = h.mem_alloc(n);
                got.push(p);
                live.mem.push(p);
            }
            Op::MemFree(pick) if !live.mem.is_empty() => {
                let p = live.mem.remove(pick % live.mem.len());
                h.mem_free(p);
            }
            Op::PieceAlloc(n) => {
                let p = h.piece_alloc(n);
                got.push(p);
                live.pieces.push((p, n));
            }
            Op::PieceFree(pick) if !live.pieces.is_empty() => {
                let (p, n) = live.pieces.remove(pick % live.pieces.len());
                h.piece_free(p, n);
            }
            Op::ObjAlloc(k) => {
                let p = h.obj_alloc(k);
                got.push(p);
                live.objs.push((p, k));
            }
            Op::ObjFree(pick) if !live.objs.is_empty() => {
                let (p, k) = live.objs.remove(pick % live.objs.len());
                h.obj_free(k, p);
            }
            Op::ClassAlloc(c) => {
                let p = h.class_alloc(c);
                got.push(p);
                live.class_objs.push((p, c));
            }
            Op::ClassFree(pick) if !live.class_objs.is_empty() => {
                let (p, c) = live.class_objs.remove(pick % live.class_objs.len());
                h.class_free(c, p);
            }
            Op::ChangeClass(pick, sel) if !live.class_objs.is_empty() => {
                let i = pick % live.class_objs.len();
                let c = live.class_objs[i].1;
                let to = sharing_root(c, sel);
                h.change_class(c, to);
                live.class_objs[i].1 = to;
            }
            _ => {}
        }
    }
    vec![Out::Words(RESULTS, got)]
}

fn theirs(ctx: &Ctx, call: &ssbm_rt::capture::Call) -> (u32, u32) {
    let mut live = Live::default();
    let mut at = RESULTS;
    let mut put = |p: u32| {
        ctx.write_u32(at, p);
        at += 4;
    };
    let data = |k: Kind| handle(ctx, alloc_data(k));
    for op in ops(call.regs.gpr[20], call.regs.gpr[21]) {
        match op {
            Op::MemAlloc(n) => {
                let p = Handle::addr(fns::HSD_MemAlloc(ctx, n as i32));
                put(p);
                live.mem.push(p);
            }
            Op::MemFree(pick) if !live.mem.is_empty() => {
                let p = live.mem.remove(pick % live.mem.len());
                fns::HSD_Free(ctx, handle(ctx, p));
            }
            Op::PieceAlloc(n) => {
                let p = Handle::addr(fns::hsdAllocMemPiece(ctx, n as i32));
                put(p);
                live.pieces.push((p, n));
            }
            Op::PieceFree(pick) if !live.pieces.is_empty() => {
                let (p, n) = live.pieces.remove(pick % live.pieces.len());
                fns::hsdFreeMemPiece(ctx, handle(ctx, p), n as i32);
            }
            Op::ObjAlloc(k) => {
                let p = Handle::addr(fns::HSD_ObjAlloc(ctx, data(k)));
                put(p);
                live.objs.push((p, k));
            }
            Op::ObjFree(pick) if !live.objs.is_empty() => {
                let (p, k) = live.objs.remove(pick % live.objs.len());
                fns::HSD_ObjFree(ctx, data(k), handle(ctx, p));
            }
            Op::ClassAlloc(c) => {
                let p = Handle::addr(fns::_hsdClassAlloc(ctx, handle(ctx, class_info(c))));
                put(p);
                live.class_objs.push((p, c));
            }
            Op::ClassFree(pick) if !live.class_objs.is_empty() => {
                let (p, c) = live.class_objs.remove(pick % live.class_objs.len());
                // `_hsdClassDestroy` finds the class through the object's first word.
                ctx.write_u32(p, class_info(c));
                fns::_hsdClassDestroy(ctx, handle(ctx, p));
            }
            Op::ChangeClass(pick, sel) if !live.class_objs.is_empty() => {
                let i = pick % live.class_objs.len();
                let (p, c) = live.class_objs[i];
                let to = sharing_root(c, sel);
                ctx.write_u32(p, class_info(c));
                fns::hsdChangeClass(ctx, handle(ctx, p), handle(ctx, class_info(to)));
                live.class_objs[i].1 = to;
            }
            _ => {}
        }
    }
    (0, 0)
}

/// One of the classes sharing `c`'s root, by `sel`.
fn sharing_root(c: Class, sel: usize) -> Class {
    let family: Vec<Class> = Class::ALL
        .iter()
        .copied()
        .filter(|k| k.root() == c.root())
        .collect();
    family[sel % family.len()]
}

fn check(dirs: &'static [&'static str], base: &'static str, mix: u32) {
    let perturb = move |f: &mut calls::Fuzz| {
        f.regs.gpr[20] = f.below(u64::from(u32::MAX));
        f.regs.gpr[21] = if mix == 9 { f.below(5) } else { mix };
    };
    Check {
        perturb_all: true,
        variants: 6,
        heap: true,
        dirs,
        perturb: &perturb,
        ..Check::new(base, &ours)
    }
    .with_theirs(&theirs)
    .run_n(4);
}

#[test]
fn os_heap() {
    check(VANILLA, "fn_8016C7D0", 0);
    check(SLIPPI, "Fighter_procAnim", 0);
}

#[test]
fn mem_pieces() {
    check(VANILLA, "fn_8016C7D0", 1);
    check(SLIPPI, "Fighter_procAnim", 1);
}

#[test]
fn obj_pools() {
    check(VANILLA, "fn_8016C7D0", 2);
    check(SLIPPI, "Fighter_procAnim", 2);
}

#[test]
fn classes() {
    check(VANILLA, "fn_8016C7D0", 3);
    check(SLIPPI, "Fighter_procAnim", 3);
}

#[test]
fn mixed() {
    check(VANILLA, "fn_8016C7D0", 9);
    check(SLIPPI, "Fighter_procAnim", 9);
}
