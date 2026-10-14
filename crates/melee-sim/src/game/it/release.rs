//! An item leaving a fighter's hand (`it_2725.c`): `it_80273B50` lets go of the model and
//! sends the item off at a speed from the hand, and `it_80273F34` makes it a thrown item in the
//! air, out of its holder's hand. `it_8027429C` and `it_80274250` are the two together.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::item::{apply_scale, attr_of, release_from, stop_sfx2};
use super::{hitbox, item, item_mut, jobj_of, lib26b1, lib279c, lib2725, maplib};
use crate::fpu::{mem, reg};
use crate::game::code::unported_call;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `M_PI / 2`.
const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;

/// The item's model leaves the hand at `vel`, `it_80273B50`: its attach bone's constraints go
/// (`it_80275070`), it can be picked up unless a stage item, its blink ends, its model resets
/// to its descriptors and scale, its spin is set from its angle; it flies at `vel` times its
/// throw speed, facing its way; and it starts at its holder's hand joint (less its attach
/// bone's offset for a heavy item held two-handed, or a held special item).
#[doc(alias = "it_80273B50")]
pub fn let_go_model(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let data = r.data;
    let ip = item(r.game, gobj);
    let mut owner = ip.owner;
    let attach = super::item::article(ip).model(data).bone_attach_id(data);
    // `it_80275070`.
    if let Some(j) = lib2725::bone(r.game, data, gobj, attach) {
        crate::game::ft::co::throw::unconstrain(&mut r.game.objs, j);
    }
    if !is_stage_item(r.game, gobj) {
        lib26b1::enable_pickup(r.game, gobj);
    }
    hitbox::end_blink(r.game, gobj);
    let rotation = lib2725::spin_angle(r.game, data, gobj);
    let root = jobj_of(r.game, gobj);
    let joint = item(r.game, gobj).xC8_joint;
    crate::game::gr::anime::reset_tree_srt(&mut r.game.objs, data, root, joint);
    apply_scale(r.game, gobj);
    lib2725::clamp_fall(r.game, gobj, rotation);
    let ip = item(r.game, gobj);
    if ip.xDC8_word.flags.x0 {
        owner = ip.x51C;
    }
    let mul = reg(attr_of(ip).f32(data, super::data::attr::THROW_SPEED_MUL));
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = vel;
    ip.x40_vel.x = mem(fp::fmuls(reg(ip.x40_vel.x), mul));
    ip.x40_vel.y = mem(fp::fmuls(reg(ip.x40_vel.y), mul));
    ip.x40_vel.z = mem(fp::fmuls(reg(ip.x40_vel.z), mul));
    let x = reg(ip.x40_vel.x);
    let speed = if x < 0.0 { fp::fneg(x) } else { x };
    if !(speed < 1e-5_f32 as f64) || ip.facing_dir == 0.0 {
        ip.facing_dir = if ip.x40_vel.x >= 0.0 { 1.0 } else { -1.0 };
    }
    let dir = if ip.facing_dir == -1.0 { -1 } else { 1 };
    ip.x378_itemColl.set_facing_dir(dir);
    if ip.xDC8_word.flags.x19 {
        let yaw = mem(fp::frsp(fp::fmul(HALF_PI, reg(ip.facing_dir))));
        let j = root.expect("an item without a model");
        jobj::set_rotation_y(&mut r.game.objs.jobjs, j, yaw);
    }
    let ip = item(r.game, gobj);
    let heavy = lib26b1::is_heavy(r.game, data, gobj);
    let offset = if (heavy && ip.hold_kind != 4 && ip.hold_kind != 6) || ip.hold_kind == 8 {
        let j = lib2725::bone(r.game, data, gobj, attach).expect("the item's attach bone");
        let t = r.game.objs.jobjs[j].translate;
        Vec3::new(
            mem(fp::fneg(reg(t.x))),
            mem(fp::fneg(reg(t.y))),
            mem(fp::fneg(reg(t.z))),
        )
    } else {
        Vec3::new(0.0, 0.0, 0.0)
    };
    // `getOwnerJointPosition`.
    let owner = owner.expect("a held item without a holder");
    let part = item(r.game, gobj).xDC4;
    let hand = r.bone_joint(owner, part as i32);
    let at = crate::lb::joint::world_position(&mut r.game.objs, data, hand, Some(offset));
    let ip = item_mut(r.game, gobj);
    ip.pos.x = at.x;
    ip.pos.y = at.y;
    ip.pos.z = 0.0;
    let pos = ip.pos;
    let j = root.expect("an item without a model");
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
}

