//! Mewtwo's copy (`ftkirbyspecialmewtwo.c`): Shadow Ball as Kirby charges it. Mewtwo's own
//! code ([`ft::mewtwo::special_n`](super::super::mewtwo::special_n)) over his states, Mewtwo's
//! attributes 0x384 bytes on in his, the ball he holds, the charge and the effects' flag in
//! his `u` (`x98`, `x9C`, `xA0`), his hand, the copy's own ball kind, sounds, flash and glow,
//! his generic cleanups as hooks, and one branch of his own ([`KIRBY`]); the charge's
//! variables are laid out as Mewtwo's (`mv.kb.specialhi`). Only his copy's addresses are here.

use super::super::super::state::Running;
use super::super::mewtwo::special_n::{Phase, ShadowBall};
use super::{attr, ms};
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const RELEASED: u32 = 0x8010_7040;
    pub const CLEANUP: u32 = 0x8010_709C;
    pub const DROP_BALL: u32 = 0x8010_7130;
    pub const RELEASE: u32 = 0x8010_71FC;
    pub const CHARGE_SFX: u32 = 0x8010_7410;
    pub const ENTER: u32 = 0x8010_7568;
    pub const AIR_ENTER: u32 = 0x8010_7638;
    pub const N_START_ANIM: u32 = 0x8010_7700;
    pub const N_LOOP_ANIM: u32 = 0x8010_7884;
    pub const N_LOOP_FULL_ANIM: u32 = 0x8010_7A84;
    pub const N_CANCEL_ANIM: u32 = 0x8010_7AB4;
    pub const N_END_ANIM: u32 = 0x8010_7B38;
    pub const AIR_N_START_ANIM: u32 = 0x8010_7B7C;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_7D0C;
    pub const AIR_N_LOOP_FULL_ANIM: u32 = 0x8010_7F0C;
    pub const AIR_N_CANCEL_ANIM: u32 = 0x8010_7F3C;
    pub const AIR_N_END_ANIM: u32 = 0x8010_7FC0;
    pub const N_START_IASA: u32 = 0x8010_8040;
    pub const N_LOOP_IASA: u32 = 0x8010_8044;
    pub const N_LOOP_FULL_IASA: u32 = 0x8010_81F8;
    pub const N_CANCEL_IASA: u32 = 0x8010_8394;
    pub const N_END_IASA: u32 = 0x8010_8398;
    pub const AIR_N_START_IASA: u32 = 0x8010_839C;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_83A0;
    pub const AIR_N_LOOP_FULL_IASA: u32 = 0x8010_8504;
    pub const AIR_N_CANCEL_IASA: u32 = 0x8010_8650;
    pub const AIR_N_END_IASA: u32 = 0x8010_8654;
    pub const N_START_PHYS: u32 = 0x8010_8658;
    pub const N_LOOP_PHYS: u32 = 0x8010_8678;
    pub const N_LOOP_FULL_PHYS: u32 = 0x8010_8698;
    pub const N_CANCEL_PHYS: u32 = 0x8010_86B8;
    pub const N_END_PHYS: u32 = 0x8010_86D8;
    pub const AIR_N_START_PHYS: u32 = 0x8010_86F8;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_8718;
    pub const AIR_N_LOOP_FULL_PHYS: u32 = 0x8010_8738;
    pub const AIR_N_CANCEL_PHYS: u32 = 0x8010_8758;
    pub const AIR_N_END_PHYS: u32 = 0x8010_8778;
    pub const N_START_COLL: u32 = 0x8010_8798;
    pub const N_LOOP_COLL: u32 = 0x8010_8820;
    pub const N_LOOP_FULL_COLL: u32 = 0x8010_88A8;
    pub const N_CANCEL_COLL: u32 = 0x8010_8930;
    pub const N_END_COLL: u32 = 0x8010_89B8;
    pub const AIR_N_START_COLL: u32 = 0x8010_8A40;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_8AC8;
    pub const AIR_N_LOOP_FULL_COLL: u32 = 0x8010_8B50;
    pub const AIR_N_CANCEL_COLL: u32 = 0x8010_8BD8;
    pub const AIR_N_END_COLL: u32 = 0x8010_8C60;
}

/// Kirby's Shadow Ball. The ball forms at and fires from his hand (`FtPart_RHandN`, bone 39);
/// his hooks are his generic cleanups, with no third one; and his charge's effects are
/// destroyed whether or not any are out.
pub const KIRBY: ShadowBall = ShadowBall {
    states: ms::MT_SPECIAL_N_START - super::super::mewtwo::ms::SPECIAL_N_START,
    attrs: attr::MT_CHARGE_TIME,
    u: 0x94,
    hand: 39,
    ball_kind: ItKind::KirbyMewtwoShadowBall.id(),
    charge_sfx: [0x30DB9, 0x30DBC, 0x30DBF, 0x30DC2],
    full_flash_y: 4.0,
    full_glow: 0x5D,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    on_death3: None,
    effects_off_always: true,
};

