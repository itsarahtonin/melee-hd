//! Random mutations of the geometry, on both sides, with the full state compared after each.

use std::cell::RefCell;

use leaf_check::{SCRATCH, arg, bits, handle};
use melee_sim::mp::{GROUP_DYNAMIC, JObjTree, JointPose};
use melee_sim::mtx::Mtx34;
use ssbm_rt::{Addr, Ctx, FnPtr};
use ssbm_types::fns;
use ssbm_types::records::{CollData, CollJoint, HSD_JObj};

use super::queries::{self, Counts, FLAGS, LINE, clear};
use super::{Stage, state};

/// Fake model joints for the collision joints to follow, clear of the output slots.
const JOBJS: u32 = SCRATCH + 0x1000;
const JOBJ_SIZE: u32 = 0x80;
const JOBJ_COUNT: u32 = 16;
/// A character's collision data, for the ledge callback.
const COLL: u32 = SCRATCH + 0x3000;
/// The fake address of the stage's joint callback.
const JOINT_CALLBACK: u32 = 0x817E_0500;

thread_local! {
    static CALLED: RefCell<Vec<[u32; 6]>> = const { RefCell::new(Vec::new()) };
}

fn native_joint_callback(ctx: &Ctx) {
    let r = |i| ctx.regs.r(i);
    CALLED.with(|c| {
        c.borrow_mut()
            .push([r(3), r(4), r(5), r(6), r(7), bits(ctx.regs.f(1))])
    });
}

pub fn register_natives(ctx: &Ctx) {
    ctx.register(JOINT_CALLBACK, native_joint_callback);
}

/// The fake joints as our side walks them: handles are their addresses.
struct Tree<'a>(&'a Ctx);

impl JObjTree<u32> for Tree<'_> {
    fn child(&self, j: u32) -> Option<u32> {
        Some(self.0.read_u32(j + 0x10)).filter(|&p| p != 0)
    }
    fn next(&self, j: u32) -> Option<u32> {
        Some(self.0.read_u32(j + 8)).filter(|&p| p != 0)
    }
    fn parent(&self, j: u32) -> Option<u32> {
        Some(self.0.read_u32(j + 0xC)).filter(|&p| p != 0)
    }
    fn flags(&self, j: u32) -> u32 {
        self.0.read_u32(j + 0x14)
    }
}

fn jobj(k: u32) -> u32 {
    JOBJS + JOBJ_SIZE * k
}

/// A random joint tree. Flags may hide a joint (`JOBJ_HIDDEN`) or make it an instance, but
/// never mark its matrix dirty: rebuilding a matrix is the joint system's business, so both
/// sides are handed the same finished matrix.
fn build_tree(s: &mut Stage) {
    let ctx = s.ctx;
    for k in 0..JOBJ_COUNT {
        let a = jobj(k);
        for o in (0..JOBJ_SIZE).step_by(4) {
            ctx.write_u32(a + o, 0);
        }
    }
    for k in 1..JOBJ_COUNT {
        let parent = jobj(s.rng.below(u64::from(k)) as u32);
        let me = jobj(k);
        ctx.write_u32(me + 0xC, parent);
        // Appended as the parent's last child.
        let first = ctx.read_u32(parent + 0x10);
        if first == 0 {
            ctx.write_u32(parent + 0x10, me);
        } else {
            let mut c = first;
            while ctx.read_u32(c + 8) != 0 {
                c = ctx.read_u32(c + 8);
            }
            ctx.write_u32(c + 8, me);
        }
        let mut flags = 0;
        if s.rng.chance(5) {
            flags |= 1 << 12;
        }
        ctx.write_u32(me + 0x14, flags);
    }
}

