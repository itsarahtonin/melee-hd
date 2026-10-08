//! The reflector (`ftfoxspeciallw.c`): its start, the loop held while B is, turning around,
//! reflecting a hit, and the end, on the ground and in the air, through platforms and off
//! edges.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::{KEEP_GFX, SKIP_COL_ANIM, UPDATE_CMD};
use super::{GA_GROUND, PAD_B, addr, attr, ext, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftFx_MF_SpecialLw_Coll` and `ftFx_MF_SpecialLwEnd_Coll`.
const COLL_FLAGS: u32 = 0x0C4C_5082;
const END_COLL_FLAGS: u32 = SKIP_COL_ANIM | UPDATE_CMD;

/// `FtPart_HipN`.
const PART_HIP_N: i32 = 4;

/// `0.01745329238474369`: degrees to radians.
const DEG_TO_RAD: f64 = 0.017_453_292_384_743_69;

impl Running<'_> {
    /// One of the reflector's effects on the hip, once (`x2219_b0`).
    fn fox_lw_gfx(&mut self, gobj: GObjId, gfx: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_HIP_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftFx_SpecialLw_CreateLoopGFX`.
    #[doc(alias = "ftFx_SpecialLw_CreateLoopGFX")]
    pub fn fox_lw_loop_gfx(&mut self, gobj: GObjId) {
        self.fox_lw_gfx(gobj, 1160);
    }

    /// `ftFx_SpecialLw_CreateStartGFX`.
    #[doc(alias = "ftFx_SpecialLw_CreateStartGFX")]
    pub fn fox_lw_start_gfx(&mut self, gobj: GObjId) {
        self.fox_lw_gfx(gobj, 1161);
    }

    /// `ftFx_SpecialLw_CreateReflectGFX`.
    #[doc(alias = "ftFx_SpecialLw_CreateReflectGFX")]
    pub fn fox_lw_reflect_gfx(&mut self, gobj: GObjId) {
        self.fox_lw_gfx(gobj, 1162);
    }

    /// `ftFox_SpecialLw_SetVars`.
    fn fox_lw_start_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = fp::fctiwz(reg(self.fox_attr_f32(f, attr::REFLECTOR_RELEASE_LAG)));
        let delay = self.fox_attr_i32(f, attr::REFLECTOR_GRAVITY_DELAY);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::LW_RELEASE_LAG, lag);
        fp.set_mv_i32(mv::LW_IS_RELEASE, 0);
        fp.cmd_vars[1] = 4;
        fp.set_mv_i32(mv::LW_GRAVITY_DELAY, delay);
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_CREATE_START_GFX));
    }

    /// The grounded reflector, `ftFx_SpecialLw_Enter`.
    #[doc(alias = "ftFx_SpecialLw_Enter")]
    pub fn fox_special_lw_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_LW_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.fox_lw_start_vars(gobj);
    }

    /// The aerial reflector, `ftFx_SpecialAirLw_Enter`.
    #[doc(alias = "ftFx_SpecialAirLw_Enter")]
    pub fn fox_special_air_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let div = reg(self.fox_attr_f32(f, attr::REFLECTOR_MOMENTUM_PRESERVE_X));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.fox_lw_start_vars(gobj);
    }

    /// Letting go of B marks the release (`isRelease`).
    fn fox_lw_check_release(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            fp.set_mv_i32(mv::LW_IS_RELEASE, 1);
        }
    }

    /// `updateRelease`: the release, and its lag counting down.
    fn fox_lw_update_release(&mut self, gobj: GObjId) {
        self.fox_lw_check_release(gobj);
        let fp = self.game.fighter_mut(gobj);
        let lag = fp.mv_i32(mv::LW_RELEASE_LAG);
        if lag > 0 {
            fp.set_mv_i32(mv::LW_RELEASE_LAG, lag - 1);
        }
    }

    /// The starts, `ftFx_SpecialLwStart_Anim` and `ftFx_SpecialAirLwStart_Anim`: the loop
    /// when they end.
    #[doc(alias = "ftFx_SpecialLwStart_Anim")]
    #[doc(alias = "ftFx_SpecialAirLwStart_Anim")]
    pub fn fox_special_lw_start_anim(&mut self, gobj: GObjId) {
        self.fox_lw_check_release(gobj);
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).ground_or_air == GA_GROUND {
                self.fox_special_lw_loop_enter(gobj);
            } else {
                self.fox_special_air_lw_loop_enter(gobj);
            }
            self.destroy_effects(gobj);
            self.game.fighter_mut(gobj).accessory4_cb =
                Some(CodeRef(addr::SPECIAL_LW_CREATE_LOOP_GFX));
        }
    }

    /// `ftFx_SpecialLwStart_IASA`.
    #[doc(alias = "ftFx_SpecialLwStart_IASA")]
    pub fn fox_special_lw_start_iasa(&mut self, gobj: GObjId) {
        self.fox_special_lw_start_check_pass(gobj);
    }

    /// Dropping through a platform during the start, `ftFx_SpecialLwStart_CheckPass`.
    #[doc(alias = "ftFx_SpecialLwStart_CheckPass")]
    pub fn fox_special_lw_start_check_pass(&mut self, gobj: GObjId) -> bool {
        if self.pass_input(gobj) {
            self.fox_special_lw_start_pass(gobj);
            return true;
        }
        false
    }

    /// `ftFco_8009A184` into an aerial state from the same frame, and the reflector.
    fn fox_lw_pass(&mut self, gobj: GObjId, msid: i32) {
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.enter_pass_as(gobj, msid, COLL_FLAGS, frame);
        self.fox_special_lw_create_reflect_hit(gobj);
    }

    /// `ftFx_SpecialLwStart_Pass`.
    #[doc(alias = "ftFx_SpecialLwStart_Pass")]
    pub fn fox_special_lw_start_pass(&mut self, gobj: GObjId) {
        self.fox_lw_pass(gobj, ms::SPECIAL_AIR_LW_START);
    }

    /// The aerial states' physics (`ftFox_SpecialLw_InlinePhys`): falling once gravity's
    /// delay is over, the quick air friction.
    #[doc(alias = "ftFx_SpecialAirLwStart_Phys")]
    #[doc(alias = "ftFx_SpecialAirLwEnd_Phys")]
    pub fn fox_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.fox_count_down(gobj, mv::LW_GRAVITY_DELAY) {
            let g = reg(self.fox_attr_f32(f, attr::REFLECTOR_FALL_ACCEL));
            let fp = &mut self.game.fighters[f.0];
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        self.ext_fighter(ext::FTCOMMON_CALC_SELF_ACCEL_DEACCEL_QUICK_AIR, gobj);
    }

    /// The loop's, turn's and hit's grounded physics: friction, and the reflector's bubble
    /// follows (`ftColl_8007AEF8`).
    #[doc(alias = "ftFx_SpecialLwLoop_Phys")]
    #[doc(alias = "ftFx_SpecialLwTurn_Phys")]
    #[doc(alias = "ftFx_SpecialLwHit_Phys")]
    pub fn fox_special_lw_reflect_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.ext_gobj(ext::FTCOLL_8007AEF8, gobj);
    }

    /// Their aerial physics.
    #[doc(alias = "ftFx_SpecialAirLwLoop_Phys")]
    #[doc(alias = "ftFx_SpecialAirLwTurn_Phys")]
    #[doc(alias = "ftFx_SpecialAirLwHit_Phys")]
    pub fn fox_special_air_lw_reflect_phys(&mut self, gobj: GObjId) {
        self.fox_special_air_lw_phys(gobj);
        self.ext_gobj(ext::FTCOLL_8007AEF8, gobj);
    }

    /// Off the ground (`ft_80082708`), into an aerial state from the same frame.
    fn fox_lw_ground_coll(&mut self, gobj: GObjId) -> bool {
        !self.ground_coll_off_ledges(gobj)
    }

    /// On the ground (`ft_80081D0C`).
    fn fox_lw_air_coll(&mut self, gobj: GObjId) -> bool {
        self.air_coll_land(gobj)
    }

    /// `ftFx_SpecialLwStart_Coll`.
    #[doc(alias = "ftFx_SpecialLwStart_Coll")]
    pub fn fox_special_lw_start_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_ground_coll(gobj) {
            self.fox_special_lw_start_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirLwStart_Coll`.
    #[doc(alias = "ftFx_SpecialAirLwStart_Coll")]
    pub fn fox_special_air_lw_start_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_air_coll(gobj) {
            self.fox_special_air_lw_start_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialLwStart_GroundToAir`.
    #[doc(alias = "ftFx_SpecialLwStart_GroundToAir")]
    pub fn fox_special_lw_start_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_START, COLL_FLAGS);
    }

    /// Lands into `msid`, its speed within the air drift.
    fn fox_lw_land(&mut self, gobj: GObjId, msid: i32, flags: u32) {
        self.fox_air_to_ground(gobj, msid, flags);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftFx_SpecialAirLwStart_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirLwStart_AirToGround")]
    pub fn fox_special_air_lw_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_lw_land(gobj, ms::SPECIAL_LW_START, COLL_FLAGS);
    }

    /// The loops, `ftFx_SpecialLwLoop_Anim` and `ftFx_SpecialAirLwLoop_Anim`: the end once
    /// B is let go and the release lag is over.
    #[doc(alias = "ftFx_SpecialLwLoop_Anim")]
    #[doc(alias = "ftFx_SpecialAirLwLoop_Anim")]
    pub fn fox_special_lw_loop_anim(&mut self, gobj: GObjId) {
        self.fox_lw_update_release(gobj);
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::LW_RELEASE_LAG) <= 0 && fp.mv_i32(mv::LW_IS_RELEASE) != 0 {
            if fp.ground_or_air == GA_GROUND {
                self.fox_special_lw_end_enter(gobj);
                return;
            }
            self.fox_special_air_lw_end_enter(gobj);
        }
    }

    /// `ftFx_SpecialLwLoop_IASA`: turning, jumping, or dropping through a platform.
    #[doc(alias = "ftFx_SpecialLwLoop_IASA")]
    pub fn fox_special_lw_loop_iasa(&mut self, gobj: GObjId) {
        if !self.fox_special_lw_turn_check(gobj) && !self.jump_check(gobj) {
            self.fox_special_lw_loop_check_pass(gobj);
        }
    }

    /// `ftFx_SpecialAirLwLoop_IASA`: turning, or a double jump (`ftCo_800CB870`).
    #[doc(alias = "ftFx_SpecialAirLwLoop_IASA")]
    pub fn fox_special_air_lw_loop_iasa(&mut self, gobj: GObjId) {
        if !self.fox_special_lw_turn_check(gobj) {
            self.jump_aerial_check(gobj);
        }
    }

    /// `ftFx_SpecialLwLoop_CheckPass`.
    #[doc(alias = "ftFx_SpecialLwLoop_CheckPass")]
    pub fn fox_special_lw_loop_check_pass(&mut self, gobj: GObjId) -> bool {
        if self.pass_input(gobj) {
            self.fox_special_lw_loop_pass(gobj);
            return true;
        }
        false
    }

    /// `ftFx_SpecialLwLoop_Pass`.
    #[doc(alias = "ftFx_SpecialLwLoop_Pass")]
    pub fn fox_special_lw_loop_pass(&mut self, gobj: GObjId) {
        self.fox_lw_pass(gobj, ms::SPECIAL_AIR_LW_LOOP);
    }

    /// `ftFx_SpecialLwLoop_Coll`.
    #[doc(alias = "ftFx_SpecialLwLoop_Coll")]
    pub fn fox_special_lw_loop_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_ground_coll(gobj) {
            self.fox_special_lw_loop_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirLwLoop_Coll`.
    #[doc(alias = "ftFx_SpecialAirLwLoop_Coll")]
    pub fn fox_special_air_lw_loop_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_air_coll(gobj) {
            self.fox_special_air_lw_loop_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialLwLoop_GroundToAir`.
    #[doc(alias = "ftFx_SpecialLwLoop_GroundToAir")]
    pub fn fox_special_lw_loop_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_LOOP, COLL_FLAGS);
        self.fox_special_lw_create_reflect_hit(gobj);
    }

    /// `ftFx_SpecialAirLwLoop_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirLwLoop_AirToGround")]
    pub fn fox_special_air_lw_loop_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_lw_land(gobj, ms::SPECIAL_LW_LOOP, COLL_FLAGS);
        self.fox_special_lw_create_reflect_hit(gobj);
    }

    /// The reflector's bubble, `ftFx_SpecialLw_CreateReflectHit`: the attributes'
    /// `ReflectDesc` (at `+0xB0`), reflecting into `ftFx_SpecialLwHit_Enter`.
    #[doc(alias = "ftFx_SpecialLw_CreateReflectHit")]
    pub fn fox_special_lw_create_reflect_hit(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.fox_attr_f32(f, o);
        let d = crate::game::ft::co::guard::ReflectDesc {
            bone: self.fox_attr_i32(f, 0xB0),
            max_damage: self.fox_attr_i32(f, 0xB4),
            offset: crate::vec::Vec3::new(a(0xB8), a(0xBC), a(0xC0)),
            size: a(0xC4),
            damage_mul: a(0xC8),
            speed_mul: a(0xCC),
            behavior: (self.fox_attr_i32(f, 0xD0) as u32 >> 24) as u8,
        };
        self.create_reflect_hit(gobj, &d, CodeRef(addr::SPECIAL_LW_HIT_ENTER));
    }

    /// The grounded loop, `ftFx_SpecialLwLoop_Enter`.
    #[doc(alias = "ftFx_SpecialLwLoop_Enter")]
    pub fn fox_special_lw_loop_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_LW_LOOP, KEEP_GFX, 0.0, 1.0, 0.0, None);
        self.fox_special_lw_create_reflect_hit(gobj);
    }

    /// The aerial loop, `ftFx_SpecialAirLwLoop_Enter`.
    #[doc(alias = "ftFx_SpecialAirLwLoop_Enter")]
    pub fn fox_special_air_lw_loop_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_LOOP, KEEP_GFX, 0.0, 1.0, 0.0, None);
        self.fox_special_lw_create_reflect_hit(gobj);
    }

    /// A frame of the turn, `ftFx_SpecialLw_Turn`: facing flips halfway, the body spins.
    #[doc(alias = "ftFx_SpecialLw_Turn")]
    pub fn fox_special_lw_turn(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = reg(self.fox_attr_f32(f, attr::REFLECTOR_TURN_FRAMES));
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::LW_TURN_FRAMES).wrapping_sub(1);
        fp.set_mv_i32(mv::LW_TURN_FRAMES, n);
        if fp.cmd_vars[0] == 0 && fp::frsp(f64::from(n)) <= frames {
            fp.cmd_vars[0] = 1;
            fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        }
        let rot_z = self
            .ext(ext::FT_PART_GET_ROT_Z, &[Val::Fighter(f), Val::Int(0)])
            .float();
        let v = fp::fnmsubs(DEG_TO_RAD, fp::fdivs(fp::frsp(180.0), frames), rot_z);
        self.game.part_set_rot_y(f, 0, mem(v));
    }

    /// The turns, `ftFx_SpecialLwTurn_Anim` and `ftFx_SpecialAirLwTurn_Anim`: the loop or
    /// the end when the turn's frames run out.
    #[doc(alias = "ftFx_SpecialLwTurn_Anim")]
    #[doc(alias = "ftFx_SpecialAirLwTurn_Anim")]
    pub fn fox_special_lw_turn_anim(&mut self, gobj: GObjId) {
        self.fox_lw_update_release(gobj);
        self.fox_special_lw_turn(gobj);
        if self.game.fighter(gobj).mv_i32(mv::LW_TURN_FRAMES) <= 0 {
            self.fox_special_lw_hit_check(gobj);
        }
    }

    /// `ftFx_SpecialLwTurn_Coll`.
    #[doc(alias = "ftFx_SpecialLwTurn_Coll")]
    pub fn fox_special_lw_turn_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_ground_coll(gobj) {
            self.fox_special_lw_turn_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirLwTurn_Coll`.
    #[doc(alias = "ftFx_SpecialAirLwTurn_Coll")]
    pub fn fox_special_air_lw_turn_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_air_coll(gobj) {
            self.fox_special_air_lw_turn_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialLwHit_SetCall`: reflecting, into the hit when something is.
    #[doc(alias = "ftFx_SpecialLwHit_SetCall")]
    pub fn fox_special_lw_hit_set_call(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.reflecting = true;
        fp.reflect_hit_cb = Some(CodeRef(addr::SPECIAL_LW_HIT_ENTER));
    }

    /// `ftFx_SpecialLwTurn_GroundToAir`.
    #[doc(alias = "ftFx_SpecialLwTurn_GroundToAir")]
    pub fn fox_special_lw_turn_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_TURN, COLL_FLAGS);
        self.fox_special_lw_hit_set_call(gobj);
    }

    /// The aerial turn landing, `ftFx_SpecialAirLwTurn_GroundToAir`.
    #[doc(alias = "ftFx_SpecialAirLwTurn_GroundToAir")]
    pub fn fox_special_air_lw_turn_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_lw_land(gobj, ms::SPECIAL_LW_TURN, COLL_FLAGS);
        self.fox_special_lw_hit_set_call(gobj);
    }

    /// Turning around in the loop, `ftFx_SpecialLwTurn_Check`.
    #[doc(alias = "ftFx_SpecialLwTurn_Check")]
    pub fn fox_special_lw_turn_check(&mut self, gobj: GObjId) -> bool {
        if !self.turn_input(gobj) {
            return false;
        }
        let msid = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            ms::SPECIAL_LW_TURN
        } else {
            ms::SPECIAL_AIR_LW_TURN
        };
        self.change_motion_state(gobj, msid, KEEP_GFX, 0.0, 1.0, 0.0, None);
        // `ftFox_SpecialLwTurn_SetVarAll`.
        let f = self.game.fighter_id(gobj);
        let frames = fp::fctiwz(reg(self.fox_attr_f32(f, attr::REFLECTOR_TURN_FRAMES)));
        let fp = &mut self.game.fighters[f.0];
        fp.reflecting = true;
        fp.reflect_hit_cb = Some(CodeRef(addr::SPECIAL_LW_HIT_ENTER));
        fp.set_mv_i32(mv::LW_TURN_FRAMES, frames);
        fp.cmd_vars[0] = 0;
        self.fox_special_lw_turn(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_CREATE_LOOP_GFX));
        true
    }

    /// After a turn or a hit, `ftFx_SpecialLwHit_Check`: the end once released and the lag
    /// is over (false), else the loop again (true).
    #[doc(alias = "ftFx_SpecialLwHit_Check")]
    pub fn fox_special_lw_hit_check(&mut self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        let ground = fp.ground_or_air == GA_GROUND;
        if fp.mv_i32(mv::LW_RELEASE_LAG) <= 0 && fp.mv_i32(mv::LW_IS_RELEASE) != 0 {
            if ground {
                self.fox_special_lw_end_enter(gobj);
            } else {
                self.fox_special_air_lw_end_enter(gobj);
            }
            return false;
        }
        if ground {
            self.fox_special_lw_loop_enter(gobj);
        } else {
            self.fox_special_air_lw_loop_enter(gobj);
        }
        true
    }

    /// The hits, `ftFx_SpecialLwHit_Anim` and `ftFx_SpecialAirLwHit_Anim`.
    #[doc(alias = "ftFx_SpecialLwHit_Anim")]
    #[doc(alias = "ftFx_SpecialAirLwHit_Anim")]
    pub fn fox_special_lw_hit_anim(&mut self, gobj: GObjId) {
        self.fox_lw_update_release(gobj);
        if !self.anim_frames_remaining(gobj) && self.fox_special_lw_hit_check(gobj) {
            self.destroy_effects(gobj);
            self.game.fighter_mut(gobj).accessory4_cb =
                Some(CodeRef(addr::SPECIAL_LW_CREATE_LOOP_GFX));
        }
    }

    /// `ftFx_SpecialLwHit_Coll`.
    #[doc(alias = "ftFx_SpecialLwHit_Coll")]
    pub fn fox_special_lw_hit_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_ground_coll(gobj) {
            self.fox_special_lw_hit_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirLwHit_Coll`.
    #[doc(alias = "ftFx_SpecialAirLwHit_Coll")]
    pub fn fox_special_air_lw_hit_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_air_coll(gobj) {
            self.fox_special_air_lw_hit_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialLwHit_GroundToAir`.
    #[doc(alias = "ftFx_SpecialLwHit_GroundToAir")]
    pub fn fox_special_lw_hit_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_HIT, COLL_FLAGS);
        self.fox_special_lw_hit_set_call(gobj);
    }

    /// `ftFx_SpecialAirLwHit_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirLwHit_AirToGround")]
    pub fn fox_special_air_lw_hit_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_lw_land(gobj, ms::SPECIAL_LW_HIT, COLL_FLAGS);
        self.fox_special_lw_hit_set_call(gobj);
    }

    /// Reflecting a hit, `ftFx_SpecialLwHit_Enter` (the reflect callback): facing the way
    /// the hit says, a rumble from the hip, the hit state.
    #[doc(alias = "ftFx_SpecialLwHit_Enter")]
    pub fn fox_special_lw_hit_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = fp.ReflectAttr.x1A2C_reflectHitDirection;
        let j = self.part_joint(gobj, PART_HIP_N);
        let at = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let third = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 3.0)));
        crate::lb::wind::blast(&mut self.game.gr.winds, at, 120, 3.0, 0.1, third);
        let msid = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            ms::SPECIAL_LW_HIT
        } else {
            ms::SPECIAL_AIR_LW_HIT
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.fox_special_lw_hit_set_call(gobj);
        self.game.fighter_mut(gobj).accessory4_cb =
            Some(CodeRef(addr::SPECIAL_LW_CREATE_REFLECT_GFX));
    }

    /// The ends, `ftFx_SpecialLwEnd_Anim` and `ftFx_SpecialAirLwEnd_Anim`.
    #[doc(alias = "ftFx_SpecialLwEnd_Anim")]
    #[doc(alias = "ftFx_SpecialAirLwEnd_Anim")]
    pub fn fox_special_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.destroy_effects(gobj);
            self.fall_or_wait(gobj);
        }
    }

    /// `ftFx_SpecialLwEnd_Coll`.
    #[doc(alias = "ftFx_SpecialLwEnd_Coll")]
    pub fn fox_special_lw_end_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_ground_coll(gobj) {
            self.fox_special_lw_end_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirLwEnd_Coll`.
    #[doc(alias = "ftFx_SpecialAirLwEnd_Coll")]
    pub fn fox_special_air_lw_end_coll(&mut self, gobj: GObjId) {
        if self.fox_lw_air_coll(gobj) {
            self.fox_special_air_lw_end_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialLwEnd_GroundToAir`.
    #[doc(alias = "ftFx_SpecialLwEnd_GroundToAir")]
    pub fn fox_special_lw_end_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_END, END_COLL_FLAGS);
    }

    /// `ftFx_SpecialAirLwEnd_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirLwEnd_AirToGround")]
    pub fn fox_special_air_lw_end_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_lw_land(gobj, ms::SPECIAL_LW_END, END_COLL_FLAGS);
    }

    /// `ftFx_SpecialLwEnd_Enter`.
    #[doc(alias = "ftFx_SpecialLwEnd_Enter")]
    pub fn fox_special_lw_end_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_LW_END, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftFx_SpecialAirLwEnd_Enter`.
    #[doc(alias = "ftFx_SpecialAirLwEnd_Enter")]
    pub fn fox_special_air_lw_end_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW_END, 0, 0.0, 1.0, 0.0, None);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_CREATE_LOOP_GFX => g.fox_lw_loop_gfx(gobj),
        addr::SPECIAL_LW_CREATE_START_GFX => g.fox_lw_start_gfx(gobj),
        addr::SPECIAL_LW_CREATE_REFLECT_GFX => g.fox_lw_reflect_gfx(gobj),
        addr::SPECIAL_LW_ENTER => g.fox_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.fox_special_air_lw_enter(gobj),
        0x800E_8694 | 0x800E_8720 => g.fox_special_lw_start_anim(gobj),
        0x800E_87AC => g.fox_special_lw_start_iasa(gobj),
        0x800E_87D0 | 0x800E_91FC | 0x800E_9200 | 0x800E_9844 | 0x800E_9848 | 0x800E_9BC8
        | 0x800E_9BCC => g.fox_no_iasa(gobj),
        0x800E_87D4 => {
            g.fox_special_lw_start_check_pass(gobj);
        }
        0x800E_881C => g.fox_special_lw_start_pass(gobj),
        0x800E_8864 | 0x800E_9BD0 => g.fox_ground_phys(gobj),
        0x800E_8884 | 0x800E_9BF0 => g.fox_special_air_lw_phys(gobj),
        0x800E_88E4 => g.fox_special_lw_start_coll(gobj),
        0x800E_8920 => g.fox_special_air_lw_start_coll(gobj),
        0x800E_895C => g.fox_special_lw_start_ground_to_air(gobj),
        0x800E_89BC => g.fox_special_air_lw_start_air_to_ground(gobj),
        0x800E_8A24 | 0x800E_8A9C => g.fox_special_lw_loop_anim(gobj),
        0x800E_8B14 => g.fox_special_lw_loop_iasa(gobj),
        0x800E_8B64 => g.fox_special_air_lw_loop_iasa(gobj),
        0x800E_8BA4 => {
            g.fox_special_lw_loop_check_pass(gobj);
        }
        0x800E_8BEC => g.fox_special_lw_loop_pass(gobj),
        0x800E_8C34 | 0x800E_9204 | 0x800E_984C => g.fox_special_lw_reflect_phys(gobj),
        0x800E_8C68 | 0x800E_9238 | 0x800E_9880 => g.fox_special_air_lw_reflect_phys(gobj),
        0x800E_8CDC => g.fox_special_lw_loop_coll(gobj),
        0x800E_8D18 => g.fox_special_air_lw_loop_coll(gobj),
        0x800E_8D54 => g.fox_special_lw_loop_ground_to_air(gobj),
        0x800E_8DBC => g.fox_special_air_lw_loop_air_to_ground(gobj),
        0x800E_8E2C => g.fox_special_lw_create_reflect_hit(gobj),
        0x800E_8E60 => g.fox_special_lw_loop_enter(gobj),
        0x800E_8EC0 => g.fox_special_air_lw_loop_enter(gobj),
        0x800E_8F20 => g.fox_special_lw_turn(gobj),
        0x800E_8FDC | 0x800E_90EC => g.fox_special_lw_turn_anim(gobj),
        0x800E_92AC => g.fox_special_lw_turn_coll(gobj),
        0x800E_92E8 => g.fox_special_air_lw_turn_coll(gobj),
        0x800E_9324 => g.fox_special_lw_turn_ground_to_air(gobj),
        0x800E_93A4 => g.fox_special_air_lw_turn_ground_to_air(gobj),
        0x800E_942C => {
            g.fox_special_lw_turn_check(gobj);
        }
        0x800E_9564 => {
            g.fox_special_lw_hit_check(gobj);
        }
        0x800E_965C | 0x800E_97B4 => g.fox_special_lw_hit_anim(gobj),
        0x800E_98F4 => g.fox_special_lw_hit_coll(gobj),
        0x800E_9930 => g.fox_special_air_lw_hit_coll(gobj),
        0x800E_996C => g.fox_special_lw_hit_ground_to_air(gobj),
        0x800E_99D4 => g.fox_special_air_lw_hit_air_to_ground(gobj),
        0x800E_9A44 => g.fox_special_lw_hit_set_call(gobj),
        addr::SPECIAL_LW_HIT_ENTER => g.fox_special_lw_hit_enter(gobj),
        0x800E_9B40 | 0x800E_9B84 => g.fox_special_lw_end_anim(gobj),
        0x800E_9C50 => g.fox_special_lw_end_coll(gobj),
        0x800E_9C8C => g.fox_special_air_lw_end_coll(gobj),
        0x800E_9CC8 => g.fox_special_lw_end_ground_to_air(gobj),
        0x800E_9D24 => g.fox_special_air_lw_end_air_to_ground(gobj),
        0x800E_9D88 => g.fox_special_lw_end_enter(gobj),
        0x800E_9DC0 => g.fox_special_air_lw_end_enter(gobj),
        _ => return false,
    }
    true
}
