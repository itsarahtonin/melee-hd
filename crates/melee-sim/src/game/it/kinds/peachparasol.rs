//! Peach's parasol, `melee/it/kinds/itpeachparasol.c`: made in her hand by her up special,
//! opened and closed by the common parasol states (`ftCommon_8007E83C`), and gone once she
//! leaves the move, putting back the item she held.

use super::super::item::{animate, change, change_state, debug_owner_bubbles, destroy, spawn_air};
use super::super::{item, item_mut, lib26b1};
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802B_DB4C;
pub const HITLAG_START: u32 = 0x802B_DBF8;
pub const HITLAG_END: u32 = 0x802B_DC18;
pub const PICKED_UP: u32 = 0x802B_DC74;
pub const MOTION2_ANIM: u32 = 0x802B_DCC0;
pub const ACCESSORY: u32 = 0x802B_DD3C;
pub const EVT_UNK: u32 = 0x802B_DDF8;

/// `It_Kind_Peach_Parasol`.
pub const KIND: i32 = 103;

/// Runs one of the parasol's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION2_ANIM => animated(r, gobj),
        _ => return None,
    })
}

/// Runs one of the parasol's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        HITLAG_START => hitlag_start(r, gobj),
        HITLAG_END => hitlag_end(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        ACCESSORY => {}
        _ => return false,
    }
    true
}

/// The parasol's opening animation's frame count for its state `i` (1 or 2), `it_802BDA40`:
/// 15 for the first animation, 16 for the second.
#[doc(alias = "it_802BDA40")]
pub fn anim_frames(i: i32) -> i32 {
    // `it_803F74F8[i].anim_id`: -1, 0, 1.
    match i {
        1 => 15,
        2 => 16,
        _ => panic!("the parasol's state {i} has no animation"),
    }
}

/// Makes the parasol for `parent` at `pos`, in its hand at bone `part`, `it_802BDA64`.
#[doc(alias = "it_802BDA64")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: KIND,
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

/// `itPeachParasol_Logic60_Destroyed`: its owner puts it away, and forgets it when that gave
/// back a held item.
#[doc(alias = "itPeachParasol_Logic60_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item(r.game, gobj).owner
        && r.peach_parasol_gone(owner)
    {
        item_mut(r.game, gobj).owner = None;
    }
}

/// Takes the parasol away, `it_802BDB94`: its owner puts it away, and it goes.
#[doc(alias = "it_802BDB94")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroyed(r, gobj);
    destroy(r, gobj);
}

/// The parasol pauses in its owner's hitlag, `it_802BDBF8`.
#[doc(alias = "it_802BDBF8")]
pub fn hitlag_start(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_start(r, gobj);
}

/// The parasol goes on after hitlag, `it_802BDC18`.
#[doc(alias = "it_802BDC18")]
pub fn hitlag_end(r: &mut Running, gobj: GObjId) {
    super::seakchain::hitlag_end(r, gobj);
}

/// Whether the parasol has finished opening, `it_802BDC38`: in its open state (2), 16 frames
/// in.
#[doc(alias = "it_802BDC38")]
pub fn is_open(r: &Running, gobj: GObjId) -> bool {
    let ip = item(r.game, gobj);
    ip.msid == 2 && ip.x5CC_currentAnimFrame >= 16.0
}

/// `itPeachParasol_Logic60_PickedUp`: opened at once, animated, at normal speed.
#[doc(alias = "itPeachParasol_Logic60_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    open(r, gobj, 16.0);
    animate(r, gobj);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
}

/// `itPeachparasol_UnkMotion2_Anim`: gone once its owner (if any) leaves the up special and
/// isn't holding it open.
#[doc(alias = "itPeachparasol_UnkMotion2_Anim")]
fn animated(r: &mut Running, gobj: GObjId) -> bool {
    let gone = match item(r.game, gobj).owner {
        Some(owner) => r.peach_special_hi_not_active(owner),
        None => true,
    };
    if gone {
        destroyed(r, gobj);
        return true;
    }
    false
}

/// Opens the parasol (state 1) at `speed`, with its sound, `it_802BDD40`.
#[doc(alias = "it_802BDD40")]
pub fn opening(r: &mut Running, gobj: GObjId, speed: f32) {
    super::super::item::play_sfx(r, gobj, 0xF7, 0x7F, 0x40);
    item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
}

/// Holds the parasol open (state 2) at `speed`, `it_802BDDB4`.
#[doc(alias = "it_802BDDB4")]
#[doc(alias = "it_802BDD3C")]
pub fn open(r: &mut Running, gobj: GObjId, speed: f32) {
    item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ACCESSORY));
}

/// `itPeachParasol_Logic60_EvtUnk`: forgets `other`.
#[doc(alias = "itPeachParasol_Logic60_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
