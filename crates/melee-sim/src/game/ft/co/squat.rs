//! Crouching and dropping through platforms (`ftCo_Squat.c`, `ftCo_SquatWait.c`,
//! `ftCo_SquatRv.c`, `ftCo_Pass.c`).

use gekko_fp as fp;

use super::super::super::code::unported_call;
use super::super::super::state::Running;
use super::PAD_LR;
use super::checks::{KIND_CLINK, KIND_LINK};
use super::ids::{ms, sm};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::mp::LINE_PLATFORM;

/// `ftCommon_MotionVars_squat` (and `pass`, which shares it).
const SQUAT_X0: usize = 0x0;
const SQUAT_X4: usize = 0x4;

impl Running<'_> {
    /// Whether the stick is held down past `ftCommonData` x90, `ftCo_Squat_CheckInput`.
    #[doc(alias = "ftCo_Squat_CheckInput")]
    #[doc(alias = "fn_800D5F84")]
    pub fn squat_input(&self, gobj: GObjId) -> bool {
        let t = fp::fneg(reg(self.common().f32(0x90)));
        reg(self.game.fighter(gobj).input.lstick[0].y) < t
    }

    /// Crouching when the stick is held down, `ftCo_800D5FB0`.
    #[doc(alias = "ftCo_800D5FB0")]
    pub fn squat_check(&mut self, gobj: GObjId) -> bool {
        if self.squat_input(gobj) {
            self.enter_squat(gobj);
            return true;
        }
        false
    }

    /// Crouching, `ftCo_Squat_Enter`.
    #[doc(alias = "ftCo_Squat_Enter")]
    pub fn enter_squat(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SQUAT, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let x5f0 = self.common().u32(0x5F0);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(SQUAT_X0, 0);
        fp.x209A = x5f0 as u16;
    }

    /// `ftCo_Squat_Anim`: the crouch settles into its wait.
    #[doc(alias = "ftCo_Squat_Anim")]
    pub fn co_squat_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_squat_wait_settled(gobj);
        }
    }

    /// Whether the fighter stands on a platform, `mpColl_IsOnPlatform`.
    fn on_platform(&self, gobj: GObjId) -> bool {
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        g.line_surface(self.game.fighter(gobj).coll_data.floor.index) & LINE_PLATFORM != 0
    }

    /// `ftCo_Squat_IASA`: anything standing allows, a platform drop, and (`inline`) the drop
    /// a stick flick arms counting down to dropping through.
    #[doc(alias = "ftCo_Squat_IASA")]
    pub fn co_squat_iasa(&mut self, gobj: GObjId) {
        if self.special_s_check(gobj)
            || self.special_hi_check(gobj)
            || self.special_n_check(gobj)
            || self.special_lw_check(gobj)
            || self.catch_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.pass_check(gobj)
        {
            return;
        }
        // `ftCo_Squat_IASA_inline`.
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_i32(SQUAT_X0) != 0 && fp.mv_f32(SQUAT_X4) != 0.0 {
            fp.set_mv_f32(SQUAT_X4, fp::fsubs(reg(fp.mv_f32(SQUAT_X4)), 1.0));
            if fp.mv_f32(SQUAT_X4) == 0.0 && self.on_platform(gobj) {
                self.enter_pass(gobj);
            }
        }
    }

    /// Crouching and waiting when the stick is held down, `ftCo_SquatWait_CheckInput`.
    #[doc(alias = "ftCo_SquatWait_CheckInput")]
    pub fn squat_wait_check(&mut self, gobj: GObjId) -> bool {
        if self.squat_input(gobj) {
            self.enter_squat_wait(gobj, 0, true);
            return true;
        }
        false
    }

    /// The crouch's wait (`ftCo_SquatWait_Enter_inline`): with an item in one hand, its own
    /// animation; `nametag` restarts the nametag's timer.
    fn enter_squat_wait(&mut self, gobj: GObjId, flags: u32, nametag: bool) {
        self.change_motion_state(gobj, ms::SQUAT_WAIT, flags, 0.0, 1.0, 0.0, None);
        if self.holding_one_handed(gobj) {
            let f = self.game.fighter_id(gobj);
            let entry = self.anim_entry(f, sm::SQUAT_WAIT_ITEM);
            if self.data.u32(entry.at(8)) != 0 {
                self.play_anim(gobj, sm::SQUAT_WAIT_ITEM);
            }
        }
        if nametag {
            let x5f0 = self.common().u32(0x5F0);
            self.game.fighter_mut(gobj).x209A = x5f0 as u16;
        }
        match self.game.fighter(gobj).kind {
            KIND_LINK => unported_call("ftLk_AttackAir_800EB3BC"),
            KIND_CLINK => unported_call("ftCl_Init_8014919C"),
            _ => {}
        }
    }

    /// `fn_800D62C4`: the crouch's wait from standing.
    #[doc(alias = "fn_800D62C4")]
    pub fn enter_squat_wait_from_stand(&mut self, gobj: GObjId) {
        self.enter_squat_wait(gobj, 0, true);
    }

    /// `ftCo_800D638C`: the crouch's wait once the crouch settles.
    #[doc(alias = "ftCo_800D638C")]
    pub fn enter_squat_wait_settled(&mut self, gobj: GObjId) {
        // `Ft_MF_SkipNametagVis`.
        self.enter_squat_wait(gobj, 1 << 15, false);
    }

    /// `ftCo_SquatWait_Anim`: the crouch's idle animations (`ftData::x28`).
    #[doc(alias = "ftCo_SquatWait_Anim")]
    pub fn co_squat_wait_anim(&mut self, gobj: GObjId) {
        self.wait_anim_tick(gobj, 0x28);
    }

    /// `ftCo_SquatWait_IASA`.
    #[doc(alias = "ftCo_SquatWait_IASA")]
    pub fn co_squat_wait_iasa(&mut self, gobj: GObjId) {
        let _ = self.special_lw_check(gobj)
            || self.special_hi_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.pass_check(gobj)
            || self.dash_check(gobj)
            || self.squat_rv_check(gobj);
    }

    /// Standing up from a crouch when the stick lets go, `ftCo_SquatRv_CheckInput`.
    #[doc(alias = "ftCo_SquatRv_CheckInput")]
    pub fn squat_rv_check(&mut self, gobj: GObjId) -> bool {
        let t = fp::fneg(reg(self.common().f32(0x94)));
        if reg(self.game.fighter(gobj).input.lstick[0].y) > t {
            self.enter_squat_rv(gobj);
            return true;
        }
        false
    }

    /// Standing up, `ftCo_SquatRv_Enter`.
    #[doc(alias = "ftCo_SquatRv_Enter")]
    pub fn enter_squat_rv(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SQUAT_RV, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_SquatRv_IASA`.
    #[doc(alias = "ftCo_SquatRv_IASA")]
    pub fn co_squat_rv_iasa(&mut self, gobj: GObjId) {
        let _ = self.special_lw_check(gobj)
            || self.special_hi_check(gobj)
            || self.attack_s4_check(gobj)
            || self.attack_hi4_check(gobj)
            || self.attack_lw4_check(gobj)
            || self.attack_s3_check(gobj)
            || self.attack_hi3_check(gobj)
            || self.attack_lw3_check(gobj)
            || self.attack1_check(gobj)
            || self.guard_check(gobj)
            || self.appeal_check(gobj)
            || self.jump_check(gobj)
            || self.walk_check(gobj);
    }

    /// A stick flicked down on a platform, `ftCo_80099F1C`.
    #[doc(alias = "ftCo_80099F1C")]
    pub fn pass_input(&self, gobj: GObjId) -> bool {
        let c = self.common();
        let (t, window) = (fp::fneg(reg(c.f32(0x464))), c.i32(0x468));
        let fp = self.game.fighter(gobj);
        reg(fp.input.lstick[0].y) <= t
            && i32::from(fp.active_timer.lstick.y) < window
            && self.on_platform(gobj)
    }

    /// Arming a platform drop from a crouch, `ftCo_80099F9C`: the crouch counts `ftCommonData`
    /// x470 frames down to it (with the hammer, its own).
    #[doc(alias = "ftCo_80099F9C")]
    pub fn pass_check(&mut self, gobj: GObjId) -> bool {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_800C60C8");
        }
        if self.game.fighter(gobj).mv_i32(SQUAT_X0) == 0 && self.pass_input(gobj) {
            let x470 = self.common().f32(0x470);
            let fp = self.game.fighter_mut(gobj);
            fp.set_mv_i32(SQUAT_X0, 1);
            fp.set_mv_f32(SQUAT_X4, reg(x470));
            return true;
        }
        false
    }

    /// Dropping through a platform with a shoulder held, `ftCo_8009A080`.
    #[doc(alias = "ftCo_8009A080")]
    pub fn pass_check_shield(&mut self, gobj: GObjId) -> bool {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_LR != 0 && self.pass_input(gobj) {
            self.enter_pass(gobj);
            return true;
        }
        false
    }

    /// Skipping the platform stood on, `ftCo_8009A134`.
    #[doc(alias = "ftCo_8009A134")]
    pub fn pass_skip_floor(&mut self, gobj: GObjId) -> bool {
        if self.on_platform(gobj) {
            self.game.fighter_mut(gobj).coll_data.update_floor_skip();
            return true;
        }
        false
    }

    /// Dropping through a platform into `msid`, `ftCo_8009A184`.
    #[doc(alias = "ftCo_8009A184")]
    pub fn enter_pass_as(&mut self, gobj: GObjId, msid: i32, flags: u32, anim_start: f32) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.clamp_air_drift(f);
        let x46c = self.common().f32(0x46C);
        self.game.fighters[f.0].self_vel.y = x46c;
        self.change_motion_state(gobj, msid, flags, anim_start, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.coll_data.update_floor_skip();
        fp.active_timer.lstick.y = 0xFE;
    }

    /// Dropping through a platform, `ftCo_8009A228`.
    #[doc(alias = "ftCo_8009A228")]
    pub fn enter_pass(&mut self, gobj: GObjId) {
        self.enter_pass_as(gobj, ms::PASS, 0, 0.0);
    }

    /// `ftCo_Pass_IASA`.
    #[doc(alias = "ftCo_Pass_IASA")]
    pub fn co_pass_iasa(&mut self, gobj: GObjId) {
        let _ = self.special_air_check(gobj)
            || self.item_throw_air_check(gobj)
            || self.air_catch_item_check(gobj)
            || self.tether_check(gobj)
            || self.escape_air_check(gobj)
            || self.attack_air_check(gobj)
            || self.air_z_check(gobj)
            || self.jump_aerial_check(gobj);
    }

    /// `ftCo_Pass_Coll`, `ftCo_MissFoot_Coll`: `ft_80082F28`.
    #[doc(alias = "ftCo_Pass_Coll")]
    #[doc(alias = "ftCo_MissFoot_Coll")]
    pub fn co_air_full_coll(&mut self, gobj: GObjId) {
        self.air_coll_full(gobj);
    }
}
