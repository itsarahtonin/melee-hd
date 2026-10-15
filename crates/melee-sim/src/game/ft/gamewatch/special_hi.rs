//! Fire (`ftgamewatchspecialhi.c`): the firemen's trampoline (`it::kinds::gamewatchrescue`)
//! under him throws him up, angled once by the stick, and he drifts down helpless under his
//! parachute (its animation).

use gekko_fp as fp;

use super::super::super::it::kinds::gamewatchrescue as rescue;
use super::super::super::state::Running;
use super::super::sound::Track;
use super::{addr, attr, ms, part, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The trampoline's jump sound.
const SFX_JUMP: i32 = 290_066;

/// `stickGetDir`: the stick's distance from `x2`.
fn stick_dir(x1: f64, x2: f64) -> f64 {
    if x1 < x2 { fp::fneg(x1) } else { x1 }
}

impl Running<'_> {
    /// The trampoline under him, `ftGw_SpecialHi_ItemRescueSetup`: 2.5 below his root (by his
    /// model's scale), in the state's variant (ground or air).
    #[doc(alias = "ftGw_SpecialHi_ItemRescueSetup")]
    pub fn gw_rescue_setup(&mut self, gobj: GObjId) {
        if self.gw_item(gobj, u::RESCUE).is_some() {
            return;
        }
        let mut pos = self.gw_part_pos(gobj, part::TOP_N, None);
        let fp = self.game.fighter(gobj);
        let scale = Self::model_scale(fp);
        pos.y = mem(fp::fnmsubs(2.5, scale, reg(pos.y)));
        let (variant, facing) = (fp.motion_id - ms::SPECIAL_HI, fp.facing_dir);
        let item = rescue::spawn(self, Some(gobj), pos, part::TOP_N, variant, facing, 2.5);
        self.gw_set_item(gobj, u::RESCUE, item);
        if item.is_some() {
            self.gw_damage_cbs(gobj);
        }
        self.gw_hitlag_cbs(
            gobj,
            addr::SPECIAL_HI_ITEM_RESCUE_ENTER_HITLAG,
            addr::SPECIAL_HI_ITEM_RESCUE_EXIT_HITLAG,
        );
        self.gw_set_accessory(gobj, None);
    }

    /// Whether the trampoline should go, `ftGw_SpecialHi_ItemCheckRescueRemove`: out of Fire.
    #[doc(alias = "ftGw_SpecialHi_ItemCheckRescueRemove")]
    pub fn gw_rescue_should_go(&self, gobj: GObjId) -> bool {
        !matches!(
            self.game.fighter(gobj).motion_id,
            ms::SPECIAL_HI | ms::SPECIAL_AIR_HI
        )
    }

    /// The trampoline forgotten, `ftGw_SpecialHi_ItemRescueSetNULL`.
    #[doc(alias = "ftGw_SpecialHi_ItemRescueSetNULL")]
    pub fn gw_rescue_set_null(&mut self, gobj: GObjId) {
        self.gw_set_item(gobj, u::RESCUE, None);
        self.gw_clear_damage_cbs(gobj);
    }

    /// The trampoline taken away, `ftGw_SpecialHi_ItemRescueRemove`.
    #[doc(alias = "ftGw_SpecialHi_ItemRescueRemove")]
    pub fn gw_rescue_remove(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::RESCUE) {
            rescue::remove(self, item);
            self.gw_rescue_set_null(gobj);
        }
    }

    /// `ftGw_SpecialHi_ItemRescueEnterHitlag`.
    #[doc(alias = "ftGw_SpecialHi_ItemRescueEnterHitlag")]
    pub fn gw_rescue_enter_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::RESCUE) {
            rescue::hitlag_start(self, item);
        }
    }

    /// `ftGw_SpecialHi_ItemRescueExitHitlag`.
    #[doc(alias = "ftGw_SpecialHi_ItemRescueExitHitlag")]
    pub fn gw_rescue_exit_hitlag(&mut self, gobj: GObjId) {
        if let Some(item) = self.gw_item(gobj, u::RESCUE) {
            rescue::hitlag_end(self, item);
        }
    }

    /// Fire's start (`ftGameWatch_SpecialHi_SetVars`): the script's variables cleared.
    fn gw_special_hi_start(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.gw_set_accessory(gobj, Some(addr::SPECIAL_HI_ITEM_RESCUE_SETUP));
    }

    /// `ftGw_SpecialHi_Enter`: from the ground, no fall, into the air with his jumps used.
    #[doc(alias = "ftGw_SpecialHi_Enter")]
    pub fn gw_special_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x74_self_accel.y = 0.0;
        fp.self_vel.y = 0.0;
        self.go_airborne_jumped(f);
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.gw_special_hi_start(gobj);
        self.anim_update(gobj);
        self.fighter_play_track(f, Track::Effect2, SFX_JUMP, 127, 64);
    }

    /// `ftGw_SpecialAirHi_Enter`.
    #[doc(alias = "ftGw_SpecialAirHi_Enter")]
    pub fn gw_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.gw_special_hi_start(gobj);
        self.anim_update(gobj);
        self.fighter_play_track(f, Track::Effect2, SFX_JUMP, 127, 64);
    }

    /// Fire's animation (`ftGw_SpecialHi_Anim` calls `ftGw_SpecialAirHi_Anim`): once its frames
    /// run out, a fall, helpless with the attributes' landing lag unless that's 0.
    #[doc(alias = "ftGw_SpecialHi_Anim")]
    #[doc(alias = "ftGw_SpecialAirHi_Anim")]
    pub fn gw_special_hi_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let landing = self.gw_attr_f32(f, attr::RESCUE_LANDING);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        if reg(landing) == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, landing, 0.0, true);
    }

    /// Fire's interrupt (`ftGw_SpecialHi_IASA` calls `ftGw_SpecialAirHi_IASA`): once, the stick
    /// past the attributes' range turns him its way and angles the throw.
    #[doc(alias = "ftGw_SpecialHi_IASA")]
    #[doc(alias = "ftGw_SpecialAirHi_IASA")]
    pub fn gw_special_hi_iasa(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let range = reg(self.gw_attr_f32(f, attr::RESCUE_STICK_RANGE));
        let angle_mul = reg(self.gw_attr_f32(f, attr::RESCUE_ANGLE));
        let fp = &self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 {
            return;
        }
        let x = reg(fp.input.lstick[0].x);
        let stick = stick_dir(x, 0.0);
        if stick > range {
            let over = fp::fsubs(stick, range);
            let dir = if x > 0.0 { 1.0 } else { fp::fneg(1.0) };
            let angle = fp::fmuls(fp::fmuls(over, dir), angle_mul);
            self.face_stick(gobj);
            let facing = reg(self.game.fighters[f.0].facing_dir);
            let rot = fp::frsp(fp::fmul(fp::fdiv(std::f64::consts::PI, 2.0), facing));
            self.game.part_set_rot_y(f, 0, mem(rot));
            let fp = &mut self.game.fighters[f.0];
            fp.lstick_angle = mem(fp::fneg(angle));
            fp.cmd_vars[0] = 1;
        }
    }

    /// Fire's physics (`ftGw_SpecialHi_Phys` calls `ftGw_SpecialAirHi_Phys`): the animation's
    /// motion turned by the angle (`ft_80085154`).
    #[doc(alias = "ftGw_SpecialHi_Phys")]
    #[doc(alias = "ftGw_SpecialAirHi_Phys")]
    pub fn gw_special_hi_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion_angled(gobj);
    }

    /// Fire's collision (`ftGw_SpecialHi_Coll` calls `ftGw_SpecialAirHi_Coll`): past frame 4,
    /// on the way up a landing ends it; on the way down a landing or a ledge.
    #[doc(alias = "ftGw_SpecialHi_Coll")]
    #[doc(alias = "ftGw_SpecialAirHi_Coll")]
    pub fn gw_special_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let landing = self.gw_attr_f32(f, attr::RESCUE_LANDING);
        let fp = &self.game.fighters[f.0];
        if fp.cur_anim_frame <= 4.0 || fp.cur_anim_frame.is_nan() {
            return;
        }
        if fp.self_vel.y >= 0.0 {
            if self.air_coll_land(gobj) {
                self.gw_rescue_remove(gobj);
                self.land(gobj);
                self.enter_landing_fall_special(gobj, 0, landing);
                self.gw_clear_damage_cbs(gobj);
            }
        } else {
            let dir = if self.game.fighters[f.0].facing_dir == 1.0 {
                1
            } else {
                -1
            };
            if self.air_coll_ground_and_ledge(gobj, dir) {
                self.enter_landing_fall_special(gobj, 0, landing);
            } else if self.catch_ledge_check(gobj) {
                self.enter_cliff_catch(gobj);
            }
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ITEM_RESCUE_SETUP => g.gw_rescue_setup(gobj),
        addr::SPECIAL_HI_ITEM_RESCUE_SET_NULL => g.gw_rescue_set_null(gobj),
        addr::SPECIAL_HI_ITEM_RESCUE_REMOVE => g.gw_rescue_remove(gobj),
        addr::SPECIAL_HI_ITEM_RESCUE_ENTER_HITLAG => g.gw_rescue_enter_hitlag(gobj),
        addr::SPECIAL_HI_ITEM_RESCUE_EXIT_HITLAG => g.gw_rescue_exit_hitlag(gobj),
        addr::SPECIAL_HI_ENTER => g.gw_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.gw_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM | addr::SPECIAL_AIR_HI_ANIM => g.gw_special_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => g.gw_special_hi_iasa(gobj),
        addr::SPECIAL_HI_PHYS | addr::SPECIAL_AIR_HI_PHYS => g.gw_special_hi_phys(gobj),
        addr::SPECIAL_HI_COLL | addr::SPECIAL_AIR_HI_COLL => g.gw_special_hi_coll(gobj),
        _ => return false,
    }
    true
}
