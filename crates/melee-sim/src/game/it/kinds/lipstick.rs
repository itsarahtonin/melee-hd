//! Lip's Stick, `melee/it/kinds/itlipstick.c`: a swung item with charges (`xD4C`) that falls,
//! lands and is held; each swing's script flag puffs spores (`itlipstickspore.c`) while
//! charges last; thrown, the first thing it touches bursts its spores and it bounces off. Its
//! callbacks are the held items' ([`super::held`]), but for the throw's flight and its
//! readiness in hand.

use super::super::ground::{air, air_touches, try_land};
use super::super::{item, item_mut, maplib};
use super::held::{self, Held};
use super::lipstickspore;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const SPAWNED: u32 = 0x8029_570C;
pub const LANDED: u32 = 0x8029_57AC;
pub const ANIM0: u32 = 0x8029_5804;
pub const PHYS0: u32 = 0x8029_580C;
pub const COLL0: u32 = 0x8029_5810;
pub const FELL: u32 = 0x8029_583C;
pub const ANIM1: u32 = 0x8029_5864;
pub const PHYS1: u32 = 0x8029_586C;
pub const COLL1: u32 = 0x8029_589C;
pub const PICKED_UP: u32 = 0x8029_58C8;
pub const ANIM2: u32 = 0x8029_58F0;
pub const PHYS2: u32 = 0x8029_5904;
pub const DROPPED: u32 = 0x8029_5908;
pub const COLL4: u32 = 0x8029_5930;
pub const THROWN: u32 = 0x8029_5974;
pub const PHYS4: u32 = 0x8029_599C;
pub const COLL3: u32 = 0x8029_59E8;
pub const DMG_DEALT: u32 = 0x8029_5ACC;
pub const ENTERED_AIR: u32 = 0x8029_5B08;
pub const ANIM5: u32 = 0x8029_5B30;
pub const PHYS5: u32 = 0x8029_5B38;
pub const COLL5: u32 = 0x8029_5B3C;
pub const CLANKED: u32 = 0x8029_5B70;
pub const REFLECTED: u32 = 0x8029_5BAC;
pub const HIT_SHIELD: u32 = 0x8029_5BCC;
pub const SHIELD_BOUNCED: u32 = 0x8029_5C08;

/// Lip's Stick: its charges and its spores not yet ready, thrown into state 3 and dropped into
/// 4, skidding in 5, its hits bouncing it only once let go of.
pub const LIPS_STICK: Held = Held {
    name: "Lip's Stick",
    charges: true,
    on_spawn: Some(unready),
    fell: held::fell,
    dropped: 4,
    thrown: 3,
    drop_update: true,
    skid: 5,
    bounces_only_let_go: true,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of Lip's Stick's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM5 => held::still(r, gobj),
        ANIM2 => ready(r, gobj),
        COLL0 => held::grounded(r, gobj, &LIPS_STICK),
        COLL1 => held::lands(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => held::lands_loaded(r, gobj),
        COLL5 => held::skid(r, gobj, &LIPS_STICK),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &LIPS_STICK),
        REFLECTED => held::reflected(r, gobj, &LIPS_STICK),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &LIPS_STICK),
        _ => return None,
    })
}

/// Runs one of Lip's Stick's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &LIPS_STICK),
        LANDED => held::landed(r, gobj),
        FELL => held::fell(r, gobj),
        PHYS0 | PHYS2 | PHYS5 => held::idle(r, gobj),
        PHYS1 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED => held::dropped(r, gobj, &LIPS_STICK),
        THROWN => held::thrown(r, gobj, &LIPS_STICK),
        PHYS4 => held::thrown_phys(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &LIPS_STICK),
        _ => return false,
    }
    true
}

/// Its spawn's own part: its spores not ready until it's held (`xDAC`).
fn unready(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
}

/// Where a swing's spores leave from, in the hand's space ([`held::hand_offset`]).
pub fn spore_offset(r: &Running, gobj: GObjId) -> Vec3 {
    held::hand_offset(r, gobj, &LIPS_STICK)
}

/// A swing's spores from `pos`, facing `facing` (`kind` by the swing), a charge spent,
/// `it_80295770`.
#[doc(alias = "it_80295770")]
pub fn puff(r: &mut Running, gobj: GObjId, pos: Vec3, facing: f32, kind: i32) {
    let owner = held::spend_charge(r, gobj).expect("a Lip's Stick swung with no owner");
    lipstickspore::swing(r, owner, pos, facing, kind);
}

/// `itLipstick_UnkMotion2_Anim`: held, it will spread spores if thrown.
#[doc(alias = "itLipstick_UnkMotion2_Anim")]
pub fn ready(r: &mut Running, gobj: GObjId) -> bool {
    item_mut(r.game, gobj).xDAC_itcmd_var0 = 1;
    false
}

/// `itLipstick_UnkMotion3_Coll`: thrown with charges left, the first thing it touches puffs
/// its spores, it bounces, and on the floor it lands once it settles; empty, it falls
/// through.
#[doc(alias = "itLipstick_UnkMotion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).xD4C == 0 {
        return air(r, gobj);
    }
    let t = air_touches(r, gobj);
    if t & 0xF != 0 {
        if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
            let ip = item(r.game, gobj);
            let owner = ip.owner.expect("a Lip's Stick thrown with no owner");
            let (pos, facing) = (ip.pos, ip.facing_dir);
            lipstickspore::burst(r, owner, pos, facing, 0);
            item_mut(r.game, gobj).xDAC_itcmd_var0 = 0;
        }
        maplib::bounce(r, gobj, t);
        if t & 1 != 0 && try_land(r, gobj) {
            held::landed(r, gobj);
        }
    }
    false
}
