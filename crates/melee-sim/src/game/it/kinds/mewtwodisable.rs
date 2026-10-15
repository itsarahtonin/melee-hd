//! Mewtwo's Disable, `melee/it/kinds/itmewtwodisable.c`: the flash his down special sends
//! ahead of him, flying at his height's scale until its life runs out or it meets a wall or
//! ceiling. Its hitbox's element stuns the fighters it hits facing it (the common damage code).

use gekko_fp as fp;

use super::super::item::{self, change, change_state, spawn_air};
use super::super::{ground, item_mut, lib26b1, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `It_Kind_Mewtwo_Disable`.
pub const KIND: i32 = super::MEWTWO_DISABLE;

pub const DESTROY: u32 = 0x802C_49E0;
pub const DESTROYED: u32 = 0x802C_4A00;
pub const SPAWN: u32 = 0x802C_4A40;
pub const START: u32 = 0x802C_4B38;
pub const SCALE: u32 = 0x802C_4BB8;
pub const ANIM: u32 = 0x802C_4C08;
pub const PHYS: u32 = 0x802C_4C3C;
pub const COLL: u32 = 0x802C_4C40;
pub const DMG_DEALT: u32 = 0x802C_4C90;
pub const REFLECTED: u32 = 0x802C_4C98;
pub const CLANKED: u32 = 0x802C_4CB8;
pub const HIT_SHIELD: u32 = 0x802C_4CC0;
pub const ABSORBED: u32 = 0x802C_4CC8;
pub const SHIELD_BOUNCED: u32 = 0x802C_4CD0;
pub const EVT_UNK: u32 = 0x802C_4CD8;

/// `itMDisableAttributes`: its life and speed.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const X_VEL: u32 = 0x4;
}

/// Runs one of Disable's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj) != 0,
        DMG_DEALT | CLANKED | HIT_SHIELD | ABSORBED | SHIELD_BOUNCED => true,
        REFLECTED => lib2725::reflected_back(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of Disable's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS => {}
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = item::article(super::super::item(r.game, gobj))
        .special(r.data)
        .expect("Disable without attributes");
    r.data.f32(attrs.at(off))
}

/// The fighter who sent it, if any.
fn owner(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    item_mut(r.game, gobj).xDD4_itemVar.mdisable().owner.get()
}

/// Gone at Mewtwo's asking, `itMewtwoDisable_Logic67_Destroy`.
#[doc(alias = "itMewtwoDisable_Logic67_Destroy")]
pub fn destroy(r: &mut Running, gobj: GObjId) {
    item::destroy(r, gobj);
}

/// Its end, `itMewtwoDisable_Logic67_Destroyed`: its sender no longer has it out
/// (`ftMt_SpecialLw_ClearDisableGObj`), and it has no sender.
#[doc(alias = "itMewtwoDisable_Logic67_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(o) = owner(r, gobj) {
        r.mt_clear_disable(o);
    }
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .mdisable()
        .owner
        .set(None);
}

/// Sends it from `pos` facing `facing_dir`, `itMewtwoDisable_Logic67_SpawnMewtwoDisable`:
/// its sender kept, flying ([`start`]), not to be picked up.
#[doc(alias = "itMewtwoDisable_Logic67_SpawnMewtwoDisable")]
pub fn spawn(r: &mut Running, fighter: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = lib26b1::parent_ecb_center(r.game, fighter);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(fighter);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    let o = ip.owner;
    ip.xDD4_itemVar.mdisable().owner.set(o);
    start(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    item::debug_owner_bubbles(r.game, gobj, Some(fighter));
    item_mut(r.game, gobj).xDCC_flag.b3 = true;
    Some(gobj)
}

/// Flying, `it_802C4B38`: its speed its way at its sender's height's scale, its life, and
/// [`scale_hitboxes`].
#[doc(alias = "it_802C4B38")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let o = owner(r, gobj).expect("Disable without a sender");
    let speed = reg(attr_f32(r, gobj, attr::X_VEL));
    let scale = reg(r.game.ft_scale(o));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(fp::fmuls(speed, reg(ip.facing_dir)), scale));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    scale_hitboxes(r, gobj);
}

/// Its state and its hitboxes scaled by its sender's height's scale, `it_802C4BB8`.
#[doc(alias = "it_802C4BB8")]
pub fn scale_hitboxes(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let o = owner(r, gobj).expect("Disable without a sender");
    let scale = r.game.ft_scale(o);
    super::super::hitbox::scale_hitboxes(r.game, gobj, reg(scale));
}

/// `itMewtwodisable_UnkMotion0_Anim`: done once its life runs out (`Item_TickLifetime`).
#[doc(alias = "itMewtwodisable_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// `itMewtwodisable_UnkMotion0_Coll`: its collision in the air; the walls and ceiling it
/// touched.
#[doc(alias = "itMewtwodisable_UnkMotion0_Coll")]
pub fn collide(r: &mut Running, gobj: GObjId) -> i32 {
    ground::air_stay(r, gobj);
    let mut flags = 0;
    flags |= maplib::touched_wall(r.game, gobj);
    flags |= maplib::touched_ceiling(r.game, gobj);
    flags
}

/// Forgets a GObj, `itMewtwoDisable_Logic67_EvtUnk`: its sender too if it's that one.
#[doc(alias = "itMewtwoDisable_Logic67_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    if owner(r, gobj) == other {
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .mdisable()
            .owner
            .set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}
