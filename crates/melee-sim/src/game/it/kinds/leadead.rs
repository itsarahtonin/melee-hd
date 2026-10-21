//! The ReDead, `melee/it/kinds/itleadead.c` (`It_Kind_Leadead`): one of the monsters a container
//! releases (`it_8026F6BC`). It walks toward the nearest fighter, turns when it faces away or
//! meets a wall or ledge, and with a fighter close ahead lunges and grabs it (state 4), holding
//! it in its arms (state 5, the fighter in `ft::co::capture_leadead`) until the fighter mashes
//! free or a hit makes it let go. Hit, it's knocked back or, past its health, flung away as an
//! enemy KO. A fighter can pick it up and throw it.
//!
//! Its variables are the union's bytes ([`zako::bytes`]): the enemies' common words first, then
//! its own at the decomp's offsets ([`off`]), which overlap them (its `xDEC` is the common
//! `x14.y`).

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;
use melee_data::archive::DataRef;

use super::super::item::{article, attr_of, change, change_state, play_sfx};
use super::super::{data, ground, hitbox, item, item_mut, lib26b1, lib2725, maplib, owner, zako};
use crate::fpu::{mem, reg};
use crate::game::camera;
use crate::game::code;
use crate::game::state::Running;
use crate::game::structs::_itECB;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::math;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x802E_8BCC;
pub const DMG_RECEIVED: u32 = 0x802E_8CD8;
pub const DESTROYED: u32 = 0x802E_A41C;
pub const PICKED_UP: u32 = 0x802E_A0E0;
pub const DROPPED: u32 = 0x802E_A154;
pub const THROWN: u32 = 0x802E_A1B4;
pub const LAND: u32 = 0x802E_8F24;
pub const FALL: u32 = 0x802E_9308;
pub const TURN: u32 = 0x802E_9BA0;
pub const ANIM1: u32 = 0x802E_8F7C;
pub const PHYS1: u32 = 0x802E_8F84;
pub const COLL1: u32 = 0x802E_8FFC;
pub const ANIM2: u32 = 0x802E_90E8;
pub const PHYS2: u32 = 0x802E_91AC;
pub const COLL2: u32 = 0x802E_92A0;
pub const ANIM10: u32 = 0x802E_9354;
pub const PHYS10: u32 = 0x802E_935C;
pub const COLL10: u32 = 0x802E_938C;
pub const ANIM11: u32 = 0x802E_959C;
pub const ANIM12: u32 = 0x802E_9624;
pub const PHYS12: u32 = 0x802E_9674;
pub const COLL12: u32 = 0x802E_969C;
pub const ANIM7: u32 = 0x802E_9780;
pub const PHYS7: u32 = 0x802E_9850;
pub const COLL7: u32 = 0x802E_98A4;
pub const ANIM8: u32 = 0x802E_9948;
pub const PHYS8: u32 = 0x802E_9950;
pub const COLL8: u32 = 0x802E_99C4;
pub const ANIM9: u32 = 0x802E_9A38;
pub const PHYS9: u32 = 0x802E_9B60;
pub const COLL9: u32 = 0x802E_9B64;
pub const ANIM3: u32 = 0x802E_9BE4;
pub const PHYS3: u32 = 0x802E_9D10;
pub const COLL3: u32 = 0x802E_9D14;
pub const ANIM4: u32 = 0x802E_9D94;
pub const PHYS4: u32 = 0x802E_9D9C;
pub const COLL4: u32 = 0x802E_9DE8;
pub const ANIM5: u32 = 0x802E_9F28;
pub const PHYS5: u32 = 0x802E_A004;
pub const COLL5: u32 = 0x802E_A008;
pub const ANIM6: u32 = 0x802E_A010;
pub const PHYS6: u32 = 0x802E_A0A0;
pub const COLL6: u32 = 0x802E_A0A4;
pub const ANIM13: u32 = 0x802E_A108;
pub const PHYS13: u32 = 0x802E_A150;
pub const ANIM14: u32 = 0x802E_A204;
pub const PHYS14: u32 = 0x802E_A24C;
pub const COLL14: u32 = 0x802E_A27C;
pub const ANIM15: u32 = 0x802E_A308;
pub const PHYS15: u32 = 0x802E_A310;
pub const COLL15: u32 = 0x802E_A314;
pub const ANIM16: u32 = 0x802E_A394;
pub const PHYS16: u32 = 0x802E_A39C;
pub const COLL16: u32 = 0x802E_A3B8;
pub const ANIM17: u32 = 0x802E_A3D8;
pub const PHYS17: u32 = 0x802E_A3E0;
pub const COLL17: u32 = 0x802E_A3FC;

/// Its variables' offsets in the union (`itLeadead_ItemVars`).
pub mod off {
    /// `xDEC`, the common `x14.y`, cleared as it's flung away.
    pub const XDEC: usize = 0x18;
    /// `x38`: the fighter it holds.
    pub const VICTIM: usize = 0x38;
    /// `x3C`: how far in front of it the held fighter's part sits (its joint's z).
    pub const HOLD_Z: usize = 0x3C;
    /// `xE14`: the held fighter's part its arms follow.
    pub const PART: usize = 0x40;
    /// `x44`: a state's frame count.
    pub const FRAME: usize = 0x44;
    /// `x48`: a state's timer.
    pub const TIMER: usize = 0x48;
    /// `x4C`: the frames before it may lunge again.
    pub const LUNGE_WAIT: usize = 0x4C;
    /// `x50`: knockbacks shrugged off in a row.
    pub const SHRUGS: usize = 0x50;
    /// `xE30`: its first hurtbox's capsule (its offsets and scale, from `+0x60`).
    pub const HURT: usize = 0x5C;
    /// `xE50`: its ECB as it spawned.
    pub const ECB: usize = 0x7C;
}

/// Its special attributes (`itLeadeadAttributes`), `off` bytes in.
fn attr(r: &Running, gobj: GObjId, off: u32) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a ReDead without attributes")
        .at(off)
}

fn attr_f32(r: &Running, gobj: GObjId, off: u32) -> f32 {
    r.data.f32(attr(r, gobj, off))
}

