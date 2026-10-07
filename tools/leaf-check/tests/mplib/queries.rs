//! Random queries against the geometry, on both sides.

use std::cell::RefCell;
use std::collections::BTreeMap;

use leaf_check::{SCRATCH, arg, bits, handle};
use melee_sim::mp::{
    self, CEILING, FLOOR, Hit, IslandId, LEFT_WALL, Probe, RIGHT_WALL, line_intersection,
    line_intersection_h, line_intersection_v,
};
use melee_sim::vec::Vec3;
use ssbm_rt::{Ctx, F32, Handle, Val};
use ssbm_types::fns;
use ssbm_types::records::{Vec as CVec, mp_UnkStruct0};
use ssbm_types::tu::melee__mp__mplib as local_fns;

use super::{Rng, Stage, state};

/// Output slots for arguments passed by pointer, refilled with a pattern before each call so
/// an output the original leaves alone shows.
pub const OUT: u32 = SCRATCH;
pub const LINE: u32 = SCRATCH + 0x10;
pub const FLAGS: u32 = SCRATCH + 0x14;
pub const NORMAL: u32 = SCRATCH + 0x20;
pub const F: u32 = SCRATCH + 0x30;
pub const POS: u32 = SCRATCH + 0x40;
pub const OUT2: u32 = SCRATCH + 0x50;
const UNTOUCHED: u32 = 0xDEAD_BEEF;

/// The fake address of the floor search's line filter (`mpCheckFloor`'s callback).
pub const FILTER: u32 = 0x817E_0400;

thread_local! {
    static FILTERED: RefCell<Vec<(u32, i32)>> = const { RefCell::new(Vec::new()) };
}

/// The filter's decision for a line: a fixed pseudo-random function of the line and the
/// "fighter" passed alongside.
pub fn decide(gobj: u32, line: i32) -> bool {
    !((gobj ^ (line as u32).wrapping_mul(0x9E37_79B9)) >> 7).is_multiple_of(3)
}

fn native_filter(ctx: &Ctx) {
    let (gobj, line) = (ctx.regs.r(3), ctx.regs.r(4) as i32);
    FILTERED.with(|f| f.borrow_mut().push((gobj, line)));
    ctx.regs.set_r(3, u32::from(decide(gobj, line)));
}

pub fn register_natives(ctx: &Ctx) {
    ctx.register(FILTER, native_filter);
    super::mutations::register_natives(ctx);
}

/// How many times each function was compared.
#[derive(Default)]
pub struct Counts(pub BTreeMap<&'static str, u64>);

impl Counts {
    pub fn add(&mut self, name: &'static str) {
        *self.0.entry(name).or_default() += 1;
    }
}

impl std::fmt::Display for Counts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total: u64 = self.0.values().sum();
        write!(f, "{total} comparisons:")?;
        for (n, c) in &self.0 {
            write!(f, " {n} {c}")?;
        }
        Ok(())
    }
}

pub fn clear(ctx: &Ctx) {
    for a in (SCRATCH..SCRATCH + 0x60).step_by(4) {
        ctx.write_u32(a, UNTOUCHED);
    }
}

pub fn vec_at(ctx: &Ctx, a: u32) -> [u32; 3] {
    [ctx.read_u32(a), ctx.read_u32(a + 4), ctx.read_u32(a + 8)]
}

pub fn vbits(v: Vec3) -> [u32; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

pub fn put_vec(ctx: &Ctx, a: u32, v: Vec3) {
    ctx.write_u32(a, v.x.to_bits());
    ctx.write_u32(a + 4, v.y.to_bits());
    ctx.write_u32(a + 8, v.z.to_bits());
}

pub fn cvec(ctx: &Ctx, a: u32) -> CVec<'_> {
    handle(ctx, a)
}

/// The stage's extent, from the vertices.
fn extent(s: &Stage) -> (f32, f32, f32, f32) {
    let mut e = (f32::MAX, f32::MAX, -f32::MAX, -f32::MAX);
    for v in &s.g.vertices {
        e.0 = e.0.min(v.pos.x);
        e.1 = e.1.min(v.pos.y);
        e.2 = e.2.max(v.pos.x);
        e.3 = e.3.max(v.pos.y);
    }
    e
}

/// A point around the geometry: anywhere in its extent, on or near a line, or at a vertex.
pub fn point(s: &mut Stage) -> (f32, f32) {
    let (l, b, r, t) = extent(s);
    let pad_x = (r - l) * 0.2 + 10.0;
    let pad_y = (t - b) * 0.2 + 10.0;
    let rng = &mut s.rng;
    match rng.below(5) {
        0 => (
            rng.range(l - pad_x, r + pad_x),
            rng.range(b - pad_y, t + pad_y),
        ),
        1 | 2 => {
            let line = s.g.lines[rng.below(s.g.lines.len() as u64) as usize];
            let (a, c) = (
                s.g.vertices[line.v0 as usize].pos,
                s.g.vertices[line.v1 as usize].pos,
            );
            let k = rng.range(-0.15, 1.15);
            let j = if rng.chance(3) { 0.0 } else { 4.0 };
            (
                a.x + (c.x - a.x) * k + rng.range(-j, j),
                a.y + (c.y - a.y) * k + rng.range(-j, j),
            )
        }
        3 => {
            let v = s.g.vertices[rng.below(s.g.vertices.len() as u64) as usize].pos;
            (v.x, v.y)
        }
        _ => {
            let v = s.g.vertices[rng.below(s.g.vertices.len() as u64) as usize].pos;
            (v.x + rng.range(-0.6, 0.6), v.y + rng.range(-0.6, 0.6))
        }
    }
}