/// The copy's callbacks; its grounded states take friction (`ft_80084F3C`) and its aerial
/// ones a fall without drift (`ft_80084EEC`).
#[doc(alias = "ftKb_MtSpecialNStart_Phys")]
#[doc(alias = "ftKb_MtSpecialNLoop_Phys")]
#[doc(alias = "ftKb_MtSpecialNLoopFull_Phys")]
#[doc(alias = "ftKb_MtSpecialNCancel_Phys")]
#[doc(alias = "ftKb_MtSpecialNEnd_Phys")]
#[doc(alias = "ftKb_MtSpecialAirNStart_Phys")]
#[doc(alias = "ftKb_MtSpecialAirNLoop_Phys")]
#[doc(alias = "ftKb_MtSpecialAirNLoopFull_Phys")]
#[doc(alias = "ftKb_MtSpecialAirNCancel_Phys")]
#[doc(alias = "ftKb_MtSpecialAirNEnd_Phys")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::RELEASED => g.mt_n_ball_released(gobj, p),
        addr::CLEANUP => g.mt_n_lose_ball(gobj, p),
        addr::DROP_BALL => g.mt_n_drop_ball(gobj, p),
        addr::RELEASE => g.mt_n_release(gobj, p),
        addr::CHARGE_SFX => g.mt_n_charge_sfx(gobj, p),
        addr::ENTER => g.mt_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.mt_special_air_n_enter(gobj, p),
        addr::N_START_ANIM => g.mt_special_n_start_anim(gobj, p),
        addr::N_LOOP_ANIM => g.mt_special_n_loop_anim(gobj, p),
        addr::N_LOOP_FULL_ANIM => g.mt_special_n_loop_full_anim(gobj, p),
        addr::N_CANCEL_ANIM => g.mt_special_n_cancel_anim(gobj, p),
        addr::N_END_ANIM => g.mt_special_n_end_anim(gobj, p),
        addr::AIR_N_START_ANIM => g.mt_special_air_n_start_anim(gobj, p),
        addr::AIR_N_LOOP_ANIM => g.mt_special_air_n_loop_anim(gobj, p),
        addr::AIR_N_LOOP_FULL_ANIM => g.mt_special_air_n_loop_full_anim(gobj, p),
        addr::AIR_N_CANCEL_ANIM => g.mt_special_air_n_cancel_anim(gobj, p),
        addr::AIR_N_END_ANIM => g.mt_special_air_n_end_anim(gobj, p),
        addr::N_START_IASA
        | addr::N_CANCEL_IASA
        | addr::N_END_IASA
        | addr::AIR_N_START_IASA
        | addr::AIR_N_CANCEL_IASA
        | addr::AIR_N_END_IASA => g.mt_special_n_nothing(gobj),
        addr::N_LOOP_IASA => g.mt_special_n_loop_iasa(gobj, p),
        addr::N_LOOP_FULL_IASA => g.mt_special_n_loop_full_iasa(gobj, p),
        addr::AIR_N_LOOP_IASA => g.mt_special_air_n_loop_iasa(gobj, p),
        addr::AIR_N_LOOP_FULL_IASA => g.mt_special_air_n_loop_full_iasa(gobj, p),
        addr::N_START_PHYS
        | addr::N_LOOP_PHYS
        | addr::N_LOOP_FULL_PHYS
        | addr::N_CANCEL_PHYS
        | addr::N_END_PHYS => g.ground_friction(gobj),
        addr::AIR_N_START_PHYS
        | addr::AIR_N_LOOP_PHYS
        | addr::AIR_N_LOOP_FULL_PHYS
        | addr::AIR_N_CANCEL_PHYS
        | addr::AIR_N_END_PHYS => g.air_physics_no_drift(gobj),
        addr::N_START_COLL => g.mt_special_n_coll(gobj, p, Phase::Start),
        addr::N_LOOP_COLL => g.mt_special_n_coll(gobj, p, Phase::Loop),
        addr::N_LOOP_FULL_COLL => g.mt_special_n_coll(gobj, p, Phase::LoopFull),
        addr::N_CANCEL_COLL => g.mt_special_n_coll(gobj, p, Phase::Cancel),
        addr::N_END_COLL => g.mt_special_n_coll(gobj, p, Phase::End),
        addr::AIR_N_START_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Start),
        addr::AIR_N_LOOP_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Loop),
        addr::AIR_N_LOOP_FULL_COLL => g.mt_special_air_n_coll(gobj, p, Phase::LoopFull),
        addr::AIR_N_CANCEL_COLL => g.mt_special_air_n_coll(gobj, p, Phase::Cancel),
        addr::AIR_N_END_COLL => g.mt_special_air_n_coll(gobj, p, Phase::End),
        _ => return false,
    }
    true
}
