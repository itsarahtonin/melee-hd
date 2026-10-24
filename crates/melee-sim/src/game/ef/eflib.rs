//! efLib (`melee/ef/eflib.c`): effects' lifetimes and updates, making effects and generators,
//! and the callbacks the particle system and joints call.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::super::code::ParticleHooks;
use super::super::handles::CodeRef;
use super::super::state::{Game, OBJ_JOBJ, Obj, Running, UserData};
use super::{
    Effect, EffectId, LOAD_COUNTED, MAX_COUNTED, P_LINK, P_LINK_AUX, ParamEntry, SCALE_INHERIT,
    STATE_ASYNC, STATE_ENTERING_PAUSE, STATE_PAUSED, Update, desc, desc_fields,
};
use crate::fpu::{copy, mem, reg};
use crate::hsd::Objects;
use crate::hsd::appsrt;
use crate::hsd::generator::{self, GeneratorId};
use crate::hsd::gobj::{self, GObjId};
use crate::hsd::jobj::{self, JObjId};
use crate::hsd::particle::{self, GeneratorHook, Hooks, ParticleId, UserFunc};
use crate::hsd::{mobj, robj};
use crate::math;
use crate::vec::Vec3;

/// `efLib_Update`, effects' process.
pub const UPDATE_PROC: u32 = 0x8005_BC50;
/// `efLib_particles_proc_main` and `efLib_particles_proc_aux`, the particle system's processes.
pub const PARTICLES_PROC_MAIN: u32 = 0x8005_C9A4;
pub const PARTICLES_PROC_AUX: u32 = 0x8005_C9D0;
/// `efLib_render_callback`, the particle GObjs' render callback.
pub const RENDER: u32 = 0x8005_C904;
/// `HSD_GObj_JObjCallback`, effects' render callback.
pub const JOBJ_RENDER: u32 = 0x8039_1070;
/// `efLib_Cb_PtclAppSRTHook`, the hook on new generators.
pub const PTCL_APPSRT_HOOK: GeneratorHook = GeneratorHook(0x8005_DCD0);
/// `lbl_803BF810`: the user functions that give some common effects' particles their own copy
/// of their generator's app-SRT (`efLib_Cb_ParticleRender` on creation).
pub const PARTICLE_RENDER_FUNCS: UserFunc = UserFunc(0x803B_F810);

/// Skips particle links 1 and 2 (`hsd_8039CEAC` and `hsd_8039EE24`'s mask), and link 0.
const SKIP_LINKS_1_2: u32 = 0x60000;
const SKIP_LINK_0: u32 = 0x10000;

/// `M_PI_2` stored as a float.
const HALF_PI: f32 = 1.570_796_4;

/// `rot.y` for a facing: `-M_PI_2` behind, else `M_PI_2` (computed in double, stored single).
fn facing_rot(facing: f32) -> f32 {
    if reg(facing) < 0.0 { -HALF_PI } else { HALF_PI }
}

/// A GObj's joint (`gobj->hsd_obj`).
fn jobj_of(game: &Game, gobj: GObjId) -> Option<JObjId> {
    match game.world.gobjs[gobj.0].obj {
        Some(Obj::JObj(j)) => Some(j),
        None => None,
        other => panic!("GObj {gobj:?} holds {other:?}, not a joint"),
    }
}

/// An effect's joint.
fn effect_jobj(game: &Game, e: EffectId) -> Option<JObjId> {
    jobj_of(game, game.ef.effects[e.0].gobj.expect("an effect's GObj"))
}

/// An effect's joint, which the code reading it assumes is there.
fn ejobj(game: &Game, e: EffectId) -> JObjId {
    effect_jobj(game, e).expect("an effect's joint")
}

/// The root of a joint's tree.
fn root_of(objs: &Objects, mut j: JObjId) -> JObjId {
    while let Some(p) = objs.jobjs[j].parent {
        j = p;
    }
    j
}

/// Sets up efLib for a scene, `efLib_Init`: empty effect and generator pools, the particle
/// system emptied, efLib's hooks into it and joints, and the particle system's two GObjs.
#[doc(alias = "efLib_Init")]
#[doc(alias = "efAsync_QueueInit")]
pub fn init(r: &mut Running) {
    crate::hsd::heap::init_pools(&[
        crate::hsd::heap::Kind::Effect,
        crate::hsd::heap::Kind::EffectQueued,
    ]);
    let g = &mut *r.game;
    g.ef.effects = Default::default();
    g.ef.count = 0;
    for t in &mut g.ef.tables[..50] {
        *t = None;
    }
    generator::init(&mut g.objs);
    particle::init(&mut g.objs);
    g.objs.generators.on_create = Some(PTCL_APPSRT_HOOK);
    let gobj = g.world.create(super::CLASS, P_LINK, 1);
    g.world.setup_gx_link(gobj, CodeRef(RENDER), 7, 2);
    g.world.setup_proc(gobj, CodeRef(PARTICLES_PROC_MAIN), 0xF);
    let gobj = g.world.create(super::CLASS, P_LINK_AUX, 1);
    g.world.setup_gx_link(gobj, CodeRef(RENDER), 8, 2);
    g.world.setup_proc(gobj, CodeRef(PARTICLES_PROC_AUX), 0xF);
    g.objs.jobj.sptcl = true;
    jobj::set_dptcl_callback(&mut g.objs, true);
    appsrt::init(&mut g.objs, 0, 0xA4);
    g.ef.queued = Default::default();
    for p in &mut g.ef.params {
        p.gobj = None;
        p.alpha = 0xFF;
    }
}

/// `f` on each effect on both effect links whose parent is `parent`, in link order (it
/// changes no links).
fn each_child(game: &mut Game, parent: Option<GObjId>, mut f: impl FnMut(&mut Game, EffectId)) {
    for link in [P_LINK, P_LINK_AUX] {
        let mut g = game.world.first_in(link);
        while let Some(gobj) = g {
            g = game.world.gobjs[gobj.0].next;
            if let Some(UserData::Effect(e)) = game.world.gobjs[gobj.0].user_data
                && game.ef.effects[e.0].parent == parent
            {
                f(game, e);
            }
        }
    }
}

/// Marks every effect `gobj` spawned with `flags`, `efLib_SetFlags`.
#[doc(alias = "efLib_SetFlags")]
pub fn set_flags(game: &mut Game, gobj: Option<GObjId>, flags: i32) {
    each_child(game, gobj, |game, e| {
        let ef = &mut game.ef.effects[e.0];
        ef.expire_flags = (i32::from(ef.expire_flags) | flags) as i8;
    });
}

/// Destroys a counted effect's GObj, removing the generators on its joints and any stored
/// alpha for it, `efLib_Destroy`.
#[doc(alias = "efLib_Destroy")]
pub fn destroy(r: &mut Running, gobj: GObjId) {
    let Some(UserData::Effect(e)) = r.game.world.gobjs[gobj.0].user_data else {
        return;
    };
    if r.game.ef.effects[e.0].is_async != 0 {
        return;
    }
    for p in &mut r.game.ef.params {
        if p.gobj == Some(gobj) {
            p.gobj = None;
        }
    }
    detach_tree(r, gobj);
    gobj::free(r, gobj);
}

