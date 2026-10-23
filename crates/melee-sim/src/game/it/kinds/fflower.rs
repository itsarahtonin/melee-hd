//! The Fire Flower, `melee/it/kinds/itfflower.c`: a shooting item that falls, lands, loops
//! its animation and is held (its flames, `it_80292F14`, come with the fighter's shooting
//! states, not ported).

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, next_attack_id};
use super::super::{IgnoreId, item, item_mut, lib26b1, lib2725};
use crate::game::ef::eflib;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8029_2E04;
pub const ANIM0: u32 = 0x8029_3040;
pub const PHYS0: u32 = 0x8029_310C;
pub const COLL0: u32 = 0x8029_3110;
pub const FELL: u32 = 0x8029_313C;
pub const ANIM1: u32 = 0x8029_3164;
pub const PHYS1: u32 = 0x8029_3210;
pub const COLL1: u32 = 0x8029_3240;
pub const PICKED_UP: u32 = 0x8029_3284;
pub const ANIM2: u32 = 0x8029_32AC;
pub const PHYS2: u32 = 0x8029_33C0;
pub const DROPPED: u32 = 0x8029_33C4;
pub const THROWN: u32 = 0x8029_33EC;
pub const PHYS6: u32 = 0x8029_3414;
pub const COLL6: u32 = 0x8029_3444;
pub const DMG_DEALT: u32 = 0x8029_3488;
pub const CLANKED: u32 = 0x8029_34AC;
pub const HIT_SHIELD: u32 = 0x8029_34D0;
pub const REFLECTED: u32 = 0x8029_34F4;
pub const SHIELD_BOUNCED: u32 = 0x8029_3514;
pub const ENTERED_AIR: u32 = 0x8029_3534;
pub const ANIM7: u32 = 0x8029_355C;
pub const PHYS7: u32 = 0x8029_3608;
pub const COLL7: u32 = 0x8029_360C;

/// Runs one of the Fire Flower's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => held_anim(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 | COLL6 => coll1(r, gobj),
        COLL7 => coll7(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => bounce(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Fire Flower's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS7 => idle(r, gobj),
        PHYS1 | PHYS6 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => dropped(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// `itFFlower_Logic25_Spawned`: its attributes' fuel, an attack id of its own, falling.
#[doc(alias = "itFFlower_Logic25_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Fire Flower without attributes");
    let fuel = r.data.u32(a) as i32;
    item_mut(r.game, gobj).xD4C = fuel;
    let id = next_attack_id(r.game);
    let ip = item_mut(r.game, gobj);
    ip.xAC4_ignoreItemID = IgnoreId(Some(id));
    let v = ip.xDD4_itemVar.fflower();
    v.x0.set(0);
    v.x4.set(0);
    fell(r, gobj);
}

/// The flower's flame effect ends (`efLib_DestroyAll`) unless it's shooting with fuel left
/// (state 4), the states' animation callbacks' inline.
fn end_flame(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let end = match ip.msid {
        0 | 1 | 2 | 3 | 5 | 6 | 7 => true,
        4 => ip.xD4C <= 0,
        _ => false,
    };
    if end {
        eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj).xDD4_itemVar.fflower().x4.set(0);
    }
}

/// From the fighter's shooting: a shot starts, `it_80292E64`.
#[doc(alias = "it_80292E64")]
pub fn start_shooting(r: &mut Running, gobj: GObjId) {
    if matches!(item(r.game, gobj).msid, 2 | 5) {
        change_state(r, gobj, 3, change::ANIM_UPDATE);
    }
}

/// From the fighter's shooting: the shots stop, `it_80292EAC`.
#[doc(alias = "it_80292EAC")]
pub fn stop_shooting(r: &mut Running, gobj: GObjId) {
    if matches!(item(r.game, gobj).msid, 3 | 4) {
        change_state(r, gobj, 5, change::ANIM_UPDATE);
    }
}

/// The nozzle, in the hand's space, `it_80292EF8`.
#[doc(alias = "it_80292EF8")]
pub fn muzzle() -> crate::vec::Vec3 {
    crate::vec::Vec3::new(0.0, 5.5, 2.5)
}

/// A flame from the nozzle at `pos` the way `dir` faces while fuel lasts, `it_80292F14`:
/// the flower's flame effect on its fourth bone once, a unit of fuel, and a fresh attack id
/// for the flames every sixth.
#[doc(alias = "it_80292F14")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: crate::vec::Vec3, dir: f32) {
    if item(r.game, gobj).xD4C <= 0 {
        return;
    }
    if item_mut(r.game, gobj).xDD4_itemVar.fflower().x4.get() == 0 {
        let t = item(r.game, gobj)
            .xBBC_dynamicBoneTable
            .expect("a Fire Flower without bones");
        let j = r.game.it.bone_tables[t.0].bones[4];
        crate::game::ef::spawn(r, 0x473, Some(gobj), &[crate::game::ef::Arg::JObj(j)]);
        item_mut(r.game, gobj).xDD4_itemVar.fflower().x4.set(1);
    }
    let ip = item_mut(r.game, gobj);
    ip.xD4C -= 1;
    let owner = ip.owner;
    let ignore = ip.xAC4_ignoreItemID;
    let ignore = ignore.read(&mut r.game.objs.stale_reads).unwrap_or(0);
    super::lgunbeam::spawn(r, owner, pos, ignore, dir);
    let v = item_mut(r.game, gobj).xDD4_itemVar.fflower();
    v.x0.update(|n| n.wrapping_add(1));
    v.x0.update(|n| n.wrapping_rem(6));
    if v.x0.get() == 0 {
        let id = next_attack_id(r.game);
        item_mut(r.game, gobj).xAC4_ignoreItemID = IgnoreId(Some(id));
    }
}

