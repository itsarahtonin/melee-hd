//! Every mpcoll function on random collision data, both sides.

use leaf_check::{arg, bits, handle, sym};
use melee_sim::game::handles::CodeRef;
use melee_sim::game::structs::{CollData, Vec2, ftCollisionBox};
use melee_sim::hsd::gobj::GObjId;
use melee_sim::mp::coll::{Coll, CollStatics, Pass};
use melee_sim::mp::{CEILING, FLOOR, LEFT_WALL, RIGHT_WALL};
use melee_sim::vec::Vec3;
use mpstage::T;
use ssbm_rt::{Addr, Ctx, FnPtr};
use ssbm_types::fns;
use ssbm_types::records::{CollData as CColl, HSD_GObj, Vec as CVec, ftCollisionBox as CBox};

use super::joints::Tree;
use super::place::{self as place, Writer, line_of, off};
use super::{
    COLL, COLL2, Hooks, OUT_INT, OUT_VEC, World, both, import, mpstage, result_hung, take_theirs,
};

type C<'x, 'y> = Coll<'x, T, Hooks<'y>>;

fn cd<'a>(ctx: &'a Ctx) -> CColl<'a> {
    handle(ctx, COLL)
}

fn b(v: bool) -> u64 {
    u64::from(v)
}

fn w<'c>(w: &World<'_, 'c>) -> Writer<'c> {
    Writer {
        ctx: w.ctx(),
        base: COLL,
    }
}

/// Runs one function on both sides from the collision data in ssbm-rs's memory and compares
/// everything after. Returns the collision data (ours), or None if both halted.
pub fn case(
    wd: &mut World,
    name: &'static str,
    theirs: impl FnOnce(&Ctx) -> u64,
    ours: impl FnOnce(&mut C, &mut CollData) -> u64,
) -> Option<CollData> {
    let ctx = wd.ctx();
    mpstage::trace(name);
    wd.handles.what = format!("{}: {name}", wd.st.what());
    let mut ours_cd = import(ctx, COLL, &mut wd.handles);
    take_theirs();
    let what = format!("{}: {name}", wd.st.what());
    let stale = wd.statics.stale_reads;
    let result = both(
        ctx,
        &what,
        || theirs(ctx),
        || wd.with_coll(|c| ours(c, &mut ours_cd)),
    );
    match result {
        Some((rt, (ro, calls))) => {
            assert_eq!(ro, rt, "{what}: result");
            if wd.statics.stale_reads != stale {
                // A floor or ceiling that didn't reach its own end: the original took the
                // flags and normal on its stack, which ours counted as a stale read. Any
                // value matches them.
                wd.stale += 1;
                let theirs = import(ctx, COLL, &mut wd.handles);
                for (o, t) in [
                    (&mut ours_cd.floor, &theirs.floor),
                    (&mut ours_cd.ceiling, &theirs.ceiling),
                ] {
                    if o.index == -1 {
                        o.flags = t.flags;
                        o.normal = t.normal;
                    }
                }
            }
            wd.check(&ours_cd, COLL, &calls, name);
            wd.count(name);
            if rt != 0 {
                *wd.hits.entry(name.to_owned()).or_default() += 1;
            }
            if !calls.is_empty() {
                *wd.hits.entry(format!("{name} (with calls)")).or_default() += 1;
            }
            Some(ours_cd)
        }
        None if result_hung() => {
            // The original would hang: it isn't run, and ours starts the next call from
            // where both were.
            wd.hangs += 1;
            fns::mpUncheckBounding(ctx);
            wd.st.g.uncheck_bounding();
            wd.statics = CollStatics {
                stale_reads: wd.statics.stale_reads,
                watchdog: wd.statics.watchdog,
                ..super::statics_theirs(ctx)
            };
            take_theirs();
            None
        }
        None => {
            // Both halted partway: start the next call from the same place.
            wd.halts += 1;
            fns::mpUncheckBounding(ctx);
            wd.st.g.uncheck_bounding();
            wd.statics = CollStatics {
                stale_reads: wd.statics.stale_reads,
                watchdog: wd.statics.watchdog,
                ..super::statics_theirs(ctx)
            };
            take_theirs();
            None
        }
    }
}

/// Sets both sides' statics field by field.
fn set_tiny(wd: &mut World, tiny: bool) {
    wd.ctx().write_u32(sym("mpColl_IsEcbTiny"), u32::from(tiny));
    wd.statics.ecb_tiny = tiny;
}

fn set_filter(wd: &mut World, on: bool, gobj: u32) {
    let ctx = wd.ctx();
    ctx.write_u32(
        sym("mpColl_804D64A0"),
        if on { super::FLOOR_FILTER } else { 0 },
    );
    ctx.write_u32(sym("mpColl_804D64A4"), gobj);
    wd.statics.floor_filter = on.then_some(CodeRef(super::FLOOR_FILTER));
    wd.statics.floor_filter_gobj = (gobj != 0).then_some(GObjId(gobj));
}

/// Gives the joints random callbacks on both sides, each told apart by its user data.
pub fn set_callbacks(wd: &mut World) {
    let ctx = wd.ctx();
    for j in 0..wd.st.g.joints.len() as i32 {
        for second in [false, true] {
            let cb = if wd.st.rng.chance(3) {
                0
            } else if second {
                super::CEILING_CB
            } else {
                super::LEDGE_CB
            };
            let user = if wd.st.rng.chance(5) {
                0
            } else {
                0x8171_0000 + 8 * j as u32 + if second { 4 } else { 0 }
            };
            let (u, c) = (handle::<Addr>(ctx, user), handle::<FnPtr>(ctx, cb));
            let (ou, oc) = ((user != 0).then_some(user), (cb != 0).then_some(cb));
            if second {
                fns::mpJointSetCb2(ctx, j, u, c);
                wd.st.g.set_callback(j, ou, oc);
            } else {
                fns::mpJointSetCb1(ctx, j, u, c);
                wd.st.g.set_ledge_callback(j, ou, oc);
            }
        }
    }
}