/// Removes the generators following any joint of a GObj's tree (`HSD_JObjWalkTree` with
/// `hsd_8039D688`), if it holds joints.
fn detach_tree(r: &mut Running, gobj: GObjId) {
    let w = &r.game.world.gobjs[gobj.0];
    if w.obj_kind != OBJ_JOBJ {
        return;
    }
    let Some(Obj::JObj(j)) = w.obj else {
        panic!("a joint GObj without a joint")
    };
    // `HSD_JObjWalkTree`'s order: a joint, then its own children (detaching changes no joint
    // links).
    fn walk(r: &mut Running, j: JObjId) {
        generator::detach_jobj(&mut r.game.objs, r.data, &mut ParticleHooks, j);
        if r.game.objs.jobjs[j].flags & jobj::INSTANCE == 0 {
            let mut cur = r.game.objs.jobjs[j].child;
            while let Some(c) = cur {
                walk(r, c);
                cur = r.game.objs.jobjs[c].next;
            }
        }
    }
    walk(r, j);
}

/// Destroys every effect `gobj` spawned and the generators on `gobj`'s own joints, and forgets
/// its stored alpha, `efLib_DestroyAll`.
#[doc(alias = "efLib_DestroyAll")]
pub fn destroy_all(r: &mut Running, gobj: GObjId) {
    for p in &mut r.game.ef.params {
        if p.gobj == Some(gobj) {
            p.gobj = None;
        }
    }
    for link in [P_LINK, P_LINK_AUX] {
        let mut cur = r.game.world.first_in(link);
        while let Some(g) = cur {
            cur = r.game.world.gobjs[g.0].next;
            if let Some(UserData::Effect(e)) = r.game.world.gobjs[g.0].user_data
                && r.game.ef.effects[e.0].parent == Some(gobj)
            {
                let eg = r.game.ef.effects[e.0].gobj.expect("an effect's GObj");
                detach_tree(r, eg);
                gobj::free(r, eg);
            }
        }
    }
    detach_tree(r, gobj);
}

/// Pauses every effect `gobj` spawned from its next update, `efLib_PauseAll`.
#[doc(alias = "efLib_PauseAll")]
pub fn pause_all(game: &mut Game, gobj: Option<GObjId>) {
    each_child(game, gobj, |game, e| {
        let ef = &mut game.ef.effects[e.0];
        ef.state_flags = ef.state_flags & STATE_ASYNC | STATE_ENTERING_PAUSE;
    });
}

/// Resumes every effect `gobj` spawned, `efLib_ResumeAll`.
#[doc(alias = "efLib_ResumeAll")]
pub fn resume_all(game: &mut Game, gobj: Option<GObjId>) {
    each_child(game, gobj, |game, e| {
        game.ef.effects[e.0].state_flags &= STATE_ASYNC;
    });
}

/// Frees an effect as its GObj goes, `efLib_remove_user_data`.
#[doc(alias = "efLib_remove_user_data")]
pub fn remove_user_data(game: &mut Game, e: EffectId) {
    let ef = &game.ef.effects[e.0];
    assert!(ef.gobj.is_some(), "Duplicate Free {e:?}");
    if ef.is_async == 0 {
        game.ef.count = game.ef.count.wrapping_sub(1);
    }
    free_effect(game, e);
}

/// Returns an effect's block to efLib's allocator, `HSD_ObjFree`: the block's first word, the
/// effect's `next`, becomes the free list's link to the block freed before it, which a spawn
/// that chained this effect still reads.
pub(super) fn free_effect(game: &mut Game, e: EffectId) {
    let link = game.ef.effects.free_head().map(EffectId);
    game.ef.effects[e.0].next = link;
    game.ef.effects.free(e.0);
}

/// Returns a deferred spawn's block to efAsync's allocator, `HSD_ObjFree`: its `next` becomes
/// the free list's link.
pub(super) fn free_queued(game: &mut Game, q: super::QueuedId) {
    let link = game.ef.queued.free_head().map(super::QueuedId);
    game.ef.queued[q.0].next = link;
    game.ef.queued.free(q.0);
}

/// Destroys the oldest counted effects until fewer than 64 are left, `efLib_RemoveLast`.
#[doc(alias = "efLib_RemoveLast")]
pub fn remove_last(r: &mut Running) {
    for link in [P_LINK, P_LINK_AUX] {
        let mut cur = r.game.world.first_in(link);
        while let Some(g) = cur {
            let next = r.game.world.gobjs[g.0].next;
            destroy(r, g);
            if r.game.ef.count < MAX_COUNTED {
                return;
            }
            cur = next;
        }
    }
    panic!("can't remove no force effect!");
}

/// An effect's process, `efLib_Update`: pausing, counting its life down (it goes at 0), its
/// joint's scale if it inherits it, its animation, then its update.
#[doc(alias = "efLib_Update")]
pub fn update(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    let e = r.game.effect_id(gobj);
    let ef = &mut r.game.ef.effects[e.0];
    match ef.state_flags {
        STATE_ENTERING_PAUSE => ef.state_flags += 1,
        STATE_PAUSED => return,
        _ => {}
    }
    if ef.lifetime != 0 {
        ef.lifetime -= 1;
        if ef.lifetime == 0 {
            gobj::free(r, gobj);
            return;
        }
    }
    let ef = &r.game.ef.effects[e.0];
    if ef.scale_flags != 0
        && let Some(attach) = ef.attach_jobj
    {
        let inherit = ef.scale_flags & SCALE_INHERIT != 0;
        let objs = &mut r.game.objs;
        let mut scale = Vec3::default();
        if objs.jobjs[attach].parent.is_some() {
            jobj::setup_matrix(objs, r.data, Some(attach));
            if inherit {
                // `HSD_JObjGetMtxPtr` brings the matrix up to date again.
                jobj::setup_matrix(objs, r.data, Some(attach));
                scale = objs.jobjs[attach].mtx.scale_part();
            }
        } else if inherit {
            scale = objs.jobjs[attach].scale;
        }
        if inherit {
            scale.y = copy(scale.y);
            scale.x = scale.y;
            scale.z = scale.y;
            jobj::set_scale(&mut objs.jobjs, j.expect("HSD_JObjSetScale: jobj"), scale);
        }
    }
    jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, j);
    if let Some(u) = r.game.ef.effects[e.0].update {
        run_update(r, e, u);
    }
}

/// Runs an effect's update.
pub fn run_update(r: &mut Running, e: EffectId, u: Update) {
    match u {
        Update::SetOffsetYFromParamY => cb_set_offset_y_from_param_y(r, e),
        Update::SetScaleFromParamX => cb_set_scale_from_param_x(r.game, e),
        Update::SetRotYAndTransition => cb_set_rot_y_and_transition(r, e),
        Update::SetJObjOffsetZ => cb_set_jobj_offset_z(r.game, e),
        Update::SetRotYFromFighterDir => cb_set_rot_y_from_fighter_dir(r.game, e),
        Update::SetRotYZFromFighter => cb_set_rot_yz_from_fighter(r.game, e),
        Update::FallFromParamY => cb_fall_from_param_y(r.game, e),
        Update::SetOffsetFromParams => cb_set_offset_from_params(r.game, e),
        Update::LifetimeEndSpawn => cb_lifetime_end_spawn(r, e),
        Update::SetScaleRotYFromFighter => cb_set_scale_rot_y_from_fighter(r.game, e),
        Update::SetRotYZFromParamZFighterDir => cb_set_rot_yz_from_param_z(r.game, e),
        Update::MarioSpecialLw => cb_special_lw(r.game, e, 349, MARIO_SPECIAL_LW_COLL),
        Update::LuigiSpecialLw => cb_special_lw(r.game, e, 357, LUIGI_SPECIAL_LW_COLL),
        Update::KoopaSpecialHi => cb_koopa_special_hi(r.game, e),
        Update::Bury => cb_bury(r, e),
        Update::ApplyStoredAlpha => cb_apply_stored_alpha(r.game, e),
        Update::AccumOffsetFromParams => cb_accum_offset_from_params(r, e),
    }
}

