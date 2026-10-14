//! Link's and Young Link's bombs, `melee/it/kinds/itlinkbomb.c`: pulled into the hand (state
//! 0), then dropped, thrown or knocked about (states 1, 2, 3 and 6), rolling on the ground
//! (state 4), until the fuse runs out or something sets it off, and it blows up (state 5). As
//! the fuse burns, a bone of its model (the dynamic bone table's fourth) sinks and tilts; for
//! the fuse's last stretch (`xC` frames) it plays its fourth state description's animation
//! instead.
//!
//! Its variables (`itLinkBomb_ItemVars`): the flags (`x0`: the last stretch, rolling, knocked),
//! the roll's direction (`x4`), the bone's sink and tilt per frame (`x8`, `xC`), and who pulled
//! it (`x10`).

use gekko_fp as fp;

use super::super::item::{
    article, attr_of, change, change_state, common_data, pick_up, set_state_anims, spawn_air,
};
use super::super::{
    LinkBombFlags, data, ground, hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, release,
};
use crate::fpu::{mem, reg};
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itLinkBomb_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::vec::Vec3;

/// `It_Kind_Link_Bomb`.
pub const KIND: i32 = super::LINK_BOMB;

pub const SET_OFF: u32 = 0x8029_D968;
pub const PICKED_UP: u32 = 0x8029_DEB0;
pub const ANIM_0: u32 = 0x8029_E074;
pub const PHYS_0: u32 = 0x8029_E218;
pub const TO_FALL: u32 = 0x8029_E21C;
pub const ANIM_1: u32 = 0x8029_E3D8;
pub const PHYS_1: u32 = 0x8029_E574;
pub const COLL_1: u32 = 0x8029_E5A4;
pub const THROWN: u32 = 0x8029_E5D0;
pub const ANIM_2: u32 = 0x8029_E920;
pub const PHYS_2: u32 = 0x8029_EAF0;
pub const COLL_2: u32 = 0x8029_EB3C;
pub const DROPPED: u32 = 0x8029_EC34;
pub const ANIM_3: u32 = 0x8029_EF84;
pub const PHYS_3: u32 = 0x8029_F04C;
pub const COLL_3: u32 = 0x8029_F098;
pub const TO_ROLL: u32 = 0x8029_F18C;
pub const ANIM_4: u32 = 0x8029_F3DC;
pub const PHYS_4: u32 = 0x8029_F60C;
pub const COLL_4: u32 = 0x8029_F670;
pub const EXPLODE: u32 = 0x8029_F69C;
pub const ANIM_5: u32 = 0x8029_F934;
pub const PHYS_5: u32 = 0x8029_F954;
pub const COLL_5: u32 = 0x8029_F958;
pub const DMG_DEALT: u32 = 0x8029_F960;
pub const DMG_RECEIVED: u32 = 0x8029_FA30;
pub const ENTERED_AIR: u32 = 0x8029_FB24;
pub const ANIM_6: u32 = 0x8029_FB4C;
pub const PHYS_6: u32 = 0x8029_FCE8;
pub const COLL_6: u32 = 0x8029_FCEC;
pub const REFLECTED: u32 = 0x8029_FD20;
pub const HIT_SHIELD: u32 = 0x8029_FD40;
pub const SHIELD_BOUNCED: u32 = 0x8029_FD64;
pub const EVT_UNK: u32 = 0x8029_FD84;

/// `itLinkBombAttributes` offsets.
mod attr {
    pub const LIFETIME: u32 = 0x0;
    pub const XC: u32 = 0xC;
    pub const X10: u32 = 0x10;
    pub const X14: u32 = 0x14;
    pub const X18: u32 = 0x18;
    pub const X1C: u32 = 0x1C;
    pub const X20: u32 = 0x20;
    pub const X24: u32 = 0x24;
    pub const X28: u32 = 0x28;
    pub const X2C: u32 = 0x2C;
    pub const X30: u32 = 0x30;
}

