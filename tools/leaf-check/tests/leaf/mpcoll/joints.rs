//! Fighter ECBs fitted around joints: a stage's own model, loaded on both sides, stands in
//! for a fighter's skeleton (`lb_8000B1CC` only reads joints' transforms and matrices).

use leaf_check::{arg, handle};
use melee_data::archive::{Archive, ArchiveId, DataRef};
use melee_sim::hsd::jobj::{self, JObjId};
use melee_sim::vec::Vec3;
use ssbm_rt::Handle;
use ssbm_types::fns;
use ssbm_types::records::{CollData as CColl, HSD_JObj};

use super::cases::{case, filtered_frame, frame};
use super::place::{self as place, Writer, off};
use super::{COLL, OUT_VEC, World};

/// A model loaded on both sides: its joints paired in pre-order.
pub struct Tree {
    pub root: (u32, Option<JObjId>),
    pub joints: Vec<(u32, JObjId)>,
}

const INSTANCE: u32 = 1 << 12;

/// The first of the stage's models with enough joints, loaded on both sides.
pub fn load(w: &mut World, a: &Archive, id: ArchiveId, base: u32) -> Option<Tree> {
    let ctx = w.ctx();
    let head = a.public("map_head")?;
    let groups = a.ptr(head + 8)?;
    for g in 0..a.u32(head + 0xC) {
        let Some(joint) = a.ptr(groups + g * 0x34) else {
            continue;
        };
        let theirs = fns::HSD_JObjLoadJoint(ctx, handle(ctx, base + joint)).addr();
        let ours = jobj::load_joint(
            &mut w.objs,
            &w.archives,
            Some(DataRef {
                archive: id,
                offset: joint,
            }),
        );
        let t = preorder_theirs(ctx, theirs);
        let o = preorder_ours(w, ours);
        assert_eq!(t.len(), o.len(), "{}: model {g}'s joints", w.st.what());
        let tree = Tree {
            root: (theirs, ours),
            joints: t.into_iter().zip(o).collect(),
        };
        if tree.joints.len() >= 7 {
            for &(a, j) in &tree.joints {
                w.handles.jobjs.insert(a, j.0);
                w.handles.jobjs_back.insert(j.0, a);
            }
            return Some(tree);
        }
        unload(w, &tree);
    }
    None
}

pub fn unload(w: &mut World, t: &Tree) {
    let ctx = w.ctx();
    fns::HSD_JObjUnref(ctx, handle(ctx, t.root.0));
    jobj::unref(&mut w.objs, t.root.1);
    w.handles.jobjs.clear();
    w.handles.jobjs_back.clear();
}

fn preorder_theirs(ctx: &ssbm_rt::Ctx, root: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(j) = stack.pop() {
        if j == 0 {
            continue;
        }
        out.push(j);
        stack.push(ctx.read_u32(j + 8));
        if ctx.read_u32(j + 0x14) & INSTANCE == 0 {
            stack.push(ctx.read_u32(j + 0x10));
        }
    }
    out
}

fn preorder_ours(w: &World, root: Option<JObjId>) -> Vec<JObjId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(j) = stack.pop() {
        let Some(j) = j else { continue };
        out.push(j);
        let o = &w.objs.jobjs[j];
        stack.push(o.next);
        if o.flags & INSTANCE == 0 {
            stack.push(o.child);
        }
    }
    out
}

/// Every joint's flags and matrix, the same on both sides.
fn check_joints(w: &World, t: &Tree, what: &str) {
    let ctx = w.ctx();
    for &(a, j) in &t.joints {
        let o = &w.objs.jobjs[j];
        assert_eq!(
            o.flags,
            ctx.read_u32(a + 0x14),
            "{what}: joint {a:#x} flags"
        );
        let theirs: Vec<u32> = (0..12).map(|k| ctx.read_u32(a + 0x44 + 4 * k)).collect();
        let ours: Vec<u32> = o.mtx.m.as_flattened().iter().map(|v| v.to_bits()).collect();
        assert_eq!(ours, theirs, "{what}: joint {a:#x} matrix");
    }
}

/// Brings every joint's matrix up to date on both sides.
fn setup_all(w: &mut World, t: &Tree) {
    let ctx = w.ctx();
    for &(a, j) in &t.joints {
        let flags = ctx.read_u32(a + 0x14);
        // `HSD_JObjMtxIsDirty`: dirty and not user-defined.
        if flags & jobj::USER_DEF_MTX == 0 && flags & jobj::MTX_DIRTY != 0 {
            fns::HSD_JObjSetupMatrixSub(ctx, handle(ctx, a));
        }
        jobj::setup_matrix(&mut w.objs, &w.archives, Some(j));
    }
}

/// Moves a few joints on both sides, their matrices to be rebuilt.
fn pose(w: &mut World, t: &Tree) {
    let ctx = w.ctx();
    for _ in 0..3 {
        let (a, j) = t.joints[w.st.rng.below(t.joints.len() as u64) as usize];
        let v = Vec3::new(
            w.st.rng.range(-20.0, 20.0),
            w.st.rng.range(-20.0, 20.0),
            w.st.rng.range(-5.0, 5.0),
        );
        leaf_check::put_vec(ctx, a + 0x38, v);
        w.objs.jobjs[j].translate = v;
        fns::HSD_JObjSetMtxDirtySub(ctx, handle(ctx, a));
        jobj::set_mtx_dirty_sub(&mut w.objs.jobjs, j);
    }
}

