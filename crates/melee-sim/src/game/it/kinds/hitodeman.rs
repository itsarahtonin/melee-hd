//! Staryu (`It_PKind_Hitodeman`) and its star (`It_Kind_Hitodeman_Star`),
//! `melee/it/kinds/ithitodeman.c`: out of the ball it follows the opponent with the lowest
//! percent, keeping a random offset from them, then turns to face them and fires stars a few
//! times, kicked back by each, and goes when it runs out (or a fighter it follows unloads).

use gekko_fp as fp;

use super::super::item::{change, change_state, play_sfx, spawn_air};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib, owner};
use crate::fpu::{mem, reg};
use crate::game::ef::{self, Arg};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::lb::sound_obj::SoundParams;
use crate::vec::Vec3;

/// `It_PKind_Hitodeman` and `It_Kind_Hitodeman_Star`.
pub const KIND: i32 = 0xB9;
pub const STAR: i32 = 0xCC;

pub const SPAWNED: u32 = 0x802D_42C4;
pub const LANDED_NOTHING: u32 = 0x802D_43AC;
pub const EVT_UNK: u32 = 0x802D_43B0;
pub const ANIM0: u32 = 0x802D_4564;
pub const PHYS0: u32 = 0x802D_472C;
pub const COLL0: u32 = 0x802D_48A8;
pub const ANIM1: u32 = 0x802D_49E8;
pub const PHYS1: u32 = 0x802D_4B14;
pub const COLL1: u32 = 0x802D_4B48;
pub const ACCESSORY: u32 = 0x802D_4B50;
pub const ANIM2: u32 = 0x802D_4BB8;
pub const PHYS2: u32 = 0x802D_4BDC;
pub const COLL2: u32 = 0x802D_4C4C;
pub const STAR_SPAWNED: u32 = 0x802D_4DE0;
pub const STAR_DMG_DEALT: u32 = 0x802D_4EF4;
pub const STAR_SHIELD_BOUNCED: u32 = 0x802D_4F08;
pub const STAR_REFLECTED: u32 = 0x802D_4F28;
pub const STAR_ABSORBED: u32 = 0x802D_4F48;
pub const STAR_HIT_SHIELD: u32 = 0x802D_4F50;
pub const STAR_EVT_UNK: u32 = 0x802D_4F58;
pub const STAR_ANIM: u32 = 0x802D_4FFC;
pub const STAR_PHYS: u32 = 0x802D_5044;
pub const STAR_COLL: u32 = 0x802D_5048;

/// Its variables past the release's (`itHitodeman_ItemVars`): its offset from the fighter it
/// follows (`x60`, `x64`), the speeds its approach keeps (`x6C`, `x70`; to `x80` cleared), the
/// frames it follows (`x84`), the shots left (`x88`), the frames to the next (`x8C`), and the
/// fighter it follows (`x90`).
const OFFSET_X: usize = 0x60;
const OFFSET_Y: usize = 0x64;
const SPEED_X: usize = 0x6C;
const SPEED_Y: usize = 0x70;
const CLEARED_END: usize = 0x80;
const FOLLOW_FRAMES: usize = 0x84;
const SHOTS: usize = 0x88;
const SHOT_FRAMES: usize = 0x8C;
const TARGET: usize = 0x90;

/// The words it keeps a handle in, for the importer.
pub const HANDLES: &[(usize, &str)] = &[(TARGET, "GObjId")];

/// Its cry, and the star's sounds by the way it flies.
const CRY: i32 = 0x2738;
const STAR_RIGHT: i32 = 0x2739;
const STAR_LEFT: i32 = 0x273A;

/// A quarter turn, as the inline `HSD_JObjSetRotationY` takes it (`1.5707964f`).
const QUARTER_TURN: f32 = std::f32::consts::FRAC_PI_2;

/// Runs one of Staryu's or its star's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        STAR_DMG_DEALT => star_dmg_dealt(r, gobj),
        STAR_SHIELD_BOUNCED => star_shield_bounced(r, gobj),
        STAR_REFLECTED => star_reflected(r, gobj),
        STAR_ABSORBED => star_absorbed(r, gobj),
        STAR_HIT_SHIELD => star_hit_shield(r, gobj),
        STAR_ANIM => star_anim(r, gobj),
        STAR_COLL => star_coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of Staryu's or its star's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED_NOTHING => landed_nothing(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        ACCESSORY => accessory(r, gobj),
        PHYS2 => phys2(r, gobj),
        STAR_SPAWNED => star_spawned(r, gobj),
        STAR_PHYS => star_phys(r, gobj),
        _ => return false,
    }
    true
}