/// A segment around the geometry: a short motion, a long one to another point, an
/// axis-aligned one, or none at all.
pub fn segment(s: &mut Stage) -> (f32, f32, f32, f32) {
    let (ax, ay) = point(s);
    let rng = &mut s.rng;
    match rng.below(6) {
        0 => {
            let (bx, by) = point(s);
            (ax, ay, bx, by)
        }
        1 => (ax, ay, ax, ay + rng.range(-40.0, 40.0)),
        2 => (ax, ay, ax + rng.range(-40.0, 40.0), ay),
        3 if rng.chance(4) => (ax, ay, ax, ay),
        _ => {
            let len = if rng.chance(2) {
                rng.range(0.0, 3.0)
            } else {
                rng.range(0.0, 30.0)
            };
            let dir = rng.range(0.0, std::f32::consts::TAU);
            (ax, ay, ax + len * dir.cos(), ay + len * dir.sin())
        }
    }
}

fn some_line(s: &mut Stage) -> i32 {
    s.rng.below(s.g.lines.len() as u64) as i32
}

fn line_of(s: &mut Stage, kind: u32) -> Option<i32> {
    let n = s.g.lines.len() as u64;
    for _ in 0..16 {
        let l = s.rng.below(n) as usize;
        if s.g.lines[l].flags & kind != 0 {
            return Some(l as i32);
        }
    }
    None
}

/// A joint id for the skip/only arguments: usually none, sometimes a real one.
fn joint_arg(s: &mut Stage) -> i32 {
    if s.rng.chance(4) {
        s.rng.below(s.g.joints.len() as u64) as i32
    } else {
        -1
    }
}

/// Floats that exercise the pure functions: the geometry's own, edges and special values.
fn edge_float(rng: &mut Rng, near: f32) -> f32 {
    match rng.below(12) {
        0 => 0.0,
        1 => -0.0,
        2 => f32::INFINITY,
        3 => f32::NEG_INFINITY,
        4 => f32::NAN,
        5 => f32::from_bits(rng.bits() as u32),
        6 => 1e30,
        7 => near + rng.range(-1e-4, 1e-4),
        _ => near + rng.range(-30.0, 30.0),
    }
}

/// Compares a search's outputs: ours a hit or none, theirs a return value and the slots.
pub fn check_hit(ctx: &Ctx, ret: i32, ours: Option<Hit>, what: &dyn Fn() -> String) {
    match ours {
        None => {
            assert_eq!(ret, 0, "{}: ours found nothing", what());
            for a in [
                OUT,
                OUT + 4,
                OUT + 8,
                LINE,
                FLAGS,
                NORMAL,
                NORMAL + 4,
                NORMAL + 8,
            ] {
                assert_eq!(
                    ctx.read_u32(a),
                    UNTOUCHED,
                    "{}: an output was written",
                    what()
                );
            }
        }
        Some(h) => {
            assert_ne!(ret, 0, "{}: ours found {h:?}", what());
            assert_eq!(
                (vbits(h.pos), h.line, h.flags, vbits(h.normal)),
                (
                    vec_at(ctx, OUT),
                    ctx.read_u32(LINE) as i32,
                    ctx.read_u32(FLAGS),
                    vec_at(ctx, NORMAL)
                ),
                "{}",
                what()
            );
        }
    }
}

fn check_probe(ctx: &Ctx, ret: i32, ours: Option<Probe>, what: &dyn Fn() -> String) {
    match ours {
        None => {
            assert_eq!(ret, -1, "{}: ours found nothing", what());
            for a in [F, FLAGS, NORMAL, NORMAL + 4, NORMAL + 8] {
                assert_eq!(
                    ctx.read_u32(a),
                    UNTOUCHED,
                    "{}: an output was written",
                    what()
                );
            }
        }
        Some(p) => assert_eq!(
            (p.line, p.offset.to_bits(), p.flags, vbits(p.normal)),
            (
                ret,
                ctx.read_u32(F),
                ctx.read_u32(FLAGS),
                vec_at(ctx, NORMAL)
            ),
            "{}",
            what()
        ),
    }
}

/// Our island for ssbm-rs's address.
pub fn island_for(s: &Stage, addr: u32) -> Option<IslandId> {
    (addr != 0).then(|| {
        IslandId(
            *s.islands
                .get(&addr)
                .unwrap_or_else(|| panic!("{}: unknown island {addr:#x}", s.what())),
        )
    })
}

