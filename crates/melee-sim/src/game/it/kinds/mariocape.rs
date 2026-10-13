//! Mario's cape and Dr. Mario's sheet, `melee/it/kinds/itmariocape.c`: held in the thumb
//! through the swing, flashing when its script says, gone once the owner leaves the swing.

use super::super::effect::queue;
use super::super::item::{animate, change, change_state, debug_owner_bubbles, destroy, spawn_air};
use super::super::{item, item_mut, lib2725};
use crate::game::ef::SpawnKind;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802B_2644;
pub const PICKED_UP: u32 = 0x802B_2700;
pub const MOTION1_ANIM: u32 = 0x802B_2788;
pub const EVT_UNK: u32 = 0x802B_2870;

/// Runs one of the cape's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION1_ANIM => animate_cape(r, gobj),
        _ => return None,
    })
}

/// Runs one of the cape's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        _ => return false,
    }
    true
}

/// Pulls a cape of `kind` for `parent` at `pos`, in its hand at bone `part`, `it_802B2560`.
#[doc(alias = "it_802B2560")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    facing_dir: f32,
    pos: Vec3,
    part: i32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    // `Item_InitSpawnPosition`, keeping its depth.
    spawn.prev_pos = pos;
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
    // `Item_AttachToParent`.
    super::super::item::pick_up(r, gobj, parent, part);
    debug_owner_bubbles(r.game, gobj, Some(parent));
    Some(gobj)
}

/// `itMarioCape_Logic41_Destroyed`: the owner lets go of it.
#[doc(alias = "itMarioCape_Logic41_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.mr_special_s_reset(owner);
    }
}

/// Takes the cape away, `it_802B2674`: the owner lets go of it, and it goes.
#[doc(alias = "it_802B2674")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner {
        r.mr_special_s_reset(owner);
    }
    destroy(r, gobj);
}

/// The cape pauses in its owner's hitlag, `it_802B26C0`.
#[doc(alias = "it_802B26C0")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// The cape goes on after hitlag, `it_802B26E0`.
#[doc(alias = "it_802B26E0")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// `it_2725_Logic41_PickedUp`: its script's flags cleared, its state, its animation, sized as
/// its owner. The state it picks reads `ftLib_GetGroundAir` on the cape's own GObj: the word at
/// the fighter's `ground_or_air` offset (`+0xE0`) is, in an item, its first dynamic bone set's
/// count, which the cape (with no dynamic bones) leaves as its block held it (docs/engine.md).
#[doc(alias = "it_2725_Logic41_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    if ip.owner.is_none() {
        return;
    }
    let word = ip.xD4_dynamicBones.0[0].dyn_desc.count as i32;
    let state = if word != 1 { 0 } else { 1 };
    change_state(r, gobj, state, change::ANIM_UPDATE);
    animate(r, gobj);
    // `it_80274574`.
    lib2725::scale_by_owner(r, gobj);
}

/// `itMariocape_UnkMotion1_Anim`: its two flashes when its script says (on its bones 16 and
/// 6); gone once the owner has left the swing (or there's no owner), letting go of it.
#[doc(alias = "itMariocape_UnkMotion1_Anim")]
fn animate_cape(r: &mut Running, gobj: GObjId) -> bool {
    let bone = |r: &Running, i: usize| {
        let t = item(r.game, gobj)
            .xBBC_dynamicBoneTable
            .expect("a cape without a bone table");
        r.game.it.bone_tables[t.0].bones[i]
    };
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        let j = bone(r, 16);
        queue(r, gobj, SpawnKind::Attach, 0x47D, j, &[]);
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
        let j = bone(r, 6);
        queue(r, gobj, SpawnKind::Attach, 0x47E, j, &[]);
    }
    let removed = match item(r.game, gobj).owner {
        Some(owner) => r.mr_special_s_check_item_cape_remove(owner),
        None => true,
    };
    if removed {
        if let Some(owner) = item(r.game, gobj).owner {
            r.mr_special_s_reset(owner);
        }
        return true;
    }
    false
}
