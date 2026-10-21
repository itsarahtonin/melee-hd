//! An item leaving a fighter's hand (`it_2725.c`): `it_80273B50` lets go of the model and
//! sends the item off at a speed from the hand, and `it_80273F34` makes it a thrown item in the
//! air, out of its holder's hand. `it_8027429C` and `it_80274250` are the two together.

// NaN takes the original's branches, which the negated comparisons spell out.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::item::{apply_scale, attr_of, release_from, stop_sfx2};
use super::{hitbox, item, item_mut, jobj_of, lib26b1, lib279c, lib2725, maplib};
use crate::fpu::{mem, reg};
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
    let (owner, attach, root) = let_go_start(r, gobj, vel);
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

/// The item's model leaves the hand at `vel` from `pos`, `it_80273748`: as
/// [`let_go_model`], placed at `pos` (moved by the attach bone's offset in the hand for a
/// heavy item held two-handed, or a held special item).
#[doc(alias = "it_80273748")]
pub fn let_go_model_at(r: &mut Running, gobj: GObjId, pos: Vec3, vel: Vec3) {
    let data = r.data;
    let (owner, attach, root) = let_go_start(r, gobj, vel);
    let ip = item(r.game, gobj);
    let heavy = lib26b1::is_heavy(r.game, data, gobj);
    let offset = if (heavy && ip.hold_kind != 4 && ip.hold_kind != 6) || ip.hold_kind == 8 {
        let j = lib2725::bone(r.game, data, gobj, attach).expect("the item's attach bone");
        let t = r.game.objs.jobjs[j].translate;
        let back = Vec3::new(
            mem(fp::fneg(reg(t.x))),
            mem(fp::fneg(reg(t.y))),
            mem(fp::fneg(reg(t.z))),
        );
        let owner = owner.expect("a held item without a holder");
        let part = item(r.game, gobj).xDC4;
        let hand = r.bone_joint(owner, part as i32);
        let a = crate::lb::joint::world_position(&mut r.game.objs, data, hand, Some(back));
        let b = crate::lb::joint::world_position(&mut r.game.objs, data, hand, None);
        Vec3::new(
            mem(fp::fsubs(reg(a.x), reg(b.x))),
            mem(fp::fsubs(reg(a.y), reg(b.y))),
            0.0,
        )
    } else {
        Vec3::new(0.0, 0.0, 0.0)
    };
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fadds(reg(pos.x), reg(offset.x)));
    ip.pos.y = mem(fp::fadds(reg(pos.y), reg(offset.y)));
    ip.pos.z = 0.0;
    let pos = ip.pos;
    let j = root.expect("an item without a model");
    jobj::set_translate(&mut r.game.objs.jobjs, j, pos);
}

