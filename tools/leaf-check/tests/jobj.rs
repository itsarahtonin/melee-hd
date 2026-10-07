//! Joints (JObj), their references (RObj) and display-object lists against ssbm-rs: every
//! joint tree the disc's archives name, with the animation the archive pairs it with, loaded,
//! animated frame by frame, its matrices set up, and edited at random (flags, SRT values and
//! every update channel, requests, tree surgery, removal), comparing every joint, reference,
//! animation and display-object list after each step.

use std::collections::HashMap;

use gekko_fp::{FpMode, set_fp_mode};
use leaf_check::{FILE_MAX, Rand, handle, hsd_reference, load_archive, stage_anim_roots};
use melee_data::archive::{Archive, ArchiveId, Archives, DataRef};
use melee_data::disc::Disc;
use melee_sim::hsd::Objects;
use melee_sim::hsd::aobj::{self, AObjId};
use melee_sim::hsd::fobj::{FObjId, Value};
use melee_sim::hsd::jobj::{self, Contents, JObjId};
use melee_sim::hsd::pobj::{self, PObjId};
use melee_sim::hsd::robj::{self, RObjId};
use melee_sim::vec::Vec3;
use ssbm_rt::{Addr, Ctx, Handle};
use ssbm_types::fns;
use ssbm_types::records::{HSD_JObj, HSD_ObjData};

const MODES: [FpMode; 2] = [FpMode::Hardware, FpMode::Slippi];
/// The tree walk's callback, registered at a free address.
const WALK_CB: u32 = 0x817E_0400;

