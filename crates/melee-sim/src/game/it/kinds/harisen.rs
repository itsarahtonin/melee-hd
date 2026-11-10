//! The fan, `melee/it/kinds/itharisen.c`: a swung item that falls, lands and is held; its
//! swings play states 3 to 6 at the swing's speed and grow it while it hits
//! (`ftCo_HarisenSwing_*`); thrown, it bounces off what it hits.

use gekko_fp as fp;

use super::super::data::attr;
use super::super::ground::{air, fall, ground_or_fall, ground_skid};
use super::super::item::{apply_scale, article, attr_of, change, change_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725};
use super::falling_phys;
use crate::fpu::{mem, reg};
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

/// `Item.xD4C`'s offset in the item's block.
const XD4C: u32 = 0xD4C;

pub const SPAWNED: u32 = 0x8029_285C;
pub const ANIM0: u32 = 0x8029_28D4;
pub const PHYS0: u32 = 0x8029_28DC;
pub const COLL0: u32 = 0x8029_28E0;
pub const ANIM8: u32 = 0x8029_2934;
pub const PHYS1: u32 = 0x8029_293C;
pub const COLL1: u32 = 0x8029_296C;
pub const PICKED_UP: u32 = 0x8029_2A5C;
pub const ANIM6: u32 = 0x8029_2A84;
pub const PHYS6: u32 = 0x8029_2A8C;
pub const DROPPED: u32 = 0x8029_2A90;
pub const COLL8: u32 = 0x8029_2AE8;
pub const THROWN: u32 = 0x8029_2B2C;
pub const PHYS8: u32 = 0x8029_2B54;
pub const COLL7: u32 = 0x8029_2BA0;
pub const DMG_DEALT: u32 = 0x8029_2BCC;
pub const ENTERED_AIR: u32 = 0x8029_2C08;
pub const ANIM9: u32 = 0x8029_2C30;
pub const PHYS9: u32 = 0x8029_2C38;
pub const COLL9: u32 = 0x8029_2C3C;
pub const CLANKED: u32 = 0x8029_2C70;
pub const REFLECTED: u32 = 0x8029_2CAC;
pub const HIT_SHIELD: u32 = 0x8029_2CCC;
pub const SHIELD_BOUNCED: u32 = 0x8029_2D08;

/// Runs one of the fan's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 | ANIM8 | ANIM6 | ANIM9 => still(r, gobj),
        COLL0 => coll0(r, gobj),
        COLL1 | COLL7 => coll1(r, gobj),
        COLL8 => coll8(r, gobj),
        COLL9 => coll9(r, gobj),
        DMG_DEALT | CLANKED | HIT_SHIELD => dmg_dealt(r, gobj),
        REFLECTED => reflected(r, gobj),
        SHIELD_BOUNCED => shield_bounced(r, gobj),
        _ => return None,
    })
}

/// Runs one of the fan's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => fell(r, gobj),
        PHYS0 | PHYS6 | PHYS9 => idle(r, gobj),
        PHYS1 => gravity(r, gobj),
        PICKED_UP => picked_up(r, gobj),
        DROPPED => dropped(r, gobj),
        THROWN => thrown(r, gobj),
        PHYS8 => thrown_phys(r, gobj),
        ENTERED_AIR => entered_air(r, gobj),
        _ => return false,
    }
    true
}

/// Grown for a swing's hit, `it_802927E8`: its attributes' scale times its own.
#[doc(alias = "it_802927E8")]
pub fn grow(r: &mut Running, gobj: GObjId) {
    let ip = item(r.game, gobj);
    let a = article(ip)
        .special(r.data)
        .expect("a fan without attributes");
    let base = attr_of(ip).f32(r.data, attr::SCALE);
    let scale = mem(fp::fmuls(reg(r.data.f32(a)), reg(base)));
    item_mut(r.game, gobj).scl = scale;
    let j = jobj_of(r.game, gobj).expect("a fan without a model");
    lib2725::set_uniform_scale(r.game, j, scale);
}

