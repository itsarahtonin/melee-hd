//! The Poké Ball, `melee/it/kinds/itmball.c`: it falls and lies about; thrown, it opens where
//! it lands and, at its attributes' moment, lets a Pokémon out (`it_8027AB64`, not ported).
//! Out of the count of Poké Balls (`Item_804A0C64.x1C`) once gone, unless its Pokémon came
//! out.

use gekko_fp as fp;

use super::super::effect;
use super::super::ground::{fall, fall_to_floor, ground_or_fall, ground_skid};
use super::super::item::{article, change, change_state, play_sfx};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, maplib};
use super::falling_phys;
use crate::fpu::reg;
use crate::game::code;
use crate::game::ef::SpawnKind;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

pub const DESTROYED: u32 = 0x8029_78DC;
pub const SPAWNED: u32 = 0x8029_7904;
pub const LANDED: u32 = 0x8029_7944;
pub const ANIM0: u32 = 0x8029_799C;
pub const PHYS0: u32 = 0x8029_79A4;
pub const COLL0: u32 = 0x8029_79A8;
pub const FELL: u32 = 0x8029_79D4;
pub const ANIM1: u32 = 0x8029_7A1C;
pub const PHYS1: u32 = 0x8029_7A24;
pub const COLL1: u32 = 0x8029_7A54;
pub const PICKED_UP: u32 = 0x8029_7ABC;
pub const ANIM2: u32 = 0x8029_7AE4;
pub const PHYS2: u32 = 0x8029_7AEC;
pub const DROPPED: u32 = 0x8029_7AF0;
pub const THROWN: u32 = 0x8029_7B18;
pub const PHYS3: u32 = 0x8029_7B40;
pub const COLL3: u32 = 0x8029_7B8C;
pub const DMG_DEALT: u32 = 0x8029_7BB8;
pub const ENTERED_AIR: u32 = 0x8029_7BF8;
pub const ANIM4: u32 = 0x8029_7C84;
pub const PHYS4: u32 = 0x8029_7C8C;
pub const COLL4: u32 = 0x8029_7C90;
pub const OPEN: u32 = 0x8029_7CC4;
pub const ON_ACCESSORY: u32 = 0x8029_7DD8;
pub const ANIM5: u32 = 0x8029_7E28;
pub const PHYS5: u32 = 0x8029_7E5C;
pub const COLL5: u32 = 0x8029_7E60;
pub const OPEN_GROUNDED: u32 = 0x8029_7E8C;
pub const ANIM6: u32 = 0x8029_7F94;
pub const PHYS6: u32 = 0x8029_7FC8;
pub const COLL6: u32 = 0x8029_7FF8;
pub const CLANKED: u32 = 0x8029_8060;
pub const REFLECTED: u32 = 0x8029_80A0;
pub const HIT_SHIELD: u32 = 0x8029_80D4;
pub const SHIELD_BOUNCED: u32 = 0x8029_8114;

/// Runs one of the Poké Ball's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM1 | ANIM2 | ANIM4 => still(r, gobj),
        ANIM5 | ANIM6 => opening(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 => coll1(r, gobj),
        COLL3 => coll3(r, gobj),
        COLL4 => coll4(r, gobj),
        COLL5 => coll5(r, gobj),
        COLL6 => coll6(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => bounce(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the Poké Ball's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        DESTROYED => destroyed(r, gobj),
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        FELL => fell(r, gobj),
        PHYS0 | PHYS2 | PHYS4 | PHYS5 => idle(r, gobj),
        PHYS1 | PHYS6 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED | THROWN => thrown(r, gobj),
        PHYS3 => falling_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        OPEN => open(r, gobj),
        ON_ACCESSORY => on_accessory(r, gobj),
        OPEN_GROUNDED => open_grounded(r, gobj),
        _ => return false,
    }
    true
}

/// `itMBallAttributes`: how long it stays open (0), and the moment its Pokémon comes out (4).
fn attr(r: &Running, gobj: GObjId, off: u32) -> f32 {
    let a = article(item(r.game, gobj))
        .special(r.data)
        .expect("a Poké Ball without attributes");
    r.data.f32(a.at(off))
}

/// `itMball_Destroyed`: out of the count of Poké Balls, unless its Pokémon came out.
#[doc(alias = "itMball_Destroyed")]
pub fn destroyed(r: &mut Running, gobj: GObjId) {
    if !item_mut(r.game, gobj).xDD4_itemVar.mball().b1.get() {
        let c = &mut r.game.it.counts;
        c.x1C = c.x1C.wrapping_sub(1);
    }
}

/// `itMball_Spawned`: closed, falling.
#[doc(alias = "itMball_Spawned")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let v = item_mut(r.game, gobj).xDD4_itemVar.mball();
    v.b0.set(false);
    v.b1.set(false);
    fell(r, gobj);
}

