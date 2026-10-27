//! Ness's bat, `melee/it/kinds/itnessbat.c`: held in his hand through the forward smash, gone
//! once he leaves it or loses it. (State 1, its copy in Kirby's hand, isn't ported.)

use super::super::item::{change, change_state, destroy, pick_up, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SPAWN: u32 = 0x802A_D478;
pub const REMOVE: u32 = 0x802A_D6B8;
pub const PICKED_UP: u32 = 0x802A_D6F0;
pub const ANIM_0: u32 = 0x802A_D768;
pub const ANIM_1: u32 = 0x802A_D8DC;
#[doc(alias = "itNessbat_UnkMotion1_Phys")]
pub const PHYS_1: u32 = 0x802A_D9F0;
#[doc(alias = "itNessbat_UnkMotion1_Coll")]
pub const COLL_1: u32 = 0x802A_D9F4;
#[doc(alias = "itNessbat_UnkMotion0_Phys")]
pub const PHYS_0: u32 = 0x802A_D8D0;
#[doc(alias = "itNessbat_UnkMotion0_Coll")]
pub const COLL_0: u32 = 0x802A_D8D4;
pub const EVT_UNK: u32 = 0x802A_D9FC;

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = 1 << 4;

/// Runs one of the bat's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => animate_0(r, gobj),
        COLL_0 | COLL_1 => false,
        ANIM_1 => animate_1(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bat's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 | PHYS_1 => {}
        _ => return false,
    }
    true
}

/// The bat in `ness`'s hand at `part`, `it_802AD478`.
#[doc(alias = "it_802AD478")]
pub fn spawn(
    r: &mut Running,
    ness: Option<GObjId>,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let ness = ness?;
    let mut spawn = SpawnItem {
        kind: super::NESS_BAT,
        ..Default::default()
    };
    // `Item_InitSpawnPosition(&spawn, pos, true)`.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields(&spawn, gobj, facing_dir, false)`.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(ness);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let bat = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, bat);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDCC_flag.b3 = false;
    ip.xDD4_itemVar.ness_bat().x0.set(Some(ness));
    lib2725::set_lifetime(r.game, r.data, bat, 1200.0);
    pick_up(r, bat, ness, part);
    Some(bat)
}

/// The bat in a posing Ness's hand at `part`, `it_802AD590`: made at his place and facing,
/// its script's variables 1, held.
#[doc(alias = "it_802AD590")]
pub fn spawn_demo(r: &mut Running, ness: GObjId, part: i32) -> Option<GObjId> {
    let fp = r.game.fighter_mut(ness);
    let (pos, facing_dir) = (fp.cur_pos, fp.facing_dir);
    let mut spawn = SpawnItem {
        kind: super::NESS_BAT,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(ness);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let bat = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, bat);
    ip.xDB8_itcmd_var3 = 1;
    ip.xDB4_itcmd_var2 = 1;
    ip.xDB0_itcmd_var1 = 1;
    ip.xDAC_itcmd_var0 = 1;
    ip.xDCC_flag.b3 = false;
    ip.xDD4_itemVar.ness_bat().x0.set(Some(ness));
    pick_up(r, bat, ness, part);
    Some(bat)
}

/// The bat in a posing Ness's hand matches his size while he's frozen in a KO toward the
/// screen or asleep (`ftCo_800BF228`), `itNessbat_UnkMotion1_Anim`.
#[doc(alias = "itNessbat_UnkMotion1_Anim")]
pub fn animate_1(r: &mut Running, gobj: GObjId) -> bool {
    let ness = item_mut(r.game, gobj).xDD4_itemVar.ness_bat().x0.get();
    if let Some(n) = ness
        && crate::game::ft::demo::frozen_or_asleep(r.game, n)
    {
        lib2725::copy_owner_scale(r.game, gobj, n);
    }
    false
}

/// The bat gone, `it_802AD6B8`.
#[doc(alias = "it_802AD6B8")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.ness_bat().x0.set(None);
    ip.owner = None;
    destroy(r, gobj);
}

/// Taken in hand, `it_2725_Logic58_PickedUp`: state 1 if any script variable is set (Kirby's),
/// else 0, and animated.
#[doc(alias = "it_2725_Logic58_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let any = ip.xDAC_itcmd_var0 | ip.xDB0_itcmd_var1 | ip.xDB4_itcmd_var2 | ip.xDB8_itcmd_var3;
    change_state(r, gobj, if any != 0 { 1 } else { 0 }, change::ANIM_UPDATE);
    super::super::item::animate(r, gobj);
}

/// `itNessbat_UnkMotion0_Anim`: shown unless its first script variable is set; it goes once
/// Ness leaves the forward smash or loses it (`ftNs_AttackS4_CheckNessBatRemove`), telling him.
#[doc(alias = "itNessbat_UnkMotion0_Anim")]
pub fn animate_0(r: &mut Running, gobj: GObjId) -> bool {
    let child = jobj_of(r.game, gobj).and_then(|j| r.game.objs.jobjs[j].child);
    if item(r.game, gobj).xDAC_itcmd_var0 == 0 {
        jobj::clear_flags_all(&mut r.game.objs.jobjs, child, HIDDEN);
    } else {
        jobj::set_flags_all(&mut r.game.objs.jobjs, child, HIDDEN);
    }
    let ness = item_mut(r.game, gobj).xDD4_itemVar.ness_bat().x0.get();
    if let Some(n) = ness
        && !r.ness_bat_should_go(n)
    {
        return false;
    }
    // `itNessbat_ClearOwnerRef`.
    let ip = item_mut(r.game, gobj);
    let owner = ip.owner;
    if let Some(n) = ip.xDD4_itemVar.ness_bat().x0.get()
        && owner == Some(n)
    {
        r.ness_bat_set_null(n);
    }
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.ness_bat().x0.set(None);
    ip.owner = None;
    // `itNessbat_RemoveItem`.
    let ip = item_mut(r.game, gobj);
    ip.xDD4_itemVar.ness_bat().x0.set(None);
    ip.owner = None;
    true
}

/// Forgets a GObj the bat points to, `itNessBat_Logic58_EvtUnk` (`it_8026B894`).
// Ports `itNessBat_Logic58_EvtUnk`; aliased once a check reaches it.
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
