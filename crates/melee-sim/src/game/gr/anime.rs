//! Stage animation, `melee/gr/granime.c`: attaching a stage archive's animations to stage
//! models, and setting rates, loops and frames on the animations of a model's parts.

use melee_data::archive::{Archives, DataRef};

use super::super::state::Game;
use super::datfiles;
use super::ground::{ground_of, jobj_of, nth_joint};
use crate::fpu::copy;
use crate::hsd::Objects;
use crate::hsd::aobj::{self, AObjId};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::{pobj, tobj};

/// `HSD_TypeMask` bits: which parts' animations a walk visits.
pub const DOBJ_MASK: u32 = 1 << 2;
pub const JOBJ_MASK: u32 = 1 << 5;
pub const MOBJ_MASK: u32 = 1 << 7;
pub const POBJ_MASK: u32 = 1 << 8;
pub const ROBJ_MASK: u32 = 1 << 9;
pub const TOBJ_MASK: u32 = 1 << 10;

/// The masks granime's functions take apart from `arg2`'s bits: joints and references,
/// materials and textures, shapes.
fn part_mask(parts: u32) -> u32 {
    let mut m = 0;
    if parts & 1 != 0 {
        m |= JOBJ_MASK | ROBJ_MASK;
    }
    if parts & 2 != 0 {
        m |= 0x7484;
    }
    if parts & 4 != 0 {
        m |= POBJ_MASK;
    }
    m
}

/// The animations of a joint's parts in the order `HSD_ForeachAnim` and granime's own walk
/// (`grAnime_801C7228`) visit them: the joint's, each display object's (its own, its material's
/// and textures', its first shape's), the references', then the children's if `recurse`
/// (not entering instances').
#[doc(alias = "HSD_ForeachAnim")]
#[doc(alias = "grAnime_801C7228")]
#[doc(alias = "grAnime_801C70E0")]
#[doc(alias = "grAnime_801C706C")]
pub fn joint_anims(objs: &mut Objects, j: JObjId, mask: u32, recurse: bool) -> Vec<AObjId> {
    let mut out = Vec::new();
    walk(objs, j, mask, recurse, &mut out);
    out
}

fn walk(objs: &mut Objects, j: JObjId, mask: u32, recurse: bool, out: &mut Vec<AObjId>) {
    let o = &objs.jobjs[j];
    if mask & JOBJ_MASK != 0
        && let Some(a) = o.aobj
    {
        out.push(a);
    }
    let mut d = jobj::get_dobj(&objs.jobjs, Some(j));
    while let Some(dobj) = d {
        let dd = &objs.dobjs[dobj];
        if mask & DOBJ_MASK != 0
            && let Some(a) = dd.aobj
        {
            out.push(a);
        }
        if let Some(m) = dd.mobj {
            let mo = &objs.mobjs[m];
            if mask & MOBJ_MASK != 0
                && let Some(a) = mo.aobj
            {
                out.push(a);
            }
            let mut t = mo.tobj;
            while let Some(to) = t {
                if mask & TOBJ_MASK != 0
                    && let Some(a) = objs.tobjs[to].aobj
                {
                    out.push(a);
                }
                t = objs.tobjs[to].next;
            }
        }
        if mask & POBJ_MASK != 0
            && let Some(p) = dd.pobj
            && let Some(a) = pobj::shape_set(&mut objs.pobjs, p).and_then(|s| s.aobj)
        {
            out.push(a);
        }
        d = objs.dobjs[dobj].next;
    }
    let mut r = objs.jobjs[j].robj;
    while let Some(ro) = r {
        if mask & ROBJ_MASK != 0
            && let Some(a) = objs.robjs[ro].aobj
        {
            out.push(a);
        }
        r = objs.robjs[ro].next;
    }
    if recurse && objs.jobjs[j].flags & jobj::INSTANCE == 0 {
        let mut c = objs.jobjs[j].child;
        while let Some(child) = c {
            walk(objs, child, mask, recurse, out);
            c = objs.jobjs[child].next;
        }
    }
}

/// What granime's walks do to each animation they visit.
#[derive(Clone, Copy, Debug)]
pub enum AnimOp {
    /// `fn_801C6EE4`: loop between the frames `grAnime_801C775C` set.
    LoopBetween,
    /// `fn_801C6F2C`: loop.
    Loop,
    /// `HSD_AObjClearFlags(AOBJ_LOOP)`.
    NoLoop,
    /// `HSD_AObjSetRate`.
    Rate(f32),
    /// `HSD_AObjSetFlags`.
    SetFlags(u32),
}