/// Lands and lies still, `itMball_80297944`.
#[doc(alias = "itMball_80297944")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itMball_Motion0_Anim` (2, 3 for 1 and 3, 4).
#[doc(alias = "itMball_Motion0_Anim")]
#[doc(alias = "itMball_Motion2_Anim")]
#[doc(alias = "itMball_Motion3_Anim")]
#[doc(alias = "itMball_Motion4_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itMball_Motion0_Phys` (2, 4, 5).
#[doc(alias = "itMball_Motion0_Phys")]
#[doc(alias = "itMball_Motion2_Phys")]
#[doc(alias = "itMball_Motion4_Phys")]
#[doc(alias = "itMball_Motion5_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itMball_Motion0_Coll`: falls off ledges.
#[doc(alias = "itMball_Motion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `itMball_802979D4`: a closed ball's state keeps its animation from where it was.
#[doc(alias = "itMball_802979D4")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    let open = item_mut(r.game, gobj).xDD4_itemVar.mball().b0.get();
    let flags = if open { change::ANIM_UPDATE } else { 1 };
    change_state(r, gobj, 1, flags);
}

/// Gravity, `itMball_Motion1_Phys` and `itMball_Motion6_Phys`.
#[doc(alias = "itMball_Motion1_Phys")]
#[doc(alias = "itMball_Motion6_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// A fall that turns to face its motion when it bounces, the falling states' collision.
fn fall_facing(r: &mut Running, gobj: GObjId, landed: fn(&mut Running, GObjId)) {
    let before = item(r.game, gobj).x40_vel.x;
    fall(r, gobj, landed);
    if item(r.game, gobj).x40_vel.x != before {
        lib2725::face_motion(r.game, gobj);
    }
}

/// `itMball_Motion1_Coll`: lands, facing its motion after a bounce.
#[doc(alias = "itMball_Motion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall_facing(r, gobj, landed);
    false
}

/// Held, `itMball_PickedUp`.
#[doc(alias = "itMball_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of or thrown, `itMball_Dropped` and `itMball_Thrown`.
#[doc(alias = "itMball_Dropped")]
#[doc(alias = "itMball_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 3, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itMball_Motion3_Coll`: thrown, it opens where it lands.
#[doc(alias = "itMball_Motion3_Coll")]
pub fn coll3(r: &mut Running, gobj: GObjId) -> bool {
    fall_to_floor(r, gobj, open);
    false
}

/// Bounces off what it hit, a clank or a shield, no longer to be picked up, its hitboxes
/// off, `itMball_DmgDealt` (and `Clanked`, `HitShield`).
#[doc(alias = "itMball_DmgDealt")]
#[doc(alias = "itMball_Clanked")]
#[doc(alias = "itMball_HitShield")]
pub fn bounce(r: &mut Running, gobj: GObjId) -> bool {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::clear_hitboxes(r.game, r.data, gobj);
    lib2725::bounce_off_victim(r.game, r.data, gobj);
    false
}

/// Sent back by a reflection, `itMball_Reflected`.
#[doc(alias = "itMball_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itMball_ShieldBounced`.
#[doc(alias = "itMball_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib26b1::disable_pickup(r.game, gobj);
    lib2725::bounce_off_shield(r.game, gobj)
}