/// The bone the fuse moves: the dynamic bone table's fourth.
const FUSE_BONE: usize = 3;
/// `it_803B8640`.
const ZERO: Vec3 = Vec3 {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

/// Runs one of the bomb's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => anim(r, gobj, picked_up),
        ANIM_1 => anim(r, gobj, to_fall),
        ANIM_2 => anim(r, gobj, thrown),
        ANIM_3 => anim3(r, gobj),
        ANIM_4 => anim(r, gobj, to_roll),
        ANIM_6 => anim(r, gobj, entered_air),
        ANIM_5 => anim5(r, gobj),
        COLL_1 => coll1(r, gobj),
        COLL_2 | COLL_3 => coll_flying(r, gobj),
        COLL_4 => coll4(r, gobj),
        COLL_5 => false,
        COLL_6 => coll6(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        REFLECTED => reflected(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bomb's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SET_OFF => set_off(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        TO_FALL => to_fall(r, gobj),
        THROWN => thrown(r, gobj),
        DROPPED => dropped(r, gobj),
        TO_ROLL => to_roll(r, gobj),
        EXPLODE => explode(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        PHYS_0 | PHYS_5 | PHYS_6 => still(r, gobj),
        PHYS_1 => phys_fall(r, gobj),
        PHYS_2 | PHYS_3 => phys_flying(r, gobj),
        PHYS_4 => phys_roll(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itLinkBomb_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.link_bomb()
}

fn attrs(r: &Running, gobj: GObjId) -> melee_data::archive::DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a bomb without attributes")
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attrs(r, gobj).at(off))
}

fn attr_u32(r: &Running, gobj: GObjId, off: u32) -> u32 {
    r.data.u32(attrs(r, gobj).at(off))
}

fn fuse_bone(game: &Game, gobj: GObjId) -> JObjId {
    let t = item(game, gobj)
        .xBBC_dynamicBoneTable
        .expect("a bomb without a bone table");
    game.it.bone_tables[t.0].bones[FUSE_BONE].expect("a bomb without its fuse's bone")
}

/// Sets a bomb off as its owner's blast sets off held bombs (`it_8026C368`), `it_8029D968`:
/// unless it is held or blowing up already.
#[doc(alias = "it_8029D968")]
pub fn set_off(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDC8_word.flags.x13 && ip.msid != 5 {
        explode(r, gobj);
    }
}

/// Changes state keeping the fuse's bone's height and tilt, `it_8029D9A4`: with the state's
/// script alone in the fuse's last stretch (whose animation plays on), else its animation.
#[doc(alias = "it_8029D9A4")]
fn change_keeping_bone(r: &mut Running, gobj: GObjId, msid: i32, flags: u32) {
    let j = fuse_bone(r.game, gobj);
    let (y, x) = {
        let o = &r.game.objs.jobjs[j];
        (o.translate.y, o.rotate.x)
    };
    let last = vars(r.game, gobj).x0.get().fuse_end;
    let more = if last {
        change::CMD_UPDATE
    } else {
        change::ANIM_UPDATE
    };
    change_state(r, gobj, msid, flags | more);
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.y = y);
    jobj::set_angle(&mut r.game.objs.jobjs, j, |q| q.x = x);
}

/// The fuse's frame (the file's inlines around `it_8029DB5C`): its last stretch begins (the
/// fourth state description's animation, played from its start) once the life left is down
/// to the attributes' `xC`; a frame off its life; and before the last stretch, the bone sinks
/// and tilts.
fn burn(r: &mut Running, gobj: GObjId) {
    let xc = attr_u32(r, gobj, attr::XC);
    let last = vars(r.game, gobj).x0.get().fuse_end;
    if item(r.game, gobj).xD44_lifeTimer <= mem(fp::frsp(f64::from(xc))) && !last {
        let desc = article(item(r.game, gobj))
            .states(r.data)
            .expect("an article without states")
            .at(0x10 * 3);
        item_mut(r.game, gobj).xD0_itemStateDesc = Some(desc);
        set_state_anims(r, gobj, desc);
        let j = jobj_of(r.game, gobj);
        jobj::anim_all(
            &mut r.game.objs,
            r.data,
            &mut crate::game::code::ParticleHooks,
            j,
        );
        vars(r.game, gobj).x0.update(|f| LinkBombFlags {
            fuse_end: true,
            ..f
        });
    }
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    if !vars(r.game, gobj).x0.get().fuse_end {
        let v = vars(r.game, gobj);
        let (dy, dx) = (v.x8.get(), v.xC.get());
        let j = fuse_bone(r.game, gobj);
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.translate.y = mem(fp::fadds(reg(o.translate.y), reg(dy)));
        });
        jobj::set_srt(&mut r.game.objs.jobjs, j, |o| {
            o.rotate.x = mem(fp::fadds(reg(o.rotate.x), reg(dx)));
        });
    }
}

/// The fuse's frame, blowing up once its life is out, `it_8029DB5C`.
#[doc(alias = "it_8029DB5C")]
fn burn_or_explode(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xD44_lifeTimer <= 0.0 {
        explode(r, gobj);
    } else {
        burn(r, gobj);
    }
}