fn attr_i32(r: &Running, gobj: GObjId, off: u32) -> i32 {
    r.data.u32(attr(r, gobj, off)) as i32
}

fn attr_s16(r: &Running, gobj: GObjId, off: u32) -> i32 {
    i32::from(r.data.u16(attr(r, gobj, off)) as i16)
}

/// `attr->x0`: its health (`x0`) and walking speed (`x4`).
fn health(r: &Running, gobj: GObjId) -> i32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a ReDead's health");
    r.data.u32(p) as i32
}

fn walk_speed(r: &Running, gobj: GObjId) -> f32 {
    let p = r.data.ptr(attr(r, gobj, 0)).expect("a ReDead's speed");
    r.data.f32(p.at(4))
}

fn get(r: &mut Running, gobj: GObjId, off: usize) -> i32 {
    zako::bytes(r.game, gobj).i32(off)
}

fn set(r: &mut Running, gobj: GObjId, off: usize, v: i32) {
    zako::bytes(r.game, gobj).set_i32(off, v);
}

fn victim(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    zako::bytes(r.game, gobj).gobj(off::VICTIM)
}

fn ecb(r: &mut Running, gobj: GObjId) -> _itECB {
    let v = zako::bytes(r.game, gobj);
    _itECB {
        top: v.f32(off::ECB),
        bottom: v.f32(off::ECB + 4),
        right: v.f32(off::ECB + 8),
        left: v.f32(off::ECB + 0xC),
    }
}

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

/// Stops (the inline `itResetVelocity`): z, then y, then x.
fn stop(r: &mut Running, gobj: GObjId) {
    let v = &mut item_mut(r.game, gobj).x40_vel;
    v.z = 0.0;
    v.y = 0.0;
    v.x = 0.0;
}

/// Sets the bone the held fighter's part follows to `z` in front of it (the inline
/// `HSD_JObjSetTranslateZ` on bone 1).
fn set_hold_z(r: &mut Running, gobj: GObjId, z: f32) {
    let j = lib2725::bone(r.game, r.data, gobj, 1).expect("a ReDead without its arms' bone");
    jobj::set_srt(&mut r.game.objs.jobjs, j, |o| o.translate.z = z);
}

/// Runs one of the ReDead's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        DMG_RECEIVED => damaged(r, gobj),
        ANIM1 | ANIM8 | ANIM10 | ANIM4 | ANIM15 | ANIM16 | ANIM17 => answers_false(r, gobj),
        COLL5 => answers_false(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        COLL10 => coll10(r, gobj),
        ANIM11 => anim11(r, gobj),
        ANIM12 => anim12(r, gobj),
        COLL12 => coll12(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL7 | COLL8 | COLL9 => coll_shrug(r, gobj),
        ANIM9 => anim9(r, gobj),
        ANIM3 => anim3(r, gobj),
        COLL3 | COLL6 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        ANIM5 => anim5(r, gobj),
        ANIM6 => anim6(r, gobj),
        ANIM13 => anim13(r, gobj),
        ANIM14 => anim14(r, gobj),
        COLL14 => coll14(r, gobj),
        COLL15 => coll15(r, gobj),
        COLL16 => coll16(r, gobj),
        COLL17 => coll17(r, gobj),
        _ => return None,
    })
}

/// Runs one of the ReDead's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        LAND => land(r, gobj),
        FALL => fall(r, gobj),
        TURN => turn(r, gobj),
        PHYS1 => phys1(r, gobj),
        PHYS2 => phys2(r, gobj),
        PHYS10 | PHYS14 => phys_fall(r, gobj),
        PHYS12 => phys12(r, gobj),
        PHYS7 => phys7(r, gobj),
        PHYS8 => phys8(r, gobj),
        PHYS9 | PHYS3 | PHYS5 | PHYS6 | PHYS13 | PHYS15 => still(r, gobj),
        PHYS4 => phys4(r, gobj),
        PHYS16 | PHYS17 => phys_gravity(r, gobj),
        _ => return false,
    }
    true
}

/// The ReDead's setup, `it_802E8BCC`: the enemies' common variables, facing most fighters,
/// upright, its collision an enemy's, its lunge's wait, its first hurtbox's capsule and its ECB
/// kept, holding nobody, and falling.
#[doc(alias = "it_802E8BCC")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    zako::reset(r.game, gobj);
    let pos = item(r.game, gobj).pos;
    let facing = owner::facing_for(r.game, r.data, pos);
    item_mut(r.game, gobj).facing_dir = facing;
    zako::face(r, gobj, facing);
    set_coll_facing(r, gobj);
    // `Item_InitZakoCollision`.
    let ip = item_mut(r.game, gobj);
    ip.xD5C = 0;
    ip.xDC8_word.flags.x15 = false;
    hitbox::clear_dcd_b7(r.game, gobj);
    zako::set_dd0_b4(r.game, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x19 = true;
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::SHRUGS, 0);
    let wait = attr_s16(r, gobj, 0x1C);
    set(r, gobj, off::LUNGE_WAIT, wait);
    let (a, b, scale) = super::super::coll::hurt_capsule(r.game, r.data, gobj, 0);
    let v = zako::bytes(r.game, gobj);
    v.set_vec(off::HURT + 4, a);
    v.set_vec(off::HURT + 0x10, b);
    v.set_f32(off::HURT + 0x1C, scale);
    let e = item(r.game, gobj).xC1C.clone();
    let v = zako::bytes(r.game, gobj);
    v.set_f32(off::ECB, e.top);
    v.set_f32(off::ECB + 4, e.bottom);
    v.set_f32(off::ECB + 8, e.right);
    v.set_f32(off::ECB + 0xC, e.left);
    v.set_gobj(off::VICTIM, None);
    fall(r, gobj);
}