thread_local! {
    static WALK: std::cell::RefCell<Vec<(u32, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn native_walk(ctx: &Ctx) {
    let (j, ty) = (ctx.regs.r(3), ctx.regs.r(5));
    WALK.with(|w| w.borrow_mut().push((j, ty)));
}

/// Where the update value handed to `JObjUpdateFunc` goes.
const VALUE: u32 = 0x817E_0000;

/// The trees an archive names: each joint, and the AnimJoint the archive pairs with it.
fn trees(a: &Archive) -> Vec<(u32, Option<u32>)> {
    let mut out = Vec::new();
    if let Some(head) = a.public("map_head")
        && let Some(groups) = a.ptr(head + 8)
    {
        for g in 0..a.u32(head + 0xC) {
            let group = groups + g * 0x34;
            if let Some(joint) = a.ptr(group) {
                let anim = a.ptr(group + 4).and_then(|list| a.ptr(list));
                out.push((joint, anim));
            }
        }
    }
    let _ = stage_anim_roots;
    for (name, joint) in a.publics() {
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with("_joint") || lower.contains("anim") {
            continue;
        }
        let anim = a.public(&format!(
            "{}_animjoint",
            &name[..name.len() - "_joint".len()]
        ));
        out.push((joint, anim));
    }
    // Some AnimJoints hold -1 where a link would be (Mute City's), which the original
    // follows and faults on: the game never pairs those with these trees.
    for (_, anim) in &mut out {
        if anim.is_some_and(|aj| !links_valid(a, aj)) {
            *anim = None;
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Whether every child and next link of an AnimJoint tree is null or a pointer.
fn links_valid(a: &Archive, root: u32) -> bool {
    let mut stack = vec![root];
    while let Some(aj) = stack.pop() {
        for at in [aj, aj + 4] {
            if a.u32(at) != 0 && !a.is_pointer(at) {
                return false;
            }
            stack.extend(a.ptr(at));
        }
    }
    true
}

struct Sides<'c> {
    ctx: &'c Ctx,
    base: u32,
    id: ArchiveId,
    archives: Archives,
    objs: Objects,
}

impl Sides<'_> {
    fn at(&self, offset: u32) -> DataRef {
        DataRef {
            archive: self.id,
            offset,
        }
    }

    fn data_offset(&self, addr: u32) -> Option<u32> {
        (addr != 0).then(|| addr.wrapping_sub(self.base))
    }

    /// A tree in pre-order (an instance's child is the tree it shares, not walked).
    fn theirs_tree(&self, root: u32) -> Vec<u32> {
        let ctx = self.ctx;
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(j) = stack.pop() {
            if j == 0 {
                continue;
            }
            out.push(j);
            stack.push(ctx.read_u32(j + 8));
            if ctx.read_u32(j + 0x14) & jobj::INSTANCE == 0 {
                stack.push(ctx.read_u32(j + 0x10));
            }
        }
        out
    }

    fn ours_tree(&self, root: Option<JObjId>) -> Vec<JObjId> {
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(j) = stack.pop() {
            let Some(j) = j else { continue };
            out.push(j);
            let o = &self.objs.jobjs[j];
            stack.push(o.next);
            if o.flags & jobj::INSTANCE == 0 {
                stack.push(o.child);
            }
        }
        out
    }

    fn floats(&self, addr: u32, n: u32) -> Vec<u32> {
        (0..n).map(|i| self.ctx.read_u32(addr + 4 * i)).collect()
    }

    fn fobj_theirs(&self, a: u32) -> String {
        let ctx = self.ctx;
        let f = |o| ctx.read_u32(a + o);
        let ad = match f(4) {
            0 => 0,
            p => p.wrapping_sub(self.base),
        };
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            ad,
            f(8).wrapping_sub(self.base),
            f(0xC),
            ctx.read_u8(a + 0x10),
            ctx.read_u8(a + 0x11),
            ctx.read_u8(a + 0x12),
            ctx.read_u8(a + 0x13),
            ctx.read_u8(a + 0x14),
            ctx.read_u8(a + 0x15),
            ctx.read_u16(a + 0x16),
            ctx.read_u16(a + 0x18) as i16,
            ctx.read_u16(a + 0x1A),
            f(0x1C),
            f(0x20),
            f(0x24),
            f(0x28),
            f(0x2C),
        )
    }

    fn fobj_ours(&self, id: FObjId) -> String {
        let o = &self.objs.fobjs[id.0];
        format!(
            "ad {:#x} head {:#x} len {} flags {:#x} op {} intrp {} type {} fv {} fs {} pack {} start {} fterm {} time {:#x} p0 {:#x} p1 {:#x} d0 {:#x} d1 {:#x}",
            o.ad,
            o.ad_head,
            o.length,
            o.flags,
            o.op,
            o.op_intrp,
            o.obj_type,
            o.frac_value,
            o.frac_slope,
            o.nb_pack,
            o.startframe,
            o.fterm,
            o.time.to_bits(),
            o.p0.to_bits(),
            o.p1.to_bits(),
            o.d0.to_bits(),
            o.d1.to_bits(),
        )
    }

    /// The joint descriptor a joint was loaded from, which identifies it across sides.
    fn theirs_id(&self, j: u32) -> Option<u32> {
        (j != 0).then(|| self.ctx.read_u32(j + 0x84).wrapping_sub(self.base))
    }

    fn ours_id(&self, j: Option<JObjId>) -> Option<u32> {
        j.map(|j| self.objs.jobjs[j].id.expect("joint without an ID").offset)
    }

    fn compare_aobj(&self, t: u32, o: Option<AObjId>, what: &str) {
        let ctx = self.ctx;
        let Some(o) = o else {
            assert_eq!(t, 0, "{what}: theirs has an AObj, ours none");
            return;
        };
        assert_ne!(t, 0, "{what}: ours has an AObj, theirs none");
        let a = &self.objs.aobjs[o];
        assert_eq!(
            (
                ctx.read_u32(t),
                ctx.read_u32(t + 4),
                ctx.read_u32(t + 8),
                ctx.read_u32(t + 0xC),
                ctx.read_u32(t + 0x10),
            ),
            (
                a.flags,
                a.curr_frame.to_bits(),
                a.rewind_frame.to_bits(),
                a.end_frame.to_bits(),
                a.framerate.to_bits(),
            ),
            "{what}: AObj"
        );
        assert_eq!(
            self.theirs_id(ctx.read_u32(t + 0x18)),
            self.ours_id(a.hsd_obj),
            "{what}: AObj's bound joint"
        );
        let (mut tf, mut of) = (ctx.read_u32(t + 0x14), a.fobj);
        let mut i = 0;
        while let Some(f) = of {
            assert_ne!(tf, 0, "{what}: FObj chains' lengths");
            assert_eq!(self.fobj_ours(f), self.fobj_theirs(tf), "{what}: FObj {i}");
            tf = ctx.read_u32(tf);
            of = self.objs.fobjs[f.0].next;
            i += 1;
        }
        assert_eq!(tf, 0, "{what}: FObj chains' lengths");
    }

    fn compare_robjs(&self, t: u32, o: Option<RObjId>, what: &str) {
        let ctx = self.ctx;
        let (mut t, mut o) = (t, o);
        let mut i = 0;
        while let Some(r) = o {
            assert_ne!(t, 0, "{what}: RObj lists' lengths");
            let ro = &self.objs.robjs[r];
            let what = format!("{what}: RObj {i}");
            assert_eq!(ctx.read_u32(t + 4), ro.flags, "{what}: flags");
            match ro.u {
                robj::Ref::None => assert_eq!(ctx.read_u32(t + 8), 0, "{what}: target"),
                robj::Ref::JObj(j) => assert_eq!(
                    self.theirs_id(ctx.read_u32(t + 8)),
                    self.ours_id(Some(j)),
                    "{what}: target"
                ),
                robj::Ref::Limit(l) => assert_eq!(ctx.read_u32(t + 8), l.to_bits(), "{what}"),
                robj::Ref::IkHint {
                    bone_length,
                    rotate_x,
                } => assert_eq!(
                    self.floats(t + 8, 2),
                    vec![bone_length.to_bits(), rotate_x.to_bits()],
                    "{what}: IK hint"
                ),
                robj::Ref::Exp(e) => {
                    // A C expression's function: the original's `dummy_func` for ours' none.
                    let expr = ctx.read_u32(t + 8);
                    let expr = if !e.is_bytecode && expr == leaf_check::sym("dummy_func") {
                        None
                    } else {
                        self.data_offset(expr)
                    };
                    assert_eq!(
                        (expr, ctx.read_u32(t + 0x10), ctx.read_u8(t + 0x14) != 0,),
                        (e.bytecode.map(|b| b.offset), e.nb_args, e.is_bytecode),
                        "{what}: expression"
                    );
                    let (mut tv, mut ov) = (ctx.read_u32(t + 0xC), e.rvalue);
                    while let Some(v) = ov {
                        assert_ne!(tv, 0, "{what}: rvalue lists' lengths");
                        let rv = &self.objs.rvalues[v];
                        assert_eq!(
                            (ctx.read_u32(tv + 4), self.theirs_id(ctx.read_u32(tv + 8))),
                            (rv.flags, self.ours_id(rv.jobj)),
                            "{what}: rvalue"
                        );
                        tv = ctx.read_u32(tv);
                        ov = rv.next;
                    }
                    assert_eq!(tv, 0, "{what}: rvalue lists' lengths");
                }
            }
            self.compare_aobj(ctx.read_u32(t + 0x18), ro.aobj, &format!("{what}: aobj"));
            t = ctx.read_u32(t);
            o = ro.next;
            i += 1;
        }
        assert_eq!(t, 0, "{what}: RObj lists' lengths");
    }

    fn compare_pobjs(&self, t: u32, o: Option<PObjId>, what: &str) {
        let ctx = self.ctx;
        let (mut t, mut o) = (t, o);
        while let Some(p) = o {
            assert_ne!(t, 0, "{what}: PObj lists' lengths");
            let po = &self.objs.pobjs[p];
            assert_eq!(
                fns::HSD_PObjGetFlags(ctx, handle(ctx, t)),
                pobj::get_flags(&self.objs.pobjs, Some(p)),
                "{what}: HSD_PObjGetFlags"
            );
            assert_eq!(
                (
                    self.data_offset(ctx.read_u32(t + 8)),
                    ctx.read_u16(t + 0xC),
                    ctx.read_u16(t + 0xE),
                    self.data_offset(ctx.read_u32(t + 0x10)),
                ),
                (
                    po.verts.map(|v| v.offset),
                    po.flags,
                    po.n_display,
                    po.display.map(|d| d.offset)
                ),
                "{what}: PObj"
            );
            let u = ctx.read_u32(t + 0x14);
            match &po.u {
                pobj::Binding::Skin(j) => {
                    assert_eq!(self.theirs_id(u), self.ours_id(*j), "{what}: skin joint")
                }
                pobj::Binding::Envelopes(envs) => {
                    let mut list = u;
                    for env in envs {
                        assert_ne!(list, 0, "{what}: envelope lists' lengths");
                        let mut e = ctx.read_u32(list + 4);
                        for w in env {
                            assert_ne!(e, 0, "{what}: envelope lengths");
                            assert_eq!(
                                (self.theirs_id(ctx.read_u32(e + 4)), ctx.read_u32(e + 8)),
                                (self.ours_id(w.jobj), w.weight.to_bits()),
                                "{what}: envelope"
                            );
                            e = ctx.read_u32(e);
                        }
                        assert_eq!(e, 0, "{what}: envelope lengths");
                        list = ctx.read_u32(list);
                    }
                    assert_eq!(list, 0, "{what}: envelope lists' lengths");
                }
                pobj::Binding::ShapeSet(set) => {
                    assert_eq!(
                        (
                            ctx.read_u16(u),
                            ctx.read_u16(u + 2),
                            ctx.read_u32(u + 4) as i32
                        ),
                        (set.flags, set.nb_shape, set.nb_vertex_index),
                        "{what}: shape set"
                    );
                    self.compare_aobj(
                        ctx.read_u32(u + 0x20),
                        set.aobj,
                        &format!("{what}: shape aobj"),
                    );
                }
            }
            t = ctx.read_u32(t + 4);
            o = po.next;
        }
        assert_eq!(t, 0, "{what}: PObj lists' lengths");
    }

    /// Compares two trees joint by joint.
    fn compare(&self, t_root: u32, o_root: Option<JObjId>, what: &str) {
        let ctx = self.ctx;
        let tt = self.theirs_tree(t_root);
        let ot = self.ours_tree(o_root);
        assert_eq!(tt.len(), ot.len(), "{what}: tree sizes");
        let map: HashMap<u32, JObjId> = tt.iter().copied().zip(ot.iter().copied()).collect();
        let link = |t: u32, o: Option<JObjId>, field: &str, at: usize| {
            match (t, o) {
                (0, None) => {}
                (t, Some(o)) if map.get(&t) == Some(&o) => {}
                // A link out of the tree (an instance's shared tree, a removed joint's old
                // parent) matches by descriptor.
                (t, o) if t != 0 && o.is_some() && !map.contains_key(&t) => assert_eq!(
                    self.theirs_id(t),
                    self.ours_id(o),
                    "{what}: joint {at} {field} (outside)"
                ),
                _ => panic!("{what}: joint {at} {field}: theirs {t:#x}, ours {o:?}"),
            }
        };
        for (i, (&t, &o)) in tt.iter().zip(&ot).enumerate() {
            let j = &self.objs.jobjs[o];
            let what = format!("{what}: joint {i}");
            link(ctx.read_u32(t + 8), j.next, "next", i);
            link(ctx.read_u32(t + 0xC), j.parent, "parent", i);
            link(ctx.read_u32(t + 0x10), j.child, "child", i);
            assert_eq!(
                (
                    ctx.read_u16(t + 4),
                    ctx.read_u16(t + 6),
                    ctx.read_u32(t + 0x14)
                ),
                (j.ref_count, j.ref_count_individual, j.flags),
                "{what}: ref counts and flags"
            );
            let q = &j.rotate;
            let ours_srt = [q.x, q.y, q.z, q.w, j.scale.x, j.scale.y, j.scale.z]
                .into_iter()
                .chain([j.translate.x, j.translate.y, j.translate.z])
                .map(f32::to_bits)
                .collect::<Vec<_>>();
            assert_eq!(self.floats(t + 0x1C, 10), ours_srt, "{what}: SRT");
            let m: Vec<u32> = j.mtx.m.iter().flatten().map(|v| v.to_bits()).collect();
            let robjs: Vec<(u32, robj::Ref)> =
                std::iter::successors(j.robj, |&r| self.objs.robjs[r].next)
                    .map(|r| (self.objs.robjs[r].flags, self.objs.robjs[r].u))
                    .collect();
            assert_eq!(
                self.floats(t + 0x44, 12),
                m,
                "{what}: matrix (flags {:#x}, rotate {:?}, scale {:?}, translate {:?}, robjs {robjs:x?})",
                j.flags,
                j.rotate,
                j.scale,
                j.translate
            );
            let scl = ctx.read_u32(t + 0x74);
            assert_eq!(
                (scl != 0).then(|| self.floats(scl, 3)),
                j.scl
                    .map(|v| vec![v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]),
                "{what}: scl"
            );
            let env = ctx.read_u32(t + 0x78);
            assert_eq!(
                (env != 0).then(|| self.floats(env, 12)),
                j.envelopemtx
                    .map(|e| e.m.iter().flatten().map(|v| v.to_bits()).collect()),
                "{what}: envelope matrix"
            );
            assert_eq!(self.theirs_id(t), self.ours_id(Some(o)), "{what}: id");
            let u = ctx.read_u32(t + 0x18);
            match j.u {
                Contents::Spline(s) | Contents::Particles(s) => assert_eq!(
                    self.data_offset(u),
                    s.map(|s| s.offset),
                    "{what}: spline or particles"
                ),
                Contents::DObjs(first) => {
                    let (mut td, mut od) = (u, first);
                    while let Some(d) = od {
                        assert_ne!(td, 0, "{what}: DObj lists' lengths");
                        assert_eq!(
                            ctx.read_u32(td + 0x14),
                            self.objs.dobjs[d].flags,
                            "{what}: DObj flags"
                        );
                        self.compare_pobjs(ctx.read_u32(td + 0xC), self.objs.dobjs[d].pobj, &what);
                        td = ctx.read_u32(td + 4);
                        od = self.objs.dobjs[d].next;
                    }
                    assert_eq!(td, 0, "{what}: DObj lists' lengths");
                }
            }
            self.compare_aobj(ctx.read_u32(t + 0x7C), j.aobj, &format!("{what}: aobj"));
            self.compare_robjs(ctx.read_u32(t + 0x80), j.robj, &what);
        }
    }

    /// Sets up every dirty matrix of a tree in pre-order on both sides (the inline
    /// `HSD_JObjSetupMatrix` on each joint); false where the original halts (and ours with it).
    fn setup_all(&mut self, t_root: u32, o_root: Option<JObjId>) -> bool {
        let ctx = self.ctx;
        for (t, o) in self
            .theirs_tree(t_root)
            .into_iter()
            .zip(self.ours_tree(o_root))
        {
            let flags = ctx.read_u32(t + 0x14);
            let dirty = flags & jobj::USER_DEF_MTX == 0 && flags & jobj::MTX_DIRTY != 0;
            let (objs, archives) = (&mut self.objs, &self.archives);
            let carried_on = both(
                "setup",
                || {
                    if dirty {
                        fns::HSD_JObjSetupMatrixSub(ctx, handle(ctx, t));
                    }
                },
                || jobj::setup_matrix(objs, archives, Some(o)),
            );
            if !carried_on {
                return false;
            }
        }
        true
    }
}

/// Live joints, DObjs and PObjs (their classes' `nb_exist`) and RObjs, AObjs and FObjs (their
/// allocators' `used`).
fn live_theirs(ctx: &Ctx) -> [u32; 6] {
    let exist = |class: &str| ctx.read_u32(leaf_check::sym(class) + 0x20);
    let used = |data: &str| ctx.read_u32(leaf_check::sym(data) + 8);
    [
        exist("hsdJObj"),
        exist("hsdDObj"),
        exist("hsdPObj"),
        used("robj_alloc_data"),
        used("aobj_alloc_data"),
        used("fobj_alloc_data"),
    ]
}

fn live_ours(o: &Objects) -> [u32; 6] {
    [
        o.jobjs.pool.used(),
        o.dobjs.pool.used(),
        o.pobjs.pool.used(),
        o.robjs.pool.used(),
        o.aobjs.pool.used(),
        o.fobjs.used(),
    ]
}

/// Runs a step on both sides, the original first. Where the original halts (an assertion, or a
/// fault on data the game never pairs this way), ours must halt too; returns whether both
/// carried on.
fn both(what: &str, theirs: impl FnOnce(), ours: impl FnOnce()) -> bool {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    if catch_unwind(AssertUnwindSafe(theirs)).is_ok() {
        ours();
        return true;
    }
    assert!(
        catch_unwind(AssertUnwindSafe(ours)).is_err(),
        "{what}: the original halts here, ours carries on"
    );
    false
}

#[derive(Default)]
struct Counts {
    halted: u64,
    trees: u64,
    animated: u64,
    joints: u64,
    frames: u64,
    steps: u64,
    ik: u64,
    constraints: u64,
    ops: HashMap<&'static str, u64>,
}

#[test]
fn jobjs_match() {
    // Loading recurses along sibling chains as the original does, and Rainbow Cruise's run
    // long.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(check_all)
        .unwrap()
        .join()
        .unwrap();
}

fn check_all() {
    if hsd_reference().is_none() {
        return;
    }
    let disc = Disc::open(std::env::var("SSBM_DISC").unwrap()).unwrap();
    let mut counts = Counts::default();
    let mut rand = Rand(0x9E37_79B9_7F4A_7C15);
    for name in disc.files().unwrap().iter().filter(|f| f.ends_with(".dat")) {
        let bytes = disc.read_file(name).unwrap();
        let Ok(archive) = Archive::parse(&bytes) else {
            continue;
        };
        let trees = trees(&archive);
        if trees.is_empty() || bytes.len() > FILE_MAX {
            continue;
        }
        // A halt leaves the reference mid-call, so the rest runs on a fresh one.
        let mut from = 0;
        while let Some(halted) = check_archive(name, &bytes, &trees, from, &mut counts, &mut rand) {
            counts.halted += 1;
            from = halted + 1;
        }
    }
    report(&counts);
}

/// Checks an archive's trees from `from` on; returns the index of the tree where the original
/// halted, if one did.
fn check_archive(
    name: &str,
    bytes: &[u8],
    trees: &[(u32, Option<u32>)],
    from: usize,
    counts: &mut Counts,
    rand: &mut Rand,
) -> Option<usize> {
    let ctx = hsd_reference().unwrap();
    let ctx = &ctx;
    ctx.register(WALK_CB, native_walk);
    let archive = Archive::parse(bytes).unwrap();
    {
        let base = load_archive(ctx, bytes);
        let baseline = live_theirs(ctx);
        let mut archives = Archives::default();
        let id = archives.add(archive);
        let mut s = Sides {
            ctx,
            base,
            id,
            archives,
            objs: Objects::default(),
        };
        for (ti, &(joint, anim)) in trees.iter().enumerate().skip(from) {
            let mode = MODES[ti % 2];
            set_fp_mode(mode);
            let what = format!("{name} joint {joint:#x} in {mode:?}");
            if let Ok(only) = std::env::var("JOBJ_ONLY")
                && only != format!("{name}:{joint:#x}")
            {
                continue;
            }
            if let Ok(seed) = std::env::var("JOBJ_SEED") {
                rand.0 = u64::from_str_radix(&seed, 16).unwrap();
            }
            if std::env::var_os("JOBJ_TRACE").is_some() {
                eprintln!("{what} anim {anim:x?} seed {:x}", rand.0);
            }
            let t_root = fns::HSD_JObjLoadJoint(ctx, handle(ctx, base + joint)).addr();
            let jd = s.at(joint);
            let o_root = jobj::load_joint(&mut s.objs, &s.archives, Some(jd));
            s.compare(t_root, o_root, &format!("{what}: load"));
            counts.trees += 1;
            let size = s.ours_tree(o_root).len();
            counts.joints += size as u64;
            for &o in &s.ours_tree(o_root) {
                let f = s.objs.jobjs[o].flags;
                if f & jobj::JOINT_MASK != 0 {
                    counts.ik += 1;
                }
                if s.objs.jobjs[o].robj.is_some() {
                    counts.constraints += 1;
                }
            }
            if let Some(aj) = anim {
                let ad = s.at(aj);
                let carried_on = both(
                    &format!("{what}: add anim"),
                    || {
                        fns::HSD_JObjAddAnimAll(
                            ctx,
                            handle(ctx, t_root),
                            handle(ctx, base + aj),
                            handle(ctx, 0),
                            handle(ctx, 0),
                        )
                    },
                    || jobj::add_anim_all(&mut s.objs, &s.archives, o_root, Some(ad), None, None),
                );
                if !carried_on {
                    return Some(ti);
                }
                s.compare(t_root, o_root, &format!("{what}: add anim"));
                counts.animated += 1;
            }
            let start = [0.0f32, 0.0, 1.0, 10.0][(rand.bits() % 4) as usize];
            fns::HSD_JObjReqAnimAll(ctx, handle(ctx, t_root), start as f64);
            jobj::req_anim_all(&mut s.objs, o_root, start);
            s.compare(t_root, o_root, &format!("{what}: request at {start}"));
            let frames = if anim.is_some() { 90 } else { 8 };
            let mut halted = false;
            for frame in 0..frames {
                let carried_on = both(
                    &format!("{what}: frame {frame} anim"),
                    || fns::HSD_JObjAnimAll(ctx, handle(ctx, t_root)),
                    || jobj::anim_all(&mut s.objs, &s.archives, o_root),
                );
                if !carried_on {
                    halted = true;
                    break;
                }
                s.compare(t_root, o_root, &format!("{what}: frame {frame} anim"));
                if !rand.bits().is_multiple_of(3) {
                    if !s.setup_all(t_root, o_root) {
                        return Some(ti);
                    }
                    s.compare(t_root, o_root, &format!("{what}: frame {frame} setup"));
                }
                counts.frames += 1;
                if rand.bits().is_multiple_of(2) {
                    let op = random_op(&mut s, rand, t_root, o_root, false);
                    if std::env::var_os("JOBJ_TRACE").is_some() {
                        eprintln!("  frame {frame}: {op}");
                    }
                    s.compare(t_root, o_root, &format!("{what}: frame {frame} {op}"));
                    *counts.ops.entry(op).or_default() += 1;
                    counts.steps += 1;
                }
            }
            if halted {
                return Some(ti);
            }
            fns::HSD_JObjSetCurrent(ctx, handle(ctx, 0));
            jobj::set_current(&mut s.objs, None);
            fns::HSD_JObjUnref(ctx, handle(ctx, t_root));
            jobj::unref(&mut s.objs, o_root);
            // Whatever survives (the original leaks a joint some references outlive) must
            // be what survives there.
            assert_eq!(
                live_theirs(ctx)
                    .iter()
                    .zip(baseline)
                    .map(|(a, b)| a - b)
                    .collect::<Vec<_>>(),
                live_ours(&s.objs).to_vec(),
                "{what}: live joints, DObjs, PObjs, RObjs, AObjs and FObjs after removal"
            );
        }
    }
    None
}

fn report(counts: &Counts) {
    set_fp_mode(FpMode::Hardware);
    let mut ops: Vec<_> = counts.ops.iter().collect();
    ops.sort();
    eprintln!(
        "{} halted where the original does; {} trees ({} animated), {} joints ({} IK, {} with references), {} frames, {} edits: {ops:?}",
        counts.halted,
        counts.trees,
        counts.animated,
        counts.joints,
        counts.ik,
        counts.constraints,
        counts.frames,
        counts.steps
    );
    assert!(counts.trees >= 1000, "only {} trees", counts.trees);
}

/// One random edit on both sides; returns its name.
///
/// `tame` leaves out tree surgery and new constraints, which on trees with references can
/// build dependency cycles the game never builds (and the original recurses on forever).
fn random_op(
    s: &mut Sides,
    rand: &mut Rand,
    t_root: u32,
    o_root: Option<JObjId>,
    tame: bool,
) -> &'static str {
    let ctx = s.ctx;
    let tt = s.theirs_tree(t_root);
    let ot = s.ours_tree(o_root);
    let k = (rand.bits() as usize) % tt.len();
    let (t, o) = (tt[k], Some(ot[k]));
    let h = |a: u32| handle::<HSD_JObj>(ctx, a);
    let flag = [
        jobj::HIDDEN,
        jobj::CLASSICAL_SCALE,
        jobj::MTX_INDEP_SRT,
        jobj::MTX_INDEP_PARENT,
        jobj::USER_DEF_MTX,
        jobj::MTX_DIRTY,
    ][(rand.bits() % 6) as usize];
    match rand.bits() % 24 {
        0 => {
            fns::HSD_JObjSetFlags(ctx, h(t), flag);
            jobj::set_flags(&mut s.objs.jobjs, o, flag);
            "set flags"
        }
        1 => {
            fns::HSD_JObjClearFlags(ctx, h(t), flag);
            jobj::clear_flags(&mut s.objs.jobjs, o, flag);
            "clear flags"
        }
        2 => {
            fns::HSD_JObjSetFlagsAll(ctx, h(t), flag);
            jobj::set_flags_all(&mut s.objs.jobjs, o, flag);
            "set flags all"
        }
        3 => {
            fns::HSD_JObjClearFlagsAll(ctx, h(t), flag);
            jobj::clear_flags_all(&mut s.objs.jobjs, o, flag);
            "clear flags all"
        }
        4..=8 => {
            // An update channel with a value: SRT, visibility, matrix columns and rebuilds.
            let ty = [
                1u8, 2, 3, 5, 6, 7, 8, 9, 10, 11, 12, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38,
                0x39,
            ][(rand.bits() % 19) as usize];
            if ty <= 3 && s.objs.jobjs[o.unwrap()].flags & jobj::USE_QUATERNION != 0 {
                return "skipped quaternion rotation";
            }
            let r = |rand: &mut Rand| match rand.bits() % 6 {
                0 => 0.0,
                1 => 1.0,
                2 => -0.0004,
                3 => 0.6,
                _ => ((rand.bits() >> 40) as f32 / (1u64 << 23) as f32 - 1.0) * 4.0,
            };
            let value = if (0x32..=0x35).contains(&ty) {
                let p = Vec3::new(r(rand), r(rand), r(rand));
                for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                    ctx.write_u32(VALUE + 4 * i as u32, v.to_bits());
                }
                Value::Point(p)
            } else if ty >= 0x36 {
                Value::Empty
            } else {
                let v = r(rand);
                ctx.write_u32(VALUE, v.to_bits());
                Value::Float(v)
            };
            let data = if matches!(value, Value::Empty) {
                0
            } else {
                VALUE
            };
            fns::JObjUpdateFunc(
                ctx,
                handle::<Addr>(ctx, t),
                ty as i32,
                handle::<HSD_ObjData>(ctx, data),
            );
            jobj::update(&mut s.objs, &s.archives, o.unwrap(), ty, value);
            "update"
        }
        9 => {
            let f = [0.0f32, 3.0, 25.0][(rand.bits() % 3) as usize];
            fns::HSD_JObjReqAnimAll(ctx, h(t), f as f64);
            jobj::req_anim_all(&mut s.objs, o, f);
            "request subtree"
        }
        10 => {
            let joint = s.objs.jobjs[o.unwrap()].id.unwrap();
            fns::HSD_JObjResetRST(ctx, h(t), handle(ctx, s.base + joint.offset));
            jobj::reset_rst(&mut s.objs.jobjs, &s.archives, o, Some(joint));
            "reset SRT"
        }
        11 => {
            fns::HSD_JObjRemoveAnim(ctx, h(t));
            jobj::remove_anim(&mut s.objs, o);
            "remove anim"
        }
        12 => {
            // Moves a joint under another outside its subtree.
            // A joint an animation binds can't move under the animated joint's subtree
            // without a cycle the game never builds: trees with bindings stay as they are.
            let binds = ot.iter().any(|&j| {
                s.objs.jobjs[j]
                    .aobj
                    .is_some_and(|a| s.objs.aobjs[a].hsd_obj.is_some())
            });
            if k == 0 || binds || tame {
                return "skipped reparent";
            }
            let sub: Vec<u32> = s.theirs_tree(t);
            let candidates: Vec<usize> = (0..tt.len())
                .filter(|&i| !sub_contains(s, t, tt[i], &sub))
                .collect();
            if candidates.is_empty() {
                return "skipped reparent";
            }
            let p = candidates[(rand.bits() as usize) % candidates.len()];
            if ctx.read_u32(tt[p] + 0x14) & jobj::INSTANCE != 0 && ctx.read_u32(tt[p] + 0x10) != 0 {
                return "skipped reparent";
            }
            let tn = fns::HSD_JObjReparent(ctx, h(t), h(tt[p])).addr();
            let on = jobj::reparent(&mut s.objs.jobjs, o, Some(ot[p]));
            assert_eq!(
                s.theirs_id(tn),
                s.ours_id(on),
                "reparent's returned sibling"
            );
            "reparent"
        }
        13 => {
            let tp = fns::HSD_JObjGetPrev(ctx, h(t)).addr();
            let op = jobj::get_prev(&s.objs.jobjs, o);
            assert_eq!(s.theirs_id(tp), s.ours_id(op), "get prev");
            WALK.with(|w| w.borrow_mut().clear());
            fns::HSD_JObjWalkTree(ctx, h(t), handle(ctx, WALK_CB), handle(ctx, 0));
            let theirs: Vec<(Option<u32>, u32)> = WALK
                .with(|w| std::mem::take(&mut *w.borrow_mut()))
                .into_iter()
                .map(|(j, ty)| (s.theirs_id(j), ty))
                .collect();
            let mut ours = Vec::new();
            jobj::walk_tree(&s.objs.jobjs, o, &mut |j, ty| {
                ours.push((s.ours_id(Some(j)), ty))
            });
            assert_eq!(theirs, ours, "walk tree");
            "walk and prev"
        }
        14 => {
            fns::HSD_JObjCheckDepend(ctx, h(t));
            jobj::check_depend(&mut s.objs.jobjs, o);
            "check depend"
        }
        15 => {
            fns::HSD_JObjSetMtxDirtySub(ctx, h(t));
            jobj::set_mtx_dirty_sub(&mut s.objs.jobjs, o.unwrap());
            "set dirty"
        }
        16 | 17 => {
            // A constraint to another joint, as lb_8000C1C0 and its siblings make them:
            // position (1), direction (2), up (3) or orientation (4).
            let target = (rand.bits() as usize) % tt.len();
            if target == k || tame {
                return "skipped constraint";
            }
            let flags = 0x9000_0000 | (1 + rand.below(4) as u32);
            let tr = fns::HSD_RObjAlloc(ctx);
            fns::HSD_RObjSetFlags(ctx, tr, flags);
            fns::HSD_RObjSetConstraintObj(ctx, tr, handle::<Addr>(ctx, tt[target]));
            fns::HSD_JObjPrependRObj(ctx, h(t), tr);
            let or = robj::alloc(&mut s.objs.robjs);
            robj::set_flags(&mut s.objs.robjs, Some(or), flags);
            robj::set_constraint_obj(&mut s.objs, Some(or), ot[target]);
            jobj::prepend_robj(&mut s.objs, o, Some(or));
            "constrain"
        }
        18 => {
            // A limit, as lb_8000C420 makes them.
            let flags = 0xA000_0000 | (1 + rand.below(12) as u32);
            let limit = [0.0f32, -0.5, 0.5, 2.0, -3.0][rand.below(5) as usize];
            let tr = fns::HSD_RObjAlloc(ctx);
            fns::HSD_RObjSetFlags(ctx, tr, flags);
            ctx.write_u32(tr.addr() + 8, limit.to_bits());
            fns::HSD_JObjPrependRObj(ctx, h(t), tr);
            let or = robj::alloc(&mut s.objs.robjs);
            robj::set_flags(&mut s.objs.robjs, Some(or), flags);
            s.objs.robjs[or].u = robj::Ref::Limit(limit);
            jobj::prepend_robj(&mut s.objs, o, Some(or));
            "limit"
        }
        19 => {
            // Removes the joint's constraints, as lb_8000C390 does.
            let mut tc = ctx.read_u32(t + 0x80);
            loop {
                tc = fns::HSD_RObjGetByType(ctx, handle(ctx, tc), robj::JOBJ, 0).addr();
                if tc == 0 {
                    break;
                }
                let next = ctx.read_u32(tc);
                fns::HSD_JObjDeleteRObj(ctx, h(t), handle(ctx, tc));
                fns::HSD_RObjRemove(ctx, handle(ctx, tc));
                tc = next;
            }
            let mut oc = s.objs.jobjs[o.unwrap()].robj;
            loop {
                oc = robj::get_by_type(&s.objs.robjs, oc, robj::JOBJ, 0);
                let Some(c) = oc else { break };
                let next = s.objs.robjs[c].next;
                jobj::delete_robj(&mut s.objs, o, Some(c));
                robj::remove(&mut s.objs, Some(c));
                oc = next;
            }
            "unconstrain"
        }
        20 => {
            // Unlinks a joint with at most one child, which takes its place.
            if k == 0
                || tame
                || ctx.read_u32(t + 0x14) & jobj::INSTANCE != 0
                || ctx.read_u32(t + 0x10) != 0 && ctx.read_u32(ctx.read_u32(t + 0x10) + 8) != 0
            {
                return "skipped remove";
            }
            let tc = fns::HSD_JObjRemove(ctx, h(t)).addr();
            let oc = jobj::remove(&mut s.objs, o);
            assert_eq!(s.theirs_id(tc), s.ours_id(oc), "remove's returned child");
            "remove"
        }
        21 => {
            // Puts another joint in this one's place, with this one and its siblings under
            // it; the other joint must be outside the parent's subtree and not above it.
            let parent = ctx.read_u32(t + 0xC);
            if k == 0 || parent == 0 || tame {
                return "skipped add next";
            }
            let above = |x: u32, y: u32| {
                let mut c = y;
                while c != 0 {
                    if c == x {
                        return true;
                    }
                    c = ctx.read_u32(c + 0xC);
                }
                false
            };
            let candidates: Vec<usize> = (1..tt.len())
                .filter(|&i| !above(parent, tt[i]) && !above(tt[i], parent))
                .filter(|&i| ctx.read_u32(tt[i] + 0x14) & jobj::INSTANCE == 0)
                .collect();
            let binds = ot.iter().any(|&j| {
                s.objs.jobjs[j]
                    .aobj
                    .is_some_and(|a| s.objs.aobjs[a].hsd_obj.is_some())
            });
            if candidates.is_empty() || binds {
                return "skipped add next";
            }
            let n = candidates[(rand.bits() as usize) % candidates.len()];
            fns::HSD_JObjAddNext(ctx, h(t), h(tt[n]));
            jobj::add_next(&mut s.objs.jobjs, o, Some(ot[n]));
            "add next"
        }
        22 => {
            fns::HSD_JObjSetCurrent(ctx, h(t));
            jobj::set_current(&mut s.objs, o);
            assert_eq!(
                s.theirs_id(fns::HSD_JObjGetCurrent(ctx).addr()),
                s.ours_id(jobj::get_current(&s.objs)),
                "current joint"
            );
            "set current"
        }
        23 => {
            let flags = rand.below(0x800) as u32;
            let f = [0.0f32, 2.0, 7.0][rand.below(3) as usize];
            match rand.below(4) {
                0 => {
                    fns::HSD_JObjReqAnimByFlags(ctx, h(t), flags, f as f64);
                    jobj::req_anim_by_flags(&mut s.objs, o, flags, f);
                }
                1 => {
                    fns::HSD_JObjReqAnim(ctx, h(t), f as f64);
                    jobj::req_anim(&mut s.objs, o, f);
                }
                2 => {
                    fns::HSD_JObjRemoveAnimAllByFlags(ctx, h(t), flags);
                    jobj::remove_anim_all_by_flags(&mut s.objs, o, flags);
                }
                _ => {
                    fns::HSD_JObjRemoveAnimAll(ctx, h(t));
                    jobj::remove_anim_all(&mut s.objs, o);
                }
            }
            assert_eq!(
                fns::HSD_JObjGetFlags(ctx, h(t)),
                jobj::get_flags(&s.objs.jobjs, o),
                "get flags"
            );
            assert_eq!(
                fns::HSD_JObjGetDObj(ctx, h(t)).addr() != 0,
                jobj::get_dobj(&s.objs.jobjs, o).is_some(),
                "get dobj"
            );
            "anim by flags"
        }
        _ => unreachable!(),
    }
}

