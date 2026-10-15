//! Yoshi's ground pound, the down special (`ftyoshispeciallw.c`): a hop on the ground, then a
//! fast fall that lands in two stars, one each way.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::yoshistar;
use super::super::super::state::Running;
use super::super::fox::facing_dir_int;
use super::super::motion::flags::{
    SKIP_COL_ANIM, SKIP_HIT, SKIP_ITEM_VIS, SKIP_MAT_ANIM, SKIP_MODEL_FLAGS, SKIP_MODEL_PART_VIS,
    UNK_19, UNK_27, UPDATE_CMD,
};
use super::super::parts::TRANS_N;
use super::{addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// The ground pound's landing stars, `fn_8012E644` (the landing's accessory callback):
    /// one each side of his body at his attributes' offset, then no more.
    #[doc(alias = "fn_8012E644")]
    pub fn ys_lw_stars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let joint = self.bone_joint(gobj, TRANS_N);
        let at = crate::lb::joint::world_position(&mut self.game.objs, self.data, joint, None);
        let ox = reg(self.ys_attr_f32(f, attr::LW_STAR_OFFSET));
        let oy = reg(self.ys_attr_f32(f, attr::LW_STAR_OFFSET + 4));
        let mut pos = Vec3::new(
            mem(fp::fmadds(fp::fneg(1.0), ox, reg(at.x))),
            mem(fp::fadds(reg(at.y), oy)),
            at.z,
        );
        yoshistar::spawn(self, gobj, pos, -1.0);
        pos.x = mem(fp::fadds(reg(at.x), ox));
        yoshistar::spawn(self, gobj, pos, 1.0);
        self.game.fighters[f.0].accessory4_cb = None;
    }

    /// The ground pound's variables at its start (`ftYoshi_SpecialLw_SetVars`, `reset`): no
    /// fall yet, no throw flag, no speed or rise, and `x2223_b4`.
    fn ys_lw_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        fp.set_throw_flag(0, false);
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.x6A4_transNOffset.y = 0.0;
        fp.x2223_b4 = true;
    }

    /// `ftYs_SpecialLw_Enter`: the hop starts in the air.
    #[doc(alias = "ftYs_SpecialLw_Enter")]
    pub fn ys_special_lw_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.change_motion_state(gobj, ms::SPECIAL_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ys_lw_reset(gobj);
    }

    /// `ftYs_SpecialAirLw_Enter`.
    #[doc(alias = "ftYs_SpecialAirLw_Enter")]
    pub fn ys_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.ys_lw_reset(gobj);
    }

    /// `ftYs_SpecialLw_Anim`: the hop over, the fall from its frame 20.
    #[doc(alias = "ftYs_SpecialLw_Anim")]
    pub fn ys_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ys_lw_to_air(gobj);
        }
    }

    /// `ftYs_SpecialAirLw_Anim`: past its animation the fall holds its speed (`cmd_vars[1]`).
    #[doc(alias = "ftYs_SpecialAirLw_Anim")]
    pub fn ys_special_air_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).cmd_vars[1] = 1;
        }
    }

    /// `ftYs_SpecialLw_Phys`: the animation's motion, never down.
    #[doc(alias = "ftYs_SpecialLw_Phys")]
    pub fn ys_special_lw_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.self_vel.y < 0.0 {
            fp.self_vel.y = 0.0;
        }
    }

    /// `ftYs_SpecialAirLw_Phys`: the animation's motion, falling at least `x114`, and at it
    /// once the animation is over.
    #[doc(alias = "ftYs_SpecialAirLw_Phys")]
    pub fn ys_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let fall = self.ys_attr_f32(f, attr::X114);
        let fp = self.game.fighter_mut(gobj);
        if fp.self_vel.y < fall || fp.cmd_vars[1] != 0 {
            fp.self_vel.y = fall;
        }
    }

    /// `ftYs_SpecialAirLw_Coll` (and `ftYs_SpecialLw_Coll`): on the ground once falling it
    /// lands in the stars, before that it stays airborne; off the ground, falling, a ledge.
    #[doc(alias = "ftYs_SpecialAirLw_Coll")]
    #[doc(alias = "ftYs_SpecialLw_Coll")]
    pub fn ys_special_air_lw_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let dir = facing_dir_int(self.game.fighters[f.0].facing_dir);
        if self.air_coll_ground_and_ledge(gobj, dir) {
            let fp = &self.game.fighters[f.0];
            if fp.cmd_vars[0] != 0 && fp.self_vel.y <= 0.0 {
                self.ys_lw_landing_enter(gobj);
            } else {
                self.go_airborne(f);
            }
        } else if self.game.fighters[f.0].cmd_vars[0] != 0 && self.catch_ledge_check(gobj) {
            self.enter_cliff_catch(gobj);
        }
    }

    /// The fall from the hop's end, `ftYs_SpecialS_8012EA04`: the aerial state from its frame
    /// 20, keeping the hop's hitboxes, colors and models.
    #[doc(alias = "ftYs_SpecialS_8012EA04")]
    pub fn ys_lw_to_air(&mut self, gobj: GObjId) {
        let flags = SKIP_HIT
            | SKIP_MAT_ANIM
            | SKIP_COL_ANIM
            | UPDATE_CMD
            | SKIP_ITEM_VIS
            | UNK_19
            | SKIP_MODEL_PART_VIS
            | SKIP_MODEL_FLAGS
            | UNK_27;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_LW, flags, 20.0, 1.0, 0.0, None);
    }

    /// `ftYs_SpecialLwLanding_Anim`: waiting once it plays out.
    #[doc(alias = "ftYs_SpecialLwLanding_Anim")]
    pub fn ys_special_lw_landing_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftYs_SpecialLwLanding_Coll`: falling if it went off the ground.
    #[doc(alias = "ftYs_SpecialLwLanding_Coll")]
    pub fn ys_special_lw_landing_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The landing, `ftYs_SpecialS_8012EAD8`: still, grounded, its stars on the next frame.
    #[doc(alias = "ftYs_SpecialS_8012EAD8")]
    pub fn ys_lw_landing_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).gr_vel = 0.0;
        self.land(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_LW_LANDING, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_STARS));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_STARS => g.ys_lw_stars(gobj),
        addr::SPECIAL_LW_ENTER => g.ys_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.ys_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_ANIM => g.ys_special_lw_anim(gobj),
        addr::SPECIAL_AIR_LW_ANIM => g.ys_special_air_lw_anim(gobj),
        addr::SPECIAL_LW_PHYS => g.ys_special_lw_phys(gobj),
        addr::SPECIAL_AIR_LW_PHYS => g.ys_special_air_lw_phys(gobj),
        addr::SPECIAL_LW_COLL | addr::SPECIAL_AIR_LW_COLL => g.ys_special_air_lw_coll(gobj),
        addr::LW_FALL => g.ys_lw_to_air(gobj),
        addr::SPECIAL_LW_LANDING_ANIM => g.ys_special_lw_landing_anim(gobj),
        addr::SPECIAL_LW_LANDING_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_LW_LANDING_COLL => g.ys_special_lw_landing_coll(gobj),
        addr::LW_LANDING => g.ys_lw_landing_enter(gobj),
        _ => return false,
    }
    true
}
