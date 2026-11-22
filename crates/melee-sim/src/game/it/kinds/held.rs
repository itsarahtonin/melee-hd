//! The held items' shared callbacks: the Fire Flower, the Fan, the Ray Gun, Lip's Stick, the
//! Poké Ball, the Parasol, the Super Scope and the Star Rod (`melee/it/kinds/itfflower.c`,
//! `itharisen.c`, `itlgun.c`, `itlipstick.c`, `itmball.c`, `itparasol.c`, `itsscope.c`,
//! `itstarrod.c`, near-copies of each other here). Each lies still (state 0), falls (1) and is
//! held (2); let go of or thrown, it falls in its own state, bounces off what it hits and lands;
//! off a slope it skids. What they share is the spawn, the fall, the landings, the pickup, the
//! drop and the throw, the hits' bounces, the reflection and the skid, over a [`Held`] per item;
//! what each does in hand (the shots, the swings, the flames, the parasol's opening) and the
//! Poké Ball's opening stay each item's own.

use gekko_fp as fp;

use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state};
use super::super::{item, item_mut, lib26b1, lib2725};
use crate::fpu::reg;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// One held item's settings the shared callbacks read.
pub struct Held {
    /// Its name, for the stops.
    pub name: &'static str,
    /// Whether its spawn loads its attributes' first word into `xD4C` (the shooting items'
    /// ammunition, the swung items' charges).
    pub charges: bool,
    /// What its spawn sets up after the charges, before it falls.
    pub on_spawn: Option<fn(&mut Running, GObjId)>,
    /// Its fall (state 1): [`fell`], or the Poké Ball's, which keeps a closed ball's animation.
    pub fell: fn(&mut Running, GObjId),
    /// Its state let go of (`Dropped`).
    pub dropped: i32,
    /// Its state thrown (`Thrown`).
    pub thrown: i32,
    /// Whether letting go of it or throwing it updates the drop (`DROP_UPDATE`): all but the
    /// Fire Flower.
    pub drop_update: bool,
    /// Its state skidding off a slope.
    pub skid: i32,
    /// Whether its hits bounce it only once it's let go of or thrown (the swung items), not
    /// whatever its state.
    pub bounces_only_let_go: bool,
    /// The Poké Ball's: a hit, a reflection or a shield's bounce stops it being picked up, and
    /// a hit turns its hitboxes off.
    pub hits_disarm: bool,
    /// The Poké Ball's: skidding off a slope, it turns to face its motion.
    pub skid_faces_motion: bool,
}

/// Its spawn: with [`Held::charges`], its attributes' first word as its charges; its own set-up
/// ([`Held::on_spawn`]); then it falls.
#[doc(alias = "itFFlower_Logic25_Spawned")]
#[doc(alias = "itHarisen_Logic24_Spawned")]
#[doc(alias = "itLGun_Logic16_Spawned")]
#[doc(alias = "itLipstick_Logic23_Spawned")]
#[doc(alias = "itMball_Spawned")]
#[doc(alias = "itParasol_Logic13_Spawned")]
#[doc(alias = "itSScope_Logic21_Spawned")]
#[doc(alias = "itStarRod_Logic22_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId, h: &Held) {
    if h.charges {
        let a = article(item(r.game, gobj))
            .special(r.data)
            .unwrap_or_else(|| panic!("a {} without attributes", h.name));
        let charges = r.data.u32(a) as i32;
        item_mut(r.game, gobj).ammo = charges;
    }
    if let Some(f) = h.on_spawn {
        f(r, gobj);
    }
    (h.fell)(r, gobj);
}

/// Where its shots or swings leave from, in the hand's space (its attributes' `x4`, `x8` and
/// `xC`): `it_8028E774` (the Ray Gun's muzzle), `it_80295748` (Lip's Stick's spores),
/// `it_80292394` (the Star Rod's stars).
#[doc(alias = "it_8028E774")]
#[doc(alias = "it_80295748")]
#[doc(alias = "it_80292394")]
pub fn hand_offset(r: &Running, gobj: GObjId, h: &Held) -> Vec3 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .unwrap_or_else(|| panic!("a {} without attributes", h.name));
    Vec3::new(
        r.data.f32(a.at(4)),
        r.data.f32(a.at(8)),
        r.data.f32(a.at(0xC)),
    )
}

