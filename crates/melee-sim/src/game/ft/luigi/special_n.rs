//! Luigi's Fireball (`ftluigispecialn.c`): a fireball thrown from his hand when the script
//! raises its flag, on the ground or in the air.

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::luigifireball;
use super::super::super::state::Running;
use super::init::IT_KIND_LUIGI_FIRE;
use super::{addr, ms, part};
use crate::hsd::gobj::GObjId;

/// The flags the throw keeps going between the ground and the air (`ftLg_MF_SpecialN_Coll`).
const COLL_FLAGS: u32 = 0x5000;

impl Running<'_> {
    fn lg_special_n_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_FIRE_SPAWN));
    }

    /// `ftLg_SpecialN_Enter`.
    #[doc(alias = "ftLg_SpecialN_Enter")]
    pub fn lg_special_n_enter(&mut self, gobj: GObjId) {
        self.lg_special_n_start(gobj, ms::SPECIAL_N);
    }

    /// `ftLg_SpecialAirN_Enter`.
    #[doc(alias = "ftLg_SpecialAirN_Enter")]
    pub fn lg_special_air_n_enter(&mut self, gobj: GObjId) {
        self.lg_special_n_start(gobj, ms::SPECIAL_AIR_N);
    }

    /// `ftLg_SpecialN_Anim`: waiting when the throw ends.
    #[doc(alias = "ftLg_SpecialN_Anim")]
    pub fn lg_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftLg_SpecialAirN_Anim`: a fall when the throw ends.
    #[doc(alias = "ftLg_SpecialAirN_Anim")]
    pub fn lg_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftLg_SpecialN_IASA`: once the script allows (`cmd_vars[0]`), what waiting allows.
    #[doc(alias = "ftLg_SpecialN_IASA")]
    pub fn lg_special_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftLg_SpecialAirN_IASA`: once the script allows, what falling allows.
    #[doc(alias = "ftLg_SpecialAirN_IASA")]
    pub fn lg_special_air_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftLg_SpecialN_Coll`: off a ledge, the aerial throw from the same frame.
    #[doc(alias = "ftLg_SpecialN_Coll")]
    pub fn lg_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_FIRE_SPAWN));
        }
    }

    /// `ftLg_SpecialAirN_Coll`: landing, the grounded throw from the same frame.
    #[doc(alias = "ftLg_SpecialAirN_Coll")]
    pub fn lg_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ms::SPECIAL_N, COLL_FLAGS);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_FIRE_SPAWN));
        }
    }

    /// `ftLg_SpecialN_FireSpawn`: when the script raises the throw flag, the fireball from his
    /// hand (`FtPart_L1stNb`) and its flash.
    #[doc(alias = "ftLg_SpecialN_FireSpawn")]
    pub fn lg_special_n_fire_spawn(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, part::L_1ST_NB);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let facing = self.game.fighter(gobj).facing_dir;
        luigifireball::spawn(self, gobj, pos, IT_KIND_LUIGI_FIRE, facing);
        let hand = self.part_joint(gobj, part::L_1ST_NB);
        let facing = self.game.fighter(gobj).facing_dir;
        spawn(
            self,
            0x507,
            Some(gobj),
            &[Arg::JObj(hand), Arg::Float(facing)],
        );
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.lg_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.lg_special_air_n_enter(gobj),
        addr::SPECIAL_N_ANIM => g.lg_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.lg_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.lg_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.lg_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.lg_ground_friction(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.lg_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.lg_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.lg_special_air_n_coll(gobj),
        addr::SPECIAL_N_FIRE_SPAWN => g.lg_special_n_fire_spawn(gobj),
        _ => return false,
    }
    true
}