/// States 0, 1, 2, 4 and 6's animation (`itLinkbomb_UnkMotion0_Anim` and the others): the
/// state again (`again`) once its animation ends, then the fuse.
#[doc(alias = "itLinkbomb_UnkMotion0_Anim")]
#[doc(alias = "itLinkbomb_UnkMotion1_Anim")]
#[doc(alias = "itLinkbomb_UnkMotion2_Anim")]
#[doc(alias = "itLinkbomb_UnkMotion4_Anim")]
#[doc(alias = "itLinkbomb_UnkMotion6_Anim")]
fn anim(r: &mut Running, gobj: GObjId, again: fn(&mut Running, GObjId)) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        again(r, gobj);
    }
    burn_or_explode(r, gobj);
    false
}

/// `itLinkbomb_UnkMotion3_Anim`: dropped again once its animation ends, and falling if that
/// ends at once; then the fuse.
#[doc(alias = "itLinkbomb_UnkMotion3_Anim")]
fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        dropped(r, gobj);
    }
    if !lib2725::is_animating(r.game, gobj) {
        to_fall(r, gobj);
    }
    burn_or_explode(r, gobj);
    false
}

/// A bomb in `parent`'s hand at bone `part`, `it_8029DD58`: of `kind`, at `pos` facing
/// `facing_dir`, with its fuse, its fuse bone's sink and tilt per frame for its size, silent
/// until picked up.
#[doc(alias = "it_8029DD58")]
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
    spawn.prev_pos = pos;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let life = attr_u32(r, gobj, attr::LIFETIME);
    let xc = attr_u32(r, gobj, attr::XC);
    lib2725::set_lifetime(r.game, r.data, gobj, mem(fp::frsp(f64::from(life))));
    let scl = item(r.game, gobj).scl;
    let span = fp::frsp(f64::from(life.wrapping_sub(xc)));
    let v = vars(r.game, gobj);
    v.x0.set(LinkBombFlags {
        fuse_end: false,
        rolling: None,
        knocked: false,
    });
    v.x10.set(Some(parent));
    let sink = fp::fdivs(fp::fneg(reg(2.800_000_2_f32)), span);
    v.x8.set(mem(fp::fmuls(sink, reg(scl))));
    let tilt = fp::fdivs(fp::fneg(reg(0.157_079_64_f32)), span);
    v.xC.set(mem(fp::fmuls(tilt, reg(scl))));
    lib26b1::silence(r.game, gobj);
    pick_up(r, gobj, parent, part);
    Some(gobj)
}

/// `it_8029DEB0` (picked up): held (state 0), heard again.
#[doc(alias = "it_8029DEB0")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    change_keeping_bone(r, gobj, 0, 0);
    lib26b1::unsilence(r.game, gobj);
}

/// Falling, `fn_8029E21C` (state 1).
#[doc(alias = "fn_8029E21C")]
fn to_fall(r: &mut Running, gobj: GObjId) {
    change_keeping_bone(r, gobj, 1, 0);
}