/// Makes sure the collision data has a surface of `kind` at `at` (the game only calls some
/// functions with one).
fn need(wd: &mut World, at: u32, kind: u32) -> bool {
    let wr = w(wd);
    if wr.r32(at) as i32 != -1 {
        return true;
    }
    let Some(l) = line_of(wd, kind) else {
        return false;
    };
    let wr = w(wd);
    wr.i32(at, l);
    wr.u32(at + 4, u32::from(wd.st.g.lines[l as usize].lo_flags));
    true
}

fn rand_bool(wd: &mut World) -> bool {
    wd.st.rng.chance(2)
}

fn rand_f(wd: &mut World, lo: f32, hi: f32) -> f32 {
    wd.st.rng.range(lo, hi)
}

fn box_at(wd: &mut World, at: u32) -> ftCollisionBox {
    let e = place::ecb(wd);
    let ctx = wd.ctx();
    // ftCollisionBox: top, bottom, then left and right points.
    let vals = [e[1], e[3], e[6], e[7], e[4], e[5]];
    leaf_check::write_f32s(ctx, at, &vals);
    ftCollisionBox {
        top: vals[0],
        bottom: vals[1],
        left: Vec2 {
            x: vals[2],
            y: vals[3],
        },
        right: Vec2 {
            x: vals[4],
            y: vals[5],
        },
    }
}

fn gobj_of(addr: u32) -> Option<GObjId> {
    (addr != 0).then_some(GObjId(addr))
}

/// An output int slot, refilled so an untouched one shows.
fn out_int(ctx: &Ctx) -> ssbm_rt::Val<'_, i32> {
    ctx.write_u32(OUT_INT, 0xDEAD_BEEF);
    handle(ctx, OUT_INT)
}

fn out_vec(ctx: &Ctx) -> CVec<'_> {
    for i in 0..3 {
        ctx.write_u32(OUT_VEC + 4 * i, 0xDEAD_BEEF);
    }
    handle(ctx, OUT_VEC)
}

fn written_int(ctx: &Ctx, ours: Option<i32>) {
    let theirs = ctx.read_u32(OUT_INT);
    let ours = ours.map_or(0xDEAD_BEEF, |l| l as u32);
    assert_eq!(ours, theirs, "output line");
}

fn written_vec(ctx: &Ctx, ours: Option<Vec3>) {
    let theirs: Vec<u32> = (0..3).map(|i| ctx.read_u32(OUT_VEC + 4 * i)).collect();
    let ours = ours.map_or(vec![0xDEAD_BEEF; 3], |v| {
        vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
    });
    assert_eq!(ours, theirs, "output vector");
}

type OutFn = for<'a> fn(&'a Ctx, CColl<'a>, ssbm_rt::Val<'a, i32>) -> i32;
type SlideFn = for<'a> fn(&'a Ctx, CColl<'a>, i32, i32);
type Edge = for<'a> fn(&'a Ctx, CColl<'a>, i32) -> i32;
type EdgeCase = (
    &'static str,
    Edge,
    fn(&mut C, &mut CollData, i32) -> bool,
    u32,
);
type Wrapper = for<'a> fn(&'a Ctx, CColl<'a>) -> i32;
type FilteredWrapper = for<'a> fn(&'a Ctx, CColl<'a>, FnPtr<'a>, HSD_GObj<'a>) -> i32;
type BoxWrapper = for<'a> fn(&'a Ctx, CColl<'a>, CBox<'a>) -> i32;

/// The frames: (original, theirs, ECB flags (None: 0x12 unlocked), pass, pass flags).
const FRAMES: [(&str, Wrapper, Option<u32>, Pass, u32); 17] = [
    (
        "mpColl_800471F8",
        fns::mpColl_800471F8,
        Some(6),
        Pass::Air,
        0,
    ),
    (
        "mpColl_800473CC",
        fns::mpColl_800473CC,
        Some(6),
        Pass::Air,
        4,
    ),
    (
        "mpColl_800474E0",
        fns::mpColl_800474E0,
        Some(5),
        Pass::Air,
        4,
    ),
    (
        "mpColl_800477E0",
        fns::mpColl_800477E0,
        Some(6),
        Pass::Air,
        1,
    ),
    (
        "mpColl_800478F4",
        fns::mpColl_800478F4,
        Some(5),
        Pass::Air,
        1,
    ),
    (
        "mpColl_80048160",
        fns::mpColl_80048160,
        Some(10),
        Pass::Air,
        0,
    ),
    (
        "mpColl_80048274",
        fns::mpColl_80048274,
        Some(10),
        Pass::Air,
        1,
    ),
    (
        "mpColl_80048464",
        fns::mpColl_80048464,
        Some(10),
        Pass::Air,
        4,
    ),
    (
        "mpColl_80048654",
        fns::mpColl_80048654,
        Some(5),
        Pass::Air,
        0,
    ),
    ("mpColl_80048388", fns::mpColl_80048388, None, Pass::Air, 1),
    ("mpColl_80048578", fns::mpColl_80048578, None, Pass::Air, 4),
    ("mpColl_80048768", fns::mpColl_80048768, None, Pass::Air, 0),
    (
        "mpColl_8004B108",
        fns::mpColl_8004B108,
        Some(5),
        Pass::Ground,
        0,
    ),
    (
        "mpColl_8004B2DC",
        fns::mpColl_8004B2DC,
        Some(5),
        Pass::Ground,
        2,
    ),
    (
        "mpColl_8004B4B0",
        fns::mpColl_8004B4B0,
        Some(5),
        Pass::Ground,
        1,
    ),
    (
        "mpColl_8004B5C4",
        fns::mpColl_8004B5C4,
        Some(9),
        Pass::Ground,
        1,
    ),
    (
        "mpColl_8004C750",
        fns::mpColl_8004C750,
        Some(5),
        Pass::Ceiling,
        2,
    ),
];