/// Back to its own size, `it_8029282C`.
#[doc(alias = "it_8029282C")]
pub fn shrink(r: &mut Running, gobj: GObjId) {
    let base = attr_of(item(r.game, gobj)).f32(r.data, attr::SCALE);
    item_mut(r.game, gobj).scl = base;
    apply_scale(r.game, gobj);
}

/// Lands and lies still, `it_8029287C`.
#[doc(alias = "it_8029287C")]
pub fn landed(r: &mut Running, gobj: GObjId) {
    lib26b1::enable_pickup(r.game, gobj);
    lib2725::clear_velocity(r.game, gobj);
    change_state(r, gobj, 0, change::ANIM_UPDATE);
}

/// The states that animate nothing, `itHarisen_UnkMotion0_Anim` (and 6, 8, 9).
#[doc(alias = "itHarisen_UnkMotion0_Anim")]
#[doc(alias = "itHarisen_UnkMotion6_Anim")]
#[doc(alias = "itHarisen_UnkMotion8_Anim")]
#[doc(alias = "itHarisen_UnkMotion9_Anim")]
pub fn still(_: &mut Running, _: GObjId) -> bool {
    false
}

/// The states that move nothing, `itHarisen_UnkMotion0_Phys` (and 6, 9).
#[doc(alias = "itHarisen_UnkMotion0_Phys")]
#[doc(alias = "itHarisen_UnkMotion6_Phys")]
#[doc(alias = "itHarisen_UnkMotion9_Phys")]
pub fn idle(_: &mut Running, _: GObjId) {}

/// `itHarisen_UnkMotion0_Coll`: falls off ledges.
#[doc(alias = "itHarisen_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    ground_or_fall(r, gobj, fell);
    false
}

/// Falls, `it_8029290C` (and `itHarisen_Logic24_Spawned`).
#[doc(alias = "it_8029290C")]
#[doc(alias = "itHarisen_Logic24_Spawned")]
pub fn fell(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 1, change::ANIM_UPDATE);
}

/// `itHarisen_UnkMotion1_Phys`: gravity.
#[doc(alias = "itHarisen_UnkMotion1_Phys")]
pub fn gravity(r: &mut Running, gobj: GObjId) {
    super::gravity(r, gobj);
}

/// `itHarisen_UnkMotion1_Coll` (and 7's): lands.
#[doc(alias = "itHarisen_UnkMotion1_Coll")]
#[doc(alias = "itHarisen_UnkMotion7_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// A swing's state (3 to 5, the jab, tilt and smash) at `speed`: `it_80292998`,
/// `it_802929C8`, `it_802929F8`.
#[doc(alias = "it_80292998")]
#[doc(alias = "it_802929C8")]
#[doc(alias = "it_802929F8")]
pub fn swing(r: &mut Running, gobj: GObjId, msid: i32, speed: f32) {
    item_mut(r.game, gobj).x5D0_animFrameSpeed = speed;
    change_state(r, gobj, msid, change::ANIM_UPDATE);
}

/// The dash swing's state, 6, at its own speed, `it_80292A28`.
#[doc(alias = "it_80292A28")]
pub fn swing_dash(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    change_state(r, gobj, 6, change::ANIM_UPDATE);
}

/// Held, `itHarisen_Logic24_PickedUp`.
#[doc(alias = "itHarisen_Logic24_PickedUp")]
pub fn picked_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
}

