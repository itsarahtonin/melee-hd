//! Disable (`ftmewtwospeciallw.c`): the flash sent from his left hand when the script says,
//! on the ground or hanging in the air with his own fall, and taken back when the move ends.
//! The flash is an item (`it::kinds::mewtwodisable`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::mewtwodisable;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{PART_L3RD_NB, addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `FTMEWTWO_SPECIALLW_COLL_FLAG`: what the switches between the ground and air keep.
const COLL_FLAGS: u32 = KEEP_GFX
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

impl Running<'_> {
    /// Disable gone from his hands, `ftMt_SpecialLw_ClearDisableGObj` (the flash's end).
    #[doc(alias = "ftMt_SpecialLw_ClearDisableGObj")]
    pub fn mt_clear_disable(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).u.set_gobj_at(u::DISABLE, None);
    }

    /// Disable taken back if out, `ftMt_SpecialLw_RemoveDisable`.
    #[doc(alias = "ftMt_SpecialLw_RemoveDisable")]
    pub fn mt_remove_disable(&mut self, gobj: GObjId) {
        if let Some(d) = self.game.fighter(gobj).u.gobj_at(u::DISABLE) {
            mewtwodisable::destroy(self, d);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::DISABLE, None);
        }
    }

    /// `ftMewtwo_SpecialLw_SetCall`: with the flash out, his cleanup when hit or KO'd.
    fn mt_lw_set_damage_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::DISABLE).is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DEATH2));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_TAKE_DAMAGE));
        }
    }

    /// The down special's state on the ground, `ftMt_SpecialLw_Enter`.
    #[doc(alias = "ftMt_SpecialLw_Enter")]
    pub fn mt_special_lw_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.u.set_gobj_at(u::DISABLE, None);
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_CREATE_DISABLE));
    }

    /// In the air, `ftMt_SpecialAirLw_Enter`: no rise.
    #[doc(alias = "ftMt_SpecialAirLw_Enter")]
    pub fn mt_special_air_lw_enter(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.x2210 = [0; 4];
        fp.cmd_vars[0] = 0;
        fp.u.set_gobj_at(u::DISABLE, None);
        fp.self_vel.y = 0.0;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_CREATE_DISABLE));
    }

    /// `ftMt_SpecialLw_Anim`: at the end the flash taken back, and waiting.
    #[doc(alias = "ftMt_SpecialLw_Anim")]
    pub fn mt_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mt_remove_disable(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftMt_SpecialAirLw_Anim`: at the end the flash taken back, and falling.
    #[doc(alias = "ftMt_SpecialAirLw_Anim")]
    pub fn mt_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mt_remove_disable(gobj);
            self.enter_fall(gobj);
        }
    }

    /// The states' empty callbacks.
    #[doc(alias = "ftMt_SpecialLw_IASA")]
    #[doc(alias = "ftMt_SpecialAirLw_IASA")]
    pub fn mt_special_lw_nothing(&mut self, _gobj: GObjId) {}

    /// `ftMt_SpecialAirLw_Phys`: his own fall (`x78`, `x7C`), slowing by the air's friction.
    #[doc(alias = "ftMt_SpecialAirLw_Phys")]
    pub fn mt_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.mt_attr_f32(f, attr::LW_GRAVITY));
        let t = reg(self.mt_attr_f32(f, attr::LW_TERMINAL_VEL));
        let fp = &mut self.game.fighters[f.0];
        Self::fall(fp, g, t);
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftMt_SpecialLw_GroundToAir`: off the ground, no rise, the drift held to the most.
    #[doc(alias = "ftMt_SpecialLw_GroundToAir")]
    pub fn mt_special_lw_to_air(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.game.fighters[f.0].self_vel.y = 0.0;
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, COLL_FLAGS, frame, 1.0, 0.0, None);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::LW_CREATE_DISABLE));
        self.mt_lw_set_damage_cbs(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftMt_SpecialAirLw_AirToGround`.
    #[doc(alias = "ftMt_SpecialAirLw_AirToGround")]
    pub fn mt_special_air_lw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_CREATE_DISABLE));
        self.mt_lw_set_damage_cbs(gobj);
    }

    /// `ftMt_SpecialLw_Coll`.
    #[doc(alias = "ftMt_SpecialLw_Coll")]
    pub fn mt_special_lw_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.mt_special_lw_to_air(gobj));
    }

    /// `ftMt_SpecialAirLw_Coll`.
    #[doc(alias = "ftMt_SpecialAirLw_Coll")]
    pub fn mt_special_air_lw_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.mt_special_air_lw_to_ground(gobj));
    }

    /// The flash sent when the script says (`cmd_vars[0]`), `ftMt_SpecialLw_CreateDisable`
    /// (an accessory): from his left hand's third joint, offset his way (`x80`, `x84`).
    #[doc(alias = "ftMt_SpecialLw_CreateDisable")]
    pub fn mt_lw_create_disable(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            return;
        }
        let j = self.bone_joint(gobj, PART_L3RD_NB);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let ox = reg(self.mt_attr_f32(f, attr::LW_OFFSET_X));
        let oy = reg(self.mt_attr_f32(f, attr::LW_OFFSET_Y));
        let facing = self.game.fighters[f.0].facing_dir;
        pos.x = mem(fp::fmadds(ox, reg(facing), reg(pos.x)));
        pos.y = mem(fp::fadds(reg(pos.y), oy));
        let d = mewtwodisable::spawn(self, gobj, pos, facing);
        self.game.fighters[f.0].u.set_gobj_at(u::DISABLE, d);
        self.mt_lw_set_damage_cbs(gobj);
        self.game.fighters[f.0].cmd_vars[0] = 0;
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_CLEAR_DISABLE => g.mt_clear_disable(gobj),
        addr::LW_REMOVE_DISABLE => g.mt_remove_disable(gobj),
        addr::SPECIAL_LW_ENTER => g.mt_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.mt_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.mt_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.mt_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_IASA | addr::SPECIAL_AIR_LW_IASA => g.mt_special_lw_nothing(gobj),
        addr::SPECIAL_LW_PHYS => g.mt_ground_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.mt_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_GROUND_TO_AIR => g.mt_special_lw_to_air(gobj),
        addr::SPECIAL_AIR_LW_AIR_TO_GROUND => g.mt_special_air_lw_to_ground(gobj),
        addr::SPECIAL_LW_COLL => g.mt_special_lw_coll(gobj),
        addr::SPECIAL_AIR_LW_COLL => g.mt_special_air_lw_coll(gobj),
        addr::LW_CREATE_DISABLE => g.mt_lw_create_disable(gobj),
        _ => return false,
    }
    true
}
