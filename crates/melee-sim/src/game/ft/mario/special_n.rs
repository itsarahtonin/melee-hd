//! Mario's Fireball and Dr. Mario's Megavitamin (`ftmariospecialn.c`): thrown from the hand
//! when the script raises its flag, on the ground or in the air. Dr. Mario's pill takes a
//! random color combination other than his last two.

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{drmariopill, mariofireball};
use super::super::super::state::Running;
use super::init::IT_KIND_MARIO_FIRE;
use super::{KIND_MARIO, addr, ms, part, u};
use crate::hsd::gobj::GObjId;

/// `It_Kind_DrMario_Vitamin`.
const IT_KIND_DRMARIO_VITAMIN: i32 = 49;

/// The flags the throw keeps going between the ground and the air.
const COLL_FLAGS: u32 = 0x5000;

impl Running<'_> {
    /// `ftMr_SpecialN_VitaminRandom`: a pill color combination (0 to 8) other than the current
    /// and the last (`HSD_Randi` over those left), which becomes the current.
    // Ports `ftMr_SpecialN_VitaminRandom`; aliased once a check reaches it.
    pub fn mr_special_n_vitamin_random(&mut self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        let (curr, prev) = (fp.u_i32(u::VITAMIN_CURR), fp.u_i32(u::VITAMIN_PREV));
        let mut left = [0i32; 9];
        let mut n = 0;
        for i in 0..9 {
            if i != curr && i != prev {
                left[n as usize] = i;
                n += 1;
            }
        }
        let pick = left[self.game.objs.rng.below(n) as usize];
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::VITAMIN_PREV, curr);
        fp.set_u_i32(u::VITAMIN_CURR, pick);
        pick
    }

    fn mr_special_n_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_ITEM_FIRE_SPAWN));
    }

    /// `ftMr_SpecialN_Enter`.
    // Ports `ftMr_SpecialN_Enter`; aliased once a check reaches it.
    pub fn mr_special_n_enter(&mut self, gobj: GObjId) {
        self.mr_special_n_start(gobj, ms::SPECIAL_N);
    }

    /// `ftMr_SpecialAirN_Enter`.
    // Ports `ftMr_SpecialAirN_Enter`; aliased once a check reaches it.
    pub fn mr_special_air_n_enter(&mut self, gobj: GObjId) {
        self.mr_special_n_start(gobj, ms::SPECIAL_AIR_N);
    }

    /// `ftMr_SpecialN_Anim`: waiting when the throw ends.
    // Ports `ftMr_SpecialN_Anim`; aliased once a check reaches it.
    pub fn mr_special_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftMr_SpecialAirN_Anim`: a fall when the throw ends.
    // Ports `ftMr_SpecialAirN_Anim`; aliased once a check reaches it.
    pub fn mr_special_air_n_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftMr_SpecialN_IASA`: once the script allows, what waiting allows.
    // Ports `ftMr_SpecialN_IASA`; aliased once a check reaches it.
    pub fn mr_special_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_wait_iasa(gobj);
        }
    }

    /// `ftMr_SpecialAirN_IASA`: once the script allows, what falling allows.
    // Ports `ftMr_SpecialAirN_IASA`; aliased once a check reaches it.
    pub fn mr_special_air_n_iasa(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.co_fall_iasa(gobj);
        }
    }

    /// `ftMr_SpecialN_Phys`: friction.
    // Ports `ftMr_SpecialN_Phys`; aliased once a check reaches it.
    pub fn mr_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftMr_SpecialAirN_Phys`: the air's physics.
    // Ports `ftMr_SpecialAirN_Phys`; aliased once a check reaches it.
    pub fn mr_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftMr_SpecialN_Coll`.
    // Ports `ftMr_SpecialN_Coll`; aliased once a check reaches it.
    pub fn mr_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mr_special_n_ground_to_air(gobj);
        }
    }

    /// `ftMr_SpecialAirN_Coll`.
    // Ports `ftMr_SpecialAirN_Coll`; aliased once a check reaches it.
    pub fn mr_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mr_special_air_n_air_to_ground(gobj);
        }
    }

    /// `ftMr_SpecialN_GroundToAir`: the aerial throw from the same frame.
    // Ports `ftMr_SpecialN_GroundToAir`; aliased once a check reaches it.
    pub fn mr_special_n_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_ITEM_FIRE_SPAWN));
    }

    /// `ftMr_SpecialAirN_AirToGround`: the grounded throw from the same frame.
    // Ports `ftMr_SpecialAirN_AirToGround`; aliased once a check reaches it.
    pub fn mr_special_air_n_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_N_ITEM_FIRE_SPAWN));
    }

    /// `ftMr_SpecialN_ItemFireSpawn`: when the script raises the throw flag, from the hand
    /// (`FtPart_L1stNb`), Mario's fireball and its flash, or Dr. Mario's pill.
    // Ports `ftMr_SpecialN_ItemFireSpawn`; aliased once a check reaches it.
    pub fn mr_special_n_item_fire_spawn(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        let hand = self.part_joint(gobj, part::L_1ST_NB);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, hand, None);
        let fp = self.game.fighter(gobj);
        let facing = fp.facing_dir;
        if fp.kind == KIND_MARIO {
            mariofireball::spawn(self, gobj, pos, IT_KIND_MARIO_FIRE, facing);
            let hand = self.part_joint(gobj, part::L_1ST_NB);
            let facing = self.game.fighter(gobj).facing_dir;
            spawn(
                self,
                0x47A,
                Some(gobj),
                &[Arg::JObj(hand), Arg::Float(facing)],
            );
        } else {
            let color = self.mr_special_n_vitamin_random(gobj);
            drmariopill::spawn(self, gobj, pos, color, IT_KIND_DRMARIO_VITAMIN, facing);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.mr_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.mr_special_air_n_enter(gobj),
        addr::SPECIAL_N_ANIM => g.mr_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.mr_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.mr_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.mr_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.mr_special_n_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.mr_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.mr_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.mr_special_air_n_coll(gobj),
        addr::SPECIAL_N_ITEM_FIRE_SPAWN => g.mr_special_n_item_fire_spawn(gobj),
        _ => return false,
    }
    true
}
