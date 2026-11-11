//! The Polar Bear, `melee/it/kinds/itwhitebea.c` from `it_802E37BC` on (`It_Kind_Whitebea`; the
//! file's start is Topi's, [`super::oldottosea`]): an adventure enemy the adventure's Icicle
//! Mountain's spawner makes. It falls to the floor, stands a while, then walks the way it faces
//! (a step's sound every 15 frames), turning at walls, ledges and the script's turn points;
//! standing, one draw in two (never after a walk) it crouches and jumps, and its landing speeds
//! the climb up (`grIceMt_801FA6D8`). Hit, it's knocked back, or past its health beaten and
//! flung away (toward the screen by chance); a fighter can pick it up and throw it.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words, then its
//! own ([`off`]).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802E_37BC;
pub const DMG_RECEIVED: u32 = 0x802E_3884;
pub const PICKED_UP: u32 = 0x802E_468C;
pub const DROPPED: u32 = 0x802E_4700;
pub const THROWN: u32 = 0x802E_47C0;
pub const EVT_UNK: u32 = 0x802E_4A24;
pub const ANIM0: u32 = 0x802E_39C8;
pub const PHYS0: u32 = 0x802E_3A10;
pub const COLL0: u32 = 0x802E_3A8C;
pub const ANIM1: u32 = 0x802E_3BB8;
pub const PHYS1: u32 = 0x802E_3C70;
pub const COLL1: u32 = 0x802E_3D38;
pub const ANIM2: u32 = 0x802E_41D4;
pub const PHYS2: u32 = 0x802E_432C;
pub const COLL2: u32 = 0x802E_4330;
pub const ANIM3: u32 = 0x802E_3E5C;
pub const PHYS3: u32 = 0x802E_3E64;
pub const COLL3: u32 = 0x802E_3E94;
pub const ANIM4: u32 = 0x802E_3FAC;
pub const PHYS4: u32 = 0x802E_4018;
pub const COLL4: u32 = 0x802E_4040;
pub const ANIM5: u32 = 0x802E_43C0;
pub const PHYS5: u32 = 0x802E_4424;
pub const COLL5: u32 = 0x802E_4428;
pub const ANIM6: u32 = 0x802E_44E4;
pub const PHYS6: u32 = 0x802E_44EC;
pub const COLL6: u32 = 0x802E_451C;
pub const ANIM7: u32 = 0x802E_45D0;
pub const PHYS7: u32 = 0x802E_464C;
pub const COLL7: u32 = 0x802E_4650;
pub const ANIM8: u32 = 0x802E_46B4;
pub const PHYS8: u32 = 0x802E_46FC;
pub const ANIM9: u32 = 0x802E_4818;
pub const PHYS9: u32 = 0x802E_4860;
pub const COLL9: u32 = 0x802E_4890;
pub const ANIM10: u32 = 0x802E_4914;
pub const PHYS10: u32 = 0x802E_495C;
pub const COLL10: u32 = 0x802E_4960;
pub const ANIM11: u32 = 0x802E_49E0;
pub const PHYS11: u32 = 0x802E_49E8;
pub const COLL11: u32 = 0x802E_4A04;

/// Its variables' offsets in the union (`itWhiteBea_ItemVars`).
pub mod off {
    /// `x18` (the common `x14.y`), cleared as it's flung away.
    pub const X18: usize = 0x18;
    /// `x3C`: a walk's frames left (1 after a jump or a throw, which a walk redraws).
    pub const WALK: usize = 0x3C;
    /// `x40`: a countdown (standing, turning, crouching).
    pub const TIMER: usize = 0x40;
    /// `x44`: the frames to its next step's sound.
    pub const STEP: usize = 0x44;
}

/// Its special attributes (`itWhiteBeaAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a Polar Bear without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

fn attr_h(r: &Running, gobj: GObjId, off: u32) -> i32 {
    i32::from(r.data.u16(attr(r, gobj, off)) as i16)
}

/// `attr->x0`: its health (`x0`) and walking speed (`x4`).
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Polar Bear's health");
    r.data.u32(p) as i32
}

fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a Polar Bear's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

