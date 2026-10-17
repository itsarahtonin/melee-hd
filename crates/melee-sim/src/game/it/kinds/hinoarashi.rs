//! Cyndaquil (`It_PKind_Hinoarashi`), `melee/it/kinds/ithinoarashi.c`: out of the ball it
//! lands and blows flames (`It_Kind_Hinoarashi_Flame`) from its back as its script says, until
//! its animation ends; hit hard enough, it's launched and falls away.
//!
//! The flame's state and callbacks (`it_802D66F8` to `it_802D6850`) are at the top of
//! `itmaril.c`; their collision turns the flame with [`touch_normal`] and
//! [`turn_off_surface`].

use gekko_fp as fp;

use super::super::ground::air;
use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, effect_hitlag, table_bone, take_bone_motion};
use super::super::{hitbox, item, item_mut, jobj_of, lib26b1, lib2725, maplib, set_flag};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::it::data;
use crate::game::it::item::attr_of;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802D_5CF8;
pub const ACCESSORY: u32 = 0x802D_5D7C;
pub const ANIM0: u32 = 0x802D_5EC8;
pub const PHYS0: u32 = 0x802D_5EEC;
pub const COLL0: u32 = 0x802D_5F0C;
pub const LANDED: u32 = 0x802D_5F34;
pub const ANIM1: u32 = 0x802D_5FC8;
pub const PHYS1: u32 = 0x802D_6084;
pub const COLL1: u32 = 0x802D_60A4;
pub const ANIM2: u32 = 0x802D_60C8;
pub const PHYS2: u32 = 0x802D_60D0;
pub const DMG_RECEIVED: u32 = 0x802D_6100;
pub const EVT_UNK: u32 = 0x802D_61A8;

/// `It_Kind_Hinoarashi_Flame`.
pub const FLAME: i32 = super::HINOARASHI_FLAME;

/// Cyndaquil's variables (`itHinoarashi_ItemVars`): its flags (`x60`: `b0` its back's flame
/// effect lit, `b1` its flames out) and the frames to its next flame sound (`x64`).
const FLAGS: usize = 0x60;
const LIT: u8 = 0x80;
const OUT: u8 = 0x40;
const SOUND_FRAMES: usize = 0x64;

/// A flame's variables (the same union): the normal it last touched (`x60`, a vector), its
/// heading (`x6C`, `itMaril_ItemVars`' unit velocity) and its angle (`x78`).
pub const NORMAL: usize = 0x60;
pub const HEADING: usize = 0x6C;
pub const ANGLE: usize = 0x78;

const PI: f64 = std::f64::consts::PI;
const TAU: f64 = std::f64::consts::TAU;

/// Runs one of Cyndaquil's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        DMG_RECEIVED => dmg_received(r, gobj),
        _ => return None,
    })
}

/// Runs one of Cyndaquil's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        ACCESSORY => accessory(r, gobj),
        PHYS0 => phys0(r, gobj),
        LANDED => landed(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        _ => return false,
    }
    true
}

/// Cyndaquil out of the ball, `it_802D5CF8`: no flames yet, at its first size (its own),
/// its sound, falling out of the ball.
#[doc(alias = "it_802D5CF8")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    let scl = ip.scl;
    let v = pokemon::vars(r.game, gobj);
    v.set_bit(FLAGS, LIT, false);
    v.set_bit(FLAGS, OUT, false);
    v.set_f32(SOUND_FRAMES, 0.0);
    pokemon::start(r, gobj, scl);
    play_sfx(r, gobj, 0x2734, 0x7F, 0x40);
    enter_air(r, gobj);
}

/// Its script's flames, `it_802D5D7C` (the accessory callback): a flame from its back's bone
/// (lighting the back's effect the first time) when the script asks (`xDAC`), and the flames
/// out when it asks (`xDB0`).
#[doc(alias = "it_802D5D7C")]
pub fn accessory(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        if !pokemon::vars(r.game, gobj).bit(FLAGS, LIT) {
            pokemon::vars(r.game, gobj).set_bit(FLAGS, LIT, true);
            let bone = table_bone(r.game, gobj, 2);
            ef::spawn(r, 0x473, Some(gobj), &[Arg::JObj(bone)]);
        }
        let bone = table_bone(r.game, gobj, 2);
        let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, bone, None);
        let id = super::super::item::next_attack_id(r.game);
        let facing = item(r.game, gobj).facing_dir;
        spawn_flame(r, gobj, pos, id, facing);
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        ef::destroy_all(r, gobj);
        pokemon::vars(r.game, gobj).set_bit(FLAGS, OUT, true);
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
    }
}

/// Into the air falling out of the ball, `it_802D5E4C`: posed at its first frame, no longer
/// kept clear of.
#[doc(alias = "it_802D5E4C")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    super::drmariopill::set_pose(r, gobj, 0, 0.0);
    lib26b1::stop_keeping_clear(r.game, gobj);
}

/// `it_802D5EC8`: growing out of the ball.
#[doc(alias = "it_802D5EC8")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `it_802D5EEC`: falling out of the ball.
#[doc(alias = "it_802D5EEC")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    pokemon::fall(r, gobj);
}