/// Staryu out of the ball, `it_2725_Logic24_Spawned`: facing nowhere, at its first size, its
/// fighter to follow and its offset, how long it follows and how many shots it has, its
/// approach's speeds cleared, into the air out of the ball, and its cry.
#[doc(alias = "it_2725_Logic24_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDBC[0] &= !0x80;
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
    choose_target(r, gobj);
    choose_follow_frames(r, gobj);
    choose_shots(r, gobj);
    let every = attr_i32(r, gobj, 0x3C);
    let v = pokemon::vars(r.game, gobj);
    v.set_f32(SHOT_FRAMES, mem(fp::frsp(f64::from(every))));
    for off in (SPEED_X..=CLEARED_END).step_by(4) {
        v.set_f32(off, 0.0);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    enter_air(r, gobj);
    play_sfx(r, gobj, CRY, 0x7F, 0x40);
}

/// Lands out of the ball: nothing, `it_802D43AC`.
#[doc(alias = "it_802D43AC")]
pub fn landed_nothing(_: &mut Running, _: GObjId) {}

/// Forgets a GObj going away, `it_802D43B0` (`it_8026B894`): no fighter to follow any more,
/// and it goes.
#[doc(alias = "it_802D43B0")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
    pokemon::vars(r.game, gobj).set_gobj(TARGET, None);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
}

/// Its fighter and offset, `it_802D43EC`: the opponent of its owner with the lowest percent
/// (its owner with none), an offset from them random between its attributes' bounds (`x8` to
/// `x4` across, `x10` to `xC` up), on a random side.
#[doc(alias = "it_802D43EC")]
pub fn choose_target(r: &mut Running, gobj: GObjId) {
    let own = item(r.game, gobj).owner;
    let target = owner::lowest_percent_opponent(r.game, own).or(own);
    pokemon::vars(r.game, gobj).set_gobj(TARGET, target);
    let (x4, x8) = (attr_f32(r, gobj, 4), attr_f32(r, gobj, 8));
    let randf = reg(r.game.objs.rng.draw_f32());
    let diff = fp::fsubs(reg(x4), reg(x8));
    let x = mem(fp::fmadds(diff, randf, reg(x8)));
    pokemon::vars(r.game, gobj).set_f32(OFFSET_X, x);
    let (xc, x10) = (attr_f32(r, gobj, 0xC), attr_f32(r, gobj, 0x10));
    let randf = reg(r.game.objs.rng.draw_f32());
    let diff = fp::fsubs(reg(xc), reg(x10));
    let y = mem(fp::fmadds(diff, randf, reg(x10)));
    pokemon::vars(r.game, gobj).set_f32(OFFSET_Y, y);
    if r.game.objs.rng.below(2) != 0 {
        let v = pokemon::vars(r.game, gobj);
        let x = v.f32(OFFSET_X);
        v.set_f32(OFFSET_X, mem(fp::fmuls(reg(x), fp::fneg(1.0))));
    }
}

/// How long it follows, `it_802D4494`: `x18` frames and up to `x14 - x18` more.
#[doc(alias = "it_802D4494")]
pub fn choose_follow_frames(r: &mut Running, gobj: GObjId) {
    let (x14, x18) = (attr_f32(r, gobj, 0x14), attr_f32(r, gobj, 0x18));
    let n = fp::fctiwz(fp::fsubs(reg(x14), reg(x18)));
    let more = r.game.objs.rng.below(n);
    let frames = mem(fp::fadds(reg(x18), fp::frsp(f64::from(more))));
    pokemon::vars(r.game, gobj).set_f32(FOLLOW_FRAMES, frames);
}

/// How many shots it fires, `it_802D4510`: `x44` and up to `x40 - x44` more.
#[doc(alias = "it_802D4510")]
pub fn choose_shots(r: &mut Running, gobj: GObjId) {
    let (x40, x44) = (attr_i32(r, gobj, 0x40), attr_i32(r, gobj, 0x44));
    let more = r.game.objs.rng.below(x40.wrapping_sub(x44));
    pokemon::vars(r.game, gobj).set_i32(SHOTS, x44.wrapping_add(more));
}

