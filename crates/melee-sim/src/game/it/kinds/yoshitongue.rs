//! An item on Yoshi's tongue, `melee/it/kinds/ityoshitongue.c`: the egg lay's tongue catches
//! an item (any kind), which follows the tongue's tip in a state of its own until Yoshi swallows
//! it, making an egg of it, or lets it go. Kirby's copy of Yoshi has its own copy of it
//! (`melee/it/kinds/itkirbyyoshispecialn.c`), the same but for its state table.

use super::super::item::{article, change_state, destroy};
use super::super::{item_mut, lib2725, maplib, zako};
use crate::game::handles::ItemStatesRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const ANIM: u32 = 0x802F_2CCC;
pub const PHYS: u32 = 0x802F_2CD4;
pub const COLL: u32 = 0x802F_2CD8;
/// Kirby's copy's item's state callbacks.
pub const KB_ANIM: u32 = 0x802F_2E68;
pub const KB_PHYS: u32 = 0x802F_2E70;
pub const KB_COLL: u32 = 0x802F_2E74;

/// `it_803F9480`, the tongue's own state table, and Kirby's copy's (`it_803F9490`).
const STATES: u32 = 0x803F_9480;
const KB_STATES: u32 = 0x803F_9490;

/// The joint of a Yoshi's tongue's tip (`ftYs_SpecialN_GetBoneIndex`, his `FtPart_TransN2`;
/// Kirby's copy's `ftKb_SpecialNYs_80109238` is the same for him).
fn tip(r: &Running, yoshi: GObjId) -> i32 {
    let fp = r.game.fighter(yoshi);
    r.bone_index(fp, crate::game::ft::parts::TRANS_N2)
}

/// Runs one of the caught item's predicates, if `addr` is one.
pub fn run_pred(_r: &mut Running, _gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM | KB_ANIM => anim(),
        COLL | KB_COLL => coll(),
        _ => return None,
    })
}

/// Runs one of the caught item's events, if `addr` is one.
pub fn run_event(_r: &mut Running, _gobj: GObjId, addr: u32) -> bool {
    match addr {
        PHYS | KB_PHYS => phys(),
        _ => return false,
    }
    true
}

/// Caught by `owner`'s tongue, `it_802F2BFC`: its victim the catcher, its source the
/// catcher's player, the tongue's state on the tongue's tip, in the air, blinking, still.
// Ports `it_802F2BFC`; aliased once a check reaches it.
pub fn catch(r: &mut Running, gobj: GObjId, owner: GObjId) {
    catch_in(r, gobj, owner, STATES);
}

/// Caught by Kirby's copy's tongue, `it_802F2D98`: the same in its own state table.
// Ports `it_802F2D98` and `ftKb_SpecialNYs_80109238`; aliased once a check reaches them.
pub fn catch_kirby(r: &mut Running, gobj: GObjId, owner: GObjId) {
    catch_in(r, gobj, owner, KB_STATES);
}

fn catch_in(r: &mut Running, gobj: GObjId, owner: GObjId, states: u32) {
    let slot = i32::from(r.game.fighter(owner).player_idx);
    let ip = item_mut(r.game, gobj);
    ip.atk_victim = Some(owner);
    ip.grab_victim = Some(owner);
    // `it_8026C220`.
    ip.xCB0_source_ply = slot;
    super::super::set_flag(&mut ip.xDD0_flag, 1, false);
    ip.xBC_itemStateContainer = Some(ItemStatesRef(states));
    let bone = article(ip).model(r.data).bone_attach_id(r.data);
    let part = tip(r, owner);
    lib2725::attach_to_part(r.game, r.data, gobj, bone, owner, part);
    change_state(r, gobj, 0, 0);
    maplib::enter_air(item_mut(r.game, gobj));
    super::super::hitbox::start_blink(r.game, gobj);
    // `it_80274ECC`.
    item_mut(r.game, gobj).xD09 = 1;
    lib2725::clear_stomp(r.game, gobj);
    // `it_8026BD54`.
    item_mut(r.game, gobj).xDCC_flag.b3 = false;
    // `it_80273408`.
    lib2725::clear_velocity(r.game, gobj);
    lib2725::clear_x64(r.game, gobj);
    lib2725::clear_x58(r.game, gobj);
    lib2725::clear_nudge(r.game, gobj);
    lib2725::clear_knockback(r.game, gobj);
}

/// `it_2F2B_UnkMotion0_Anim` (and Kirby's copy's `itKirbyyoshispecialn_UnkMotion0_Anim`).
// Ports `it_2F2B_UnkMotion0_Anim`; aliased once a check reaches it.
fn anim() -> bool {
    false
}

/// `it_2F2B_UnkMotion0_Phys`.
// Ports `it_2F2B_UnkMotion0_Phys`; aliased once a check reaches it.
fn phys() {}

/// `it_2F2B_UnkMotion0_Coll`.
// Ports `it_2F2B_UnkMotion0_Coll`; aliased once a check reaches it.
fn coll() -> bool {
    false
}

/// Let go by the tongue, `it_802F2CE0`: off the tongue at its tip's place, the catch counted
/// as the catcher's hit on an enemy, its destruction kind cleared when `clear`, its victims
/// forgotten, knocked out as an enemy is and gone. Kirby's copy's item lets go the same way
/// (`it_802F2E7C`).
// Ports `it_802F2CE0` and `it_802F2E7C`; aliased once a check reaches them.
pub fn release(r: &mut Running, gobj: GObjId, clear: bool) {
    let bone = article(super::super::item(r.game, gobj))
        .model(r.data)
        .bone_attach_id(r.data);
    let j = lib2725::bone(r.game, r.data, gobj, bone).expect("lb_8000C390: a missing joint");
    crate::game::ft::co::throw::unconstrain(&mut r.game.objs, j);
    let yoshi = super::super::item(r.game, gobj)
        .grab_victim
        .expect("an item on a tongue without its catcher");
    let part = tip(r, yoshi);
    let tip_joint = r.bone_joint(yoshi, part);
    let pos = crate::lb::joint::world_position(&mut r.game.objs, r.data, tip_joint, None);
    item_mut(r.game, gobj).pos = pos;
    super::super::coll::enemy_hit_by_fighter(r, yoshi, gobj);
    super::super::coll::enemy_hit_stats(r, yoshi, gobj, 0.0);
    let ip = item_mut(r.game, gobj);
    if clear {
        ip.destroy_type = 0;
    }
    ip.atk_victim = None;
    ip.grab_victim = None;
    zako::ko(r, gobj);
    destroy(r, gobj);
}