/// Skidding off a slope, `itMball_EnteredAir`: facing the way it moves.
#[doc(alias = "itMball_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 4, change::ANIM_UPDATE);
    let ip = item_mut(r.game, gobj);
    let v = fp::fadds(fp::fadds(reg(ip.x40_vel.x), reg(ip.x7C.x)), reg(ip.x88.x));
    ip.facing_dir = if v >= 0.0 { 1.0 } else { -1.0 };
    let dir = if -1.0 == ip.facing_dir { -1 } else { 1 };
    ip.x378_itemColl.set_facing_dir(dir);
}

/// `itMball_Motion4_Coll`: skidding, it stops or falls.
#[doc(alias = "itMball_Motion4_Coll")]
pub fn coll4(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Opens, the first time with its flash, sound and the time it stays open; then into
/// `state`, its animation starting over (the first time) or kept.
fn open_into(r: &mut Running, gobj: GObjId, state: i32) {
    let life = attr(r, gobj, 0);
    let j = jobj_of(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.xDC8_word.flags.x1A = false;
    ip.xD5C = 0;
    lib2725::clear_velocity(r.game, gobj);
    lib2725::stop_spin(r.game, r.data, gobj);
    if !item_mut(r.game, gobj).xDD4_itemVar.mball().b0.get() {
        let ip = item_mut(r.game, gobj);
        ip.xD44_lifeTimer = life;
        ip.xDD4_itemVar.mball().b0.set(true);
        effect::queue(r, gobj, SpawnKind::Attach, 0x449, j, &[]);
        play_sfx(r, gobj, 0x10C, 0x7F, 0x40);
        change_state(r, gobj, state, change::ANIM_UPDATE);
    } else {
        change_state(r, gobj, state, 1);
    }
    item_mut(r.game, gobj).on_accessory = Some(CodeRef(ON_ACCESSORY));
}

/// Opens where it landed, `itMball_80297CC4`: its open collision box.
#[doc(alias = "itMball_80297CC4")]
pub fn open(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    let b = item(r.game, gobj).xC0C.clone();
    maplib::set_box(r.game, gobj, b);
    let ip = item_mut(r.game, gobj);
    ip.x40_vel = Vec3::new(0.0, 0.0, 0.0);
    open_into(r, gobj, 5);
}

/// `itMball_OnAccessory`: at its attributes' moment, its Pokémon comes out.
#[doc(alias = "itMball_OnAccessory")]
pub fn on_accessory(r: &mut Running, gobj: GObjId) {
    let at = attr(r, gobj, 4);
    if item(r.game, gobj).xD44_lifeTimer == at {
        code::unported_call("it_8027AB64 (a Poké Ball's Pokémon)");
    }
}

/// Open until its time runs out, `itMball_Motion5_Anim` and `itMball_Motion6_Anim`
/// (`Item_TickLifetime`).
#[doc(alias = "itMball_Motion5_Anim")]
#[doc(alias = "itMball_Motion6_Anim")]
pub fn opening(r: &mut Running, gobj: GObjId) -> bool {
    let ip = item_mut(r.game, gobj);
    if reg(ip.xD44_lifeTimer) <= 0.0 {
        return true;
    }
    ip.xD44_lifeTimer = crate::fpu::mem(fp::fsubs(reg(ip.xD44_lifeTimer), 1.0));
    false
}

/// `itMball_Motion5_Coll`: open, off a ledge it falls open.
#[doc(alias = "itMball_Motion5_Coll")]
pub fn coll5(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, open_grounded);
    false
}

/// Opens falling, `itMball_80297E8C`.
#[doc(alias = "itMball_80297E8C")]
pub fn open_grounded(r: &mut Running, gobj: GObjId) {
    lib26b1::disable_pickup(r.game, gobj);
    open_into(r, gobj, 6);
}

/// `itMball_Motion6_Coll`: falling open, it opens where it lands, facing its motion after
/// a bounce.
#[doc(alias = "itMball_Motion6_Coll")]
pub fn coll6(r: &mut Running, gobj: GObjId) -> bool {
    fall_facing(r, gobj, open);
    false
}