/// `GA_Air`.
const GA_AIR: i32 = 1;
/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;
/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;

/// The collision's facing from the item's (`mpCollSetFacingDir`).
fn set_coll_facing(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let f = if reg(ip.facing_dir) == fp::fneg(1.0) {
        -1
    } else {
        1
    };
    ip.x378_itemColl.set_facing_dir(f);
}

/// Stops (the inline `itResetVelocity`).
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Runs one of the Polar Bear's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        ANIM0 => anim0(r, gobj),
        COLL0 | COLL2 => coll_ground(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM3 | ANIM6 | ANIM11 => answers_false(r, gobj),
        COLL3 => coll3(r, gobj),
        ANIM4 => anim4(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM2 => anim2(r, gobj),
        ANIM5 => anim5(r, gobj),
        COLL5 | COLL7 => coll_ground_or(r, gobj),
        COLL6 => coll6(r, gobj),
        ANIM7 => anim7(r, gobj),
        ANIM8 => anim8(r, gobj),
        ANIM9 => anim9(r, gobj),
        COLL9 => coll9(r, gobj),
        ANIM10 => anim10(r, gobj),
        COLL10 => coll10(r, gobj),
        COLL11 => coll11(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Polar Bear's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS3 | PHYS6 | PHYS9 => phys_fall(r, gobj),
        PHYS4 => phys4(r, gobj),
        PHYS2 | PHYS5 | PHYS7 | PHYS8 | PHYS10 => still(r, gobj),
        PHYS11 => phys11(r, gobj),
        _ => return false,
    }
    true
}

/// The Polar Bear's setup, `it_802E37BC`: the enemies' common variables, facing most fighters,
/// its collision an enemy's, no spin, its walk's frames 1 and no countdown; then it falls.
#[doc(alias = "it_802E37BC")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    zako::face(r, gobj, facing);
    set_coll_facing(r, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::WALK, 1);
    set(r, gobj, off::TIMER, 0);
    fall(r, gobj);
}

/// Takes a hit, `it_802E3884`: its facing kept, the hit's element adds to the damage; past its
/// health (or hit while thrown, 9) it's beaten (`Item_ZakoDefeat`) and flung away (toward the
/// screen by the enemies' common odds), else knocked back.
#[doc(alias = "it_802E3884")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let (damage, msid) = (ip.xC9C, ip.msid);
    if damage > health(r, gobj) || msid == 9 {
        // `Item_ZakoDefeat`.
        zako::hurt_sound(r, gobj);
        hitbox::start_blink(r.game, gobj);
        hitbox::clear_hit_ids(r.game, gobj);
        zako::ko(r, gobj);
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        let f = reg(r.game.objs.rng.draw_f32());
        let z = r.game.it.zako.expect("no enemies' common data");
        if f < reg(r.data.f32(z.at(8))) {
            fling_to_screen(r, gobj);
        } else {
            fling(r, gobj);
        }
    } else {
        knocked_back(r, gobj);
    }
    false
}

