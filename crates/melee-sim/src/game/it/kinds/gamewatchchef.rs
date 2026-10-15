//! Mr. Game & Watch's sausages, `melee/it/kinds/itgamewatchchef.c`: thrown from Chef's pan,
//! each of the five by its own speed, spin and fall (the attributes' entries), turning back
//! off walls and stopping on the floor, a hit or a shield, then blinking out.

use gekko_fp as fp;

use super::super::item::{article, change, change_state};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use super::gamewatch;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DMG_DEALT: u32 = 0x802C_847C;
pub const ANIM0: u32 = 0x802C_8540;
pub const PHYS0: u32 = 0x802C_8690;
pub const COLL0: u32 = 0x802C_86D0;
pub const ANIM1: u32 = 0x802C_87B4;
pub const PHYS1: u32 = 0x802C_8828;
pub const COLL1: u32 = 0x802C_882C;
pub const CLANKED: u32 = 0x802C_88E4;
pub const HIT_SHIELD: u32 = 0x802C_8940;
pub const ABSORBED: u32 = 0x802C_899C;
pub const SHIELD_BOUNCED: u32 = 0x802C_8A08;
pub const REFLECTED: u32 = 0x802C_8A28;
pub const EVT_UNK: u32 = 0x802C_8B08;

/// `itGamewatchchefAttributes`: the wall's bounce (`x4`), the lifetime thrown (`x8`) and
/// stopped (`xC`), then an entry of five floats per sausage from `x10`: its speed ahead and
/// up, its gravity and top fall speed, and its spin.
mod attr {
    pub const BOUNCE: u32 = 0x4;
    pub const LIFETIME: u32 = 0x8;
    pub const STOPPED_LIFETIME: u32 = 0xC;
    pub const ENTRIES: u32 = 0x10;
    pub const ENTRY_SIZE: u32 = 0x14;
}

/// Runs one of the sausage's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_DEALT => dmg_dealt(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        CLANKED => clanked(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        ABSORBED => absorbed(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        REFLECTED => reflected(r, gobj),
        _ => return None,
    })
}

/// Runs one of the sausage's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a sausage without attributes");
    r.data.f32(attrs.at(off))
}

/// A float of the sausage's entry.
fn entry_f32(r: &mut Running, gobj: GObjId, off: u32) -> f32 {
    let i = gamewatch::vars(r, gobj).x4.get();
    attr_f32(r, gobj, attr::ENTRIES + attr::ENTRY_SIZE * i as u32 + off)
}

/// Sausage `index` of kind `kind` from `parent`'s pan at `pos`, `it_802C837C`: placed from
/// its owner's ECB center with a first collision, thrown, and colored as he is.
#[doc(alias = "it_802C837C")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    kind: i32,
    index: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    let attr = gamewatch::first_attr(r, gobj);
    throw(r, gobj, index);
    super::super::item::debug_owner_bubbles(r.game, gobj, Some(parent));
    gamewatch::colors(r, gobj, parent, attr);
    Some(gobj)
}

/// It hit something, `itGameWatchChef_Logic112_DmgDealt`: stopped.
#[doc(alias = "itGameWatchChef_Logic112_DmgDealt")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    stop(r, gobj);
    false
}

