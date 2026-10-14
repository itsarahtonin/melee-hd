//! Peach's vegetables, `melee/it/kinds/itpeachturnip.c`: pulled into her hand with a face
//! drawn by weighted odds, which sets its damage, then held, thrown or dropped, bouncing off
//! what it hits, until its life runs out.

use super::super::data::{attr, common};
use super::super::item::{animate, article, attr_of, change, change_state, common_data, destroy};
use super::super::{TurnipFlag, item, item_mut, jobj_of, lib26b1, lib2725};
use crate::game::state::Running;
use crate::game::structs::SpawnItem;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;
use gekko_fp as fp;

use crate::fpu::{mem, reg};

pub const DESTROYED: u32 = 0x802B_D47C;
pub const PICKED_UP: u32 = 0x802B_D628;
pub const MOTION4_ANIM: u32 = 0x802B_D6D4;
pub const MOTION4_PHYS: u32 = 0x802B_D6DC;
pub const MOTION1_ANIM: u32 = 0x802B_D6E0;
pub const MOTION1_PHYS: u32 = 0x802B_D718;
pub const THROWN: u32 = 0x802B_D748;
pub const MOTION3_ANIM: u32 = 0x802B_D7E0;
pub const MOTION3_PHYS: u32 = 0x802B_D818;
pub const MOTION3_COLL: u32 = 0x802B_D864;
pub const DROPPED: u32 = 0x802B_D8CC;
pub const DMG_DEALT: u32 = 0x802B_D938;
pub const CLANKED: u32 = 0x802B_D978;
pub const REFLECTED: u32 = 0x802B_D99C;
pub const HIT_SHIELD: u32 = 0x802B_D9C0;
pub const SHIELD_BOUNCED: u32 = 0x802B_D9E4;
pub const EVT_UNK: u32 = 0x802B_DA08;

/// `It_Kind_Peach_Turnip`.
pub const KIND: i32 = 99;
/// `It_Kind_BombHei`, `It_Kind_Dosei` (Mr. Saturn) and `It_Kind_Sword`, which she can pull too.
const BOMBHEI: i32 = 6;
const DOSEI: i32 = 7;
const SWORD: i32 = 12;

/// Runs one of the vegetable's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        MOTION4_ANIM => still(r, gobj),
        MOTION1_ANIM | MOTION3_ANIM => life_out(r, gobj),
        MOTION3_COLL => flying_coll(r, gobj),
        DMG_DEALT => dmg_dealt(r, gobj),
        CLANKED | HIT_SHIELD => bounced_off(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the vegetable's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        MOTION4_PHYS => idle(r, gobj),
        MOTION1_PHYS => falling(r, gobj),
        THROWN => thrown(r, gobj),
        MOTION3_PHYS => flying(r, gobj),
        DROPPED => dropped(r, gobj),
        _ => return false,
    }
    true
}

/// `itPeachTurnipAttributes`' word at `off`: `x0_lifetime`, `x4_length`, then (odds, damage)
/// pairs from `x8`.
fn attr_word(r: &Running, gobj: GObjId, off: u32) -> u32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a vegetable without attributes");
    r.data.u32(a.at(off))
}

/// The face the vegetable gets, by the attributes' weighted odds, `it_802BD32C`.
#[doc(alias = "it_802BD32C")]
pub fn pick_face(r: &mut Running, gobj: GObjId) -> i32 {
    let len = attr_word(r, gobj, 4) as i32;
    let odds = |r: &Running, i: i32| attr_word(r, gobj, 8 + 8 * i as u32) as i32;
    let mut sum = 0i32;
    for i in 0..len {
        sum = sum.wrapping_add(odds(r, i));
    }
    let rand = r.game.objs.rng.below(sum);
    let mut cum = odds(r, 0);
    let mut ret = -1;
    let mut i = 0;
    while i < len && ret == -1 {
        if rand < cum {
            ret = i;
        }
        cum = cum.wrapping_add(odds(r, i + 1));
        i += 1;
    }
    ret
}

/// Takes the vegetable away, `it_802BD45C`.
#[doc(alias = "it_802BD45C")]
pub fn remove(r: &mut Running, gobj: GObjId) {
    destroy(r, gobj);
}

