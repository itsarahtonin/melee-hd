//! Samus's bomb, `melee/it/kinds/itsamusbomb.c`: dropped from the morph ball, it falls (its
//! state 1) or rests (state 0), knocked about (state 2), pulsing faster near its end, and blows
//! up (state 3) when its time runs out or it hits something; its blast throws Samus herself up
//! if it touches her (`ftSs_Init_80128944`).

use gekko_fp as fp;

use super::super::item::{article, attr_of, change, change_state, debug_owner_bubbles, spawn_air};
use super::super::{ground, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `It_Kind_Samus_Bomb`.
pub const KIND: i32 = super::SAMUS_BOMB;

pub const SPAWN: u32 = 0x802B_4AC8;
pub const START: u32 = 0x802B_4BA0;
pub const TO_REST: u32 = 0x802B_4C10;
pub const ANIM_0: u32 = 0x802B_4C38;
pub const PHYS_0: u32 = 0x802B_4CC4;
pub const COLL_0: u32 = 0x802B_4CC8;
pub const TO_FALL: u32 = 0x802B_4CF4;
pub const ANIM_1: u32 = 0x802B_4D1C;
pub const PHYS_1: u32 = 0x802B_4F1C;
pub const COLL_1: u32 = 0x802B_4F4C;
pub const ENTERED_AIR: u32 = 0x802B_4F78;
pub const ANIM_2: u32 = 0x802B_4FA0;
pub const PHYS_2: u32 = 0x802B_51A0;
pub const COLL_2: u32 = 0x802B_5284;
pub const DMG_DEALT: u32 = 0x802B_52B8;
pub const CLANKED: u32 = 0x802B_52EC;
pub const HIT_SHIELD: u32 = 0x802B_5320;
pub const SHIELD_BOUNCED: u32 = 0x802B_5354;
pub const REFLECTED: u32 = 0x802B_5374;
pub const EXPLODE: u32 = 0x802B_53CC;
pub const ANIM_3: u32 = 0x802B_5458;
pub const BLAST: u32 = 0x802B_5478;
pub const EVT_UNK: u32 = 0x802B_54E4;

/// `itSamusBombAttributes`.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const PULSE_TIME: u32 = 0x4;
    pub const PULSE_RATE: u32 = 0x8;
    pub const KNOCK_SPEED_MAX: u32 = 0xC;
}

/// `M_PI`.
const PI: f64 = std::f64::consts::PI;

/// Runs one of the bomb's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => {
            tick(r, gobj);
            false
        }
        COLL_0 => {
            ground::ground_or_fall(r, gobj, to_fall);
            false
        }
        ANIM_1 | ANIM_2 => {
            tick(r, gobj);
            roll(r, gobj);
            false
        }
        COLL_1 => {
            ground::fall(r, gobj, to_rest);
            false
        }
        COLL_2 => {
            ground::ground_skid(r, gobj, to_rest, to_fall);
            false
        }
        DMG_DEALT | CLANKED | HIT_SHIELD => {
            if item(r.game, gobj).msid != 3 {
                explode(r, gobj);
            }
            false
        }
        SHIELD_BOUNCED => lib2725::bounce_off_shield(r.game, gobj),
        REFLECTED => reflected(r, gobj),
        ANIM_3 => lib2725::tick_life(r.game, gobj),
        _ => return None,
    })
}

/// Runs one of the bomb's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        TO_REST => to_rest(r, gobj),
        TO_FALL => to_fall(r, gobj),
        PHYS_0 => {}
        PHYS_1 => {
            let a = attr_of(item(r.game, gobj));
            let accel = a.f32(r.data, super::super::data::attr::FALL_SPEED);
            let max = a.f32(r.data, super::super::data::attr::FALL_SPEED_MAX);
            lib2725::fall(r.game, gobj, accel, max);
        }
        ENTERED_AIR => change_state(r, gobj, 2, 0x11),
        PHYS_2 => physics_knocked(r, gobj),
        EXPLODE => explode(r, gobj),
        BLAST => blast(r, gobj),
        _ => return false,
    }
    true
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let attrs = article(item(r.game, gobj))
        .special(r.data)
        .expect("a bomb without attributes");
    r.data.f32(attrs.at(off))
}

