//! Random collision data around a stage's lines, written into ssbm-rs's memory the way the
//! game lays it out (`CollData`, 0x19C bytes).

use leaf_check::sym;
use melee_sim::mp::{
    CEILING, FLOOR, LEFT_WALL, LINE_ENABLED, LINE_HIDDEN, LINE_PLATFORM, RIGHT_WALL,
};
use ssbm_rt::Ctx;

use super::World;

/// Field offsets in `CollData`.
#[allow(dead_code)]
pub mod off {
    pub const GOBJ: u32 = 0x0;
    pub const CUR: u32 = 0x4;
    pub const PREV: u32 = 0x10;
    pub const LAST: u32 = 0x1C;
    pub const X28: u32 = 0x28;
    pub const X34: u32 = 0x34;
    pub const X35: u32 = 0x35;
    pub const FACING: u32 = 0x36;
    pub const X38: u32 = 0x38;
    pub const FLOOR_SKIP: u32 = 0x3C;
    pub const LEDGE_RIGHT: u32 = 0x40;
    pub const LEDGE_LEFT: u32 = 0x44;
    pub const JOINT_SKIP: u32 = 0x48;
    pub const JOINT_ONLY: u32 = 0x4C;
    pub const X50: u32 = 0x50;
    pub const SNAP: u32 = 0x54;
    pub const LSTICK: u32 = 0x60;
    pub const X64_ECB: u32 = 0x64;
    pub const DESIRED: u32 = 0x84;
    pub const ECB: u32 = 0xA4;
    pub const PREV_ECB: u32 = 0xC4;
    pub const XE4_ECB: u32 = 0xE4;
    pub const SOURCE: u32 = 0x104;
    pub const X130: u32 = 0x130;
    pub const ENV: u32 = 0x134;
    pub const PREV_ENV: u32 = 0x138;
    pub const X13C: u32 = 0x13C;
    pub const CONTACT: u32 = 0x140;
    pub const FLOOR: u32 = 0x14C;
    pub const LEFT_WALL: u32 = 0x160;
    pub const RIGHT_WALL: u32 = 0x174;
    pub const CEILING: u32 = 0x188;
}

/// Writes into the collision data at `base`.
pub struct Writer<'c> {
    pub ctx: &'c Ctx,
    pub base: u32,
}

impl Writer<'_> {
    pub fn u32(&self, o: u32, v: u32) {
        self.ctx.write_u32(self.base + o, v);
    }
    pub fn i32(&self, o: u32, v: i32) {
        self.u32(o, v as u32);
    }
    pub fn f(&self, o: u32, v: f32) {
        self.u32(o, v.to_bits());
    }
    pub fn fs(&self, o: u32, vs: &[f32]) {
        for (i, v) in vs.iter().enumerate() {
            self.f(o + 4 * i as u32, *v);
        }
    }
    pub fn u8(&self, o: u32, v: u8) {
        self.ctx.write_bytes(self.base + o, &[v]);
    }
    pub fn r32(&self, o: u32) -> u32 {
        self.ctx.read_u32(self.base + o)
    }
    pub fn rf(&self, o: u32) -> f32 {
        f32::from_bits(self.r32(o))
    }
}

/// A line of one kind (any kind for 0) that the game's searches see, if the stage has one.
/// Never one on a closed run of its kind, whose walks never end in the original.
pub fn line_of(w: &mut World, kind: u32) -> Option<i32> {
    let g = &w.st.g;
    let ok: Vec<i32> = (0..g.lines.len())
        .filter(|&l| {
            let f = g.lines[l].flags;
            (kind == 0 || f & kind != 0)
                && f & LINE_ENABLED != 0
                && f & LINE_HIDDEN == 0
                && !super::queries::walk_loops(g, l as i32)
        })
        .map(|l| l as i32)
        .collect();
    if ok.is_empty() {
        // Any line of the kind, live or not.
        let any: Vec<i32> = (0..g.lines.len())
            .filter(|&l| {
                (kind == 0 || g.lines[l].flags & kind != 0)
                    && !super::queries::walk_loops(g, l as i32)
            })
            .map(|l| l as i32)
            .collect();
        if any.is_empty() {
            return None;
        }
        return Some(any[w.st.rng.below(any.len() as u64) as usize]);
    }
    Some(ok[w.st.rng.below(ok.len() as u64) as usize])
}

/// A float from a game-like distribution around `0..scale`, now and then an awkward one.
pub fn wobble(w: &mut World, scale: f32) -> f32 {
    let r = &mut w.st.rng;
    match r.below(16) {
        0 => 0.0,
        1 => -0.0,
        2 => r.range(-0.01, 0.01),
        _ => r.range(-scale, scale),
    }
}

