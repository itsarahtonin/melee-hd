//! Samus's Charge Shot, `melee/it/kinds/itsamuschargeshot.c`: a ball in her hand that grows
//! with her charge (state 0) until she fires it, then a shot of the charge's size and speed
//! (states 1 to 8, by charge) that flies until its life runs out or it meets the stage. The
//! Shadow Ball's code ([`super::chargedshot`]) with its own glow in the hand and sparkles.
//!
//! Kirby's copy (`It_Kind_Kirby_SamusCharge`) runs the same code with his own checks
//! (`ft::kirby::special_samus`).

use super::super::item::{change, change_state};
use super::super::vars::ItemVars;
use super::super::{item, item_mut};
use super::chargedshot::{self as cs, ChargedShot, Hold, Top, Vars};
use crate::game::ef::{self, Arg};
use crate::game::ft::samus::special_n::ChargeShot;
use crate::game::ft::{kirby, samus};
use crate::game::it::ItKind;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const HIT_STAGE: u32 = 0x802B_5518;
pub const SPAWN: u32 = 0x802B_55C8;
pub const FIRE: u32 = 0x802B_56E4;
pub const DESTROYED: u32 = 0x802B_58D8;
pub const REMOVE: u32 = 0x802B_5974;
pub const PICKED_UP: u32 = 0x802B_59C4;
pub const ANIM_0: u32 = 0x802B_5A98;
#[doc(alias = "itSamuschargeshot_UnkMotion0_Phys")]
pub const PHYS_0: u32 = 0x802B_5CB0;
#[doc(alias = "itSamuschargeshot_UnkMotion0_Coll")]
pub const COLL_0: u32 = 0x802B_5CB4;
pub const FIRED: u32 = 0x802B_5CBC;
pub const ANIM_8: u32 = 0x802B_5D54;
pub const PHYS_8: u32 = 0x802B_5E50;
#[doc(alias = "itSamuschargeshot_UnkMotion8_Coll")]
pub const COLL_8: u32 = 0x802B_5EA0;
pub const SPARKLE: u32 = 0x802B_5EDC;
#[doc(alias = "itSamusChargeshot_Logic108_DmgDealt")]
pub const DMG_DEALT: u32 = 0x802B_5FD8;
#[doc(alias = "itSamusChargeshot_Logic108_Clanked")]
pub const CLANKED: u32 = 0x802B_5FE0;
#[doc(alias = "itSamusChargeshot_Logic108_Absorbed")]
pub const ABSORBED: u32 = 0x802B_5FE8;
pub const REFLECTED: u32 = 0x802B_5FF0;
#[doc(alias = "itSamusChargeshot_Logic108_HitShield")]
pub const HIT_SHIELD: u32 = 0x802B_613C;
pub const SHIELD_BOUNCED: u32 = 0x802B_6144;
#[doc(alias("EVT_UNK"))]
pub const FORGET: u32 = 0x802B_62B0;

/// The ball's glow in her hand, and the full shot's sparkles.
const GFX_HELD: i32 = 0x47F;
const GFX_FULL: [i32; 2] = [0x480, 0x481];
/// Kirby's shot's glow in his hand and full charge's sparkles.
const GFX_HELD_KIRBY: i32 = 0x4A1;
const GFX_FULL_KIRBY: [i32; 2] = [0x4A2, 0x4A3];

/// The Charge Shot: its maker's queries, its sparkles when full, its state at most the eighth,
/// no wobble, and it turns to face the way it flies back.
pub const CHARGE_SHOT: ChargedShot = ChargedShot {
    vars: shared,
    released,
    hold,
    sparkle: SPARKLE,
    top: Top::Eighth,
    full_sparkles,
    wobbles: false,
    turns: true,
};

/// Runs one of the shot's predicates, if `addr` is one.
#[melee_callbacks::callbacks(Pred)]
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM_0 => cs::animate_held(r, gobj, &CHARGE_SHOT),
        COLL_0 => false,
        ANIM_8 => cs::animate_flying(r, gobj, &CHARGE_SHOT),
        COLL_8 => cs::hit_stage(r, gobj),
        DMG_DEALT | CLANKED | ABSORBED | HIT_SHIELD => cs::hit_ends(r, gobj),
        REFLECTED => cs::reflected(r, gobj, &CHARGE_SHOT),
        SHIELD_BOUNCED => cs::shield_bounced(r, gobj, &CHARGE_SHOT),
        _ => return None,
    })
}

