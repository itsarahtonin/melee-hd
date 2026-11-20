//! Marth's and Roy's copies (`ftkirbyspecialmars.c`): Shield Breaker and Flare Blade as Kirby
//! swings them, with a sword of his own in his hand (an accessory from the copy's data).
//! Marth's own code ([`ft::mars::special_n`](super::super::mars::special_n)) over his states
//! and attributes for each copy, his callbacks and flashes, and three branches of his own
//! ([`KIRBY`]); the charge's frames are laid out as Marth's (`mv.ms.specialn`). Only his
//! copies' addresses and his sword are here.

use super::super::super::state::Running;
use super::super::mars::ms as mars;
use super::super::mars::special_n::ShieldBreaker;

use super::super::motion::flags::KEEP_ACCESSORY;
use super::copy::KindFrom;
use super::{ms, u};
use crate::fpu::mem;
use crate::game::ft::FighterKind;
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// The copies' function addresses.
pub(super) mod addr {
    pub const FLASH: u32 = 0x8010_B1F4;
    pub const RESET: u32 = 0x8010_B2E8;
    pub const ENTER: u32 = 0x8010_B2FC;
    pub const AIR_ENTER: u32 = 0x8010_B4A0;
    pub const N_START_ANIM: u32 = 0x8010_B65C;
    pub const AIR_N_START_ANIM: u32 = 0x8010_B6C4;
    pub const N_START_IASA: u32 = 0x8010_B72C;
    pub const AIR_N_START_IASA: u32 = 0x8010_B730;
    pub const N_START_PHYS: u32 = 0x8010_B734;
    pub const AIR_N_START_PHYS: u32 = 0x8010_B78C;
    pub const N_START_COLL: u32 = 0x8010_B7F0;
    pub const AIR_N_START_COLL: u32 = 0x8010_B82C;
    pub const START_TO_AIR: u32 = 0x8010_B868;
    pub const START_TO_GROUND: u32 = 0x8010_B8E0;
    pub const N_LOOP_ANIM: u32 = 0x8010_B958;
    pub const AIR_N_LOOP_ANIM: u32 = 0x8010_B9C0;
    pub const N_LOOP_IASA: u32 = 0x8010_BA28;
    pub const AIR_N_LOOP_IASA: u32 = 0x8010_BA60;
    pub const N_LOOP_PHYS: u32 = 0x8010_BA98;
    pub const AIR_N_LOOP_PHYS: u32 = 0x8010_BAB8;
    pub const N_LOOP_COLL: u32 = 0x8010_BAD8;
    pub const AIR_N_LOOP_COLL: u32 = 0x8010_BB14;
    pub const LOOP_TO_AIR: u32 = 0x8010_BB50;
    pub const LOOP_TO_GROUND: u32 = 0x8010_BBC8;
    pub const GROUND_LOOP: u32 = 0x8010_BC40;
    pub const AIR_LOOP: u32 = 0x8010_BC90;
    pub const N_END_ANIM: u32 = 0x8010_BCE0;
    pub const AIR_N_END_ANIM: u32 = 0x8010_BDD8;
    pub const N_END_IASA: u32 = 0x8010_BED0;
    pub const AIR_N_END_IASA: u32 = 0x8010_BED4;
    pub const N_END_PHYS: u32 = 0x8010_BED8;
    pub const AIR_N_END_PHYS: u32 = 0x8010_BEF8;
    pub const N_END_COLL: u32 = 0x8010_BF18;
    pub const AIR_N_END_COLL: u32 = 0x8010_BF54;
    pub const END_TO_AIR: u32 = 0x8010_BF90;
    pub const END_TO_GROUND: u32 = 0x8010_C06C;
    pub const SWING: u32 = 0x8010_C148;
    pub const AIR_SWING: u32 = 0x8010_C1E8;
}

/// Kirby's Shield Breaker and Flare Blade: his states and attributes by his copy, his own
/// callbacks, the copy's flashes, his sword kept through every change of state, no wind, one
/// flash callback for both swings, and the sword put in his hand at the draw.
pub const KIRBY: ShieldBreaker = ShieldBreaker {
    kind: KindFrom::Copy,
    states: [
        ms::MS_SPECIAL_N_START - mars::SPECIAL_N_START,
        ms::FE_SPECIAL_N_START - mars::SPECIAL_N_START,
    ],
    attrs: [0x35C, 0x370],
    reset: addr::RESET,
    flash: [addr::FLASH, addr::FLASH],
    gfx: [[0x4B4, 0x4B5], [0x4B8, 0x4B9]],
    keep: KEEP_ACCESSORY,
    wind: false,
    one_flash: true,
    sword: true,
};