/// The pure functions: intersections and the remap.
fn pure(s: &mut Stage, n: &mut Counts) {
    super::trace("pure");
    let ctx = s.ctx;
    let (x, y) = point(s);
    let rng = &mut s.rng;
    let v: Vec<f32> = (0..10)
        .map(|k| {
            let near = if k % 2 == 0 { x } else { y };
            if rng.chance(3) {
                edge_float(rng, near)
            } else {
                near + rng.range(-50.0, 50.0)
            }
        })
        .collect();
    let a = |k: usize| arg(v[k]);
    clear(ctx);
    let ret = local_fns::mpLineIntersection(
        ctx,
        a(0),
        a(1),
        a(2),
        a(3),
        a(4),
        a(5),
        a(6),
        a(7),
        handle::<Val<F32>>(ctx, F),
        handle(ctx, F + 4),
    );
    let ours = line_intersection(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
    let what = || format!("{}: mpLineIntersection{:?}", s.what(), &v[..8]);
    match ours {
        None => assert_eq!((ret, ctx.read_u32(F)), (0, UNTOUCHED), "{}", what()),
        Some((ix, iy)) => assert_eq!(
            (ret, ix.to_bits(), iy.to_bits()),
            (1, ctx.read_u32(F), ctx.read_u32(F + 4)),
            "{}",
            what()
        ),
    }
    n.add("mpLineIntersection");
    for (vert, name) in [
        (false, "mpLineIntersectionH"),
        (true, "mpLineIntersectionV"),
    ] {
        clear(ctx);
        let (ret, ours) = if vert {
            (
                fns::mpLineIntersectionV(
                    ctx,
                    handle(ctx, F),
                    handle(ctx, F + 4),
                    a(0),
                    a(1),
                    a(2),
                    a(3),
                    a(4),
                    a(5),
                    a(6),
                ),
                line_intersection_v(v[0], v[1], v[2], v[3], v[4], v[5], v[6]),
            )
        } else {
            (
                fns::mpLineIntersectionH(
                    ctx,
                    handle(ctx, F),
                    handle(ctx, F + 4),
                    a(0),
                    a(1),
                    a(2),
                    a(3),
                    a(4),
                    a(5),
                    a(6),
                ),
                line_intersection_h(v[0], v[1], v[2], v[3], v[4], v[5], v[6]),
            )
        };
        let what = || format!("{}: {name}{:?}", s.what(), &v[..7]);
        match ours {
            None => assert_eq!((ret, ctx.read_u32(F)), (0, UNTOUCHED), "{}", what()),
            Some((ix, iy)) => assert_eq!(
                (ret, ix.to_bits(), iy.to_bits()),
                (1, ctx.read_u32(F), ctx.read_u32(F + 4)),
                "{}",
                what()
            ),
        }
        n.add(name);
    }
    clear(ctx);
    local_fns::mpRemap2d(
        ctx,
        handle(ctx, F),
        handle(ctx, F + 4),
        a(0),
        a(1),
        a(2),
        a(3),
        a(4),
        a(5),
        a(6),
        a(7),
        a(8),
        a(9),
    );
    let ours = mp::remap(a(0), a(1), a(2), a(3), a(4), a(5), a(6), a(7), a(8), a(9));
    assert_eq!(
        [ours.0.to_bits(), ours.1.to_bits()],
        [ctx.read_u32(F), ctx.read_u32(F + 4)],
        "{}: mpRemap2d{v:?}",
        s.what()
    );
    n.add("mpRemap2d");
}

/// Queries about one line: neighbours, walks, ends and properties.
fn line_queries(s: &mut Stage, n: &mut Counts) {
    super::trace("line_queries");
    let ctx = s.ctx;
    let line = some_line(s);
    let other = some_line(s);
    let g = &s.g;
    let what = |f: &str| format!("{}: {f}({line})", s.what());
    assert_eq!(
        g.line_next(line),
        fns::mpLineGetNext(ctx, line),
        "{}",
        what("mpLineGetNext")
    );
    assert_eq!(
        g.line_prev(line),
        fns::mpLineGetPrev(ctx, line),
        "{}",
        what("mpLineGetPrev")
    );
    assert_eq!(
        g.joint_from_line(line),
        fns::mpJointFromLine(ctx, line),
        "{}",
        what("mpJointFromLine")
    );
    assert_eq!(
        g.line_kind(line) as i32,
        fns::mpLineGetKind(ctx, line),
        "{}",
        what("mpLineGetKind")
    );
    assert_eq!(
        g.line_surface(line),
        fns::mpLineGetFlags(ctx, line),
        "{}",
        what("mpLineGetFlags")
    );
    assert_eq!(
        i32::from(g.line_live(line)),
        fns::mpLib_80054ED8(ctx, line),
        "{}",
        what("mpLib_80054ED8")
    );
    clear(ctx);
    fns::mpLineGetNormal(ctx, line, cvec(ctx, NORMAL));
    assert_eq!(
        vbits(g.line_normal(line)),
        vec_at(ctx, NORMAL),
        "{}",
        what("mpLineGetNormal")
    );
    for (v1, name) in [(false, "mpLineGetV0Pos"), (true, "mpLineGetV1Pos")] {
        clear(ctx);
        let ours = if v1 {
            fns::mpLineGetV1Pos(ctx, line, cvec(ctx, OUT));
            g.line_v1(line)
        } else {
            fns::mpLineGetV0Pos(ctx, line, cvec(ctx, OUT));
            g.line_v0(line)
        };
        assert_eq!(vbits(ours), vec_at(ctx, OUT), "{}", what(name));
    }
    clear(ctx);
    fns::mpLib_8004ED5C(
        ctx,
        line,
        handle(ctx, F),
        handle(ctx, F + 4),
        handle(ctx, F + 8),
        handle(ctx, F + 12),
    );
    let e = g.extended_ends(line);
    assert_eq!(
        [e.0, e.1, e.2, e.3].map(f32::to_bits),
        [
            ctx.read_u32(F),
            ctx.read_u32(F + 4),
            ctx.read_u32(F + 8),
            ctx.read_u32(F + 12)
        ],
        "{}",
        what("mpLib_8004ED5C")
    );
    if !walk_loops(g, line) {
        assert_eq!(
            i32::from(g.lines_connected(line, other)),
            fns::mpLinesConnected(ctx, line, other),
            "{}: mpLinesConnected({line}, {other})",
            s.what()
        );
    }
    let island = g.island_of_line(line);
    let theirs = fns::mpIsland_8005AB54(ctx, line).addr();
    assert_eq!(
        island,
        island_for(s, theirs),
        "{}",
        what("mpIsland_8005AB54")
    );
    for name in [
        "mpLineGetNext",
        "mpLineGetPrev",
        "mpJointFromLine",
        "mpLineGetKind",
        "mpLineGetFlags",
        "mpLib_80054ED8",
        "mpLineGetNormal",
        "mpLineGetV0Pos",
        "mpLineGetV1Pos",
        "mpLib_8004ED5C",
        "mpLinesConnected",
        "mpIsland_8005AB54",
    ] {
        n.add(name);
    }
    kind_walks(s, n);
}

/// Whether walking from `line` along lines of its kind can go around forever: in some stages
/// a closed run of lines shares one kind (Pokémon Stadium's dynamic loops before their kinds
/// are first updated), and the original's walks never end on those, so the game never walks
/// one. Either direction, following links or the data's neighbours.
pub fn walk_loops(g: &mp::Geometry<super::T>, line: i32) -> bool {
    let kind = g.lines[line as usize].flags & 0xF;
    let step = |id: i32, k: u8| match k {
        0 => g.line_next(id),
        1 => g.line_prev(id),
        2 => i32::from(g.lines[id as usize].next),
        _ => i32::from(g.lines[id as usize].prev),
    };
    (0..4).any(|k| {
        let mut id = line;
        for _ in 0..=g.lines.len() {
            id = step(id, k);
            if id == -1 || g.lines[id as usize].flags & kind == 0 {
                return false;
            }
        }
        true
    })
}

/// The walks and ends that the game asks only of lines of their kind.
fn kind_walks(s: &mut Stage, n: &mut Counts) {
    type Ours = fn(&mp::Geometry<super::T>, i32) -> i32;
    type Theirs = for<'a> fn(&'a Ctx, i32) -> i32;
    type OursEnd = fn(&mp::Geometry<super::T>, i32) -> Vec3;
    type TheirsEnd = for<'a> fn(&'a Ctx, i32, CVec<'a>);
    let walks: [(u32, &str, Ours, Theirs); 16] = [
        (
            FLOOR,
            "mpLineNextNonFloor",
            |g, l| g.next_non_floor(l),
            fns::mpLineNextNonFloor,
        ),
        (
            FLOOR,
            "mpLinePrevNonFloor",
            |g, l| g.prev_non_floor(l),
            fns::mpLinePrevNonFloor,
        ),
        (
            CEILING,
            "mpLinePrevNonCeiling",
            |g, l| g.prev_non_ceiling(l),
            fns::mpLinePrevNonCeiling,
        ),
        (
            CEILING,
            "mpLineNextNonCeiling",
            |g, l| g.next_non_ceiling(l),
            fns::mpLineNextNonCeiling,
        ),
        (
            LEFT_WALL,
            "mpLineNextNonLeftWall",
            |g, l| g.next_non_left_wall(l),
            fns::mpLineNextNonLeftWall,
        ),
        (
            LEFT_WALL,
            "mpLinePrevNonLeftWall",
            |g, l| g.prev_non_left_wall(l),
            fns::mpLinePrevNonLeftWall,
        ),
        (
            RIGHT_WALL,
            "mpLinePrevNonRightWall",
            |g, l| g.prev_non_right_wall(l),
            fns::mpLinePrevNonRightWall,
        ),
        (
            RIGHT_WALL,
            "mpLineNextNonRightWall",
            |g, l| g.next_non_right_wall(l),
            fns::mpLineNextNonRightWall,
        ),
        (
            FLOOR,
            "mpLib_80053394_Floor",
            |g, l| g.data_next_non_floor(l),
            fns::mpLib_80053394_Floor,
        ),
        (
            FLOOR,
            "mpLib_80053448_Floor",
            |g, l| g.data_prev_non_floor(l),
            fns::mpLib_80053448_Floor,
        ),
        (
            FLOOR,
            "mpLib_800534FC_Floor",
            |g, l| g.next_floor_link(l),
            fns::mpLib_800534FC_Floor,
        ),
        (
            FLOOR,
            "mpLib_800536CC_Floor",
            |g, l| g.prev_floor_link(l),
            fns::mpLib_800536CC_Floor,
        ),
        (
            CEILING,
            "mpLib_8005389C_Ceiling",
            |g, l| g.data_prev_non_ceiling(l),
            fns::mpLib_8005389C_Ceiling,
        ),
        (
            CEILING,
            "mpLib_80053950_Ceiling",
            |g, l| g.data_next_non_ceiling(l),
            fns::mpLib_80053950_Ceiling,
        ),
        (
            CEILING,
            "mpLib_80053A04_Ceiling",
            |g, l| g.prev_ceiling_link(l),
            fns::mpLib_80053A04_Ceiling,
        ),
        (
            CEILING,
            "mpLib_80053BD4_Ceiling",
            |g, l| g.next_ceiling_link(l),
            fns::mpLib_80053BD4_Ceiling,
        ),
    ];
    let ctx = s.ctx;
    for (kind, name, ours, theirs) in walks {
        let Some(line) = line_of(s, kind) else {
            continue;
        };
        if walk_loops(&s.g, line) {
            continue;
        }
        super::trace(name);
        assert_eq!(
            ours(&s.g, line),
            theirs(ctx, line),
            "{}: {name}({line})",
            s.what()
        );
        n.add(name);
    }
    let ends: [(u32, &str, OursEnd, TheirsEnd); 10] = [
        (
            FLOOR,
            "mpLib_80053DA4_Floor",
            |g, l| g.data_floor_right(l),
            fns::mpLib_80053DA4_Floor,
        ),
        (
            FLOOR,
            "mpLib_80053ECC_Floor",
            |g, l| g.data_floor_left(l),
            fns::mpLib_80053ECC_Floor,
        ),
        (
            FLOOR,
            "mpFloorGetRight",
            |g, l| g.floor_right(l),
            fns::mpFloorGetRight,
        ),
        (
            FLOOR,
            "mpFloorGetLeft",
            |g, l| g.floor_left(l),
            fns::mpFloorGetLeft,
        ),
        (
            CEILING,
            "mpCeilingGetRight",
            |g, l| g.ceiling_right(l),
            fns::mpCeilingGetRight,
        ),
        (
            CEILING,
            "mpCeilingGetLeft",
            |g, l| g.ceiling_left(l),
            fns::mpCeilingGetLeft,
        ),
        (
            LEFT_WALL,
            "mpLeftWallGetTop",
            |g, l| g.left_wall_top(l),
            fns::mpLeftWallGetTop,
        ),
        (
            LEFT_WALL,
            "mpLeftWallGetBottom",
            |g, l| g.left_wall_bottom(l),
            fns::mpLeftWallGetBottom,
        ),
        (
            RIGHT_WALL,
            "mpRightWallGetTop",
            |g, l| g.right_wall_top(l),
            fns::mpRightWallGetTop,
        ),
        (
            RIGHT_WALL,
            "mpRightWallGetBottom",
            |g, l| g.right_wall_bottom(l),
            fns::mpRightWallGetBottom,
        ),
    ];
    for (kind, name, ours, theirs) in ends {
        let Some(line) = line_of(s, kind) else {
            continue;
        };
        if walk_loops(&s.g, line) {
            continue;
        }
        clear(ctx);
        theirs(ctx, line, cvec(ctx, OUT));
        assert_eq!(
            vbits(ours(&s.g, line)),
            vec_at(ctx, OUT),
            "{}: {name}({line})",
            s.what()
        );
        n.add(name);
    }
}

