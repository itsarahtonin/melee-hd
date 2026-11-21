//! Link's and Young Link's copies (`ftkirbyspeciallink.c`): the bow as Kirby draws it. Link's
//! own code ([`ft::link::special_n`](super::super::link::special_n)) over his states and
//! attributes for each copy, the arrow and the bow in his `u` (`x74`, `x78`), his bones, the
//! copy's item kinds (from his attributes), his generic cleanups as hooks, and three branches
//! of his own ([`KIRBY`]): his draw has no bones to follow, its points a table of zeros
//! (`ftKb_Init_803B7548`), so the bow's angle and the string stay at the origin. Only his
//! copies' addresses are here.

use super::super::super::state::Running;
use super::super::kirby::copy::KindFrom;
use super::super::link::ms as lk;
use super::super::link::special_n::Bow;
use super::ms;
use crate::hsd::gobj::GObjId;

/// The copy's function addresses.
pub(super) mod addr {
    pub const UNSET_ARROW: u32 = 0x800F_B418;
    pub const UNSET_BOW: u32 = 0x800F_B444;
    pub const LOOSE: u32 = 0x800F_B6DC;
    pub const REMOVE_ARROW: u32 = 0x800F_B800;
    pub const REMOVE_BOW: u32 = 0x800F_B840;
    pub const ENTER: u32 = 0x800F_B880;
    pub const AIR_ENTER: u32 = 0x800F_BA00;
    pub const N_START_ANIM: u32 = 0x800F_BBC4;
    pub const N_LOOP_ANIM: u32 = 0x800F_BDFC;
    pub const N_END_ANIM: u32 = 0x800F_BED0;
    pub const AIR_N_START_ANIM: u32 = 0x800F_BF34;
    pub const AIR_N_LOOP_ANIM: u32 = 0x800F_C1A0;
    pub const AIR_N_END_ANIM: u32 = 0x800F_C274;
    pub const N_START_IASA: u32 = 0x800F_C374;
    pub const N_LOOP_IASA: u32 = 0x800F_C46C;
    pub const N_END_IASA: u32 = 0x800F_C53C;
    pub const AIR_N_START_IASA: u32 = 0x800F_C540;
    pub const AIR_N_LOOP_IASA: u32 = 0x800F_C638;
    pub const AIR_N_END_IASA: u32 = 0x800F_C708;
    pub const N_START_PHYS: u32 = 0x800F_C70C;
    pub const N_LOOP_PHYS: u32 = 0x800F_C72C;
    pub const N_END_PHYS: u32 = 0x800F_C74C;
    pub const AIR_N_START_PHYS: u32 = 0x800F_C76C;
    pub const AIR_N_LOOP_PHYS: u32 = 0x800F_C78C;
    pub const AIR_N_END_PHYS: u32 = 0x800F_C7AC;
    pub const N_START_COLL: u32 = 0x800F_C7CC;
    pub const N_LOOP_COLL: u32 = 0x800F_C880;
    pub const N_END_COLL: u32 = 0x800F_C934;
    pub const AIR_N_START_COLL: u32 = 0x800F_C9F0;
    pub const AIR_N_LOOP_COLL: u32 = 0x800F_CAA4;
    pub const AIR_N_END_COLL: u32 = 0x800F_CB58;
}

/// Kirby's bow: his states and attributes by his copy, the arrow made on his bone 39 and the
/// bow on 44, the words cleared whatever they held, the draw from a table of zeros, and no
/// sword to put back.
pub const KIRBY: Bow = Bow {
    kind: KindFrom::Copy,
    states: [
        ms::LK_SPECIAL_N_START - lk::SPECIAL_N_START,
        ms::CL_SPECIAL_N_START - lk::SPECIAL_N_START,
    ],
    attrs: [0x268, 0x27C],
    u: 0x64,
    parts: false,
    arrow_bone: 39,
    bow_bone: 44,
    on_death: super::addr::DEATH3,
    on_damage: super::addr::DEATH1,
    checks_out: false,
    draw_from_zeros: true,
    sword_parts: false,
};

impl Running<'_> {
    /// The copy's cleanup when hit or KO'd: the arrow and the bow taken away.
    pub(crate) fn kb_lk_cleanup(&mut self, gobj: GObjId) {
        self.link_process_arrow(gobj, &KIRBY);
        self.link_process_bow(gobj, &KIRBY);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    let p = &KIRBY;
    match a {
        UNSET_ARROW => g.link_unset_arrow(gobj, p),
        UNSET_BOW => g.link_unset_bow(gobj, p),
        LOOSE => g.link_loose(gobj, p),
        REMOVE_ARROW => g.link_process_arrow(gobj, p),
        REMOVE_BOW => g.link_process_bow(gobj, p),
        ENTER => g.link_special_n_enter(gobj, p, false),
        AIR_ENTER => g.link_special_n_enter(gobj, p, true),
        N_START_ANIM => g.link_special_n_start_anim(gobj, p, false),
        AIR_N_START_ANIM => g.link_special_n_start_anim(gobj, p, true),
        N_LOOP_ANIM | AIR_N_LOOP_ANIM => g.link_special_n_loop_anim(gobj, p),
        N_END_ANIM => g.link_special_n_end_anim(gobj, p, false),
        AIR_N_END_ANIM => g.link_special_n_end_anim(gobj, p, true),
        N_START_IASA => g.link_special_n_start_iasa(gobj, p, false),
        AIR_N_START_IASA => g.link_special_n_start_iasa(gobj, p, true),
        N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, p, false),
        AIR_N_LOOP_IASA => g.link_special_n_loop_iasa(gobj, p, true),
        N_END_IASA | AIR_N_END_IASA => {}
        N_START_PHYS | N_LOOP_PHYS | N_END_PHYS => g.link_special_n_phys(gobj, false),
        AIR_N_START_PHYS | AIR_N_LOOP_PHYS | AIR_N_END_PHYS => g.link_special_n_phys(gobj, true),
        N_START_COLL => g.link_special_n_coll(gobj, p, lk::SPECIAL_N_START),
        N_LOOP_COLL => g.link_special_n_coll(gobj, p, lk::SPECIAL_N_LOOP),
        N_END_COLL => g.link_special_n_coll(gobj, p, lk::SPECIAL_N_END),
        AIR_N_START_COLL => g.link_special_air_n_coll(gobj, p, lk::SPECIAL_AIR_N_START),
        AIR_N_LOOP_COLL => g.link_special_air_n_coll(gobj, p, lk::SPECIAL_AIR_N_LOOP),
        AIR_N_END_COLL => g.link_special_air_n_coll(gobj, p, lk::SPECIAL_AIR_N_END),
        _ => return false,
    }
    true
}