/// Applies `op` to the animations of a joint's parts that `mask` selects, `grAnime_801C752C`.
#[doc(alias = "grAnime_801C752C")]
pub fn apply(game: &mut Game, j: Option<JObjId>, recurse: bool, mask: u32, op: AnimOp) {
    let Some(j) = j else { return };
    let (rewind, end) = game.gr.anime_loop;
    let aobjs = joint_anims(&mut game.objs, j, mask, recurse);
    let a = &mut game.objs.aobjs;
    for x in aobjs {
        match op {
            AnimOp::LoopBetween => {
                aobj::set_flags(a, Some(x), aobj::LOOP);
                aobj::set_rewind_frame(a, Some(x), rewind);
                aobj::set_end_frame(a, Some(x), end);
            }
            AnimOp::Loop => aobj::set_flags(a, Some(x), aobj::LOOP),
            AnimOp::NoLoop => aobj::clear_flags(a, Some(x), aobj::LOOP),
            AnimOp::Rate(r) => aobj::set_rate(a, Some(x), r),
            AnimOp::SetFlags(f) => aobj::set_flags(a, Some(x), f),
        }
    }
}

/// Loops a stage object's part's animations between two frames, `grAnime_801C775C`.
#[doc(alias = "grAnime_801C775C")]
pub fn loop_between(game: &mut Game, gobj: GObjId, part: i32, parts: u32, rewind: f32, end: f32) {
    let Some(j) = nth_joint(game, gobj, part) else {
        return;
    };
    game.gr.anime_loop = (rewind, end);
    apply(game, Some(j), true, part_mask(parts), AnimOp::LoopBetween);
}

/// Loops a stage object's part's animations (`grAnime_801C77FC`, and without its descendants
/// `grAnime_801C787C`).
#[doc(alias = "grAnime_801C77FC")]
#[doc(alias = "grAnime_801C787C")]
pub fn set_loop(game: &mut Game, gobj: GObjId, part: i32, parts: u32, recurse: bool) {
    let j = nth_joint(game, gobj, part);
    if j.is_some() {
        apply(game, j, recurse, part_mask(parts), AnimOp::Loop);
    }
}

/// Stops a stage object's part's animations looping (`grAnime_801C78FC`, and without its
/// descendants `grAnime_801C7980`).
#[doc(alias = "grAnime_801C78FC")]
#[doc(alias = "grAnime_801C7980")]
pub fn clear_loop(game: &mut Game, gobj: GObjId, part: i32, parts: u32, recurse: bool) {
    let j = nth_joint(game, gobj, part);
    if j.is_some() {
        apply(game, j, recurse, part_mask(parts), AnimOp::NoLoop);
    }
}

/// Sets the rate of a stage object's part's animations (`grAnime_801C7A04`, and without its
/// descendants `grAnime_801C7A94`).
#[doc(alias = "grAnime_801C7A04")]
#[doc(alias = "grAnime_801C7A94")]
pub fn set_rate(game: &mut Game, gobj: GObjId, part: i32, parts: u32, rate: f32, recurse: bool) {
    let j = nth_joint(game, gobj, part);
    if j.is_some() {
        apply(game, j, recurse, part_mask(parts), AnimOp::Rate(rate));
    }
}

/// The `HSD_JObjReqAnim*ByFlags` flags for granime's part bits.
fn req_flags(parts: u32) -> u32 {
    let mut f = 0;
    if parts & 1 != 0 {
        f |= 0x81;
    }
    if parts & 2 != 0 {
        f |= 0x416;
    }
    if parts & 4 != 0 {
        f |= 8;
    }
    f
}

/// Restarts a stage object's part's animations at a frame (`grAnime_801C7B24`, and without its
/// descendants `grAnime_801C7BA0`).
#[doc(alias = "grAnime_801C7B24")]
#[doc(alias = "grAnime_801C7BA0")]
pub fn req_anim(game: &mut Game, gobj: GObjId, part: i32, parts: u32, frame: f32, recurse: bool) {
    let Some(j) = nth_joint(game, gobj, part) else {
        return;
    };
    let f = req_flags(parts);
    if recurse {
        jobj::req_anim_all_by_flags(&mut game.objs, Some(j), f, frame);
    } else {
        jobj::req_anim_by_flags(&mut game.objs, Some(j), f, frame);
    }
}