/// Runs one of the shot's events, if `addr` is one.
#[melee_callbacks::callbacks(Proc)]
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => cs::destroyed(r, gobj, &CHARGE_SHOT),
        PICKED_UP => picked_up(r, gobj),
        PHYS_0 => {}
        PHYS_8 => cs::fly(r, gobj, &CHARGE_SHOT),
        SPARKLE => cs::sparkle(r, gobj, &CHARGE_SHOT),
        _ => return false,
    }
    true
}

/// The fields the Shadow Ball has too.
fn shared(v: &mut ItemVars) -> &mut dyn Vars {
    v.samus_chargeshot()
}

/// The Charge Shot its maker charges it with, by the shot's kind: Samus's or Kirby's copy's.
fn maker_shot(kind: i32) -> Option<&'static ChargeShot> {
    match ItKind::of(kind) {
        ItKind::SamusChargeShot => Some(&samus::special_n::SAMUS),
        ItKind::KirbySamusChargeShot => Some(&kirby::special_samus::KIRBY),
        _ => None,
    }
}

/// Its maker lets it go, `ftSs_SpecialN_801291F0`.
fn released(r: &mut Running, maker: GObjId, kind: i32) {
    if let Some(p) = maker_shot(kind) {
        r.samus_shot_released(maker, p);
    }
}

/// Its maker's charge, unless she lost it (hit, out of the charge's states, or holding none).
fn hold(r: &mut Running, maker: GObjId, kind: i32) -> Hold {
    let Some(p) = maker_shot(kind) else {
        return Hold::Unknown;
    };
    if r.samus_shot_lost(Some(maker), p) || r.samus_left_charge(Some(maker), p) {
        return Hold::Lost;
    }
    match r.samus_charge_of(Some(maker), p) {
        Some((charge, max)) => Hold::Charge(charge, max),
        None => Hold::Lost,
    }
}

/// Samus's or Kirby's sparkles when full.
fn full_sparkles(kind: i32) -> &'static [i32] {
    match ItKind::of(kind) {
        ItKind::SamusChargeShot => &GFX_FULL,
        ItKind::KirbySamusChargeShot => &GFX_FULL_KIRBY,
        _ => &[],
    }
}

/// Makes the ball in `parent`'s hand at `pos` ([`cs::spawn_held`]): not fired, no effects,
/// her model's scale kept.
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
) -> Option<GObjId> {
    cs::spawn_held(r, parent, pos, part, kind, facing_dir, init)
}

/// Its variables as it's made in `parent`'s hand.
fn init(r: &mut Running, shot: GObjId, parent: GObjId) {
    let scale = crate::fpu::mem(Running::model_scale(r.game.fighter(parent)));
    let v = item_mut(r.game, shot).vars.samus_chargeshot();
    v.fired.set(0);
    v.unread_e0.set(0.0);
    v.effects.set(0);
    v.maker.set(Some(parent));
    v.unread_scale.set(scale);
    v.unread_e2c.set(0.0);
    v.unread_e30.set(0.0);
    v.unread_e34.set(0.0);
}

/// Fires the shot ([`cs::fire`]).
pub fn fire(r: &mut Running, gobj: GObjId, pos: Vec3, angle: f32, charge: f32, max: f32) {
    cs::fire(r, gobj, pos, angle, charge, max, &CHARGE_SHOT);
}

/// The ball taken away from her hand ([`cs::remove`]).
pub fn remove(r: &mut Running, gobj: GObjId) {
    cs::remove(r, gobj, &CHARGE_SHOT);
}

/// In her hand, `it_2725_Logic108_PickedUp`: the held state and its glow.
#[doc(alias = "it_2725_Logic108_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    let j = cs::grandchild(r, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
    let gfx = match ItKind::of(item(r.game, gobj).kind) {
        ItKind::SamusChargeShot => GFX_HELD,
        ItKind::KirbySamusChargeShot => GFX_HELD_KIRBY,
        _ => return,
    };
    ef::spawn(r, gfx, Some(gobj), &[Arg::JObj(Some(j))]);
    item_mut(r.game, gobj)
        .vars
        .samus_chargeshot()
        .effects
        .set(1);
}
