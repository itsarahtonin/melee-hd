//! Link's and Young Link's bow, `melee/it/kinds/itlinkbow.c`: made in his hand when he draws,
//! its state following his (the draw's start, loop and end, on the ground and in the air), at
//! his size, gone once he lets go of the bow's states.
//!
//! Its variables (`itLinkBow_ItemVars`): his size (`x0`) and its owner (`x4`).

use super::super::item::{animate, change, change_state, destroy, pick_up, seek_anim, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1};
use crate::fpu::mem;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itLinkBow_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x802A_F298;
pub const PICKED_UP: u32 = 0x802A_F434;
pub const ANIM: u32 = 0x802A_F64C;
pub const PHYS: u32 = 0x802A_F844;
pub const COLL: u32 = 0x802A_F848;
pub const ANIM_6: u32 = 0x802A_F90C;
pub const PHYS_6: u32 = 0x802A_F914;
pub const COLL_6: u32 = 0x802A_F918;
pub const EVT_UNK: u32 = 0x802A_F920;

/// The states for each of his bow states (`it_803F6F08`), and the state of the same draw on
/// the other footing, which keeps its frame (`it_803F6F24`).
const STATES: [i32; 7] = [0, 1, 2, 3, 4, 5, 6];
const OTHER: [i32; 7] = [3, 4, 5, 0, 1, 2, 6];

/// Runs one of the bow's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        ANIM_6 | COLL_6 => gone(r, gobj),
        _ => return None,
    })
}

/// Runs one of the bow's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        PHYS | PHYS_6 => still(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itLinkBow_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.link_bow()
}

/// The bow at his size.
fn scale(r: &mut Running, gobj: GObjId) {
    let s = vars(r.game, gobj).x0.get();
    let j = jobj_of(r.game, gobj).expect("a bow without a model");
    jobj::set_scale(&mut r.game.objs.jobjs, j, Vec3::new(s, s, s));
}

/// A bow in `owner`'s hand at bone `part`, `it_802AF1A4`: of `kind`, at `pos` (on the plane)
/// facing `facing_dir`, its script's variables cleared, at his size.
#[doc(alias = "it_802AF1A4")]
pub fn spawn(
    r: &mut Running,
    facing_dir: f32,
    owner: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(owner);
    spawn.x4_parent_gobj2 = Some(owner);
    spawn.x44_flag[0] |= 0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDCC_flag.b3 = false;
    let o = item(r.game, gobj).owner.expect("a bow without its owner");
    let s = mem(crate::game::state::Running::model_scale(r.game.fighter(o)));
    let v = vars(r.game, gobj);
    v.x0.set(s);
    v.x4.set(Some(owner));
    pick_up(r, gobj, owner, part);
    Some(gobj)
}

/// `itLinkBow_Logic100_Destroyed`: its owner forgets it.
#[doc(alias = "itLinkBow_Logic100_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(o) = item(r.game, gobj).owner {
        r.link_unset_bow(o);
    }
}

/// Takes the bow away, `it_802AF304`.
#[doc(alias = "it_802AF304")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroy(r, gobj);
}

/// The owner's bow state, if the bow is still his.
fn owner_index(r: &mut Running, gobj: GObjId) -> Option<i32> {
    let owner = vars(r.game, gobj).x4.get()?;
    if item(r.game, gobj).owner != Some(owner) {
        return None;
    }
    Some(r.link_bow_index(owner))
}

/// Its state following his, `it_802AF32C`: the same draw on the other footing keeps its
/// frame.
#[doc(alias = "it_802AF32C")]
fn follow(r: &mut Running, gobj: GObjId) {
    let Some(i) = owner_index(r, gobj) else {
        return;
    };
    let msid = item(r.game, gobj).msid;
    if msid == STATES[i as usize] {
        return;
    }
    if msid == OTHER[i as usize] {
        let frame = item(r.game, gobj).x5CC_currentAnimFrame;
        change_state(r, gobj, i, change::ANIM_UPDATE);
        animate(r, gobj);
        seek_anim(r, gobj, frame);
        return;
    }
    change_state(r, gobj, i, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// `itLinkBow_Logic100_PickedUp`: the draw's start on the ground (state 0) or in the air
/// (state 3), at his size.
#[doc(alias = "itLinkBow_Logic100_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    let Some(i) = owner_index(r, gobj) else {
        return;
    };
    let msid = match i {
        0 => 0,
        3 | 6 => 3,
        _ => return,
    };
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    animate(r, gobj);
    scale(r, gobj);
}

/// `itLinkbow_UnkMotion5_Anim` (states 0 to 5): at his size; the ends go at their animation's
/// end, the others follow his state; gone once he's out of the bow's states or let go.
#[doc(alias = "itLinkbow_UnkMotion5_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    scale(r, gobj);
    let ip = item(r.game, gobj);
    match ip.msid {
        2 | 5 => {
            if ip.x5CC_currentAnimFrame == 0.0 || ip.x5CC_currentAnimFrame >= 24.0 {
                destroyed(r, gobj);
                return true;
            }
        }
        _ => follow(r, gobj),
    }
    let Some(owner) = vars(r.game, gobj).x4.get() else {
        return true;
    };
    if item(r.game, gobj).owner != Some(owner) {
        return true;
    }
    r.link_bow_released(owner)
}

/// `itLinkbow_UnkMotion5_Coll`: at his size.
#[doc(alias = "itLinkbow_UnkMotion5_Coll")]
fn coll(r: &mut Running, gobj: GObjId) -> bool {
    scale(r, gobj);
    false
}

/// `itLinkbow_UnkMotion6_Anim` and `itLinkbow_UnkMotion6_Coll`: gone.
#[doc(alias = "itLinkbow_UnkMotion6_Anim")]
#[doc(alias = "itLinkbow_UnkMotion6_Coll")]
fn gone(_: &mut Running, _: GObjId) -> bool {
    true
}

/// `itLinkbow_UnkMotion5_Phys` and `itLinkbow_UnkMotion6_Phys`: nothing moves it.
#[doc(alias = "itLinkbow_UnkMotion5_Phys")]
#[doc(alias = "itLinkbow_UnkMotion6_Phys")]
fn still(_: &mut Running, _: GObjId) {}

/// `itLinkBow_Logic100_EvtUnk`: forgets `other`.
// Ports `itLinkBow_Logic100_EvtUnk`; aliased once a check reaches it (nothing dispatches the
// items' `evt_unk` yet).
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