/// A Fire Flower standing at `pos`, `it_80292D48`.
#[doc(alias = "it_80292D48")]
pub fn spawn_at(r: &mut Running, pos: crate::vec::Vec3) -> Option<GObjId> {
    let mut spawn = crate::game::structs::SpawnItem {
        kind: super::F_FLOWER,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = -1.0;
    spawn.x3C_damage = 0;
    spawn.vel = crate::vec::Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = None;
    spawn.x4_parent_gobj2 = None;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    landed(r, gobj);
    Some(gobj)
}

/// Forgets a GObj the flower points to, `itFFlower_Logic25_EvtUnk` (`it_8026B894`).
// Ports `itFFlower_Logic25_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// Stops and stands, `it_80292FF0`.
#[doc(alias = "it_80292FF0")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib2725::clear_velocity(r.game, gobj);
    lib26b1::enable_pickup(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// `itFflower_UnkMotion0_Anim`: standing, its animation loops, holding still once it has.
#[doc(alias = "itFflower_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    let held = item(r.game, gobj).xDC8_word.flags.x2;
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        if held {
            lib2725::hold_still(r.game, gobj);
            lib26b1::hold_flags_on(r.game, gobj);
        }
    }
    false
}

/// The states that move nothing, `itFflower_UnkMotion0_Phys` (5, 7).
#[doc(alias = "itFflower_UnkMotion0_Phys")]
#[doc(alias = "itFflower_UnkMotion5_Phys")]
#[doc(alias = "itFflower_UnkMotion7_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itFflower_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itFflower_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029313C`.
#[doc(alias = "it_8029313C")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itFflower_UnkMotion6_Anim` (states 1 and 6): falling, the animation loops in state 1.
#[doc(alias = "itFflower_UnkMotion6_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 1, change::ANIM_UPDATE);
    }
    false
}

/// Gravity, `itFflower_UnkMotion1_Phys` and `itFflower_UnkMotion6_Phys`.
#[doc(alias = "itFflower_UnkMotion1_Phys")]
#[doc(alias = "itFflower_UnkMotion6_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itFflower_UnkMotion1_Coll` and `itFflower_UnkMotion6_Coll`: with fuel left it lands;
/// empty, it falls through, gone once it's past the stage.
#[doc(alias = "itFflower_UnkMotion1_Coll")]
#[doc(alias = "itFflower_UnkMotion6_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Held, `itFFlower_Logic25_PickedUp`.
#[doc(alias = "itFFlower_Logic25_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// `itFflower_UnkMotion5_Anim` (states 2 to 5): each loops into the held state, or
/// shooting's into shooting.
#[doc(alias = "itFflower_UnkMotion5_Anim")]
pub fn held_anim(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    let next = match item(r.game, gobj).msid {
        2 | 5 => 2,
        3 | 4 => 4,
        _ => return false,
    };
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, next, change::ANIM_UPDATE);
    }
    false
}

/// Let go of or thrown, it falls, `itFFlower_Logic25_Dropped` and `itFFlower_Logic25_Thrown`.
#[doc(alias = "itFFlower_Logic25_Dropped")]
#[doc(alias = "itFFlower_Logic25_Thrown")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Bounces off what it hit, a clank or a shield, `itFFlower_Logic25_DmgDealt` (and
/// `Clanked`, `HitShield`).
#[doc(alias = "itFFlower_Logic25_DmgDealt")]
#[doc(alias = "itFFlower_Logic25_Clanked")]
#[doc(alias = "itFFlower_Logic25_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Sent back by a reflection, `itFFlower_Logic25_Reflected`.
#[doc(alias = "itFFlower_Logic25_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itFFlower_Logic25_ShieldBounced`.
#[doc(alias = "itFFlower_Logic25_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, `itFFlower_Logic25_EnteredAir`.
#[doc(alias = "itFFlower_Logic25_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE);
}

/// `itFflower_UnkMotion7_Anim`: skidding, the animation loops.
#[doc(alias = "itFflower_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 7, change::ANIM_UPDATE);
    }
    false
}

/// `itFflower_UnkMotion7_Coll`: skidding, it stops or falls.
#[doc(alias = "itFflower_UnkMotion7_Coll")]
pub fn coll7(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}