/// Whether `j` is in the subtree rooted at `root` (its pre-order list `sub`, which also holds
/// the root's later siblings).
fn sub_contains(s: &Sides, root: u32, j: u32, sub: &[u32]) -> bool {
    let ctx = s.ctx;
    // The subtree proper: the root and joints whose parent chain reaches it.
    let mut cur = j;
    while cur != 0 {
        if cur == root {
            return true;
        }
        cur = ctx.read_u32(cur + 0xC);
    }
    let _ = sub;
    false
}

/// A DAT file under construction: data and the offsets of its pointers.
#[derive(Default)]
struct Builder {
    data: Vec<u8>,
    relocs: Vec<u32>,
}

impl Builder {
    fn alloc(&mut self, n: usize) -> u32 {
        let at = self.data.len() as u32;
        self.data.resize(self.data.len() + n.next_multiple_of(4), 0);
        at
    }

    fn u32(&mut self, at: u32, v: u32) {
        self.data[at as usize..at as usize + 4].copy_from_slice(&v.to_be_bytes());
    }

    fn f32(&mut self, at: u32, v: f32) {
        self.u32(at, v.to_bits());
    }

    fn ptr(&mut self, at: u32, target: u32) {
        self.u32(at, target);
        self.relocs.push(at);
    }

    /// An `HSD_Joint` with unit scale.
    fn joint(&mut self, flags: u32, pos: [f32; 3], rot: [f32; 3], robj: Option<u32>) -> u32 {
        let j = self.alloc(0x40);
        self.u32(j + 4, flags);
        for (i, v) in rot.into_iter().enumerate() {
            self.f32(j + 0x14 + 4 * i as u32, v);
        }
        for i in 0..3 {
            self.f32(j + 0x20 + 4 * i, 1.0);
        }
        for (i, v) in pos.into_iter().enumerate() {
            self.f32(j + 0x2C + 4 * i as u32, v);
        }
        if let Some(r) = robj {
            self.ptr(j + 0x3C, r);
        }
        j
    }

