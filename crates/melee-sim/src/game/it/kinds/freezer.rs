//! Articuno (`It_PKind_Freezer`), `melee/it/kinds/itfreezer.c`: out of the ball it spreads its
//! wings and breathes its icy wind (its script's hitboxes), a frost effect every so many frames,
//! then flies up out of the top blast zone.

use gekko_fp as fp;

use super::super::ground::fall;
use super::super::item::{change, change_state};
use super::super::pokemon::{self, attr_f32, attr_i32, effect_hitlag, enter_first_state};
use super::super::{item, item_mut, jobj_of, lib26b1, lib2725, set_flag};
use crate::fpu::{mem, reg};
use crate::game::gr::stage;
use crate::game::handles::CodeRef;
use crate::game::state::Running;
use crate::hsd::gobj::GObjId;

pub const SPAWNED: u32 = 0x802C_CF9C;
pub const LANDED: u32 = 0x802C_CFFC;
pub const EVT_UNK: u32 = 0x802C_D000;
pub const ANIM1: u32 = 0x802C_D020;
pub const PHYS1: u32 = 0x802C_D060;
pub const COLL1: u32 = 0x802C_D064;
pub const BREATH_ACCESSORY: u32 = 0x802C_D090;
pub const ANIM2: u32 = 0x802C_D18C;
pub const PHYS2: u32 = 0x802C_D230;
pub const COLL2: u32 = 0x802C_D288;
pub const FROST_ACCESSORY: u32 = 0x802C_D290;
pub const ANIM0: u32 = 0x802C_D360;
pub const PHYS0: u32 = 0x802C_D384;
pub const COLL0: u32 = 0x802C_D424;

/// Its variable past the release's (`itFreezer_ItemVars`' `x60`): the frames to its next frost
/// effect.
const FROST_FRAMES: usize = 0x60;

/// Its effects' size (`it_8027ADEC`'s divisor).
const EFFECT_SIZE: f32 = 1.4;

/// Runs one of Articuno's predicates, if `addr` is one.
pub fn run_pred(r: &mut Running, gobj: GObjId, addr: u32) -> Option<bool> {
    Some(match addr {
        ANIM0 => anim0(r, gobj),
        COLL0 => coll0(r, gobj),
        ANIM1 => anim1(r, gobj),
        COLL1 => coll1(r, gobj),
        ANIM2 => anim2(r, gobj),
        COLL2 => coll2(r, gobj),
        _ => return None,
    })
}

/// Runs one of Articuno's events, if `addr` is one.
pub fn run_event(r: &mut Running, gobj: GObjId, addr: u32) -> bool {
    match addr {
        SPAWNED => spawned(r, gobj),
        LANDED => landed(r, gobj),
        PHYS0 => phys0(r, gobj),
        PHYS1 => phys1(r, gobj),
        BREATH_ACCESSORY => breath_effects(r, gobj),
        PHYS2 => phys2(r, gobj),
        FROST_ACCESSORY => frost(r, gobj),
        _ => return false,
    }
    true
}

/// Articuno out of the ball, `itFreezer_802CCF9C`: facing nowhere, its script's flags clear,
/// in the air at its first size.
#[doc(alias = "itFreezer_802CCF9C")]
pub fn spawned(r: &mut Running, gobj: GObjId) {
    let ip = item_mut(r.game, gobj);
    ip.facing_dir = 0.0;
    ip.xDAC_itcmd_var0 = 0;
    ip.xDB0_itcmd_var1 = 0;
    enter_air(r, gobj);
    let size = attr_f32(r, gobj, 0);
    pokemon::start(r, gobj, size);
}

/// Lands, out of the ball or breathing: nothing, `itFreezer_802CCFFC`.
#[doc(alias = "itFreezer_802CCFFC")]
pub fn landed(_r: &mut Running, _gobj: GObjId) {}

/// Forgets a GObj going away, `itFreezer_802CD000` (`it_8026B894`).
#[doc(alias = "itFreezer_802CD000")]
pub fn evt_unk(r: &mut Running, gobj: GObjId, other: Option<GObjId>) {
    lib26b1::forget(r.game, gobj, other);
}

/// `itFreezer_UnkMotion1_Anim`: breathing; at its animation's end it flies up.
#[doc(alias = "itFreezer_UnkMotion1_Anim")]
pub fn anim1(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_up(r, gobj);
    }
    false
}

/// `itFreezer_UnkMotion1_Phys`: nothing.
#[doc(alias = "itFreezer_UnkMotion1_Phys")]
pub fn phys1(_r: &mut Running, _gobj: GObjId) {}

/// `itFreezer_UnkMotion1_Coll`: falls, landing doing nothing.
#[doc(alias = "itFreezer_UnkMotion1_Coll")]
pub fn coll1(r: &mut Running, gobj: GObjId) -> bool {
    fall(r, gobj, landed);
    false
}

