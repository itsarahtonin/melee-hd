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
#[doc(alias = "it_8026B30C")]
pub fn use_kind(game: &Game, data: &Archives, gobj: GObjId) -> i32 {
    i32::from(attr_of(item(game, gobj)).x0_78(data))
}

/// Whether the item, of kind 16, 21 to 23 or 25, has no uses left (`xD4C`), `it_8026B594`.
// Ports `it_8026B594`; aliased once a check reaches it.
pub fn is_empty_shooter(game: &Game, gobj: GObjId) -> bool {
    let ip = item(game, gobj);
    matches!(ip.kind, 16 | 21 | 22 | 23 | 25) && ip.xD4C <= 0
}

/// The item's kind, `itGetKind`.
#[doc(alias = "itGetKind")]
pub fn kind(game: &Game, gobj: GObjId) -> i32 {
    item(game, gobj).kind
}

/// The Lucky's egg and Whispy's apple, the healing items besides the heart, tomato and food.
pub const LUCKY_EGG: i32 = 205;
pub const WHISPY_HEAL_APPLE: i32 = 226;

/// A healing item's heal, 0 for any other item, `it_8026B47C`.
#[doc(alias = "it_8026B47C")]
pub fn heal_amount(game: &mut Game, gobj: GObjId) -> i32 {
    let kind = item(game, gobj).kind;
    let v = &mut item_mut(game, gobj).xDD4_itemVar;
    match kind {
        super::kinds::HEART => v.heart().xDD4_heal.get(),
        super::kinds::TOMATO => v.tomato().heal_amount.get(),
        super::kinds::FOODS => v.foods().heal_amount.get() as i32,
        LUCKY_EGG => v.egg().heal_amount.get(),
        WHISPY_HEAL_APPLE => code::unported_call("it_8026B47C for Whispy's apple"),
        _ => 0,
    }
}

/// How a fighter's hand holds the item, `itGetHoldKind`.
#[doc(alias = "itGetHoldKind")]
pub fn hold_kind(game: &Game, data: &Archives, gobj: GObjId) -> i32 {
    i32::from(attr_of(item(game, gobj)).hold_kind(data))
}

/// The item's damage multiplier, `itGetDamageMultiplier`.
#[doc(alias = "itGetDamageMultiplier")]
pub fn damage_mul(game: &Game, data: &Archives, gobj: GObjId) -> f32 {
    attr_of(item(game, gobj)).f32(data, super::data::attr::DAMAGE_MUL)
}

/// Where fighters reach for the item, `it_8026B344`: its position plus its grab offset
/// (`xBCC_unk`) the way it faces.
// Ports `it_8026B344`; aliased once a check reaches it.
pub fn grab_pos(game: &Game, gobj: GObjId) -> Vec3 {
    let ip = item(game, gobj);
    Vec3::new(
        mem(fp::fmadds(
            reg(ip.facing_dir),
            reg(ip.xBCC_unk.x),
            reg(ip.pos.x),
        )),
        mem(fp::fadds(reg(ip.pos.y), reg(ip.xBCC_unk.y))),
        ip.pos.z,
    )
}

/// How far from its grab point fighters reach it, `itGetGrabRangeX` and `itGetGrabRangeY`.
// Ports `itGetGrabRangeX` and `itGetGrabRangeY`; aliased once a check reaches them.
pub fn grab_range(game: &Game, gobj: GObjId) -> (f32, f32) {
    let r = &item(game, gobj).xBD4_grabRange;
    (r.x, r.y)
}

/// Lets the item make its throw and drop sounds, `it_8026BCF4`.
#[doc(alias = "it_8026BCF4")]
pub fn unsilence(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDCD_flag.x0.b2 = false;
}