/// Takes a hit, `it_802E8CD8`: whoever it held let go; then, unless already grabbing (7) or
/// knocked back in the air (12), the hit's element adds to the damage, and past its health (or
/// when hit while flung, 14) it's an enemy KO and flung away, toward the screen at random;
/// knocked down (8) it's knocked back again; past part of its health it turns to dodge if the
/// floor ahead is level, else, after two shrugs, may be flung, else shrugs the knockback off
/// faster; otherwise it's knocked back.
///
/// ssbm-rs transliterates it from its machine code because it calls [`ground_ahead`], which
/// reads a register its caller left ([`ground_ahead`]'s note).
#[doc(alias = "it_802E8CD8")]
pub fn damaged(r: &mut Running, gobj: GObjId) -> bool {
    if let Some(f) = victim(r, gobj) {
        let_go(r, gobj);
        r.capture_leadead_cut(f);
    }
    set(r, gobj, off::TIMER, 0);
    let wait = attr_s16(r, gobj, 0x1C);
    set(r, gobj, off::LUNGE_WAIT, wait);
    let ip = item_mut(r.game, gobj);
    ip.init_facing_dir = ip.facing_dir;
    let msid = ip.msid;
    if msid == 7 || msid == 0xC {
        return false;
    }
    let extra = reg(zako::element_damage(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.xC9C = fp::fctiwz(fp::fadds(fp::frsp(f64::from(ip.xC9C)), extra));
    let damage = ip.xC9C;
    let hp = health(r, gobj);
    if damage > hp || msid == 0xE {
        zako::hurt_sound(r, gobj);
        hitbox::start_blink(r.game, gobj);
        zako::ko(r, gobj);
        let pos = item(r.game, gobj).pos;
        camera::request_quake(r, QUAKE_SMALL, Some(pos));
        let f = reg(r.game.objs.rng.draw_f32());
        if f < reg(zako_f32(r, 8)) {
            fling_to_screen(r, gobj);
        } else {
            fling(r, gobj);
        }
    } else if msid == 8 {
        knocked_down_again(r, gobj);
    } else if fp::frsp(f64::from(damage))
        > fp::fmuls(fp::frsp(f64::from(hp)), reg(attr_f32(r, gobj, 8)))
    {
        if ground_ahead(r, gobj, 15.0) {
            dodge(r, gobj);
        } else if get(r, gobj, off::SHRUGS) > 1 && r.game.objs.rng.below(2) == 0 {
            let f = reg(r.game.objs.rng.draw_f32());
            if f < reg(zako_f32(r, 8)) {
                fling_to_screen(r, gobj);
            } else {
                fling(r, gobj);
            }
        } else {
            let ip = item_mut(r.game, gobj);
            ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), reg(1.5)));
            let n = get(r, gobj, off::SHRUGS).wrapping_add(1);
            set(r, gobj, off::SHRUGS, n);
            knocked_back(r, gobj);
        }
    } else {
        knocked_back(r, gobj);
    }
    false
}

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// `ITEM_UNK_0x1`, a state change's first flag.
const UNK_0X1: u32 = 1;

/// The enemies' common data (`it_804D6D40`), the float `off` bytes in.
fn zako_f32(r: &Running, off: u32) -> f32 {
    let z = r.game.it.zako.expect("no enemies' common data");
    r.data.f32(z.at(off))
}

/// Lands, `it_802E8F24`: settled, and standing (state 1).
#[doc(alias = "it_802E8F24")]
pub fn land(r: &mut Running, gobj: GObjId) {
    zako::settle(r, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x1A = true;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// The states' animation callbacks that answer false (`itLeadead_UnkMotion1_Anim` and its
/// like), and its being held's collision.
#[doc(alias = "itLeadead_UnkMotion1_Anim")]
#[doc(alias = "itLeadead_UnkMotion4_Anim")]
#[doc(alias = "itLeadead_UnkMotion5_Coll")]
#[doc(alias = "itLeadead_UnkMotion8_Anim")]
#[doc(alias = "itLeadead_UnkMotion10_Anim")]
#[doc(alias = "itLeadead_UnkMotion15_Anim")]
#[doc(alias = "itLeadead_UnkMotion16_Anim")]
#[doc(alias = "itLeadead_UnkMotion17_Anim")]
pub fn answers_false(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states' physics that do nothing (`itLeadead_UnkMotion3_Phys` and its like).
#[doc(alias = "itLeadead_UnkMotion3_Phys")]
#[doc(alias = "itLeadead_UnkMotion5_Phys")]
#[doc(alias = "itLeadead_UnkMotion6_Phys")]
#[doc(alias = "itLeadead_UnkMotion9_Phys")]
#[doc(alias = "itLeadead_UnkMotion13_Phys")]
#[doc(alias = "itLeadead_UnkMotion15_Phys")]
pub fn still(_: &mut Running, _: GObjId) {}

/// Standing's physics, `itLeadead_UnkMotion1_Phys`: its timer done, it faces its way and walks;
/// until then it turns its model to the floor.
#[doc(alias = "itLeadead_UnkMotion1_Phys")]
pub fn phys1(r: &mut Running, gobj: GObjId) {
    let facing = item(r.game, gobj).facing_dir;
    if get(r, gobj, off::TIMER) == 0 {
        zako::face(r, gobj, facing);
        walk(r, gobj);
        return;
    }
    zako::align_on_floor(r, gobj, None, reg(facing), 2.0);
    let t = get(r, gobj, off::TIMER).wrapping_sub(1);
    set(r, gobj, off::TIMER, t);
}

/// Standing's collision, `itLeadead_UnkMotion1_Coll`: off the floor, it falls.
#[doc(alias = "itLeadead_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Walks, `it_802E9038` (and the inlines that enter walking again): its speed its way, facing
/// it, no blink, its frame count reset, its timer to the turning check's period if it has
/// none, its walking box, state 2.
#[doc(alias = "it_802E9038")]
pub fn walk(r: &mut Running, gobj: GObjId) {
    let speed = reg(walk_speed(r, gobj));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.facing_dir), speed));
    ip.x40_vel.z = 0.0;
    ip.x40_vel.y = 0.0;
    let facing = ip.facing_dir;
    zako::face(r, gobj, facing);
    hitbox::end_blink(r.game, gobj);
    set(r, gobj, off::FRAME, 0);
    if get(r, gobj, off::TIMER) == 0 {
        let t = attr_s16(r, gobj, 0x18);
        set(r, gobj, off::TIMER, t);
    }
    set_box(r, gobj, 0, 0);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Walking's animation, `itLeadead_UnkMotion2_Anim`: walks again when the animation ends.
#[doc(alias = "itLeadead_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        walk(r, gobj);
    }
    false
}

