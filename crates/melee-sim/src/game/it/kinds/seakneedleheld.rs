//! The needles Sheik holds while she charges them, `melee/it/kinds/itseakneedleheld.c`: a fan
//! of six needles in her hand, as many shown as she holds, gone once she lets go of them.

use gekko_fp as fp;

use super::super::item::{change, change_state, pick_up, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1};
use crate::fpu::mem;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const SHOW_NEEDLES: u32 = 0x802B_18B0;
pub const SPAWN: u32 = 0x802B_19AC;
pub const PICKED_UP: u32 = 0x802B_1AAC;
pub const ANIM: u32 = 0x802B_1AD4;
pub const PHYS: u32 = 0x802B_1C34;
pub const COLL: u32 = 0x802B_1C38;
pub const EVT_UNK: u32 = 0x802B_1C40;

/// Runs one of the needles' predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => animate(r, gobj),
        COLL => collide(r, gobj),
        _ => return None,
    })
}

/// Runs one of the needles' events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS => physics(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// The fighter holding the needles (`seakneedleheld.owner`).
fn owner_of(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .seak_needle_held()
        .owner
        .get()
}

/// Shows as many needles as the owner holds, `it_802B18B0`.
#[doc(alias = "it_802B18B0")]
pub fn show_needles(r: &mut Running, gobj: GObjId) {
    let Some(owner) = owner_of(r, gobj) else {
        return;
    };
    if item(r.game, gobj).owner != Some(owner) {
        return;
    }
    let count = match item(r.game, gobj).kind {
        super::SEAK_NEEDLE_HELD => r.seak_held_count(Some(owner)),
        k => panic!("it_802B18B0 with kind {k}: Kirby's needles aren't ported"),
    };
    let root = jobj_of(r.game, gobj);
    let jobjs = &mut r.game.objs.jobjs;
    let child = root.and_then(|j| jobjs[j].child);
    let mut needle = child.and_then(|j| jobjs[j].child);
    for i in 0..6 {
        if i < count {
            jobj::clear_flags_all(jobjs, needle, jobj::HIDDEN);
        } else {
            jobj::set_flags_all(jobjs, needle, jobj::HIDDEN);
        }
        needle = needle.and_then(|j| jobjs[j].next);
    }
}

/// Puts the needles in a fighter's hand, `it_802B19AC`: made at `pos` facing `facing_dir`,
/// then held at its part `part`.
#[doc(alias = "it_802B19AC")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition` on the plane.
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    // `Item_InitSpawnCommonFields`, with a first collision.
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    // `Item_ClearCmdVars`.
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDCC_flag.b3 = false;
    ip.xDD4_itemVar.seak_needle_held().owner.set(Some(parent));
    pick_up(r, gobj, parent, part);
    Some(gobj)
}

/// The needles' pickup callback, `itSeakNeedleHeld_Logic110_PickedUp`: their only state.
#[doc(alias = "itSeakNeedleHeld_Logic110_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The needles' animation callback, `itSeakneedleheld_UnkMotion0_Anim`: gone once the owner
/// lets go of them; else as many shown as it holds, at its size.
#[doc(alias = "itSeakneedleheld_UnkMotion0_Anim")]
pub fn animate(r: &mut Running, gobj: GObjId) -> bool {
    let Some(owner) = owner_of(r, gobj) else {
        return true;
    };
    if item(r.game, gobj).owner != Some(owner) {
        return false;
    }
    match item(r.game, gobj).kind {
        super::SEAK_NEEDLE_HELD => {
            if r.seak_lets_go(Some(owner)) {
                return true;
            }
        }
        k => panic!("itSeakneedleheld_UnkMotion0_Anim with kind {k}: Kirby's aren't ported"),
    }
    show_needles(r, gobj);
    let s = mem(fp::frsp(Running::model_scale(r.game.fighter(owner))));
    let root = jobj_of(r.game, gobj);
    if let Some(needle) = root.and_then(|j| r.game.objs.jobjs[j].child) {
        jobj::set_scale(&mut r.game.objs.jobjs, needle, Vec3::new(s, s, s));
    } else {
        panic!("jobj (the needles' model has no child)");
    }
    false
}

/// The needles' physics, `itSeakneedleheld_UnkMotion0_Phys`: none.
#[doc(alias = "itSeakneedleheld_UnkMotion0_Phys")]
pub fn physics(_r: &mut Running, _gobj: GObjId) {}

/// The needles' collision, `itSeakneedleheld_UnkMotion0_Coll`: none.
#[doc(alias = "itSeakneedleheld_UnkMotion0_Coll")]
pub fn collide(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Forgets a GObj the needles point to, `itSeakNeedleHeld_Logic110_EvtUnk` (`it_8026B894`).
#[doc(alias = "itSeakNeedleHeld_Logic110_EvtUnk")]
pub fn forget(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