/// A charge spent if it has any (`xD4C`), as a shot or a swing's does; its owner, who fires.
pub fn spend_charge(r: &mut Running, gobj: GObjId) -> Option<GObjId> {
    let ip = item_mut(r.game, gobj);
    if ip.ammo > 0 {
        ip.ammo -= 1;
    }
    ip.owner
}

/// Stops and lies still, to be picked up: `it_80292FF0` (the Fire Flower), `it_8029287C` (the
/// Fan), `it_8028E7D8` (the Ray Gun), `it_802957AC` (Lip's Stick), `itMball_80297944`,
/// `it_8028B0EC` (the Parasol), `it_80292030` (the Super Scope), `it_802923F8` (the Star Rod).
#[doc(alias = "it_80292FF0")]
#[doc(alias = "it_8029287C")]
#[doc(alias = "it_8028E7D8")]
#[doc(alias = "it_802957AC")]
#[doc(alias = "itMball_80297944")]
#[doc(alias = "it_8028B0EC")]
#[doc(alias = "it_80292030")]
#[doc(alias = "it_802923F8")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// Falls (state 1): `it_8029313C` (the Fire Flower), `it_8029290C` (the Fan), `it_8028E860`
/// (the Ray Gun), `it_8029583C` (Lip's Stick), `it_8028B17C` (the Parasol), `it_802920B8` (the
/// Super Scope), `it_80292488` (the Star Rod). The Poké Ball's is its own.
#[doc(alias = "it_8029313C")]
#[doc(alias = "it_8029290C")]
#[doc(alias = "it_8028E860")]
#[doc(alias = "it_8029583C")]
#[doc(alias = "it_8028B17C")]
#[doc(alias = "it_802920B8")]
#[doc(alias = "it_80292488")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// The states that animate nothing.
#[doc(alias = "itHarisen_UnkMotion0_Anim")]
#[doc(alias = "itHarisen_UnkMotion6_Anim")]
#[doc(alias = "itHarisen_UnkMotion8_Anim")]
#[doc(alias = "itHarisen_UnkMotion9_Anim")]
#[doc(alias = "itLgun_UnkMotion0_Anim")]
#[doc(alias = "itLgun_UnkMotion2_Anim")]
#[doc(alias = "itLgun_UnkMotion4_Anim")]
#[doc(alias = "itLgun_UnkMotion5_Anim")]
#[doc(alias = "itLipstick_UnkMotion0_Anim")]
#[doc(alias = "itLipstick_UnkMotion4_Anim")]
#[doc(alias = "itLipstick_UnkMotion5_Anim")]
#[doc(alias = "itMball_Motion0_Anim")]
#[doc(alias = "itMball_Motion2_Anim")]
#[doc(alias = "itMball_Motion3_Anim")]
#[doc(alias = "itMball_Motion4_Anim")]
#[doc(alias = "itParasol_UnkMotion0_Anim")]
#[doc(alias = "itParasol_UnkMotion3_Anim")]
#[doc(alias = "itParasol_UnkMotion10_Anim")]
#[doc(alias = "itSscope_UnkMotion0_Anim")]
#[doc(alias = "itSscope_UnkMotion2_Anim")]
#[doc(alias = "itSscope_UnkMotion3_Anim")]
#[doc(alias = "itSscope_UnkMotion4_Anim")]
#[doc(alias = "itStarrod_UnkMotion0_Anim")]
#[doc(alias = "itStarrod_UnkMotion2_Anim")]
#[doc(alias = "itStarrod_UnkMotion4_Anim")]
#[doc(alias = "itStarrod_UnkMotion5_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing.
#[doc(alias = "itFflower_UnkMotion0_Phys")]
#[doc(alias = "itFflower_UnkMotion5_Phys")]
#[doc(alias = "itFflower_UnkMotion7_Phys")]
#[doc(alias = "itHarisen_UnkMotion0_Phys")]
#[doc(alias = "itHarisen_UnkMotion6_Phys")]
#[doc(alias = "itHarisen_UnkMotion9_Phys")]
#[doc(alias = "itLgun_UnkMotion0_Phys")]
#[doc(alias = "itLgun_UnkMotion2_Phys")]
#[doc(alias = "itLgun_UnkMotion3_Phys")]
#[doc(alias = "itLgun_UnkMotion5_Phys")]
#[doc(alias = "itLipstick_UnkMotion0_Phys")]
#[doc(alias = "itLipstick_UnkMotion2_Phys")]
#[doc(alias = "itLipstick_UnkMotion5_Phys")]
#[doc(alias = "itMball_Motion0_Phys")]
#[doc(alias = "itMball_Motion2_Phys")]
#[doc(alias = "itMball_Motion4_Phys")]
#[doc(alias = "itMball_Motion5_Phys")]
#[doc(alias = "itParasol_UnkMotion0_Phys")]
#[doc(alias = "itParasol_UnkMotion3_Phys")]
#[doc(alias = "itParasol_UnkMotion10_Phys")]
#[doc(alias = "itSscope_UnkMotion0_Phys")]
#[doc(alias = "itSscope_UnkMotion2_Phys")]
#[doc(alias = "itSscope_UnkMotion4_Phys")]
#[doc(alias = "itStarrod_UnkMotion0_Phys")]
#[doc(alias = "itStarrod_UnkMotion2_Phys")]
#[doc(alias = "itStarrod_UnkMotion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// Lying still, it falls off ledges.
#[doc(alias = "itFflower_UnkMotion0_Coll")]
#[doc(alias = "itHarisen_UnkMotion0_Coll")]
#[doc(alias = "itLgun_UnkMotion0_Coll")]
#[doc(alias = "itLipstick_UnkMotion0_Coll")]
#[doc(alias = "itMball_Motion0_Coll")]
#[doc(alias = "itParasol_UnkMotion0_Coll")]
#[doc(alias = "itSscope_UnkMotion0_Coll")]
#[doc(alias = "itStarrod_UnkMotion0_Coll")]
pub fn grounded(r: &mut Running, gobj: GObjId, h: &Held) -> bool {
    ground_or_fall(r, gobj, h.fell);
    false
}

