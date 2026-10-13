//! Item helpers from `melee/it/it_26B1.c`: flags, attributes, positions, and the item ends of
//! effects and bursts.

use gekko_fp as fp;
use melee_data::archive::Archives;

use super::data::common;
use super::item::{attr_of, common_data, destroy};
use super::{flag, item, item_mut};
use crate::fpu::{mem, reg};
use crate::game::code;
use crate::game::state::{Game, Running};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// How many items of `kind` are out, `it_8026B3C0`.
#[doc(alias = "it_8026B3C0")]
pub fn count_kind(game: &Game, kind: i32) -> i32 {
    let mut n = 0;
    let mut cur = game.world.first_in(super::P_LINK_ITEM);
    while let Some(g) = cur {
        if item(game, g).kind == kind {
            n += 1;
        }
        cur = game.world.gobjs[g.0].next;
    }
    n
}

/// Whether the item is heavy (a crate), `itIsHeavy`.
#[doc(alias = "itIsHeavy")]
pub fn is_heavy(game: &Game, data: &Archives, gobj: GObjId) -> bool {
    attr_of(item(game, gobj)).is_heavy(data)
}

/// How a fighter uses the item (its attributes' `x0_78`), `it_8026B30C`.
// Ports `it_8026B30C`; aliased once a check reaches it.
pub fn use_kind(game: &Game, data: &Archives, gobj: GObjId) -> i32 {
    i32::from(attr_of(item(game, gobj)).x0_78(data))
}

/// Scales the item to `mul` times its attributes' scale, `it_8026BAE8`.
#[doc(alias = "it_8026BAE8")]
pub fn set_scale(game: &mut Game, data: &Archives, gobj: GObjId, mul: f32) {
    let ip = item(game, gobj);
    let scale = mem(fp::fmuls(
        reg(mul),
        reg(attr_of(ip).f32(data, super::data::attr::SCALE)),
    ));
    super::item_mut(game, gobj).scl = scale;
    let j = super::jobj_of(game, gobj).expect("the item's joint");
    super::lib2725::set_uniform_scale(game, j, scale);
}

/// Lets fighters pick the item up, `it_8026B390`.
#[doc(alias = "it_8026B390")]
pub fn enable_pickup(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x15 = true;
}

/// Stops fighters picking the item up, `it_8026B3A8`.
#[doc(alias = "it_8026B3A8")]
pub fn disable_pickup(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x15 = false;
}

/// The hitlag a hit of `damage` deals an item, `it_8026B424`: truncated, as a float.
#[doc(alias = "it_8026B424")]
pub fn hitlag_frames(game: &Game, data: &Archives, damage: i32) -> f64 {
    let c = common_data(game);
    let v = fp::fmadds(
        fp::frsp(f64::from(damage)),
        reg(c.f32(data, common::XB8)),
        reg(c.f32(data, common::XBC)),
    );
    fp::frsp(f64::from(fp::fctiwz(v)))
}

/// Forgets a GObj the item points to, `it_8026B894`: whether it was the owner.
#[doc(alias = "it_8026B894")]
pub fn forget(game: &mut Game, gobj: GObjId, other: Option<GObjId>) -> bool {
    let ip = item_mut(game, gobj);
    let mut was_owner = false;
    if ip.owner == other {
        ip.owner = None;
        was_owner = true;
    }
    if ip.xC64_reflectGObj == other {
        ip.xC64_reflectGObj = None;
    }
    if ip.xC90_absorbGObj == other {
        ip.xC90_absorbGObj = None;
    }
    if ip.xCEC_fighterGObj == other {
        ip.xCEC_fighterGObj = None;
        ip.xCB0_source_ply = 6;
    }
    if ip.xCF4_fighterGObjUnk == other {
        ip.xCF4_fighterGObjUnk = None;
    }
    if ip.toucher == other {
        ip.toucher = None;
    }
    was_owner
}

/// The ECB's center, `it_8026BB88`.
#[doc(alias = "it_8026BB88")]
pub fn ecb_center(game: &Game, gobj: GObjId) -> Vec3 {
    let ip = item(game, gobj);
    let e = &ip.x378_itemColl.ecb;
    let dy = fp::fmuls(0.5, fp::fadds(reg(e.top.y), reg(e.bottom.y)));
    Vec3::new(
        mem(fp::fadds(reg(ip.pos.x), 0.0)),
        mem(fp::fadds(reg(ip.pos.y), dy)),
        mem(fp::fadds(reg(ip.pos.z), 0.0)),
    )
}