/// Lands, `fn_802E398C`: settled, state 0.
#[doc(alias = "fn_802E398C")]
pub fn land(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Standing's animation, `itWhitebea_UnkMotion0_Anim`: looped.
#[doc(alias = "itWhitebea_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Standing's physics, `itWhitebea_UnkMotion0_Phys`: its countdown done, a draw in two; after
/// a walk (frames left) or on 1 it walks, else it crouches to jump.
#[doc(alias = "itWhitebea_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        let rand = r.game.objs.rng.below(2);
        if get(r, gobj, off::WALK) != 0 || rand != 0 {
            walk(r, gobj);
            return;
        }
        crouch(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
}

/// Standing's and turning's collision (`itWhitebea_UnkMotion0_Coll`, `_2_Coll`): off the floor
/// it falls.
#[doc(alias = "itWhitebea_UnkMotion0_Coll")]
#[doc(alias = "itWhitebea_UnkMotion2_Coll")]
pub fn coll_ground(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Walks (`it_802E3AC8`, and its copies `it_802E40A4` and turning's end): at its speed its way,
/// its model and collision facing it, no blink; a walk's frames drawn between `xC` and `xE`
/// unless it has some left within them (1 and under always redraws); its step's sound in 4,
/// state 1.
#[doc(alias = "it_802E3AC8")]
#[doc(alias = "it_802E40A4")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    let speed = reg(walk_speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), speed));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    let facing = ip.facing_dir;
    zako::face(r, gobj, facing);
    set_coll_facing(r, gobj);
    hitbox::end_blink(r.game, gobj);
    let timer = get(r, gobj, off::WALK);
    let (lo, hi) = (attr_h(r, gobj, 0xC), attr_h(r, gobj, 0xE));
    if timer < 2 || timer > hi {
        let range = hi.wrapping_sub(lo).wrapping_abs();
        let n = lo.wrapping_add(r.game.objs.rng.below(range));
        set(r, gobj, off::WALK, n);
    }
    set(r, gobj, off::STEP, 4);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// Walking's animation, `itWhitebea_UnkMotion1_Anim`: looped keeping its hits; every 15 frames
/// a step's sound (0x13C) and the floor's own step sound, if it has one.
#[doc(alias = "itWhitebea_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    let step = get(r, gobj, off::STEP);
    if step <= 0 {
        let line = item(r.game, gobj).xC30;
        let flags = r
            .game
            .geometry
            .as_ref()
            .expect("no collision geometry")
            .line_surface(line);
        let sfx = crate::mp::terrain_sounds(r.game.stage_info.grkind, flags)
            .sounds(0)
            .0[0];
        set(r, gobj, off::STEP, 14);
        play_sfx(r, gobj, 0x13C, 127, 64);
        if sfx != -1 {
            play_sfx(r, gobj, sfx, 127, 64);
        }
    } else {
        set(r, gobj, off::STEP, step.wrapping_sub(1));
    }
    false
}

/// Walking's physics, `itWhitebea_UnkMotion1_Phys`: its walk done, it stands for a countdown
/// drawn between `x8` and `xA`; at the script's turn point it turns; along the floor.
#[doc(alias = "itWhitebea_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let w = get(r, gobj, off::WALK);
    if w == 0 {
        let (lo, hi) = (attr_h(r, gobj, 8), attr_h(r, gobj, 0xA));
        let range = hi.wrapping_sub(lo).wrapping_abs();
        let n = lo.wrapping_add(r.game.objs.rng.below(range));
        set(r, gobj, off::TIMER, n);
        zako::settle(r, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    } else {
        set(r, gobj, off::WALK, w.wrapping_sub(1));
    }
    if lib2725::xdc0_has(r.game, gobj, 2) {
        turn(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
}

/// Walking's collision, `itWhitebea_UnkMotion1_Coll`: at a ledge or into a wall it turns; off
/// the floor it falls.
#[doc(alias = "itWhitebea_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
    } else {
        fall(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Falls, `it_802E3DA0`: no countdown, its sideways speed slowed by `x4`, turned away from a
/// nudge against its facing, state 3.
#[doc(alias = "it_802E3DA0")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    let k = reg(attr_f32(r, gobj, 4));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    let (facing, nudge) = (reg(ip.facing_dir), reg(ip.x70_nudge.x));
    if facing > 0.0 && nudge < 0.0 || facing < 0.0 && nudge > 0.0 {
        item_mut(r.game, gobj).facing_dir = mem(fp::fneg(facing));
        set_coll_facing(r, gobj);
    }
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// The states' animation callbacks that answer false (`itWhitebea_UnkMotion3_Anim`, `_6_Anim`
/// and `_11_Anim`).
#[doc(alias = "itWhitebea_UnkMotion3_Anim")]
#[doc(alias = "itWhitebea_UnkMotion6_Anim")]
#[doc(alias = "itWhitebea_UnkMotion11_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The falling states' physics (`itWhitebea_UnkMotion3_Phys`, `_6_Phys`, `_9_Phys`): gravity.
#[doc(alias = "itWhitebea_UnkMotion3_Phys")]
#[doc(alias = "itWhitebea_UnkMotion6_Phys")]
// Ports itWhitebea_UnkMotion9_Phys; aliased once a check reaches it.
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itWhitebea_UnkMotion3_Coll`: landing, it stands.
#[doc(alias = "itWhitebea_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, land);
    zako::answers_false(r, gobj)
}

/// Knocked back, `it_802E3ED0`: along the hit's knockback (up at least 0.2, else a fifth of
/// it, in the air) or, if it keeps to the floor, a fifth of its sideways speed on the ground;
/// facing as before the hit, state 4.
#[doc(alias = "it_802E3ED0")]
pub fn knocked_back(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        // A double 0.2 in the comparison, as the C writes it.
        if reg(ip.x40_vel.y) <= 0.2 {
            ip.x40_vel.y = mem(fp::frsp(0.2));
        } else {
            ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), fp::frsp(0.2)));
        }
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
        ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), fp::frsp(0.2)));
    }
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    change_state(r, gobj, 4, UNK_0X1 | change::ANIM_UPDATE);
}

/// Knocked back's animation, `itWhitebea_UnkMotion4_Anim`: when it ends, in the air again, on
/// the ground it walks.
#[doc(alias = "itWhitebea_UnkMotion4_Anim")]
pub fn anim4(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        if item(r.game, gobj).ground_or_air == GA_AIR {
            change_state(r, gobj, 4, change::ANIM_UPDATE);
        } else {
            walk(r, gobj);
        }
    }
    false
}

/// Knocked back's physics, `itWhitebea_UnkMotion4_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itWhitebea_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys11(r, gobj);
    }
}

/// Knocked back's collision, `itWhitebea_UnkMotion4_Coll`: in the air, landing it walks; on the
/// ground, off the floor it falls.
#[doc(alias = "itWhitebea_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        ground::fall_to_floor(r, gobj, walk);
    } else {
        ground::ground_or_fall(r, gobj, fall);
    }
    zako::answers_false(r, gobj)
}

/// Turns, `it_802E4190`: stopped, a 15-frame countdown, state 2.
#[doc(alias = "it_802E4190")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set(r, gobj, off::TIMER, 0xF);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Turning's animation, `itWhitebea_UnkMotion2_Anim`: its facing flips when the countdown
/// reaches 0; when the animation ends, no countdown, and it walks.
#[doc(alias = "itWhitebea_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::TIMER) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r, gobj);
    }
    let t = get(r, gobj, off::TIMER).wrapping_sub(1);
    set(r, gobj, off::TIMER, t);
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::TIMER, 0);
        walk(r, gobj);
    }
    false
}