/// The point queries against one surface.
fn probes(s: &mut Stage, n: &mut Counts) {
    let ctx = s.ctx;
    type Ours = fn(&mp::Geometry<super::T>, i32, Vec3) -> Option<Probe>;
    type Theirs = for<'a> fn(&'a Ctx, i32, CVec<'a>, Val<'a, F32>, Val<'a, u32>, CVec<'a>) -> i32;
    let probes: [(u32, &str, Ours, Theirs); 4] = [
        (
            FLOOR,
            "mpLib_8004DD90_Floor",
            |g, l, p| g.probe_floor(l, p),
            fns::mpLib_8004DD90_Floor,
        ),
        (
            CEILING,
            "mpLib_8004E090_Ceiling",
            |g, l, p| g.probe_ceiling(l, p),
            fns::mpLib_8004E090_Ceiling,
        ),
        (
            LEFT_WALL,
            "mpLib_8004E398_LeftWall",
            |g, l, p| g.probe_left_wall(l, p),
            fns::mpLib_8004E398_LeftWall,
        ),
        (
            RIGHT_WALL,
            "mpLib_8004E684_RightWall",
            |g, l, p| g.probe_right_wall(l, p),
            fns::mpLib_8004E684_RightWall,
        ),
    ];
    for (kind, name, ours, theirs) in probes {
        let Some(line) = line_of(s, kind) else {
            continue;
        };
        if walk_loops(&s.g, line) {
            continue;
        }
        super::trace(name);
        let (x, y) = point(s);
        let pos = Vec3::new(x, y, s.rng.range(-5.0, 5.0));
        clear(ctx);
        put_vec(ctx, POS, pos);
        let ret = theirs(
            ctx,
            line,
            cvec(ctx, POS),
            handle(ctx, F),
            handle(ctx, FLAGS),
            cvec(ctx, NORMAL),
        );
        check_probe(ctx, ret, ours(&s.g, line, pos), &|| {
            format!("{}: {name}({line}, {pos:?})", s.what())
        });
        n.add(name);
    }
    // How a line carries a point.
    let line = some_line(s);
    let (x, y) = point(s);
    let pos = Vec3::new(x, y, 1.0);
    clear(ctx);
    put_vec(ctx, POS, pos);
    let ret = fns::mpGetSpeed(ctx, line, cvec(ctx, POS), cvec(ctx, OUT));
    match s.g.line_speed(line, pos) {
        None => assert_eq!(
            (ret, ctx.read_u32(OUT)),
            (0, UNTOUCHED),
            "{}: mpGetSpeed({line})",
            s.what()
        ),
        Some(v) => assert_eq!(
            (ret, vbits(v)),
            (1, vec_at(ctx, OUT)),
            "{}: mpGetSpeed({line}, {pos:?})",
            s.what()
        ),
    }
    n.add("mpGetSpeed");
    super::trace("walk_floor");
    // A walk along the floor. The stale register only matters when the walk starts on a
    // wall, which happens only when a line changed kind under the walker.
    // A walk from a wall (a line that turned under the walker) starts from the stale value.
    let kind = if s.rng.chance(4) {
        LEFT_WALL | RIGHT_WALL
    } else {
        FLOOR
    };
    if let Some(line) = line_of(s, kind).filter(|&l| !walk_loops(&s.g, l)) {
        let (x, y) = point(s);
        let pos = Vec3::new(x, y, s.rng.range(-3.0, 3.0));
        let distance = if s.rng.chance(8) {
            0.0
        } else {
            s.rng.range(-80.0, 80.0)
        };
        let max_wall = s.rng.range(0.0, 30.0);
        let stale = arg(s.rng.range(-10.0, 10.0));
        clear(ctx);
        put_vec(ctx, POS, pos);
        ctx.regs.set_f(27, stale);
        let ret = fns::mpLib_80056C54(
            ctx,
            line,
            cvec(ctx, POS),
            handle(ctx, LINE),
            cvec(ctx, OUT),
            handle(ctx, FLAGS),
            cvec(ctx, NORMAL),
            arg(distance),
            arg(max_wall),
        );
        let ours = s.g.walk_floor(line, pos, distance, max_wall, stale);
        let what = format!(
            "{}: mpLib_80056C54({line}, {pos:?}, {distance}, {max_wall})",
            s.what()
        );
        match ours {
            None => {
                assert_eq!(ret, 0, "{what}");
                assert_eq!(ctx.read_u32(LINE), UNTOUCHED, "{what}");
            }
            Some(w) => {
                assert_eq!(
                    (ret != 0, ctx.read_u32(LINE) as i32, vec_at(ctx, OUT)),
                    (w.reached, w.line, vbits(w.pos)),
                    "{what}"
                );
                match w.surface {
                    Some((f, nrm)) => assert_eq!(
                        (f, vbits(nrm)),
                        (ctx.read_u32(FLAGS), vec_at(ctx, NORMAL)),
                        "{what}"
                    ),
                    None => assert_eq!(ctx.read_u32(FLAGS), UNTOUCHED, "{what}"),
                }
            }
        }
        n.add("mpLib_80056C54");
    }
}