const FILTERED: [(&str, FilteredWrapper, Option<u32>, u32); 7] = [
    ("mpColl_800476B4", fns::mpColl_800476B4, Some(6), 3),
    ("mpColl_80047AC8", fns::mpColl_80047AC8, Some(6), 2),
    ("mpColl_80047BF4", fns::mpColl_80047BF4, Some(10), 2),
    ("mpColl_80047E14", fns::mpColl_80047E14, Some(6), 6),
    ("mpColl_80047F40", fns::mpColl_80047F40, Some(10), 6),
    ("mpColl_80047D20", fns::mpColl_80047D20, None, 2),
    ("mpColl_8004806C", fns::mpColl_8004806C, None, 6),
];

const BOXED: [(&str, BoxWrapper, Pass, u32); 5] = [
    ("mpColl_8004730C", fns::mpColl_8004730C, Pass::Air, 0),
    ("mpColl_800475F4", fns::mpColl_800475F4, Pass::Air, 4),
    ("mpColl_80047A08", fns::mpColl_80047A08, Pass::Air, 1),
    ("mpColl_8004B21C", fns::mpColl_8004B21C, Pass::Ground, 0),
    ("mpColl_8004B3F0", fns::mpColl_8004B3F0, Pass::Ground, 2),
];

/// Runs frame `i` of [`FRAMES`] on both sides.
pub fn frame(wd: &mut World, i: usize) -> Option<CollData> {
    let (name, theirs, ecb, pass, flags) = FRAMES[i];
    case(
        wd,
        name,
        |ctx| theirs(ctx, cd(ctx)) as u64,
        |c, cd| {
            b(match (pass, ecb) {
                (Pass::Air, Some(e)) => c.air_frame(cd, e, flags),
                (Pass::Air, None) => c.air_frame_unlocked(cd, flags),
                (Pass::Ground, Some(e)) => c.ground_frame(cd, e, flags),
                (Pass::Ceiling, _) => c.ceiling_frame(cd),
                _ => unreachable!(),
            })
        },
    )
}

/// Runs filtered frame `i` on both sides, with or without a filter.
pub fn filtered_frame(wd: &mut World, i: usize) -> Option<CollData> {
    let (name, theirs, ecb, flags) = FILTERED[i];
    let filter = wd.st.rng.chance(4).then_some(super::FLOOR_FILTER);
    let gobj = if rand_bool(wd) { 0x8170_0040 } else { 0 };
    case(
        wd,
        name,
        |ctx| {
            theirs(
                ctx,
                cd(ctx),
                handle(ctx, filter.unwrap_or(0)),
                handle(ctx, gobj),
            ) as u64
        },
        |c, cd| b(c.air_frame_filtered(cd, ecb, flags, filter.map(CodeRef), gobj_of(gobj))),
    )
}

/// One round: the joints' callbacks, then every function on fresh collision data, then a few
/// frames run in sequence.
pub fn round(wd: &mut World, tree: Option<&Tree>) {
    hide_loops(wd);
    set_callbacks(wd);
    for _ in 0..3 {
        singles(wd);
    }
    for _ in 0..4 {
        sequence(wd);
    }
    if let Some(t) = tree {
        super::joints::round(wd, t);
    }
    // The mutations between rounds expect only their own callbacks.
    let ctx = wd.ctx();
    for j in 0..wd.st.g.joints.len() as i32 {
        let none = || (handle::<Addr>(ctx, 0), handle::<FnPtr>(ctx, 0));
        let (u, c) = none();
        fns::mpJointSetCb1(ctx, j, u, c);
        let (u, c) = none();
        fns::mpJointSetCb2(ctx, j, u, c);
        wd.st.g.set_ledge_callback(j, None, None);
        wd.st.g.set_callback(j, None, None);
    }
}

/// Hides the joints with a searchable line on a closed run of its kind (Poké Floats' parts
/// out of play, shown or left looping by the mutations): the original's walks never leave such
/// a run, so the game keeps those parts hidden.
fn hide_loops(wd: &mut World) {
    let ctx = wd.ctx();
    let g = &wd.st.g;
    let mut joints: Vec<i32> = (0..g.lines.len() as i32)
        .filter(|&l| {
            let f = g.lines[l as usize].flags;
            f & melee_sim::mp::LINE_ENABLED != 0
                && f & melee_sim::mp::LINE_HIDDEN == 0
                && super::queries::walk_loops(g, l)
        })
        .map(|l| g.joint_from_line(l))
        .filter(|&j| j != -1)
        .collect();
    joints.dedup();
    for j in joints {
        fns::mpJointHide(ctx, j);
        wd.st.g.hide_joint(j);
    }
}

/// Frames in a row, each moving the object on from where the last left it.
fn sequence(wd: &mut World) {
    place::place(wd, COLL);
    let kind = wd.st.rng.below(3);
    for _ in 0..8 {
        let r = match kind {
            0 => {
                let i = wd.st.rng.below(FRAMES.len() as u64) as usize;
                frame(wd, i)
            }
            1 => {
                let i = wd.st.rng.below(FILTERED.len() as u64) as usize;
                filtered_frame(wd, i)
            }
            _ => {
                let i = wd.st.rng.below(BOXED.len() as u64) as usize;
                boxed_frame(wd, i)
            }
        };
        if r.is_none() {
            return;
        }
        // On from here: last frame's position, a new move.
        let wr = w(wd);
        let cur = [wr.rf(off::CUR), wr.rf(off::CUR + 4), wr.rf(off::CUR + 8)];
        wr.fs(off::LAST, &cur);
        let speed = if wd.st.rng.chance(3) { 1.0 } else { 6.0 };
        let (vx, vy) = (place::wobble(wd, speed), place::wobble(wd, speed) - 1.0);
        let wr = w(wd);
        wr.fs(off::CUR, &[cur[0] + vx, cur[1] + vy, cur[2]]);
    }
}