/// Moves an animation's FObj of type 12 to the front of its list, `grAnime_801C69FC`.
#[doc(alias = "grAnime_801C69FC")]
pub fn type_12_first(objs: &mut Objects, a: Option<AObjId>) {
    let Some(a) = a else { return };
    let Some(head) = objs.aobjs[a].fobj else {
        return;
    };
    let mut prev: Option<crate::hsd::fobj::FObjId> = None;
    let mut cur = Some(head);
    while let Some(f) = cur {
        if objs.fobjs[f.0].obj_type == 0xC {
            let next = objs.fobjs[f.0].next;
            match prev {
                None => objs.aobjs[a].fobj = next,
                Some(p) => objs.fobjs[p.0].next = next,
            }
            objs.fobjs[f.0].next = objs.aobjs[a].fobj;
            objs.aobjs[a].fobj = Some(f);
            return;
        }
        prev = Some(f);
        cur = objs.fobjs[f.0].next;
    }
}

/// Replaces an animation with one loaded from `desc`, when there is a descriptor.
fn reload(
    objs: &mut Objects,
    data: &Archives,
    slot: Option<AObjId>,
    desc: Option<DataRef>,
) -> Option<Option<AObjId>> {
    let desc = desc?;
    if slot.is_some() {
        jobj::remove_aobj(objs, slot);
    }
    Some(jobj::load_aobj(objs, data, Some(desc)))
}

/// Gives each morphing shape of a list its animation from a ShapeAnim list,
/// `grAnime_801C6620`: only where the ShapeAnim has a descriptor.
#[doc(alias = "grAnime_801C6620")]
pub fn add_shape_anims(
    objs: &mut Objects,
    data: &Archives,
    p: Option<pobj::PObjId>,
    anim: Option<DataRef>,
) {
    if p.is_none() || anim.is_none() {
        return;
    }
    let (mut cur, mut sa) = (p, anim);
    while let Some(po) = cur {
        let old = pobj::shape_set(&mut objs.pobjs, po)
            .expect("pobj_type(pobj) == POBJ_SHAPEANIM && pobj->u.shape_set")
            .aobj;
        if let Some(s) = sa
            && let Some(a) = reload(objs, data, old, data.ptr(s.at(4)))
        {
            pobj::shape_set(&mut objs.pobjs, po).unwrap().aobj = a;
        }
        cur = objs.pobjs[po].next;
        sa = sa.and_then(|s| data.ptr(s));
    }
}

/// Gives a texture the animation with its id from a TexAnim list, `grAnime_801C6710`: its
/// frames and image table, leaving its palettes alone.
#[doc(alias = "grAnime_801C6710")]
pub fn add_tex_anim(
    objs: &mut Objects,
    data: &Archives,
    t: Option<tobj::TObjId>,
    texanim: Option<DataRef>,
) {
    let Some(t) = t else { return };
    let Some(ta) = tobj::lookup_texanim(data, objs.tobjs[t].id, texanim) else {
        return;
    };
    let old = objs.tobjs[t].aobj;
    if let Some(a) = reload(objs, data, old, data.ptr(ta.at(8))) {
        objs.tobjs[t].aobj = a;
        objs.tobjs[t].imagetbl = data.ptr(ta.at(0xC));
    }
}

/// Gives a material its animation from a MatAnim and its textures theirs, `grAnime_801C67A8`.
#[doc(alias = "grAnime_801C67A8")]
pub fn add_mat_anim(
    objs: &mut Objects,
    data: &Archives,
    m: Option<crate::hsd::mobj::MObjId>,
    matanim: Option<DataRef>,
) {
    let (Some(m), Some(ma)) = (m, matanim) else {
        return;
    };
    let old = objs.mobjs[m].aobj;
    if let Some(a) = reload(objs, data, old, data.ptr(ma.at(4))) {
        objs.mobjs[m].aobj = a;
    }
    let texanim = data.ptr(ma.at(8));
    let mut t = objs.mobjs[m].tobj;
    while let Some(to) = t {
        add_tex_anim(objs, data, Some(to), texanim);
        t = objs.tobjs[to].next;
    }
}

