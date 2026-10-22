//! PK Flash's blast, `melee/it/kinds/itnesspkflashexplode.c`: as big and as strong as the flash
//! had grown, for its animation's length, with a rumble and, for the bigger ones, a quake.

use gekko_fp as fp;

use super::super::item::{article, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{coll, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::camera::request_quake;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::hitcapsule;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_F940;
pub const DESTROYED: u32 = 0x802A_FA2C;
pub const START: u32 = 0x802A_FA70;
pub const ANIM: u32 = 0x802A_FBA4;
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Phys")]
pub const PHYS: u32 = 0x802A_FD2C;
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Coll")]
pub const COLL: u32 = 0x802A_FD44;
pub const CLANKED: u32 = 0x802A_FD4C;
#[doc(alias = "itNessPKFlashExplode_Logic103_Absorbed")]
pub const ABSORBED: u32 = 0x802A_FD54;
#[doc(alias = "itNessPKFlashExplode_Logic103_ShieldBounced")]
pub const SHIELD_BOUNCED: u32 = 0x802A_FD5C;
#[doc(alias = "itNessPKFlashExplode_Logic103_HitShield")]
pub const HIT_SHIELD: u32 = 0x802A_FD64;
pub const EVT_UNK: u32 = 0x802A_FD6C;

/// `itFlashExplAttributes`: the flash's size it scales by, the first and the grown scale, and
/// the damage's base and growth.
mod attr {
    pub const HITBOX_SIZE_MUL: u32 = 0x0;
    pub const GRAPHIC_SIZE_INIT: u32 = 0x4;
    pub const GRAPHIC_SIZE_GROWTH: u32 = 0x8;
    pub const BASE_DAMAGE: u32 = 0xC;
    pub const DAMAGE_MUL: u32 = 0x10;
}

/// `QuakeKind_Small`, `QuakeKind_Medium`.
const QUAKE_SMALL: i32 = 2;
const QUAKE_MEDIUM: i32 = 3;

/// Runs one of the blast's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => false,
        CLANKED | ABSORBED | SHIELD_BOUNCED | HIT_SHIELD => true,
        _ => return None,
    })
}

/// Runs one of the blast's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PHYS => {
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.z = 0.0;
            ip.x40_vel.y = 0.0;
        }
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("PK Flash's blast without attributes");
    r.data.f32(attrs.at(off))
}

/// The blast at `pos` of a flash grown to `charge`, `it_802AF940`.
#[doc(alias = "it_802AF940")]
pub fn spawn(
    r: &mut Running,
    owner: GObjId,
    flash: GObjId,
    pos: Vec3,
    kind: i32,
    facing_dir: f32,
    charge: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(flash);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    let v = ip.xDD4_itemVar.pk_flush_explode();
    v.xDD4.set(charge);
    v.xDDC.set(Some(owner));
    start(r, gobj);
    Some(gobj)
}

/// The blast's end, `it_2725_Logic103_Destroyed`.
#[doc(alias = "it_2725_Logic103_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    item_mut(r.game, gobj).owner = None;
}

/// The blast's start, `it_802AFA70`: its first frame, and by the charge a rumble and a quake.
#[doc(alias = "it_802AFA70")]
pub fn start(r: &mut Running, gobj: GObjId) {
    // `Item_ClearFlagsAndEnterState(gobj, ip, 0)`.
    lib26b1::disable_pickup(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x13 = false;
    lib2725::show(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib2725::set_lifetime(r.game, r.data, gobj, 1024.0);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .pk_flush_explode()
        .xDD8
        .set(0.0);
    // `Item_802694CC`.
    super::super::item::animate(r, gobj);
    animate(r, gobj);
    let full = reg(attr_f32(r, gobj, attr::HITBOX_SIZE_MUL));
    let charge = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .pk_flush_explode()
        .xDD4
        .get());
    let ratio = fp::fdivs(charge, full);
    let pos = item(r.game, gobj).pos;
    if ratio > f64::from(0.849_999_96_f32) {
        request_quake(r, QUAKE_MEDIUM, Some(pos));
        lib2725::rumble_owner(r, gobj, 0xE, 0x14);
    } else if ratio > f64::from(0.65_f32) {
        request_quake(r, QUAKE_SMALL, Some(pos));
        lib2725::rumble_owner(r, gobj, 0xD, 0xF);
    } else if ratio > f64::from(0.45_f32) {
        lib2725::rumble_owner(r, gobj, 0xC, 0xA);
    } else {
        lib2725::rumble_owner(r, gobj, 0xB, 5);
    }
    let owner = item(r.game, gobj).owner;
    debug_owner_bubbles(r.game, gobj, owner);
}

/// `itNessPKFlashExplode_UnkMotion0_Anim`: scaled by the charge, its damage set once, its
/// hitbox grown with it; it goes when its animation ends.
#[doc(alias = "itNessPKFlashExplode_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let (full, init, growth) = (
        reg(attr_f32(r, gobj, attr::HITBOX_SIZE_MUL)),
        reg(attr_f32(r, gobj, attr::GRAPHIC_SIZE_INIT)),
        reg(attr_f32(r, gobj, attr::GRAPHIC_SIZE_GROWTH)),
    );
    let (base, mul) = (
        reg(attr_f32(r, gobj, attr::BASE_DAMAGE)),
        reg(attr_f32(r, gobj, attr::DAMAGE_MUL)),
    );
    let charge = reg(item_mut(r.game, gobj)
        .xDD4_itemVar
        .pk_flush_explode()
        .xDD4
        .get());
    let s = mem(fp::fmadds(
        charge,
        fp::fdivs(fp::fsubs(growth, init), full),
        init,
    ));
    let j = jobj_of(r.game, gobj).expect("PK Flash's blast without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
    let ip = item(r.game, gobj);
    if ip.xDB4_itcmd_var2 == 0 && ip.x5D4_hitboxes[0].hit.state != hitcapsule::DISABLED {
        let dmg = fp::fmadds(charge, mul, base);
        coll::set_hit_damage(r.game, r.data, gobj, 0, crate::ppc::cvt_fp2unsigned(dmg));
        item_mut(r.game, gobj).xDB4_itcmd_var2 = 1;
    }
    let ip = item_mut(r.game, gobj);
    let first = ip.x5D4_hitboxes[0].hit.scale;
    let v = ip.xDD4_itemVar.pk_flush_explode();
    if v.xDD8.get() == 0.0 {
        v.xDD8.set(first);
    }
    let k = v.xDD8.get();
    ip.x5D4_hitboxes[0].hit.scale = mem(fp::fmuls(reg(k), reg(s)));
    !lib2725::is_animating(r.game, gobj)
}

/// Forgets a GObj the blast points to, `itNessPKFlashExplode_Logic103_EvtUnk`.
// Ports `itNessPKFlashExplode_Logic103_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