/// Makes effect `gfx_id` for `parent`, `efLib_Create`: its GObj (process link 12 for ids 25
/// and 26, else 11) with its descriptor's model, animations started at frame 0 (and queued to
/// run once before the spawn returns), and its lifetime. A counted effect past the 64th
/// destroys the oldest first.
#[doc(alias = "efLib_Create")]
pub fn create(r: &mut Running, gfx_id: i32, parent: Option<GObjId>) -> Option<EffectId> {
    let d = desc(r.game, gfx_id);
    if r.game.ef.load_kind == LOAD_COUNTED {
        if r.game.ef.count >= MAX_COUNTED {
            remove_last(r);
        }
        r.game.ef.count = r.game.ef.count.wrapping_add(1);
    }
    let load_kind = r.game.ef.load_kind;
    let e = EffectId(r.game.ef.effects.alloc(Effect {
        is_async: load_kind as u8,
        ..Effect::default()
    }));
    assert!(
        !(parent.is_none() && load_kind != 0),
        "error no parent gobj!"
    );
    let p_link = if !(25..27).contains(&gfx_id) {
        P_LINK
    } else {
        P_LINK_AUX
    };
    r.game.ef.effects[e.0].parent = parent;
    let gobj = r.game.world.create(super::CLASS, p_link, 0);
    r.game.ef.effects[e.0].gobj = Some(gobj);
    r.game.world.setup_gx_link(gobj, CodeRef(JOBJ_RENDER), 7, 1);
    r.game
        .world
        .init_user_data(gobj, super::USER_DATA_KIND, UserData::Effect(e));
    let (lifetime, [joint, anim, matanim, shapeanim]) = desc_fields(r.data, d);
    let Some(j) = jobj::load_joint(&mut r.game.objs, r.data, joint) else {
        gobj::free(r, gobj);
        return None;
    };
    r.game.world.set_obj(gobj, OBJ_JOBJ, Obj::JObj(j));
    let tenths = crate::ppc::cvt_fp2unsigned(fp::fmuls(10.0, reg(lifetime)));
    if !tenths.is_multiple_of(10) {
        crate::lb::joint::set_mobj_flags(&mut r.game.objs, j, 0x0800_0000);
    }
    let life = crate::ppc::cvt_fp2unsigned(reg(lifetime)) as u16;
    r.game.ef.effects[e.0].lifetime = if life != 0 { life + 1 } else { 0 };
    if anim.is_some() || matanim.is_some() || shapeanim.is_some() {
        jobj::add_anim_all(&mut r.game.objs, r.data, Some(j), anim, matanim, shapeanim);
        jobj::req_anim_all(&mut r.game.objs, Some(j), 0.0);
        let ef = &mut r.game.ef;
        ef.anim_queue[ef.anim_count as usize] = Some(j);
        ef.anim_count += 1;
        assert!(ef.anim_count < 32, "Over Anime Call");
    }
    r.game.world.setup_proc(gobj, CodeRef(UPDATE_PROC), 15);
    Some(e)
}

/// An effect constrained to a joint's position, starting there, `efLib_Create_Attach`.
#[doc(alias = "efLib_Create_Attach")]
pub fn create_attach(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
) -> Option<EffectId> {
    let e = create(r, gfx_id, gobj)?;
    let Some(ej) = effect_jobj(r.game, e) else {
        gobj::free(r, r.game.ef.effects[e.0].gobj.expect("an effect's GObj"));
        return None;
    };
    let target = jobj.expect("lb_8000C1C0: a constraint joint");
    constrain(&mut r.game.objs, ej, target, 0x9000_0001);
    let t = crate::lb::joint::world_position(&mut r.game.objs, r.data, jobj, None);
    jobj::set_translate(&mut r.game.objs.jobjs, ej, t);
    r.game.ef.effects[e.0].attach_jobj = jobj;
    Some(e)
}

/// Gives `j` a constraint to `target` of the kind `flags` names: position (`lb_8000C1C0`),
/// direction (`lb_8000C228`) or orientation (`lb_8000C290`).
#[doc(alias = "lb_8000C1C0")]
#[doc(alias = "lb_8000C290")]
pub fn constrain(objs: &mut Objects, j: JObjId, target: JObjId, flags: u32) {
    let r = robj::alloc(&mut objs.robjs);
    robj::set_flags(&mut objs.robjs, Some(r), flags);
    robj::set_constraint_obj(objs, Some(r), target);
    jobj::prepend_robj(objs, Some(j), Some(r));
}

/// [`create_attach`] plus the joint's orientation, `efLib_Create_AttachChild`.
#[doc(alias = "efLib_Create_AttachChild")]
pub fn create_attach_child(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
) -> Option<EffectId> {
    let e = create_attach(r, gfx_id, gobj, jobj)?;
    let ej = ejobj(r.game, e);
    constrain(
        &mut r.game.objs,
        ej,
        jobj.expect("a constraint joint"),
        0x9000_0004,
    );
    Some(e)
}

/// The spawner's y scale on all three axes (`HSD_JObjGetScale` of its joint).
fn spawner_scale(game: &Game, gobj: Option<GObjId>) -> Vec3 {
    let j = jobj_of(game, gobj.expect("a spawner GObj")).expect("HSD_JObjGetScale: jobj");
    let y = copy(game.objs.jobjs[j].scale.y);
    Vec3::new(y, y, y)
}

/// [`create_attach`] at the spawner's y scale, `efLib_Create_Attach_Scale`.
#[doc(alias = "efLib_Create_Attach_Scale")]
pub fn create_attach_scale(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
) -> Option<EffectId> {
    let e = create_attach(r, gfx_id, gobj, jobj)?;
    let s = spawner_scale(r.game, gobj);
    let ej = ejobj(r.game, e);
    jobj::set_scale(&mut r.game.objs.jobjs, ej, s);
    Some(e)
}

/// [`create_attach_child`] at the spawner's y scale, `efLib_Create_AttachChild_Scale`.
#[doc(alias = "efLib_Create_AttachChild_Scale")]
pub fn create_attach_child_scale(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
) -> Option<EffectId> {
    let e = create_attach_child(r, gfx_id, gobj, jobj)?;
    let s = spawner_scale(r.game, gobj);
    let ej = ejobj(r.game, e);
    jobj::set_scale(&mut r.game.objs.jobjs, ej, s);
    Some(e)
}

/// [`create_attach_scale`] turning to the spawning fighter's facing each frame,
/// `efLib_Create_Attach_Scale_FacingDir`.
#[doc(alias = "efLib_Create_Attach_Scale_FacingDir")]
pub fn create_attach_scale_facing(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
) -> Option<EffectId> {
    let e = create_attach_scale(r, gfx_id, gobj, jobj)?;
    r.game.ef.effects[e.0].update = Some(Update::SetRotYFromFighterDir);
    Some(e)
}

/// An effect at a position, `efLib_Create_Attach_Pos`.
#[doc(alias = "efLib_Create_Attach_Pos")]
pub fn create_attach_pos(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    pos: Vec3,
) -> Option<EffectId> {
    let e = create(r, gfx_id, gobj)?;
    let ej = effect_jobj(r.game, e).expect("HSD_JObjSetTranslate: jobj");
    jobj::set_translate(&mut r.game.objs.jobjs, ej, pos);
    Some(e)
}