/// Walking's physics, `itLeadead_UnkMotion2_Phys`: each period, turns if the nearest fighter
/// is behind it; between, lunges if one is in reach ahead; turns when pushed back; walks along
/// the floor.
#[doc(alias = "itLeadead_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    let behind = faces_away(r, gobj);
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if behind {
            turn(r, gobj);
        } else {
            let p = attr_s16(r, gobj, 0x18);
            set(r, gobj, off::TIMER, p);
        }
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
        if !behind && in_reach(r, gobj) {
            lunge(r, gobj);
        }
    }
    let n = get(r, gobj, off::FRAME).wrapping_add(1);
    set(r, gobj, off::FRAME, n);
    let ip = item(r.game, gobj);
    let facing = reg(ip.facing_dir);
    let nudge = reg(ip.x70_nudge.x);
    if (facing > 0.0 && nudge < 0.0) || (facing < 0.0 && nudge > 0.0) {
        turn(r, gobj);
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
}

/// Walking's collision, `itLeadead_UnkMotion2_Coll`: at a ledge or a wall it turns; off the
/// floor it falls.
#[doc(alias = "itLeadead_UnkMotion2_Coll")]
pub fn coll2(r: &mut Running, gobj: GObjId) -> bool {
    if ground::ground_to_ledge(r, gobj, turn) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        }
    } else {
        fall(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Falls, `it_802E9308`: its timer cleared, slowed by `xC`, state 10.
#[doc(alias = "it_802E9308")]
pub fn fall(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    let k = reg(attr_f32(r, gobj, 0xC));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), k));
    change_state(r, gobj, 10, change::ANIM_UPDATE);
}

/// The falling states' physics (`itLeadead_UnkMotion10_Phys`, and being thrown's): gravity.
#[doc(alias = "itLeadead_UnkMotion10_Phys")]
#[doc(alias = "itLeadead_UnkMotion14_Phys")]
pub fn phys_fall(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, data::attr::FALL_SPEED);
    let max = a.f32(r.data, data::attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// Falling's collision, `itLeadead_UnkMotion10_Coll`: it lands on a floor.
#[doc(alias = "itLeadead_UnkMotion10_Coll")]
pub fn coll10(r: &mut Running, gobj: GObjId) -> bool {
    ground::fall_to_floor(r, gobj, land);
    zako::answers_false(r, gobj)
}

/// Knocked back, `it_802E93C8`: along the hit's knockback (up at least a little if off the
/// floor, then in the air), facing as before the hit, for `x4` frames, its lunge's wait reset,
/// state 11.
#[doc(alias = "it_802E93C8")]
pub fn knocked_back(r: &mut Running, gobj: GObjId) {
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        if reg(ip.x40_vel.y) <= reg(0.2) {
            ip.x40_vel.y = 0.2;
        }
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
    }
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    let frames = attr_i32(r, gobj, 4);
    set(r, gobj, off::TIMER, frames);
    let wait = attr_s16(r, gobj, 0x1C);
    set(r, gobj, off::LUNGE_WAIT, wait);
    change_state(r, gobj, 11, UNK_0X1);
}

/// Knocked down again while down, `it_802E9494`: the knockback's height cut to 0.3 (no lower
/// than its fall speed's step), no sideways, frame count 1, state 12.
#[doc(alias = "it_802E9494")]
pub fn knocked_down_again(r: &mut Running, gobj: GObjId) {
    let mut v = item(r.game, gobj).x40_vel;
    let up = zako::knockback_vel(r, gobj, &mut v);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = v;
    if up {
        if reg(ip.x40_vel.y) <= reg(0.2) {
            ip.x40_vel.y = 0.2;
        }
        maplib::enter_air(ip);
    } else {
        maplib::enter_ground(ip);
    }
    let fall = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.x = 0.0;
    ip.x40_vel.y = mem(fp::fmuls(reg(0.3), reg(ip.x40_vel.y)));
    if reg(ip.x40_vel.y) <= fall {
        ip.x40_vel.y = mem(fp::fadds(reg(0.1), fall));
    }
    set(r, gobj, off::FRAME, 1);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set_coll_facing(r, gobj);
    let frames = attr_i32(r, gobj, 4);
    set(r, gobj, off::TIMER, frames);
    let wait = attr_s16(r, gobj, 0x1C);
    set(r, gobj, off::LUNGE_WAIT, wait);
    change_state(r, gobj, 12, UNK_0X1);
}

/// Knocked back's animation, `itLeadead_UnkMotion11_Anim`: its timer done on the ground, it
/// stands (the inline [`land`]).
#[doc(alias = "itLeadead_UnkMotion11_Anim")]
pub fn anim11(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if item(r.game, gobj).ground_or_air == GA_GROUND {
            land(r, gobj);
        }
    } else if t > 0 {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    false
}

/// `GA_Ground` and `GA_Air`.
const GA_GROUND: i32 = 0;
const GA_AIR: i32 = 1;

/// Knocked down's animation, `itLeadead_UnkMotion12_Anim`: its timer done on the ground, it
/// lies down again.
#[doc(alias = "itLeadead_UnkMotion12_Anim")]
pub fn anim12(r: &mut Running, gobj: GObjId) -> bool {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        if item(r.game, gobj).ground_or_air == GA_GROUND {
            lie_down(r, gobj);
        }
    } else if t > 0 {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    false
}

/// The knockbacks' physics, `itLeadead_UnkMotion12_Phys`: in the air its fall speed off the
/// vertical velocity.
#[doc(alias = "itLeadead_UnkMotion12_Phys")]
pub fn phys12(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        phys_gravity(r, gobj);
    }
}