/// Silences the item's throw and drop sounds, `it_8026BD0C`.
#[doc(alias = "it_8026BD0C")]
pub fn silence(game: &mut Game, gobj: GObjId) {
    item_mut(game, gobj).xDCD_flag.x0.b2 = true;
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

/// The item's spawn number (`x1C`), `it_8026B7CC`.
#[doc(alias = "it_8026B7CC")]
pub fn spawn_number(game: &Game, gobj: GObjId) -> i32 {
    item(game, gobj).x1C
}

/// What's left of a lit fuse, or -1: a Bob-omb's (`xDEC`) unless it's exploding (state 11),
/// Link's bomb's life unless it's exploding (state 5), `it_8026B960`.
#[doc(alias = "it_8026B960")]
pub fn fuse_left(game: &mut Game, gobj: GObjId) -> f32 {
    let ip = super::item_mut(game, gobj);
    match ip.kind {
        super::kinds::BOMBHEI if ip.msid != 0xB => ip.xDD4_itemVar.bombhei().xDEC.get(),
        super::kinds::LINK_BOMB if ip.msid != 5 => ip.xD44_lifeTimer,
        _ => -1.0,
    }
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

/// The first float of the kind's own attributes, `it_8026B560` (the Metal Box's frames).
#[doc(alias = "it_8026B560")]
pub fn special_x0(game: &Game, data: &Archives, gobj: GObjId) -> f32 {
    let a = super::item::article(item(game, gobj))
        .special(data)
        .expect("an item without its attributes");
    data.f32(a)
}

/// The same float, `it_8026B54C` (the Bunny Hood's frames).
#[doc(alias = "it_8026B54C")]
pub fn special_x0_hood(game: &Game, data: &Archives, gobj: GObjId) -> f32 {
    special_x0(game, data, gobj)
}

/// The second float of the kind's own attributes, `it_8026B574` (the Metal Box's health).
#[doc(alias = "it_8026B574")]
pub fn special_x4(game: &Game, data: &Archives, gobj: GObjId) -> f32 {
    let a = super::item::article(item(game, gobj))
        .special(data)
        .expect("an item without its attributes");
    data.f32(a.at(4))
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
        58 | 59 => super::kinds::linkbomb::set_off(r, gobj),
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

/// Turns off the flags an item holding still for its animation keeps (`xDD0_flag.b3`,
/// `xDCD_flag.b3` and `b4`) and on `xDCC_flag.b3` and `xDC8_word.x1A`, `it_8026BDCC`.
#[doc(alias = "it_8026BDCC")]
pub fn hold_flags_off(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    super::set_flag(&mut ip.xDD0_flag, 3, false);
    ip.xDCC_flag.b3 = true;
    ip.xDCD_flag.x0.b3 = false;
    ip.xDCD_flag.x0.b4 = false;
    ip.xDC8_word.flags.x1A = true;
}

/// The inverse of [`hold_flags_off`], `it_8026BE28`.
#[doc(alias = "it_8026BE28")]
pub fn hold_flags_on(game: &mut Game, gobj: GObjId) {
    let ip = item_mut(game, gobj);
    super::set_flag(&mut ip.xDD0_flag, 3, true);
    ip.xDCC_flag.b3 = false;
    ip.xDCD_flag.x0.b3 = true;
    ip.xDCD_flag.x0.b4 = true;
    ip.xDC8_word.flags.x1A = false;
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

/// Where an item from `fighter` starts, `it_8026BB68`: the fighter's ECB center
/// (`ftLib_GetECBCenter`).
#[doc(alias = "it_8026BB68")]
pub fn parent_ecb_center(game: &Game, fighter: GObjId) -> crate::vec::Vec3 {
    super::owner::ecb_center(game, fighter)
}

/// The nearest target item ahead of `pos` on the side `facing_dir` looks toward (a target, a
/// lock-on target, an enemy or kind 7, not grabbed, not held unless ownerless),
/// `it_8026C258`.
#[doc(alias = "it_8026C258")]
pub fn nearest_target_item(game: &Game, pos: Vec3, facing_dir: f32) -> Option<GObjId> {
    let mut best = f64::from(f32::MAX);
    let mut result = None;
    let mut cur = game.world.first_in(super::P_LINK_ITEM);
    while let Some(g) = cur {
        cur = game.world.gobjs[g.0].next;
        let ip = item(game, g);
        let k = ip.hold_kind;
        if !matches!(k, 4..=7)
            || ip.grab_victim.is_some()
            || (ip.xDC8_word.flags.x13 && ip.owner.is_some())
            || (facing_dir == -1.0 && ip.pos.x > pos.x)
            || (facing_dir == 1.0 && ip.pos.x < pos.x)
        {
            continue;
        }
        let dx = fp::fsubs(reg(pos.x), reg(ip.pos.x));
        let dy = fp::fsubs(reg(pos.y), reg(ip.pos.y));
        let d = fp::fmadds(dx, dx, fp::fmuls(dy, dy));
        if d < best {
            best = d;
            result = Some(g);
        }
    }
    result
}