/// The states' physics that do nothing (`itWhitebea_UnkMotion2_Phys` and its like).
#[doc(alias = "itWhitebea_UnkMotion2_Phys")]
#[doc(alias = "itWhitebea_UnkMotion5_Phys")]
#[doc(alias = "itWhitebea_UnkMotion7_Phys")]
// Ports itWhitebea_UnkMotion8_Phys; aliased once a check reaches it.
#[doc(alias = "itWhitebea_UnkMotion10_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Crouches to jump, `it_802E436C`: on the ground, state 5, a 20-frame countdown.
#[doc(alias = "it_802E436C")]
pub fn crouch(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    set(r, gobj, off::TIMER, 0x14);
}

/// Crouching's animation, `itWhitebea_UnkMotion5_Anim`: at the animation's end or the
/// countdown's it jumps.
#[doc(alias = "itWhitebea_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::TIMER);
    if !lib2725::is_animating(r.game, gobj) || t == 0 {
        jump(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    false
}

/// Crouching's and landing's collision (`itWhitebea_UnkMotion5_Coll`, `_7_Coll`): off the
/// floor it falls (`it_8026D6F4`).
#[doc(alias = "itWhitebea_UnkMotion5_Coll")]
#[doc(alias = "itWhitebea_UnkMotion7_Coll")]
pub fn coll_ground_or(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Jumps, `it_802E4464`: in the air at `x10` up, its collision a falling one, a sound (0x4A),
/// state 6.
#[doc(alias = "it_802E4464")]
pub fn jump(r: &mut Running, gobj: GObjId) {
    let up = attr_f32(r, gobj, 0x10);
    let ip = item_mut(r.game, gobj);
    maplib::enter_air(ip);
    ip.x40_vel.y = up;
    zako::clear_dd0_b4(r.game, gobj);
    play_sfx(r, gobj, 0x4A, 127, 64);
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Jumping's collision, `itWhitebea_UnkMotion6_Coll`: landing, [`landed`].
#[doc(alias = "itWhitebea_UnkMotion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, landed);
    zako::answers_false(r, gobj)
}

/// Lands from its jump, `it_802E4558`: on the ground, two sounds (0x46, 0x13C), state 7, and
/// the adventure's climb's burst (`grIceMt_801FA6D8`).
#[doc(alias = "it_802E4558")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    maplib::enter_ground(item_mut(r.game, gobj));
    play_sfx(r, gobj, 0x46, 127, 64);
    play_sfx(r, gobj, 0x13C, 127, 64);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
    crate::game::gr::icemt::bear_landed(r.game, r.data);
}

/// Landing's animation, `itWhitebea_UnkMotion7_Anim`: when it ends, a walk's frame left, a
/// countdown of `x14`, settled, standing.
#[doc(alias = "itWhitebea_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::WALK, 1);
        let t = attr_h(r, gobj, 0x14);
        set(r, gobj, off::TIMER, t);
        zako::settle(r, gobj);
        change_state(r, gobj, 0, change::ANIM_UPDATE);
    }
    false
}

