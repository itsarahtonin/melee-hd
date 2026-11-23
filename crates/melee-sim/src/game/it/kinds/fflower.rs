//! The Fire Flower, `melee/it/kinds/itfflower.c`: a shooting item that falls, lands, loops
//! its animation and is held (its flames, `it_80292F14`, come with the fighter's shooting
//! states, not ported). The held items' callbacks ([`super::held`]), but for its animations,
//! which loop and end its flames.

use super::super::item::{change, change_state, next_attack_id};
use super::super::{IgnoreId, item, item_mut, lib26b1, lib2725};
use super::held::{self, Held};
use crate::game::ef::eflib;
use crate::game::it::ItKind;
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

/// The Fire Flower: its fuel and an attack id of its own, let go of or thrown into state 6
/// (without the drop's update), skidding in 7.
pub const FIRE_FLOWER: Held = Held {
    name: "Fire Flower",
    charges: true,
    on_spawn: Some(arm),
    fell: held::fell,
    dropped: 6,
    thrown: 6,
    drop_update: false,
    skid: 7,
    bounces_only_let_go: false,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of the Fire Flower's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        ANIM1 => anim1(r, gobj),
        ANIM2 => held_anim(r, gobj),
        ANIM7 => anim7(r, gobj),
        COLL0 => held::grounded(r, gobj, &FIRE_FLOWER),
        COLL1 | COLL6 => held::lands_loaded(r, gobj),
        COLL7 => held::skid(r, gobj, &FIRE_FLOWER),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &FIRE_FLOWER),
        REFLECTED => held::reflected(r, gobj, &FIRE_FLOWER),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &FIRE_FLOWER),
        _ => return None,
    })
}

/// Runs one of the Fire Flower's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &FIRE_FLOWER),
        FELL => held::fell(r, gobj),
        PHYS0 | PHYS2 | PHYS7 => held::idle(r, gobj),
        PHYS1 | PHYS6 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED | THROWN => held::dropped(r, gobj, &FIRE_FLOWER),
        ENTERED_AIR => held::entered_air(r, gobj, &FIRE_FLOWER),
        _ => return false,
    }
    true
}

/// Its spawn's own part, after its fuel: an attack id of its own, its flames' count and effect
/// cleared.
fn arm(r: &mut Running, gobj: GObjId) {
    let id = next_attack_id(r.game);
    let ip = item_mut(r.game, gobj);
    ip.ignore_id = IgnoreId(Some(id));
    let v = ip.vars.fflower();
    v.shots.set(0);
    v.flame_on.set(0);
}

/// The flower's flame effect ends (`efLib_DestroyAll`) unless it's shooting with fuel left
/// (state 4), the states' animation callbacks' inline.
fn end_flame(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let end = match ip.msid {
        0 | 1 | 2 | 3 | 5 | 6 | 7 => true,
        4 => ip.ammo <= 0,
        _ => false,
    };
    if end {
        eflib::destroy_all(r, gobj);
        item_mut(r.game, gobj).vars.fflower().flame_on.set(0);
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
    if item(r.game, gobj).ammo <= 0 {
        return;
    }
    if item_mut(r.game, gobj).vars.fflower().flame_on.get() == 0 {
        let t = item(r.game, gobj)
            .bone_table
            .expect("a Fire Flower without bones");
        let j = r.game.it.bone_tables[t.0].bones[4];
        crate::game::ef::spawn(r, 0x473, Some(gobj), &[crate::game::ef::Arg::JObj(j)]);
        item_mut(r.game, gobj).vars.fflower().flame_on.set(1);
    }
    let ip = item_mut(r.game, gobj);
    ip.ammo -= 1;
    let owner = ip.owner;
    let ignore = ip.ignore_id;
    let ignore = ignore.read(&mut r.game.objs.stale_reads).unwrap_or(0);
    super::lgunbeam::spawn(r, owner, pos, ignore, dir);
    let v = item_mut(r.game, gobj).vars.fflower();
    v.shots.update(|n| n.wrapping_add(1));
    v.shots.update(|n| n.wrapping_rem(6));
    if v.shots.get() == 0 {
        let id = next_attack_id(r.game);
        item_mut(r.game, gobj).ignore_id = IgnoreId(Some(id));
    }
}

/// A Fire Flower standing at `pos`, `it_80292D48`.
#[doc(alias = "it_80292D48")]
pub fn spawn_at(r: &mut Running, pos: crate::vec::Vec3) -> Option<GObjId> {
    let mut spawn = crate::game::structs::SpawnItem {
        kind: ItKind::FireFlower.id(),
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = -1.0;
    spawn.damage = 0;
    spawn.vel = crate::vec::Vec3::new(0.0, 0.0, 0.0);
    spawn.owner = None;
    spawn.maker = None;
    spawn.flags[0] |= 0x80;
    spawn.ignore_id = 0;
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    held::landed(r, gobj);
    Some(gobj)
}

/// Forgets a GObj the flower points to, `itFFlower_Logic25_EvtUnk` (`it_8026B894`).
#[doc(alias = "itFFlower_Logic25_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itFflower_UnkMotion0_Anim`: standing, its animation loops, holding still once it has.
#[doc(alias = "itFflower_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    let held = item(r.game, gobj).flags.flags.holds_still;
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        if held {
            lib2725::hold_still(r.game, gobj);
            lib26b1::hold_flags_on(r.game, gobj);
        }
    }
    false
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

/// `itFflower_UnkMotion7_Anim`: skidding, the animation loops.
#[doc(alias = "itFflower_UnkMotion7_Anim")]
pub fn anim7(r: &mut Running, gobj: GObjId) -> bool {
    end_flame(r, gobj);
    if !lib2725::is_animating(r.game, gobj) {
        change_state(r, gobj, 7, change::ANIM_UPDATE);
    }
    false
}