/// The particle GObjs' render callback, `efLib_render_callback`: the display pass's
/// particle drawing for its pass (`psDispParticles`, links 0 and 2 on render link 7, link 1 on
/// 8). The color update state it sets is presentation.
#[doc(alias = "efLib_render_callback")]
pub fn render(game: &mut Game, gobj: GObjId, pass: i32) -> crate::hsd::psdisp::DrawnLinks {
    let sw = match pass {
        0..=2 => pass as u32,
        _ => return crate::hsd::psdisp::DrawnLinks::default(),
    };
    let links = if game.world.gobjs[gobj.0].gx_link == 7 {
        0b101
    } else {
        0b010
    };
    crate::hsd::psdisp::disp_particles(&mut game.objs, links, sw)
}

/// Runs the particles and generators of links 0, `efLib_particles_proc_main`.
#[doc(alias = "efLib_particles_proc_main")]
pub fn particles_proc_main(r: &mut Running) {
    let objs = &mut r.game.objs;
    particle::execute_all(objs, r.data, &mut ParticleHooks, SKIP_LINKS_1_2);
    generator::run_all(objs, r.data, &mut ParticleHooks, SKIP_LINKS_1_2);
}

/// Runs the particles and generators of links 1 and 2, `efLib_particles_proc_aux`.
#[doc(alias = "efLib_particles_proc_aux")]
pub fn particles_proc_aux(r: &mut Running) {
    let objs = &mut r.game.objs;
    particle::execute_all(objs, r.data, &mut ParticleHooks, SKIP_LINK_0);
    generator::run_all(objs, r.data, &mut ParticleHooks, SKIP_LINK_0);
}

/// The particle link a generator of `gfx_id` goes on: 2 for id 0x121, 1 for four others.
fn link_for(gfx_id: i32) -> i32 {
    match gfx_id {
        0x121 => 2,
        0xFF | 0x7918 | 0xFC | 0xF7 => 1,
        _ => 0,
    }
}

/// A generator of effect `gfx_id` at `pos`, `efLib_CreateGenerator`.
#[doc(alias = "efLib_CreateGenerator")]
pub fn create_generator(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    gfx_id: i32,
    pos: Vec3,
) -> Option<GeneratorId> {
    let g = generator::spawn(objs, data, hooks, link_for(gfx_id), gfx_id / 1000, gfx_id)?;
    let o = &mut objs.generators[g];
    o.pos.x = copy(pos.x);
    o.pos.y = copy(pos.y);
    o.pos.z = copy(pos.z);
    Some(g)
}

/// Gives a generator an app-SRT of `status` if it has none (`psAddGeneratorAppSRT_begin`,
/// which always finds one here: the original kills the generator when the pool is full).
fn ensure_appsrt(objs: &mut Objects, g: GeneratorId, status: i32) -> appsrt::AppSrtId {
    match objs.generators[g].appsrt {
        Some(s) => s,
        None => appsrt::add_to_generator(objs, g, status),
    }
}

/// A generator of effect `gfx_id` on link 0 with an app-SRT, `efLib_CreateGenerator_AddAppSRT`.
#[doc(alias = "efLib_CreateGenerator_AddAppSRT")]
pub fn create_generator_appsrt(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    gfx_id: i32,
) -> Option<GeneratorId> {
    let g = generator::spawn(objs, data, hooks, 0, gfx_id / 1000, gfx_id)?;
    ensure_appsrt(objs, g, 1);
    Some(g)
}

/// A generator with an app-SRT at `translation`, turned to a facing (`direction`), marked
/// mirrored when facing left, `efLib_CreateGenerator_Translate_FacingDir`.
#[doc(alias = "efLib_CreateGenerator_Translate_FacingDir")]
pub fn create_generator_facing(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    gfx_id: i32,
    translation: Vec3,
    direction: f32,
) -> Option<GeneratorId> {
    let g = create_generator_appsrt(objs, data, hooks, gfx_id)?;
    let s = objs.generators[g].appsrt.expect("an app-SRT");
    let srt = &mut objs.app_srts[s];
    srt.translate.x = copy(translation.x);
    srt.translate.y = copy(translation.y);
    srt.translate.z = copy(translation.z);
    if reg(direction) < 0.0 {
        objs.generators[g].kind |= 0x40000;
        objs.app_srts[s].rot.y = -HALF_PI;
    } else {
        objs.app_srts[s].rot.y = HALF_PI;
    }
    Some(g)
}

/// A generator following a joint, emitting unrotated, `efLib_CreateGenerator_Attach`.
#[doc(alias = "efLib_CreateGenerator_Attach")]
pub fn create_generator_attach(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    gfx_id: i32,
    jobj: Option<JObjId>,
) -> Option<GeneratorId> {
    let g = generator::spawn_on_jobj(objs, data, hooks, 0, gfx_id / 1000, gfx_id, jobj)?;
    objs.generators[g].ty &= !generator::JOINT_ROTATION;
    Some(g)
}

/// A generator following a joint with an app-SRT the joint drives, moving its translation
/// instead of the generator (`efLib_CreateGenerator_Attach_AddAppSRT`, and the inline
/// `eflib_create_generator_add_appsrt`).
#[doc(alias = "efLib_CreateGenerator_Attach_AddAppSRT")]
pub fn create_generator_attach_appsrt(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    gfx_id: i32,
    jobj: Option<JObjId>,
) -> Option<GeneratorId> {
    let g = generator::spawn_on_jobj(objs, data, hooks, 0, gfx_id / 1000, gfx_id, jobj)?;
    let s = ensure_appsrt(objs, g, 0);
    objs.app_srts[s].gp = Some(g);
    let o = &mut objs.generators[g];
    o.ty &= !(generator::JOINT_POSITION | generator::JOINT_ROTATION);
    o.ty |= generator::JOINT_APP_TRANSLATE;
    Some(g)
}

/// Sets all three of a generator's app-SRT scales.
fn set_appsrt_scale(objs: &mut Objects, g: GeneratorId, s: f32) {
    let srt = objs.generators[g].appsrt.expect("an app-SRT");
    let s = copy(s);
    let sc = &mut objs.app_srts[srt].scale;
    sc.z = s;
    sc.y = s;
    sc.x = s;
}

/// [`create_generator_attach_appsrt`] on the joint the arguments give, at the spawner's y
/// scale, `efLib_CreateGenerator_Attach_Scale`.
#[doc(alias = "efLib_CreateGenerator_Attach_Scale")]
pub fn create_generator_attach_scale(
    r: &mut Running,
    gfx_id: i32,
    args: &mut super::Args,
    gobj: Option<GObjId>,
) -> Option<GeneratorId> {
    let j = args.jobj();
    let g =
        create_generator_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, j)?;
    let s = spawner_scale(r.game, gobj);
    set_appsrt_scale(&mut r.game.objs, g, s.y);
    Some(g)
}

/// [`create_generator_attach_appsrt`] on the joint the arguments give, at the scale they give
/// next, `efLib_CreateGenerator_AppSRT_SetScale`.
#[doc(alias = "efLib_CreateGenerator_AppSRT_SetScale")]
pub fn create_generator_appsrt_scale(
    r: &mut Running,
    gfx_id: i32,
    args: &mut super::Args,
) -> Option<GeneratorId> {
    let j = args.jobj();
    let g =
        create_generator_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, j)?;
    let s = args.float();
    set_appsrt_scale(&mut r.game.objs, g, s);
    Some(g)
}