/// Let go of, at its own size and speed, it falls, `itHarisen_Logic24_Dropped`.
#[doc(alias = "itHarisen_Logic24_Dropped")]
pub fn dropped(r: &mut Running, gobj: GObjId) {
    item_mut(r.game, gobj).x5D0_animFrameSpeed = 1.0;
    shrink(r, gobj);
    change_state(r, gobj, 8, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// `itHarisen_UnkMotion8_Coll`: dropped, it lands if its `xD4C` (the shooting items' ammo)
/// is set, else it's gone at the first floor. No fan code writes the word, so it holds what
/// the item's block held before (docs/melee-bugs.md). Where that is the GPU's pixels, which
/// the simulation doesn't have, what is known of the copy decides it ([`Pixels`]): every byte
/// of a fighter's shadow is nonzero (a white background, the fighter in its stage's
/// intensity, at least 105, copied to 4 bits a texel) and so is each texel's first byte of the
/// results screen's RGB5A3 images (opaque: the EFB has no alpha), so the word is; under the
/// debug display's shadow volumes it is zero, the image's fill (docs/engine.md, "A fan over
/// the GPU's pixels"). Any other copy's pixels stop by name.
///
/// [`Pixels`]: crate::hsd::image::Pixels
#[doc(alias = "itHarisen_UnkMotion8_Coll")]
pub fn coll8(r: &mut Running, gobj: GObjId) -> bool {
    if landing_word_set(r, gobj) {
        fall(r, gobj, landed);
        return false;
    }
    air(r, gobj)
}

/// Whether the dropped fan's `xD4C` is nonzero, as [`coll8`] reads it.
fn landing_word_set(r: &Running, gobj: GObjId) -> bool {
    let slot = super::super::item_of(r.game, gobj).0;
    let pixels: Vec<bool> = (XD4C..XD4C + 4)
        .map(|o| {
            r.game
                .it
                .render_leftovers
                .get(&slot)
                .is_some_and(|runs| runs.iter().any(|&(a, b)| a <= o && o < b))
        })
        .collect();
    if pixels.iter().any(|&p| p) {
        let at = r.game.items.addr(slot) + XD4C;
        let copied = crate::hsd::heap::with(|h| h.image.copied_pixels(at, 4))
            .unwrap_or_else(|| vec![None; 4]);
        let known = |i: usize| copied[i].and_then(|p| p.nonzero(at + i as u32));
        let bytes = || (0..4).filter(|&i| pixels[i]).map(known);
        if bytes().any(|b| b == Some(true)) {
            return true;
        }
        if bytes().any(|b| b.is_none()) {
            crate::game::code::unported_call(
                "a dropped fan's landing reading its unset xD4C over the GPU's pixels (a copy not known nonzero)",
            );
        }
    }
    item(r.game, gobj).xD4C != 0
}

/// Thrown, `itHarisen_Logic24_Thrown`.
#[doc(alias = "itHarisen_Logic24_Thrown")]
pub fn thrown(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 7, change::ANIM_UPDATE | change::DROP_UPDATE);
}

/// Thrown or dropped, gravity and the spin it slows, `itHarisen_UnkMotion8_Phys`.
#[doc(alias = "itHarisen_UnkMotion8_Phys")]
pub fn thrown_phys(r: &mut Running, gobj: GObjId) {
    falling_phys(r, gobj);
}

/// Bounces off what it hit when thrown or dropped, a clank or a shield,
/// `itHarisen_Logic24_DmgDealt` (and `Clanked`, `HitShield`).
#[doc(alias = "itHarisen_Logic24_DmgDealt")]
#[doc(alias = "itHarisen_Logic24_Clanked")]
#[doc(alias = "itHarisen_Logic24_HitShield")]
pub fn dmg_dealt(r: &mut Running, gobj: GObjId) -> bool {
    if matches!(item(r.game, gobj).msid, 7 | 8) {
        lib2725::bounce_off_victim(r.game, r.data, gobj);
    }
    false
}

/// Skidding off a slope, `itHarisen_Logic24_EnteredAir`.
#[doc(alias = "itHarisen_Logic24_EnteredAir")]
pub fn entered_air(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 9, change::ANIM_UPDATE);
}

/// `itHarisen_UnkMotion9_Coll`: skidding, it stops or falls.
#[doc(alias = "itHarisen_UnkMotion9_Coll")]
pub fn coll9(r: &mut Running, gobj: GObjId) -> bool {
    ground_skid(r, gobj, landed, fell);
    false
}

/// Sent back by a reflection, `itHarisen_Logic24_Reflected`.
#[doc(alias = "itHarisen_Logic24_Reflected")]
pub fn reflected(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::reflected_back(r.game, gobj)
}

/// Bounces off a shield, `itHarisen_Logic24_ShieldBounced`.
#[doc(alias = "itHarisen_Logic24_ShieldBounced")]
pub fn shield_bounced(r: &mut Running, gobj: GObjId) -> bool {
    lib2725::bounce_off_shield(r.game, gobj)
}
