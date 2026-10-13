//! Falcon Kick and Wizard's Foot (`ftcaptainspeciallw.c`): the flaming kick along the floor
//! or down through the air, slowing with each hit, its landing, and the rebound off a wall
//! (`ftCa_MS_SpecialHiThrow1`).

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, SpawnKind};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::{addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `Collide_LeftWallHug` and `Collide_RightWallHug` in `CollData::env_flags`.
const LEFT_WALL_HUG: i32 = 0x20;
const RIGHT_WALL_HUG: i32 = 0x800;

impl Running<'_> {
    /// The kick's flame when the script raises its flag (`ftCa_SpecialHi_800E3EAC`): the
    /// first time, on the kicking foot, tilted by `speciallw_flame_particle_angle` in the air;
    /// again, its effects end.
    ///
    /// The original takes the foot and the tilt only in the kick's two states and otherwise
    /// reads a register and a stack word it never set; only those states' physics call it,
    /// so the port stops in any other.
    #[doc(alias = "ftCa_SpecialHi_800E3EAC")]
    pub fn ca_special_lw_flame(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if !fp.throw_flag(1) {
            return;
        }
        fp.set_throw_flag(1, false);
        if fp.x2219_b0 {
            self.destroy_effects(gobj);
            return;
        }
        let motion = fp.motion_id;
        let (angle, foot) = match motion {
            ms::SPECIAL_LW => (0.0, part::R_FOOT_JA),
            ms::SPECIAL_AIR_LW => {
                let a = reg(self.ca_attr_f32(f, attr::SPECIALLW_FLAME_PARTICLE_ANGLE));
                (mem(fp::fmuls(0.017_453_292_384_743_69, a)), part::L_FOOT_JA)
            }
            m => panic!(
                "ftCa_SpecialHi_800E3EAC in state {m} reads a register and a stack word it never set"
            ),
        };
        let bone = self.bone_index(&self.game.fighters[f.0], foot);
        if let Some(id) = self.ca_effect_id(gobj, 0x490, 0x50C) {
            let j = self.bone_joint(gobj, bone);
            let mut head = self.game.fighters[f.0].x60C;
            ef::queue(
                self,
                Some(gobj),
                &mut head,
                SpawnKind::AttachParam,
                id,
                j,
                &[Arg::Float(angle)],
            );
            self.game.fighters[f.0].x60C = head;
        }
        self.game.fighters[f.0].x2219_b0 = true;
    }

    /// The kick hit someone (`ftCa_SpecialHi_800E400C`, `deal_dmg_cb`): up to
    /// `speciallw_unk2` more times, it slows by `speciallw_on_hit_spd_modifier`.
    #[doc(alias = "ftCa_SpecialHi_800E400C")]
    pub fn ca_special_lw_on_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let limit = self.ca_attr_i32(f, attr::SPECIALLW_UNK2);
        let modifier = reg(self.ca_attr_f32(f, attr::SPECIALLW_ON_HIT_SPD_MODIFIER));
        let fp = &mut self.game.fighters[f.0];
        let x0 = u16::from_be_bytes(fp.mv[mv::LW_X0..mv::LW_X0 + 2].try_into().unwrap());
        if i32::from(x0) <= limit {
            fp.mv[mv::LW_X0..mv::LW_X0 + 2].copy_from_slice(&x0.wrapping_add(1).to_be_bytes());
            let friction = fp::fmuls(reg(fp.mv_f32(mv::LW_FRICTION)), modifier);
            fp.set_mv_f32(mv::LW_FRICTION, friction);
        }
    }

    /// `ftCa_SpecialLw_Enter`: no hits yet, full speed, the hit hook.
    #[doc(alias = "ftCa_SpecialLw_Enter")]
    pub fn ca_special_lw_enter(&mut self, gobj: GObjId) {
        self.ca_reset_cmd_and_throw(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.mv[mv::LW_X0..mv::LW_X0 + 2].copy_from_slice(&[0, 0]);
        fp.set_mv_f32(mv::LW_FRICTION, 1.0);
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).deal_dmg_cb = Some(CodeRef(addr::LW_ON_HIT));
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `ftCa_SpecialAirLw_Enter`.
    #[doc(alias = "ftCa_SpecialAirLw_Enter")]
    pub fn ca_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.ca_reset_cmd_and_throw(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `ftCa_SpecialLw_Anim`: at the kick's end, its end on the ground (slowed by
    /// `speciallw_ground_lag_mul`) or in the air.
    #[doc(alias = "ftCa_SpecialLw_Anim")]
    pub fn ca_special_lw_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let grounded = self.game.fighters[f.0].ground_or_air == GA_GROUND;
        self.ca_reset_cmd_and_throw(gobj);
        if grounded {
            let speed = self.ca_attr_f32(f, attr::SPECIALLW_GROUND_LAG_MUL);
            self.land(gobj);
            self.change_motion_state(gobj, ms::SPECIAL_LW_END, 0, 0.0, speed, 0.0, None);
        } else {
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::SPECIAL_LW_END_AIR, 0, 0.0, 1.0, 0.0, None);
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `ftCa_SpecialLwEnd_Anim` and `ftCa_SpecialLwEndAir_Anim`: waiting or falling at the
    /// end (`ftCommon_8007D92C`).
    #[doc(alias = "ftCa_SpecialLwEnd_Anim")]
    #[doc(alias = "ftCa_SpecialLwEndAir_Anim")]
    pub fn ca_special_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// `ftCa_SpecialAirLw_Anim`: at the kick's end, its end in the air.
    #[doc(alias = "ftCa_SpecialAirLw_Anim")]
    pub fn ca_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ca_reset_cmd_and_throw(gobj);
            let f = self.game.fighter_id(gobj);
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_END_AIR, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftCa_SpecialAirLwEnd_Anim`: waiting at the landing's end.
    #[doc(alias = "ftCa_SpecialAirLwEnd_Anim")]
    pub fn ca_special_air_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftCa_SpecialAirLwEndAir_Anim` and `ftCa_SpecialHiThrow1_Anim`: a fall at the end.
    #[doc(alias = "ftCa_SpecialAirLwEndAir_Anim")]
    #[doc(alias = "ftCa_SpecialHiThrow1_Anim")]
    pub fn ca_special_lw_fall_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The model tilted along the floor, `ftCommon_8007E5AC`; the original asserts the
    /// fighter is on the ground.
    #[doc(alias = "ftCommon_8007E5AC")]
    pub fn tilt_to_floor(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let n = fp.coll_data.floor.normal;
        let tilt = mem(fp::fneg(reg(crate::math::atan2(n.x, n.y))));
        assert!(
            fp.ground_or_air == GA_GROUND,
            "the original's assertion failed: fp->ground_or_air == GA_Ground"
        );
        self.game.part_set_rot_z(f, 0, tilt);
    }

    /// The model upright in the air (`ftPartSetRotZ(fp, 0, 0)`).
    fn ca_untilt(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_z(f, 0, 0.0);
    }

    /// The kick's speed slowed by its hits (`ftCa_Special_Inline_Friction`).
    fn ca_special_lw_friction(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let friction = reg(fp.mv_f32(mv::LW_FRICTION));
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), friction));
        fp.self_vel.y = mem(fp::fmuls(reg(fp.self_vel.y), friction));
    }

    /// `ftCa_SpecialLw_Phys`: along the floor or through the air by the animation, slowed by
    /// its hits, and the flame.
    #[doc(alias = "ftCa_SpecialLw_Phys")]
    pub fn ca_special_lw_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.tilt_to_floor(gobj);
            self.ground_anim_speed_facing(gobj);
        } else {
            self.ca_untilt(gobj);
            self.air_anim_motion(gobj);
        }
        self.ca_special_lw_friction(gobj);
        self.ca_special_lw_flame(gobj);
    }

    /// Ground traction `traction` times the character's ground friction once the script says
    /// (`cmd_vars[2]`), else plain friction.
    fn ca_special_lw_traction(&mut self, gobj: GObjId, traction: usize) {
        if self.game.fighter(gobj).cmd_vars[2] != 0 {
            let f = self.game.fighter_id(gobj);
            let t = reg(self.ca_attr_f32(f, traction));
            let fp = &mut self.game.fighters[f.0];
            let friction = fp::fmuls(t, reg(fp.co_attrs.ground_friction));
            Self::ground_accel_deaccel(fp, friction);
            self.self_movement_from_ground(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftCa_SpecialLwEnd_Phys`: on the ground, tilted, with the end's traction; in the air,
    /// upright without drift; then slowed by the kick's hits.
    #[doc(alias = "ftCa_SpecialLwEnd_Phys")]
    pub fn ca_special_lw_end_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.tilt_to_floor(gobj);
            self.ca_special_lw_traction(gobj, attr::SPECIALLW_GROUND_TRACTION);
        } else {
            self.ca_untilt(gobj);
            self.air_physics_no_drift(gobj);
        }
        self.ca_special_lw_friction(gobj);
    }

    /// `ftCa_SpecialLwEndAir_Phys`: on the ground, tilted, by the animation; in the air,
    /// upright, without drift once the script says (`cmd_vars[0]`), else by the animation.
    #[doc(alias = "ftCa_SpecialLwEndAir_Phys")]
    pub fn ca_special_lw_end_air_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.tilt_to_floor(gobj);
            self.ground_anim_speed_facing(gobj);
            return;
        }
        self.ca_untilt(gobj);
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.air_physics_no_drift(gobj);
        } else {
            self.air_anim_motion(gobj);
        }
    }

    /// `ftCa_SpecialAirLw_Phys`: by the animation, and the flame.
    #[doc(alias = "ftCa_SpecialAirLw_Phys")]
    pub fn ca_special_air_lw_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.ca_special_lw_flame(gobj);
    }

    /// `ftCa_SpecialAirLwEnd_Phys`: the landing's traction.
    #[doc(alias = "ftCa_SpecialAirLwEnd_Phys")]
    pub fn ca_special_air_lw_end_phys(&mut self, gobj: GObjId) {
        self.ca_special_lw_traction(gobj, attr::SPECIALLW_AIR_LANDING_TRACTION);
    }

    /// `ftCa_SpecialAirLwEndAir_Phys`: the air without drift.
    #[doc(alias = "ftCa_SpecialAirLwEndAir_Phys")]
    pub fn ca_special_air_lw_end_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftCa_SpecialHiThrow1_Phys`: by the animation.
    #[doc(alias = "ftCa_SpecialHiThrow1_Phys")]
    pub fn ca_special_hi_throw1_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
    }

    /// `ftCa_SpecialLw_Coll`: airborne off the ground or landed from the air, then once the
    /// script says (`cmd_vars[0]`), a wall the kick runs into rebounds it.
    #[doc(alias = "ftCa_SpecialLw_Coll")]
    pub fn ca_special_lw_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            if !self.ground_coll_off_ledges(gobj) {
                self.go_airborne(f);
            }
        } else if self.air_coll_land(gobj) {
            self.land(gobj);
        }
        let fp = &self.game.fighters[f.0];
        let facing = reg(fp.facing_dir);
        let env = fp.coll_data.env_flags;
        if fp.cmd_vars[0] != 0
            && ((facing == -1.0 && env & RIGHT_WALL_HUG != 0)
                || (facing == 1.0 && env & LEFT_WALL_HUG != 0))
        {
            self.ca_reset_cmd_and_throw(gobj);
            self.go_airborne(f);
            self.change_motion_state(gobj, ms::SPECIAL_HI_THROW1, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftCa_SpecialLwEnd_Coll` and `ftCa_SpecialLwEndAir_Coll`: on the ground, airborne off
    /// it (sliding off ledges once the script says, `cmd_vars[1]`); in the air, landed.
    #[doc(alias = "ftCa_SpecialLwEnd_Coll")]
    #[doc(alias = "ftCa_SpecialLwEndAir_Coll")]
    pub fn ca_special_lw_end_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            let on = if self.game.fighters[f.0].cmd_vars[1] != 0 {
                self.ground_coll_slide(gobj)
            } else {
                self.ground_coll_off_ledges(gobj)
            };
            if !on {
                self.go_airborne(f);
            }
        } else if self.air_coll_land(gobj) {
            self.land(gobj);
        }
    }

    /// The aerial kick landing (`doColl`): its landing, slowed by
    /// `speciallw_landing_lag_mul`.
    fn ca_special_air_lw_land(&mut self, gobj: GObjId) {
        if !self.air_coll_land(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let speed = self.ca_attr_f32(f, attr::SPECIALLW_LANDING_LAG_MUL);
        self.ca_reset_cmd_and_throw(gobj);
        self.land(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_END, 0, 0.0, speed, 0.0, None);
    }

    /// `ftCa_SpecialAirLw_Coll` and `ftCa_SpecialAirLwEndAir_Coll`.
    #[doc(alias = "ftCa_SpecialAirLw_Coll")]
    #[doc(alias = "ftCa_SpecialAirLwEndAir_Coll")]
    pub fn ca_special_air_lw_coll(&mut self, gobj: GObjId) {
        self.ca_special_air_lw_land(gobj);
    }

    /// `ftCa_SpecialAirLwEnd_Coll`: ground collision that falls.
    #[doc(alias = "ftCa_SpecialAirLwEnd_Coll")]
    pub fn ca_special_air_lw_end_coll(&mut self, gobj: GObjId) {
        self.ground_coll(gobj);
    }

    /// `ftCa_SpecialHiThrow1_Coll`: the air's collision with ledges (`ftCo_AirCatchHit_Coll`).
    #[doc(alias = "ftCa_SpecialHiThrow1_Coll")]
    pub fn ca_special_hi_throw1_coll(&mut self, gobj: GObjId) {
        self.co_air_catch_hit_coll(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_FLAME => g.ca_special_lw_flame(gobj),
        addr::LW_ON_HIT => g.ca_special_lw_on_hit(gobj),
        addr::SPECIAL_LW_ENTER => g.ca_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.ca_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.ca_special_lw_anim(gobj),
        addr::SPECIAL_LW_END_ANIM | addr::SPECIAL_LW_END_AIR_ANIM => g.ca_special_lw_end_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.ca_special_air_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_END_ANIM => g.ca_special_air_lw_end_anim(gobj),
        addr::SPECIAL_AIR_LW_END_AIR_ANIM | addr::SPECIAL_HI_THROW1_ANIM => {
            g.ca_special_lw_fall_anim(gobj)
        }
        addr::SPECIAL_LW_PHYS => g.ca_special_lw_phys(gobj),
        addr::SPECIAL_LW_END_PHYS => g.ca_special_lw_end_phys(gobj),
        addr::SPECIAL_LW_END_AIR_PHYS => g.ca_special_lw_end_air_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.ca_special_air_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_END_PHYS => g.ca_special_air_lw_end_phys(gobj),
        addr::SPECIAL_AIR_LW_END_AIR_PHYS => g.ca_special_air_lw_end_air_phys(gobj),
        addr::SPECIAL_HI_THROW1_PHYS => g.ca_special_hi_throw1_phys(gobj),
        addr::SPECIAL_LW_COLL => g.ca_special_lw_coll(gobj),
        addr::SPECIAL_LW_END_COLL | addr::SPECIAL_LW_END_AIR_COLL => g.ca_special_lw_end_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL | addr::SPECIAL_AIR_LW_END_AIR_COLL => {
            g.ca_special_air_lw_coll(gobj)
        }
        addr::SPECIAL_AIR_LW_END_COLL => g.ca_special_air_lw_end_coll(gobj),
        addr::SPECIAL_HI_THROW1_COLL => g.ca_special_hi_throw1_coll(gobj),
        _ => return false,
    }
    true
}