/// Sets a generator's app-SRT turn for a facing.
fn set_appsrt_facing(objs: &mut Objects, g: GeneratorId, facing: f32) {
    let s = objs.generators[g].appsrt.expect("an app-SRT");
    objs.app_srts[s].rot.y = facing_rot(facing);
}

/// [`create_generator_attach_appsrt`] on the joint the arguments give, turned to the facing
/// they give next, `efLib_CreateGenerator_AppSRT_SetFacingDir`.
#[doc(alias = "efLib_CreateGenerator_AppSRT_SetFacingDir")]
pub fn create_generator_appsrt_facing(
    r: &mut Running,
    gfx_id: i32,
    args: &mut super::Args,
) -> Option<GeneratorId> {
    let j = args.jobj();
    let g =
        create_generator_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, j)?;
    let facing = args.float();
    set_appsrt_facing(&mut r.game.objs, g, facing);
    Some(g)
}

/// [`create_generator_appsrt_facing`], then the scale the arguments give,
/// `efLib_CreateGenerator_AppSRT_SetFacingDirScale`.
#[doc(alias = "efLib_CreateGenerator_AppSRT_SetFacingDirScale")]
pub fn create_generator_appsrt_facing_scale(
    r: &mut Running,
    gfx_id: i32,
    args: &mut super::Args,
) -> Option<GeneratorId> {
    let j = args.jobj();
    let g =
        create_generator_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, j)?;
    let facing = args.float();
    set_appsrt_facing(&mut r.game.objs, g, facing);
    let s = args.float();
    set_appsrt_scale(&mut r.game.objs, g, s);
    Some(g)
}

/// The generator for a joint's particle entry or event (bank and index), as efLib makes it,
/// `efLib_SpawnParticleEffect`: some ids follow the joint with an app-SRT that takes their
/// tree's facing, scale or position, some stand alone at the tree's position, and the rest
/// follow the joint plainly (`dynamic`) or oriented by it.
#[doc(alias = "efLib_SpawnParticleEffect")]
pub fn spawn_particle_effect(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    bank: i32,
    gfx_id: i32,
    jobj: JObjId,
    dynamic: bool,
) {
    let mut root = jobj;
    let mut link = 0;
    match gfx_id {
        0x4A38 | 0x4A39 | 0x4A3A | 0xB798 | 0xB799 | 0xB79A => {
            if let Some(g) = create_generator_attach_appsrt(objs, data, hooks, gfx_id, Some(jobj)) {
                root = root_of(objs, root);
                let s = objs.generators[g].appsrt.expect("an app-SRT");
                objs.app_srts[s].rot.y = copy(objs.jobjs[root].rotate.y);
            }
            return;
        }
        0x2D | 0x2E | 0x31 => {
            if let Some(g) = create_generator_attach_appsrt(objs, data, hooks, gfx_id, Some(jobj)) {
                objs.generators[g].ty |= generator::JOINT_APP_SCALE;
            }
            return;
        }
        0x127 => {
            if let Some(g) = create_generator_attach_appsrt(objs, data, hooks, gfx_id, Some(jobj)) {
                root = root_of(objs, root);
                let s = objs.generators[g].appsrt.expect("an app-SRT");
                objs.app_srts[s].rot.y = copy(objs.jobjs[root].rotate.y);
                objs.app_srts[s].scale = objs.jobjs[root].scale;
            }
            return;
        }
        0x2 | 0x6 | 0xA3 | 0xA7 | 0xAA | 0xF2 | 0x12A | 0x132 | 0x133 => {
            if let Some(g) = create_generator_attach_appsrt(objs, data, hooks, gfx_id, Some(jobj)) {
                root = root_of(objs, root);
                let s = objs.generators[g].appsrt.expect("an app-SRT");
                objs.app_srts[s].scale = objs.jobjs[root].scale;
            }
            return;
        }
        0x16D | 0x16E | 0x16F | 0x170 | 0x7E2 => {
            if let Some(g) = generator::spawn(objs, data, hooks, 0, gfx_id / 1000, gfx_id) {
                let s = ensure_appsrt(objs, g, 1);
                root = root_of(objs, root);
                objs.app_srts[s].rot.y = copy(objs.jobjs[root].rotate.y);
                objs.app_srts[s].translate = objs.jobjs[root].translate;
            }
            return;
        }
        0xD4 | 0x243 => {
            if let Some(g) = generator::spawn(objs, data, hooks, 2, bank, gfx_id) {
                let s = ensure_appsrt(objs, g, 1);
                root = root_of(objs, root);
                objs.app_srts[s].rot.z = copy(objs.jobjs[root].rotate.z);
                objs.app_srts[s].translate = objs.jobjs[root].translate;
                if gfx_id == 0xD4 {
                    objs.app_srts[s].scale = objs.jobjs[root].scale;
                }
            }
            return;
        }
        0xE3 => {
            if let Some(g) = generator::spawn(objs, data, hooks, 0, gfx_id / 1000, gfx_id) {
                let s = ensure_appsrt(objs, g, 1);
                let t = crate::lb::joint::world_position(objs, data, Some(jobj), None);
                objs.app_srts[s].translate = t;
                root = root_of(objs, root);
                objs.app_srts[s].scale = objs.jobjs[root].scale;
            }
            return;
        }
        0xFC | 0xFF | 0xF7 | 0x7918 => link = 1,
        _ => {}
    }
    if dynamic {
        generator::spawn_on_jobj(objs, data, hooks, link, bank, gfx_id, Some(root));
    } else {
        generator::spawn_oriented_on_jobj(objs, data, hooks, link, bank, gfx_id, Some(root));
    }
}

/// The stage's effect bank: joints' particles from it are the stage's (`grLib_801C99C0`).
const STAGE_BANK: i32 = 0x1E;

/// A joint's particle list entry, spawned the first time it is displayed, `efLib_Cb_SPtcl`.
#[doc(alias = "efLib_Cb_SPtcl")]
pub fn cb_sptcl(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    bank: i32,
    gfx_id: i32,
    jobj: JObjId,
) {
    if bank == STAGE_BANK {
        super::super::gr::grlib::joint_particles(objs, data, hooks, bank, gfx_id, jobj, false);
        return;
    }
    spawn_particle_effect(objs, data, hooks, bank, gfx_id, jobj, false);
}

/// A joint's animated particle event, `efLib_Cb_DPtcl`.
#[doc(alias = "efLib_Cb_DPtcl")]
pub fn cb_dptcl(
    objs: &mut Objects,
    data: &Archives,
    hooks: &mut dyn Hooks,
    bank: i32,
    gfx_id: i32,
    jobj: JObjId,
) {
    if bank == STAGE_BANK {
        super::super::gr::grlib::joint_particles(objs, data, hooks, bank, gfx_id, jobj, true);
        return;
    }
    spawn_particle_effect(objs, data, hooks, bank, gfx_id, jobj, true);
}

/// A new particle of a generator with [`PARTICLE_RENDER_FUNCS`] trades the app-SRT it shares
/// for a copy of its own, held once its matrix is made, `efLib_Cb_ParticleRender`.
#[doc(alias = "efLib_Cb_ParticleRender")]
pub fn cb_particle_render(objs: &mut Objects, p: ParticleId) {
    let Some(s) = objs.particles[p].appsrt else {
        return;
    };
    let o = objs.app_srts[s].clone();
    appsrt::remove_from_particle(objs, p);
    let n = appsrt::add_to_particle(objs, p, 1);
    let srt = &mut objs.app_srts[n];
    srt.translate = o.translate;
    srt.rot = o.rot;
    srt.scale = o.scale;
}