fn boxed_frame(wd: &mut World, i: usize) -> Option<CollData> {
    let (name, theirs, pass, flags) = BOXED[i];
    let bx = box_at(wd, OUT_VEC);
    case(
        wd,
        name,
        |ctx| theirs(ctx, cd(ctx), handle(ctx, OUT_VEC)) as u64,
        |c, cd| {
            b(match pass {
                Pass::Air => c.air_frame_box(cd, &bx, flags),
                _ => c.ground_frame_box(cd, &bx, flags),
            })
        },
    )
}

/// Every function once, each on fresh collision data.
fn singles(wd: &mut World) {
    let fresh = |wd: &mut World| place::place(wd, COLL);

    fresh(wd);
    case(
        wd,
        "mpColl_80041EE4",
        |ctx| {
            fns::mpColl_80041EE4(ctx, cd(ctx));
            0
        },
        |c, cd| {
            c.init(cd);
            0
        },
    );

    fresh(wd);
    let vals = [
        rand_f(wd, 0.0, 20.0),
        rand_f(wd, -2.0, 10.0),
        rand_f(wd, 0.0, 12.0),
        rand_f(wd, 0.0, 12.0),
    ];
    let gobj = if rand_bool(wd) { 0x8170_0020 } else { 0 };
    case(
        wd,
        "mpColl_SetECBSource_Fixed",
        |ctx| {
            fns::mpColl_SetECBSource_Fixed(
                ctx,
                cd(ctx),
                handle(ctx, gobj),
                arg(vals[0]),
                arg(vals[1]),
                arg(vals[2]),
                arg(vals[3]),
            );
            0
        },
        |_, cd| {
            cd.set_ecb_source_box(gobj_of(gobj), vals[0], vals[1], vals[2], vals[3]);
            0
        },
    );

    fresh(wd);
    let snap = [
        rand_f(wd, 0.0, 16.0),
        rand_f(wd, -5.0, 20.0),
        rand_f(wd, 0.0, 25.0),
    ];
    case(
        wd,
        "mpColl_SetLedgeSnap",
        |ctx| {
            fns::mpColl_SetLedgeSnap(ctx, cd(ctx), arg(snap[0]), arg(snap[1]), arg(snap[2]));
            0
        },
        |_, cd| {
            cd.set_ledge_snap(snap[0], snap[1], snap[2]);
            0
        },
    );

    // A desired ECB as the loads might leave it, or worse.
    fresh(wd);
    let mut d = place::ecb(wd);
    for v in &mut d {
        if wd.st.rng.chance(3) {
            *v = place::wobble(wd, 12.0);
        }
    }
    w(wd).fs(off::DESIRED, &d);
    case(
        wd,
        "mpColl_80042384",
        |ctx| {
            fns::mpColl_80042384(ctx, cd(ctx));
            0
        },
        |_, cd| {
            cd.fix_desired_ecb();
            0
        },
    );

    fresh(wd);
    if w(wd).r32(off::SOURCE) != 1 {
        case(
            wd,
            "mpColl_LoadECB_Fixed",
            |ctx| {
                fns::mpColl_LoadECB_Fixed(ctx, cd(ctx));
                0
            },
            |_, cd| {
                cd.load_ecb_box();
                0
            },
        );
        fresh(wd);
        case(
            wd,
            "mpColl_LoadECB",
            |ctx| {
                fns::mpColl_LoadECB(ctx, cd(ctx));
                0
            },
            |c, cd| {
                c.load_ecb(cd, 6);
                0
            },
        );
    }

    fresh(wd);
    let bx = box_at(wd, OUT_VEC);
    case(
        wd,
        "mpColl_80042C58",
        |ctx| {
            fns::mpColl_80042C58(ctx, cd(ctx), handle(ctx, OUT_VEC));
            0
        },
        |_, cd| {
            cd.load_ecb_from(&bx);
            0
        },
    );

    fresh(wd);
    let t = match wd.st.rng.below(4) {
        0 => 1.0,
        1 => 0.5,
        2 => rand_f(wd, 0.0, 1.0),
        _ => rand_f(wd, 0.0, 1.5),
    };
    // Now and then a NaN in the desired ECB, where the original asserts.
    if wd.st.rng.chance(16) {
        w(wd).f(off::DESIRED + 4 * wd.st.rng.below(8) as u32, f32::NAN);
    }
    case(
        wd,
        "mpCollInterpolateECB",
        |ctx| {
            fns::mpCollInterpolateECB(ctx, cd(ctx), arg(t));
            0
        },
        |_, cd| {
            cd.interpolate_ecb(t);
            0
        },
    );

    fresh(wd);
    let line = if wd.st.rng.chance(5) {
        -1
    } else {
        line_of(wd, 0).unwrap()
    };
    let (landed, dy) = (rand_bool(wd), place::wobble(wd, 10.0));
    case(
        wd,
        "mpColl_80043268",
        |ctx| {
            fns::mpColl_80043268(ctx, cd(ctx), line, i32::from(landed), arg(dy));
            0
        },
        |c, cd| {
            c.floor_callback(cd, line, landed, dy);
            0
        },
    );

    fresh(wd);
    let (a1, a2) = (rand_bool(wd), rand_bool(wd));
    case(
        wd,
        "mpCollEnd",
        |ctx| {
            fns::mpCollEnd(ctx, cd(ctx), i32::from(a1), i32::from(a2));
            0
        },
        |c, cd| {
            c.end(cd, a1, a2);
            0
        },
    );

    fresh(wd);
    let line = line_of(wd, 0).unwrap();
    case(
        wd,
        "mpColl_80043558",
        |ctx| {
            fns::mpColl_80043558(ctx, cd(ctx), line);
            0
        },
        |c, cd| {
            c.line_callback(cd, line);
            0
        },
    );

    fresh(wd);
    case(
        wd,
        "mpColl_80043670",
        |ctx| {
            fns::mpColl_80043670(ctx, cd(ctx));
            0
        },
        |_, cd| {
            cd.request_clear();
            0
        },
    );

    fresh(wd);
    let pos = Vec3::new(
        rand_f(wd, -100.0, 100.0),
        rand_f(wd, -100.0, 100.0),
        place::wobble(wd, 3.0),
    );
    case(
        wd,
        "mpColl_80043680",
        |ctx| {
            leaf_check::put_vec(ctx, OUT_VEC, pos);
            fns::mpColl_80043680(ctx, cd(ctx), handle(ctx, OUT_VEC));
            0
        },
        |_, cd| {
            cd.set_position(pos);
            0
        },
    );

    fresh(wd);
    let facing = [-1, 0, 1, 7][wd.st.rng.below(4) as usize];
    case(
        wd,
        "mpCollSetFacingDir",
        |ctx| {
            fns::mpCollSetFacingDir(ctx, cd(ctx), facing);
            0
        },
        |_, cd| {
            cd.set_facing_dir(facing);
            0
        },
    );

    // The original hangs rotating a joint source, so only boxes.
    fresh(wd);
    if w(wd).r32(off::SOURCE) == 2 {
        let angle = match wd.st.rng.below(3) {
            0 => rand_f(wd, -7.0, 7.0),
            1 => rand_f(wd, -20.0, 20.0),
            _ => std::f32::consts::TAU,
        };
        case(
            wd,
            "mpColl_800436E4",
            |ctx| {
                fns::mpColl_800436E4(ctx, cd(ctx), arg(angle));
                0
            },
            |_, cd| {
                cd.set_ecb_angle(angle);
                0
            },
        );
    }

    fresh(wd);
    case(
        wd,
        "mpCollPrev",
        |ctx| {
            fns::mpCollPrev(ctx, cd(ctx));
            0
        },
        |c, cd| {
            c.prev(cd);
            0
        },
    );
    case(
        wd,
        "mpColl_80041C78",
        |ctx| {
            fns::mpColl_80041C78(ctx);
            0
        },
        |c, _| {
            c.s.reset();
            0
        },
    );

    fresh(wd);
    let flags = wd.st.rng.below(8) as u32;
    case(
        wd,
        "mpCollCheckBounding",
        |ctx| {
            fns::mpCollCheckBounding(ctx, cd(ctx), flags);
            0
        },
        |c, cd| {
            c.check_bounding(cd, flags);
            0
        },
    );
    case(
        wd,
        "mpUncheckBounding",
        |ctx| {
            fns::mpUncheckBounding(ctx);
            0
        },
        |c, _| {
            c.g.uncheck_bounding();
            0
        },
    );

    // The substeps over each pass.
    fresh(wd);
    let pass = wd.st.rng.below(4);
    let (addr, p) = match pass {
        0 => (0x8004_6904, Pass::Air),
        1 => (0x8004_6F78, Pass::Any),
        2 => (0x8004_ACE4, Pass::Ground),
        _ => (0x8004_C534, Pass::Ceiling),
    };
    let flags = wd.st.rng.below(8) as u32;
    let tiny = rand_bool(wd);
    set_tiny(wd, tiny);
    case(
        wd,
        "mpColl_80043754",
        |ctx| fns::mpColl_80043754(ctx, handle(ctx, addr), cd(ctx), flags) as u64,
        |c, cd| b(c.step(cd, p, flags)),
    );

    walls(wd);
    floors(wd);
    passes(wd);
    misc(wd);
}

