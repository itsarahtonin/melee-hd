//! The flower the Lip's Stick's spores plant on a fighter's head, `melee/it/kinds/itfflowerflame.c`
//! (kind 41, `It_Kind_F_Flower_Flame`): held at the head while it lasts (state 2), its petals
//! (bone 1) sized and its stem (bone 2) set off as the fighter's damage grows it
//! (`ftCommon_8007FDA0`); dropped, it falls and lies until its life runs out.

use super::super::ground::{air, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, set_translate, spawn_air};
use super::super::{item, item_mut, lib26b1, lib2725};
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itFFlowerFlame_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;
use gekko_fp as fp;

use crate::fpu::{mem, reg};

pub const SPAWNED: u32 = 0x8029_A9FC;
pub const LANDED: u32 = 0x8029_AA1C;
pub const ANIM0: u32 = 0x8029_AA6C;
pub const PHYS0: u32 = 0x8029_AB60;
pub const COLL0: u32 = 0x8029_AB64;
pub const FELL: u32 = 0x8029_AB90;
pub const ANIM1: u32 = 0x8029_ABCC;
pub const PHYS1: u32 = 0x8029_ACCC;
pub const COLL1: u32 = 0x8029_ACFC;
pub const PICKED_UP: u32 = 0x8029_AD1C;
pub const ANIM2: u32 = 0x8029_AD58;
pub const DROPPED: u32 = 0x8029_AD94;
pub const ANIM3: u32 = 0x8029_ADD0;
pub const PHYS3: u32 = 0x8029_AED0;
pub const COLL3: u32 = 0x8029_AF00;
pub const ENTERED_AIR: u32 = 0x8029_AF34;
pub const ANIM4: u32 = 0x8029_AF70;
pub const PHYS4: u32 = 0x8029_B070;
pub const COLL4: u32 = 0x8029_B074;

/// Runs one of the flower's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 | ANIM3 | ANIM4 => anim_life(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        _ => return None,
    })
}

/// Runs one of the flower's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 | PHYS4 => idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS1 | PHYS3 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itFFlowerFlame_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.fflower_flame()
}

/// Grows a flower on `owner`'s `part`, facing `facing`, `it_8029A748`: its life the
/// attributes' first float, held there, and its stem's rest position noted.
#[doc(alias = "it_8029A748")]
pub fn grow(r: &mut Running, owner: GObjId, pos: Vec3, part: i32, facing: f32) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::F_FLOWER_FLAME,
        ..Default::default()
    };
    // `Item_InitSpawnPositionFromParent`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = lib26b1::parent_ecb_center(r.game, owner);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = facing;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a flower without attributes");
    let life = r.data.f32(a);
    lib2725::set_lifetime(r.game, r.data, gobj, life);
    super::super::item::pick_up(r, gobj, owner, part);
    lib26b1::disable_pickup(r.game, gobj);
    let stem = lib2725::bone(r.game, r.data, gobj, 2).expect("a flower without a stem");
    let t = r.game.objs.jobjs[stem].translate;
    vars(r.game, gobj).pos.set(t);
    Some(gobj)
}

/// Sizes the petals, `it_8029A89C`.
#[doc(alias = "it_8029A89C")]
pub fn set_size(r: &mut Running, gobj: GObjId, size: f32) {
    let petals = lib2725::bone(r.game, r.data, gobj, 1);
    vars(r.game, gobj).unk.set(size);
    let size = vars(r.game, gobj).unk.get();
    lib2725::set_uniform_scale(r.game, petals.expect("a flower without petals"), size);
}

/// Sets the stem off its rest position by `offset`, `it_8029A8F4`.
#[doc(alias = "it_8029A8F4")]
pub fn set_offset(r: &mut Running, gobj: GObjId, offset: Vec3) {
    let stem = lib2725::bone(r.game, r.data, gobj, 2).expect("a flower without a stem");
    let p = vars(r.game, gobj).pos.get();
    let t = Vec3::new(
        mem(fp::fadds(reg(p.x), reg(offset.x))),
        mem(fp::fadds(reg(p.y), reg(offset.y))),
        mem(fp::fadds(reg(p.z), reg(offset.z))),
    );
    set_translate(r.game, stem, t);
}

/// `itFFlowerFlame_Logic41_Spawned`: not to be picked up.
#[doc(alias = "itFFlowerFlame_Logic41_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
}

/// Lands and lies still, `it_8029AA1C`.
#[doc(alias = "it_8029AA1C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x40_vel = Vec3::new(0.0, 0.0, 0.0);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// The petals' size and the stem's place kept each frame.
fn keep_shape(r: &mut Running, gobj: GObjId) {
    let size = vars(r.game, gobj).unk.get();
    let petals = lib2725::bone(r.game, r.data, gobj, 1).expect("a flower without petals");
    lib2725::set_uniform_scale(r.game, petals, size);
    let stem = lib2725::bone(r.game, r.data, gobj, 2).expect("a flower without a stem");
    let p = vars(r.game, gobj).pos.get();
    set_translate(r.game, stem, p);
}

/// `itFflowerflame_UnkMotion0_Anim`: lying, its shape kept.
#[doc(alias = "itFflowerflame_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    keep_shape(r, gobj);
    false
}

/// `itFflowerflame_UnkMotion1_Anim` (and 3, 4): its shape kept, gone once its life runs out.
#[doc(alias = "itFflowerflame_UnkMotion1_Anim")]
#[doc(alias = "itFflowerflame_UnkMotion3_Anim")]
#[doc(alias = "itFflowerflame_UnkMotion4_Anim")]
pub fn anim_life(r: &mut Running, gobj: GObjId) -> bool {
    keep_shape(r, gobj);
    lib2725::tick_lifetime(r.game, gobj)
}

/// `itFflowerflame_UnkMotion2_Anim`: on the head, the petals' size kept.
#[doc(alias = "itFflowerflame_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    let size = vars(r.game, gobj).unk.get();
    let petals = lib2725::bone(r.game, r.data, gobj, 1).expect("a flower without petals");
    lib2725::set_uniform_scale(r.game, petals, size);
    false
}

/// The states that move nothing, `itFflowerflame_UnkMotion0_Phys` (and 4).
#[doc(alias = "itFflowerflame_UnkMotion0_Phys")]
#[doc(alias = "itFflowerflame_UnkMotion4_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itFflowerflame_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itFflowerflame_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029AB90`.
#[doc(alias = "it_8029AB90")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itFflowerflame_UnkMotion1_Phys` (and 3): gravity.
#[doc(alias = "itFflowerflame_UnkMotion1_Phys")]
#[doc(alias = "itFflowerflame_UnkMotion3_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itFflowerflame_UnkMotion1_Coll`: falling, gone once it's past the stage.
#[doc(alias = "itFflowerflame_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// On the head, `itFFlowerFlame_Logic41_PickedUp`.
#[doc(alias = "itFFlowerFlame_Logic41_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// Off the head, it falls, `itFFlowerFlame_Logic41_Dropped`.
#[doc(alias = "itFFlowerFlame_Logic41_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE | change::DROP_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itFflowerflame_UnkMotion3_Coll`: gone once it touches the floor.
#[doc(alias = "itFflowerflame_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    air(r, gobj)
}

/// Skidding off a slope, `itFFlowerFlame_Logic41_EnteredAir`.
#[doc(alias = "itFFlowerFlame_Logic41_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    lib26b1::disable_pickup(r.game, gobj);
}

/// `itFflowerflame_UnkMotion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itFflowerflame_UnkMotion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}
