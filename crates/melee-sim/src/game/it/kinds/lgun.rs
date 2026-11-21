//! The Ray Gun, `melee/it/kinds/itlgun.c`: a shooting item that falls, lands and is held, and
//! fires a ray (`lgunray`) from the fighter's shooting states while it has shots. Its
//! callbacks are the held items' ([`super::held`]), but for its recoil.

use super::super::item::{change, change_state};
use super::super::item_mut;
use super::held::{self, Held};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x8028_E738;
pub const ANIM0: u32 = 0x8028_E828;
pub const PHYS0: u32 = 0x8028_E830;
pub const COLL0: u32 = 0x8028_E834;
pub const FELL: u32 = 0x8028_E860;
pub const ANIM1: u32 = 0x8028_E888;
pub const PHYS1: u32 = 0x8028_E890;
pub const COLL1: u32 = 0x8028_E8C0;
pub const PICKED_UP: u32 = 0x8028_E904;
pub const ANIM2: u32 = 0x8028_E92C;
pub const PHYS2: u32 = 0x8028_E934;
pub const ANIM3: u32 = 0x8028_E96C;
pub const PHYS3: u32 = 0x8028_E9B4;
pub const DROPPED: u32 = 0x8028_E9B8;
pub const THROWN: u32 = 0x8028_E9E0;
pub const PHYS4: u32 = 0x8028_EA08;
pub const DMG_DEALT: u32 = 0x8028_EA54;
pub const CLANKED: u32 = 0x8028_EA78;
pub const HIT_SHIELD: u32 = 0x8028_EA9C;
pub const REFLECTED: u32 = 0x8028_EAC0;
pub const SHIELD_BOUNCED: u32 = 0x8028_EAE0;
pub const ENTERED_AIR: u32 = 0x8028_EB00;
pub const ANIM5: u32 = 0x8028_EB28;
pub const PHYS5: u32 = 0x8028_EB30;
pub const COLL5: u32 = 0x8028_EB34;

/// The Ray Gun: its shots and its recoil's timer cleared, let go of or thrown into state 4,
/// skidding in 5.
pub const RAY_GUN: Held = Held {
    name: "Ray Gun",
    charges: true,
    on_spawn: Some(unloaded_recoil),
    fell: held::fell,
    dropped: 4,
    thrown: 4,
    drop_update: true,
    skid: 5,
    bounces_only_let_go: false,
    hits_disarm: false,
    skid_faces_motion: false,
};

/// Runs one of the Ray Gun's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM5 => held::still(r, gobj),
        ANIM3 => recoil(r, gobj),
        COLL0 => held::grounded(r, gobj, &RAY_GUN),
        COLL1 => held::lands_loaded(r, gobj),
        COLL5 => held::skid(r, gobj, &RAY_GUN),
        DMG_DEALT | CLANKED | HIT_SHIELD => held::bounce(r, gobj, &RAY_GUN),
        REFLECTED => held::reflected(r, gobj, &RAY_GUN),
        SHIELD_BOUNCED => held::shield_bounced(r, gobj, &RAY_GUN),
        _ => return None,
    })
}

/// Runs one of the Ray Gun's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => held::spawned(r, gobj, &RAY_GUN),
        FELL => held::fell(r, gobj),
        PHYS0 | PHYS2 | PHYS3 | PHYS5 => held::idle(r, gobj),
        PHYS1 => held::gravity(r, gobj),
        PICKED_UP => held::picked_up(r, gobj),
        DROPPED | THROWN => held::dropped(r, gobj, &RAY_GUN),
        PHYS4 => held::thrown_phys(r, gobj),
        ENTERED_AIR => held::entered_air(r, gobj, &RAY_GUN),
        _ => return false,
    }
    true
}

/// Its spawn's own part: no recoil.
fn unloaded_recoil(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDD4_itemVar.lgun().timer.set(0);
}

/// The muzzle, in the hand's space (`ItLGunAttr::pos`, [`held::hand_offset`]).
pub fn muzzle(r: &Running, gobj: GObjId) -> crate::vec::Vec3 {
    held::hand_offset(r, gobj, &RAY_GUN)
}

/// A shot spent, a ray from its owner at `pos`, `it_8028E79C`.
#[doc(alias = "it_8028E79C")]
pub fn fire(r: &mut Running, gobj: GObjId, pos: crate::vec::Vec3, facing: f32) {
    let owner = held::spend_charge(r, gobj);
    super::lgunray::spawn(r, owner, pos, facing);
}

/// The shots a bonus counts to, `it_8026C1D4`: the Ray Gun's attributes read as its variables
/// (`itLGun_ItemVars::timer`, their first word, which is its ammunition).
#[doc(alias = "it_8026C1D4")]
pub fn shots_bonus(game: &crate::game::state::Game, data: &melee_data::archive::Archives) -> u32 {
    let a = super::super::item::article_of(game, data, ItKind::RayGun.id())
        .expect("no Ray Gun article");
    let attrs = super::super::data::Article(a)
        .special(data)
        .expect("a Ray Gun without attributes");
    data.u32(attrs)
}

/// A shot's recoil, `it_8028E938`.
#[doc(alias = "it_8028E938")]
pub fn shot(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).xDD4_itemVar.lgun().timer.set(40);
    change_state(r, gobj, 3, change::ANIM_UPDATE);
}

/// `itLgun_UnkMotion3_Anim`: the recoil counts down, then back to held.
#[doc(alias = "itLgun_UnkMotion3_Anim")]
pub fn recoil(r: &mut Running, gobj: GObjId) -> bool {
    let v = item_mut(r.game, gobj).xDD4_itemVar.lgun();
    let t = v.timer.get().wrapping_sub(1);
    v.timer.set(t);
    if t <= 0 {
        change_state(r, gobj, 2, change::ANIM_UPDATE);
    }
    false
}