type Check = for<'a> fn(&'a Ctx, CColl<'a>) -> i32;
type OursCheck = fn(&mut C, &mut CollData) -> bool;

/// The wall checks, each followed by its correction on what it collected.
fn walls(wd: &mut World) {
    let checks: [(
        &'static str,
        Check,
        OursCheck,
        &'static str,
        Check,
        OursCheck,
    ); 6] = [
        (
            "mpColl_80044E10_RightWall",
            fns::mpColl_80044E10_RightWall,
            |c, cd| c.check_right_walls_air(cd),
            "mpColl_800454A4_RightWall",
            fns::mpColl_800454A4_RightWall,
            |c, cd| c.collide_right_walls_air(cd),
        ),
        (
            "mpColl_80045B74_LeftWall",
            fns::mpColl_80045B74_LeftWall,
            |c, cd| c.check_left_walls_air(cd),
            "mpColl_80046224_LeftWall",
            fns::mpColl_80046224_LeftWall,
            |c, cd| c.collide_left_walls_air(cd),
        ),
        (
            "mpColl_80048AB0_RightWall",
            fns::mpColl_80048AB0_RightWall,
            |c, cd| c.check_right_walls_ground(cd),
            "mpColl_800491C8_RightWall",
            fns::mpColl_800491C8_RightWall,
            |c, cd| c.collide_right_walls_ground(cd),
        ),
        (
            "mpColl_80049778_LeftWall",
            fns::mpColl_80049778_LeftWall,
            |c, cd| c.check_left_walls_ground(cd),
            "mpColl_80049EAC_LeftWall",
            fns::mpColl_80049EAC_LeftWall,
            |c, cd| c.collide_left_walls_ground(cd),
        ),
        (
            "mpColl_8004B894_RightWall",
            fns::mpColl_8004B894_RightWall,
            |c, cd| c.check_right_walls_ceiling(cd),
            "mpColl_800491C8_RightWall",
            fns::mpColl_800491C8_RightWall,
            |c, cd| c.collide_right_walls_ground(cd),
        ),
        (
            "mpColl_8004BDD4_LeftWall",
            fns::mpColl_8004BDD4_LeftWall,
            |c, cd| c.check_left_walls_ceiling(cd),
            "mpColl_80049EAC_LeftWall",
            fns::mpColl_80049EAC_LeftWall,
            |c, cd| c.collide_left_walls_ground(cd),
        ),
    ];
    for (name, theirs, ours, cname, ctheirs, cours) in checks {
        place::place(wd, COLL);
        if wd.st.rng.chance(2) {
            place::at_wall_end(wd, name.ends_with("RightWall"));
        }
        let tiny = wd.st.rng.chance(4);
        set_tiny(wd, tiny);
        let hit = case(
            wd,
            name,
            |ctx| theirs(ctx, cd(ctx)) as u64,
            |c, cd| b(ours(c, cd)),
        );
        if hit.is_some() {
            case(
                wd,
                cname,
                |ctx| ctheirs(ctx, cd(ctx)) as u64,
                |c, cd| b(cours(c, cd)),
            );
        }
    }
}