/// `it_802D4564`, following: gone once told to; its animation looping; when its frames to
/// follow run out, it faces its fighter (or its owner's nearest opponent, with none), stops,
/// cries and starts shooting.
#[doc(alias = "it_802D4564")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        return true;
    }
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
    }
    let v = pokemon::vars(r.game, gobj);
    let left = mem(fp::fsubs(reg(v.f32(FOLLOW_FRAMES)), fp::frsp(1.0)));
    v.set_f32(FOLLOW_FRAMES, left);
    if reg(left) < 0.0 {
        match pokemon::vars(r.game, gobj).gobj(TARGET) {
            Some(target) => {
                let j = jobj_of(r.game, gobj).expect("Staryu without a model");
                let t = r.game.ft_pos(target);
                let ip = item_mut(r.game, gobj);
                ip.facing_dir = if reg(t.x) < reg(ip.pos.x) { -1.0 } else { 1.0 };
                face(r, gobj, j);
            }
            None => pokemon::face_nearest_opponent(r, gobj),
        }
        start_shooting(r, gobj);
        item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
        play_sfx(r, gobj, CRY, 127, 64);
    }
    false
}

/// Turns its model to face its way (the inline `HSD_JObjSetRotationY` with a quarter turn).
fn face(r: &mut Running, gobj: GObjId, j: crate::hsd::jobj::JObjId) {
    let facing = item(r.game, gobj).facing_dir;
    let y = mem(fp::fmuls(reg(QUARTER_TURN), reg(facing)));
    lib2725::set_rotation_y(r.game, j, y);
}

/// `it_802D472C`, following: toward its fighter plus its offset, faster the farther (`x34`
/// per unit past `x38`), each axis's speed eased toward that ([`approach`]).
#[doc(alias = "it_802D472C")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let Some(target) = pokemon::vars(r.game, gobj).gobj(TARGET) else {
        return;
    };
    let mut t = r.game.ft_pos(target);
    let v = pokemon::vars(r.game, gobj);
    let (ox, oy) = (v.f32(OFFSET_X), v.f32(OFFSET_Y));
    t.x = mem(fp::fadds(reg(t.x), reg(ox)));
    t.y = mem(fp::fadds(reg(t.y), reg(oy)));
    t.z = 0.0;
    let mut dir = t - item(r.game, gobj).pos;
    let dist = dir.length();
    if reg(dist) > reg(attr_f32(r, gobj, 0x38)) {
        dir.normalize();
        let speed = fp::fmuls(reg(dist), reg(attr_f32(r, gobj, 0x34)));
        dir.x = mem(fp::fmuls(reg(dir.x), speed));
        dir.y = mem(fp::fmuls(reg(dir.y), speed));
        dir.z = mem(fp::fmuls(reg(dir.z), speed));
    }
    let a = |off| reg(attr_f32(r, gobj, off));
    let (max_x, accel_x, decel_x) = (a(0x24), a(0x20), a(0x1C));
    let (max_y, accel_y, decel_y) = (a(0x30), a(0x2C), a(0x28));
    let v = pokemon::vars(r.game, gobj);
    let mut sx = v.f32(SPEED_X);
    let x = approach(&mut sx, reg(dir.x), max_x, accel_x, decel_x);
    v.set_f32(SPEED_X, sx);
    item_mut(r.game, gobj).x40_vel.x = mem(x);
    let v = pokemon::vars(r.game, gobj);
    let mut sy = v.f32(SPEED_Y);
    let y = approach(&mut sy, reg(dir.y), max_y, accel_y, decel_y);
    v.set_f32(SPEED_Y, sy);
    item_mut(r.game, gobj).x40_vel.y = mem(y);
}