/// Gravity.
#[doc(alias = "itFflower_UnkMotion1_Phys")]
#[doc(alias = "itFflower_UnkMotion6_Phys")]
#[doc(alias = "itHarisen_UnkMotion1_Phys")]
#[doc(alias = "itLgun_UnkMotion1_Phys")]
#[doc(alias = "itLipstick_UnkMotion1_Phys")]
#[doc(alias = "itMball_Motion1_Phys")]
#[doc(alias = "itMball_Motion6_Phys")]
#[doc(alias = "itSscope_UnkMotion1_Phys")]
#[doc(alias = "itStarrod_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// Let go of or thrown, gravity and the spin it slows (`Item_ApplyFallingPhysics`).
#[doc(alias = "itHarisen_UnkMotion8_Phys")]
#[doc(alias = "itLgun_UnkMotion4_Phys")]
#[doc(alias = "itLipstick_UnkMotion4_Phys")]
#[doc(alias = "itMball_Motion3_Phys")]
#[doc(alias = "itSscope_UnkMotion3_Phys")]
#[doc(alias = "itStarrod_UnkMotion4_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    super::falling_phys(r, gobj);
}

/// Falling, it lands.
#[doc(alias = "itHarisen_UnkMotion1_Coll")]
#[doc(alias = "itHarisen_UnkMotion7_Coll")]
#[doc(alias = "itLipstick_UnkMotion1_Coll")]
#[doc(alias = "itParasol_UnkMotion2_Coll")]
#[doc(alias = "itStarrod_UnkMotion1_Coll")]
pub fn lands(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Falling with charges left (`xD4C`), it lands; empty, it falls through, gone once it's past
/// the stage.
#[doc(alias = "itFflower_UnkMotion1_Coll")]
#[doc(alias = "itFflower_UnkMotion6_Coll")]
#[doc(alias = "itLgun_UnkMotion4_Coll")]
#[doc(alias = "itLipstick_UnkMotion4_Coll")]
#[doc(alias = "itSscope_UnkMotion3_Coll")]
#[doc(alias = "itStarrod_UnkMotion3_Coll")]
#[doc(alias = "itStarrod_UnkMotion4_Coll")]
pub fn lands_loaded(r: &mut Running, gobj: GObjId) -> bool {
    if item(r.game, gobj).ammo != 0 {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Picked up, held (state 2). The Parasol's is its own.
#[doc(alias = "itFFlower_Logic25_PickedUp")]
#[doc(alias = "itHarisen_Logic24_PickedUp")]
#[doc(alias = "itLGun_Logic16_PickedUp")]
#[doc(alias = "itLipstick_Logic23_PickedUp")]
#[doc(alias = "itMball_PickedUp")]
#[doc(alias = "itSScope_Logic21_PickedUp")]
#[doc(alias = "itStarRod_Logic22_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Into `state`, let go of or thrown.
fn let_go(r: &mut Running, gobj: GObjId, h: &Held, state: i32) {
    let flags = if h.drop_update {
        change::ANIM_UPDATE | change::DROP_UPDATE
    } else {
        change::ANIM_UPDATE
    };
    change_state(r, gobj, state, flags);
}

/// Let go of, it falls in its [`Held::dropped`] state (where the item's throw is the same
/// callback, thrown too). The Fan's and the Parasol's are their own.
#[doc(alias = "itFFlower_Logic25_Dropped")]
#[doc(alias = "itFFlower_Logic25_Thrown")]
#[doc(alias = "itLGun_Logic16_Dropped")]
#[doc(alias = "itLGun_Logic16_Thrown")]
#[doc(alias = "itLipstick_Logic23_Dropped")]
#[doc(alias = "itMball_Dropped")]
#[doc(alias = "itMball_Thrown")]
#[doc(alias = "itSScope_Logic21_Dropped")]
#[doc(alias = "itSScope_Logic21_Thrown")]
#[doc(alias = "itStarRod_Logic22_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId, h: &Held) {
    let_go(r, gobj, h, h.dropped);
}

/// Thrown, it flies in its [`Held::thrown`] state.
#[doc(alias = "itHarisen_Logic24_Thrown")]
#[doc(alias = "itLipstick_Logic23_Thrown")]
#[doc(alias = "itStarRod_Logic22_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId, h: &Held) {
    let_go(r, gobj, h, h.thrown);
}

/// Bounces off what it hit, a clank or a shield (with [`Held::bounces_only_let_go`], only let
/// go of or thrown; with [`Held::hits_disarm`], no longer to be picked up, its hitboxes off).
#[doc(alias = "itFFlower_Logic25_DmgDealt")]
#[doc(alias = "itFFlower_Logic25_Clanked")]
#[doc(alias = "itFFlower_Logic25_HitShield")]
#[doc(alias = "itHarisen_Logic24_DmgDealt")]
#[doc(alias = "itHarisen_Logic24_Clanked")]
#[doc(alias = "itHarisen_Logic24_HitShield")]
#[doc(alias = "itLGun_Logic16_DmgDealt")]
#[doc(alias = "itLGun_Logic16_Clanked")]
#[doc(alias = "itLGun_Logic16_HitShield")]
#[doc(alias = "itLipstick_Logic23_DmgDealt")]
#[doc(alias = "itLipstick_Logic23_Clanked")]
#[doc(alias = "itLipstick_Logic23_HitShield")]
#[doc(alias = "itMball_DmgDealt")]
#[doc(alias = "itMball_Clanked")]
#[doc(alias = "itMball_HitShield")]
#[doc(alias = "itParasol_Logic13_DmgDealt")]
#[doc(alias = "itParasol_Logic13_Clanked")]
#[doc(alias = "itParasol_Logic13_HitShield")]
#[doc(alias = "itSScope_Logic21_DmgDealt")]
#[doc(alias = "itSScope_Logic21_Clanked")]
#[doc(alias = "itSScope_Logic21_HitShield")]
#[doc(alias = "itStarRod_Logic22_DmgDealt")]
#[doc(alias = "itStarRod_Logic22_Clanked")]
#[doc(alias = "itStarRod_Logic22_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId, h: &Held) -> bool {
    if h.bounces_only_let_go {
        let msid = item(r.game, gobj).msid;
        if msid != h.thrown && msid != h.dropped {
            return false;
        }
    }
    if h.hits_disarm {
        lib26b1::disable_pickup(r.game, gobj);
        lib2725::clear_hitboxes(r.game, r.data, gobj);
    }
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Sent back by a reflection (with [`Held::hits_disarm`], no longer to be picked up).
#[doc(alias = "itFFlower_Logic25_Reflected")]
#[doc(alias = "itHarisen_Logic24_Reflected")]
#[doc(alias = "itLGun_Logic16_Reflected")]
#[doc(alias = "itLipstick_Logic23_Reflected")]
#[doc(alias = "itMball_Reflected")]
#[doc(alias = "itParasol_Logic13_Reflected")]
#[doc(alias = "itSScope_Logic21_Reflected")]
#[doc(alias = "itStarRod_Logic22_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId, h: &Held) -> bool {
    if h.hits_disarm {
        lib26b1::disable_pickup(r.game, gobj);
    }
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield (with [`Held::hits_disarm`], no longer to be picked up).
#[doc(alias = "itFFlower_Logic25_ShieldBounced")]
#[doc(alias = "itHarisen_Logic24_ShieldBounced")]
#[doc(alias = "itLGun_Logic16_ShieldBounced")]
#[doc(alias = "itLipstick_Logic23_ShieldBounced")]
#[doc(alias = "itMball_ShieldBounced")]
#[doc(alias = "itParasol_Logic13_ShieldBounced")]
#[doc(alias = "itSScope_Logic21_ShieldBounced")]
#[doc(alias = "itStarRod_Logic22_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId, h: &Held) -> bool {
    if h.hits_disarm {
        lib26b1::disable_pickup(r.game, gobj);
    }
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, its [`Held::skid`] state (with [`Held::skid_faces_motion`], facing the
/// way it moves).
#[doc(alias = "itFFlower_Logic25_EnteredAir")]
#[doc(alias = "itHarisen_Logic24_EnteredAir")]
#[doc(alias = "itLGun_Logic16_EnteredAir")]
#[doc(alias = "itLipstick_Logic23_EnteredAir")]
#[doc(alias = "itMball_EnteredAir")]
#[doc(alias = "itParasol_Logic13_EnteredAir")]
#[doc(alias = "itSScope_Logic21_EnteredAir")]
#[doc(alias = "itStarRod_Logic22_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId, h: &Held) {
    change_state(r, gobj, h.skid, change::ANIM_UPDATE);
    if h.skid_faces_motion {
        let ip = item_mut(r.game, gobj);
        let v = fp::fadds(fp::fadds(reg(ip.vel.x), reg(ip.skid_vel.x)), reg(ip.slide_vel.x));
        ip.facing_dir = if v >= 0.0 { 1.0 } else { -1.0 };
        let dir = if -1.0 == ip.facing_dir { -1 } else { 1 };
        ip.coll.set_facing_dir(dir);
    }
}

/// Skidding, it stops or falls.
#[doc(alias = "itFflower_UnkMotion7_Coll")]
#[doc(alias = "itHarisen_UnkMotion9_Coll")]
#[doc(alias = "itLgun_UnkMotion5_Coll")]
#[doc(alias = "itLipstick_UnkMotion5_Coll")]
#[doc(alias = "itMball_Motion4_Coll")]
#[doc(alias = "itParasol_UnkMotion3_Coll")]
#[doc(alias = "itSscope_UnkMotion4_Coll")]
#[doc(alias = "itStarrod_UnkMotion5_Coll")]
pub fn skid(r: &mut Running, gobj: GObjId, h: &Held) -> bool {
    ground_skid(r, gobj, landed, h.fell);
    false
}