/// The knockbacks' collision, `itLeadead_UnkMotion12_Coll`: in the air it bounces off walls
/// and ceilings and stops on a floor; on the ground it stays or takes off.
#[doc(alias = "itLeadead_UnkMotion12_Coll")]
pub fn coll12(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ground_or_air == GA_AIR {
        let coll = ground::air_touches_walls(r, gobj);
        if coll & 0xC != 0 {
            let _ = maplib::reflect_velocity(r.game, r.data, gobj);
        }
        if coll & 1 != 0 {
            maplib::enter_ground(item_mut(r.game, gobj));
            stop(r, gobj);
        }
    } else {
        ground::ground_or_air(r, gobj);
    }
    zako::answers_false(r, gobj)
}

/// Dodges a hit, `it_802E9738`: stopped, its counts cleared, state 7.
#[doc(alias = "it_802E9738")]
pub fn dodge(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set(r, gobj, off::FRAME, 0);
    set(r, gobj, off::SHRUGS, 0);
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// Dodging's animation, `itLeadead_UnkMotion7_Anim`: its box through the dodge's four shapes,
/// then getting up.
#[doc(alias = "itLeadead_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    let n = get(r, gobj, off::FRAME);
    if n < 27 {
        set_box(r, gobj, 1, 0);
    } else if n < 42 {
        set_box(r, gobj, 1, 1);
    } else if n < 66 {
        set_box(r, gobj, 1, 2);
    } else if n < 81 {
        set_box(r, gobj, 1, 3);
    } else {
        set_box(r, gobj, 2, 0);
        lie_down(r, gobj);
    }
    let n = get(r, gobj, off::FRAME).wrapping_add(1);
    set(r, gobj, off::FRAME, n);
    false
}

/// Dodging's physics, `itLeadead_UnkMotion7_Phys`: along the floor, its model turned to it.
#[doc(alias = "itLeadead_UnkMotion7_Phys")]
pub fn phys7(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, facing, 5.0);
}

/// The dodge's and lying's collision (`itLeadead_UnkMotion7_Coll`, `_8_Coll`, `_9_Coll`): off
/// the floor it falls.
#[doc(alias = "itLeadead_UnkMotion7_Coll")]
#[doc(alias = "itLeadead_UnkMotion8_Coll")]
#[doc(alias = "itLeadead_UnkMotion9_Coll")]
pub fn coll_shrug(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Lies down after the dodge, `it_802E98E0`: stopped, blinking off, for its damage times
/// `x1A` frames, state 8.
#[doc(alias = "it_802E98E0")]
pub fn lie_down(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    item_mut(r.game, gobj).xDC8_word.flags.x1A = false;
    let k = attr_s16(r, gobj, 0x1A);
    let frames = item(r.game, gobj).xC9C.wrapping_mul(k);
    set(r, gobj, off::TIMER, frames);
    set(r, gobj, off::FRAME, 0);
    change_state(r, gobj, 8, change::ANIM_UPDATE);
}

/// Lying's physics, `itLeadead_UnkMotion8_Phys`: its timer done, it gets up; along the floor.
#[doc(alias = "itLeadead_UnkMotion8_Phys")]
pub fn phys8(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t == 0 {
        get_up(r, gobj);
    } else {
        set(r, gobj, off::TIMER, t.wrapping_sub(1));
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::align_floor(r, gobj, facing, 5.0);
}

/// Gets up, `it_802E9A00`: counts cleared, state 9.
#[doc(alias = "it_802E9A00")]
pub fn get_up(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    set(r, gobj, off::FRAME, 0);
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// Getting up's animation, `itLeadead_UnkMotion9_Anim`: its box through the getting up's
/// shapes, then standing for 10 frames.
#[doc(alias = "itLeadead_UnkMotion9_Anim")]
pub fn anim9(r: &mut Running, gobj: GObjId) -> bool {
    let n = get(r, gobj, off::FRAME);
    if n < 46 {
        set_box(r, gobj, 2, 0);
    } else if n < 87 {
        set_box(r, gobj, 3, 1);
    } else if n < 130 {
        set_box(r, gobj, 3, 2);
    } else if n < 170 {
        set_box(r, gobj, 3, 3);
    } else if n < 0x105 {
        set_box(r, gobj, 3, 4);
    } else {
        set_box(r, gobj, 0, 0);
    }
    let n = get(r, gobj, off::FRAME).wrapping_add(1);
    set(r, gobj, off::FRAME, n);
    if !lib2725::is_animating(r.game, gobj) {
        set(r, gobj, off::TIMER, 10);
        land(r, gobj);
    }
    false
}

/// Turns, `it_802E9BA0`: stopped for 140 frames, state 3.
#[doc(alias = "it_802E9BA0")]
pub fn turn(r: &mut Running, gobj: GObjId) {
    stop(r, gobj);
    set(r, gobj, off::TIMER, 140);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// Turning's animation, `itLeadead_UnkMotion3_Anim`: at its timer's end it faces the other
/// way; when the animation ends it walks, its timer three of the turning check's periods.
#[doc(alias = "itLeadead_UnkMotion3_Anim")]
pub fn anim3(r: &mut Running, gobj: GObjId) -> bool {
    if get(r, gobj, off::TIMER) == 0 {
        let ip = item_mut(r.game, gobj);
        ip.facing_dir = mem(fp::fneg(reg(ip.facing_dir)));
        set_coll_facing(r, gobj);
    }
    let t = get(r, gobj, off::TIMER).wrapping_sub(1);
    set(r, gobj, off::TIMER, t);
    if !lib2725::is_animating(r.game, gobj) {
        let p = attr_s16(r, gobj, 0x18).wrapping_mul(3);
        set(r, gobj, off::TIMER, p);
        walk(r, gobj);
    }
    false
}

/// Turning's and letting go's collision (`itLeadead_UnkMotion3_Coll`, `_6_Coll`): off the
/// floor it falls.
#[doc(alias = "itLeadead_UnkMotion3_Coll")]
#[doc(alias = "itLeadead_UnkMotion6_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    zako::answers_false(r, gobj)
}

/// Lunges, `it_802E9D50`: its timer cleared, stopped, state 4.
#[doc(alias = "it_802E9D50")]
pub fn lunge(r: &mut Running, gobj: GObjId) {
    set(r, gobj, off::TIMER, 0);
    stop(r, gobj);
    change_state(r, gobj, 4, change::ANIM_UPDATE);
}

/// Lunging's physics, `itLeadead_UnkMotion4_Phys`: from frame 37 it moves at `x14` its way,
/// and from 114 it stops.
#[doc(alias = "itLeadead_UnkMotion4_Phys")]
pub fn phys4(r: &mut Running, gobj: GObjId) {
    let t = get(r, gobj, off::TIMER);
    if t > 36 {
        let s = reg(attr_f32(r, gobj, 0x14));
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.x = mem(fp::fmuls(s, reg(ip.facing_dir)));
    }
    if get(r, gobj, off::TIMER) > 113 {
        item_mut(r.game, gobj).x40_vel.x = 0.0;
    }
    let t = get(r, gobj, off::TIMER).wrapping_add(1);
    set(r, gobj, off::TIMER, t);
}

/// Lunging's collision, `itLeadead_UnkMotion4_Coll`: off the floor it falls; when the lunge
/// ends it turns at a wall, else walks on, its timer 1; along the floor.
#[doc(alias = "itLeadead_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground::ground_or_fall(r, gobj, fall);
    if !lib2725::is_animating(r.game, gobj) {
        if maplib::touched_wall(r.game, gobj) != 0 {
            turn(r, gobj);
        } else {
            set(r, gobj, off::TIMER, 1);
            walk(r, gobj);
        }
    }
    let ip = item_mut(r.game, gobj);
    let (normal, facing) = (ip.x378_itemColl.floor.normal, ip.facing_dir);
    zako::along_surface(&mut ip.x40_vel, normal, facing);
    zako::answers_false(r, gobj)
}

/// Holding's animation, `itLeadead_UnkMotion5_Anim`: the hold's animation again when it ends,
/// keeping its hits, the held part's bone in front of it.
#[doc(alias = "itLeadead_UnkMotion5_Anim")]
pub fn anim5(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 5, change::ANIM_UPDATE | change::HIT_PRESERVE);
        let z = zako::bytes(r.game, gobj).f32(off::HOLD_Z);
        set_hold_z(r, gobj, z);
    }
    false
}

