//! Foods, `melee/it/kinds/itfoods.c`: one of the attributes' dishes, drawn at random, that falls,
//! lands with a sound and heals whoever picks it up (the fighter's side,
//! `ftpickupitem_8009447C`). Mr. Game & Watch's Judgement 7 spawns them too.

use super::super::ground::{fall_to_floor, ground_or_fall};
use super::super::item::{article, change, change_state, play_sfx, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::{GObjId, Host};
use crate::hsd::jobj;
use crate::vec::Vec3;
use gekko_fp as fp;
use melee_data::archive::DataRef;

pub const SPAWNED: u32 = 0x8028_FBE4;
pub const FELL: u32 = 0x8028_FC5C;
pub const ANIM0: u32 = 0x8028_FC84;
pub const PHYS0: u32 = 0x8028_FC8C;
pub const COLL0: u32 = 0x8028_FCBC;
pub const LANDED: u32 = 0x8028_FCE8;
pub const ANIM1: u32 = 0x8028_FD44;
pub const PHYS1: u32 = 0x8028_FD4C;
pub const COLL1: u32 = 0x8028_FD50;
pub const PICKED_UP: u32 = 0x8028_FD7C;
pub const ANIM2: u32 = 0x8028_FDD8;
pub const PHYS2: u32 = 0x8028_FDE0;
pub const DROPPED: u32 = 0x8028_FDE4;
pub const ANIM3: u32 = 0x8028_FE0C;
pub const PHYS3: u32 = 0x8028_FE14;
pub const COLL3: u32 = 0x8028_FE44;

/// The landing sound.
const SFX_LAND: i32 = 0x107;

/// Runs one of the food's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM3 => still(r, gobj),
        COLL0 | COLL3 => falling(r, gobj),
        COLL1 => coll1(r, gobj),
        _ => return None,
    })
}

/// Runs one of the food's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS3 => gravity(r, gobj),
        PHYS1 | PHYS2 => idle(r, gobj),
        LANDED => landed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        _ => return false,
    }
    true
}

/// `itFoodsAttributes`: a dish count (the first entry's `x0`), then a dish per 16 bytes, its
/// model (`x4`) and heal (`x8`).
fn attrs(r: &Running, gobj: GObjId) -> DataRef {
    article(item(r.game, gobj))
        .special(r.data)
        .expect("a food without attributes")
}

/// A food at `pos` from `parent`, if foods are on, `it_8028FAF4`: falling from the parent's
/// center (`it_8026BB68`) or `pos`, facing left.
#[doc(alias = "it_8028FAF4")]
pub fn spawn(r: &mut Running, parent: Option<GObjId>, pos: Vec3) -> Option<GObjId> {
    if !super::super::spawn::kind_on(r.game, super::FOODS) {
        return None;
    }
    let mut spawn = SpawnItem {
        kind: super::FOODS,
        prev_pos: pos,
        ..Default::default()
    };
    spawn.pos = match parent {
        Some(p) => lib26b1::parent_ecb_center(r.game, p),
        None => spawn.prev_pos,
    };
    spawn.facing_dir = -1.0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x3C_damage = 0;
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    lib2725::count_spawned(r.game);
    Some(gobj)
}

/// Places the food at `pos` plus its dish's offset (the attributes read as `Vec4`s: the
/// dish's `w` times `dir` across, the next one's `x` up), `it_8028F9D8`.
// Ports `it_8028F9D8`; aliased once a check reaches it.
pub fn place(r: &mut Running, gobj: GObjId, pos: Vec3, dir: f64) {
    let a = attrs(r, gobj);
    let dish = item_mut(r.game, gobj).xDD4_itemVar.foods().x0.get();
    let w = r.data.f32(a.at(dish.wrapping_mul(16).wrapping_add(12)));
    let next_x = r.data.f32(a.at(dish.wrapping_add(1).wrapping_mul(16)));
    let ip = item_mut(r.game, gobj);
    ip.pos.x = mem(fp::fmadds(dir, reg(w), reg(pos.x)));
    ip.pos.y = mem(fp::fadds(reg(next_x), reg(pos.y)));
    ip.pos.z = pos.z;
    let p = ip.pos;
    let j = jobj_of(r.game, gobj).expect("a food without a model");
    super::super::item::set_translate(r.game, j, p);
}

/// `itFoods_Logic18_Spawned`: a random dish, its heal and model, falling.
#[doc(alias = "itFoods_Logic18_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let a = attrs(r, gobj);
    let count = r.data.u32(a) as i32;
    let dish = r.game.objs.rng.below(count);
    let entry = a.at((dish as u32).wrapping_mul(16));
    let heal = r.data.u32(entry.at(8));
    let joint = r.data.ptr(entry.at(4));
    let v = item_mut(r.game, gobj).xDD4_itemVar.foods();
    v.heal_amount.set(heal);
    v.x0.set(dish as u32);
    set_model(r, gobj, joint);
    fell(r, gobj);
}

/// Gives the item another model, `it_80273318`: the old one released, the new one made from
/// `joint`, scaled and placed at the item.
#[doc(alias = "it_80273318")]
pub fn set_model(r: &mut Running, gobj: GObjId, joint: Option<DataRef>) {
    item_mut(r.game, gobj).xC8_joint = joint;
    // `HSD_GObjObject_80390B0C`.
    if let Some((kind, obj)) = r.game.world.take_obj(gobj) {
        r.remove_obj(kind, obj);
    }
    super::super::item::make_model(r, gobj);
    super::super::item::apply_scale(r.game, gobj);
    let p = item(r.game, gobj).pos;
    let j = jobj_of(r.game, gobj).expect("an item without a model");
    super::super::item::set_translate(r.game, j, p);
}

/// Falls, `it_8028FC5C`.
#[doc(alias = "it_8028FC5C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itFoods_UnkMotion0_Anim` (1, 2, 3).
#[doc(alias = "itFoods_UnkMotion0_Anim")]
#[doc(alias = "itFoods_UnkMotion1_Anim")]
#[doc(alias = "itFoods_UnkMotion2_Anim")]
#[doc(alias = "itFoods_UnkMotion3_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itFoods_UnkMotion1_Phys` (2).
#[doc(alias = "itFoods_UnkMotion1_Phys")]
#[doc(alias = "itFoods_UnkMotion2_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Gravity, `itFoods_UnkMotion0_Phys` and `itFoods_UnkMotion3_Phys`.
#[doc(alias = "itFoods_UnkMotion0_Phys")]
#[doc(alias = "itFoods_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// Lands, `itFoods_UnkMotion0_Coll` and `itFoods_UnkMotion3_Coll`.
#[doc(alias = "itFoods_UnkMotion0_Coll")]
#[doc(alias = "itFoods_UnkMotion3_Coll")]
pub fn falling(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, landed);
    false
}

/// Lands, stopped, with a sound, `it_8028FCE8`.
#[doc(alias = "it_8028FCE8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    play_sfx(r, gobj, SFX_LAND, 127, 64);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itFoods_UnkMotion1_Coll`: falls off ledges.
#[doc(alias = "itFoods_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Shown and held, `itFoods_Logic18_PickedUp`.
#[doc(alias = "itFoods_Logic18_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    jobj::clear_flags_all(&mut r.game.objs.jobjs, child, jobj::HIDDEN);
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, it falls, `itFoods_Logic18_Dropped`.
#[doc(alias = "itFoods_Logic18_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Forgets a GObj the food points to, `itFoods_Logic18_EvtUnk` (`it_8026B894`).
// Ports `itFoods_Logic18_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