/// `it_802D5F0C`: lands and blows flames.
#[doc(alias = "it_802D5F0C")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}

/// Lands and blows flames, `it_802D5F34`: done blinking, hittable, stopped, its script's
/// flames on.
#[doc(alias = "it_802D5F34")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    hitbox::end_blink(r.game, gobj);
    hitbox::set_hit_ids2(r.game, gobj);
    hitbox::clear_xdce_b2(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(ACCESSORY));
    set_flag(&mut ip.xDD1_flag, 1, true);
}

/// `itHinoarashi_UnkMotion1_Anim`: gone when its animation ends; while its flames burn, a
/// flame sound every few frames (`x8`); the bone's motion.
#[doc(alias = "itHinoarashi_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        return true;
    }
    let v = pokemon::vars(r.game, gobj);
    if v.bit(FLAGS, LIT) && !v.bit(FLAGS, OUT) {
        if reg(v.f32(SOUND_FRAMES)) == 0.0 {
            let frames = attr_f32(r, gobj, 8);
            pokemon::vars(r.game, gobj).set_f32(SOUND_FRAMES, frames);
            lib2725::play_random_sfx2(r, gobj, 3, 0x2735);
        }
        let v = pokemon::vars(r.game, gobj);
        let n = mem(fp::fsubs(reg(v.f32(SOUND_FRAMES)), 1.0));
        v.set_f32(SOUND_FRAMES, n);
    }
    let bone = table_bone(r.game, gobj, 1);
    take_bone_motion(r, bone, gobj);
    false
}

/// `itHinoarashi_UnkMotion1_Phys`: the bone's motion.
#[doc(alias = "itHinoarashi_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    pokemon::move_by_bone(r.game, gobj);
}

/// `itHinoarashi_UnkMotion1_Coll`.
#[doc(alias = "itHinoarashi_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj);
    false
}

/// `itHinoarashi_UnkMotion2_Anim`: launched, nothing.
#[doc(alias = "itHinoarashi_UnkMotion2_Anim")]
pub fn anim2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// `itHinoarashi_UnkMotion2_Phys`: launched, falling by the item's gravity.
#[doc(alias = "itHinoarashi_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let (g, max) = (
        a.f32(r.data, data::attr::FALL_SPEED),
        a.f32(r.data, data::attr::FALL_SPEED_MAX),
    );
    lib2725::fall(r.game, gobj, g, max);
}

/// `it_2725_Logic27_DmgReceived`: hit for its damage limit (`x4`) or more, it's launched in
/// the air, blinking, its flames out, posed at its launch's first frame.
#[doc(alias = "it_2725_Logic27_DmgReceived")]
pub fn dmg_received(r: &mut Running, gobj: GObjId) -> bool {
    let limit = attr_f32(r, gobj, 4);
    let taken = item(r.game, gobj).xC9C;
    if fp::frsp(f64::from(taken)) >= reg(limit) {
        pokemon::launch(r, gobj);
        maplib::enter_air(item_mut(r.game, gobj));
        hitbox::start_blink(r.game, gobj);
        ef::destroy_all(r, gobj);
        change_state(r, gobj, 2, change::ANIM_UPDATE);
        super::drmariopill::set_pose(r, gobj, 0, 0.0);
    }
    false
}

/// Forgets a GObj going away, `it_802D61A8` (`it_8026B894`).
#[doc(alias = "it_802D61A8")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Wraps an angle to within half a turn of zero, the larger side first
/// (`Item_ClampAngleReverse`, or with `low_first` `Item_ClampAngle`), in double precision.
fn wrap_angle(a: f32, low_first: bool) -> f32 {
    let mut a = reg(a);
    let low = |a: &mut f64| {
        while *a < fp::fneg(PI) {
            *a = fp::frsp(fp::fadd(*a, TAU));
        }
    };
    let high = |a: &mut f64| {
        while *a > PI {
            *a = fp::frsp(fp::fsub(*a, TAU));
        }
    };
    if low_first {
        low(&mut a);
        high(&mut a);
    } else {
        high(&mut a);
        low(&mut a);
    }
    mem(a)
}

/// The flame's angle wrapped, and the normal of the surfaces `coll` says it touched, summed and
/// made unit, `it_802D61C8`.
#[doc(alias = "it_802D61C8")]
pub fn touch_normal(r: &mut Running, gobj: GObjId, coll: i32) {
    let v = pokemon::vars(r.game, gobj);
    let a = wrap_angle(v.f32(ANGLE), true);
    v.set_f32(ANGLE, a);
    let c = &item(r.game, gobj).x378_itemColl;
    let mut n = Vec3::new(0.0, 0.0, 0.0);
    for (bit, s) in [
        (1, c.floor.normal),
        (2, c.ceiling.normal),
        (8, c.left_facing_wall.normal),
        (4, c.right_facing_wall.normal),
    ] {
        if coll & bit != 0 {
            n.x = mem(fp::fadds(reg(n.x), reg(s.x)));
            n.y = mem(fp::fadds(reg(n.y), reg(s.y)));
        }
    }
    n.normalize();
    pokemon::vars(r.game, gobj).set_vec(NORMAL, n);
}