/// A transform for a model joint: a uniform scale, a rotation, an uneven scale or a shear,
/// each with a translation.
fn random_matrix(s: &mut Stage) -> Mtx34 {
    let rng = &mut s.rng;
    let (tx, ty, tz) = (
        rng.range(-60.0, 60.0),
        rng.range(-60.0, 60.0),
        rng.range(-5.0, 5.0),
    );
    let sc = rng.range(0.5, 2.0);
    let m = match rng.below(6) {
        0 => [[sc, 0.0, 0.0, tx], [0.0, sc, 0.0, ty], [0.0, 0.0, sc, tz]],
        1 => [
            [1.0, 0.0, 0.0, tx],
            [0.0, 1.0, 0.0, ty],
            [0.0, 0.0, 1.0, tz],
        ],
        2 => {
            let a = rng.range(-1.0, 1.0);
            let (sn, c) = a.sin_cos();
            [
                [c * sc, -sn * sc, 0.0, tx],
                [sn * sc, c * sc, 0.0, ty],
                [0.0, 0.0, sc, tz],
            ]
        }
        3 => [
            [sc, 0.0, 0.0, tx],
            [0.0, rng.range(0.5, 2.0), 0.0, ty],
            [0.0, 0.0, 1.0, tz],
        ],
        4 => [[sc, 0.0, 0.0, tx], [0.0, sc, 0.0, ty], [0.0, 0.0, 1.0, tz]],
        _ => [
            [sc, rng.range(-0.3, 0.3), rng.range(-0.3, 0.3), tx],
            [rng.range(-0.3, 0.3), sc, rng.range(-0.3, 0.3), ty],
            [rng.range(-0.3, 0.3), rng.range(-0.3, 0.3), sc, tz],
        ],
    };
    Mtx34 { m }
}

fn write_matrix(ctx: &Ctx, j: u32, m: &Mtx34) {
    for (r, row) in m.m.iter().enumerate() {
        for (c, v) in row.iter().enumerate() {
            ctx.write_u32(j + 0x44 + 4 * (r * 4 + c) as u32, v.to_bits());
        }
    }
}

/// Our side's reading of a model joint, as the original reads it.
fn pose(ctx: &Ctx, j: u32) -> JointPose {
    if ctx.read_u32(j + 0x14) & 0x10 != 0 {
        return JointPose::Hidden;
    }
    let mut m = Mtx34::default();
    for (r, row) in m.m.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = f32::from_bits(ctx.read_u32(j + 0x44 + 4 * (r * 4 + c) as u32));
        }
    }
    JointPose::Visible(m)
}

/// Whether reclassifying a joint's dynamic lines is safe: the original asserts on a dynamic
/// line without length, so the game never moves one onto its other end.
fn dynamics_safe(s: &Stage, joint: usize) -> bool {
    let r = s.g.joints[joint].desc.ranges[GROUP_DYNAMIC];
    (r.start as usize..(r.start + r.count.max(0)) as usize).all(|l| {
        let line = s.g.lines[l];
        let (a, b) = (
            s.g.vertices[line.v0 as usize],
            s.g.vertices[line.v1 as usize],
        );
        let ok = |p: melee_sim::mp::Vec2, q: melee_sim::mp::Vec2| {
            let (dx, dy) = (q.x - p.x, q.y - p.y);
            dx.is_finite() && dy.is_finite() && (dx != 0.0 || dy != 0.0)
        };
        ok(a.pos, b.pos) && ok(a.base, b.base)
    })
}

fn joint_handle(ctx: &Ctx, j: usize) -> CollJoint<'_> {
    let base = ctx.read_u32(leaf_check::sym("groundCollJoint"));
    handle(ctx, base + 0x34 * j as u32)
}