/// Gives generators of four common command lists [`PARTICLE_RENDER_FUNCS`],
/// `efLib_Cb_PtclAppSRTHook`.
#[doc(alias = "efLib_Cb_PtclAppSRTHook")]
pub fn cb_ptcl_appsrt_hook(objs: &mut Objects, data: &Archives, g: GeneratorId) {
    let b = objs.particles.banks[0].expect("ptclref_804D0E5C[0]: the common bank");
    for idx in [0x96, 0x97, 0x98, 0x21B] {
        let cl = b
            .cmd_list(data, idx)
            .expect("ptclref_804D0E5C[0][idx]->cmdList");
        if objs.generators[g].cmd_list == Some(cl.list()) {
            generator::set_user_func(objs, g, Some(PARTICLE_RENDER_FUNCS));
        }
    }
}

/// Moves the effect to its joint's position, raised by `params.y`,
/// `efLib_Cb_SetOffsetY_FromParamY`.
#[doc(alias = "efLib_Cb_SetOffsetY_FromParamY")]
pub fn cb_set_offset_y_from_param_y(r: &mut Running, e: EffectId) {
    let ej = effect_jobj(r.game, e);
    let ef = &r.game.ef.effects[e.0];
    let (attach, py) = (ef.attach_jobj, ef.params.y);
    let mut t = crate::lb::joint::world_position(&mut r.game.objs, r.data, attach, None);
    t.y = mem(fp::fadds(reg(t.y), reg(py)));
    jobj::set_translate(&mut r.game.objs.jobjs, ej.expect("jobj"), t);
}

/// Scales the effect by `params.x` each frame, `efLib_Cb_SetScale_FromParamX`.
#[doc(alias = "efLib_Cb_SetScale_FromParamX")]
pub fn cb_set_scale_from_param_x(game: &mut Game, e: EffectId) {
    let j = ejobj(game, e);
    let px = reg(game.ef.effects[e.0].params.x);
    let jobjs = &mut game.objs.jobjs;
    let x = mem(fp::fmuls(px, reg(jobjs[j].scale.x)));
    jobj::set_srt(jobjs, j, |o| o.scale.x = x);
    let y = mem(fp::fmuls(px, reg(jobjs[j].scale.y)));
    jobj::set_srt(jobjs, j, |o| o.scale.y = y);
    let z = mem(fp::fmuls(px, reg(jobjs[j].scale.z)));
    jobj::set_srt(jobjs, j, |o| o.scale.z = z);
}

/// Turns the effect to its fighter's facing; when it expires, plays its ending from frame 65
/// for 11 frames, else loops its last stretch from frame 60, `efLib_Cb_SetRotYAndTransition`.
#[doc(alias = "efLib_Cb_SetRotYAndTransition")]
pub fn cb_set_rot_y_and_transition(r: &mut Running, e: EffectId) {
    let j = ejobj(r.game, e);
    if let Some(f) = r.game.ef.effects[e.0].user_data {
        let facing = r.game.fighters[f.0].facing_dir;
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, facing_rot(facing));
    }
    let ef = &mut r.game.ef.effects[e.0];
    if ef.expire_flags != 0 {
        ef.lifetime = 11;
        ef.update = None;
        jobj::req_anim_all(&mut r.game.objs, Some(j), 65.0);
        return;
    }
    if ef.lifetime == 1 {
        ef.lifetime = 6;
        jobj::req_anim_all(&mut r.game.objs, Some(j), 60.0);
    }
}

/// Moves the effect's child 2 further along z each frame, `efLib_Cb_SetJObjOffsetZ`.
#[doc(alias = "efLib_Cb_SetJObjOffsetZ")]
pub fn cb_set_jobj_offset_z(game: &mut Game, e: EffectId) {
    let child = effect_jobj(game, e)
        .and_then(|j| game.objs.jobjs[j].child)
        .expect("jobj");
    let jobjs = &mut game.objs.jobjs;
    let z = mem(fp::fadds(2.0, reg(jobjs[child].translate.z)));
    jobj::set_srt(jobjs, child, |o| o.translate.z = z);
}

/// The spawning fighter's facing.
fn parent_facing(game: &Game, e: EffectId) -> f32 {
    let p = game.ef.effects[e.0].parent.expect("an effect's parent");
    game.fighter(p).facing_dir
}

/// Turns the effect to its fighter's facing, `efLib_Cb_SetRotY_FromFighterDir`.
#[doc(alias = "efLib_Cb_SetRotY_FromFighterDir")]
pub fn cb_set_rot_y_from_fighter_dir(game: &mut Game, e: EffectId) {
    let j = ejobj(game, e);
    let rot = facing_rot(parent_facing(game, e));
    jobj::set_rotation_y(&mut game.objs.jobjs, j, rot);
}

/// Turns the effect to its fighter's facing and tilts it by the angle the fighter's action
/// state keeps (`mv.co.common.x4.x`), `efLib_Cb_SetRotYZ_FromFighter`.
#[doc(alias = "efLib_Cb_SetRotYZ_FromFighter")]
pub fn cb_set_rot_yz_from_fighter(game: &mut Game, e: EffectId) {
    let p = game.ef.effects[e.0].parent.expect("an effect's parent");
    let fp_ = game.fighter(p);
    let angle = f32::from_be_bytes(fp_.mv[4..8].try_into().unwrap());
    let facing = fp_.facing_dir;
    let j = ejobj(game, e);
    let tilt = fp::fsubs(reg(HALF_PI), reg(angle));
    let jobjs = &mut game.objs.jobjs;
    if reg(facing) < 0.0 {
        jobj::set_rotation_y(jobjs, j, -HALF_PI);
        jobj::set_rotation_z(jobjs, j, mem(tilt));
    } else {
        jobj::set_rotation_y(jobjs, j, HALF_PI);
        jobj::set_rotation_z(jobjs, j, mem(fp::fneg(tilt)));
    }
}

/// Falls: down by `params.y`, which grows 0.2 each frame, along its turn in x and z, and
/// tumbling about x, `efLib_Cb_Fall_FromParamY`.
#[doc(alias = "efLib_Cb_Fall_FromParamY")]
pub fn cb_fall_from_param_y(game: &mut Game, e: EffectId) {
    let j = ejobj(game, e);
    let o = &game.objs.jobjs[j];
    let (tx, ty, tz) = (reg(o.translate.x), reg(o.translate.y), reg(o.translate.z));
    let (rx, ry) = (reg(o.rotate.x), reg(o.rotate.y));
    let ef = &mut game.ef.effects[e.0];
    ef.params.y = mem(fp::fsubs(reg(ef.params.y), reg(0.2)));
    let py = reg(ef.params.y);
    let x = mem(fp::fadds(tx, reg(math::sin(mem(ry)))));
    let jobjs = &mut game.objs.jobjs;
    jobj::set_srt(jobjs, j, |o| o.translate.x = x);
    let y = mem(fp::fadds(ty, py));
    jobj::set_srt(jobjs, j, |o| o.translate.y = y);
    let z = mem(fp::fadds(tz, reg(math::cos(mem(ry)))));
    jobj::set_srt(jobjs, j, |o| o.translate.z = z);
    jobj::set_rotation_x(jobjs, j, mem(fp::fadds(0.5, rx)));
}