/// Thrown into the air by `thrower`, `it_80273F34`: not held, counted as thrown, its
/// collision started from its thrower, its spin's state reset, facing its way; out of a
/// fighter's hand with the fighter's attack taken; its second sound stopped, in the air at its
/// position, and a stage item's throw counted for its thrower.
#[doc(alias = "it_80273F34")]
pub fn thrown(r: &mut Running, gobj: GObjId, thrower: Option<GObjId>) {
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x13 = false;
    ip.xDC8_word.flags.x14 = true;
    ip.xDC4 = 0;
    ip.xD54_throwNum = ip.xD54_throwNum.wrapping_add(1);
    ip.xD50_landNum = 0;
    start_coll(r, gobj, thrower);
    lib2725::init_spin(r.game, gobj);
    let root = jobj_of(r.game, gobj).expect("an item without a model");
    let ip = item(r.game, gobj);
    if ip.xDC8_word.flags.x19 {
        let yaw = mem(fp::frsp(fp::fmul(HALF_PI, reg(ip.facing_dir))));
        jobj::set_rotation_y(&mut r.game.objs.jobjs, root, yaw);
    }
    if super::item::is_fighter(r.game, thrower) {
        let t = thrower.unwrap();
        release_from(r, gobj, t);
        lib279c::copy_fighter_attack(r.game, gobj, t);
    }
    let ip = item_mut(r.game, gobj);
    let dir = if ip.facing_dir == -1.0 { -1 } else { 1 };
    ip.x378_itemColl.set_facing_dir(dir);
    stop_sfx2(r, gobj);
    maplib::enter_air(item_mut(r.game, gobj));
    let pos = item(r.game, gobj).pos;
    jobj::set_translate(&mut r.game.objs.jobjs, root, pos);
    if is_stage_item(r.game, gobj) {
        unported_call("it_8027B4A4 and it_8027B378 (a stage item's throw in its thrower's stats)");
    }
}

/// The thrown item's collision, `it_80275BC8`: in the air from its position, its box from the
/// thrower's ECB center (or its own position) at the common data's scale.
#[doc(alias = "it_80275BC8")]
pub fn start_coll(r: &mut Running, gobj: GObjId, thrower: Option<GObjId>) {
    let ip = item_mut(r.game, gobj);
    let mut b = ip.xBFC.clone();
    maplib::enter_air(ip);
    ip.x378_itemColl.cur_pos = ip.pos;
    let from = match thrower {
        Some(t) if super::item::is_fighter(r.game, Some(t)) => {
            lib26b1::parent_ecb_center(r.game, t)
        }
        Some(t) if super::is_item(r.game, Some(t)) => lib26b1::ecb_center(r.game, t),
        Some(_) => panic!("the original's assertion failed: couldn't get Owner_GObj_Kind!!"),
        None => item(r.game, gobj).pos,
    };
    let s = reg(super::item::common_data(r.game).f32(r.data, super::data::common::XE8));
    b.top = mem(fp::fmuls(reg(b.top), s));
    b.bottom = mem(fp::fmuls(reg(b.bottom), s));
    b.right = mem(fp::fmuls(reg(b.right), s));
    b.left = mem(fp::fmuls(reg(b.left), s));
    let ip = item_mut(r.game, gobj);
    ip.xC0C = ip.xC1C.clone();
    if super::flag(ip.xDCE_flag, 7) {
        maplib::set_box(r.game, gobj, b);
    }
    maplib::place_air(r, gobj, from);
}

/// Let go of and thrown at `vel` by its holder, `it_80274250`.
// Ports `it_80274250`; aliased once a check reaches it.
pub fn throw(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let_go_model(r, gobj, vel);
    let ip = item(r.game, gobj);
    let thrower = if ip.xDC8_word.flags.x0 {
        ip.x51C
    } else {
        ip.owner
    };
    thrown(r, gobj, thrower);
}

/// [`throw`], and an item held for another (`xDC8` bit 0) goes back to that owner,
/// `it_8027429C`.
#[doc(alias = "it_8027429C")]
pub fn let_go(r: &mut Running, gobj: GObjId, vel: Vec3) {
    let_go_model(r, gobj, vel);
    let ip = item_mut(r.game, gobj);
    let mut thrower = ip.owner;
    if ip.xDC8_word.flags.x0 {
        thrower = ip.x51C;
        ip.owner = thrower;
        ip.x51C = None;
    }
    thrown(r, gobj, thrower);
}

/// Whether the item is a stage's (an enemy or a stage item, but for the Kyasarin's egg and
/// Whispy's apple), `it_8026B6C8`.
#[doc(alias = "it_8026B6C8")]
pub fn is_stage_item(game: &crate::game::state::Game, gobj: GObjId) -> bool {
    use super::kinds::{ARWING_LASER, KURIBOH, OCTAROCK_STONE, OLD_KURI};
    let k = item(game, gobj).kind;
    ((KURIBOH..OCTAROCK_STONE).contains(&k) || (OLD_KURI..ARWING_LASER).contains(&k))
        && k != KYASARIN_EGG
        && k != WHISPY_APPLE
}

/// `It_Kind_Kyasarin_Egg` and `It_Kind_WhispyApple`.
const KYASARIN_EGG: i32 = 241;
const WHISPY_APPLE: i32 = 230;