/// An ECB as the loads leave it: top above bottom, the sides between.
pub fn ecb(w: &mut World) -> [f32; 8] {
    let r = &mut w.st.rng;
    let tiny = r.chance(6);
    let bottom_y = match r.below(4) {
        0 | 1 => 0.0,
        2 => r.range(0.0, 4.0),
        _ => r.range(-2.0, 2.0),
    };
    let height = if tiny {
        r.range(1.0, 6.5)
    } else {
        r.range(4.0, 24.0)
    };
    let top_y = bottom_y + height;
    let mid = bottom_y + height * r.range(0.3, 0.7);
    let skew = if r.chance(3) { r.range(-2.0, 2.0) } else { 0.0 };
    let (right, left) = if tiny {
        (r.range(0.5, 3.0), -r.range(0.5, 3.0))
    } else {
        (r.range(1.5, 10.0), -r.range(1.5, 10.0))
    };
    let side_dy = if r.chance(4) { r.range(-1.0, 1.0) } else { 0.0 };
    [
        skew,
        top_y,
        skew * 0.5,
        bottom_y,
        right,
        mid + side_dy,
        left,
        mid - side_dy,
    ]
}

fn perturb(w: &mut World, e: [f32; 8]) -> [f32; 8] {
    if w.st.rng.chance(2) {
        return e;
    }
    let mut out = e;
    for v in &mut out {
        if w.st.rng.chance(3) {
            *v += w.st.rng.range(-3.0, 3.0);
        }
    }
    // Keep it a diamond.
    if out[1] < out[3] + 1.0 {
        out[1] = out[3] + 1.0;
    }
    out
}

/// A surface the object touches: a line of `kind` (or none), its flags and its normal.
fn surface(w: &mut World, at: u32, kind: u32, none_one_in: u64) {
    let wr = Writer {
        ctx: w.ctx(),
        base: super::COLL,
    };
    let line = if w.st.rng.chance(none_one_in) {
        None
    } else {
        line_of(w, kind)
    };
    match line {
        None => {
            wr.i32(at, -1);
            wr.u32(at + 4, 0);
            let ny = if kind & (FLOOR | RIGHT_WALL) != 0 {
                1.0
            } else {
                -1.0
            };
            wr.fs(at + 8, &[0.0, ny, 0.0]);
        }
        Some(l) => {
            let g = &w.st.g;
            let n = g.line_normal(l);
            wr.i32(at, l);
            wr.u32(at + 4, u32::from(g.lines[l as usize].lo_flags));
            wr.fs(at + 8, &[n.x, n.y, n.z]);
        }
    }
}

