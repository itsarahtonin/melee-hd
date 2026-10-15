//! Young Link's milk, `melee/it/kinds/itclinkmilk.c`: the bottle his side taunt makes in his
//! hand, shown while the taunt's script says, at his size, gone once he leaves the taunt or
//! lets go of it.
//!
//! Its variables (`itCLinkMilk_ItemVars`): its owner (`x0`).

use super::super::item::{animate, change, change_state, destroy, pick_up, spawn_air};
use super::super::{item, item_mut, jobj_of, lib26b1};
use crate::fpu::mem;
use crate::game::state::{Game, Running};
use crate::game::structs::{SpawnItem, itCLinkMilk_ItemVars};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

pub const PICKED_UP: u32 = 0x802C_8C74;
pub const ANIM: u32 = 0x802C_8CDC;
pub const PHYS: u32 = 0x802C_8F20;
pub const COLL: u32 = 0x802C_8F24;
pub const EVT_UNK: u32 = 0x802C_8F2C;

/// `JOBJ_HIDDEN`.
const HIDDEN: u32 = 1 << 4;

/// Runs one of the milk's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM => anim(r, gobj),
        COLL => coll(r, gobj),
        _ => return None,
    })
}

/// Runs one of the milk's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        PICKED_UP => picked_up(r, gobj),
        PHYS => phys(r, gobj),
        _ => return false,
    }
    true
}

fn vars(game: &mut Game, gobj: GObjId) -> &mut itCLinkMilk_ItemVars {
    item_mut(game, gobj).xDD4_itemVar.clink_milk()
}

/// The milk in `parent`'s hand at bone `bone`, `it_802C8B28`: at `pos` (on the plane) facing
/// `facing_dir`, its script's variables cleared, `parent` its owner.
#[doc(alias = "it_802C8B28")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    bone: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    let mut spawn = SpawnItem {
        kind: super::CLINK_MILK,
        ..Default::default()
    };
    spawn.prev_pos = pos;
    spawn.prev_pos.z = 0.0;
    spawn.pos = spawn.prev_pos;
    spawn.facing_dir = facing_dir;
    spawn.x3C_damage = 0;
    spawn.vel = Vec3::new(0.0, 0.0, 0.0);
    spawn.x0_parent_gobj = Some(parent);
    spawn.x4_parent_gobj2 = Some(parent);
    spawn.x44_flag[0] &= !0x80;
    spawn.x40 = 0;
    let gobj = spawn_air(r, &mut spawn)?;
    let ip = item_mut(r.game, gobj);
    ip.xDB8_itcmd_var3 = 0;
    ip.xDB4_itcmd_var2 = 0;
    ip.xDB0_itcmd_var1 = 0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDCC_flag.b3 = false;
    vars(r.game, gobj).x0.set(Some(parent));
    pick_up(r, gobj, parent, bone);
    Some(gobj)
}

/// Takes the milk away, `it_802C8C34`: no owner, destroyed.
#[doc(alias = "it_802C8C34")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    vars(r.game, gobj).x0.set(None);
    item_mut(r.game, gobj).owner = None;
    destroy(r, gobj);
}

/// `it_2725_Logic80_PickedUp`: the state facing its way (0 right, 1 left), animated.
#[doc(alias = "it_2725_Logic80_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    let msid = if item(r.game, gobj).facing_dir == 1.0 {
        0
    } else {
        1
    };
    change_state(r, gobj, msid, change::ANIM_UPDATE);
    animate(r, gobj);
}

/// The milk lets go of its owner (`itCLinkMilk_NotifyParent`): the owner forgets it if it
/// still holds it (`ftCl_Init_801492C4`).
fn notify_parent(r: &mut Running, gobj: GObjId) {
    let parent = vars(r.game, gobj).x0.get();
    if let Some(p) = parent
        && item(r.game, gobj).owner == Some(p)
    {
        r.clink_forget_milk(p);
    }
    vars(r.game, gobj).x0.set(None);
    item_mut(r.game, gobj).owner = None;
}

/// The end once let go (`itCLinkMilk_Destroy`): no owner.
fn release(r: &mut Running, gobj: GObjId) -> bool {
    notify_parent(r, gobj);
    vars(r.game, gobj).x0.set(None);
    item_mut(r.game, gobj).owner = None;
    true
}

/// `itClinkmilk_UnkMotion1_Anim` (both states): its first child shown while the taunt's
/// script says; it ends once its owner leaves the taunt or lets go of it
/// (`ftCl_Init_8014920C`), else takes the script's word (`ftCl_Init_801492F4`) and its owner's
/// size.
#[doc(alias = "itClinkmilk_UnkMotion1_Anim")]
fn anim(r: &mut Running, gobj: GObjId) -> bool {
    let model = jobj_of(r.game, gobj);
    let child = model.and_then(|j| r.game.objs.jobjs[j].child);
    if item(r.game, gobj).xDAC_itcmd_var0 == 1 {
        jobj::clear_flags(&mut r.game.objs.jobjs, child, HIDDEN);
    } else {
        jobj::set_flags(&mut r.game.objs.jobjs, child, HIDDEN);
    }
    let Some(owner) = vars(r.game, gobj).x0.get() else {
        return release(r, gobj);
    };
    if r.clink_milk_done(owner) {
        return release(r, gobj);
    }
    let word = r.clink_appeal_cmd_var1(owner);
    item_mut(r.game, gobj).xDAC_itcmd_var0 = word;
    let s = mem(crate::game::state::Running::model_scale(
        r.game.fighter(owner),
    ));
    let child = child.expect("HSD_JObjSetScale on no joint");
    jobj::set_scale(&mut r.game.objs.jobjs, child, Vec3::new(s, s, s));
    false
}

/// `itClinkmilk_UnkMotion1_Phys`: nothing moves it.
#[doc(alias = "itClinkmilk_UnkMotion1_Phys")]
fn phys(_: &mut Running, _: GObjId) {}

/// `itClinkmilk_UnkMotion1_Coll`: nothing ends it.
#[doc(alias = "itClinkmilk_UnkMotion1_Coll")]
fn coll(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itCLinkMilk_Logic80_EvtUnk`: forgets `other` (`it_8026B894`).
#[doc(alias = "itCLinkMilk_Logic80_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}