/// Letting go's animation, `itLeadead_UnkMotion6_Anim`: when it ends, the lunge's wait reset
/// and standing.
#[doc(alias = "itLeadead_UnkMotion6_Anim")]
pub fn anim6(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        let wait = attr_s16(r, gobj, 0x1C);
        set(r, gobj, off::LUNGE_WAIT, wait);
        set(r, gobj, off::TIMER, 0);
        land(r, gobj);
    }
    false
}

/// Picked up, `itLeadead_Logic1_PickedUp`: state 13.
#[doc(alias = "itLeadead_Logic1_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 13, change::ANIM_UPDATE);
}

/// Held's animation, `itLeadead_UnkMotion13_Anim`: looped.
#[doc(alias = "itLeadead_UnkMotion13_Anim")]
pub fn anim13(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 13, change::ANIM_UPDATE);
    }
    false
}

/// Dropped, `itLeadead_Logic1_Dropped`: the enemies' drop, then falling.
#[doc(alias = "itLeadead_Logic1_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    zako::dropped(r, gobj);
    fall(r, gobj);
}

/// Thrown, `itLeadead_Logic1_Thrown`: the enemies' throw, its timer cleared, state 14.
#[doc(alias = "itLeadead_Logic1_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    zako::thrown(r, gobj);
    set(r, gobj, off::TIMER, 0);
    change_state(r, gobj, 14, change::ANIM_UPDATE);
}

/// Thrown's animation, `itLeadead_UnkMotion14_Anim`: looped, keeping its hits.
#[doc(alias = "itLeadead_UnkMotion14_Anim")]
pub fn anim14(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 14, change::ANIM_UPDATE | change::HIT_PRESERVE);
    }
    false
}

/// Thrown's collision, `itLeadead_UnkMotion14_Coll`: touching the stage ends the flight as a
/// KO.
#[doc(alias = "itLeadead_UnkMotion14_Coll")]
pub fn coll14(r: &mut Running, gobj: GObjId) -> bool {
    zako::flight_lands(r, gobj, None)
}

/// Flung away, `it_802EA2A0`: in the air, launched along its knockback, facing as before the
/// hit, `xDEC` cleared, state 15 (keeping its sounds).
#[doc(alias = "it_802EA2A0")]
pub fn fling(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    zako::launch(r, gobj, false);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = ip.init_facing_dir;
    set(r, gobj, off::XDEC, 0);
    change_state(r, gobj, 15, UNK_0X1 | change::ANIM_UPDATE);
}

/// Flung away's collision, `itLeadead_UnkMotion15_Coll`: it bounces.
#[doc(alias = "itLeadead_UnkMotion15_Coll")]
pub fn coll15(r: &mut Running, gobj: GObjId) -> bool {
    zako::bounce(r, gobj)
}

/// Flung toward the screen, `it_802EA334` (the inline `Item_UpdateZakoVelocity`): state 16.
#[doc(alias = "it_802EA334")]
pub fn fling_to_screen(r: &mut Running, gobj: GObjId) {
    maplib::enter_air(item_mut(r.game, gobj));
    let v = zako::toward_camera(r, gobj);
    item_mut(r.game, gobj).x40_vel = v;
    maplib::enter_air(item_mut(r.game, gobj));
    change_state(r, gobj, 16, UNK_0X1 | change::ANIM_UPDATE);
}

/// The flights' physics (`itLeadead_UnkMotion16_Phys`, `_17_Phys`, and the knockbacks' in the
/// air): its fall speed off the vertical velocity.
#[doc(alias = "itLeadead_UnkMotion16_Phys")]
#[doc(alias = "itLeadead_UnkMotion17_Phys")]
pub fn phys_gravity(r: &mut Running, gobj: GObjId) {
    let accel = reg(attr_of(item(r.game, gobj)).f32(r.data, data::attr::FALL_SPEED));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel.y = mem(fp::fsubs(reg(ip.x40_vel.y), accel));
}

