//! The Ice Climbers' copy (`ftkirbyspecialiceclimber.c`): Ice Shot as Kirby throws it, with a
//! hammer of his own in his hand (an accessory from the copy's data). The Ice Climbers' own
//! code ([`ft::popo::special_n`](super::super::popo::special_n)) over his states, their
//! attributes 0x398 bytes on in his, the ice (`xC0`), the air shot's hop used (`xC4`) and the
//! ice's height (`xC8`) in his `u`, the copy's own ice kind, his generic cleanups as hooks,
//! his voice, and two branches of his own ([`KIRBY`]). Only his copy's addresses and his
//! hammer are here.

use super::super::super::state::Running;
use super::super::popo::special_n::IceShot;
use super::{attr, ms};
use crate::game::ft::FighterKind;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const REMOVE_ICE: u32 = 0x8010_8D04;
    pub const ENTER: u32 = 0x8010_8D64;
    pub const AIR_ENTER: u32 = 0x8010_8E14;
    pub const N_ANIM: u32 = 0x8010_8EFC;
    pub const AIR_N_ANIM: u32 = 0x8010_8F38;
    pub const N_IASA: u32 = 0x8010_8F74;
    pub const AIR_N_IASA: u32 = 0x8010_8F78;
    pub const N_PHYS: u32 = 0x8010_8F7C;
    pub const AIR_N_PHYS: u32 = 0x8010_8F9C;
    pub const N_COLL: u32 = 0x8010_8FBC;
    pub const AIR_N_COLL: u32 = 0x8010_9030;
    pub const ICE: u32 = 0x8010_90D4;
}

/// Kirby's Ice Shot: his ice kept, his hooks left in place when it's forgotten, and the
/// copy's hammer in his hand.
pub const KIRBY: IceShot = IceShot {
    states: [ms::PP_SPECIAL_N, ms::PP_SPECIAL_AIR_N],
    attrs: attr::PP_AIR_HOP - 0x4,
    u_ice: 0xC0,
    u_air: 0xA4,
    ice_kind: ItKind::KirbyIceClimbersIceShot.id(),
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    ice_cb: addr::ICE,
    voices: [0x2233B, 0x2233B],
    clears_hooks: false,
    hammer: true,
};

/// `FtPart_LThumbNb`: the hand the hammer is in.
const PART_L_THUMB_NB: i32 = 31;

impl Running<'_> {
    /// The hammer in his hand (the copy's `hat_dynamics`' second accessory).
    pub(crate) fn kb_ic_hammer(&mut self, gobj: GObjId) {
        let copy =
            self.game.ft.kirby.copies[FighterKind::Popo.index()].expect("a copy without its data");
        let hammer = self.data.ptr(copy.at(0x10)).expect("the copy's hammer");
        self.set_accessory(gobj, hammer);
        let acc = self.game.fighter(gobj).accessory;
        let hand = self.part_joint(gobj, PART_L_THUMB_NB);
        crate::lb::anim::attach(&mut self.game.objs, acc, hand);
    }
}

#[doc(alias = "ftKb_PpSpecialN_IASA")]
#[doc(alias = "ftKb_PpSpecialAirN_IASA")]
#[doc(alias = "ftKb_PpSpecialN_Phys")]
#[doc(alias = "ftKb_PpSpecialAirN_Phys")]
#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::REMOVE_ICE => g.popo_remove_ice(gobj, p),
        addr::ENTER => g.popo_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.popo_special_air_n_enter(gobj, p),
        addr::N_ANIM => g.popo_special_n_anim(gobj),
        addr::AIR_N_ANIM => g.popo_special_air_n_anim(gobj),
        addr::N_IASA | addr::AIR_N_IASA => {}
        addr::N_PHYS => g.ground_friction(gobj),
        addr::AIR_N_PHYS => g.air_physics_no_drift(gobj),
        addr::N_COLL => g.popo_special_n_coll(gobj, p),
        addr::AIR_N_COLL => g.popo_special_air_n_coll(gobj, p),
        addr::ICE => g.popo_special_n_ice(gobj, p),
        _ => return false,
    }
    true
}