/// Seven of the model's joints: the source's root and the six it's fitted around.
fn pick(w: &mut World, t: &Tree) -> [(u32, JObjId); 7] {
    std::array::from_fn(|_| t.joints[w.st.rng.below(t.joints.len() as u64) as usize])
}

/// Writes a joint source into the collision data in ssbm-rs's memory.
fn write_source(w: &mut World, js: &[(u32, JObjId); 7]) {
    let wr = Writer {
        ctx: w.ctx(),
        base: COLL,
    };
    wr.i32(off::SOURCE, 1);
    for (i, (a, _)) in js.iter().enumerate() {
        wr.u32(off::SOURCE + 4 + 4 * i as u32, *a);
    }
    let x124 = w.st.rng.range(-2.0, 2.0);
    let (h, wd) = match w.st.rng.below(3) {
        0 => (10.0, 10.0),
        _ => (w.st.rng.range(0.0, 14.0), w.st.rng.range(0.0, 14.0)),
    };
    wr.fs(off::SOURCE + 0x20, &[x124, h, wd]);
}

/// A round with joint sources: setting one, loading it alone and in the frames, and
/// `lb_8000B1CC` on its own.
pub fn round(w: &mut World, t: &Tree) {
    pose(w, t);
    let ctx = w.ctx();

    place::place(w, COLL);
    let js = pick(w, t);
    let x124 = w.st.rng.range(-2.0, 2.0);
    let gobj = if w.st.rng.chance(2) { 0x8170_00A0 } else { 0 };
    case(
        w,
        "mpColl_SetECBSource_JObj",
        |ctx| {
            let h = |a: u32| handle::<HSD_JObj>(ctx, a);
            fns::mpColl_SetECBSource_JObj(
                ctx,
                handle::<CColl>(ctx, COLL),
                handle(ctx, gobj),
                h(js[0].0),
                h(js[1].0),
                h(js[2].0),
                h(js[3].0),
                h(js[4].0),
                h(js[5].0),
                h(js[6].0),
                arg(x124),
            );
            0
        },
        |_, cd| {
            let j: [Option<JObjId>; 7] = std::array::from_fn(|i| Some(js[i].1));
            cd.set_ecb_source_joints(
                (gobj != 0).then_some(melee_sim::hsd::gobj::GObjId(gobj)),
                j[0],
                [j[1], j[2], j[3], j[4], j[5], j[6]],
                x124,
            );
            0
        },
    );
    check_joints(w, t, "after setting a joint source");

    for _ in 0..4 {
        place::place(w, COLL);
        let js = pick(w, t);
        write_source(w, &js);
        let flags = w.st.rng.below(0x20) as u32;
        case(
            w,
            "mpColl_LoadECB_JObj",
            |ctx| {
                fns::mpColl_LoadECB_JObj(ctx, handle(ctx, COLL), flags);
                0
            },
            |c, cd| {
                c.load_ecb_joints(cd, flags);
                0
            },
        );
        check_joints(w, t, "mpColl_LoadECB_JObj");
        pose(w, t);
    }

    place::place(w, COLL);
    let js = pick(w, t);
    write_source(w, &js);
    case(
        w,
        "mpColl_LoadECB",
        |ctx| {
            fns::mpColl_LoadECB(ctx, handle(ctx, COLL));
            0
        },
        |c, cd| {
            c.load_ecb(cd, 6);
            0
        },
    );

    // Frames loading joint sources.
    for _ in 0..6 {
        place::place(w, COLL);
        let js = pick(w, t);
        write_source(w, &js);
        if w.st.rng.chance(3) {
            let i = w.st.rng.below(7) as usize;
            filtered_frame(w, i);
        } else {
            let i = w.st.rng.below(17) as usize;
            frame(w, i);
        }
        if super::result_hung() {
            // Ours set up the joints it read before giving up; bring both sides level.
            setup_all(w, t);
        }
        check_joints(w, t, "a frame");
        pose(w, t);
    }

    // The world position itself, with and without an offset.
    for _ in 0..6 {
        let (a, j) = t.joints[w.st.rng.below(t.joints.len() as u64) as usize];
        let offset = match w.st.rng.below(4) {
            0 => None,
            1 => Some(Vec3::new(0.0, -0.0, 0.0)),
            _ => Some(Vec3::new(
                w.st.rng.range(-5.0, 5.0),
                w.st.rng.range(-5.0, 5.0),
                w.st.rng.range(-5.0, 5.0),
            )),
        };
        let pos0 = SCRATCH_POS;
        if let Some(o) = offset {
            leaf_check::put_vec(ctx, pos0, o);
        }
        fns::lb_8000B1CC(
            ctx,
            handle(ctx, a),
            handle(ctx, if offset.is_some() { pos0 } else { 0 }),
            handle(ctx, OUT_VEC),
        );
        let ours = melee_sim::lb::joint::world_position(&mut w.objs, &w.archives, Some(j), offset);
        let theirs: Vec<u32> = (0..3).map(|k| ctx.read_u32(OUT_VEC + 4 * k)).collect();
        assert_eq!(
            vec![ours.x.to_bits(), ours.y.to_bits(), ours.z.to_bits()],
            theirs,
            "{}: lb_8000B1CC",
            w.st.what()
        );
        check_joints(w, t, "lb_8000B1CC");
        w.count("lb_8000B1CC");
        pose(w, t);
    }
}

/// An offset for `lb_8000B1CC`, clear of the other scratch.
const SCRATCH_POS: u32 = leaf_check::SCRATCH + 0x5600;