/// The last collision's ECB center, `it_8026BBCC`.
#[doc(alias = "it_8026BBCC")]
pub fn last_ecb_center(game: &Game, gobj: GObjId) -> Vec3 {
    let cd = &item(game, gobj).x378_itemColl;
    let e = &cd.xE4_ecb;
    let dy = fp::fmuls(0.5, fp::fadds(reg(e.top.y), reg(e.bottom.y)));
    Vec3::new(
        mem(fp::fadds(reg(cd.last_pos.x), 0.0)),
        mem(fp::fadds(reg(cd.last_pos.y), dy)),
        mem(fp::fadds(reg(cd.last_pos.z), 0.0)),
    )
}

/// `xDD0_flag.b0`, `it_8026BC68`.
#[doc(alias = "it_8026BC68")]
pub fn is_stomp(game: &Game, gobj: GObjId) -> bool {
    flag(item(game, gobj).xDD0_flag, 0)
}

/// The item's owner, `itGetOwner`.
#[doc(alias = "itGetOwner")]
pub fn owner(game: &Game, gobj: GObjId) -> Option<GObjId> {
    item(game, gobj).owner
}

/// The item's attack id, `itGetAttackId`.
#[doc(alias = "itGetAttackId")]
pub fn attack_id(game: &Game, gobj: GObjId) -> i32 {
    item(game, gobj).xD88_attackID
}

/// Marks the item quiet, `it_8026BD6C`.
#[doc(alias = "it_8026BD6C")]
pub fn set_xdcd_b3(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDCD_flag.x0.b3 = true;
}

/// Spawns the effects the item queued (`efAsync_QueueFlush` on `xBC0`).
pub fn flush_effects(r: &mut Running, gobj: GObjId) {
    let mut head = item(r.game, gobj).xBC0;
    if head.is_some() {
        crate::game::ef::flush(r, Some(gobj), &mut head);
        item_mut(r.game, gobj).xBC0 = head;
    }
}

/// Drops the effects the item queued (`efAsync_QueueClear` on `xBC0`).
pub fn clear_effect_queue(game: &mut Game, gobj: GObjId) {
    let mut head = item(game, gobj).xBC0;
    if head.is_some() {
        crate::game::ef::queue_clear(game, &mut head);
        item_mut(game, gobj).xBC0 = head;
    }
}

/// Bursts the item if its collision says so (`x34_flags.b7`), `it_8026C368`: bombs explode,
/// other items vanish.
#[doc(alias = "it_8026C368")]
pub fn check_burst(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    if !ip.x378_itemColl.x34_flags.b7 {
        return;
    }
    match ip.kind {
        6 => code::unported_call("it_8027D730"),
        19 => code::unported_call("it_8028FE90"),
        58 | 59 => code::unported_call("it_8029D968"),
        177 => code::unported_call("it_802D09D0"),
        _ => {
            item_mut(r.game, gobj).destroy_type = 0;
            destroy(r, gobj);
        }
    }
}

/// `xDD0_flag.b3`, `it_8026BD24`.
#[doc(alias = "it_8026BD24")]
pub fn set_xdd0_b3(game: &mut Game, gobj: GObjId) {
    super::set_flag(&mut item_mut(game, gobj).xDD0_flag, 3, true);
}

/// Stops the item keeping clear of others (`xDC8_word.x1A`), `it_8026BDB4`.
#[doc(alias = "it_8026BDB4")]
pub fn stop_keeping_clear(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDC8_word.flags.x1A = false;
}

/// Hides the item's model, `it_8026BB44`.
#[doc(alias = "it_8026BB44")]
pub fn hide(game: &mut Game, gobj: GObjId) {
    let j = super::jobj_of(game, gobj);
    super::lib2725::hide_tree(game, j);
}

/// Shows the item's model, `it_8026BB20`.
#[doc(alias = "it_8026BB20")]
pub fn show(game: &mut Game, gobj: GObjId) {
    let j = super::jobj_of(game, gobj);
    super::lib2725::show_tree(game, j);
}