    fn link(&mut self, parent: u32, children: &[u32]) {
        if let Some(&first) = children.first() {
            self.ptr(parent + 8, first);
        }
        for w in children.windows(2) {
            self.ptr(w[0] + 0xC, w[1]);
        }
    }

    /// A chain of `HSD_RObjDesc`s: flags and what the union holds (a pointer, or a raw word).
    fn robjs(&mut self, list: &[(u32, Result<u32, u32>)]) -> Option<u32> {
        let mut next = None;
        for &(flags, u) in list.iter().rev() {
            let r = self.alloc(0xC);
            if let Some(n) = next {
                self.ptr(r, n);
            }
            self.u32(r + 4, flags);
            match u {
                Ok(p) => self.ptr(r + 8, p),
                Err(raw) => self.u32(r + 8, raw),
            }
            next = Some(r);
        }
        next
    }

    fn ik_hint(&mut self, bone_length: f32, rotate_x: f32) -> u32 {
        let h = self.alloc(8);
        self.f32(h, bone_length);
        self.f32(h + 4, rotate_x);
        h
    }

    /// An AObjDesc, looping over `end` frames, with a constant-interpolated channel per
    /// `(type, keys)`: u8 values, each held 10 frames.
    fn aobj(&mut self, end: f32, channels: &[(u8, &[u8])]) -> u32 {
        let mut next = None;
        for &(ty, keys) in channels.iter().rev() {
            let mut bytes = vec![0x01 | ((keys.len() as u8 - 1) << 4)];
            for &k in keys {
                bytes.extend([k, 10]);
            }
            let d = self.alloc(bytes.len());
            self.data[d as usize..d as usize + bytes.len()].copy_from_slice(&bytes);
            let f = self.alloc(0x14);
            if let Some(n) = next {
                self.ptr(f, n);
            }
            self.u32(f + 4, bytes.len() as u32);
            self.data[f as usize + 0xC] = ty;
            self.data[f as usize + 0xD] = 0x80;
            self.ptr(f + 0x10, d);
            next = Some(f);
        }
        let a = self.alloc(0x10);
        self.u32(a, aobj::LOOP);
        self.f32(a + 4, end);
        if let Some(f) = next {
            self.ptr(a + 8, f);
        }
        a
    }