/// Flung toward the screen's collision, `itLeadead_UnkMotion16_Coll`: nothing.
#[doc(alias = "itLeadead_UnkMotion16_Coll")]
pub fn coll16(r: &mut Running, gobj: GObjId) -> bool {
    zako::answers_false(r, gobj)
}

/// State 17's collision, `itLeadead_UnkMotion17_Coll`: whether it touched the stage.
#[doc(alias = "itLeadead_UnkMotion17_Coll")]
pub fn coll17(r: &mut Running, gobj: GObjId) -> bool {
    ground::air_touched_any(r, gobj)
}

/// Its destroy callback, `itLeadead_Logic1_Destroyed`: whoever it held let go, then the
/// enemies' destroy.
#[doc(alias = "itLeadead_Logic1_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(f) = victim(r, gobj) {
        let_go(r, gobj);
        r.capture_leadead_cut(f);
    }
    zako::destroyed(r, gobj);
}

/// Sets its box for a state's shape, `it_802EA478`: shape `kind` (1 the dodge, 2 lying, 3
/// getting up, else walking) at step `step`, from its spawn's ECB (step 0) or its current one.
#[doc(alias = "it_802EA478")]
pub fn set_box(r: &mut Running, gobj: GObjId, kind: i32, step: i32) {
    let mut e = if step == 0 {
        ecb(r, gobj)
    } else {
        item(r.game, gobj).xC1C.clone()
    };
    let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
    let sub = |a: f32, b: f32| mem(fp::fsubs(reg(a), reg(b)));
    match kind {
        1 => match step {
            1 => e.top = sub(e.top, 0.3),
            2 => {
                e.top = sub(e.top, 0.1);
                e.right = add(e.right, 0.1);
            }
            3 => {
                e.top = sub(e.top, 0.4);
                e.right = add(e.right, 0.5);
            }
            _ => {}
        },
        2 => {
            e.right = mem(fp::fmuls(reg(0.9), reg(e.top)));
            e.top = 3.0;
            e.left = mem(fp::fmuls(reg(e.left), 2.0));
        }
        3 => match step {
            1 => {
                e.top = add(e.top, 0.1);
                e.right = sub(e.right, 0.05);
                e.left = 0.0;
            }
            2 => e.top = add(e.top, 0.1),
            3 => {
                e.top = add(e.top, 0.2);
                e.right = sub(e.right, 0.1);
            }
            4 => {
                e.left = zako::bytes(r.game, gobj).f32(off::ECB + 0xC);
                e.right = sub(e.right, 0.1);
            }
            _ => {}
        },
        _ => {}
    }
    maplib::set_box(r.game, gobj, e);
    set_coll_facing(r, gobj);
}

/// Whether the nearest fighter is behind it, `it_802EA674`.
#[doc(alias = "it_802EA674")]
pub fn faces_away(r: &mut Running, gobj: GObjId) -> bool {
    let pos = item(r.game, gobj).pos;
    let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) else {
        return false;
    };
    let v = r.game.ft_pos(f);
    let facing = if reg(pos.x) < reg(v.x) {
        1.0
    } else {
        fp::fneg(1.0)
    };
    facing != reg(item(r.game, gobj).facing_dir)
}

/// Whether a fighter is in reach to lunge at, `it_802EA6F4`: once its lunge's wait is done,
/// one draw in `x1E` picks the nearest fighter, if it isn't held by an enemy already and is
/// within `x10` sideways and 5 up or down. The wait counts down otherwise.
#[doc(alias = "it_802EA6F4")]
pub fn in_reach(r: &mut Running, gobj: GObjId) -> bool {
    let wait = get(r, gobj, off::LUNGE_WAIT);
    if wait != 0 {
        set(r, gobj, off::LUNGE_WAIT, wait.wrapping_sub(1));
        return false;
    }
    let odds = i32::from(r.data.u8(attr(r, gobj, 0x1E)) as i8);
    if r.game.objs.rng.below(odds) != 0 {
        return false;
    }
    let pos = item(r.game, gobj).pos;
    let Some(f) = owner::nearest_opponent(r.game, r.data, pos, None) else {
        return false;
    };
    if zako::held_by_enemy(r.game, f) {
        return false;
    }
    let v = r.game.ft_pos(f);
    let mut dx = fp::fsubs(reg(v.x), reg(pos.x));
    if dx < 0.0 {
        dx = fp::fneg(dx);
    }
    if dx <= reg(attr_f32(r, gobj, 0x10)) {
        let mut dy = fp::fsubs(reg(v.y), reg(pos.y));
        if dy < 0.0 {
            dy = fp::fneg(dy);
        }
        if dy <= 5.0 {
            return true;
        }
    }
    false
}

/// Whether the floor `range` ahead is close to level with where it stands, `it_802EA804`: the
/// floor's angle under it and at the end of a walk `range` its way along the floor differ by at
/// most 30°, and the two floors' lines by at most 3; or, when the walk doesn't get there, what
/// its collision touches there ([`touches_ahead`]).
///
/// `mpLib_80056C54` keeps the walk's run of walls in a register it never sets before the first
/// line, so a walk starting on a wall adds to what the damage callback's caller left there
/// (melee-bugs.md). The ReDead stands on a floor, whose walk resets it; one starting on any
/// other line stops here.
#[doc(alias = "it_802EA804")]
pub fn ground_ahead(r: &mut Running, gobj: GObjId, range: f32) -> bool {
    let ip = item(r.game, gobj);
    let (line, pos, facing) = (ip.x378_itemColl.floor.index, ip.pos, ip.facing_dir);
    let g = r.game.geometry.as_ref().expect("no collision geometry");
    if g.line_live(line) && g.line_kind(line) & crate::mp::FLOOR == 0 {
        code::unported_call("it_802EA804 (a walk starting on a wall reads f27 its caller left)");
    }
    let here = g.walk_floor(line, pos, 0.0, 0.0, 0.0);
    let ahead = mem(fp::fmuls(reg(facing), reg(range)));
    let there = g.walk_floor(line, pos, ahead, 0.0, 0.0);
    let Some(there) = there.filter(|w| w.reached) else {
        return touches_ahead(r, gobj, ahead, 0.0) != 0;
    };
    // The first walk's normal; it can't miss when the second reaches.
    let Some((_, n1)) = here.and_then(|w| w.surface) else {
        code::unported_call("it_802EA804 (a floor normal the first walk left unwritten)");
    };
    let Some((_, n2)) = there.surface else {
        code::unported_call("it_802EA804 (a floor normal the walk left unwritten)");
    };
    let deg = reg(57.295_78);
    let deg1 = fp::fmuls(deg, reg(math::atan2(n1.y, n1.x)));
    let deg2 = fp::fmuls(deg, reg(math::atan2(n2.y, n2.x)));
    let neg_dot = |p: Vec3, n: Vec3| {
        let a = fp::fneg(fp::fmuls(reg(p.x), reg(n.x)));
        let b = fp::fnmsubs(reg(p.y), reg(n.y), a);
        fp::fnmsubs(reg(p.z), reg(n.z), b)
    };
    let dot1 = neg_dot(pos, n1);
    let dot2 = neg_dot(there.pos, n2);
    let mut d = fp::fsubs(deg2, deg1);
    if d < 0.0 {
        d = fp::fneg(d);
    }
    if d > reg(30.0) {
        return false;
    }
    let mut d = fp::fsubs(dot1, dot2);
    if d < 0.0 {
        d = fp::fneg(d);
    }
    !(d > reg(3.0))
}