/// Picked up, `itWhiteBea_Logic9_PickedUp`: state 8.
// Ports itWhiteBea_Logic9_PickedUp; aliased once a check reaches it.
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Held's animation, `itWhitebea_UnkMotion8_Anim`: looped.
// Ports itWhitebea_UnkMotion8_Anim; aliased once a check reaches it.
pub fn anim8(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 8, change::ANIM_UPDATE);
    }
    false
}

/// Dropped, `it_2725_Logic9_Dropped`: the enemies' drop, then it falls.
// Ports it_2725_Logic9_Dropped; aliased once a check reaches it.
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `it_2725_Logic9_Thrown`: the enemies' throw, no countdown, a walk's frame left,
/// state 9.
// Ports it_2725_Logic9_Thrown; aliased once a check reaches it.
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::WALK, 1);
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// Thrown's animation, `itWhitebea_UnkMotion9_Anim`: looped.
// Ports itWhitebea_UnkMotion9_Anim; aliased once a check reaches it.
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 9, change::ANIM_UPDATE);
    }
    false
}

/// Thrown's collision, `itWhitebea_UnkMotion9_Coll`: touching the stage ends the flight as a
/// KO.
// Ports itWhitebea_UnkMotion9_Coll; aliased once a check reaches it.
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Flung away, `it_802E48B4`: in the air, launched along its knockback, `x18` cleared, state
/// 10.
#[doc(alias = "it_802E48B4")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    zako::launch(r, gobj, false);
    set(r, gobj, off::X18, 0);
    change_state(r, gobj, 10, change::ANIM_UPDATE);
}

/// Flung away's animation, `itWhitebea_UnkMotion10_Anim`: looped.
#[doc(alias = "itWhitebea_UnkMotion10_Anim")]
pub fn anim10(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 10, change::ANIM_UPDATE);
    }
    false
}

/// Flung away's collision, `itWhitebea_UnkMotion10_Coll`: bounces off the stage.
#[doc(alias = "itWhitebea_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Flung toward the screen, `it_802E4980` (the inline `Item_UpdateZakoVelocity`): state 11.
#[doc(alias = "it_802E4980")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 11, UNK_0X1 | change::ANIM_UPDATE);
}

/// Flung toward the screen's physics, `itWhitebea_UnkMotion11_Phys` (and the knockback's in the
/// air): its fall speed off the vertical velocity.
#[doc(alias = "itWhitebea_UnkMotion11_Phys")]
pub fn phys11(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Flung toward the screen's collision, `itWhitebea_UnkMotion11_Coll`: nothing.
#[doc(alias = "itWhitebea_UnkMotion11_Coll")]
pub fn coll11(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}