    /// An AnimJoint: its AObjDesc and its RObjs' AObjDescs.
    fn anim_joint(&mut self, aobj: Option<u32>, robj_anims: &[u32]) -> u32 {
        let mut list = None;
        for &a in robj_anims.iter().rev() {
            let r = self.alloc(8);
            if let Some(n) = list {
                self.ptr(r, n);
            }
            self.ptr(r + 4, a);
            list = Some(r);
        }
        let aj = self.alloc(0x14);
        if let Some(a) = aobj {
            self.ptr(aj + 8, a);
        }
        if let Some(l) = list {
            self.ptr(aj + 0xC, l);
        }
        aj
    }

    /// Links AnimJoints (child, then next) as `link` links joints.
    fn link_anim(&mut self, parent: u32, children: &[u32]) {
        if let Some(&first) = children.first() {
            self.ptr(parent, first);
        }
        for w in children.windows(2) {
            self.ptr(w[0] + 4, w[1]);
        }
    }

    /// An rvalue list ending at a null joint.
    fn rvalues(&mut self, list: &[(u32, u32)]) -> u32 {
        let l = self.alloc(8 * (list.len() + 1));
        for (i, &(flags, joint)) in list.iter().enumerate() {
            self.u32(l + 8 * i as u32, flags);
            self.ptr(l + 8 * i as u32 + 4, joint);
        }
        l
    }
}