/// What its collision touches moved by (`dx`, `dy`), `it_802EA988`: 8 for a wall, 4 for a
/// floor or ceiling (from the collision's last pass; the moved copy goes unused).
#[doc(alias = "it_802EA988")]
pub fn touches_ahead(r: &mut Running, gobj: GObjId, _dx: f32, _dy: f32) -> i32 {
    let flags = item(r.game, gobj).x378_itemColl.env_flags;
    let mut ret = 0;
    if flags & 0x3F != 0 {
        ret = 8;
    }
    if flags & 0xFC0 != 0 {
        ret = 4;
    }
    ret
}

/// Grabs `fighter` by its part `part`, `it_802EAAEC`: the fighter's part 12 in front, no
/// pickup, stopped, `xDD0`'s bit 4 cleared, holding (state 5) with its arms' bone there, a
/// grab sound, facing the fighter, on the ground, its bone 0 bound to the part.
#[doc(alias = "it_802EAAEC")]
pub fn grab(r: &mut Running, gobj: GObjId, fighter: GObjId, part: i32) {
    zako::bytes(r.game, gobj).set_f32(off::HOLD_Z, -12.0);
    lib26b1::clear_dcc_b3(r.game, gobj);
    stop(r, gobj);
    zako::clear_dd0_b4(r.game, gobj);
    change_state(r, gobj, 5, change::ANIM_UPDATE);
    let z = zako::bytes(r.game, gobj).f32(off::HOLD_Z);
    set_hold_z(r, gobj, z);
    lib2725::set_xd09(r.game, gobj, true);
    play_sfx(r, gobj, 0x135, 127, 64);
    let v = r.game.ft_pos(fighter);
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = if reg(ip.pos.x) < reg(v.x) { 1.0 } else { -1.0 };
    set_coll_facing(r, gobj);
    let facing = item(r.game, gobj).facing_dir;
    zako::face(r, gobj, facing);
    let _ = ground::ground(r, gobj);
    lib2725::bind_bone(r, gobj, 0, fighter, part);
    zako::bytes(r.game, gobj).set_gobj(off::VICTIM, Some(fighter));
    set(r, gobj, off::PART, part);
}

/// Lets go of the fighter it holds, `it_802EAC8C`: its bone 0 unbound, placed where the held
/// part is less its hold's offset (and set on the floor there, `it_802759DC`), holding nobody,
/// its arms' bone back, its lunge's wait reset, pickup again.
#[doc(alias = "it_802EAC8C")]
pub fn let_go(r: &mut Running, gobj: GObjId) {
    lib2725::unbind_bone(r, gobj, 0);
    let z = zako::bytes(r.game, gobj).f32(off::HOLD_Z);
    let offset = Vec3::new(0.0, 0.0, mem(fp::fneg(reg(z))));
    let f = victim(r, gobj).expect("a ReDead letting go of nobody");
    let part = get(r, gobj, off::PART);
    let j = r.ft_part_jobj(f, part);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, j, Some(offset));
    item_mut(r.game, gobj).pos = pos;
    maplib::place_by(r, gobj, f);
    zako::bytes(r.game, gobj).set_gobj(off::VICTIM, None);
    set(r, gobj, off::PART, 0);
    set_hold_z(r, gobj, 0.0);
    let wait = attr_s16(r, gobj, 0x1C);
    set(r, gobj, off::LUNGE_WAIT, wait);
    set(r, gobj, off::TIMER, 0);
    lib26b1::set_dcc_b3(r.game, gobj);
}

/// The held fighter is hit, `it_802EADD8`: it lets go, then stands on the ground or falls.
#[doc(alias = "it_802EADD8")]
pub fn victim_hit(r: &mut Running, gobj: GObjId) {
    let_go(r, gobj);
    if item(r.game, gobj).ground_or_air == GA_GROUND {
        land(r, gobj);
    } else {
        fall(r, gobj);
    }
}

/// The held fighter mashed free, `it_802EAE80`: it lets go, then on the ground stops and lets
/// go of its hold (state 6), or falls.
#[doc(alias = "it_802EAE80")]
pub fn victim_free(r: &mut Running, gobj: GObjId) {
    let_go(r, gobj);
    if item(r.game, gobj).ground_or_air == GA_GROUND {
        stop(r, gobj);
        zako::clear_dd0_b4(r.game, gobj);
        change_state(r, gobj, 6, change::ANIM_UPDATE);
    } else {
        fall(r, gobj);
    }
}

/// The fighter a ReDead holds, `it_802EAF28`.
#[doc(alias = "it_802EAF28")]
pub fn held(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    victim(r, gobj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_follow_the_decomp() {
        assert_eq!(off::XDEC, zako::off::X14 + 4);
        assert_eq!(off::HURT + 4, 0x60);
        assert_eq!(off::ECB, 0x7C);
    }
}
