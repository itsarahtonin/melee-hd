//! Whirling Fortress, Bowser's up special (`ftkoopaspecialhi.c`): spinning in his shell, on
//! the ground steered by the stick, in the air rising and drifting, turning his model's lean
//! with the floor; the shell's intangibility follows the script (`ftKp_SpecialN_80135780`).

use gekko_fp as fp;

use super::super::super::ef;
use super::super::super::state::Running;
use super::super::common::GA_AIR;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The flags the spin keeps when it switches between the ground and the air.
const SWITCH: u32 = 0x0C4C_5292;

impl Running<'_> {
    /// The spin's common start (`ftKp_SpecialHi_Enter`'s and `ftKp_SpecialAirHi_Enter`'s
    /// tail): its jumps used up, no frames spun yet, off the ground, the spin's effect on his
    /// top joint, and his effects paused in hitlag.
    fn kp_hi_start(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.set_mv_f32(mv::HI_FRAMES, 0.0);
        fp.set_mv_i32(mv::HI_GROUNDED, 0);
        let top = self.bone_joint(gobj, 0);
        ef::spawn(self, 0x4DA, Some(gobj), &[ef::Arg::JObj(top)]);
        self.game.fighter_mut(gobj).x2219_b0 = true;
        self.set_effect_hitlag_cbs(gobj);
        self.anim_update(gobj);
    }

    /// `ftKp_SpecialHi_Enter`.
    #[doc(alias = "ftKp_SpecialHi_Enter")]
    pub fn kp_special_hi_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.self_vel.y = 0.0;
        self.kp_hi_start(gobj);
    }

    /// `ftKp_SpecialAirHi_Enter`: the ground speed held to `x60`, the rise `x54`, and the
    /// spin's first frame at once.
    #[doc(alias = "ftKp_SpecialAirHi_Enter")]
    pub fn kp_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        let x60 = reg(self.kp_attr_f32(f, attr::X60));
        let x54 = self.kp_attr_f32(f, attr::X54);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        Self::clamp_ground_vel(fp, x60);
        fp.self_vel.y = x54;
        self.kp_hi_start(gobj);
        self.kp_special_hi_anim(gobj);
    }

    /// A frame more spun (`mv.co.walk.middle_anim_frame`) and the shell.
    fn kp_hi_tick(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp::fadds(reg(fp.mv_f32(mv::HI_FRAMES)), 1.0);
        fp.set_mv_f32(mv::HI_FRAMES, n);
        self.kp_hi_shell(gobj);
    }

    /// `ftKp_SpecialHi_Anim`: waiting once it plays out.
    #[doc(alias = "ftKp_SpecialHi_Anim")]
    pub fn kp_special_hi_anim(&mut self, gobj: GObjId) {
        self.kp_hi_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The spin's helpless end (`x7C`'s landing lag, or a plain fall without one).
    fn kp_hi_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.kp_attr_f32(f, attr::X7C);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// `ftKp_SpecialAirHi_Anim`: when the script ends the spin in the air (`cmd_vars[2]` 2),
    /// its effects go and he falls; else falling once it plays out.
    #[doc(alias = "ftKp_SpecialAirHi_Anim")]
    pub fn kp_special_air_hi_anim(&mut self, gobj: GObjId) {
        self.kp_hi_tick(gobj);
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] == 2 {
            if self.game.fighters[f.0].ground_or_air == GA_AIR {
                self.go_airborne_jumped(f);
                ef::eflib::destroy_all(self, gobj);
                let fp = &mut self.game.fighters[f.0];
                fp.x2219_b0 = false;
                fp.pre_hitlag_cb = None;
                fp.post_hitlag_cb = None;
                self.kp_hi_fall(gobj);
            }
        } else if !self.anim_frames_remaining(gobj) {
            self.go_airborne_jumped(f);
            self.kp_hi_fall(gobj);
        }
    }

    /// `ftKp_SpecialHi_Phys`: until the script says (`cmd_vars[0]`), steered by the stick
    /// (`x68` toward `x60`); then friction.
    #[doc(alias = "ftKp_SpecialHi_Phys")]
    pub fn kp_special_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            let x68 = reg(self.kp_attr_f32(f, attr::X68));
            let x60 = reg(self.kp_attr_f32(f, attr::X60));
            Self::ground_accel_to_stick(&mut self.game.fighters[f.0], 0.0, x68, x60);
            self.self_movement_from_ground(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftKp_SpecialAirHi_Phys`: until the script says, falling by `x58` to `x5C` and
    /// drifting (`x6C` toward `x64`); then the air's physics.
    #[doc(alias = "ftKp_SpecialAirHi_Phys")]
    pub fn kp_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            let x58 = reg(self.kp_attr_f32(f, attr::X58));
            let x5c = reg(self.kp_attr_f32(f, attr::X5C));
            let x6c = reg(self.kp_attr_f32(f, attr::X6C));
            let x64 = reg(self.kp_attr_f32(f, attr::X64));
            let fp = &mut self.game.fighters[f.0];
            Self::fall(fp, x58, x5c);
            Self::drift_simple(fp, 0.0, x6c, x64);
        } else {
            self.air_physics(gobj);
        }
    }

    /// The model's lean (`ftKp_SpecialHi_Coll_inline`): with the script's `cmd_vars[3]` and
    /// on the ground, tilted with the floor; else upright.
    fn kp_hi_lean(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let v = if fp.cmd_vars[3] != 0 && fp.mv_i32(mv::HI_GROUNDED) != 0 {
            let n = fp.coll_data.floor.normal;
            let a = reg(crate::math::atan2(n.x, n.y));
            mem(fp::fmuls(reg(fp.facing_dir), a))
        } else {
            0.0
        };
        self.game.part_set_rot_x(f, 0, v);
    }

    /// `ftKp_SpecialHi_Coll`: off the ground, the aerial spin from the same frame (its speed
    /// held to `x64`); the shell and the lean.
    #[doc(alias = "ftKp_SpecialHi_Coll")]
    pub fn kp_special_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if !self.ground_coll_off_ledges(gobj) {
            self.go_airborne_jumped(f);
            let frame = self.game.fighters[f.0].cur_anim_frame;
            self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, SWITCH, frame, 1.0, 0.0, None);
            self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
            let x64 = reg(self.kp_attr_f32(f, attr::X64));
            Self::clamp_self_vel_x(&mut self.game.fighters[f.0], x64);
            self.set_effect_hitlag_cbs(gobj);
            self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 0);
        } else {
            self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 1);
        }
        self.kp_hi_shell(gobj);
        self.kp_hi_lean(gobj);
    }

    /// The aerial spin landing (`ftKp_SpecialAirHi_Coll_inline`): the grounded spin from the
    /// same frame (from `x78` frames before it in a window of `x70` that can't hold), its
    /// ground speed held to `x60`.
    fn kp_air_hi_land(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.land(gobj);
        let x70 = self.kp_attr_f32(f, attr::X70);
        let x78 = reg(self.kp_attr_f32(f, attr::X78));
        let fp = &self.game.fighters[f.0];
        let spun = fp.mv_f32(mv::HI_FRAMES);
        let frame = if x70 < spun && spun < x70 {
            mem(fp::fsubs(reg(fp.cur_anim_frame), x78))
        } else {
            fp.cur_anim_frame
        };
        self.change_motion_state(gobj, ms::SPECIAL_HI, SWITCH, frame, 1.0, 0.0, None);
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
        let x60 = reg(self.kp_attr_f32(f, attr::X60));
        Self::clamp_ground_vel(&mut self.game.fighters[f.0], x60);
    }

    /// `ftKp_SpecialAirHi_Coll`: rising, landing as the ground's spin; falling, landing (on
    /// the ground or a ledge's edge) likewise, else a ledge catch. The shell and the lean
    /// follow a landing or a rise.
    #[doc(alias = "ftKp_SpecialAirHi_Coll")]
    pub fn kp_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].self_vel.y >= 0.0 {
            if self.air_coll_land(gobj) {
                self.kp_air_hi_land(gobj);
                self.set_effect_hitlag_cbs(gobj);
                self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 1);
            } else {
                self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 0);
            }
            self.kp_hi_shell(gobj);
            self.kp_hi_lean(gobj);
        } else if self.air_coll_ground_and_ledge(gobj, 0) {
            self.kp_air_hi_land(gobj);
            self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 1);
            self.kp_hi_shell(gobj);
            self.kp_hi_lean(gobj);
            self.set_effect_hitlag_cbs(gobj);
        } else {
            self.game.fighters[f.0].set_mv_i32(mv::HI_GROUNDED, 0);
            if self.catch_ledge_check(gobj) {
                self.enter_cliff_catch(gobj);
            }
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ENTER => g.kp_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.kp_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM => g.kp_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.kp_special_air_hi_anim(gobj),
        // `ftKp_SpecialHi_IASA` and `ftKp_SpecialAirHi_IASA`: nothing.
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => {}
        addr::SPECIAL_HI_PHYS => g.kp_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.kp_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_COLL => g.kp_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.kp_special_air_hi_coll(gobj),
        _ => return false,
    }
    true
}