/// Joints with every kind of reference the disc's trees lack: two-bone and one-bone IK
/// chains with hints, poles and limits; position, direction, up and orientation constraints;
/// every limit; and C and bytecode expressions, all loaded from descriptors, then edited at
/// random as `jobjs_match` edits the disc's trees.
#[test]
fn synthetic_references_match() {
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(check_synthetic)
        .unwrap()
        .join()
        .unwrap();
}

fn check_synthetic() {
    if hsd_reference().is_none() {
        return;
    }
    let mut b = Builder::default();
    let deg = |d: f32| Err(d.to_bits());
    // Targets first: joints the others refer to.
    let target = b.joint(0, [2.0, -2.0, 1.0], [0.0; 3], None);
    let pole = b.joint(0, [0.5, 1.0, 3.0], [0.0; 3], None);
    let oriented_parent = b.joint(
        jobj::CLASSICAL_SCALE,
        [1.0, 0.0, 0.0],
        [0.3, 0.2, 0.1],
        None,
    );
    let oriented = b.joint(
        jobj::CLASSICAL_SCALE,
        [0.0, 1.0, 0.0],
        [0.5, -0.4, 0.9],
        None,
    );
    b.link(oriented_parent, &[oriented]);
    let free_oriented = b.joint(0, [0.0, 0.0, 1.0], [1.1, 0.2, -0.7], None);
    // A two-bone chain aiming at the target, with a pole and limits on its second joint.
    let eff_r = b.robjs(&[(0x9000_0001, Ok(target))]);
    let effector = b.joint(jobj::EFFECTOR, [2.5, 0.0, 0.0], [0.0; 3], eff_r);
    let h2 = b.ik_hint(2.5, 0.0);
    let j2_r = b.robjs(&[
        (0xC000_0004, Ok(h2)),
        (0xA000_0005, deg(-60.0)),
        (0xA000_0006, deg(45.0)),
    ]);
    let joint2 = b.joint(jobj::JOINT2, [3.0, 0.0, 0.0], [0.0; 3], j2_r);
    b.link(joint2, &[effector]);
    let h1 = b.ik_hint(3.0, 0.2);
    let j1_r = b.robjs(&[(0xC000_0000, Ok(h1)), (0x9000_0003, Ok(pole))]);
    let joint1 = b.joint(jobj::JOINT1, [0.0, 1.0, 0.0], [0.0; 3], j1_r);
    b.link(joint1, &[joint2]);
    // A one-bone chain without a pole.
    let eff2_r = b.robjs(&[(0x9000_0001, Ok(pole))]);
    let effector2 = b.joint(jobj::EFFECTOR, [1.5, 0.0, 0.0], [0.0; 3], eff2_r);
    let h3 = b.ik_hint(1.5, 0.0);
    let single_r = b.robjs(&[(0xC000_0000, Ok(h3))]);
    let single = b.joint(jobj::JOINT1, [-1.0, 0.0, 0.0], [0.0; 3], single_r);
    b.link(single, &[effector2]);
    // Constraints and every limit.
    let mut list = vec![
        (0x9000_0001, Ok(target)),
        (0x9000_0002, Ok(target)),
        (0x9000_0003, Ok(pole)),
    ];
    for sub in 1..=12u32 {
        let v = [10.0f32, -20.0, 30.0, -5.0, 0.5, -0.25][(sub % 6) as usize];
        list.push((0xA000_0000 | sub, deg(v)));
    }
    let c_r = b.robjs(&list);
    let constrained = b.joint(0, [0.0, 0.0, -2.0], [0.4, 0.4, 0.4], c_r);
    let o_r = b.robjs(&[(0x9000_0004, Ok(oriented))]);
    let orient_a = b.joint(0, [0.0, 2.0, 0.0], [0.0; 3], o_r);
    let o2_r = b.robjs(&[(0x9000_0004, Ok(free_oriented))]);
    let orient_b = b.joint(jobj::CLASSICAL_SCALE, [0.0, -2.0, 0.0], [0.0; 3], o2_r);
    // Expressions: a C one (its function is null: 0) and bytecode adding a translation and
    // a matrix rotation (in degrees).
    let rv1 = b.rvalues(&[(0x10, target), (0x1 | 0x100000 | 0x800000, constrained)]);
    let exp = b.alloc(8);
    b.ptr(exp + 4, rv1);
    let code = b.alloc(8);
    b.data[code as usize..code as usize + 8].copy_from_slice(&[2, 0, 0, 2, 0, 2, 0x17, 1]);
    let rv2 = b.rvalues(&[(0x10 | 0x20, target), (0x10000, oriented)]);
    let bcexp = b.alloc(8);
    b.ptr(bcexp, code);
    b.ptr(bcexp + 4, rv2);
    let x_r = b.robjs(&[(0x8000_0005, Ok(exp)), (0xB000_0003, Ok(bcexp))]);
    let expressive = b.joint(0, [1.0, 1.0, 1.0], [0.0; 3], x_r);
    let root = b.joint(0, [0.0; 3], [0.0; 3], None);
    b.link(
        root,
        &[
            target,
            pole,
            oriented_parent,
            free_oriented,
            joint1,
            single,
            constrained,
            orient_a,
            orient_b,
            expressive,
        ],
    );
    // Animation: the target moves, and the constrained joint's first two constraints switch
    // on and off (RObj channels).
    let move_target = b.aobj(40.0, &[(5, &[2, 4, 1, 3]), (6, &[0, 3])]);
    let toggle = b.aobj(40.0, &[(1, &[1, 0, 1, 1])]);
    let toggle2 = b.aobj(30.0, &[(1, &[0, 1, 1])]);
    let mut kids = Vec::new();
    for i in 0..10 {
        kids.push(match i {
            0 => b.anim_joint(Some(move_target), &[]),
            6 => b.anim_joint(None, &[toggle, toggle2]),
            _ => b.anim_joint(None, &[]),
        });
    }
    let anim_root = b.anim_joint(None, &[]);
    b.link_anim(anim_root, &kids);
    let file = leaf_check::dat(&b.data, &b.relocs);
    let mut rand = Rand(0xD1B5_4A32_D192_ED03);
    let mut counts = Counts::default();
    for round in 0..40 {
        // Each round on a fresh reference, since a halt leaves one mid-call.
        let ctx = hsd_reference().unwrap();
        let ctx = &ctx;
        ctx.register(WALK_CB, native_walk);
        let base = load_archive(ctx, &file);
        let mut archives = Archives::default();
        let id = archives.add(Archive::parse(&file).unwrap());
        let mut s = Sides {
            ctx,
            base,
            id,
            archives,
            objs: Objects::default(),
        };
        let mode = MODES[round % 2];
        set_fp_mode(mode);
        let what = format!("synthetic round {round} in {mode:?}");
        let t_root = fns::HSD_JObjLoadJoint(ctx, handle(ctx, base + root)).addr();
        let jd = s.at(root);
        let o_root = jobj::load_joint(&mut s.objs, &s.archives, Some(jd));
        s.compare(t_root, o_root, &format!("{what}: load"));
        fns::HSD_JObjAddAnimAll(
            ctx,
            handle(ctx, t_root),
            handle(ctx, base + anim_root),
            handle(ctx, 0),
            handle(ctx, 0),
        );
        let ad = s.at(anim_root);
        jobj::add_anim_all(&mut s.objs, &s.archives, o_root, Some(ad), None, None);
        fns::HSD_JObjReqAnimAll(ctx, handle(ctx, t_root), 0.0);
        jobj::req_anim_all(&mut s.objs, o_root, 0.0);
        s.compare(t_root, o_root, &format!("{what}: anim"));
        let mut halted = false;
        for step in 0..150 {
            fns::HSD_JObjAnimAll(ctx, handle(ctx, t_root));
            jobj::anim_all(&mut s.objs, &s.archives, o_root);
            s.compare(t_root, o_root, &format!("{what}: step {step} anim"));
            if !s.setup_all(t_root, o_root) {
                halted = true;
                break;
            }
            s.compare(t_root, o_root, &format!("{what}: step {step} setup"));
            let op = random_op(&mut s, &mut rand, t_root, o_root, true);
            s.compare(t_root, o_root, &format!("{what}: step {step} {op}"));
            *counts.ops.entry(op).or_default() += 1;
        }
        assert_eq!(s.objs.stale_reads, 0, "{what}: stale reads");
        if halted {
            counts.halted += 1;
            continue;
        }
        fns::HSD_JObjSetCurrent(ctx, handle(ctx, 0));
        jobj::set_current(&mut s.objs, None);
        fns::HSD_JObjUnref(ctx, handle(ctx, t_root));
        jobj::unref(&mut s.objs, o_root);
    }
    set_fp_mode(FpMode::Hardware);
    eprintln!(
        "synthetic: {} rounds halted where the original does",
        counts.halted
    );
    let mut ops: Vec<_> = counts.ops.iter().collect();
    ops.sort();
    eprintln!("synthetic: {ops:?}");
}