/// The segment searches.
fn searches(s: &mut Stage, n: &mut Counts) {
    super::trace("searches");
    let ctx = s.ctx;
    let (ax, ay, bx, by) = segment(s);
    let (js, jo) = (joint_arg(s), joint_arg(s));
    let ls = if s.rng.chance(3) { some_line(s) } else { -1 };
    let yo = if s.rng.chance(3) {
        s.rng.range(-5.0, 5.0)
    } else {
        0.0
    };
    let gobj = s.rng.bits() as u32;
    let filtered = s.rng.chance(2);
    let out = |ctx| {
        (
            cvec(ctx, OUT),
            handle::<Val<i32>>(ctx, LINE),
            handle::<Val<u32>>(ctx, FLAGS),
            cvec(ctx, NORMAL),
        )
    };
    for remap in [false, true] {
        super::trace("mpCheckFloor");
        clear(ctx);
        FILTERED.with(|f| f.borrow_mut().clear());
        let (o, l, f, nm) = out(ctx);
        let cb = handle(ctx, if filtered { FILTER } else { 0 });
        let a = |x: f32| arg(x);
        let ret = if remap {
            fns::mpCheckFloorRemap(
                ctx,
                a(ax),
                a(ay),
                a(bx),
                a(by),
                a(yo),
                o,
                l,
                f,
                nm,
                ls,
                js,
                jo,
                cb,
                handle(ctx, gobj),
            )
        } else {
            fns::mpCheckFloor(
                ctx,
                a(ax),
                a(ay),
                a(bx),
                a(by),
                a(yo),
                o,
                l,
                f,
                nm,
                ls,
                js,
                jo,
                cb,
                handle(ctx, gobj),
            )
        };
        let mut calls = Vec::new();
        let mut filter = |line: i32| {
            calls.push((gobj, line));
            decide(gobj, line)
        };
        let filter: Option<&mut dyn FnMut(i32) -> bool> =
            if filtered { Some(&mut filter) } else { None };
        let ours = if remap {
            s.g.check_floor_remap(ax, ay, bx, by, yo, ls, js, jo, filter)
        } else {
            s.g.check_floor(ax, ay, bx, by, yo, ls, js, jo, filter)
        };
        let name = if remap {
            "mpCheckFloorRemap"
        } else {
            "mpCheckFloor"
        };
        let what = || {
            format!(
                "{}: {name}({ax}, {ay}, {bx}, {by}, {yo}, {ls}, {js}, {jo})",
                s.what()
            )
        };
        check_hit(ctx, ret, ours, &what);
        let theirs_calls = FILTERED.with(|f| std::mem::take(&mut *f.borrow_mut()));
        assert_eq!(calls, theirs_calls, "{}: filter calls", what());
        n.add(name);
    }
    type Ours = fn(&mut mp::Geometry<super::T>, f32, f32, f32, f32, i32, i32) -> Option<Hit>;
    type Theirs = for<'a> fn(
        &'a Ctx,
        f64,
        f64,
        f64,
        f64,
        CVec<'a>,
        Val<'a, i32>,
        Val<'a, u32>,
        CVec<'a>,
        i32,
        i32,
    ) -> i32;
    let checks: [(&str, Ours, Theirs); 6] = [
        (
            "mpCheckCeiling",
            |g, a, b, c, d, j, k| g.check_ceiling(a, b, c, d, j, k),
            fns::mpCheckCeiling,
        ),
        (
            "mpCheckCeilingRemap",
            |g, a, b, c, d, j, k| g.check_ceiling_remap(a, b, c, d, j, k),
            fns::mpCheckCeilingRemap,
        ),
        (
            "mpCheckLeftWall",
            |g, a, b, c, d, j, k| g.check_left_wall(a, b, c, d, j, k),
            fns::mpCheckLeftWall,
        ),
        (
            "mpCheckLeftWallRemap",
            |g, a, b, c, d, j, k| g.check_left_wall_remap(a, b, c, d, j, k),
            fns::mpCheckLeftWallRemap,
        ),
        (
            "mpCheckRightWall",
            |g, a, b, c, d, j, k| g.check_right_wall(a, b, c, d, j, k),
            fns::mpCheckRightWall,
        ),
        (
            "mpCheckRightWallRemap",
            |g, a, b, c, d, j, k| g.check_right_wall_remap(a, b, c, d, j, k),
            fns::mpCheckRightWallRemap,
        ),
    ];
    for (name, ours, theirs) in checks {
        super::trace(name);
        let (ax, ay, bx, by) = segment(s);
        clear(ctx);
        let (o, l, f, nm) = out(ctx);
        let ret = theirs(ctx, arg(ax), arg(ay), arg(bx), arg(by), o, l, f, nm, js, jo);
        let hit = ours(&mut s.g, ax, ay, bx, by, js, jo);
        check_hit(ctx, ret, hit, &|| {
            format!("{}: {name}({ax}, {ay}, {bx}, {by}, {js}, {jo})", s.what())
        });
        n.add(name);
    }
    super::trace("rest of searches");
    // Several at once.
    let (ax, ay, bx, by) = segment(s);
    let checks = s.rng.below(0x20) as u32;
    clear(ctx);
    let (o, l, f, nm) = out(ctx);
    let ret = fns::mpCheckMultiple(
        ctx,
        arg(ax),
        arg(ay),
        arg(bx),
        arg(by),
        o,
        l,
        f,
        nm,
        checks,
        js,
        jo,
    );
    let hit = s.g.check_multiple(ax, ay, bx, by, checks, js, jo);
    check_hit(ctx, ret, hit, &|| {
        format!(
            "{}: mpCheckMultiple({ax}, {ay}, {bx}, {by}, {checks:#x}, {js}, {jo})",
            s.what()
        )
    });
    n.add("mpCheckMultiple");
    for remap in [false, true] {
        let (ax, ay, bx, by) = segment(s);
        clear(ctx);
        let (o, l, f, nm) = out(ctx);
        let (ret, hit) = if remap {
            (
                fns::mpCheckAllRemap(ctx, o, l, f, nm, js, jo, arg(ax), arg(ay), arg(bx), arg(by)),
                s.g.check_all_remap(js, jo, ax, ay, bx, by),
            )
        } else {
            (
                fns::mpCheckAll(ctx, o, l, f, nm, js, jo, arg(ax), arg(ay), arg(bx), arg(by)),
                s.g.check_all(js, jo, ax, ay, bx, by),
            )
        };
        let name = if remap {
            "mpCheckAllRemap"
        } else {
            "mpCheckAll"
        };
        check_hit(ctx, ret, hit, &|| {
            format!("{}: {name}({ax}, {ay}, {bx}, {by})", s.what())
        });
        n.add(name);
    }
    // The wall sweeps: an edge moving from aâ€“b to câ€“d.
    for left in [false, true] {
        let (ax, ay, bx, by) = segment(s);
        let (dx, dy) = (s.rng.range(-6.0, 6.0), s.rng.range(-6.0, 6.0));
        let (cx, cy, ddx, ddy) = (ax + dx, ay + dy, bx + dx, by + dy);
        clear(ctx);
        let p: Vec<f64> = [ax, ay, bx, by, cx, cy, ddx, ddy]
            .iter()
            .map(|&x| arg(x))
            .collect();
        let (ret, ours) = if left {
            (
                fns::mpLib_800515A0_LeftWall(
                    ctx,
                    p[0],
                    p[1],
                    p[2],
                    p[3],
                    p[4],
                    p[5],
                    p[6],
                    p[7],
                    handle(ctx, LINE),
                    js,
                    jo,
                ),
                s.g.sweep_left_wall(ax, ay, bx, by, cx, cy, ddx, ddy, js, jo),
            )
        } else {
            (
                fns::mpLib_800511A4_RightWall(
                    ctx,
                    p[0],
                    p[1],
                    p[2],
                    p[3],
                    p[4],
                    p[5],
                    p[6],
                    p[7],
                    handle(ctx, LINE),
                    js,
                    jo,
                ),
                s.g.sweep_right_wall(ax, ay, bx, by, cx, cy, ddx, ddy, js, jo),
            )
        };
        let name = if left {
            "mpLib_800515A0_LeftWall"
        } else {
            "mpLib_800511A4_RightWall"
        };
        let theirs = (ret != 0).then(|| ctx.read_u32(LINE) as i32);
        if ret == 0 {
            assert_eq!(ctx.read_u32(LINE), UNTOUCHED, "{}: {name}", s.what());
        }
        assert_eq!(ours, theirs, "{}: {name}({p:?})", s.what());
        n.add(name);
    }
    // The floor under a point.
    let (x, y) = point(s);
    let pos = Vec3::new(x, y, 0.0);
    clear(ctx);
    put_vec(ctx, POS, pos);
    let theirs = fns::mpLib_8005199C_Floor(ctx, cvec(ctx, POS), js, jo);
    assert_eq!(
        s.g.floor_below(pos, js, jo),
        theirs,
        "{}: mpLib_8005199C_Floor({pos:?}, {js}, {jo})",
        s.what()
    );
    n.add("mpLib_8005199C_Floor");
    // Ledges in a box.
    let (x, y) = point(s);
    let (w, h) = (s.rng.range(0.0, 40.0), s.rng.range(0.0, 40.0));
    let dir = if s.rng.chance(2) { 1 } else { -1 };
    clear(ctx);
    let ret = fns::mpLib_80051BA8_Floor(
        ctx,
        cvec(ctx, OUT),
        ls,
        js,
        jo,
        dir,
        arg(x - w),
        arg(y - h),
        arg(x + w),
        arg(y + h),
    );
    let (ledge, at) =
        s.g.ledge_in_box(ls, js, jo, dir, x - w, y - h, x + w, y + h);
    let what = format!(
        "{}: mpLib_80051BA8_Floor({ls}, {js}, {jo}, {dir}, {}, {}, {}, {})",
        s.what(),
        x - w,
        y - h,
        x + w,
        y + h
    );
    assert_eq!(ledge, ret, "{what}");
    match at {
        Some(v) => assert_eq!(vbits(v), vec_at(ctx, OUT), "{what}"),
        None => assert_eq!(ctx.read_u32(OUT), UNTOUCHED, "{what}"),
    }
    n.add("mpLib_80051BA8_Floor");
    // The island under a point.
    let (x, y) = point(s);
    let dy = s.rng.range(-60.0, 20.0);
    let pos = Vec3::new(x, y, 0.0);
    put_vec(ctx, POS, pos);
    let theirs = fns::mpIsland_8005AC14(ctx, cvec(ctx, POS), arg(dy)).addr();
    let ours = s.g.island_below(pos, dy);
    assert_eq!(
        ours,
        island_for(s, theirs),
        "{}: mpIsland_8005AC14({pos:?}, {dy})",
        s.what()
    );
    n.add("mpIsland_8005AC14");
}