/// `it_802D48A8`: nothing.
#[doc(alias = "it_802D48A8")]
pub fn coll0(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Eases a speed toward `target`, `it_802D48B0`: by `accel` at most when farther than that,
/// by `decel` at least when nearer than that, within `max` either way; the speed kept in
/// `value` and returned.
#[doc(alias = "it_802D48B0")]
pub fn approach(value: &mut f32, target: f64, max: f64, accel: f64, decel: f64) -> f64 {
    let abs = |v: f64| if v < 0.0 { fp::fneg(v) } else { v };
    let current = reg(*value);
    let diff = fp::fsubs(target, current);
    let mut out = target;
    if abs(diff) > abs(accel) {
        out = if diff > 0.0 {
            fp::fadds(current, accel)
        } else {
            fp::fsubs(current, accel)
        };
    } else if abs(diff) < abs(decel) {
        out = if diff > 0.0 {
            fp::fadds(current, decel)
        } else {
            fp::fsubs(current, decel)
        };
    }
    if out > 0.0 {
        if out > max {
            out = max;
        }
    } else if out < fp::fneg(max) {
        out = fp::fneg(max);
    }
    *value = mem(out);
    out
}

/// Starts shooting, `it_802D4990` (and `itHitodeman_UnkMotion1_Anim`'s inline copy): its
/// second state, its `on_accessory` the one that does nothing.
#[doc(alias = "it_802D4990")]
pub fn start_shooting(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
}

/// `itHitodeman_UnkMotion1_Anim`, shooting: gone once told to; every `x3C` frames a shot,
/// gone when none are left; each shot (with a fighter to shoot at) a star and a kick back
/// (`x48`), slowing (`x4C`, [`phys1`]); its animation looping.
#[doc(alias = "itHitodeman_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        return true;
    }
    let v = pokemon::vars(r.game, gobj);
    let timer = mem(fp::fsubs(reg(v.f32(SHOT_FRAMES)), 1.0));
    v.set_f32(SHOT_FRAMES, timer);
    if reg(timer) < 0.0 {
        let shots = v.i32(SHOTS).wrapping_sub(1);
        v.set_i32(SHOTS, shots);
        if shots < 0 {
            return true;
        }
        let every = attr_i32(r, gobj, 0x3C);
        pokemon::vars(r.game, gobj).set_f32(SHOT_FRAMES, mem(fp::frsp(f64::from(every))));
        if pokemon::vars(r.game, gobj).gobj(TARGET).is_some() {
            let kick = attr_f32(r, gobj, 0x48);
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(fp::fneg(reg(ip.facing_dir)), reg(kick)));
            shoot(r, gobj);
            item_mut(r.game, gobj).xDB0_itcmd_var1 = 1;
        }
    }
    if !lib2725::is_animating(r.game, gobj) {
        start_shooting(r, gobj);
    }
    false
}

/// `itHitodeman_UnkMotion1_Phys`: once it has shot, its kick back slows by `x4C` a frame.
#[doc(alias = "itHitodeman_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDB0_itcmd_var1 == 0 {
        return;
    }
    let slow = attr_f32(r, gobj, 0x4C);
    let ip = item_mut(r.game, gobj);
    let v = fp::fnmsubs(fp::fneg(reg(ip.facing_dir)), reg(slow), reg(ip.x40_vel.x));
    ip.x40_vel.x = mem(v);
}

/// `itHitodeman_UnkMotion1_Coll`: nothing.
#[doc(alias = "itHitodeman_UnkMotion1_Coll")]
pub fn coll1(_: &mut Running, _: GObjId) -> bool {
    false
}

/// Its shooting state's `on_accessory`, `it_802D4B50`: nothing.
#[doc(alias = "it_802D4B50")]
pub fn accessory(_: &mut Running, _: GObjId) {}

/// Into the air out of the ball, `it_802D4B54` (`Item_EnterAirStateWithHitlag`, state 2).
#[doc(alias = "it_802D4B54")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
}

/// `itHitodeman_UnkMotion2_Anim`: growing out of the ball.
#[doc(alias = "itHitodeman_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itHitodeman_UnkMotion2_Phys`: falling out of the ball; at the end stopped, following.
#[doc(alias = "itHitodeman_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
    }
}

/// `itHitodeman_UnkMotion2_Coll`: lands out of the ball doing nothing.
#[doc(alias = "itHitodeman_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed_nothing)
}