/// Fresh collision data in ssbm-rs's memory at `base`, placed near a random line, imported
/// to ours by the caller. A fixed box source unless the round has joints.
pub fn place(w: &mut World, base: u32) {
    let ctx = w.ctx();
    let wr = Writer { ctx, base };
    // Fields mpcoll never reads keep random bits.
    for o in (0..super::COLL_SIZE).step_by(4) {
        wr.u32(o, w.st.rng.bits() as u32);
    }
    let anchor = line_of(w, 0).unwrap();
    let (a, b) = {
        let g = &w.st.g;
        let l = g.lines[anchor as usize];
        (g.vertices[l.v0 as usize].pos, g.vertices[l.v1 as usize].pos)
    };
    let t = w.st.rng.range(-0.1, 1.1);
    let kind = w.st.g.lines[anchor as usize].flags & 0xF;
    let e = ecb(w);
    // Beside the line on its open side, a little into it, or on it.
    let (mut x, mut y) = (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    let off = w.st.rng.range(-4.0, 8.0);
    match kind {
        k if k & FLOOR != 0 => y += off - e[3],
        k if k & CEILING != 0 => y -= off + e[1],
        k if k & LEFT_WALL != 0 => x -= off + e[4],
        k if k & RIGHT_WALL != 0 => x += off - e[6],
        _ => {}
    }
    if w.st.rng.chance(8) {
        x += w.st.rng.range(-40.0, 40.0);
        y += w.st.rng.range(-40.0, 40.0);
    }
    let speed = match w.st.rng.below(5) {
        0 => 0.0,
        1 => 2.0,
        2 | 3 => 8.0,
        _ => 40.0,
    };
    let (vx, vy) = (wobble(w, speed), wobble(w, speed));
    // Moving toward the line more often than away.
    let (vx, vy) = match kind {
        k if k & FLOOR != 0 && w.st.rng.chance(2) => (vx, -vy.abs()),
        k if k & CEILING != 0 && w.st.rng.chance(2) => (vx, vy.abs()),
        k if k & LEFT_WALL != 0 && w.st.rng.chance(2) => (vx.abs(), vy),
        k if k & RIGHT_WALL != 0 && w.st.rng.chance(2) => (-vx.abs(), vy),
        _ => (vx, vy),
    };
    let z = if w.st.rng.chance(4) {
        w.st.rng.range(-5.0, 5.0)
    } else {
        0.0
    };
    wr.fs(off::CUR, &[x, y, z]);
    let (lx, ly) = (x - vx, y - vy);
    wr.fs(off::LAST, &[lx, ly, z]);
    let f = w.st.rng.range(0.0, 1.0);
    if w.st.rng.chance(2) {
        wr.fs(off::PREV, &[lx, ly, z]);
    } else {
        wr.fs(off::PREV, &[lx + vx * f, ly + vy * f, z]);
    }
    wr.fs(off::ECB, &e);
    let pe = perturb(w, e);
    wr.fs(off::PREV_ECB, &pe);
    let de = perturb(w, e);
    wr.fs(off::DESIRED, &de);
    let xe = perturb(w, e);
    wr.fs(off::X64_ECB, &xe);
    let gobj = if w.st.rng.chance(2) {
        0
    } else {
        0x8170_0000 + 0x10 * w.st.rng.below(8) as u32
    };
    wr.u32(off::GOBJ, gobj);
    let r = &mut w.st.rng;
    let x34 = (r.bits() as u8) & !0x04 | if r.chance(3) { 0x04 } else { 0 };
    wr.u8(off::X34, x34);
    wr.u8(off::X35, r.bits() as u8);
    let facing = [-1i16, 1, 0, -1, 1][r.below(5) as usize];
    ctx.write_bytes(base + off::FACING, &facing.to_be_bytes());
    let generation = ctx.read_u32(sym("mpColl_804D64AC"));
    let x38 = if r.chance(2) {
        generation
    } else {
        generation.wrapping_sub(1 + r.below(3) as u32)
    };
    wr.u32(off::X38, x38);
    let ledge_right = r.bits() as i32;
    let ledge_left = r.bits() as i32;
    wr.i32(off::LEDGE_RIGHT, ledge_right);
    wr.i32(off::LEDGE_LEFT, ledge_left);
    let snap = [r.range(0.0, 16.0), r.range(-5.0, 20.0), r.range(5.0, 25.0)];
    wr.fs(off::SNAP, &snap);
    wr.f(off::LSTICK, r.range(-1.0, 1.0));
    wr.f(off::X50, r.range(-100.0, 100.0));
    let x130 = (r.bits() as u32) & 0x3F;
    wr.u32(off::X130, x130);
    let env_bits = [
        0x1, 0x20, 0x40, 0x800, 0x2000, 0x4000, 0x8000, 0x10000, 0x100000, 0x200000, 0x800000,
        0x1000000, 0x2000000, 0x10000000, 0x20000000,
    ];
    let mut env = 0;
    for b in env_bits {
        if r.chance(5) {
            env |= b;
        }
    }
    wr.u32(off::ENV, env);
    // Joint filters, mostly off.
    let joints = w.st.g.joints.len() as u64;
    let r = &mut w.st.rng;
    let skip = if r.chance(8) {
        r.below(joints) as i32
    } else {
        -1
    };
    let only = if r.chance(10) {
        r.below(joints) as i32
    } else {
        -1
    };
    wr.i32(off::JOINT_SKIP, skip);
    wr.i32(off::JOINT_ONLY, only);
    // Surfaces: what the object stood on or touched last frame.
    let floor_none = if kind & FLOOR != 0 { 6 } else { 2 };
    surface(w, off::FLOOR, FLOOR, floor_none);
    if kind & FLOOR != 0 && w.st.rng.chance(2) {
        wr.i32(off::FLOOR, anchor);
    }
    surface(w, off::CEILING, CEILING, 2);
    if kind & CEILING != 0 && w.st.rng.chance(2) {
        wr.i32(off::CEILING, anchor);
    }
    surface(w, off::LEFT_WALL, LEFT_WALL, 2);
    surface(w, off::RIGHT_WALL, RIGHT_WALL, 2);
    // Dropping through a platform: the floor stood on, or another platform.
    let floor = wr.r32(off::FLOOR) as i32;
    let skip = match w.st.rng.below(4) {
        0 if floor != -1 => floor,
        1 => line_of(w, FLOOR)
            .filter(|&l| u32::from(w.st.g.lines[l as usize].lo_flags) & LINE_PLATFORM != 0)
            .unwrap_or(-1),
        _ => -1,
    };
    wr.i32(off::FLOOR_SKIP, skip);
    box_source(w, base);
}

/// A fixed box source (an item's), or none set.
pub fn box_source(w: &mut World, base: u32) {
    let wr = Writer { ctx: w.ctx(), base };
    let r = &mut w.st.rng;
    for o in (off::SOURCE..off::SOURCE + 0x2C).step_by(4) {
        wr.u32(o, 0);
    }
    if r.chance(10) {
        return;
    }
    wr.i32(off::SOURCE, 2);
    let angle = match r.below(4) {
        0 | 1 => 0.0,
        2 => r.range(-7.0, 7.0),
        _ => r.range(-0.5, 0.5),
    };
    let b = [
        r.range(0.0, 20.0),
        r.range(-2.0, 10.0),
        r.range(0.0, 12.0),
        r.range(0.0, 12.0),
    ];
    wr.fs(off::SOURCE + 4, &[b[0], b[1], b[2], b[3], angle]);
    wr.fs(
        off::SOURCE + 0x20,
        &[r.range(-2.0, 2.0), r.range(0.0, 12.0), r.range(0.0, 12.0)],
    );
}

/// Moves the collision data beside a ledge's end, within about its snap box: to the left of a
/// floor's left end (grabbed by the ECB's right side) or the right of its right end.
pub fn beside_ledge(w: &mut World, right_end: bool) {
    let g = &w.st.g;
    let ledges: Vec<i32> = (0..g.lines.len() as i32)
        .filter(|&l| {
            let line = g.lines[l as usize];
            line.flags & FLOOR != 0
                && u32::from(line.lo_flags) & melee_sim::mp::LINE_LEDGE != 0
                && line.flags & LINE_ENABLED != 0
                && line.flags & LINE_HIDDEN == 0
                && !super::queries::walk_loops(g, l)
        })
        .collect();
    if ledges.is_empty() {
        return;
    }
    let l = ledges[w.st.rng.below(ledges.len() as u64) as usize];
    let edge = if right_end {
        g.floor_right(l)
    } else {
        g.floor_left(l)
    };
    let wr = Writer {
        ctx: w.ctx(),
        base: super::COLL,
    };
    let snap = [wr.rf(off::SNAP), wr.rf(off::SNAP + 4), wr.rf(off::SNAP + 8)];
    let side = if right_end {
        wr.rf(off::ECB + 0x18)
    } else {
        wr.rf(off::ECB + 0x10)
    };
    let r = &mut w.st.rng;
    let k = snap[0] * r.range(-0.2, 1.3);
    let x = if right_end {
        edge.x - side + k
    } else {
        edge.x - side - k
    };
    let y = edge.y - snap[1] + snap[2] * r.range(-0.7, 0.7);
    wr.fs(off::CUR, &[x, y, 0.0]);
    wr.fs(off::PREV, &[x + r.range(-3.0, 3.0), y, 0.0]);
}

/// Moves the collision data so one end of a wall pokes into an edge of the ECB on the wall's
/// side (the left edges for right walls), the ECB skewed: corners, where the corrections test
/// the wall's vertices against the ECB's edges.
pub fn at_wall_end(w: &mut World, right_wall: bool) {
    let Some(l) = line_of(w, if right_wall { RIGHT_WALL } else { LEFT_WALL }) else {
        return;
    };
    let v = {
        let g = &w.st.g;
        let line = g.lines[l as usize];
        let v = if w.st.rng.chance(2) { line.v0 } else { line.v1 };
        g.vertices[v as usize].pos
    };
    let mut e = ecb(w);
    let ctx = w.ctx();
    let r = &mut w.st.rng;
    e[0] = r.range(-2.0, 2.0);
    e[2] = r.range(-2.0, 2.0);
    // A point on the ECB's lower or upper edge toward the wall, from its side point (0) to
    // the bottom or top (1).
    let (sx, sy) = if right_wall {
        (e[6], e[7])
    } else {
        (e[4], e[5])
    };
    let (ex, ey) = if r.chance(2) {
        (e[2], e[3])
    } else {
        (e[0], e[1])
    };
    let f = r.range(0.0, 1.0);
    let (px, py) = (sx + (ex - sx) * f, sy + (ey - sy) * f);
    let into = r.range(-1.0, 3.0) * if right_wall { -1.0 } else { 1.0 };
    let x = v.x - px + into;
    let y = v.y - py;
    let back = r.range(0.0, 4.0) * if right_wall { 1.0 } else { -1.0 };
    let wr = Writer {
        ctx,
        base: super::COLL,
    };
    wr.fs(off::ECB, &e);
    wr.fs(off::PREV_ECB, &e);
    wr.fs(off::CUR, &[x, y, 0.0]);
    wr.fs(off::PREV, &[x + back, y + r.range(-1.0, 1.0), 0.0]);
}
