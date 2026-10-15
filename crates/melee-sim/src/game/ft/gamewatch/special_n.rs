//! Chef (`ftgamewatchspecialn.c`): each flip of the pan throws a sausage
//! (`it::kinds::gamewatchchef`), one of five other than the last two, and B pressed again (or
//! held) loops the flip back, up to the attributes' most.

use gekko_fp as fp;

use super::super::super::it::kinds::{self, gamewatchchef as chef};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{addr, attr, ms, mv, part, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

/// What Chef keeps when it changes between the ground and the air, or loops.
const KEEP_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MODEL
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// Whether he has thrown fewer sausages than the attributes allow.
    fn gw_chef_more(&self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_SAUSAGES);
        fp::frsp(f64::from(n)) < reg(self.gw_attr_f32(f, attr::CHEF_MAX))
    }

    /// A sausage at the script's mark, `ftGw_SpecialN_CreateSausage`: one of the five but the
    /// last two at random, from his thumb.
    #[doc(alias = "ftGw_SpecialN_CreateSausage")]
    pub fn gw_create_sausage(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            return;
        }
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
        if self.gw_chef_more(gobj) {
            let fp = self.game.fighter_mut(gobj);
            let n = fp.mv_i32(mv::N_SAUSAGES);
            fp.set_mv_i32(mv::N_SAUSAGES, n.wrapping_add(1));
            let bone = self.bone_index(self.game.fighter(gobj), part::L_THUMB_NB);
            let pos = self.gw_part_pos(gobj, bone, Some(Vec3::new(2.5, 6.5, 0.0)));
            let fp = self.game.fighter(gobj);
            let (last, before) = (fp.u_i32(u::CHEF_VAR1), fp.u_i32(u::CHEF_VAR2));
            let choices: Vec<i32> = (0..5).filter(|&i| i != last && i != before).collect();
            let pick = self.game.objs.rng.below(choices.len() as i32);
            let sausage = choices[pick as usize];
            let fp = self.game.fighter_mut(gobj);
            fp.set_u_i32(u::CHEF_VAR2, last);
            fp.set_u_i32(u::CHEF_VAR1, sausage);
            let facing = fp.facing_dir;
            chef::spawn(self, gobj, pos, kinds::GAME_WATCH_CHEF, sausage, facing);
        }
        self.gw_set_accessory(gobj, None);
    }

    /// Chef's start (`ftGameWatch_SpecialN_SetVars`).
    fn gw_special_n_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        fp.set_mv_i32(mv::N_SAUSAGES, 0);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_N_CREATE_SAUSAGE));
    }

    /// `ftGw_SpecialN_Enter`.
    #[doc(alias = "ftGw_SpecialN_Enter")]
    pub fn gw_special_n_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_n_start(gobj);
    }

    /// `ftGw_SpecialAirN_Enter`.
    #[doc(alias = "ftGw_SpecialAirN_Enter")]
    pub fn gw_special_air_n_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.gw_special_n_start(gobj);
    }

    /// The loop at the script's mark (`ftGameWatch_SpecialN_ChefLoop`, its air copy testing
    /// in the other order): B still held and sausages left.
    fn gw_chef_loop_mark(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.cmd_vars[2] == 0 {
            return;
        }
        fp.cmd_vars[2] = 0;
        let held = fp.mv_i32(mv::N_LOOP_DISABLE) == 0;
        if held && self.gw_chef_more(gobj) {
            let f = self.game.fighter_id(gobj);
            let frame = self.gw_attr_f32(f, attr::CHEF_LOOPFRAME);
            if air {
                self.gw_special_air_n_loop(gobj, frame);
            } else {
                self.gw_special_n_loop(gobj, frame);
            }
        }
    }

    /// `ftGw_SpecialN_Anim`: the loop's mark; waiting once its frames run out.
    #[doc(alias = "ftGw_SpecialN_Anim")]
    pub fn gw_special_n_anim(&mut self, gobj: GObjId) {
        self.gw_chef_loop_mark(gobj, false);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftGw_SpecialAirN_Anim`: falling once its frames run out.
    #[doc(alias = "ftGw_SpecialAirN_Anim")]
    pub fn gw_special_air_n_anim(&mut self, gobj: GObjId) {
        self.gw_chef_loop_mark(gobj, true);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// Chef's interrupts (`ftGw_SpecialN_IASA`, `ftGw_SpecialAirN_IASA`): B let go of ends the
    /// loop at the mark; B pressed again after the script allows it loops now.
    fn gw_chef_iasa(&mut self, gobj: GObjId, air: bool) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::N_LOOP_DISABLE, 1);
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[1] != 0 && fp.input.pressed_buttons & PAD_B != 0 && self.gw_chef_more(gobj) {
            let f = self.game.fighter_id(gobj);
            let frame = self.gw_attr_f32(f, attr::CHEF_LOOPFRAME);
            if air {
                self.gw_special_air_n_loop(gobj, frame);
            } else {
                self.gw_special_n_loop(gobj, frame);
            }
        }
    }

    /// `ftGw_SpecialN_IASA`.
    #[doc(alias = "ftGw_SpecialN_IASA")]
    pub fn gw_special_n_iasa(&mut self, gobj: GObjId) {
        self.gw_chef_iasa(gobj, false);
    }

    /// `ftGw_SpecialAirN_IASA`.
    #[doc(alias = "ftGw_SpecialAirN_IASA")]
    pub fn gw_special_air_n_iasa(&mut self, gobj: GObjId) {
        self.gw_chef_iasa(gobj, true);
    }

    /// `ftGw_SpecialN_Phys`: friction (`ft_80084F3C`).
    #[doc(alias = "ftGw_SpecialN_Phys")]
    pub fn gw_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// `ftGw_SpecialAirN_Phys`: the air's, without drift (`ft_80084EEC`).
    #[doc(alias = "ftGw_SpecialAirN_Phys")]
    pub fn gw_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftGw_SpecialN_Coll`: off a ledge, into the air.
    #[doc(alias = "ftGw_SpecialN_Coll")]
    pub fn gw_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.gw_special_n_to_air(gobj);
        }
    }

    /// `ftGw_SpecialAirN_Coll`: landing, onto the ground.
    #[doc(alias = "ftGw_SpecialAirN_Coll")]
    pub fn gw_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.gw_special_air_n_to_ground(gobj);
        }
    }

    /// `ftGw_SpecialN_GroundToAir`.
    #[doc(alias = "ftGw_SpecialN_GroundToAir")]
    pub fn gw_special_n_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, KEEP_FLAGS);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_N_CREATE_SAUSAGE));
    }

    /// `ftGw_SpecialAirN_AirToGround`.
    #[doc(alias = "ftGw_SpecialAirN_AirToGround")]
    pub fn gw_special_air_n_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N, KEEP_FLAGS);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_N_CREATE_SAUSAGE));
    }

    /// Chef from `frame` again in state `msid` (`ftGw_SpecialN_Loop`, `ftGw_SpecialAirN_Loop`).
    fn gw_chef_loop(&mut self, gobj: GObjId, msid: i32, frame: f32) {
        let start = mem(fp::fsubs(reg(frame), 1.0));
        self.change_motion_state(gobj, msid, KEEP_FLAGS, start, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::N_LOOP_DISABLE, 0);
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_N_CREATE_SAUSAGE));
    }

    /// `ftGw_SpecialN_Loop`.
    #[doc(alias = "ftGw_SpecialN_Loop")]
    pub fn gw_special_n_loop(&mut self, gobj: GObjId, frame: f32) {
        self.gw_chef_loop(gobj, ms::SPECIAL_N, frame);
    }

    /// `ftGw_SpecialAirN_Loop`.
    #[doc(alias = "ftGw_SpecialAirN_Loop")]
    pub fn gw_special_air_n_loop(&mut self, gobj: GObjId, frame: f32) {
        self.gw_chef_loop(gobj, ms::SPECIAL_AIR_N, frame);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_CREATE_SAUSAGE => g.gw_create_sausage(gobj),
        addr::SPECIAL_N_ENTER => g.gw_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.gw_special_air_n_enter(gobj),
        addr::SPECIAL_N_ANIM => g.gw_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.gw_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA => g.gw_special_n_iasa(gobj),
        addr::SPECIAL_AIR_N_IASA => g.gw_special_air_n_iasa(gobj),
        addr::SPECIAL_N_PHYS => g.gw_special_n_phys(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.gw_special_air_n_phys(gobj),
        addr::SPECIAL_N_COLL => g.gw_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.gw_special_air_n_coll(gobj),
        addr::SPECIAL_N_GROUND_TO_AIR => g.gw_special_n_to_air(gobj),
        addr::SPECIAL_AIR_N_AIR_TO_GROUND => g.gw_special_air_n_to_ground(gobj),
        _ => return false,
    }
    true
}