/// Gives a display object list its materials' and shapes' animations, `grAnime_801C683C`.
#[doc(alias = "grAnime_801C683C")]
pub fn add_dobj_anims(
    objs: &mut Objects,
    data: &Archives,
    d: Option<crate::hsd::dobj::DObjId>,
    matanim: Option<DataRef>,
    shapes: Option<DataRef>,
) {
    let (mut cur, mut ma, mut sd) = (d, matanim, shapes);
    while let Some(dobj) = cur {
        let shapeanim = sd.and_then(|s| data.ptr(s.at(4)));
        add_shape_anims(objs, data, objs.dobjs[dobj].pobj, shapeanim);
        add_mat_anim(objs, data, objs.dobjs[dobj].mobj, ma);
        cur = objs.dobjs[dobj].next;
        ma = ma.and_then(|m| data.ptr(m));
        sd = sd.and_then(|s| data.ptr(s));
    }
}

/// Gives a reference list its animations from an RObjAnimJoint list (`grAnime_801C6960`, each
/// through `grAnime_801C68F4`), as long as both last.
#[doc(alias = "grAnime_801C6960")]
#[doc(alias = "grAnime_801C68F4")]
pub fn add_robj_anims(
    objs: &mut Objects,
    data: &Archives,
    r: Option<crate::hsd::robj::RObjId>,
    anim: Option<DataRef>,
) {
    let (mut cur, mut ra) = (r, anim);
    while let (Some(ro), Some(an)) = (cur, ra) {
        let old = objs.robjs[ro].aobj;
        if let Some(a) = reload(objs, data, old, data.ptr(an.at(4))) {
            objs.robjs[ro].aobj = a;
        }
        cur = objs.robjs[ro].next;
        ra = data.ptr(an);
    }
}

/// Gives a joint its animations from an AnimJoint, MatAnimJoint and ShapeAnimJoint,
/// `grAnime_801C6A54`.
#[doc(alias = "grAnime_801C6A54")]
pub fn add_joint_anims(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    aj: Option<DataRef>,
    mj: Option<DataRef>,
    sj: Option<DataRef>,
) {
    let Some(j) = j else { return };
    if let Some(aj) = aj {
        let old = objs.jobjs[j].aobj;
        if let Some(a) = reload(objs, data, old, data.ptr(aj.at(8))) {
            objs.jobjs[j].aobj = a;
            type_12_first(objs, a);
        }
        add_robj_anims(objs, data, objs.jobjs[j].robj, data.ptr(aj.at(0xC)));
    }
    if let Some(d) = jobj::get_dobj(&objs.jobjs, Some(j)) {
        let matanim = mj.and_then(|m| data.ptr(m.at(8)));
        let shapes = sj.and_then(|s| data.ptr(s.at(8)));
        add_dobj_anims(objs, data, Some(d), matanim, shapes);
    }
}

/// [`add_joint_anims`] for a joint and its descendants, `grAnime_801C6C0C`.
#[doc(alias = "grAnime_801C6C0C")]
pub fn add_tree_anims(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    aj: Option<DataRef>,
    mj: Option<DataRef>,
    sj: Option<DataRef>,
) {
    let Some(j) = j else { return };
    add_joint_anims(objs, data, Some(j), aj, mj, sj);
    if objs.jobjs[j].flags & jobj::INSTANCE != 0 {
        return;
    }
    let child = |d: Option<DataRef>| d.and_then(|d| data.ptr(d));
    let next = |d: Option<DataRef>| d.and_then(|d| data.ptr(d.at(4)));
    let (mut a, mut m, mut s) = (child(aj), child(mj), child(sj));
    let mut c = objs.jobjs[j].child;
    while let Some(ch) = c {
        add_tree_anims(objs, data, Some(ch), a, m, s);
        c = objs.jobjs[ch].next;
        (a, m, s) = (next(a), next(m), next(s));
    }
}