/// Makes sure a wall surface is set where `left_right` says the original reads it.
fn need_walls(wd: &mut World, left_right: i32) -> bool {
    (left_right & 1 == 0 || need(wd, off::LEFT_WALL, LEFT_WALL))
        && (left_right & 2 == 0 || need(wd, off::RIGHT_WALL, RIGHT_WALL))
}

/// The floor and ceiling checks and snaps, the slides along them and the ledges.
fn floors(wd: &mut World) {
    let ctx = wd.ctx();

    place::place(wd, COLL);
    if need(wd, off::CEILING, CEILING) {
        case(
            wd,
            "mpColl_800439FC",
            |ctx| {
                fns::mpColl_800439FC(ctx, cd(ctx));
                0
            },
            |c, cd| {
                c.ceiling_left_wall(cd);
                0
            },
        );
    }
    place::place(wd, COLL);
    if need(wd, off::CEILING, CEILING) {
        case(
            wd,
            "mpColl_80043ADC",
            |ctx| {
                fns::mpColl_80043ADC(ctx, cd(ctx));
                0
            },
            |c, cd| {
                c.ceiling_right_wall(cd);
                0
            },
        );
    }

    for right in [false, true] {
        place::place(wd, COLL);
        if !need(wd, off::FLOOR, FLOOR) {
            continue;
        }
        let (name, theirs): (&'static str, OutFn) = if right {
            ("mpColl_80043E90", fns::mpColl_80043E90)
        } else {
            ("mpColl_80043BBC", fns::mpColl_80043BBC)
        };
        let mut ours_out = None;
        case(
            wd,
            name,
            |ctx| theirs(ctx, cd(ctx), out_int(ctx)) as u64,
            |c, cd| {
                ours_out = if right {
                    c.floor_right_wall(cd)
                } else {
                    c.floor_left_wall(cd)
                };
                b(ours_out.is_some())
            },
        );
        written_int(ctx, ours_out);
        let kind = if right { RIGHT_WALL } else { LEFT_WALL };
        let Some(wall) = line_of(wd, kind) else {
            continue;
        };
        let ignore = rand_bool(wd);
        let (name, theirs): (&'static str, SlideFn) = if right {
            ("mpColl_80043F40", fns::mpColl_80043F40)
        } else {
            ("mpColl_80043C6C", fns::mpColl_80043C6C)
        };
        case(
            wd,
            name,
            |ctx| {
                theirs(ctx, cd(ctx), wall, i32::from(ignore));
                0
            },
            |c, cd| {
                if right {
                    c.floor_right_wall_slide(cd, wall, ignore);
                } else {
                    c.floor_left_wall_slide(cd, wall, ignore);
                }
                0
            },
        );
    }

    for (right_end, near) in [(false, false), (true, false), (false, true), (true, true)] {
        place::place(wd, COLL);
        if near {
            place::beside_ledge(wd, right_end);
        }
        // Falling past a ledge: last frame higher.
        let wr = w(wd);
        let y = wr.rf(off::CUR + 4);
        let dy = rand_f(wd, 0.0, 12.0);
        w(wd).f(off::PREV + 4, y + dy);
        let null = wd.st.rng.chance(8);
        let (name, theirs): (&'static str, OutFn) = if right_end {
            ("mpColl_800443C4", fns::mpColl_800443C4)
        } else {
            ("mpColl_80044164", fns::mpColl_80044164)
        };
        let mut ours_out = None;
        case(
            wd,
            name,
            |ctx| {
                let out = if null { handle(ctx, 0) } else { out_int(ctx) };
                theirs(ctx, cd(ctx), out) as u64
            },
            |c, cd| {
                ours_out = if right_end {
                    c.ledge_right_end(cd)
                } else {
                    c.ledge_left_end(cd)
                };
                b(ours_out.is_some())
            },
        );
        if !null {
            written_int(ctx, ours_out);
        }
    }

    place::place(wd, COLL);
    let left_right = wd.st.rng.below(4) as i32;
    let filtered = wd.st.rng.chance(3);
    let gobj = if rand_bool(wd) { 0x8170_0060 } else { 0 };
    if need_walls(wd, left_right) {
        case(
            wd,
            "mpColl_80044628_Floor",
            |ctx| {
                let cb = if filtered { super::FLOOR_FILTER } else { 0 };
                fns::mpColl_80044628_Floor(
                    ctx,
                    cd(ctx),
                    handle(ctx, cb),
                    handle(ctx, gobj),
                    left_right,
                ) as u64
            },
            |c, cd| {
                let filter = filtered.then(|| (CodeRef(super::FLOOR_FILTER), gobj_of(gobj)));
                b(c.check_floor(cd, filter, left_right))
            },
        );
    }

    place::place(wd, COLL);
    if need(wd, off::FLOOR, FLOOR) {
        let ignore = rand_bool(wd);
        case(
            wd,
            "mpColl_80044838_Floor",
            |ctx| fns::mpColl_80044838_Floor(ctx, cd(ctx), i32::from(ignore)) as u64,
            |c, cd| b(c.snap_to_floor(cd, ignore)),
        );
    }
    place::place(wd, COLL);
    if need(wd, off::FLOOR, FLOOR) {
        case(
            wd,
            "mpColl_80044948_Floor",
            |ctx| fns::mpColl_80044948_Floor(ctx, cd(ctx)) as u64,
            |c, cd| b(c.snap_to_floor_air(cd)),
        );
    }
    place::place(wd, COLL);
    let left_right = wd.st.rng.below(4) as i32;
    if need_walls(wd, left_right) {
        case(
            wd,
            "mpColl_80044AD8_Ceiling",
            |ctx| fns::mpColl_80044AD8_Ceiling(ctx, cd(ctx), left_right) as u64,
            |c, cd| b(c.check_ceiling(cd, left_right)),
        );
    }
    place::place(wd, COLL);
    if need(wd, off::CEILING, CEILING) {
        case(
            wd,
            "mpColl_80044C74_Ceiling",
            |ctx| fns::mpColl_80044C74_Ceiling(ctx, cd(ctx)) as u64,
            |c, cd| b(c.snap_to_ceiling(cd)),
        );
    }
    place::place(wd, COLL);
    if need(wd, off::CEILING, CEILING) {
        case(
            wd,
            "mpColl_8004AB80",
            |ctx| fns::mpColl_8004AB80(ctx, cd(ctx)) as u64,
            |c, cd| b(c.snap_to_ceiling_ground(cd)),
        );
    }

    place::place(wd, COLL);
    case(
        wd,
        "mpColl_800488F4",
        |ctx| fns::mpColl_800488F4(ctx, cd(ctx)) as u64,
        |c, cd| b(c.follow_floor(cd)),
    );
    place::place(wd, COLL);
    case(
        wd,
        "mpColl_8004B6D8",
        |ctx| fns::mpColl_8004B6D8(ctx, cd(ctx)) as u64,
        |c, cd| b(c.follow_ceiling(cd)),
    );

    // Edges: the floor stood on, or any line.
    let edges: [EdgeCase; 4] = [
        (
            "mpColl_8004A45C_Floor",
            fns::mpColl_8004A45C_Floor,
            |c, cd, l| c.stop_at_edge(cd, l),
            FLOOR,
        ),
        (
            "mpColl_8004A678_Floor",
            fns::mpColl_8004A678_Floor,
            |c, cd, l| c.teeter_at_edge(cd, l),
            FLOOR,
        ),
        (
            "mpColl_8004A908_Floor",
            fns::mpColl_8004A908_Floor,
            |c, cd, l| c.other_floor(cd, l),
            FLOOR,
        ),
        (
            "mpColl_8004C328_Ceiling",
            fns::mpColl_8004C328_Ceiling,
            |c, cd, l| c.stop_at_ceiling_edge(cd, l),
            CEILING,
        ),
    ];
    for (name, theirs, ours, kind) in edges {
        place::place(wd, COLL);
        let line = match wd.st.rng.below(5) {
            0 => -1,
            1 => line_of(wd, 0).unwrap(),
            _ => line_of(wd, kind).unwrap_or(-1),
        };
        if line == -1 && name != "mpColl_8004A908_Floor" && name != "mpColl_8004A45C_Floor" {
            continue;
        }
        case(
            wd,
            name,
            |ctx| theirs(ctx, cd(ctx), line) as u64,
            |c, cd| b(ours(c, cd, line)),
        );
    }
}