/// The island queries, on every island.
pub fn island_queries(s: &mut Stage, n: &mut Counts) {
    super::trace("island_queries");
    let ctx = s.ctx;
    let pairs: Vec<(u32, u32)> = s.islands.iter().map(|(&a, &o)| (a, o)).collect();
    for (addr, ours) in pairs {
        let id = IslandId(ours);
        let isl = handle::<mp_UnkStruct0>(ctx, addr);
        assert_eq!(
            i32::from(s.g.island_moving(id)),
            fns::mpIsland_8005AC8C(ctx, isl),
            "{}: mpIsland_8005AC8C({addr:#x})",
            s.what()
        );
        n.add("mpIsland_8005AC8C");
        // With both ends wanted it walks the floors that end the island (see `walk_loops`).
        let r = s.g.joints[s.g.islands.get(id).joint as usize].desc.ranges[0];
        let loops = (r.start..r.start + r.count.max(0)).any(|l| walk_loops(&s.g, i32::from(l)));
        for (want_l, want_r) in [(true, true), (true, false), (false, true), (false, false)] {
            if want_l && want_r && loops {
                continue;
            }
            clear(ctx);
            fns::mpIsland_8005ACE8(
                ctx,
                isl,
                cvec(ctx, if want_l { OUT } else { 0 }),
                cvec(ctx, if want_r { OUT2 } else { 0 }),
            );
            let u = f32::from_bits(UNTOUCHED);
            let (mut l, mut r) = (Vec3::new(u, u, u), Vec3::new(u, u, u));
            s.g.island_ends(id, want_l.then_some(&mut l), want_r.then_some(&mut r));
            assert_eq!(
                (vbits(l), vbits(r)),
                (vec_at(ctx, OUT), vec_at(ctx, OUT2)),
                "{}: mpIsland_8005ACE8({addr:#x}, {want_l}, {want_r})",
                s.what()
            );
            n.add("mpIsland_8005ACE8");
        }
    }
}