/// Turns the flame's angle off the surface it touched, `it_802D6310`: by the angle between
/// its heading and the normal, scaled by how far round the heading is from the normal (a
/// fiftieth under a quarter turn, half beyond).
#[doc(alias = "it_802D6310")]
pub fn turn_off_surface(r: &mut Running, gobj: GObjId, coll: i32) {
    if coll == 0 {
        return;
    }
    let v = pokemon::vars(r.game, gobj);
    let (n, h) = (v.vec(NORMAL), v.vec(HEADING));
    let target = reg(crate::math::atan2(n.x, n.y));
    let delta = mem(fp::fsubs(reg(crate::math::atan2(h.x, h.y)), target));
    let delta = wrap_angle(delta, false);
    let factor = if reg(delta) == 0.0 {
        0.0
    } else {
        let angle = n.angle(h);
        let d = reg(delta);
        let abs = if d < 0.0 { fp::fneg(d) } else { d };
        let k = if abs < fp::fdiv(PI, 2.0) {
            reg(0.02f32)
        } else {
            0.5
        };
        let scale = fp::frsp(fp::fmul(k, fp::fdiv(abs, PI)));
        let signed = if d < 0.0 { fp::fneg(scale) } else { scale };
        fp::fmuls(reg(angle), signed)
    };
    let v = pokemon::vars(r.game, gobj);
    let a = mem(fp::fsubs(reg(v.f32(ANGLE)), factor));
    v.set_f32(ANGLE, a);
    let a = wrap_angle(v.f32(ANGLE), true);
    v.set_f32(ANGLE, a);
}

/// Blows a flame from `pos`, `it_802D64B8`: its life (`x0`), a random angle up its way
/// (`x10` to `x14` from straight down) and speed (`x4` to `x8`), one of four effects.
#[doc(alias = "it_802D64B8")]
pub fn spawn_flame(r: &mut Running, gobj: GObjId, pos: Vec3, attack_id: u32, facing_dir: f32) {
    let owner = item(r.game, gobj).owner;
    let mut spawn = SpawnItem {
        kind: FLAME,
        prev_pos: Vec3::new(pos.x, pos.y, 0.0),
        pos: lib26b1::ecb_center(r.game, gobj),
        facing_dir,
        x3C_damage: 0,
        vel: Vec3::new(0.0, 0.0, 0.0),
        x0_parent_gobj: owner,
        x4_parent_gobj2: Some(gobj),
        x40: attack_id as i32,
        ..Default::default()
    };
    spawn.x44_flag[0] |= 0x80;
    let Some(flame) = spawn_air(r, &mut spawn) else {
        return;
    };
    let life = attr_f32(r, flame, 0);
    lib2725::set_lifetime(r.game, r.data, flame, life);
    let (lo, hi) = (attr_f32(r, flame, 0x10), attr_f32(r, flame, 0x14));
    let rand = reg(r.game.objs.rng.draw_f32());
    let a = fp::fmadds(fp::fsubs(reg(hi), reg(lo)), rand, reg(lo));
    let a = if reg(item(r.game, flame).facing_dir) == 1.0 {
        a
    } else {
        fp::fneg(a)
    };
    let mut angle = fp::frsp(fp::fsub(a, fp::fdiv(PI, 2.0)));
    while angle < 0.0 {
        angle = fp::frsp(fp::fadd(angle, TAU));
    }
    while angle > TAU {
        angle = fp::frsp(fp::fsub(angle, TAU));
    }
    let angle = mem(angle);
    pokemon::vars(r.game, flame).set_f32(ANGLE, angle);
    let (slow, fast) = (attr_f32(r, flame, 4), attr_f32(r, flame, 8));
    let rand = reg(r.game.objs.rng.draw_f32());
    let speed = fp::fmadds(fp::fsubs(reg(fast), reg(slow)), rand, reg(slow));
    let ip = item_mut(r.game, flame);
    ip.x40_vel.x = mem(fp::fmuls(speed, reg(crate::math::cos(angle))));
    ip.x40_vel.y = mem(fp::fmuls(speed, reg(crate::math::sin(angle))));
    ip.x40_vel.z = 0.0;
    ip.xDCC_flag.b3 = false;
    let which = r.game.objs.rng.below(4);
    flame_start(r, flame, which);
}

/// The flame flies, `it_802D6674`: effect `0x44E` and the three after it, `which`.
#[doc(alias = "it_802D6674")]
pub fn flame_start(r: &mut Running, gobj: GObjId, which: i32) {
    let j = jobj_of(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let facing = item(r.game, gobj).facing_dir;
    ef::spawn(
        r,
        which.wrapping_add(0x44E),
        Some(gobj),
        &[Arg::JObj(j), Arg::Float(facing), Arg::Float(1.0)],
    );
    lib26b1::disable_pickup(r.game, gobj);
}