/// `FtPart_RThumbNb`: the hand the sword is in.
const PART_R_THUMB_NB: i32 = 49;

impl Running<'_> {
    /// The sword in his hand at the draw (`setupStartAccessory`): the copy's own (its
    /// `hat_dynamics[0]`), at his model's scale, on his right hand.
    pub(crate) fn kb_ms_sword(&mut self, gobj: GObjId) {
        let kind = if self.game.fighter(gobj).u_i32(u::HAT_KIND) == FighterKind::Mars {
            FighterKind::Mars.id()
        } else {
            FighterKind::Emblem.id()
        };
        let copy = self.game.ft.kirby.copies[kind as usize].expect("a copy without its data");
        let sword = self.data.ptr(copy.at(0xC)).expect("the copy's sword");
        self.set_accessory(gobj, sword);
        let s = mem(Self::model_scale(self.game.fighter(gobj)));
        let acc = self.game.fighter(gobj).x20A0_accessory.expect("the sword");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, Vec3::new(s, s, s));
        let hand = self.part_joint(gobj, PART_R_THUMB_NB);
        crate::lb::anim::attach(&mut self.game.objs, Some(acc), hand);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let p = &KIRBY;
    match a {
        addr::FLASH => g.mars_special_n_flash(gobj, p, false),
        addr::RESET => g.mars_special_n_reset(gobj),
        addr::ENTER => g.mars_special_n_enter(gobj, p),
        addr::AIR_ENTER => g.mars_special_air_n_enter(gobj, p),
        addr::N_START_ANIM => g.mars_special_n_start_anim(gobj, p, false),
        addr::AIR_N_START_ANIM => g.mars_special_n_start_anim(gobj, p, true),
        addr::N_START_IASA | addr::AIR_N_START_IASA | addr::N_END_IASA | addr::AIR_N_END_IASA => {
            g.mars_no_iasa(gobj)
        }
        addr::N_START_PHYS => g.mars_special_n_start_phys(gobj, p),
        addr::AIR_N_START_PHYS => g.mars_special_air_n_start_phys(gobj, p),
        addr::N_START_COLL => g.mars_special_n_start_coll(gobj, p),
        addr::AIR_N_START_COLL => g.mars_special_air_n_start_coll(gobj, p),
        addr::START_TO_AIR => g.mars_special_n_start_ground_to_air(gobj, p),
        addr::START_TO_GROUND => g.mars_special_n_start_air_to_ground(gobj, p),
        addr::N_LOOP_ANIM => g.mars_special_n_charge(gobj, p, false),
        addr::AIR_N_LOOP_ANIM => g.mars_special_n_charge(gobj, p, true),
        addr::N_LOOP_IASA => g.mars_special_n_release(gobj, p, false),
        addr::AIR_N_LOOP_IASA => g.mars_special_n_release(gobj, p, true),
        addr::N_LOOP_PHYS | addr::N_END_PHYS => g.mars_ground_phys(gobj),
        addr::AIR_N_LOOP_PHYS | addr::AIR_N_END_PHYS => g.mars_air_phys(gobj),
        addr::N_LOOP_COLL => g.mars_special_n_loop_coll(gobj, p),
        addr::AIR_N_LOOP_COLL => g.mars_special_air_n_loop_coll(gobj, p),
        addr::LOOP_TO_AIR => g.mars_special_n_loop_ground_to_air(gobj, p),
        addr::LOOP_TO_GROUND => g.mars_special_n_loop_air_to_ground(gobj, p),
        addr::GROUND_LOOP => g.mars_special_n_loop(gobj, p, false),
        addr::AIR_LOOP => g.mars_special_n_loop(gobj, p, true),
        addr::N_END_ANIM => g.mars_special_n_swing_anim(gobj, p, false),
        addr::AIR_N_END_ANIM => g.mars_special_n_swing_anim(gobj, p, true),
        addr::N_END_COLL => g.mars_special_n_end_coll(gobj, p),
        addr::AIR_N_END_COLL => g.mars_special_air_n_end_coll(gobj, p),
        addr::END_TO_AIR => g.mars_special_n_end_ground_to_air(gobj, p),
        addr::END_TO_GROUND => g.mars_special_n_end_air_to_ground(gobj, p),
        addr::SWING => g.mars_special_n_swing(gobj, p, false),
        addr::AIR_SWING => g.mars_special_n_swing(gobj, p, true),
        _ => return false,
    }
    true
}