/// `itPeachTurnip_Logic56_Destroyed`: its puller forgets it.
#[doc(alias = "itPeachTurnip_Logic56_Destroyed")]
fn destroyed(r: &mut Running, gobj: GObjId) {
    if let Some(owner) = item_mut(r.game, gobj)
        .xDD4_itemVar
        .peach_turnip()
        .xDE4_owner
        .get()
    {
        r.peach_unset_veg(owner);
    }
}

/// Pulls an item of `kind` for `parent` at `pos`, into its hand at bone `part`,
/// `it_802BD4AC`: a turnip gets its life, face, damage and puller; anything but a sword is
/// sized as its holder.
#[doc(alias = "it_802BD4AC")]
pub fn spawn(
    r: &mut Running,
    parent: GObjId,
    pos: Vec3,
    part: i32,
    kind: i32,
    facing_dir: f32,
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
    let gobj = super::super::item::spawn_air(r, &mut spawn)?;
    if kind == KIND {
        let life = f32::from_bits(attr_word(r, gobj, 0));
        lib2725::set_lifetime(r.game, r.data, gobj, life);
        let face = pick_face(r, gobj);
        item_mut(r.game, gobj)
            .xDD4_itemVar
            .peach_turnip()
            .xDD8
            .set(face);
        let damage = attr_word(r, gobj, 8 + 8 * face as u32 + 4) as i32;
        let ip = item_mut(r.game, gobj);
        let owner = ip.owner;
        let v = ip.xDD4_itemVar.peach_turnip();
        v.xDDC_damage.set(damage);
        v.xDD4.set(TurnipFlag { picked_up: false });
        v.xDE4_owner.set(owner);
        lib26b1::silence(r.game, gobj);
    }
    super::super::item::pick_up(r, gobj, parent, part);
    if kind != SWORD {
        lib2725::scale_by_owner(r, gobj);
        let ip = item_mut(r.game, gobj);
        let scl = ip.scl;
        match ip.kind {
            KIND => ip.xDD4_itemVar.peach_turnip().xDE0_scl.set(scl),
            BOMBHEI | DOSEI => crate::game::code::unported_call(
                "it_802BD4AC's scale for a Bob-omb or Mr. Saturn (their variables)",
            ),
            _ => {}
        }
    }
    Some(gobj)
}

/// The vegetable's face, its model posed at its face's frame (`it_80273670`).
fn show_face(r: &mut Running, gobj: GObjId) {
    let face = item_mut(r.game, gobj)
        .xDD4_itemVar
        .peach_turnip()
        .xDD8
        .get();
    let frame = mem(fp::frsp(f64::from(face)));
    super::drmariopill::set_pose(r, gobj, 0, frame);
}

/// `itPeachTurnip_Logic56_PickedUp`: its face shown; held (state 0, silenced the first time)
/// or, picked up again, in state 4.
#[doc(alias = "itPeachTurnip_Logic56_PickedUp")]
fn picked_up(r: &mut Running, gobj: GObjId) {
    show_face(r, gobj);
    let again = item_mut(r.game, gobj)
        .xDD4_itemVar
        .peach_turnip()
        .xDD4
        .get()
        .picked_up;
    if again {
        change_state(r, gobj, 4, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, 0, change::ANIM_UPDATE);
        lib26b1::unsilence(r.game, gobj);
    }
    item_mut(r.game, gobj)
        .xDD4_itemVar
        .peach_turnip()
        .xDD4
        .set(TurnipFlag { picked_up: true });
    animate(r, gobj);
}

/// `itPeachturnip_UnkMotion1_Anim` and `itPeachturnip_UnkMotion3_Anim` (`decrease_lifetimer`):
/// gone once its life is out.
#[doc(alias = "itPeachturnip_UnkMotion1_Anim")]
#[doc(alias = "itPeachturnip_UnkMotion3_Anim")]
fn life_out(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    ip.xD44_lifeTimer = mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    ip.xD44_lifeTimer <= 0.0
}

