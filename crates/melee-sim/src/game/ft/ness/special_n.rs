//! PK Flash (`ftnessspecialn.c`): Ness charges a flash (`it::kinds::nesspkflash`) that floats
//! up while B is held and bursts when he lets go or it reaches its peak.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{self, nesspkflash};
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::{PART_L2ND_NA, addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What PK Flash's states keep between the ground and the air (`FTNESS_SPECIALN_COLL_FLAG`).
const COLL_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

impl Running<'_> {
    /// Whether Ness charges PK Flash, `ftNs_SpecialN_CheckSpecialNHold`: a flash out and the
    /// hold state.
    #[doc(alias = "ftNs_SpecialN_CheckSpecialNHold")]
    pub fn ness_special_n_holding(&self, gobj: GObjId) -> bool {
        let fp = self.game.fighter(gobj);
        fp.u.gobj_at(u::PKFLASH).is_some()
            && (fp.motion_id == ms::SPECIAL_N_HOLD || fp.motion_id == ms::SPECIAL_AIR_N_HOLD)
    }

    /// The flash let go of, `ftNs_SpecialN_SetNULL`: no flash and no damage hooks.
    #[doc(alias = "ftNs_SpecialN_SetNULL")]
    pub fn ness_special_n_set_null(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.u.gobj_at(u::PKFLASH).is_some() {
            fp.u.set_gobj_at(u::PKFLASH, None);
        }
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Ness hit while charging, `ftNs_SpecialN_ItemPKFlushSetNULL`: the flash loses him.
    #[doc(alias = "ftNs_SpecialN_ItemPKFlushSetNULL")]
    pub fn ness_pkflash_set_null(&mut self, gobj: GObjId) {
        if let Some(flash) = self.game.fighter(gobj).u.gobj_at(u::PKFLASH) {
            nesspkflash::let_go(self, flash);
            self.game.fighter_mut(gobj).u.set_gobj_at(u::PKFLASH, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// The flash from the left hand, 3 units up (scaled), unless one is out
    /// (`ftNs_PKFlash_Init`).
    fn ness_pkflash_init(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).u.gobj_at(u::PKFLASH).is_some() {
            return;
        }
        let j = self.bone_joint(gobj, PART_L2ND_NA);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        pos.z = 0.0;
        let fp = self.game.fighter(gobj);
        pos.y = mem(fp::fmadds(3.0, reg(fp.x34_scale.y), reg(pos.y)));
        let facing = fp.facing_dir;
        let flash = nesspkflash::spawn(self, gobj, pos, kinds::NESS_PK_FLUSH, facing);
        let fp = self.game.fighter_mut(gobj);
        fp.u.set_gobj_at(u::PKFLASH, flash);
        if flash.is_some() {
            fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
            fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
        }
    }

    /// The timers from the attributes, no flash and no hooks (`SetPKFlashAttr`).
    fn ness_special_n_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let t1 = self.ness_attr_i32(f, attr::PKFLASH_TIMER1_LOOPFRAMES);
        let t2 = self.ness_attr_i32(f, attr::PKFLASH_TIMER2_LOOPFRAMES);
        let delay = self.ness_attr_i32(f, attr::PKFLASH_GRAVITY_DELAY);
        let min = self.ness_attr_i32(f, attr::PKFLASH_MINCHARGEFRAMES);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t1);
        fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t2);
        fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, delay);
        fp.u.set_gobj_at(u::PKFLASH, None);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, min);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// `ftNs_SpecialNStart_Enter`.
    #[doc(alias = "ftNs_SpecialNStart_Enter")]
    pub fn ness_special_n_start_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_N_START, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.ness_special_n_reset(gobj);
        self.anim_update(gobj);
    }

    /// `ftNs_SpecialAirNStart_Enter`: no rise kept.
    #[doc(alias = "ftNs_SpecialAirNStart_Enter")]
    pub fn ness_special_air_n_start_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N_START, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.self_vel.y = 0.0;
        self.ness_special_n_reset(gobj);
        self.anim_update(gobj);
    }

    /// The start's end (`ftNs_SpecialNStart_Anim`, `ftNs_SpecialAirNStart_Anim`): holding, the
    /// flash out, the jumps used.
    fn ness_special_n_start_anim(&mut self, gobj: GObjId, hold: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.change_motion_state(gobj, hold, 0, 0.0, 1.0, 0.0, None);
        self.ness_pkflash_init(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// The charge (`ftNs_SpecialNRelease_Anim`, `ftNs_SpecialAirNRelease_Anim`, also the hold
    /// states'): the timers run; with the flash gone, the end once both are out, else the
    /// release state; with the flash out but no longer his, he forgets it; with it bursting
    /// (`it_802AA7F0`), the release state.
    fn ness_special_n_charge(&mut self, gobj: GObjId, release: i32, end: i32) {
        let fp = self.game.fighter_mut(gobj);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND);
        if t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND, t.wrapping_sub(1));
        }
        let flash = fp.u.gobj_at(u::PKFLASH);
        let t = fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR);
        if flash.is_none() && t != 0 {
            fp.set_mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR, t.wrapping_sub(1));
        }
        let Some(flash) = flash else {
            let fp = self.game.fighter(gobj);
            if fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_GROUND) <= 0
                && fp.mv_i32(mv::N_FRAMES_TO_LOOP_CHARGE_AIR) <= 0
            {
                self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                return;
            }
            if fp.motion_id != release {
                let frame = fp.cur_anim_frame;
                self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
            }
            return;
        };
        if nesspkflash::owner(self, flash) != Some(gobj) {
            self.game.fighter_mut(gobj).u.set_gobj_at(u::PKFLASH, None);
            return;
        }
        if nesspkflash::released(self, flash) && self.game.fighter(gobj).motion_id != release {
            let frame = self.game.fighter(gobj).cur_anim_frame;
            self.change_motion_state(gobj, release, 0, frame, 1.0, 0.0, None);
        }
    }

    /// `ftNs_SpecialNEnd_Anim`: the timers reset each frame, no rotation; at the end, waiting.
    #[doc(alias = "ftNs_SpecialNEnd_Anim")]
    pub fn ness_special_n_end_anim(&mut self, gobj: GObjId) {
        self.ness_special_n_reset(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftNs_SpecialAirNEnd_Anim`: at the end, falling helpless, or just falling when the
    /// landing lag is 0.
    #[doc(alias = "ftNs_SpecialAirNEnd_Anim")]
    pub fn ness_special_air_n_end_anim(&mut self, gobj: GObjId) {
        self.ness_special_n_reset(gobj);
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let lag = self.ness_attr_f32(f, attr::PKFLASH_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        self.enter_fall_special(gobj, 1, 0, 1, 1.0, lag, 0.0, true);
    }

    /// The charge's interrupt (`ftNs_SpecialNRelease_IASA`, `ftNs_SpecialAirNRelease_IASA`):
    /// after the least charge, letting go of B lets go of the flash.
    fn ness_special_n_charge_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(mv::N_CHARGE_RELEASE_DELAY).wrapping_sub(1);
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, d);
        if fp.mv_i32(mv::N_CHARGE_RELEASE_DELAY) > 0 {
            return;
        }
        fp.set_mv_i32(mv::N_CHARGE_RELEASE_DELAY, 0);
        if fp.input.held_buttons[0] & PAD_B != 0 {
            return;
        }
        self.ness_special_n_set_null(gobj);
    }

    /// The air's physics (start, charge and end): after the delay, PK Flash's slow fall, and
    /// air friction.
    fn ness_special_air_n_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ness_attr_f32(f, attr::PKFLASH_FALL_ACCEL));
        let fp = &mut self.game.fighters[f.0];
        let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
        if d != 0 {
            fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
        } else {
            let tv = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, tv);
        }
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The ground's states off a ledge.
    fn ness_special_n_ground_coll(&mut self, gobj: GObjId, air: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air, COLL_FLAGS);
        }
    }

    /// The air's states landing.
    fn ness_special_air_n_land(&mut self, gobj: GObjId, ground: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, COLL_FLAGS);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_SET_NULL => g.ness_special_n_set_null(gobj),
        addr::SPECIAL_N_ITEM_PKFLUSH_SET_NULL => g.ness_pkflash_set_null(gobj),
        addr::SPECIAL_N_START_ENTER => g.ness_special_n_start_enter(gobj),
        addr::SPECIAL_AIR_N_START_ENTER => g.ness_special_air_n_start_enter(gobj),
        addr::SPECIAL_N_START_ANIM => g.ness_special_n_start_anim(gobj, ms::SPECIAL_N_HOLD),
        addr::SPECIAL_AIR_N_START_ANIM => g.ness_special_n_start_anim(gobj, ms::SPECIAL_AIR_N_HOLD),
        addr::SPECIAL_N_RELEASE_ANIM => {
            g.ness_special_n_charge(gobj, ms::SPECIAL_N_RELEASE, ms::SPECIAL_N_END)
        }
        addr::SPECIAL_AIR_N_RELEASE_ANIM => {
            g.ness_special_n_charge(gobj, ms::SPECIAL_AIR_N_RELEASE, ms::SPECIAL_AIR_N_END)
        }
        addr::SPECIAL_N_END_ANIM => g.ness_special_n_end_anim(gobj),
        addr::SPECIAL_AIR_N_END_ANIM => g.ness_special_air_n_end_anim(gobj),
        addr::SPECIAL_N_START_IASA
        | addr::SPECIAL_N_END_IASA
        | addr::SPECIAL_AIR_N_START_IASA
        | addr::SPECIAL_AIR_N_END_IASA => {}
        addr::SPECIAL_N_RELEASE_IASA | addr::SPECIAL_AIR_N_RELEASE_IASA => {
            g.ness_special_n_charge_iasa(gobj)
        }
        addr::SPECIAL_N_START_PHYS => {
            let fp = g.game.fighter_mut(gobj);
            let d = fp.mv_i32(mv::N_FALLING_ACCELERATION_DELAY);
            if d != 0 {
                fp.set_mv_i32(mv::N_FALLING_ACCELERATION_DELAY, d.wrapping_sub(1));
            }
            g.ground_friction(gobj);
        }
        addr::SPECIAL_N_RELEASE_PHYS | addr::SPECIAL_N_END_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N_START_PHYS
        | addr::SPECIAL_AIR_N_RELEASE_PHYS
        | addr::SPECIAL_AIR_N_END_PHYS => g.ness_special_air_n_phys(gobj),
        addr::SPECIAL_N_START_COLL => g.ness_special_n_ground_coll(gobj, ms::SPECIAL_AIR_N_START),
        addr::SPECIAL_N_RELEASE_COLL => g.ness_special_n_ground_coll(gobj, ms::SPECIAL_AIR_N_HOLD),
        addr::SPECIAL_N_END_COLL => g.ness_special_n_ground_coll(gobj, ms::SPECIAL_AIR_N_END),
        addr::SPECIAL_AIR_N_START_COLL => g.ness_special_air_n_land(gobj, ms::SPECIAL_N_START),
        addr::SPECIAL_AIR_N_RELEASE_COLL => g.ness_special_air_n_land(gobj, ms::SPECIAL_N_HOLD),
        addr::SPECIAL_AIR_N_END_COLL => g.ness_special_air_n_land(gobj, ms::SPECIAL_N_END),
        _ => return false,
    }
    true
}
