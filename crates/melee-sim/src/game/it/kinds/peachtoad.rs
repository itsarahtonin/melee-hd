//! Toad, Peach's neutral special, `melee/it/kinds/itpeachtoad.c`: made in her hand when her
//! script says, shown from its seventh frame, releasing spores when an attack meets him (his
//! second state), and gone once she leaves the move.

use super::super::item::{animate, change, change_state, debug_owner_bubbles, destroy, spawn_air};
use super::super::{item, jobj_of, lib26b1, lib2725};
use crate::fpu::reg;
use crate::game::code::ParticleHooks;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802B_DEFC;
pub const PICKED_UP: u32 = 0x802B_DFE0;
pub const MOTION0_ANIM: u32 = 0x802B_E02C;
pub const MOTION1_ANIM: u32 = 0x802B_E168;
pub const EVT_UNK: u32 = 0x802B_E1F4;

/// `It_Kind_Peach_Toad`.
pub const KIND: i32 = 104;

/// Runs one of Toad's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION0_ANIM => standing(r, gobj),
        MOTION1_ANIM => releasing(r, gobj),
        _ => return None,
    })
}

/// Runs one of Toad's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// Makes Toad of `kind` for `parent` at `pos`, in its hand at bone `part`, `it_802BDE18`.
#[doc(alias = "it_802BDE18")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, keeping its depth.
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
    // `Item_AttachToParent`.
    super::super::item::pick_up(r, gobj, parent, part);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// The owner forgets Toad (`itpeachtoad_inline_1`): Peach's hooks, or Kirby's copy's.
fn owner_forgets(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if let Some(owner) = ip.owner {
        if ip.kind == KIND {
            r.peach_toad_gone(owner);
        } else {
            r.kb_pe_toad_gone(owner);
        }
    }
}

/// Whether Toad's owner has left the move (`itpeachtoad_inline_2`); with no owner, he has.
fn owner_done(r: &Running, gobj: GObjId) -> bool {
    let ip = item(r.game, gobj);
    match ip.owner {
        Some(owner) if ip.kind == KIND => r.peach_toad_done(owner),
        Some(owner) => r.kb_pe_done(owner),
        None => true,
    }
}

/// `itPeachToad_Logic91_Destroyed`: the owner forgets him.
#[doc(alias = "itPeachToad_Logic91_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    owner_forgets(r, gobj);
}

/// Takes Toad away, `it_802BDF40`: the owner forgets him, and he goes.
#[doc(alias = "it_802BDF40")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    owner_forgets(r, gobj);
    destroy(r, gobj);
}

/// Toad pauses in his owner's hitlag, `it_802BDFA0`.
#[doc(alias = "it_802BDFA0")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// Toad goes on after hitlag, `it_802BDFC0`.
#[doc(alias = "it_802BDFC0")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// `itPeachToad_Logic91_PickedUp`: his first state, animated, hidden, sized as his owner.
#[doc(alias = "itPeachToad_Logic91_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    animate(r, gobj);
    lib26b1::hide(r.game, gobj);
    // `it_80274574`.
    lib2725::scale_by_owner(r, gobj);
}

/// `itPeachtoad_UnkMotion0_Anim`: shown on frame 7 and hidden from frame 53; gone, forgotten
/// by his owner, once she has left the move.
#[doc(alias = "itPeachtoad_UnkMotion0_Anim")]
fn standing(r: &mut Running, gobj: GObjId) -> bool {
    let frame = reg(item(r.game, gobj).x5CC_currentAnimFrame);
    if frame == 7.0 {
        lib26b1::show(r.game, gobj);
    }
    if frame >= 53.0 {
        lib26b1::hide(r.game, gobj);
    }
    if owner_done(r, gobj) {
        owner_forgets(r, gobj);
        return true;
    }
    false
}

/// An attack met Toad, `it_802BE100`: his second state, ten frames on at once, sized as his
/// owner.
#[doc(alias = "it_802BE100")]
pub fn hit(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 10.0);
    jobj::anim_all(&mut r.game.objs, r.data, &mut ParticleHooks, j);
    crate::lb::anim::set_framerate(&mut r.game.objs, j, 1.0);
    // `it_80274574`.
    lib2725::scale_by_owner(r, gobj);
}

/// `itPeachtoad_UnkMotion1_Anim`: hidden from frame 60; gone once his owner has left the
/// move.
#[doc(alias = "itPeachtoad_UnkMotion1_Anim")]
fn releasing(r: &mut Running, gobj: GObjId) -> bool {
    if reg(item(r.game, gobj).x5CC_currentAnimFrame) >= 60.0 {
        lib26b1::hide(r.game, gobj);
    }
    owner_done(r, gobj)
}

/// `itPeachToad_Logic91_EvtUnk`: forgets `other`.
#[doc(alias = "itPeachToad_Logic91_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