/// What [`let_go_model`] and [`let_go_model_at`] share: the attach bone's constraints gone
/// (`it_80275070`), pickable unless a stage item, the blink ended, the model reset to its
/// descriptors and scale, the spin set from its angle, and the flight at `vel` times its
/// throw speed, facing its way. Returns the holder, the attach bone and the model's root.
fn let_go_start(
    r: &mut Running,
    gobj: GObjId,
    vel: Vec3,
) -> (Option<GObjId>, i32, Option<jobj::JObjId>) {
    let data = r.data;
    let ip = item(r.game, gobj);
    let mut owner = ip.owner;
    let attach = super::item::article(ip).model(data).bone_attach_id(data);
    // `it_80275070`.
    if let Some(j) = lib2725::bone(r.game, data, gobj, attach) {
        crate::game::ft::co::throw::unconstrain(&mut r.game.objs, j);
    }
    if !super::coll::is_enemy(r.game, gobj) {
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
    (owner, attach, root)
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
    if super::coll::is_enemy(r.game, gobj) {
        let t = thrower.expect("a thrown enemy without its thrower");
        super::coll::enemy_hit_by_fighter(r, t, gobj);
        let dmg = super::hitbox::max_damage(r.game, gobj);
        super::coll::enemy_hit_stats(r, t, gobj, reg(dmg));
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

/// Thrown into the air by its holder, `it_80274198` and `it_802741F4` (the same code): an
/// item held for another (`xDC8` bit 0) goes back to that owner when `give_back`, else is
/// thrown by it.
#[doc(alias = "it_80274198")]
#[doc(alias = "it_802741F4")]
pub fn thrown_by_holder(r: &mut Running, gobj: GObjId, give_back: bool) {
    let ip = item_mut(r.game, gobj);
    let mut thrower = ip.owner;
    if ip.xDC8_word.flags.x0 {
        if give_back {
            thrower = ip.x51C;
            ip.owner = thrower;
            ip.x51C = None;
        } else {
            thrower = ip.x51C;
        }
    }
    thrown(r, gobj, thrower);
}

/// The drop sound (`xD78`) unless the item is silenced (`xDCD` bit 2), `it_802731A4`.
#[doc(alias = "it_802731A4")]
pub fn drop_sfx(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDCD_flag.x0.b2 {
        let sfx = ip.xD78;
        super::item::play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
}

/// The throw sound (`xD74`) unless the item is silenced (`xDCD` bit 2), `it_802731E0`.
#[doc(alias = "it_802731E0")]
pub fn throw_sfx(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.xDCD_flag.x0.b2 {
        let sfx = ip.xD74;
        super::item::play_sfx(r, gobj, sfx, 0x7F, 0x40);
    }
}

/// Dropped from the hand, `Item_8026ABD8`: its drop sound, let go at `vel` (`it_80273B50`),
/// the kind's `dropped` callback, thrown by its holder, its hits counted anew, and a stage
/// item forgets its owner.
#[doc(alias = "Item_8026ABD8")]
pub fn drop(r: &mut Running, gobj: GObjId, vel: Vec3, speed: f32) {
    item_mut(r.game, gobj).xC44 = speed;
    drop_sfx(r, gobj);
    let_go_model(r, gobj, vel);
    if let Some(cb) = super::item::logic(r.game, gobj).dropped {
        super::item::call(r, gobj, cb);
    }
    thrown_by_holder(r, gobj, true);
    hitbox::set_hit_ids2(r.game, gobj);
    if super::coll::is_enemy(r.game, gobj) {
        lib2725::drop_owner(r.game, gobj);
    }
}

/// Dropped from `pos` at `vel`, `Item_8026AC74` (a light throw's drop).
// Ports `Item_8026AC74`; aliased once a check reaches it.
pub fn drop_at(r: &mut Running, gobj: GObjId, pos: Vec3, vel: Vec3, speed: f32) {
    item_mut(r.game, gobj).xC44 = speed;
    drop_sfx(r, gobj);
    let_go_model_at(r, gobj, pos, vel);
    if let Some(cb) = super::item::logic(r.game, gobj).dropped {
        super::item::call(r, gobj, cb);
    }
    thrown_by_holder(r, gobj, true);
    hitbox::set_hit_ids2(r.game, gobj);
    if super::coll::is_enemy(r.game, gobj) {
        lib2725::drop_owner(r.game, gobj);
    }
}

/// Thrown from `pos` at `vel`, `Item_8026AD20`: its throw sound, the kind's `thrown`
/// callback, thrown by its holder and its hits counted anew. (The original's last argument,
/// whether it was a smash throw, goes unread.)
#[doc(alias = "Item_8026AD20")]
pub fn throw_at(r: &mut Running, gobj: GObjId, pos: Vec3, vel: Vec3, speed: f32) {
    throw_sfx(r, gobj);
    item_mut(r.game, gobj).xC44 = speed;
    let_go_model_at(r, gobj, pos, vel);
    if let Some(cb) = super::item::logic(r.game, gobj).thrown {
        super::item::call(r, gobj, cb);
    }
    thrown_by_holder(r, gobj, true);
    hitbox::set_hit_ids2(r.game, gobj);
}