/// The terrain tables, on this stage and every other stage kind.
pub fn terrain_queries(s: &mut Stage, n: &mut Counts) {
    let ctx = s.ctx;
    let info = fns::stage_info(ctx);
    for grkind in 0..0x47 {
        info.set_grkind(grkind);
        for terrain in 0..20u32 {
            let ours = mp::terrain_sounds(grkind, terrain);
            assert_eq!(
                mp::friction(grkind, terrain).to_bits(),
                bits(fns::mpLib_800569EC(ctx, terrain)),
                "mpLib_800569EC({grkind}, {terrain})"
            );
            type Ptr = for<'a> fn(&'a Ctx, i32, Val<'a, i32>) -> Val<'a, i32>;
            type Info = for<'a> fn(&'a Ctx, i32, Val<'a, i32>) -> i32;
            let sets: [(Ptr, Info); 3] = [
                (fns::mpLib_80056A1C, fns::mpLib_80056A54),
                (fns::mpLib_80056A8C, fns::mpLib_80056AC4),
                (fns::mpLib_80056AFC, fns::mpLib_80056B34),
            ];
            for (set, (p, i)) in sets.into_iter().enumerate() {
                clear(ctx);
                let at = p(ctx, terrain as i32, handle(ctx, LINE)).addr();
                let (sounds, first) = ours.sounds(set);
                let theirs: [i32; 4] =
                    std::array::from_fn(|k| ctx.read_u32(at + 4 * k as u32) as i32);
                assert_eq!(
                    (*sounds, first),
                    (theirs, ctx.read_u32(LINE) as i32),
                    "terrain set {set} ({grkind}, {terrain})"
                );
                let second = i(ctx, terrain as i32, handle(ctx, LINE));
                assert_eq!(
                    ours.info(set),
                    (second, ctx.read_u32(LINE) as i32),
                    "terrain info {set} ({grkind}, {terrain})"
                );
            }
        }
    }
    info.set_grkind(s.grkind);
    for name in [
        "mpLib_800569EC",
        "mpLib_80056A1C",
        "mpLib_80056A54",
        "mpLib_80056A8C",
        "mpLib_80056AC4",
        "mpLib_80056AFC",
        "mpLib_80056B34",
    ] {
        n.add(name);
    }
}