/// Flies by `params`, `params.y` dropping 0.1 each frame, `efLib_Cb_SetOffset_FromParams`.
#[doc(alias = "efLib_Cb_SetOffset_FromParams")]
pub fn cb_set_offset_from_params(game: &mut Game, e: EffectId) {
    let j = ejobj(game, e);
    let o = &game.objs.jobjs[j];
    let (tx, ty, tz) = (reg(o.translate.x), reg(o.translate.y), reg(o.translate.z));
    let ef = &mut game.ef.effects[e.0];
    ef.params.y = mem(fp::fsubs(reg(ef.params.y), reg(0.1)));
    let p = ef.params;
    let jobjs = &mut game.objs.jobjs;
    let x = mem(fp::fadds(tx, reg(p.x)));
    jobj::set_srt(jobjs, j, |o| o.translate.x = x);
    let y = mem(fp::fadds(ty, reg(p.y)));
    jobj::set_srt(jobjs, j, |o| o.translate.y = y);
    let z = mem(fp::fadds(tz, reg(p.z)));
    jobj::set_srt(jobjs, j, |o| o.translate.z = z);
}

/// On its last frame, spawns common particle 0x1AB on its joint and lives 39 more frames,
/// `efLib_Cb_LifetimeEndSpawn`.
#[doc(alias = "efLib_Cb_LifetimeEndSpawn")]
pub fn cb_lifetime_end_spawn(r: &mut Running, e: EffectId) {
    if r.game.ef.effects[e.0].lifetime != 1 {
        return;
    }
    let attach = r.game.ef.effects[e.0].attach_jobj;
    generator::spawn_on_jobj(
        &mut r.game.objs,
        r.data,
        &mut ParticleHooks,
        0,
        0,
        0x1AB,
        attach,
    );
    let ef = &mut r.game.ef.effects[e.0];
    ef.update = None;
    ef.lifetime = 39;
}

/// Scales the effect by its fighter's scale and turns it to its facing,
/// `efLib_Cb_SetScaleRotY_FromFighter`.
#[doc(alias = "efLib_Cb_SetScaleRotY_FromFighter")]
pub fn cb_set_scale_rot_y_from_fighter(game: &mut Game, e: EffectId) {
    let p = game.ef.effects[e.0].parent.expect("an effect's parent");
    let pj = jobj_of(game, p).expect("HSD_JObjGetScale: jobj");
    let j = ejobj(game, e);
    let jobjs = &mut game.objs.jobjs;
    let (s1, s2) = (jobjs[pj].scale, jobjs[j].scale);
    let m = |a: f32, b: f32| mem(fp::fmuls(reg(a), reg(b)));
    jobj::set_scale(
        jobjs,
        j,
        Vec3::new(m(s1.x, s2.x), m(s1.y, s2.y), m(s1.z, s2.z)),
    );
    let rot = facing_rot(game.fighter(p).facing_dir);
    jobj::set_rotation_y(&mut game.objs.jobjs, j, rot);
}

/// Turns the effect to its fighter's facing and tilts it by `params.z`, mirrored when facing
/// right, `efLib_Cb_SetRotYZ_FromParamZ_FighterDir`.
#[doc(alias = "efLib_Cb_SetRotYZ_FromParamZ_FighterDir")]
pub fn cb_set_rot_yz_from_param_z(game: &mut Game, e: EffectId) {
    let j = ejobj(game, e);
    let facing = parent_facing(game, e);
    let pz = game.ef.effects[e.0].params.z;
    let jobjs = &mut game.objs.jobjs;
    if reg(facing) < 0.0 {
        jobj::set_rotation_y(jobjs, j, -HALF_PI);
        jobj::set_rotation_z(jobjs, j, copy(pz));
    } else {
        jobj::set_rotation_y(jobjs, j, HALF_PI);
        jobj::set_rotation_z(jobjs, j, mem(fp::fneg(reg(pz))));
    }
}

/// Where Mario's and Luigi's down special keep whether they're grounded on a slope
/// (`mv.mr.SpecialLw.isUnkColl`, `mv.lg.SpecialLw.isUnkColl`), and where Koopa's up special
/// keeps its own (`mv.co.common.x10`): byte offsets into `mv`.
const MARIO_SPECIAL_LW_COLL: usize = 0xC;
const LUIGI_SPECIAL_LW_COLL: usize = 0xC;
const KOOPA_SPECIAL_HI_COLL: usize = 0x10;

/// A word of a fighter's action state variables (`mv`).
fn mv_word(fp_: &super::super::structs::Fighter, at: usize) -> u32 {
    u32::from_be_bytes(fp_.mv[at..at + 4].try_into().unwrap())
}

/// The floor's tilt under a fighter, `-atan2f(normal.x, normal.y)`.
fn floor_tilt(fp_: &super::super::structs::Fighter) -> f32 {
    let n = fp_.coll_data.floor.normal;
    mem(fp::fneg(reg(math::atan2(n.x, n.y))))
}

/// The effect's second model shows only in the fighter's down special (`motion`), and tilts
/// with the floor while the fighter rides it, `efLib_Cb_ftMr_SpecialLw` and
/// `efLib_Cb_ftLg_SpecialLw`.
#[doc(alias = "efLib_Cb_ftMr_SpecialLw")]
#[doc(alias = "efLib_Cb_ftLg_SpecialLw")]
pub fn cb_special_lw(game: &mut Game, e: EffectId, motion: i32, coll_at: usize) {
    let ej = effect_jobj(game, e);
    let second = ej
        .and_then(|j| game.objs.jobjs[j].child)
        .and_then(|c| game.objs.jobjs[c].next);
    let p = game.ef.effects[e.0].parent.expect("an effect's parent");
    let fp_ = game.fighter(p);
    let shown = fp_.motion_id == motion;
    let tilt = (fp_.cmd_vars[3] != 0 && mv_word(fp_, coll_at) != 0).then(|| floor_tilt(fp_));
    let jobjs = &mut game.objs.jobjs;
    if shown {
        jobj::clear_flags_all(jobjs, second, jobj::HIDDEN);
    } else {
        jobj::set_flags_all(jobjs, second, jobj::HIDDEN);
    }
    jobj::set_rotation_z(jobjs, ej.expect("jobj"), tilt.unwrap_or(0.0));
}

/// Koopa's up special: the effect's partner (`next`) shows only in motion 359, and both tilt
/// with the floor, `efLib_Cb_ftKp_SpecialHi`.
#[doc(alias = "efLib_Cb_ftKp_SpecialHi")]
pub fn cb_koopa_special_hi(game: &mut Game, e: EffectId) {
    let j1 = ejobj(game, e);
    let next = game.ef.effects[e.0].next.expect("effect->next");
    let j2 = ejobj(game, next);
    let p = game.ef.effects[e.0].parent.expect("an effect's parent");
    let fp_ = game.fighter(p);
    let shown = fp_.motion_id == 359;
    let tilt = (fp_.cmd_vars[2] & 1 != 0 && mv_word(fp_, KOOPA_SPECIAL_HI_COLL) != 0)
        .then(|| floor_tilt(fp_));
    let jobjs = &mut game.objs.jobjs;
    if shown {
        jobj::clear_flags_all(jobjs, Some(j2), jobj::HIDDEN);
    } else {
        jobj::set_flags_all(jobjs, Some(j2), jobj::HIDDEN);
    }
    let z = tilt.unwrap_or(0.0);
    jobj::set_rotation_z(jobjs, j1, z);
    jobj::set_rotation_z(jobjs, j2, z);
}