/// The passes, each one substep.
fn passes(wd: &mut World) {
    place::place(wd, COLL);
    let flags = wd.st.rng.below(8) as u32;
    let (on, gobj) = (rand_bool(wd), if rand_bool(wd) { 0x8170_0080 } else { 0 });
    set_filter(wd, on, gobj);
    let tiny = wd.st.rng.chance(4);
    set_tiny(wd, tiny);
    case(
        wd,
        "mpColl_80046904",
        |ctx| fns::mpColl_80046904(ctx, cd(ctx), flags) as u64,
        |c, cd| b(c.air(cd, flags)),
    );
    place::place(wd, COLL);
    case(
        wd,
        "mpColl_80046F78",
        |ctx| fns::mpColl_80046F78(ctx, cd(ctx), flags) as u64,
        |c, cd| b(c.any(cd, flags)),
    );
    place::place(wd, COLL);
    let flags = wd.st.rng.below(4) as u32;
    case(
        wd,
        "mpColl_8004ACE4",
        |ctx| fns::mpColl_8004ACE4(ctx, cd(ctx), flags as i32) as u64,
        |c, cd| b(c.ground(cd, flags)),
    );
    place::place(wd, COLL);
    case(
        wd,
        "mpColl_8004C534",
        |ctx| fns::mpColl_8004C534(ctx, cd(ctx), flags) as u64,
        |c, cd| b(c.ceiling(cd, flags)),
    );
    place::place(wd, COLL);
    case(
        wd,
        "mpColl_80048844",
        |ctx| fns::mpColl_80048844(ctx, cd(ctx)) as u64,
        |c, cd| b(c.any_frame(cd)),
    );
    for i in 0..FRAMES.len() {
        place::place(wd, COLL);
        frame(wd, i);
    }
    for i in 0..FILTERED.len() {
        place::place(wd, COLL);
        filtered_frame(wd, i);
    }
    for i in 0..BOXED.len() {
        place::place(wd, COLL);
        boxed_frame(wd, i);
    }
    // Somewhere an item can't stand.
    let line = line_of(wd, 0).unwrap();
    let p = {
        let g = &wd.st.g;
        let l = g.lines[line as usize];
        let (a, bb) = (g.vertices[l.v0 as usize].pos, g.vertices[l.v1 as usize].pos);
        let t = wd.st.rng.range(0.0, 1.0);
        Vec3::new(a.x + (bb.x - a.x) * t, a.y + (bb.y - a.y) * t, 0.0)
    };
    let pos = Vec3::new(
        p.x + place::wobble(wd, 8.0),
        p.y + place::wobble(wd, 8.0),
        0.0,
    );
    super::OURS_COLL.set(1);
    case(
        wd,
        "mpColl_8004D024",
        |ctx| {
            leaf_check::put_vec(ctx, OUT_VEC, pos);
            fns::mpColl_8004D024(ctx, handle(ctx, OUT_VEC)) as u64
        },
        |c, _| b(c.squeezed_at(pos)),
    );
    super::OURS_COLL.set(COLL);
}

