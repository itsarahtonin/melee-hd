//! Bowser Bomb, the down special (`ftkoopaspeciallw.c`): a hop on the ground (or a slowed
//! start in the air), then a fast fall that lands in a quake, with its fall's and landing's
//! effects.

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::{addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The flags the fall keeps from the hop (`0x0C4C5088`).
const HOP_TO_FALL: u32 = 0x0C4C_5088;

impl Running<'_> {
    /// The landing's effect at his feet, once (`fn_80134518`, the landing's accessory).
    #[doc(alias = "fn_80134518")]
    pub fn kp_lw_landing_effect(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let pos = self.game.fighter(gobj).cur_pos;
            ef::spawn(self, 0x4D8, Some(gobj), &[ef::Arg::Vec(pos)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The fall's effect on his top joint, once (`fn_80134590`, the fall's accessory).
    #[doc(alias = "fn_80134590")]
    pub fn kp_lw_fall_effect(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let top = self.bone_joint(gobj, 0);
            ef::spawn(self, 0x4DF, Some(gobj), &[ef::Arg::JObj(top)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The bomb's variables at its start (`ftKp_SpecialLw_Enter_inline`): no fall yet, no
    /// throw flag, no rise, and `x2223_b4`.
    fn kp_lw_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_throw_flag(0, false);
        fp.x6A4_transNOffset.y = 0.0;
        fp.x2223_b4 = true;
    }

    /// `ftKp_SpecialLw_Enter`: still, the hop starts in the air.
    #[doc(alias = "ftKp_SpecialLw_Enter")]
    pub fn kp_special_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kp_lw_reset(gobj);
    }

    /// `ftKp_SpecialAirLw_Enter`: his speed cut by `x80` across and `x84` up.
    #[doc(alias = "ftKp_SpecialAirLw_Enter")]
    pub fn kp_special_air_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (x80, x84) = (
            reg(self.kp_attr_f32(f, attr::X80)),
            reg(self.kp_attr_f32(f, attr::X84)),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), x80));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), x84));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.kp_lw_reset(gobj);
    }

    /// `ftKp_SpecialLw_Anim`: the hop over, the fall from its frame 30.
    #[doc(alias = "ftKp_SpecialLw_Anim")]
    pub fn kp_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.kp_lw_to_air(gobj);
        }
    }

    /// `ftKp_SpecialAirLw_Anim`: past its animation the fall holds its speed (`cmd_vars[1]`).
    #[doc(alias = "ftKp_SpecialAirLw_Anim")]
    pub fn kp_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).cmd_vars[1] = 1;
        }
    }

    /// `ftKp_SpecialLw_Phys`: the animation's motion, never down.
    #[doc(alias = "ftKp_SpecialLw_Phys")]
    pub fn kp_special_lw_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.self_vel.y < 0.0 {
            fp.self_vel.y = 0.0;
        }
    }

    /// `ftKp_SpecialAirLw_Phys`: falling by `x8C` to `x90` with the drift slowing by `x88`,
    /// at least `x94` and at it once the animation is over, when the drift stops and the
    /// fall's effect comes.
    #[doc(alias = "ftKp_SpecialAirLw_Phys")]
    pub fn kp_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x8c = reg(self.kp_attr_f32(f, attr::X8C));
        let x90 = reg(self.kp_attr_f32(f, attr::X90));
        let x88 = reg(self.kp_attr_f32(f, attr::X88));
        let x94 = self.kp_attr_f32(f, attr::X94);
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, x8c, x90);
        Self::self_accel_deaccel(fp, x88);
        if fp.self_vel.y < x94 || fp.cmd_vars[1] != 0 {
            fp.self_vel.y = x94;
        }
        if fp.cmd_vars[1] != 0 {
            fp.self_vel.x = 0.0;
            if !fp.x2219_b0 {
                fp.accessory4_cb = Some(CodeRef(addr::LW_FALL_EFFECT));
            }
        }
    }

    /// `ftKp_SpecialAirLw_Coll` (and `ftKp_SpecialLw_Coll`): on the ground once falling it
    /// lands, before that it stays airborne, unlocked; off the ground, falling, a ledge.
    #[doc(alias = "ftKp_SpecialAirLw_Coll")]
    #[doc(alias = "ftKp_SpecialLw_Coll")]
    pub fn kp_special_air_lw_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            let fp = &self.game.fighters[f.0];
            if fp.cmd_vars[0] != 0 && fp.self_vel.y <= 0.0 {
                self.kp_lw_landing_enter(gobj);
            } else {
                self.go_airborne(f);
                super::super::fighter::unlock_ecb(&mut self.game.fighters[f.0]);
            }
        } else if self.game.fighters[f.0].cmd_vars[0] != 0 && self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
        }
    }

    /// The fall from the hop's end, `ftKp_SpecialLw_80134988`: the aerial state from its frame
    /// 30, keeping the hop's hitboxes, colors and models.
    #[doc(alias = "ftKp_SpecialLw_80134988")]
    pub fn kp_lw_to_air(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, HOP_TO_FALL, 30.0, 1.0, 0.0, None);
    }

    /// `ftKp_SpecialLwLanding_Anim`: waiting once it plays out.
    #[doc(alias = "ftKp_SpecialLwLanding_Anim")]
    pub fn kp_special_lw_landing_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftKp_SpecialLwLanding_Coll`: falling if it went off the ground.
    #[doc(alias = "ftKp_SpecialLwLanding_Coll")]
    pub fn kp_special_lw_landing_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The landing, `ftKp_SpecialLw_80134A5C`: still, grounded, its effect on the next frame.
    #[doc(alias = "ftKp_SpecialLw_80134A5C")]
    pub fn kp_lw_landing_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).gr_vel = 0.0;
        self.land(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_LW_LANDING, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_LANDING_EFFECT));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_LANDING_EFFECT => g.kp_lw_landing_effect(gobj),
        addr::LW_FALL_EFFECT => g.kp_lw_fall_effect(gobj),
        addr::SPECIAL_LW_ENTER => g.kp_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.kp_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.kp_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.kp_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_PHYS => g.kp_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.kp_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL | addr::SPECIAL_AIR_LW_COLL => g.kp_special_air_lw_coll(gobj),
        addr::LW_FALL => g.kp_lw_to_air(gobj),
        addr::SPECIAL_LW_LANDING_ANIM => g.kp_special_lw_landing_anim(gobj),
        addr::SPECIAL_LW_LANDING_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_LW_LANDING_COLL => g.kp_special_lw_landing_coll(gobj),
        addr::LW_LANDING => g.kp_lw_landing_enter(gobj),
        _ => return false,
    }
    true
}