/// The bury effect's update, `efLib_Cb_ftCo_Bury`: the fighter code's (`ftCo_800C0FCC`).
#[doc(alias = "efLib_Cb_ftCo_Bury")]
pub fn cb_bury(r: &mut Running, e: EffectId) {
    let ef = &r.game.ef.effects[e.0];
    let (g, p) = (ef.gobj, ef.parent);
    super::super::code::call(
        r,
        0x800C_0FCC,
        &[
            super::super::code::Val::GObj(g),
            super::super::code::Val::GObj(p),
        ],
    );
}

/// The `count`th texture after a joint's first material's first.
fn nth_tobj(objs: &Objects, j: Option<JObjId>, count: i32) -> crate::hsd::tobj::TObjId {
    let d = jobj::get_dobj(&objs.jobjs, j);
    let m = d.and_then(|d| objs.dobjs[d].mobj);
    let mut t = mobj::tobj(&objs.mobjs, m);
    for _ in 0..count {
        t = objs.tobjs[t.expect("tobj->next")].next;
    }
    t.expect("tobj->tev")
}

/// Sets a texture's TEV constant and first register colors (0xRRGGBB),
/// `efLib_SetTevKonstColor`.
#[doc(alias = "efLib_SetTevKonstColor")]
pub fn set_tev_konst_color(game: &mut Game, j: Option<JObjId>, count: i32, konst: u32, tev0: u32) {
    let t = nth_tobj(&game.objs, j, count);
    let tev = game.objs.tobjs[t].tev.as_mut().expect("tobj->tev");
    tev.konst.r = (konst >> 16) as u8;
    tev.konst.g = (konst >> 8) as u8;
    tev.konst.b = konst as u8;
    tev.tev0.r = (tev0 >> 16) as u8;
    tev.tev0.g = (tev0 >> 8) as u8;
    tev.tev0.b = tev0 as u8;
}

/// The stored parameters' entry for `gobj`, else the first free one.
fn param_entry(game: &mut Game, gobj: Option<GObjId>) -> Option<&mut ParamEntry> {
    let p = &mut game.ef.params;
    let i = p
        .iter()
        .position(|e| e.gobj == gobj)
        .or_else(|| p.iter().position(|e| e.gobj.is_none()))?;
    Some(&mut p[i])
}

/// Stores an alpha for `gobj`'s effects, `efLib_SetParamAlpha`.
#[doc(alias = "efLib_SetParamAlpha")]
pub fn set_param_alpha(game: &mut Game, gobj: Option<GObjId>, alpha: u8) {
    if let Some(e) = param_entry(game, gobj) {
        e.gobj = gobj;
        e.alpha = u16::from(alpha);
    }
}

/// Stores an effect id for `gobj`'s effects, `efLib_SetParamGfxId`.
#[doc(alias = "efLib_SetParamGfxId")]
pub fn set_param_gfx_id(game: &mut Game, gobj: Option<GObjId>, gfx_id: i32) {
    if let Some(e) = param_entry(game, gobj) {
        e.gobj = gobj;
        e.gfx_id = gfx_id as u16;
    }
}

/// Applies the alpha stored for the effect's spawner to its model's TEV constant (a later
/// texture for id 0x417, and six more joints' for 0x419), `efLib_Cb_ApplyStoredAlpha`.
#[doc(alias = "efLib_Cb_ApplyStoredAlpha")]
pub fn cb_apply_stored_alpha(game: &mut Game, e: EffectId) {
    let mut j = effect_jobj(game, e).and_then(|j| game.objs.jobjs[j].child);
    let parent = game.ef.effects[e.0].parent;
    let Some(entry) = game.ef.params.iter().copied().find(|p| p.gobj == parent) else {
        return;
    };
    let objs = &mut game.objs;
    let t = nth_tobj(objs, j, i32::from(entry.gfx_id == 0x417));
    objs.tobjs[t].tev.as_mut().expect("tobj->tev").konst.a = entry.alpha as u8;
    if entry.gfx_id == 0x419 {
        for _ in 0..6 {
            j = objs.jobjs[j.expect("jobj->next")].next;
            let t = nth_tobj(objs, j, 0);
            objs.tobjs[t].tev.as_mut().expect("tobj->tev").konst.a = entry.alpha as u8;
        }
    }
}

/// Moves the effect to its joint's position plus `params`, `efLib_Cb_AccumOffset_FromParams`.
#[doc(alias = "efLib_Cb_AccumOffset_FromParams")]
pub fn cb_accum_offset_from_params(r: &mut Running, e: EffectId) {
    let ef = &r.game.ef.effects[e.0];
    let (attach, p) = (ef.attach_jobj, ef.params);
    let mut t = crate::lb::joint::world_position(&mut r.game.objs, r.data, attach, None);
    t.x = mem(fp::fadds(reg(t.x), reg(p.x)));
    t.y = mem(fp::fadds(reg(t.y), reg(p.y)));
    t.z = mem(fp::fadds(reg(t.z), reg(p.z)));
    let j = effect_jobj(r.game, e).expect("HSD_JObjSetTranslate: jobj");
    jobj::set_translate(&mut r.game.objs.jobjs, j, t);
}

/// Effect 0 following a joint at an offset, with a generator of `gfx_id` on its model,
/// `efLib_CreateGenerator_AppSRT_SetPos`.
#[doc(alias = "efLib_CreateGenerator_AppSRT_SetPos")]
pub fn create_generator_appsrt_pos(
    r: &mut Running,
    gfx_id: i32,
    gobj: Option<GObjId>,
    jobj: Option<JObjId>,
    offset: Vec3,
) -> Option<EffectId> {
    let e = create(r, 0, gobj)?;
    let ef = &mut r.game.ef.effects[e.0];
    ef.update = Some(Update::AccumOffsetFromParams);
    ef.attach_jobj = jobj;
    ef.params = offset;
    let ej = effect_jobj(r.game, e);
    create_generator_attach_appsrt(&mut r.game.objs, r.data, &mut ParticleHooks, gfx_id, ej);
    Some(e)
}

/// Animates the joints effects made during a spawn queued, last first (the loop that ends
/// `efSync_Spawn`, `efAsync_Dispatch` and `efAlt_Spawn`).
pub(super) fn drain_anim_queue(r: &mut Running) {
    while r.game.ef.anim_count != 0 {
        r.game.ef.anim_count -= 1;
        let j = r.game.ef.anim_queue[r.game.ef.anim_count as usize];
        jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, j);
    }
}

/// The game's particle hooks for efLib's callbacks: `efLib_Cb_ParticleRender` for
/// [`PARTICLE_RENDER_FUNCS`]' particles and `efLib_Cb_PtclAppSRTHook` on new generators.
pub(crate) fn hook_create(objs: &mut Objects, f: UserFunc, p: ParticleId) {
    match f {
        PARTICLE_RENDER_FUNCS => cb_particle_render(objs, p),
        other => panic!(
            "particle user functions {:#010x} aren't the game's",
            other.0
        ),
    }
}

pub(crate) fn generator_created(
    objs: &mut Objects,
    data: &Archives,
    h: GeneratorHook,
    g: GeneratorId,
) {
    match h {
        PTCL_APPSRT_HOOK => cb_ptcl_appsrt_hook(objs, data, g),
        other => panic!("generator hook {:#010x} isn't the game's", other.0),
    }
}

/// The effect a GObj holds, for code that keeps effects by GObj.
pub fn of(game: &Game, gobj: GObjId) -> &Effect {
    game.effect(game.effect_id(gobj))
}