/// `itLinkbomb_UnkMotion1_Phys`: gravity.
#[doc(alias = "itLinkbomb_UnkMotion1_Phys")]
fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itLinkbomb_UnkMotion2_Phys` and `itLinkbomb_UnkMotion3_Phys` (`Item_ApplyFallingPhysics`):
/// gravity, the spin slowed.
#[doc(alias = "itLinkbomb_UnkMotion2_Phys")]
#[doc(alias = "itLinkbomb_UnkMotion3_Phys")]
fn phys_flying(r: &mut Running, gobj: GObjId) {
    phys_fall(r, gobj);
    let x68 = common_data(r.game).f32(r.data, data::common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// Thrown, `it_8029E5D0` (state 2): its own and its owner's items can hit it, its hitboxes
/// kept when thrown again in flight.
#[doc(alias = "it_8029E5D0")]
fn thrown(r: &mut Running, gobj: GObjId) {
    fly(r, gobj, 2);
}

/// Dropped, `it_8029EC34` (state 3).
#[doc(alias = "it_8029EC34")]
fn dropped(r: &mut Running, gobj: GObjId) {
    fly(r, gobj, 3);
}

fn fly(r: &mut Running, gobj: GObjId, msid: i32) {
    hitbox::let_own_items_hit(r.game, gobj);
    hitbox::set_xdce_b2(r.game, gobj);
    let keep = if item(r.game, gobj).msid != msid {
        0
    } else {
        change::HIT_PRESERVE
    };
    change_keeping_bone(r, gobj, msid, keep | change::DROP_UPDATE);
}

/// Whether it hits too hard and blows up (the file's `it_LinkBomb_Inline_VelocityCompare`):
/// faster than the attributes' `x24` across or `x28` up or down.
fn too_fast(r: &mut Running, gobj: GObjId, vel: Vec3) -> bool {
    let (x24, x28) = (attr_f32(r, gobj, attr::X24), attr_f32(r, gobj, attr::X28));
    let abs = |v: f32| if v < 0.0 { mem(fp::fneg(reg(v))) } else { v };
    if abs(vel.x) > x24 || abs(vel.y) > x28 {
        explode(r, gobj);
        return true;
    }
    false
}

/// `itLinkbomb_UnkMotion2_Coll` and `itLinkbomb_UnkMotion3_Coll`: against the stage it blows
/// up if it came too fast, else on a floor it lands and rolls.
#[doc(alias = "itLinkbomb_UnkMotion2_Coll")]
#[doc(alias = "itLinkbomb_UnkMotion3_Coll")]
fn coll_flying(r: &mut Running, gobj: GObjId) -> bool {
    let vel = item(r.game, gobj).x40_vel;
    let touched = ground::air_touches(r, gobj);
    if touched & 0xF != 0 && !too_fast(r, gobj, vel) && touched & 1 != 0 {
        maplib::enter_ground(item_mut(r.game, gobj));
        let b = item(r.game, gobj).xC0C.clone();
        maplib::set_box(r.game, gobj, b);
        to_roll(r, gobj);
    }
    false
}

/// Rolling, `it_8029F18C` (state 4): along its speed's way (the original's test of its state
/// always passes).
#[doc(alias = "it_8029F18C")]
fn to_roll(r: &mut Running, gobj: GObjId) {
    let vx = item(r.game, gobj).x40_vel.x;
    let dir = if vx >= 0.0 { 1.0 } else { -1.0 };
    vars(r.game, gobj).x4.set(dir);
    let facing = if dir == -1.0 { -1 } else { 1 };
    item_mut(r.game, gobj).x378_itemColl.set_facing_dir(facing);
    vars(r.game, gobj).x0.update(|f| LinkBombFlags {
        rolling: Some(true),
        ..f
    });
    change_keeping_bone(r, gobj, 4, 0);
}

/// `itLinkbomb_UnkMotion4_Phys`: a roll slows by the attributes' `x2C` its way, and stops
/// under `x30`.
#[doc(alias = "itLinkbomb_UnkMotion4_Phys")]
fn phys_roll(r: &mut Running, gobj: GObjId) {
    let (x2c, x30) = (attr_f32(r, gobj, attr::X2C), attr_f32(r, gobj, attr::X30));
    let rolling = vars(r.game, gobj).x0.get().rolling == Some(true);
    if !rolling {
        return;
    }
    let dir = vars(r.game, gobj).x4.get();
    let ip = item_mut(r.game, gobj);
    let vx = ip.x40_vel.x;
    if vx != 0.0 {
        ip.x40_vel.x = mem(fp::fmadds(reg(x2c), reg(dir), reg(vx)));
    }
    let vx = ip.x40_vel.x;
    let abs = if vx < 0.0 { mem(fp::fneg(reg(vx))) } else { vx };
    if abs < x30 {
        ip.x40_vel.x = 0.0;
    }
}

/// Blows up, `it_8029F69C`: its hit ids new, let go of with its owner's sword put back if
/// held, hidden and out of reach, still, vanishing as it blasts, and a wind (state 5).
#[doc(alias = "it_8029F69C")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    hitbox::set_hit_ids(r.game, gobj);
    if item(r.game, gobj).xDC8_word.flags.x13 {
        release::let_go(r, gobj, ZERO);
        hitbox::start_blink(r.game, gobj);
        let owner = item(r.game, gobj).owner;
        if let Some(o) = owner
            && vars(r.game, gobj).x10.get() == Some(o)
        {
            r.link_attack_air_setup_parts(o);
        }
    }
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    lib26b1::set_xdd0_b3(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    hitbox::start_blink(r.game, gobj);
    lib2725::blast_large(r, gobj);
    change_keeping_bone(r, gobj, 5, 0);
    let pos = item(r.game, gobj).pos;
    crate::lb::wind::blast(
        &mut r.game.gr.winds,
        pos,
        0x78,
        1.0,
        0.02,
        std::f32::consts::FRAC_PI_3,
    );
}

/// `it_8029F960` (it dealt a hit): unless blowing up or blown up by the speed, it bounces back
/// the other way.
#[doc(alias = "it_8029F960")]
fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != 5 {
        let vel = item(r.game, gobj).x40_vel;
        if !too_fast(r, gobj, vel) {
            let (x1c, x20) = (attr_f32(r, gobj, attr::X1C), attr_f32(r, gobj, attr::X20));
            let ip = item_mut(r.game, gobj);
            ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
            ip.x40_vel.x = mem(fp::fmuls(reg(x1c), reg(ip.facing_dir)));
            ip.x40_vel.y = x20;
        }
    }
    false
}

/// `itLinkBomb_Logic16_DmgReceived`: enough damage (the attributes' `x10`) blows it up; else,
/// the first time, it's knocked away from the hit (or, slow, a random way across).
#[doc(alias = "itLinkBomb_Logic16_DmgReceived")]
fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 5 {
        return false;
    }
    let x10 = attr_u32(r, gobj, attr::X10) as i32;
    if item(r.game, gobj).xCA0 >= x10 {
        explode(r, gobj);
        return false;
    }
    if vars(r.game, gobj).x0.get().knocked {
        return false;
    }
    let (x14, x18, x30) = (
        attr_f32(r, gobj, attr::X14),
        attr_f32(r, gobj, attr::X18),
        attr_f32(r, gobj, attr::X30),
    );
    if item(r.game, gobj).x40_vel.x > x30 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.xCCC_incDamageDirection)));
        ip.x40_vel.x = mem(fp::fmuls(reg(x14), reg(ip.facing_dir)));
    } else {
        let rand = reg(r.game.objs.rng.draw_f32());
        let way = mem(fp::fmuls(2.0, fp::fsubs(rand, 0.5)));
        item_mut(r.game, gobj).x40_vel.x = mem(fp::fmuls(reg(x14), reg(way)));
        lib2725::face_motion(r.game, gobj);
    }
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fmuls(reg(x18), reg(ip.facing_dir)));
    vars(r.game, gobj)
        .x0
        .update(|f| LinkBombFlags { knocked: true, ..f });
    false
}

/// `itLinkBomb_Logic16_EnteredAir` (state 6, its animation from its start).
#[doc(alias = "itLinkBomb_Logic16_EnteredAir")]
fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// `it_8029FD84`: forgets `other`, its puller among them.
#[doc(alias = "it_8029FD84")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = vars(r.game, gobj);
    if v.x10.get() == other {
        v.x10.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}

/// `itLinkbomb_UnkMotion0_Phys`, `itLinkbomb_UnkMotion5_Phys` and
/// `itLinkbomb_UnkMotion6_Phys`: nothing moves it.
#[doc(alias = "itLinkbomb_UnkMotion0_Phys")]
#[doc(alias = "itLinkbomb_UnkMotion5_Phys")]
#[doc(alias = "itLinkbomb_UnkMotion6_Phys")]
fn still(_: &mut Running, _: GObjId) {}

/// `itLinkbomb_UnkMotion5_Anim`: the blast lasts its life; `itLinkbomb_UnkMotion5_Coll` has
/// nothing to do.
#[doc(alias = "itLinkbomb_UnkMotion5_Anim")]
#[doc(alias = "itLinkbomb_UnkMotion5_Coll")]
fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::tick_life(r.game, gobj)
}

/// `itLinkbomb_UnkMotion1_Coll`: falling, it rolls when it lands.
#[doc(alias = "itLinkbomb_UnkMotion1_Coll")]
fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall(r, gobj, to_roll);
    false
}

/// `itLinkbomb_UnkMotion4_Coll`: rolling, it falls off an edge.
#[doc(alias = "itLinkbomb_UnkMotion4_Coll")]
fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, to_fall);
    false
}

/// `itLinkbomb_UnkMotion6_Coll`: knocked along the ground, it rolls when it stops and falls
/// off an edge.
#[doc(alias = "itLinkbomb_UnkMotion6_Coll")]
fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_skid(r, gobj, to_roll, to_fall);
    false
}

/// `itLinkBomb_Logic16_Reflected`: sent back.
#[doc(alias = "itLinkBomb_Logic16_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// `itLinkBomb_Logic16_HitShield`: bounces off.
#[doc(alias = "itLinkBomb_Logic16_HitShield")]
fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// `itLinkBomb_Logic16_ShieldBounced`: bounces off the shield.
#[doc(alias = "itLinkBomb_Logic16_ShieldBounced")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}