/// Its breath's `on_accessory`, `itFreezer_802CD090`: when its script says, its breath's
/// effect (`0x462`) on its sixth bone; then the frost (`0x461`) every so many frames.
#[doc(alias = "itFreezer_802CD090")]
pub fn breath_effects(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let bone = pokemon::table_bone(r.game, gobj, 5);
        pokemon::effect_scaled(r, 0x462, gobj, bone, EFFECT_SIZE);
        item_mut(r.game, gobj).xDB0_itcmd_var1 = 0;
    }
    frost(r, gobj);
}

/// Flies up, `itFreezer_802CD12C` (and `itFreezer_UnkMotion2_Anim`'s inline copy): its last
/// state, the frost alone, its rise not begun.
#[doc(alias = "itFreezer_802CD12C")]
pub fn fly_up(r: &mut Running, gobj: GObjId) {
    change_state(r, gobj, 2, change::ANIM_UPDATE);
    effect_hitlag(r.game, gobj);
    let ip = item_mut(r.game, gobj);
    ip.on_accessory = Some(CodeRef(FROST_ACCESSORY));
    ip.xDB0_itcmd_var1 = 0;
}

/// `itFreezer_UnkMotion2_Anim`: flying up, its animation looping; gone above the top blast
/// zone.
#[doc(alias = "itFreezer_UnkMotion2_Anim")]
pub fn anim2(r: &mut Running, gobj: GObjId) -> bool {
    if !lib2725::is_animating(r.game, gobj) {
        fly_up(r, gobj);
    }
    reg(item(r.game, gobj).pos.y) > reg(stage::blast_top(r.game))
}

/// `itFreezer_UnkMotion2_Phys`: when its script says, it starts rising (`x4`), and then rises
/// faster (`x8`) every frame.
#[doc(alias = "itFreezer_UnkMotion2_Phys")]
pub fn phys2(r: &mut Running, gobj: GObjId) {
    if item(r.game, gobj).xDAC_itcmd_var0 != 0 {
        let up = attr_f32(r, gobj, 4);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = up;
        ip.xDAC_itcmd_var0 = 0;
        ip.xDB0_itcmd_var1 = 1;
    }
    if item(r.game, gobj).xDB0_itcmd_var1 != 0 {
        let a = attr_f32(r, gobj, 8);
        let ip = item_mut(r.game, gobj);
        ip.x40_vel.y = mem(fp::fadds(reg(ip.x40_vel.y), reg(a)));
    }
}

/// `itFreezer_UnkMotion2_Coll`: nothing.
#[doc(alias = "itFreezer_UnkMotion2_Coll")]
pub fn coll2(_r: &mut Running, _gobj: GObjId) -> bool {
    false
}

/// Its frost, `itFreezer_802CD290` (and `itFreezer_802CD090`'s copy): when its count runs out,
/// the frost effect (`0x461`) on its model and the count back to `xC`; else one frame less.
#[doc(alias = "itFreezer_802CD290")]
pub fn frost(r: &mut Running, gobj: GObjId) {
    if pokemon::vars(r.game, gobj).i32(FROST_FRAMES) <= 0 {
        let j = jobj_of(r.game, gobj);
        let frames = attr_i32(r, gobj, 0xC);
        pokemon::vars(r.game, gobj).set_i32(FROST_FRAMES, frames);
        pokemon::effect_scaled(r, 0x461, gobj, j, EFFECT_SIZE);
        return;
    }
    let v = pokemon::vars(r.game, gobj);
    let left = v.i32(FROST_FRAMES).wrapping_sub(1);
    v.set_i32(FROST_FRAMES, left);
}

/// Into the air, `itFreezer_802CD2EC` (`Item_EnterAirStateWithHitlagAndStateDesc`).
#[doc(alias = "itFreezer_802CD2EC")]
pub fn enter_air(r: &mut Running, gobj: GObjId) {
    enter_first_state(r, gobj);
}

/// `itFreezer_UnkMotion0_Anim`: growing out of the ball.
#[doc(alias = "itFreezer_UnkMotion0_Anim")]
pub fn anim0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::grow(r, gobj);
    false
}

/// `itFreezer_UnkMotion0_Phys`: falling out of the ball; at the end stopped, its frost due at
/// once, breathing, and its effects' flag set.
#[doc(alias = "itFreezer_UnkMotion0_Phys")]
pub fn phys0(r: &mut Running, gobj: GObjId) {
    if pokemon::fall(r, gobj) {
        lib2725::clear_velocity(r.game, gobj);
        pokemon::vars(r.game, gobj).set_i32(FROST_FRAMES, -1);
        change_state(r, gobj, 1, change::ANIM_UPDATE);
        effect_hitlag(r.game, gobj);
        item_mut(r.game, gobj).on_accessory = Some(CodeRef(BREATH_ACCESSORY));
        set_flag(&mut item_mut(r.game, gobj).xDD1_flag, 1, true);
    }
}

/// `itFreezer_UnkMotion0_Coll`: lands doing nothing.
#[doc(alias = "itFreezer_UnkMotion0_Coll")]
pub fn coll0(r: &mut Running, gobj: GObjId) -> bool {
    pokemon::land(r, gobj, landed)
}
