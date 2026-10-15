//! Fox Illusion's and Falco Phantasm's afterimage, `melee/it/kinds/itfoxillusion.c`: an item
//! that follows the dash's trail, its owner's ghost drawn behind it from the second half of
//! the dash, until the dash ends or its lifetime runs out.

use super::super::item::{article, change_state, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::game::state::{Game, Running};
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

pub const MOTION0_ANIM: u32 = 0x8029_D094;
pub const MOTION0_PHYS: u32 = 0x8029_D24C;
pub const MOTION0_COLL: u32 = 0x8029_D488;
pub const MOTION1_ANIM: u32 = 0x8029_D490;
pub const MOTION1_PHYS: u32 = 0x8029_D554;
pub const MOTION1_COLL: u32 = 0x8029_D790;
pub const MOTION2_ANIM: u32 = 0x8029_D7EC;
pub const MOTION2_PHYS: u32 = 0x8029_D870;
pub const MOTION2_COLL: u32 = 0x8029_D940;
pub const DMG_DEALT: u32 = 0x8029_CF8C;
pub const DESTROYED: u32 = 0x8029_CFA0;
pub const EVT_UNK: u32 = 0x8029_D948;
/// `it_8029CD18`, the kind's render callback.
pub const RENDER: u32 = 0x8029_CD18;

/// `FoxIllusionAttr` offsets: the lifetime while following and while fading.
const LIFETIME: u32 = 0;
const FADE_LIFETIME: u32 = 4;

/// Runs one of the afterimage's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION0_ANIM | MOTION1_ANIM => follow(r, gobj),
        MOTION2_ANIM => fade(r, gobj),
        MOTION0_COLL | MOTION1_COLL | MOTION2_COLL => false,
        DMG_DEALT => {
            // `itFoxIllusion_Logic14_DmgDealt`.
            item_mut(r.game, gobj).xCA8 = 0;
            false
        }
        _ => return None,
    })
}

/// Runs one of the afterimage's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        MOTION0_PHYS | MOTION1_PHYS => follow_trail(r, gobj),
        MOTION2_PHYS => fade_trail(r, gobj),
        DESTROYED => destroyed(r, gobj),
        _ => return false,
    }
    true
}

/// The owner's ghost joint tree, if it has one yet.
fn ghost(game: &mut Game, gobj: GObjId) -> Option<JObjId> {
    item_mut(game, gobj).xDD4_itemVar.fox_illusion().xDDC.get()
}

/// The afterimage behind a dash, `it_8029CEB4`: made at `pos` facing `facing_dir`, for
/// `parent`.
#[doc(alias = "it_8029CEB4")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, off the plane.
    spawn.prev_pos = pos;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    setup(r, gobj);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    super::super::lib2725::first_frame(r, gobj);
    Some(gobj)
}

/// The afterimage's start, `it_8029CFF0`: its lifetime and ghost model from its attributes,
/// not to be picked up, its state by whether the owner is in the air, sized as the owner
/// (`it_80274594`), no ghost yet.
#[doc(alias = "it_8029CFF0")]
fn setup(r: &mut Running, gobj: GObjId) {
    let art = article(item(r.game, gobj));
    let attrs = art
        .special(r.data)
        .expect("an afterimage without attributes");
    let lifetime = r.data.f32(attrs.at(LIFETIME));
    let joint = art.model(r.data).joint(r.data);
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = lifetime;
    ip.xD5C = 0;
    ip.xDD4_itemVar.fox_illusion().xDD4.set(joint);
    lib26b1::disable_pickup(r.game, gobj);
    let owner = item(r.game, gobj)
        .owner
        .expect("an afterimage without an owner");
    // `ftLib_GetGroundAir`.
    let air = super::super::owner::fighter(r.game, owner).ground_or_air == 1;
    let flags = super::super::item::change::ANIM_UPDATE;
    change_state(r, gobj, if air { 1 } else { 0 }, flags);
    lib2725::scale_by_owner(r, gobj);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .fox_illusion()
        .xDDC
        .set(None);
}

/// The owner's ghost, `it_8029CD78`: its model loaded, turned as the afterimage and sized as
/// it.
#[doc(alias = "it_8029CD78")]
fn make_ghost(r: &mut Running, gobj: GObjId) -> Option<JObjId> {
    let root = jobj_of(r.game, gobj).expect("an afterimage without a model");
    let v = item_mut(r.game, gobj).xDD4_itemVar.fox_illusion();
    assert!(v.xDDC.get().is_none(), "illusion add twice.");
    let joint = v.xDD4.get();
    let g = jobj::load_joint(&mut r.game.objs, r.data, joint)?;
    let (q, stale) = {
        let o = &r.game.objs.jobjs[root];
        (o.rotate, o.stale_w)
    };
    jobj::set_srt(&mut r.game.objs.jobjs, g, |o| {
        o.rotate = q;
        o.stale_w = stale;
    });
    let scl = item(r.game, gobj).scl;
    lib2725::set_uniform_scale(r.game, g, scl);
    Some(g)
}