/// Searches with the joints already marked against an area, as the collision code does.
fn bounded(s: &mut Stage, n: &mut Counts) {
    let ctx = s.ctx;
    let (ax, ay, bx, by) = segment(s);
    match s.rng.below(3) {
        0 => {
            fns::mpBoundingCheck(
                ctx,
                arg(ax.min(bx)),
                arg(ay.min(by)),
                arg(ax.max(bx)),
                arg(ay.max(by)),
            );
            s.g.check_bounding(ax.min(bx), ay.min(by), ax.max(bx), ay.max(by));
            n.add("mpBoundingCheck");
        }
        1 => {
            fns::mpBoundingCheck2(ctx, arg(ax), arg(ay), arg(bx), arg(by));
            s.g.check_bounding_segment(ax, ay, bx, by);
            n.add("mpBoundingCheck2");
        }
        _ => {
            let (cx, cy) = point(s);
            let (dx, dy) = point(s);
            let p: Vec<f64> = [ax, ay, bx, by, cx, cy, dx, dy]
                .iter()
                .map(|&x| arg(x))
                .collect();
            fns::mpBoundingCheck3(ctx, p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
            s.g.check_bounding_quad(ax, ay, bx, by, cx, cy, dx, dy);
            n.add("mpBoundingCheck3");
        }
    }
    assert_eq!(
        i32::from(s.g.checked_bounding()),
        fns::mpCheckedBounding(ctx),
        "{}: mpCheckedBounding",
        s.what()
    );
    n.add("mpCheckedBounding");
    state::check(s, "after a bounding check");
    searches(s, n);
    fns::mpUncheckBounding(ctx);
    s.g.uncheck_bounding();
    n.add("mpUncheckBounding");
}

pub fn run(s: &mut Stage, n: &mut Counts, iterations: usize) {
    island_queries(s, n);
    for i in 0..iterations {
        pure(s, n);
        line_queries(s, n);
        probes(s, n);
        if s.rng.chance(8) {
            bounded(s, n);
        } else {
            searches(s, n);
        }
        if i % 100 == 99 {
            state::check(s, "after queries");
        }
    }
    state::check(s, "after queries");
}