/// Squeezes, speeds, friction, platforms, floor skips and copies.
fn misc(wd: &mut World) {
    let ctx = wd.ctx();
    for vertical in [false, true] {
        place::place(wd, COLL);
        let wr = w(wd);
        let (x, y) = (wr.rf(off::CUR), wr.rf(off::CUR + 4));
        let airborne = rand_bool(wd);
        let (a, bb) = if vertical {
            (y + rand_f(wd, -2.0, 12.0), y + rand_f(wd, -12.0, 2.0))
        } else {
            (x + rand_f(wd, -12.0, 2.0), x + rand_f(wd, -2.0, 12.0))
        };
        let name = if vertical {
            "mpCollSqueezeVertical"
        } else {
            "mpCollSqueezeHorizontal"
        };
        case(
            wd,
            name,
            |ctx| {
                if vertical {
                    fns::mpCollSqueezeVertical(ctx, cd(ctx), i32::from(airborne), arg(a), arg(bb));
                } else {
                    fns::mpCollSqueezeHorizontal(
                        ctx,
                        cd(ctx),
                        i32::from(airborne),
                        arg(a),
                        arg(bb),
                    );
                }
                0
            },
            |_, cd| {
                if vertical {
                    cd.squeeze_vertical(airborne, a, bb);
                } else {
                    cd.squeeze_horizontal(airborne, a, bb);
                }
                0
            },
        );
    }

    place::place(wd, COLL);
    let grkind = wd.st.grkind;
    case(
        wd,
        "mpColl_8004CA6C",
        |ctx| u64::from(bits(fns::mpColl_8004CA6C(ctx, cd(ctx)))),
        |_, cd| u64::from(cd.floor_friction(grkind).to_bits()),
    );

    type Speed = for<'a> fn(&'a Ctx, CColl<'a>, CVec<'a>) -> i32;
    let speeds: [(&'static str, Speed, u32); 4] = [
        ("mpCollGetSpeedFloor", fns::mpCollGetSpeedFloor, off::FLOOR),
        (
            "mpCollGetSpeedCeiling",
            fns::mpCollGetSpeedCeiling,
            off::CEILING,
        ),
        (
            "mpCollGetSpeedLeftWall",
            fns::mpCollGetSpeedLeftWall,
            off::LEFT_WALL,
        ),
        (
            "mpCollGetSpeedRightWall",
            fns::mpCollGetSpeedRightWall,
            off::RIGHT_WALL,
        ),
    ];
    for (name, theirs, at) in speeds {
        place::place(wd, COLL);
        let mut ours_out = None;
        case(
            wd,
            name,
            |ctx| theirs(ctx, cd(ctx), out_vec(ctx)) as u64,
            |c, cd| {
                let s = match at {
                    off::FLOOR => cd.floor.clone(),
                    off::CEILING => cd.ceiling.clone(),
                    off::LEFT_WALL => cd.left_facing_wall.clone(),
                    _ => cd.right_facing_wall.clone(),
                };
                ours_out = c.surface_speed(cd, &s);
                b(ours_out.is_some())
            },
        );
        written_vec(ctx, ours_out);
    }

    place::place(wd, COLL);
    if need(wd, off::FLOOR, FLOOR) {
        case(
            wd,
            "mpColl_IsOnPlatform",
            |ctx| b(fns::mpColl_IsOnPlatform(ctx, cd(ctx)) != 0),
            |c, cd| b(c.on_platform(cd)),
        );
    }
    place::place(wd, COLL);
    case(
        wd,
        "mpUpdateFloorSkip",
        |ctx| {
            fns::mpUpdateFloorSkip(ctx, cd(ctx));
            0
        },
        |_, cd| {
            cd.update_floor_skip();
            0
        },
    );
    case(
        wd,
        "mpClearFloorSkip",
        |ctx| {
            fns::mpClearFloorSkip(ctx, cd(ctx));
            0
        },
        |_, cd| {
            cd.clear_floor_skip();
            0
        },
    );

    // A copy into the second collision data, compared there.
    place::place(wd, COLL2);
    place::place(wd, COLL);
    let arg2 = wd.st.rng.below(3) as i32;
    let mut dst = import(ctx, COLL2, &mut wd.handles);
    let src = import(ctx, COLL, &mut wd.handles);
    fns::mpCopyCollData(ctx, handle(ctx, COLL), handle(ctx, COLL2), arg2);
    dst.copy_from(&src, arg2);
    wd.check(&dst, COLL2, &[], "mpCopyCollData");
    wd.count("mpCopyCollData");
}