/// Following the dash, `itFoxillusion_UnkMotion0_Anim` (and `UnkMotion1`): the ghost once
/// the dash's script says (`cmd_vars[2]` 2); gone when the dash is over; fading when its
/// lifetime runs out.
#[doc(alias = "itFoxillusion_UnkMotion0_Anim")]
// Ports `itFoxillusion_UnkMotion1_Anim`; aliased once a check reaches it.
fn follow(r: &mut Running, gobj: GObjId) -> bool {
    let owner = item(r.game, gobj).owner;
    if let Some(o) = owner
        && r.fox_special_s_cmd_var2(o) == 2
        && ghost(r.game, gobj).is_none()
    {
        let g = make_ghost(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .fox_illusion()
            .xDDC
            .set(g);
    }
    if owner.is_none_or(|o| r.fox_special_s_ghost_gone(o)) {
        return true;
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = crate::fpu::mem(gekko_fp::fsubs(crate::fpu::reg(ip.xD44_lifeTimer), 1.0));
    if ip.xD44_lifeTimer <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        start_fade(r, gobj);
    }
    false
}

/// The afterimage and its ghost along the dash's trail, `itFoxillusion_UnkMotion0_Phys` (and
/// `UnkMotion1`): the trail's second and fourth points and turns.
#[doc(alias = "itFoxillusion_UnkMotion0_Phys")]
// Ports `itFoxillusion_UnkMotion1_Phys`; aliased once a check reaches it.
fn follow_trail(r: &mut Running, gobj: GObjId) {
    let Some(owner) = item(r.game, gobj).owner else {
        return;
    };
    let pos = r.fox_special_s_ghost_pos(owner, 1);
    item_mut(r.game, gobj).pos = pos;
    let root = jobj_of(r.game, gobj).expect("an afterimage without a model");
    let rot = r.fox_special_s_blend_frame(owner, 1);
    jobj::set_rotation_x(&mut r.game.objs.jobjs, root, rot);
    if let Some(g) = ghost(r.game, gobj) {
        let p = r.fox_special_s_ghost_pos(owner, 3);
        jobj::set_translate(&mut r.game.objs.jobjs, g, p);
        let rot = r.fox_special_s_blend_frame(owner, 3);
        jobj::set_rotation_x(&mut r.game.objs.jobjs, g, rot);
    }
}

/// Fading, `it_8029D798`: the fade's lifetime, the afterimage hidden, state 2.
#[doc(alias = "it_8029D798")]
fn start_fade(r: &mut Running, gobj: GObjId) {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("an afterimage without attributes");
    let lifetime = r.data.f32(attrs.at(FADE_LIFETIME));
    item_mut(r.game, gobj).xD44_lifeTimer = lifetime;
    let root = jobj_of(r.game, gobj);
    lib2725::hide_tree(r.game, root);
    change_state(r, gobj, 2, super::super::item::change::ANIM_UPDATE);
}

/// `itFoxillusion_UnkMotion2_Anim`: gone when the dash is over or the fade's lifetime runs
/// out.
#[doc(alias = "itFoxillusion_UnkMotion2_Anim")]
fn fade(r: &mut Running, gobj: GObjId) -> bool {
    let owner = item(r.game, gobj).owner;
    if owner.is_none_or(|o| r.fox_special_s_ghost_gone(o)) {
        return true;
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = crate::fpu::mem(gekko_fp::fsubs(crate::fpu::reg(ip.xD44_lifeTimer), 1.0));
    if ip.xD44_lifeTimer <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        return true;
    }
    false
}

/// `itFoxillusion_UnkMotion2_Phys`: the ghost along the trail's fourth point.
#[doc(alias = "itFoxillusion_UnkMotion2_Phys")]
fn fade_trail(r: &mut Running, gobj: GObjId) {
    let Some(owner) = item(r.game, gobj).owner else {
        return;
    };
    if let Some(g) = ghost(r.game, gobj) {
        let p = r.fox_special_s_ghost_pos(owner, 3);
        jobj::set_translate(&mut r.game.objs.jobjs, g, p);
    }
}

/// `itFoxIllusion_Logic14_Destroyed`: the ghost goes, and the owner.
#[doc(alias = "itFoxIllusion_Logic14_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    let g = ghost(r.game, gobj);
    if g.is_some() {
        jobj::remove_all(&mut r.game.objs, g);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.fox_illusion().xDDC.set(None);
    ip.owner = None;
    ip.xDC8_word.flags.x13 = false;
}

/// Forgets a GObj the afterimage points to, `it_8029D948` (`it_8026B894`).
// Ports `it_8029D948`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

impl Running<'_> {
    /// The afterimage's render callback, `it_8029CD18`: the item's, then the ghost's tree in
    /// the pass.
    #[doc(alias = "it_8029CD18")]
    pub fn render_fox_illusion(&mut self, gobj: GObjId, pass: i32) {
        self.render_item(gobj, pass);
        if let Some(g) = ghost(self.game, gobj) {
            let mut hooks = crate::game::code::ParticleHooks;
            let trsp = crate::hsd::display::pass_trsp(pass);
            crate::hsd::display::jobj_disp_all(
                &mut self.game.objs,
                self.data,
                &mut hooks,
                Some(g),
                None,
                trsp,
                0,
            );
        }
    }
}
