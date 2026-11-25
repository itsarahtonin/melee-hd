//! The capsule, `melee/it/kinds/itcapsule.c`: a container that breaks open when it lands
//! after a throw or takes a hit, dropping an item (or coins), or exploding. Its rest, falls,
//! pickup, throw and drop are the containers' ([`container`]).

use super::super::ground::fall_to_floor;
use super::super::item::{article, change_state};
use gekko_fp as fp;

use super::super::{drop, hitbox, item, item_mut, jobj_of, lib26b1, lib2725};
use super::container::{self, Container};
use crate::fpu::mem;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;

pub const SPAWNED: u32 = 0x8027_CF00;
pub const LANDED: u32 = 0x8027_D0B8;
pub const ANIM0: u32 = 0x8027_D110;
pub const PHYS0: u32 = 0x8027_D118;
pub const COLL0: u32 = 0x8027_D11C;
pub const FELL: u32 = 0x8027_D148;
pub const ANIM4: u32 = 0x8027_D170;
pub const PHYS4: u32 = 0x8027_D178;
pub const COLL4: u32 = 0x8027_D1A8;
pub const PICKED_UP: u32 = 0x8027_D1D4;
pub const ANIM2: u32 = 0x8027_D1FC;
pub const PHYS2: u32 = 0x8027_D204;
pub const DROPPED: u32 = 0x8027_D208;
pub const THROWN: u32 = 0x8027_D230;
pub const PHYS3: u32 = 0x8027_D258;
pub const COLL3: u32 = 0x8027_D2A4;
pub const BREAK: u32 = 0x8027_D2DC;
pub const ANIM5: u32 = 0x8027_D30C;
pub const PHYS5: u32 = 0x8027_D32C;
pub const COLL5: u32 = 0x8027_D330;
pub const DMG_DEALT: u32 = 0x8027_D338;
pub const DMG_RECEIVED: u32 = 0x8027_D414;
pub const ENTERED_AIR: u32 = 0x8027_D450;
pub const ANIM6: u32 = 0x8027_D478;
pub const PHYS6: u32 = 0x8027_D480;
pub const COLL6: u32 = 0x8027_D484;
pub const CLANKED: u32 = 0x8027_D4B8;
pub const REFLECTED: u32 = 0x8027_D4F4;
pub const HIT_SHIELD: u32 = 0x8027_D530;
pub const SHIELD_BOUNCED: u32 = 0x8027_D614;
/// `it_8027CF30`, the landing's break check, which the landed state's collision passes on.
pub const LAND_CHECK: u32 = 0x8027_CF30;

/// The capsule: its own fall (the state alone), held in 2, thrown into 3 and dropped into 4,
/// still to be picked up as it goes.
pub const CAPSULE: Container = Container {
    fell,
    landed: container::landed,
    tilts: false,
    held: 2,
    thrown: 3,
    dropped: 4,
    let_go_disables_pickup: false,
};

/// Runs one of the capsule's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM4 | ANIM2 | ANIM6 | COLL5 => container::still(r, gobj),
        COLL0 => container::rests(r, gobj, &CAPSULE),
        COLL4 => container::lands(r, gobj, &CAPSULE),
        COLL3 => coll3(r, gobj),
        ANIM5 => container::blast_lasts(r, gobj),
        COLL6 => container::skids(r, gobj, &CAPSULE),
        DMG_DEALT | DMG_RECEIVED | CLANKED | REFLECTED => hit(r, gobj),
        HIT_SHIELD => hit_shield(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the capsule's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => container::landed(r, gobj),
        PHYS0 | PHYS2 | PHYS5 | PHYS6 => container::idle(r, gobj),
        FELL => fell(r, gobj),
        PHYS4 => container::gravity(r, gobj),
        PICKED_UP => container::picked_up(r, gobj, &CAPSULE),
        DROPPED => container::dropped(r, gobj, &CAPSULE),
        THROWN => container::thrown(r, gobj, &CAPSULE),
        PHYS3 => container::thrown_phys(r, gobj),
        BREAK => break_open(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        LAND_CHECK => land_check(r, gobj),
        _ => return false,
    }
    true
}

/// The capsule's attributes (`ItCapsuleAttr`): the items it holds, and the chance (one in
/// `x4`) it explodes instead.
fn attrs(r: &Running, gobj: GObjId) -> (i32, i32) {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a capsule without attributes");
    (r.data.u32(a) as i32, r.data.u32(a.at(4)) as i32)
}

