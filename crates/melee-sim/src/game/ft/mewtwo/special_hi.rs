//! Teleport (`ftmewtwospecialhi.c`): the wind-up, the invisible travel in the stick's
//! direction (along the floor on the ground), and the reappearance. The move is Zelda's
//! Farore's Wind over Mewtwo's [`MEWTWO`] ([`Teleport`]), with his own effect and sound.

use super::super::super::ef::{Arg, spawn};
use super::super::super::state::Running;
use super::super::zelda::special_hi::{Teleport, Windup};
use super::{PART_WAIST_N, addr, attr, ms};
use crate::hsd::gobj::GObjId;

/// Mewtwo's own: his attributes in Zelda's layout from `HI_VEL_DIV_X` on.
pub const MEWTWO: Teleport = Teleport {
    states: ms::SPECIAL_HI_START,
    attrs: attr::HI_VEL_DIV_X,
    start_cb: addr::HI_CREATE_GFX,
    end_cb: addr::HI_SET_END_GFX,
    windup: Windup::Slowed,
    wind: false,
    vanish_at_travel: false,
    travel_sfx: Some(SFX_TRAVEL),
    end_to_air_cb: false,
    appear_part: PART_WAIST_N,
    appear_gfx: None,
    appear_pos_first: false,
};

/// The wind-up's effect, and the travel's sound.
const GFX_START: i32 = 0x4E8;
const SFX_TRAVEL: i32 = 0x30DA1;

impl Running<'_> {
    /// `ftMt_SpecialHi_CreateGFX` (an accessory): the wind-up's effect, then no more
    /// accessory.
    #[doc(alias = "ftMt_SpecialHi_CreateGFX")]
    pub fn mt_hi_start_gfx_once(&mut self, gobj: GObjId) {
        self.mt_hi_start_gfx(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The wind-up's effect at his waist, once (`x2219_b0`), `ftMt_SpecialHi_SetStartGFX`.
    #[doc(alias = "ftMt_SpecialHi_SetStartGFX")]
    pub fn mt_hi_start_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).effect_once {
            let pos = self.teleport_part_pos(gobj, PART_WAIST_N);
            spawn(self, GFX_START, Some(gobj), &[Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &MEWTWO;
    match a {
        addr::HI_CREATE_GFX => g.mt_hi_start_gfx_once(gobj),
        addr::HI_SET_START_GFX => g.mt_hi_start_gfx(gobj),
        addr::HI_SET_END_GFX => g.teleport_appear_gfx(gobj, t),
        addr::SPECIAL_HI_START_ENTER => g.teleport_enter(gobj, t),
        addr::SPECIAL_AIR_HI_START_ENTER => g.teleport_air_enter(gobj, t),
        addr::SPECIAL_HI_START_ANIM => g.teleport_start_anim(gobj, t),
        addr::SPECIAL_AIR_HI_START_ANIM => g.teleport_air_start_anim(gobj, t),
        addr::SPECIAL_HI_START_IASA
        | addr::SPECIAL_AIR_HI_START_IASA
        | addr::SPECIAL_HI_LOST_IASA
        | addr::SPECIAL_AIR_HI_LOST_IASA
        | addr::SPECIAL_AIR_HI_LOST_PHYS
        | addr::SPECIAL_HI_IASA
        | addr::SPECIAL_AIR_HI_IASA => g.teleport_nothing(gobj),
        addr::SPECIAL_HI_START_PHYS | addr::SPECIAL_HI_PHYS => g.mt_ground_phys(gobj),
        addr::SPECIAL_AIR_HI_START_PHYS => g.teleport_air_start_phys(gobj, t),
        addr::SPECIAL_HI_START_COLL => g.teleport_start_coll(gobj, t),
        addr::SPECIAL_AIR_HI_START_COLL => g.teleport_air_start_coll(gobj, t),
        addr::SPECIAL_HI_START_GROUND_TO_AIR => g.teleport_start_to_air(gobj, t),
        addr::SPECIAL_AIR_HI_START_AIR_TO_GROUND => g.teleport_start_to_ground(gobj, t),
        addr::SPECIAL_HI_LOST_ANIM => g.teleport_travel_anim(gobj, t),
        addr::SPECIAL_AIR_HI_LOST_ANIM => g.teleport_air_travel_anim(gobj, t),
        addr::SPECIAL_HI_LOST_PHYS => g.teleport_travel_phys(gobj),
        addr::SPECIAL_HI_LOST_COLL => g.teleport_travel_coll(gobj, t),
        addr::SPECIAL_AIR_HI_LOST_COLL => g.teleport_air_travel_coll(gobj, t),
        addr::SPECIAL_HI_GROUND_TO_AIR => g.teleport_travel_to_air(gobj, t),
        addr::SPECIAL_AIR_HI_AIR_TO_GROUND => g.teleport_travel_to_ground(gobj, t),
        addr::SPECIAL_HI_ENTER => g.teleport_travel_ground(gobj, t),
        addr::SPECIAL_AIR_HI_ENTER => g.teleport_travel_air(gobj, t),
        addr::SPECIAL_HI_ANIM => g.teleport_end_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.teleport_air_end_anim(gobj, t),
        addr::SPECIAL_AIR_HI_PHYS => g.teleport_air_end_phys(gobj, t),
        addr::SPECIAL_HI_COLL => g.teleport_end_coll(gobj, t),
        addr::SPECIAL_AIR_HI_COLL => g.teleport_air_end_coll(gobj, t),
        addr::SPECIAL_HI_LOST_GROUND_TO_AIR => g.teleport_end_to_air(gobj, t),
        addr::SPECIAL_HI_LOST_ENTER => g.teleport_appear_ground(gobj, t),
        addr::SPECIAL_AIR_HI_LOST_ENTER => g.teleport_appear_air(gobj, t),
        _ => return false,
    }
    true
}