/// Fires a star, `it_802D4C74`: ahead (`x54`) and up (`x58`) of it at its facing's speed
/// (`x50`), with the star's sound for the way it flies.
#[doc(alias = "it_802D4C74")]
pub fn shoot(r: &mut Running, gobj: GObjId) {
    let (ahead, up, speed) = (
        attr_f32(r, gobj, 0x54),
        attr_f32(r, gobj, 0x58),
        attr_f32(r, gobj, 0x50),
    );
    let ip = item(r.game, gobj);
    let (pos, facing, own) = (ip.pos, ip.facing_dir, ip.owner);
    let mut prev = pos;
    prev.x = mem(fp::fmadds(reg(facing), reg(ahead), reg(prev.x)));
    prev.y = mem(fp::fadds(reg(prev.y), reg(up)));
    let mut spawn = SpawnItem {
        kind: STAR,
        ..Default::default()
    };
    spawn.prev_pos = prev;
    spawn.pos = lib26b1::ecb_center(r.game, gobj);
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(mem(fp::fmuls(reg(facing), reg(speed))), 0.0, 0.0);
    spawn.x0_parent_gobj = own;
    spawn.x4_parent_gobj2 = Some(gobj);
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    if spawn_air(r, &mut spawn).is_some() {
        let sfx = if facing == 1.0 { STAR_RIGHT } else { STAR_LEFT };
        r.lb_create_sound_object(&SoundParams {
            owner: gobj,
            behavior: 1,
            sfx_id: sfx,
            start_vol: 0x7F,
            end_vol: 0x7F,
            pan_left: 0,
            pan_right: 0x7F,
            end_frame: 0x3C,
            direction: 3.0,
            track: 0,
            voice: -1,
        });
    }
}

/// The star's `it_2725_Logic43_Spawned`: its life, not spinning, flying, not pickable, turned
/// to its way.
#[doc(alias = "it_2725_Logic43_Spawned")]
pub fn star_spawned(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a star without a model");
    let life = attr_f32(r, gobj, 0);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    item_mut(r.game, gobj).xDB4_itcmd_var2 = 0;
    lib2725::stop_spin(r.game, r.data, gobj);
    star_fly(r, gobj);
    lib26b1::disable_pickup(r.game, gobj);
    face(r, gobj, j);
}

/// The star's damage dealt, `it_802D4EF4`: it goes at its next frame.
#[doc(alias = "it_802D4EF4")]
pub fn star_dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).xDB4_itcmd_var2 = 1;
    false
}

/// The star bounced off a shield, `it_802D4F08` (`it_8027AF50`).
#[doc(alias = "it_802D4F08")]
pub fn star_shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::bounce_off_surface(r, gobj)
}

/// The star reflected, `it_802D4F28` (`it_8027AE34`).
#[doc(alias = "it_802D4F28")]
pub fn star_reflected(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::bounce_off_wall(r, gobj)
}

/// The star absorbed, `itHitodeman_Logic43_Absorbed`: gone.
#[doc(alias = "itHitodeman_Logic43_Absorbed")]
pub fn star_absorbed(_: &mut Running, _: GObjId) -> bool {
    true
}

/// The star hitting a shield, `it_802D4F50`: gone.
#[doc(alias = "it_802D4F50")]
pub fn star_hit_shield(_: &mut Running, _: GObjId) -> bool {
    true
}

/// The star forgets a GObj going away, `it_802D4F58` (`it_8026B894`).
#[doc(alias = "it_802D4F58")]
pub fn star_evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// The star flies, `it_802D4F78`: its one state, and its effect (`0x46C`) on its model.
#[doc(alias = "it_802D4F78")]
pub fn star_fly(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    ef::spawn(r, 0x46C, Some(gobj), &[Arg::JObj(j), Arg::Float(1.0)]);
}

/// The star's `it_802D4FFC`: gone when its life runs out or once it has hit.
#[doc(alias = "it_802D4FFC")]
pub fn star_anim(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    let life = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    ip.xD44_lifeTimer = life;
    if reg(life) <= 0.0 {
        return true;
    }
    ip.xDB4_itcmd_var2 != 0
}

/// The star's `it_802D5044`: nothing.
#[doc(alias = "it_802D5044")]
pub fn star_phys(_: &mut Running, _: GObjId) {}

/// The star's `it_802D5048`: nothing.
#[doc(alias = "it_802D5048")]
pub fn star_coll(_: &mut Running, _: GObjId) -> bool {
    false
}