/// `itPeachturnip_UnkMotion4_Anim`: held, nothing to animate.
#[doc(alias = "itPeachturnip_UnkMotion4_Anim")]
fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// `itPeachturnip_UnkMotion4_Phys`: held, nothing to move.
#[doc(alias = "itPeachturnip_UnkMotion4_Phys")]
fn idle(_: &mut Running, _: GObjId) {}

/// `itPeachturnip_UnkMotion1_Phys`: falls.
#[doc(alias = "itPeachturnip_UnkMotion1_Phys")]
fn falling(r: &mut Running, gobj: GObjId) {
    let a = attr_of(item(r.game, gobj));
    let accel = a.f32(r.data, attr::FALL_SPEED);
    let max = a.f32(r.data, attr::FALL_SPEED_MAX);
    lib2725::fall(r.game, gobj, accel, max);
}

/// `itPeachTurnip_Logic56_Thrown`: flying (state 2), its face shown, its hitbox dealing its
/// face's damage, at its size.
#[doc(alias = "itPeachTurnip_Logic56_Thrown")]
fn thrown(r: &mut Running, gobj: GObjId) {
    let j = jobj_of(r.game, gobj).expect("a vegetable without a model");
    change_state(r, gobj, 2, change::ANIM_UPDATE | change::DROP_UPDATE);
    show_face(r, gobj);
    let v = item_mut(r.game, gobj).xDD4_itemVar.peach_turnip();
    let (damage, scl) = (v.xDDC_damage.get(), v.xDE0_scl.get());
    super::super::coll::set_hit_damage(r.game, r.data, gobj, 0, damage as u32);
    lib2725::rescale(r.game, gobj, j, scl);
}

/// `itPeachturnip_UnkMotion3_Phys` (`Item_ApplyFallingPhysics`): falls, its spin slowed as it
/// does.
#[doc(alias = "itPeachturnip_UnkMotion3_Phys")]
fn flying(r: &mut Running, gobj: GObjId) {
    falling(r, gobj);
    let x68 = common_data(r.game).f32(r.data, common::X68);
    lib2725::clamp_fall(r.game, gobj, x68);
}

/// `itPeachturnip_UnkMotion3_Coll`: off a floor, wall or ceiling it bounces and falls
/// (state 1); gone if it touched without bouncing.
#[doc(alias = "itPeachturnip_UnkMotion3_Coll")]
fn flying_coll(r: &mut Running, gobj: GObjId) -> bool {
    let chk = super::super::ground::air_touches(r, gobj);
    if chk & 0xE != 0 {
        super::super::maplib::bounce(r, gobj, chk);
        change_state(r, gobj, 1, change::ANIM_UPDATE);
    } else if chk & 0x3 != 0 {
        return true;
    }
    false
}

/// `itPeachTurnip_Logic56_Dropped`: dropping (state 3), its face shown.
#[doc(alias = "itPeachTurnip_Logic56_Dropped")]
fn dropped(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
    show_face(r, gobj);
}

/// `itPeachTurnip_Logic56_DmgDealt`: bounces off what it hit and falls.
#[doc(alias = "itPeachTurnip_Logic56_DmgDealt")]
fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    change_state(r, gobj, 1, change::ANIM_UPDATE);
    false
}

/// `itPeachTurnip_Logic56_Clanked` and `itPeachTurnip_Logic56_HitShield`: bounces off.
#[doc(alias = "itPeachTurnip_Logic56_Clanked")]
#[doc(alias = "itPeachTurnip_Logic56_HitShield")]
fn bounced_off(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// `itPeachTurnip_Logic56_Reflected`: sent back.
#[doc(alias = "itPeachTurnip_Logic56_Reflected")]
fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj);
    false
}

/// `itPeachTurnip_Logic56_ShieldBounced`: bounces off the shield.
#[doc(alias = "itPeachTurnip_Logic56_ShieldBounced")]
fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj);
    false
}

/// `itPeachTurnip_Logic56_EvtUnk`: forgets `other`, its puller among them.
#[doc(alias = "itPeachTurnip_Logic56_EvtUnk")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.peach_turnip();
    if v.xDE4_owner.get() == other {
        v.xDE4_owner.set(None);
    }
    lib26b1::forget(r.game, gobj, other);
}