/// Drops a bomb from `parent` at `pos`, `it_802B4AC8`.
#[doc(alias = "it_802B4AC8")]
pub fn spawn(r: &mut Running, parent: GObjId, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields(&spawn, gobj, facing_dir, true)`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let bomb = spawn_air(r, &mut spawn)?;
    start(r, bomb);
    debug_owner_bubbles(r.game, bomb, Some(parent));
    Some(bomb)
}

/// The bomb's start, `it_802B4BA0`: its first fall speed (the item attributes' `x18`), its
/// owner kept, its lifetime, no pickup, and the falling state.
#[doc(alias = "it_802B4BA0")]
pub fn start(r: &mut Running, gobj: GObjId) {
    let rise = attr_of(item(r.game, gobj)).f32(r.data, 0x18);
    let life = attr_f32(r, gobj, attr::LIFETIME);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = rise;
    ip.xDB4_itcmd_var2 = 0;
    let owner = ip.owner;
    ip.xDD4_itemVar.samus_bomb().owner.set(owner);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    lib26b1::disable_pickup(r.game, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Coming to rest, `it_802B4C10`: state 0.
#[doc(alias = "it_802B4C10")]
pub fn to_rest(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, 0x11);
}

/// Falling, `it_802B4CF4`: state 1.
#[doc(alias = "it_802B4CF4")]
pub fn to_fall(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 0x11);
}

/// The bomb's fuse (`itSamusBomb_UnkMotion_PreProcess`): it pulses faster near its end
/// (`lb_8000BA0C`), and blows up when its time runs out.
fn tick(r: &mut Running, gobj: GObjId) {
    let (pulse_at, rate) = (
        attr_f32(r, gobj, attr::PULSE_TIME),
        attr_f32(r, gobj, attr::PULSE_RATE),
    );
    let j = jobj_of(r.game, gobj);
    if item(r.game, gobj).xD44_lifeTimer <= pulse_at {
        crate::lb::anim::set_framerate(&mut r.game.objs, j, rate);
    }
    let ip = item_mut(r.game, gobj);
    if ip.xD44_lifeTimer <= 0.0 {
        explode(r, gobj);
    } else {
        ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), fp::frsp(1.0)));
    }
}

/// Rolling (`itSamusBomb_UnkMotion_Process`): moving at all, it faces its motion
/// (`it_80272980`) and turns its model that way.
fn roll(r: &mut Running, gobj: GObjId) {
    let x = reg(item(r.game, gobj).x40_vel.x);
    let speed = if x < fp::frsp(0.0) { fp::fneg(x) } else { x };
    if speed < 9.999_999_747_378_752e-5 {
        return;
    }
    lib2725::face_motion(r.game, gobj);
    let j = jobj_of(r.game, gobj).expect("a bomb without a model");
    let yaw = if item(r.game, gobj).facing_dir == 1.0 {
        0.0
    } else {
        mem(fp::frsp(PI))
    };
    jobj::set_rotation_y(&mut r.game.objs.jobjs, j, yaw);
}

/// `itSamusbomb_UnkMotion2_Phys`: knocked about, its knockback (`x7C`) held to the
/// attributes' `xC`.
#[doc(alias = "itSamusbomb_UnkMotion2_Phys")]
pub fn physics_knocked(r: &mut Running, gobj: GObjId) {
    let max = attr_f32(r, gobj, attr::KNOCK_SPEED_MAX);
    let ip = item_mut(r.game, gobj);
    let (x, y) = (reg(ip.x7C.x), reg(ip.x7C.y));
    let len = crate::math::sqrt_accurate(mem(fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y))));
    if len > max {
        ip.x7C.normalize_xy();
        let m = reg(max);
        ip.x7C.x = mem(fp::fmuls(reg(ip.x7C.x), m));
        ip.x7C.y = mem(fp::fmuls(reg(ip.x7C.y), m));
    }
}

/// Reflected, `it_2725_Logic50_Reflected`: sent back, rising if it fell, and no longer hers.
#[doc(alias = "it_2725_Logic50_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    let y = reg(ip.x40_vel.y);
    if y < 0.0 {
        ip.x40_vel.y = mem(fp::fmuls(fp::fneg(y), reg(ip.xC70)));
    }
    ip.xDD4_itemVar.samus_bomb().owner.set(None);
    false
}

/// Blowing up, `it_802B53CC`: state 3 at full speed, still, its model hidden, its blast
/// (`it_80272B40`), no pickup, vanishing, and its blast's check on Samus hooked.
#[doc(alias = "it_802B53CC")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::hide(r.game, gobj);
    lib2725::blast_small(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(BLAST));
    ip.xDD4_itemVar.samus_bomb().x0.set(1);
}

/// The blast's first frame (its accessory), `it_802B5478`: if it touches its owner, she
/// jumps away from it (`ftSs_Init_80128944`).
#[doc(alias = "it_802B5478")]
pub fn blast(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let v = ip.xDD4_itemVar.samus_bomb();
    if v.x0.get() == 0 {
        return;
    }
    v.x0.set(0);
    let Some(owner) = v.owner.get() else {
        return;
    };
    let scale = item(r.game, gobj).scl;
    if r.samus_bomb_touches(owner, gobj, scale) {
        let ip = item(r.game, gobj);
        let (x, reach) = (ip.pos.x, ip.x5D4_hitboxes[0].hit.scale);
        r.samus_bomb_jump(owner, x, reach);
    }
}

/// Forgets a GObj the bomb points to, `itSamusBomb_Logic50_EvtUnk`: and its owner.
// Ports `itSamusBomb_Logic50_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .samus_bomb()
        .owner
        .set(None);
}