/// Sausage `index` thrown, `it_802C84A0`: its lifetime, its entry's speed ahead and up, no
/// pickup, and its first state.
#[doc(alias = "it_802C84A0")]
pub fn throw(r: &mut Running, gobj: GObjId, index: i32) {
    gamewatch::vars(r, gobj).x4.set(index);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    item_mut(r.game, gobj).xD5C = 0;
    let ahead = reg(entry_f32(r, gobj, 0x0));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(ahead, reg(ip.facing_dir)));
    let up = entry_f32(r, gobj, 0x4);
    item_mut(r.game, gobj).x40_vel.y = up;
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itGamewatchchef_UnkMotion0_Anim`: its spin, and its lifetime counting down to its stop.
#[doc(alias = "itGamewatchchef_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    let spin = reg(entry_f32(r, gobj, 0x10));
    let rot = fp::fadds(reg(r.game.objs.jobjs[j].rotate.x), spin);
    lib2725::set_rotation_x(r.game, j, mem(rot));
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if ip.xD44_lifeTimer <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        stop(r, gobj);
    }
    false
}

/// `itGamewatchchef_UnkMotion0_Phys`: its entry's fall.
#[doc(alias = "itGamewatchchef_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let g = entry_f32(r, gobj, 0x8);
    let max = entry_f32(r, gobj, 0xC);
    lib2725::fall(r.game, gobj, g, max);
}

/// `itGamewatchchef_UnkMotion0_Coll`: moving, a wall turns it back slower and the floor or
/// ceiling stops it.
#[doc(alias = "itGamewatchchef_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).x40_vel.x != 0.0 {
        let touched = ground::air_touches(r, gobj);
        if touched & 0xC != 0 {
            let bounce = reg(attr_f32(r, gobj, attr::BOUNCE));
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::fneg(bounce)));
        }
        if touched & 3 != 0 {
            stop(r, gobj);
        }
    }
    false
}

/// Stopped, `it_802C875C`: no speed, its stopped lifetime, its second state.
#[doc(alias = "it_802C875C")]
pub fn stop(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = 0.0;
    ip.x40_vel.x = 0.0;
    let life = attr_f32(r, gobj, attr::STOPPED_LIFETIME);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itGamewatchchef_UnkMotion1_Anim`: blinking every two frames until its lifetime runs out.
#[doc(alias = "itGamewatchchef_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if ip.xD44_lifeTimer <= 0.0 {
        ip.xD44_lifeTimer = 0.0;
        return true;
    }
    if fp::fctiwz(reg(ip.xD44_lifeTimer)) & 2 != 0 {
        lib26b1::show(r.game, gobj);
    } else {
        lib26b1::hide(r.game, gobj);
    }
    false
}

/// `itGamewatchchef_UnkMotion1_Phys`: nothing.
#[doc(alias = "itGamewatchchef_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itGamewatchchef_UnkMotion1_Coll`: as moving.
#[doc(alias = "itGamewatchchef_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    coll0(r, gobj)
}

/// Clanked, `it_2725_Logic112_Clanked`: stopped.
#[doc(alias = "it_2725_Logic112_Clanked")]
pub fn clanked(r: &mut Running, gobj: GObjId) -> bool {
    stop(r, gobj);
    false
}

/// Hit a shield, `it_2725_Logic112_HitShield`: stopped.
#[doc(alias = "it_2725_Logic112_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    stop(r, gobj);
    false
}

/// Absorbed, `it_2725_Logic112_Absorbed`: stopped, its lifetime out.
#[doc(alias = "it_2725_Logic112_Absorbed")]
pub fn absorbed(r: &mut Running, gobj: GObjId) -> bool {
    stop(r, gobj);
    item_mut(r.game, gobj).xD44_lifeTimer = 0.0;
    false
}

/// Bounced off a shield, `itGameWatchChef_Logic112_ShieldBounced`.
#[doc(alias = "itGameWatchChef_Logic112_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Reflected, `itGameWatchChef_Logic112_Reflected` (`itReflectItemAndUpdateRotation`).
#[doc(alias = "itGameWatchChef_Logic112_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let half_pi = fp::frsp(fp::fdiv(std::f64::consts::PI, 2.0));
    let y = mem(fp::fmuls(half_pi, reg(item(r.game, gobj).facing_dir)));
    let j = jobj_of(r.game, gobj).expect("jobj.h: jobj");
    lib2725::set_rotation_y(r.game, j, y);
    false
}

/// Forgets a GObj, `itGameWatchChef_Logic112_EvtUnk`.
// Ports `itGameWatchChef_Logic112_EvtUnk`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