/// Registers a model's spline joints in HSD's ID table by their own address,
/// `grMaterial_801C8B68` with `arg1` 0.
#[doc(alias = "grMaterial_801C8B68")]
pub fn register_splines(objs: &Objects, j: Option<JObjId>) {
    let Some(j) = j else { return };
    assert!(
        objs.jobjs[j].flags & jobj::SPLINE == 0,
        "grMaterial_801C8B68: a spline joint keyed by its address in HSD's ID table isn't modeled"
    );
    if objs.jobjs[j].flags & jobj::INSTANCE == 0 {
        let mut c = objs.jobjs[j].child;
        while let Some(ch) = c {
            register_splines(objs, Some(ch));
            c = objs.jobjs[ch].next;
        }
    }
}

/// A model joint's root (`grMaterial_801C8B28`).
#[doc(alias = "grMaterial_801C8B28")]
pub fn root_of(objs: &Objects, j: JObjId) -> JObjId {
    let mut j = j;
    while let Some(p) = objs.jobjs[j].parent {
        j = p;
    }
    j
}

/// Starts animation `anim` of a stage object's map on part `part` of its model and the part's
/// descendants (`recurse`) or the part alone, `grAnime_801C7C1C`: the archive's AnimJoint,
/// MatAnimJoint and ShapeAnimJoint entries for that part, as `parts` selects, restarted at
/// `frame` and running at `rate`, looping if the archive marks the animation so.
#[doc(alias = "grAnime_801C7C1C")]
#[allow(clippy::too_many_arguments)]
pub fn start_part(
    game: &mut Game,
    data: &Archives,
    j: Option<JObjId>,
    map_id: i32,
    part: i32,
    parts: u32,
    anim: i32,
    recurse: bool,
    frame: f32,
    rate: f32,
) {
    let Some(j) = j else { return };
    let head = datfiles::archive_for(game, data, map_id).expect("granime.c: archive");
    let entry = head.entry(data, map_id);
    let mut req = 0;
    let mut mask = 0;
    let pick = |which: u32, size: u32| {
        let list = entry.anims(data, which)?;
        let root = data.ptr(list.at(4 * anim as u32))?;
        Some(root.at(size * part as u32))
    };
    let aj = if parts & 1 != 0 { pick(0, 0x14) } else { None };
    if aj.is_some() {
        req |= 0x81;
        mask |= 0x220;
    }
    let mj = if parts & 2 != 0 { pick(1, 0xC) } else { None };
    if mj.is_some() {
        req |= 0x416;
        mask |= 0x7484;
    }
    let sj = if parts & 4 != 0 { pick(2, 0xC) } else { None };
    if sj.is_some() {
        req |= 8;
        mask |= 0x100;
    }
    if recurse {
        add_tree_anims(&mut game.objs, data, Some(j), aj, mj, sj);
        jobj::req_anim_all_by_flags(&mut game.objs, Some(j), req, frame);
    } else {
        add_joint_anims(&mut game.objs, data, Some(j), aj, mj, sj);
        jobj::req_anim_by_flags(&mut game.objs, Some(j), req, frame);
    }
    apply(game, Some(j), recurse, mask, AnimOp::Rate(rate));
    if loops(game, data, map_id, anim) {
        apply(game, Some(j), recurse, mask, AnimOp::SetFlags(aobj::LOOP));
    }
}

/// Whether the archive marks a map's animation as looping (its `x28` byte).
fn loops(game: &Game, data: &Archives, map_id: i32, anim: i32) -> bool {
    let head = datfiles::archive_for(game, data, map_id).expect("granime.c: archive");
    match head.entry(data, map_id).loop_flags(data) {
        Some(flags) => data.u8(flags.at(anim as u32)) != 0,
        None => false,
    }
}

/// Starts an animation on a stage object's part and its descendants (`grAnime_801C7FF8`), or
/// the part alone (`grAnime_801C8098`).
#[doc(alias = "grAnime_801C7FF8")]
#[doc(alias = "grAnime_801C8098")]
#[allow(clippy::too_many_arguments)]
pub fn start(
    game: &mut Game,
    data: &Archives,
    gobj: GObjId,
    part: i32,
    parts: u32,
    anim: i32,
    frame: f32,
    rate: f32,
    recurse: bool,
) {
    let map_id = game.grounds[ground_of(game, gobj).0].map_id;
    if anim < 0 {
        return;
    }
    let Some(j) = nth_joint(game, gobj, part) else {
        return;
    };
    register_splines(&game.objs, Some(root_of(&game.objs, j)));
    start_part(
        game,
        data,
        Some(j),
        map_id,
        part,
        parts,
        anim,
        recurse,
        frame,
        rate,
    );
}

