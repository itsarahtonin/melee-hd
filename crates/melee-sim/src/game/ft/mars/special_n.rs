//! Shield Breaker (`ftmarsspecialn.c` and the accessories in `ftmars.c`): the draw, the
//! charge while B is held (its wind every half second), and the swing, stronger for each
//! half second charged and fully charged on its own once the charge runs out.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{KIND_EMBLEM, KIND_MARS, PAD_B, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// What the draw keeps between the ground and the air.
const START_FLAGS: u32 = KEEP_COL_ANIM_HIT_STATUS
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// What the charge keeps between the ground and the air.
const LOOP_FLAGS: u32 = KEEP_GFX
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_MAT_ANIM
    | KEEP_SFX
    | SKIP_RUMBLE
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// What the swing keeps between the ground and the air.
const END_FLAGS: u32 = KEEP_GFX
    | KEEP_COL_ANIM_HIT_STATUS
    | SKIP_HIT
    | SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// `MTXDegToRad(60)`, as MWCC folds it.
fn sixty_degrees() -> f32 {
    mem(fp::fmuls(fp::frsp(60.0), 0.017_453_292_384_743_69))
}

impl Running<'_> {
    /// The swing's flash, once (`x2219_b0`), then the hitlag hooks and no more accessory:
    /// `ftMs_SpecialN_801365A8` (grounded) and `ftMs_SpecialN_8013666C` (aerial).
    fn mars_special_n_flash(&mut self, gobj: GObjId, gfx: [i32; 2]) {
        if !self.game.fighter(gobj).x2219_b0 {
            let id = match self.game.ft_kind(gobj) {
                KIND_MARS => Some(gfx[0]),
                KIND_EMBLEM => Some(gfx[1]),
                _ => None,
            };
            if let Some(id) = id {
                let j = self.bone_joint(gobj, 0);
                spawn(self, id, Some(gobj), &[Arg::JObj(j)]);
            }
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The grounded swing's flash, `ftMs_SpecialN_801365A8`.
    #[doc(alias = "ftMs_SpecialN_801365A8")]
    pub fn mars_special_n_flash_ground(&mut self, gobj: GObjId) {
        self.mars_special_n_flash(gobj, [1266, 1297]);
    }

    /// The aerial swing's flash, `ftMs_SpecialN_8013666C`.
    #[doc(alias = "ftMs_SpecialN_8013666C")]
    pub fn mars_special_n_flash_air(&mut self, gobj: GObjId) {
        self.mars_special_n_flash(gobj, [1267, 1298]);
    }

    /// Leaving the move, `ftMs_SpecialN_80136730` (`x21EC`): no charge.
    #[doc(alias = "ftMs_SpecialN_80136730")]
    pub fn mars_special_n_reset(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::N_CUR_FRAME, 0);
    }

    /// The grounded draw, `ftMs_SpecialN_Enter`.
    #[doc(alias = "ftMs_SpecialN_Enter")]
    pub fn mars_special_n_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.mars_attr_f32(f, attr::SPECIALN_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::N_80136730));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), friction));
        self.change_motion_state(gobj, ms::SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The aerial draw, `ftMs_SpecialAirN_Enter`: no rise kept.
    #[doc(alias = "ftMs_SpecialAirN_Enter")]
    pub fn mars_special_air_n_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.mars_attr_f32(f, attr::SPECIALN_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        fp.x21EC = Some(CodeRef(addr::N_80136730));
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), friction));
        if fp.self_vel.y <= 0.0 {
            fp.self_vel.y = 0.0;
        }
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The draws' end (`doStartAnim`): the charge, and the charge's flash.
    fn mars_special_n_start_end(&mut self, gobj: GObjId, air: bool) {
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.mars_special_n_air_loop(gobj);
            } else {
                self.mars_special_n_ground_loop(gobj);
            }
            let color = if self.game.fighter(gobj).kind == KIND_MARS {
                99
            } else {
                100
            };
            self.start_color_anim(gobj, color, 0);
        }
    }

    /// `ftMs_SpecialNStart_Anim`.
    #[doc(alias = "ftMs_SpecialNStart_Anim")]
    pub fn mars_special_n_start_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_start_end(gobj, false);
    }

    /// `ftMs_SpecialAirNStart_Anim`.
    #[doc(alias = "ftMs_SpecialAirNStart_Anim")]
    pub fn mars_special_air_n_start_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_start_end(gobj, true);
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftMs_SpecialNStart_IASA")]
    #[doc(alias = "ftMs_SpecialAirNStart_IASA")]
    #[doc(alias = "ftMs_SpecialNEnd_IASA")]
    #[doc(alias = "ftMs_SpecialAirNEnd_IASA")]
    #[doc(alias = "ftMs_SpecialS4_IASA")]
    #[doc(alias = "ftMs_SpecialLw_IASA")]
    #[doc(alias = "ftMs_SpecialAirLw_IASA")]
    #[doc(alias = "ftMs_SpecialLwHit_IASA")]
    #[doc(alias = "ftMs_SpecialAirLwHit_IASA")]
    pub fn mars_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftMs_SpecialNStart_Phys`: the draw's own friction.
    #[doc(alias = "ftMs_SpecialNStart_Phys")]
    pub fn mars_special_n_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.mars_attr_f32(f, attr::SPECIALN_START_FRICTION));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], friction);
        self.self_movement_from_ground(gobj);
    }

    /// `ftMs_SpecialAirNStart_Phys`: gravity, and the draw's friction in the air.
    #[doc(alias = "ftMs_SpecialAirNStart_Phys")]
    pub fn mars_special_air_n_start_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let friction = reg(self.mars_attr_f32(f, attr::SPECIALN_START_FRICTION));
        let fp = &mut self.game.fighters[f.0];
        Self::fall_basic(fp);
        Self::self_accel_deaccel(fp, friction);
    }

    /// `ftMs_SpecialNStart_Coll`.
    #[doc(alias = "ftMs_SpecialNStart_Coll")]
    pub fn mars_special_n_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_start_ground_to_air(gobj);
        }
    }

    /// `ftMs_SpecialAirNStart_Coll`.
    #[doc(alias = "ftMs_SpecialAirNStart_Coll")]
    pub fn mars_special_air_n_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_start_air_to_ground(gobj);
        }
    }

    /// The draw off a ledge, `ftMs_SpecialN_80136A1C`.
    #[doc(alias = "ftMs_SpecialN_80136A1C")]
    pub fn mars_special_n_start_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N_START, START_FLAGS);
    }

    /// The draw landing, `ftMs_SpecialN_80136A7C`.
    #[doc(alias = "ftMs_SpecialN_80136A7C")]
    pub fn mars_special_n_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N_START, START_FLAGS);
    }

    /// The charge (`doLoopAnim`): wind every half second, and the swing at full charge.
    fn mars_special_n_charge(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        let full = self.mars_attr_i32(f, attr::X0);
        if self.game.fighters[f.0].mv_i32(mv::N_CUR_FRAME) % 30 == 0 {
            self.mars_hip_wind(gobj, 10, 0.5, 0.05, sixty_degrees());
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_CUR_FRAME).wrapping_add(1);
        fp.set_mv_i32(mv::N_CUR_FRAME, n);
        if n > full.wrapping_mul(30) {
            fp.cmd_vars[0] = 1;
            if air {
                self.mars_special_air_n_swing(gobj);
            } else {
                self.mars_special_n_swing(gobj);
            }
        }
    }

    /// `ftMs_SpecialNLoop_Anim`.
    #[doc(alias = "ftMs_SpecialNLoop_Anim")]
    pub fn mars_special_n_loop_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_charge(gobj, false);
    }

    /// `ftMs_SpecialAirNLoop_Anim`.
    #[doc(alias = "ftMs_SpecialAirNLoop_Anim")]
    pub fn mars_special_air_n_loop_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_charge(gobj, true);
    }

    /// The charge's release (`doLoopIasa`): letting go of B swings, not fully charged.
    fn mars_special_n_release(&mut self, gobj: GObjId, air: bool) {
        if self.game.fighter(gobj).input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 0;
            if air {
                self.mars_special_air_n_swing(gobj);
            } else {
                self.mars_special_n_swing(gobj);
            }
        }
    }

    /// `ftMs_SpecialNLoop_IASA`.
    #[doc(alias = "ftMs_SpecialNLoop_IASA")]
    pub fn mars_special_n_loop_iasa(&mut self, gobj: GObjId) {
        self.mars_special_n_release(gobj, false);
    }

    /// `ftMs_SpecialAirNLoop_IASA`.
    #[doc(alias = "ftMs_SpecialAirNLoop_IASA")]
    pub fn mars_special_air_n_loop_iasa(&mut self, gobj: GObjId) {
        self.mars_special_n_release(gobj, true);
    }

    /// Grounded friction, `ft_80084F3C`.
    #[doc(alias = "ftMs_SpecialNLoop_Phys")]
    #[doc(alias = "ftMs_SpecialNEnd_Phys")]
    #[doc(alias = "ftMs_SpecialLwHit_Phys")]
    pub fn mars_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics without drift, `ft_80084EEC`.
    #[doc(alias = "ftMs_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftMs_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftMs_SpecialAirLwHit_Phys")]
    pub fn mars_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// `ftMs_SpecialNLoop_Coll`.
    #[doc(alias = "ftMs_SpecialNLoop_Coll")]
    pub fn mars_special_n_loop_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_loop_ground_to_air(gobj);
        }
    }

    /// `ftMs_SpecialAirNLoop_Coll`.
    #[doc(alias = "ftMs_SpecialAirNLoop_Coll")]
    pub fn mars_special_air_n_loop_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_loop_air_to_ground(gobj);
        }
    }

    /// The charge off a ledge, `ftMs_SpecialN_80136DB4`.
    #[doc(alias = "ftMs_SpecialN_80136DB4")]
    pub fn mars_special_n_loop_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N_LOOP, LOOP_FLAGS);
    }

    /// The charge landing, `ftMs_SpecialN_80136E14`.
    #[doc(alias = "ftMs_SpecialN_80136E14")]
    pub fn mars_special_n_loop_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N_LOOP, LOOP_FLAGS);
    }

    /// The grounded charge, `ftMs_SpecialN_80136E74`.
    #[doc(alias = "ftMs_SpecialN_80136E74")]
    pub fn mars_special_n_ground_loop(&mut self, gobj: GObjId) {
        let flags = KEEP_SFX | SKIP_COL_ANIM;
        self.change_motion_state(gobj, ms::SPECIAL_N_LOOP, flags, 0.0, 1.0, 0.0, None);
    }

    /// The aerial charge, `ftMs_SpecialN_80136EAC`.
    #[doc(alias = "ftMs_SpecialN_80136EAC")]
    pub fn mars_special_n_air_loop(&mut self, gobj: GObjId) {
        let flags = KEEP_SFX | SKIP_COL_ANIM;
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_LOOP, flags, 0.0, 1.0, 0.0, None);
    }

    /// The swing (`inlineA0`): unless fully charged, the hitboxes' damage grows with each
    /// half second charged; wind from the hip at frame 9; then waiting or falling.
    fn mars_special_n_swing_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            let x4 = self.mars_attr_i32(f, attr::X4);
            let x8 = self.mars_attr_i32(f, attr::X8);
            for i in 0..4 {
                if self.game.fighters[f.0].x914[i].state == hitcapsule::ENABLED {
                    let n = self.game.fighters[f.0].mv_i32(mv::N_CUR_FRAME);
                    let d = x4.wrapping_add((n / 30).wrapping_mul(x8));
                    let d = crate::ppc::cvt_fp2unsigned(fp::frsp(f64::from(d)));
                    self.hitbox_damage(f, i, false, d);
                }
            }
        }
        if self.game.fighters[f.0].cur_anim_frame == 9.0 {
            self.mars_hip_wind(gobj, 120, 0.9, 0.02, sixty_degrees());
        }
        if !self.anim_frames_remaining(gobj) {
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftMs_SpecialNEnd_Anim`.
    #[doc(alias = "ftMs_SpecialNEnd_Anim")]
    pub fn mars_special_n_end_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_swing_anim(gobj, false);
    }

    /// `ftMs_SpecialAirNEnd_Anim`.
    #[doc(alias = "ftMs_SpecialAirNEnd_Anim")]
    pub fn mars_special_air_n_end_anim(&mut self, gobj: GObjId) {
        self.mars_special_n_swing_anim(gobj, true);
    }

    /// `ftMs_SpecialNEnd_Coll`.
    #[doc(alias = "ftMs_SpecialNEnd_Coll")]
    pub fn mars_special_n_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.mars_special_n_end_ground_to_air(gobj);
        }
    }

    /// `ftMs_SpecialAirNEnd_Coll`.
    #[doc(alias = "ftMs_SpecialAirNEnd_Coll")]
    pub fn mars_special_air_n_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.mars_special_n_end_air_to_ground(gobj);
        }
    }

    /// The swing's state for its charge: `full` or not.
    fn mars_swing_state(&self, gobj: GObjId, ground: i32, air: i32) -> (i32, i32) {
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            (ground, air)
        } else {
            (ground + 1, air + 1)
        }
    }

    /// The swing off a ledge, `ftMs_SpecialN_801371FC`.
    #[doc(alias = "ftMs_SpecialN_801371FC")]
    pub fn mars_special_n_end_ground_to_air(&mut self, gobj: GObjId) {
        let (_, msid) = self.mars_swing_state(gobj, ms::SPECIAL_N_END_0, ms::SPECIAL_AIR_N_END_0);
        self.fox_ground_to_air(gobj, msid, END_FLAGS);
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The swing landing, `ftMs_SpecialN_801372A8`.
    #[doc(alias = "ftMs_SpecialN_801372A8")]
    pub fn mars_special_n_end_air_to_ground(&mut self, gobj: GObjId) {
        let (msid, _) = self.mars_swing_state(gobj, ms::SPECIAL_N_END_0, ms::SPECIAL_AIR_N_END_0);
        self.fox_air_to_ground(gobj, msid, END_FLAGS);
        if self.game.fighter(gobj).x2219_b0 {
            self.set_effect_hitlag_cbs(gobj);
        }
    }

    /// The grounded swing, `ftMs_SpecialN_80137354`.
    #[doc(alias = "ftMs_SpecialN_80137354")]
    pub fn mars_special_n_swing(&mut self, gobj: GObjId) {
        let (msid, _) = self.mars_swing_state(gobj, ms::SPECIAL_N_END_0, ms::SPECIAL_AIR_N_END_0);
        self.change_motion_state(gobj, msid, 0, 1.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::N_801365A8));
    }

    /// The aerial swing, `ftMs_SpecialN_801373B8`.
    #[doc(alias = "ftMs_SpecialN_801373B8")]
    pub fn mars_special_air_n_swing(&mut self, gobj: GObjId) {
        let (_, msid) = self.mars_swing_state(gobj, ms::SPECIAL_N_END_0, ms::SPECIAL_AIR_N_END_0);
        self.change_motion_state(gobj, msid, 0, 1.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::N_8013666C));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::N_801365A8 => g.mars_special_n_flash_ground(gobj),
        addr::N_8013666C => g.mars_special_n_flash_air(gobj),
        addr::N_80136730 => g.mars_special_n_reset(gobj),
        addr::SPECIAL_N_ENTER => g.mars_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.mars_special_air_n_enter(gobj),
        addr::SPECIAL_N_START_ANIM => g.mars_special_n_start_anim(gobj),
        addr::SPECIAL_AIR_N_START_ANIM => g.mars_special_air_n_start_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_END_IASA => g.mars_no_iasa(gobj),
        addr::SPECIAL_N_START_PHYS => g.mars_special_n_start_phys(gobj),
        addr::SPECIAL_AIR_N_START_PHYS => g.mars_special_air_n_start_phys(gobj),
        addr::SPECIAL_N_START_COLL => g.mars_special_n_start_coll(gobj),
        addr::SPECIAL_AIR_N_START_COLL => g.mars_special_air_n_start_coll(gobj),
        addr::N_80136A1C => g.mars_special_n_start_ground_to_air(gobj),
        addr::N_80136A7C => g.mars_special_n_start_air_to_ground(gobj),
        addr::SPECIAL_N_LOOP_ANIM => g.mars_special_n_loop_anim(gobj),
        addr::SPECIAL_AIR_N_LOOP_ANIM => g.mars_special_air_n_loop_anim(gobj),
        addr::SPECIAL_N_LOOP_IASA => g.mars_special_n_loop_iasa(gobj),
        addr::SPECIAL_AIR_N_LOOP_IASA => g.mars_special_air_n_loop_iasa(gobj),
        addr::SPECIAL_N_LOOP_PHYS | addr::SPECIAL_N_END_PHYS => g.mars_ground_phys(gobj),
        addr::SPECIAL_AIR_N_LOOP_PHYS | addr::SPECIAL_AIR_N_END_PHYS => g.mars_air_phys(gobj),
        addr::SPECIAL_N_LOOP_COLL => g.mars_special_n_loop_coll(gobj),
        addr::SPECIAL_AIR_N_LOOP_COLL => g.mars_special_air_n_loop_coll(gobj),
        addr::N_80136DB4 => g.mars_special_n_loop_ground_to_air(gobj),
        addr::N_80136E14 => g.mars_special_n_loop_air_to_ground(gobj),
        addr::N_80136E74 => g.mars_special_n_ground_loop(gobj),
        addr::N_80136EAC => g.mars_special_n_air_loop(gobj),
        addr::SPECIAL_N_END_ANIM => g.mars_special_n_end_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.mars_special_air_n_end_anim(gobj),
        addr::SPECIAL_N_END_COLL => g.mars_special_n_end_coll(gobj),
        addr::SPECIAL_AIR_N_END_COLL => g.mars_special_air_n_end_coll(gobj),
        addr::N_801371FC => g.mars_special_n_end_ground_to_air(gobj),
        addr::N_801372A8 => g.mars_special_n_end_air_to_ground(gobj),
        addr::N_80137354 => g.mars_special_n_swing(gobj),
        addr::N_801373B8 => g.mars_special_air_n_swing(gobj),
        _ => return false,
    }
    true
}
