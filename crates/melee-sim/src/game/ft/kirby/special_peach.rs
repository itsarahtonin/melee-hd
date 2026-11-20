//! Peach's copy (`ftkirbyspecialpeach.c`): Toad held out from his hand, whose bubble turns an
//! attack into the counter's spore. Peach's own code
//! ([`ft::peach::special_n`](super::super::peach::special_n)) over his states, Peach's
//! attributes 0xE4 bytes on in his (the bubble's, `specialn_pe_absorbdesc`, 0x340 on), his
//! first aerial Toad of a jump and Toad in his `u` (`xCC`, `xD0`), his hand, the copy's own
//! Toad and spore kinds, his generic cleanups as hooks and his own callbacks, and two branches
//! of his own ([`KIRBY`]). Only his copy's addresses are here.

use super::super::super::state::Running;
use super::super::peach::ms as pe;
use super::super::peach::special_n::Toad;
use super::ms;
use crate::game::it::ItKind;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const ON_ACCESSORY4: u32 = 0x8010_C288;
    pub const ON_HIT_ACCESSORY4: u32 = 0x8010_C344;
    pub const TOAD_GONE: u32 = 0x8010_C3C0;
    pub const CLEANUP: u32 = 0x8010_C3F4;
    pub const ON_ENTER_HITLAG: u32 = 0x8010_C44C;
    pub const ON_EXIT_HITLAG: u32 = 0x8010_C47C;
    pub const ENTER: u32 = 0x8010_C4D4;
    pub const AIR_ENTER: u32 = 0x8010_C560;
    pub const LW_ANIM: u32 = 0x8010_C5FC;
    pub const AIR_LW_ANIM: u32 = 0x8010_C6B8;
    pub const LW_IASA: u32 = 0x8010_C774;
    pub const AIR_LW_IASA: u32 = 0x8010_C778;
    pub const LW_PHYS: u32 = 0x8010_C77C;
    pub const AIR_LW_PHYS: u32 = 0x8010_C7B0;
    pub const LW_COLL: u32 = 0x8010_C860;
    pub const AIR_LW_COLL: u32 = 0x8010_C89C;
    pub const TO_AIR: u32 = 0x8010_C8D8;
    pub const TO_GROUND: u32 = 0x8010_C9CC;
    pub const LW_HIT_ANIM: u32 = 0x8010_CAB4;
    pub const AIR_LW_HIT_ANIM: u32 = 0x8010_CB18;
    pub const LW_HIT_IASA: u32 = 0x8010_CB7C;
    pub const AIR_LW_HIT_IASA: u32 = 0x8010_CB80;
    pub const LW_HIT_PHYS: u32 = 0x8010_CB84;
    pub const AIR_LW_HIT_PHYS: u32 = 0x8010_CBA4;
    pub const LW_HIT_COLL: u32 = 0x8010_CBF4;
    pub const AIR_LW_HIT_COLL: u32 = 0x8010_CC30;
    pub const HIT_TO_AIR: u32 = 0x8010_CC6C;
    pub const HIT_TO_GROUND: u32 = 0x8010_CCD4;
    pub const HIT_HOOKS: u32 = 0x8010_CD44;
    pub const ON_HIT: u32 = 0x8010_CD88;
}

/// Kirby's Toad. Held on his hand (`FtPart_R3rdNa`, by bone index); not his special item; and
/// forgetting it leaves his damage hooks in place.
pub const KIRBY: Toad = Toad {
    states: ms::PE_SPECIAL_LW - pe::SPECIAL_N,
    attrs: 0xE4,
    bubble_attrs: 0x340,
    u: 0xB8,
    bone: 44,
    toad_kind: ItKind::KirbyPeachToad.id(),
    spore_kind: ItKind::KirbyPeachToadSpore.id(),
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    on_accessory4: addr::ON_ACCESSORY4,
    on_hit_accessory4: addr::ON_HIT_ACCESSORY4,
    on_enter_hitlag: addr::ON_ENTER_HITLAG,
    on_exit_hitlag: addr::ON_EXIT_HITLAG,
    on_hit: addr::ON_HIT,
    held_item_spec: false,
    clears_hooks: false,
};

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::ON_ACCESSORY4 => g.peach_toad_on_accessory4(gobj, p),
        addr::ON_HIT_ACCESSORY4 => g.peach_toad_on_hit_accessory4(gobj, p),
        addr::TOAD_GONE => g.peach_toad_gone(gobj, p),
        addr::CLEANUP => g.peach_toad_on_death2(gobj, p),
        addr::ON_ENTER_HITLAG => g.peach_toad_enter_hitlag(gobj, p),
        addr::ON_EXIT_HITLAG => g.peach_toad_exit_hitlag(gobj, p),
        addr::ENTER => g.peach_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.peach_special_air_n_enter(gobj, p),
        addr::LW_ANIM => g.peach_special_n_anim(gobj, p),
        addr::AIR_LW_ANIM => g.peach_special_air_n_anim(gobj, p),
        addr::LW_IASA | addr::AIR_LW_IASA | addr::LW_HIT_IASA | addr::AIR_LW_HIT_IASA => {
            g.peach_special_n_iasa(gobj)
        }
        addr::LW_PHYS => g.peach_special_n_phys(gobj),
        addr::AIR_LW_PHYS => g.peach_special_air_n_phys(gobj, p),
        addr::LW_COLL => g.peach_special_n_coll(gobj, p),
        addr::AIR_LW_COLL => g.peach_special_air_n_coll(gobj, p),
        addr::TO_AIR => g.peach_special_n_to_air(gobj, p),
        addr::TO_GROUND => g.peach_special_n_to_ground(gobj, p),
        addr::LW_HIT_ANIM => g.peach_special_n_hit_anim(gobj, p),
        addr::AIR_LW_HIT_ANIM => g.peach_special_air_n_hit_anim(gobj, p),
        addr::LW_HIT_PHYS => g.peach_special_n_hit_phys(gobj),
        addr::AIR_LW_HIT_PHYS => g.peach_special_air_n_hit_phys(gobj, p),
        addr::LW_HIT_COLL => g.peach_special_n_hit_coll(gobj, p),
        addr::AIR_LW_HIT_COLL => g.peach_special_air_n_hit_coll(gobj, p),
        addr::HIT_TO_AIR => g.peach_special_n_hit_to_air(gobj, p),
        addr::HIT_TO_GROUND => g.peach_special_n_hit_to_ground(gobj, p),
        addr::HIT_HOOKS => g.peach_special_n_hit_hooks(gobj, p),
        addr::ON_HIT => g.peach_toad_hit(gobj, p),
        _ => return false,
    }
    true
}