/// Resets a model tree's SRT to its descriptors' along siblings and children, marking every
/// matrix dirty, `lb_8000B804`.
#[doc(alias = "lb_8000B804")]
pub fn reset_tree_srt(
    objs: &mut Objects,
    data: &Archives,
    j: Option<JObjId>,
    joint: Option<DataRef>,
) {
    let (Some(j), Some(joint)) = (j, joint) else {
        return;
    };
    let o = &mut objs.jobjs[j];
    o.rotate.x = copy(data.f32(joint.at(0x14)));
    o.rotate.y = copy(data.f32(joint.at(0x18)));
    o.rotate.z = copy(data.f32(joint.at(0x1C)));
    let v =
        |at: DataRef| crate::vec::Vec3::new(data.f32(at), data.f32(at.at(4)), data.f32(at.at(8)));
    o.scale = v(joint.at(0x20));
    o.translate = v(joint.at(0x2C));
    jobj::clear_flags(&mut objs.jobjs, Some(j), jobj::USE_QUATERNION);
    jobj::set_flags(&mut objs.jobjs, Some(j), jobj::MTX_DIRTY);
    let (next, child) = (objs.jobjs[j].next, objs.jobjs[j].child);
    reset_tree_srt(objs, data, next, data.ptr(joint.at(0xC)));
    reset_tree_srt(objs, data, child, data.ptr(joint.at(8)));
}

/// Starts a stage object's map animation `anim` on its whole model, `grAnime_801C8138`: the
/// model's SRT reset to its descriptors, its animations replaced, restarted at frame 0, and
/// run once.
#[doc(alias = "grAnime_801C8138")]
pub fn start_map(game: &mut Game, data: &Archives, gobj: GObjId, map_id: i32, anim: i32) {
    let mut j = jobj_of(game, gobj);
    if anim < 0 {
        return;
    }
    let head = datfiles::archive_for(game, data, map_id).expect("granime.c: archive");
    let entry = head.entry(data, map_id);
    let joint = entry.joint(data);
    if let Some(c) = j.and_then(|j| game.objs.jobjs[j].child) {
        j = Some(c);
    }
    if joint.is_some() {
        reset_tree_srt(&mut game.objs, data, j, joint);
    }
    register_splines(&game.objs, j);
    jobj::remove_anim_all(&mut game.objs, j);
    let pick = |which: u32| {
        entry
            .anims(data, which)
            .and_then(|l| data.ptr(l.at(4 * anim as u32)))
    };
    let (sa, ma, aj) = (pick(2), pick(1), pick(0));
    jobj::add_anim_all(&mut game.objs, data, j, aj, ma, sa);
    jobj::req_anim_all(&mut game.objs, j, 0.0);
    if loops(game, data, map_id, anim) {
        apply(game, j, true, 0x77A4, AnimOp::SetFlags(aobj::LOOP));
    }
    jobj::anim_all(&mut game.objs, data, j);
}

/// The first animation of a stage object's part, `grAnime_801C8318`.
#[doc(alias = "grAnime_801C8318")]
pub fn first_anim(game: &mut Game, gobj: GObjId, part: i32, parts: u32) -> Option<AObjId> {
    let j = nth_joint(game, gobj, part)?;
    joint_anims(&mut game.objs, j, part_mask(parts), true)
        .first()
        .copied()
}

/// Whether a stage object's part's first animation has finished (`AOBJ_NO_ANIM`),
/// `grAnime_801C83D0`.
#[doc(alias = "grAnime_801C83D0")]
pub fn finished(game: &mut Game, gobj: GObjId, part: i32, parts: u32) -> bool {
    let a = first_anim(game, gobj, part, parts);
    aobj::flags(&game.objs.aobjs, a) & aobj::NO_ANIM != 0
}

/// Whether a stage object's part's first animation has rewound (`AOBJ_REWINDED`),
/// `grAnime_801C84A4`.
#[doc(alias = "grAnime_801C84A4")]
pub fn rewound(game: &mut Game, gobj: GObjId, part: i32, parts: u32) -> bool {
    let a = first_anim(game, gobj, part, parts);
    aobj::flags(&game.objs.aobjs, a) & aobj::REWINDED != 0
}