/// One random mutation on both sides; returns its name.
fn mutate(s: &mut Stage) -> &'static str {
    let ctx = s.ctx;
    let joints = s.g.joints.len() as u64;
    let j = s.rng.below(joints) as usize;
    let ji = j as i32;
    let line = s.rng.below(s.g.lines.len() as u64) as i32;
    let vtx = s.rng.below(s.g.vertices.len() as u64) as i32;
    match s.rng.below(32) {
        0 => {
            fns::mpJointHide(ctx, ji);
            s.g.hide_joint(ji);
            "mpJointHide"
        }
        1 => {
            fns::mpJointUnhide(ctx, ji);
            s.g.unhide_joint(ji);
            "mpJointUnhide"
        }
        2 => {
            fns::mpJointListAdd(ctx, ji);
            s.g.enable_joint(ji);
            "mpJointListAdd"
        }
        3 => {
            fns::mpLib_80057BC0(ctx, ji);
            s.g.disable_joint(ji);
            "mpLib_80057BC0"
        }
        4 => {
            fns::mpLib_80057528(ctx, line);
            s.g.enable_line(line);
            "mpLib_80057528"
        }
        5 => {
            fns::mpLib_800575B0(ctx, line);
            s.g.disable_line(line);
            "mpLib_800575B0"
        }
        6 => {
            fns::mpLib_80057FDC(ctx, ji);
            s.g.allow_islands(ji);
            "mpLib_80057FDC"
        }
        7 => {
            fns::mpLib_80058044(ctx, ji);
            s.g.forbid_islands(ji);
            "mpLib_80058044"
        }
        8 if s.rng.chance(4) => {
            fns::mpJointSetB10(ctx, ji);
            s.g.set_always_near(ji);
            "mpJointSetB10"
        }
        9 => {
            fns::mpLib_80057424(ctx, ji);
            s.g.settle_vertices(ji);
            "mpLib_80057424"
        }
        10 | 11 => {
            let (x, y) = s.g.vertex_pos(vtx);
            clear(ctx);
            fns::mpVtxGetPos(ctx, vtx, handle(ctx, LINE), handle(ctx, FLAGS));
            assert_eq!(
                [x.to_bits(), y.to_bits()],
                [ctx.read_u32(LINE), ctx.read_u32(FLAGS)],
                "{}: mpVtxGetPos({vtx})",
                s.what()
            );
            let (nx, ny) = (x + s.rng.range(-3.0, 3.0), y + s.rng.range(-3.0, 3.0));
            fns::mpVtxSetPos(ctx, vtx, arg(nx), arg(ny));
            s.g.set_vertex_pos(vtx, nx, ny);
            "mpVtxSetPos"
        }
        12 => {
            let l = s.g.lines[line as usize];
            let (a, b) = (
                s.g.vertices[l.v0 as usize].pos,
                s.g.vertices[l.v1 as usize].pos,
            );
            let d: Vec<f32> = (0..4).map(|_| s.rng.range(-2.0, 2.0)).collect();
            let p = [a.x + d[0], a.y + d[1], b.x + d[2], b.y + d[3]];
            fns::mpLineSetPos(ctx, line, arg(p[0]), arg(p[1]), arg(p[2]), arg(p[3]));
            s.g.set_line_pos(line, p[0], p[1], p[2], p[3]);
            "mpLineSetPos"
        }
        13 => {
            let p: Vec<f32> = (0..4).map(|_| s.rng.range(-20.0, 20.0)).collect();
            fns::mpLib_80056758(ctx, line, arg(p[0]), arg(p[1]), arg(p[2]), arg(p[3]));
            s.g.offset_line(line, p[0], p[1], p[2], p[3]);
            "mpLib_80056758"
        }
        14 if dynamics_safe(s, j) => {
            fns::mpJointUpdateDynamics(ctx, ji);
            s.g.update_dynamics(ji);
            "mpJointUpdateDynamics"
        }
        15 if dynamics_safe(s, j) => {
            fns::mpLib_80055E24(ctx, ji);
            s.g.update_dynamics_and_islands(ji);
            "mpLib_80055E24"
        }
        16 => {
            fns::mpLib_8005667C(ctx, ji);
            s.g.update_joint_islands(ji);
            "mpLib_8005667C"
        }
        17 => {
            fns::mpJointUpdateBounding(ctx, ji);
            s.g.update_bounding(ji);
            "mpJointUpdateBounding"
        }
        18 => {
            let root = if s.rng.chance(10) {
                0
            } else {
                jobj(s.rng.below(4) as u32)
            };
            let z = s.rng.below(18) as i32;
            fns::mpLib_800552B0(ctx, ji, handle::<HSD_JObj>(ctx, root), z);
            s.g.bind_jobj(ji, &Tree(ctx), (root != 0).then_some(root), z);
            "mpLib_800552B0"
        }
        19..=22 => {
            // Move the joint's model joint, then the joint with it.
            if let Some(jo) = s.g.joints[j].jobj {
                let m = random_matrix(s);
                write_matrix(ctx, jo, &m);
                let flags = ctx.read_u32(jo + 0x14);
                let hidden = s.rng.chance(6);
                ctx.write_u32(jo + 0x14, if hidden { flags | 0x10 } else { flags & !0x10 });
            }
            if !dynamics_safe(s, j) {
                return "skipped";
            }
            fns::mpLib_80055E9C(ctx, ji);
            s.g.update_joint(ji, |jo| pose(ctx, jo));
            "mpLib_80055E9C"
        }
        23 => {
            fns::mpLib_80058614_Floor(ctx);
            s.g.measure_floors();
            "mpLib_80058614_Floor"
        }
        24 => {
            let user = if s.rng.chance(4) {
                0
            } else {
                s.rng.bits() as u32 | 0x8000_0000
            };
            let cb = if s.rng.chance(4) { 0 } else { JOINT_CALLBACK };
            if s.rng.chance(2) {
                fns::mpJointSetCb1(ctx, ji, handle::<Addr>(ctx, user), handle::<FnPtr>(ctx, cb));
                s.g.set_ledge_callback(ji, (user != 0).then_some(user), (cb != 0).then_some(cb));
                "mpJointSetCb1"
            } else {
                fns::mpJointSetCb2(ctx, ji, handle::<Addr>(ctx, user), handle::<FnPtr>(ctx, cb));
                s.g.set_callback(ji, (user != 0).then_some(user), (cb != 0).then_some(cb));
                "mpJointSetCb2"
            }
        }
        25 => {
            fns::mpJointClearCb1(ctx, ji);
            s.g.clear_ledge_callback(ji);
            "mpJointClearCb1"
        }
        26 => {
            for (second, name) in [(false, "mpJointGetCb1"), (true, "mpJointGetCb2")] {
                clear(ctx);
                let (cb, user) = if second {
                    fns::mpJointGetCb2(ctx, ji, handle(ctx, LINE), handle(ctx, FLAGS));
                    s.g.callback(ji)
                } else {
                    fns::mpJointGetCb1(ctx, ji, handle(ctx, LINE), handle(ctx, FLAGS));
                    s.g.ledge_callback(ji)
                };
                assert_eq!(
                    [cb.unwrap_or(0), user.unwrap_or(0)],
                    [ctx.read_u32(LINE), ctx.read_u32(FLAGS)],
                    "{}: {name}({ji})",
                    s.what()
                );
            }
            // A character grabbing a ledge.
            let ledge = if s.rng.chance(8) { -1 } else { line };
            let x50 = s.rng.bits() as u32;
            ctx.write_u32(COLL + 0x50, x50);
            CALLED.with(|c| c.borrow_mut().clear());
            fns::mpLib_8005811C(ctx, handle::<CollData>(ctx, COLL), ledge);
            let theirs = CALLED.with(|c| std::mem::take(&mut *c.borrow_mut()));
            let ours: Vec<[u32; 6]> =
                s.g.grab_callback(ledge)
                    .map(|(cb, user, joint)| {
                        assert_eq!(cb, JOINT_CALLBACK);
                        // The original passes `x50`, a float, converted to an integer.
                        let x50 = gekko_fp::fctiwz(arg(f32::from_bits(x50))) as u32;
                        [user.unwrap_or(0), joint as u32, COLL, x50, 3, 0]
                    })
                    .into_iter()
                    .collect();
            assert_eq!(ours, theirs, "{}: mpLib_8005811C({ledge})", s.what());
            "mpLib_8005811C"
        }
        27 => {
            let terrain = s.rng.below(20) as u32;
            fns::mpLib_80054D68(ctx, line, terrain);
            s.g.set_line_terrain(line, terrain);
            "mpLib_80054D68"
        }
        28 if s.rng.chance(6) => {
            fns::mpJointListUnlink(ctx, joint_handle(ctx, j));
            s.g.unlink_joint(ji);
            "mpJointListUnlink"
        }
        // The game links only distinct joints (`mpLib_80058560`); a joint linked to itself
        // can link a short line to itself, and the walks then never end.
        29 if joints > 1 => {
            let other = ((j as u64 + 1 + s.rng.below(joints - 1)) % joints) as i32;
            fns::mpLib_800581DC(ctx, ji, other);
            s.g.link_joints(ji, other);
            "mpLib_800581DC"
        }
        30 => {
            // Puts a line's end exactly 2 from the start of the line it links to, where the
            // link stops counting (a squared gap of 4).
            let linked: Vec<usize> = (0..s.g.lines.len())
                .filter(|&l| s.g.lines[l].next_link != -1)
                .collect();
            if linked.is_empty() {
                return "skipped";
            }
            let l = linked[s.rng.below(linked.len() as u64) as usize];
            let target = s.g.lines[s.g.lines[l].next_link as usize].v0;
            let p = s.g.vertices[target as usize].pos;
            let d = [2.0, -2.0, 1.999_999_9, 2.000_000_2][s.rng.below(4) as usize];
            let (x, y) = if s.rng.chance(2) {
                (p.x + d, p.y)
            } else {
                (p.x, p.y + d)
            };
            let v = i32::from(s.g.lines[l].v1);
            fns::mpVtxSetPos(ctx, v, arg(x), arg(y));
            s.g.set_vertex_pos(v, x, y);
            "mpVtxSetPos"
        }
        31 if joints > 1 => {
            // Puts a vertex of one joint exactly 2 from one of another's, where linking stops,
            // then links the two.
            let other = ((j as u64 + 1 + s.rng.below(joints - 1)) % joints) as usize;
            let (a, b) = (s.g.joints[j].desc, s.g.joints[other].desc);
            if a.vtx_count <= 0 || b.vtx_count <= 0 {
                return "skipped";
            }
            let va = i32::from(a.vtx_start) + s.rng.below(a.vtx_count as u64) as i32;
            let vb = i32::from(b.vtx_start) + s.rng.below(b.vtx_count as u64) as i32;
            let p = s.g.vertices[vb as usize].pos;
            let d = [2.0, -2.0, 1.999_999_9, 0.0][s.rng.below(4) as usize];
            let e = s.rng.range(-1.5, 1.5);
            let (x, y) = if s.rng.chance(2) {
                (p.x + d, p.y + e)
            } else {
                (p.x + e, p.y + d)
            };
            fns::mpVtxSetPos(ctx, va, arg(x), arg(y));
            s.g.set_vertex_pos(va, x, y);
            fns::mpLib_800581DC(ctx, ji, other as i32);
            s.g.link_joints(ji, other as i32);
            "mpLib_800581DC"
        }
        _ => "skipped",
    }
}

pub fn run(s: &mut Stage, n: &mut Counts, steps: usize) {
    let ctx = s.ctx;
    build_tree(s);
    for k in 0..JOBJ_COUNT {
        let m = random_matrix(s);
        write_matrix(ctx, jobj(k), &m);
    }
    // Stage setup links the joints where they meet.
    fns::mpLib_80058560(ctx);
    s.g.link_all_joints();
    n.add("mpLib_80058560");
    state::check(s, "after linking the joints");
    for step in 0..steps {
        super::trace(&format!("step {step}"));
        let name = mutate(s);
        super::trace(name);
        if name == "skipped" {
            continue;
        }
        n.add(name);
        state::check(s, &format!("after step {step}, {name}"));
        if step % 30 == 29 {
            queries::run(s, n, 15);
            queries::island_queries(s, n);
        }
    }
    queries::run(s, n, 30);
}
