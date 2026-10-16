//! Kirby's hammer, `melee/it/kinds/itkirbyhammer.c`: the side special's hammer, made in his
//! right hand when his script says and gone when it says, when he's hit or when the move ends.

use super::super::item::{self, change, change_state, spawn_air};
use super::super::{effect, item_mut, lib26b1, lib2725};
use crate::game::ef::SpawnKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `It_Kind_Kirby_Hammer`.
pub const KIND: i32 = super::KIRBY_HAMMER;

pub const DESTROYED: u32 = 0x802A_DC04;
pub const REMOVE: u32 = 0x802A_DC34;
pub const SPAWN: u32 = 0x802A_DC54;
pub const PICKED_UP: u32 = 0x802A_DD88;
pub const EVT_UNK: u32 = 0x802A_DDB0;

/// Runs one of the hammer's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// Its end, `itKirbyHammer_Logic8_Destroyed`: Kirby no longer holds it
/// (`ftKb_SpecialAirLw_800F539C`).
#[doc(alias = "itKirbyHammer_Logic8_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(o) = super::super::item(r.game, gobj).owner {
        r.kb_forget_hammer(o);
    }
}

/// Takes it away, `it_802ADC34`.
#[doc(alias = "it_802ADC34")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    item::destroy(r, gobj);
}

/// Makes the hammer in `parent`'s hand at `pos`, `it_802ADC54`: held by `part`, on the ground
/// or in the air (`vars`, 1 for the air), with its effect.
#[doc(alias = "it_802ADC54")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    vars: u32,
    dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: KIND,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.pos = lib26b1::parent_ecb_center(r.game, parent);
    // `Item_InitSpawnCommonFields`.
    spawn.facing_dir = dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = spawn.x0_parent_gobj;
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    // `setupHammerParticles`.
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .kirby_hammer()
        .x0
        .set(vars);
    item::debug_owner_bubbles(r.game, gobj, Some(parent));
    item::pick_up(r, gobj, parent, part);
    let air = item_mut(r.game, gobj).xDD4_itemVar.kirby_hammer().x0.get() == 1;
    let id = if air { 1177 } else { 1176 };
    let j = lib2725::attach_bone(r.game, r.data, gobj);
    effect::queue(r, gobj, SpawnKind::Attach, id, j, &[]);
    Some(gobj)
}

/// `itKirbyHammer_Logic8_PickedUp`: its state, animated.
#[doc(alias = "itKirbyHammer_Logic8_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Forgets a GObj, `it_802ADDB0` (`it_8026B894`).
// Ports `it_802ADDB0`; aliased once a check reaches it.
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