/// `itCapsule_Logic0_Spawned`: closed, on the ground.
#[doc(alias = "itCapsule_Logic0_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).vars.capsule();
    v.hit.set(0);
    v.opened.set(0);
    fell(r, gobj);
}

/// Opens the capsule, `it_8027CF30`: coins in a coin match, else one in `x4` explodes and
/// the rest drop their items. Whether it's gone (`capsule.x0`).
#[doc(alias = "it_8027CF30")]
pub fn land_check(r: &mut Running, gobj: GObjId) {
    let pos = item(r.game, gobj).pos;
    let gone = if drop::drop_coins(r, gobj, pos, None) {
        true
    } else {
        let (n, bomb) = attrs(r, gobj);
        if r.game.objs.rng.below(bomb) == 0 {
            break_open(r, gobj);
            false
        } else {
            drop::drop_contents(r, gobj, n, false);
            true
        }
    };
    item_mut(r.game, gobj)
        .vars
        .capsule()
        .hit
        .set(i32::from(gone));
}

/// Explodes the capsule, `it_8027CFE8`: hidden, it blows up for a while, pushing the wind.
#[doc(alias = "it_8027CFE8")]
pub fn explode(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let j = jobj_of(r.game, gobj);
    jobj::set_flags_all(&mut r.game.objs.jobjs, j, jobj::HIDDEN);
    item_mut(r.game, gobj).anim_speed = 1.0;
    lib2725::clear_velocity(r.game, gobj);
    item_mut(r.game, gobj).vars.capsule().opened.set(1);
    lib2725::blast_effect(r, gobj);
    hitbox::set_hit_ids(r.game, gobj);
    hitbox::start_blink(r.game, gobj);
    lib26b1::freeze_lifetime(r.game, gobj);
    lib2725::start_vanishing(r.game, r.data, gobj);
    let pos = item(r.game, gobj).pos;
    let angle = mem(fp::frsp(60.0 * std::f64::consts::PI / 180.0));
    crate::lb::wind::blast(r.game.gr.winds_mut(), pos, 0x78, 1.0, 0.02, angle);
    change_state(r, gobj, 5, 2);
}

/// Falls, `it_8027D148`.
#[doc(alias = "it_8027D148")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, 2);
}

/// `itCapsule_UnkMotion3_Coll`: thrown, it opens where it lands.
#[doc(alias = "itCapsule_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, land_check);
    item_mut(r.game, gobj).vars.capsule().hit.get() != 0
}

/// Explodes, once, `it_8027D2DC`.
#[doc(alias = "it_8027D2DC")]
pub fn break_open(r: &mut Running, gobj: GObjId) {
    if item_mut(r.game, gobj).vars.capsule().opened.get() == 0 {
        explode(r, gobj);
    }
}

/// Hit, it opens unless it's exploding, `itCapsule_Logic0_DmgDealt` (and the damage taken,
/// clank and reflection callbacks): whether it's gone.
#[doc(alias = "itCapsule_Logic0_DmgDealt")]
#[doc(alias = "itCapsule_Logic0_DmgReceived")]
#[doc(alias = "itCapsule_Logic0_Clanked")]
#[doc(alias = "itCapsule_Logic0_Reflected")]
pub fn hit(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid != 5 {
        land_check(r, gobj);
    }
    item_mut(r.game, gobj).vars.capsule().hit.get() != 0
}

/// Opens on a shield when thrown, `itCapsule_Logic0_HitShield`.
#[doc(alias = "itCapsule_Logic0_HitShield")]
pub fn hit_shield(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).msid == 3 {
        land_check(r, gobj);
        return item_mut(r.game, gobj).vars.capsule().hit.get() != 0;
    }
    false
}

/// Bounces off a shield when thrown or dropped, `itCapsule_Logic0_ShieldBounced`.
#[doc(alias = "itCapsule_Logic0_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    let msid = item(r.game, gobj).msid;
    if msid == 3 || msid == 4 {
        lib2725::bounce_off_shield(r.game, gobj);
    }
    false
}

/// Skidding off a slope, `itCapsule_Logic0_EnteredAir`.
#[doc(alias = "itCapsule_Logic0_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 6, 2);
}
