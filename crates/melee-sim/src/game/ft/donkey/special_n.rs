//! The Giant Punch (`ftdonkeyspecialn.c`): the wind-up, the arm's swings that charge it (up to
//! `MAX_ARM_SWINGS`, kept between uses in `u`), a cancel with the shield that stores them, and
//! the punch, whose hitboxes gain `DAMAGE_PER_SWING` per swing and whose lunge grows with them.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::{PAD_B, PAD_LR};
use super::{U_SWINGS, addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::lb::hitcapsule;

/// The flags the wind-up and the swings keep between the ground and the air
/// (`ftDk_MF_SpecialN_CollCancel`), and the punch's (`ftDk_MF_SpecialN_Coll`).
const COLL_CANCEL_FLAGS: u32 = 0x0C4C_5080;
const COLL_FLAGS: u32 = 0x0C4D_508E;

/// The punch's effects, on the ground and in the air.
const EFFECT_GROUND: i32 = 1224;
const EFFECT_AIR: i32 = 1225;

impl Running<'_> {
    /// The punch's hooks (`setCallbacks`): when hit or KO'd, the stored swings lost and the
    /// effects gone; on any damage, the effects gone; the effects pausing through hitlag.
    fn dk_special_n_set_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::ON_DAMAGE));
        fp.death2_cb = Some(CodeRef(addr::ON_DAMAGE));
        fp.take_dmg_2_cb = Some(CodeRef(addr::SPECIAL_N_DESTROY_EFFECTS));
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `clearCallbacks`.
    fn dk_special_n_clear_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_2_cb = None;
        fp.pre_hitlag_cb = None;
        fp.post_hitlag_cb = None;
    }

    /// The punch's start, on the ground (`land`) or in the air: straight to the full punch
    /// with the swings at their most, else the wind-up.
    fn dk_special_n_start(&mut self, gobj: GObjId, full: i32, start: i32, land: bool) {
        let f = self.game.fighter_id(gobj);
        let max = self.dk_attr_i32(f, attr::MAX_ARM_SWINGS);
        if self.game.fighters[f.0].u_i32(U_SWINGS) == max {
            self.change_motion_state(gobj, full, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::N_EFFECT, 1);
            let swings = fp.u_i32(U_SWINGS);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(U_SWINGS, 0);
        } else {
            self.change_motion_state(gobj, start, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::N_EFFECT, 0);
            fp.set_mv_i32(mv::N_SWINGS, 0);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::N_CANCEL, 0);
        fp.set_mv_i32(mv::N_LUNGE, 0);
        fp.set_mv_i32(mv::N_DAMAGE1, -1);
        fp.set_mv_i32(mv::N_DAMAGE0, -1);
        if land {
            self.land(gobj);
            self.game.fighters[f.0].self_vel.y = 0.0;
        }
        self.dk_special_n_set_cbs(gobj);
        self.anim_update(gobj);
    }

    /// `ftDk_SpecialN_Enter`.
    #[doc(alias = "ftDk_SpecialN_Enter")]
    pub fn dk_special_n_enter(&mut self, gobj: GObjId) {
        self.dk_special_n_start(gobj, ms::SPECIAL_N_FULL, ms::SPECIAL_N_START, true);
    }

    /// `ftDk_SpecialAirN_Enter`.
    #[doc(alias = "ftDk_SpecialAirN_Enter")]
    pub fn dk_special_air_n_enter(&mut self, gobj: GObjId) {
        self.dk_special_n_start(gobj, ms::SPECIAL_AIR_N_FULL, ms::SPECIAL_AIR_N_START, false);
    }

    /// The wind-up's end: the swings' loop.
    fn dk_special_n_start_anim(&mut self, gobj: GObjId, next: i32) {
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, next, 0, 0.0, 1.0, 0.0, None);
            self.dk_special_n_set_cbs(gobj);
        }
    }

    /// `ftDk_SpecialNStart_Anim`.
    #[doc(alias = "ftDk_SpecialNStart_Anim")]
    pub fn dk_special_n_start_anim_cb(&mut self, gobj: GObjId) {
        self.dk_special_n_start_anim(gobj, ms::SPECIAL_N_LOOP);
    }

    /// `ftDk_SpecialAirNStart_Anim`.
    #[doc(alias = "ftDk_SpecialAirNStart_Anim")]
    pub fn dk_special_air_n_start_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_start_anim(gobj, ms::SPECIAL_AIR_N_LOOP);
    }

    /// A swing at each loop's start: at the most, the glow, the swings kept and the state
    /// left (waiting on the ground, falling in the air).
    fn dk_special_n_loop_anim(&mut self, gobj: GObjId, air: bool) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cur_anim_frame != 0.0 {
            return;
        }
        let max = self.dk_attr_i32(f, attr::MAX_ARM_SWINGS);
        let fp = &mut self.game.fighters[f.0];
        let swings = fp.u_i32(U_SWINGS).wrapping_add(1);
        fp.set_u_i32(U_SWINGS, swings);
        if swings >= max {
            self.start_color_anim(gobj, 57, 0);
            self.game.fighters[f.0].set_u_i32(U_SWINGS, max);
            self.dk_special_n_clear_cbs(gobj);
            if air {
                self.enter_fall(gobj);
            } else {
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftDk_SpecialNLoop_Anim`.
    #[doc(alias = "ftDk_SpecialNLoop_Anim")]
    pub fn dk_special_n_loop_anim_cb(&mut self, gobj: GObjId) {
        self.dk_special_n_loop_anim(gobj, false);
    }

    /// `ftDk_SpecialAirNLoop_Anim`.
    #[doc(alias = "ftDk_SpecialAirNLoop_Anim")]
    pub fn dk_special_air_n_loop_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_loop_anim(gobj, true);
    }

    /// `ftDk_SpecialNCancel_Anim`: waiting when the cancel ends.
    #[doc(alias = "ftDk_SpecialNCancel_Anim")]
    pub fn dk_special_n_cancel_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.dk_special_n_clear_cbs(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftDk_SpecialAirNCancel_Anim`: falling when the cancel ends.
    #[doc(alias = "ftDk_SpecialAirNCancel_Anim")]
    pub fn dk_special_air_n_cancel_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.dk_special_n_clear_cbs(gobj);
            self.enter_fall(gobj);
        }
    }

    /// The punch's effect, once the script says (`cmd_vars[0]`) or at once for the full
    /// punch: the ground's or the air's, on the top joint, facing his way.
    fn dk_special_n_effect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 0 && fp.mv_i32(mv::N_EFFECT) == 0 {
            let v = fp.mv_i32(mv::N_EFFECT) + 1;
            fp.set_mv_i32(mv::N_EFFECT, v);
        }
        if fp.mv_i32(mv::N_EFFECT) == 1 {
            let v = fp.mv_i32(mv::N_EFFECT) + 1;
            fp.set_mv_i32(mv::N_EFFECT, v);
            let id = if self.is_airborne(gobj) {
                EFFECT_AIR
            } else {
                EFFECT_GROUND
            };
            let top = self.bone_joint(gobj, part::TOP_N);
            let facing = self.game.fighters[f.0].facing_dir;
            spawn(self, id, Some(gobj), &[Arg::JObj(top), Arg::Float(facing)]);
        }
    }

    /// The punch's two hitboxes each at their base damage (read once) plus the swings'.
    fn dk_special_n_damage(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let per = self.dk_attr_i32(f, attr::DAMAGE_PER_SWING);
        for (i, off) in [(0, mv::N_DAMAGE0), (1, mv::N_DAMAGE1)] {
            let fp = &mut self.game.fighters[f.0];
            if fp.mv_i32(off) == -1 {
                let d = fp.x914[i].damage as i32;
                fp.set_mv_i32(off, d);
            }
            let fp = &self.game.fighters[f.0];
            let dmg = fp
                .mv_i32(off)
                .wrapping_add(fp.mv_i32(mv::N_SWINGS).wrapping_mul(per));
            self.hitbox_damage(f, i, false, dmg as u32);
        }
    }

    /// The lunge, once (`updateVelocity`): ahead at `PUNCH_HORIZONTAL_VEL` times the swings.
    fn dk_special_n_lunge(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        if fp.mv_i32(mv::N_LUNGE) != 1 {
            return;
        }
        let vel = reg(self.dk_attr_f32(f, attr::PUNCH_HORIZONTAL_VEL));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::N_LUNGE, 2);
        let swings = fp::frsp(f64::from(fp.mv_i32(mv::N_SWINGS)));
        fp.gr_vel = mem(fp::fmuls(reg(fp.facing_dir), fp::fmuls(vel, swings)));
    }

    /// The punch's end: no swings stored, and `end`.
    fn dk_special_n_end(&mut self, gobj: GObjId, end: fn(&mut Self, GObjId)) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).set_u_i32(U_SWINGS, 0);
            self.dk_special_n_clear_cbs(gobj);
            end(self, gobj);
        }
    }

    /// The aerial punch's end: a fall, or a helpless fall with `SPECIAL_N_LANDING_LAG`.
    fn dk_special_air_n_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.dk_attr_f32(f, attr::SPECIAL_N_LANDING_LAG);
        if lag == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// `ftDk_SpecialN_Anim`: the effect, the hitboxes' damage while they're out, the lunge
    /// when they first come out, and waiting at the end.
    #[doc(alias = "ftDk_SpecialN_Anim")]
    pub fn dk_special_n_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_effect(gobj);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            let fp = self.game.fighter_mut(gobj);
            if fp.mv_i32(mv::N_LUNGE) == 0 {
                fp.set_mv_i32(mv::N_LUNGE, 1);
            }
            self.dk_special_n_damage(gobj);
        }
        self.dk_special_n_lunge(gobj);
        self.dk_special_n_end(gobj, Self::enter_wait);
    }

    /// `ftDk_SpecialNFull_Anim`: the effect, the lunge when the hitboxes first come out, and
    /// waiting at the end.
    #[doc(alias = "ftDk_SpecialNFull_Anim")]
    pub fn dk_special_n_full_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_effect(gobj);
        let fp = self.game.fighter_mut(gobj);
        if fp.x914[0].state != hitcapsule::DISABLED && fp.mv_i32(mv::N_LUNGE) == 0 {
            fp.set_mv_i32(mv::N_LUNGE, 1);
        }
        self.dk_special_n_lunge(gobj);
        self.dk_special_n_end(gobj, Self::enter_wait);
    }

    /// `ftDk_SpecialAirN_Anim`: the effect, the hitboxes' damage while they're out, and the
    /// fall at the end.
    #[doc(alias = "ftDk_SpecialAirN_Anim")]
    pub fn dk_special_air_n_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_effect(gobj);
        if self.game.fighter(gobj).x914[0].state != hitcapsule::DISABLED {
            self.dk_special_n_damage(gobj);
        }
        self.dk_special_n_end(gobj, Self::dk_special_air_n_fall);
    }

    /// `ftDk_SpecialAirNFull_Anim`: the effect, and the fall at the end.
    #[doc(alias = "ftDk_SpecialAirNFull_Anim")]
    pub fn dk_special_air_n_full_anim(&mut self, gobj: GObjId) {
        self.dk_special_n_effect(gobj);
        self.dk_special_n_end(gobj, Self::dk_special_air_n_fall);
    }

    /// The states' empty interrupts.
    #[doc(alias = "ftDk_SpecialNStart_IASA")]
    #[doc(alias = "ftDk_SpecialNCancel_IASA")]
    #[doc(alias = "ftDk_SpecialN_IASA")]
    #[doc(alias = "ftDk_SpecialNFull_IASA")]
    #[doc(alias = "ftDk_SpecialAirNStart_IASA")]
    #[doc(alias = "ftDk_SpecialAirNCancel_IASA")]
    #[doc(alias = "ftDk_SpecialAirN_IASA")]
    #[doc(alias = "ftDk_SpecialAirNFull_IASA")]
    pub fn dk_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// The swings' interrupt: B lets the punch go with the swings so far; the shield asks for
    /// a cancel, taken at the next loop's start.
    fn dk_special_n_loop_iasa(&mut self, gobj: GObjId, punch: i32, cancel: i32) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].input.pressed_buttons & PAD_B != 0 {
            self.change_motion_state(gobj, punch, 0, 0.0, 1.0, 0.0, None);
            let fp = &mut self.game.fighters[f.0];
            let swings = fp.u_i32(U_SWINGS);
            fp.set_mv_i32(mv::N_SWINGS, swings);
            fp.set_u_i32(U_SWINGS, 0);
            self.dk_special_n_set_cbs(gobj);
            self.anim_update(gobj);
        }
        let fp = &mut self.game.fighters[f.0];
        if fp.input.pressed_buttons & PAD_LR != 0 {
            fp.set_mv_i32(mv::N_CANCEL, 1);
        }
        let fp = &self.game.fighters[f.0];
        if fp.cur_anim_frame == 0.0 && fp.mv_i32(mv::N_CANCEL) != 0 {
            self.change_motion_state(gobj, cancel, 0, 0.0, 1.0, 0.0, None);
            self.dk_special_n_set_cbs(gobj);
        }
    }

    /// `ftDk_SpecialNLoop_IASA`: a roll or a spot dodge first, else the swings' interrupt.
    #[doc(alias = "ftDk_SpecialNLoop_IASA")]
    pub fn dk_special_n_loop_iasa_cb(&mut self, gobj: GObjId) {
        if !self.escape_check(gobj) {
            self.dk_special_n_loop_iasa(gobj, ms::SPECIAL_N, ms::SPECIAL_N_CANCEL);
        }
    }

    /// `ftDk_SpecialAirNLoop_IASA`.
    #[doc(alias = "ftDk_SpecialAirNLoop_IASA")]
    pub fn dk_special_air_n_loop_iasa(&mut self, gobj: GObjId) {
        self.dk_special_n_loop_iasa(gobj, ms::SPECIAL_AIR_N, ms::SPECIAL_AIR_N_CANCEL);
    }

    /// The grounded states' physics: friction or the animation's motion.
    #[doc(alias = "ftDk_SpecialNStart_Phys")]
    #[doc(alias = "ftDk_SpecialNLoop_Phys")]
    #[doc(alias = "ftDk_SpecialNCancel_Phys")]
    #[doc(alias = "ftDk_SpecialN_Phys")]
    #[doc(alias = "ftDk_SpecialNFull_Phys")]
    pub fn dk_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// The aerial states' physics, without drift.
    #[doc(alias = "ftDk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftDk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftDk_SpecialAirNCancel_Phys")]
    #[doc(alias = "ftDk_SpecialAirN_Phys")]
    #[doc(alias = "ftDk_SpecialAirNFull_Phys")]
    pub fn dk_special_air_n_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Off a ledge, the matching aerial state from the same frame. The wind-up, the swings
    /// and the cancel stop at the ledge (`ft_80082708`); the punches slide off
    /// (`ft_800827A0`).
    fn dk_special_n_coll(&mut self, gobj: GObjId, air: i32, slide: bool, flags: u32) {
        let grounded = if slide {
            self.ground_coll_slide(gobj)
        } else {
            self.ground_coll_off_ledges(gobj)
        };
        if !grounded {
            self.fox_ground_to_air(gobj, air, flags);
            self.dk_special_n_set_cbs(gobj);
        }
    }

    /// Landing, the matching grounded state from the same frame.
    fn dk_special_air_n_coll(&mut self, gobj: GObjId, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, flags);
            self.dk_special_n_set_cbs(gobj);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        SPECIAL_N_ENTER => g.dk_special_n_enter(gobj),
        SPECIAL_AIR_N_ENTER => g.dk_special_air_n_enter(gobj),
        SPECIAL_N_START_ANIM => g.dk_special_n_start_anim_cb(gobj),
        SPECIAL_N_LOOP_ANIM => g.dk_special_n_loop_anim_cb(gobj),
        SPECIAL_N_CANCEL_ANIM => g.dk_special_n_cancel_anim(gobj),
        SPECIAL_N_ANIM => g.dk_special_n_anim(gobj),
        SPECIAL_N_FULL_ANIM => g.dk_special_n_full_anim(gobj),
        SPECIAL_AIR_N_START_ANIM => g.dk_special_air_n_start_anim(gobj),
        SPECIAL_AIR_N_LOOP_ANIM => g.dk_special_air_n_loop_anim(gobj),
        SPECIAL_AIR_N_CANCEL_ANIM => g.dk_special_air_n_cancel_anim(gobj),
        SPECIAL_AIR_N_ANIM => g.dk_special_air_n_anim(gobj),
        SPECIAL_AIR_N_FULL_ANIM => g.dk_special_air_n_full_anim(gobj),
        SPECIAL_N_START_IASA
        | SPECIAL_N_CANCEL_IASA
        | SPECIAL_N_IASA
        | SPECIAL_N_FULL_IASA
        | SPECIAL_AIR_N_START_IASA
        | SPECIAL_AIR_N_CANCEL_IASA
        | SPECIAL_AIR_N_IASA
        | SPECIAL_AIR_N_FULL_IASA => g.dk_special_n_nothing(gobj),
        SPECIAL_N_LOOP_IASA => g.dk_special_n_loop_iasa_cb(gobj),
        SPECIAL_AIR_N_LOOP_IASA => g.dk_special_air_n_loop_iasa(gobj),
        SPECIAL_N_START_PHYS
        | SPECIAL_N_LOOP_PHYS
        | SPECIAL_N_CANCEL_PHYS
        | SPECIAL_N_PHYS
        | SPECIAL_N_FULL_PHYS => g.dk_special_n_phys(gobj),
        SPECIAL_AIR_N_START_PHYS
        | SPECIAL_AIR_N_LOOP_PHYS
        | SPECIAL_AIR_N_CANCEL_PHYS
        | SPECIAL_AIR_N_PHYS
        | SPECIAL_AIR_N_FULL_PHYS => g.dk_special_air_n_phys(gobj),
        SPECIAL_N_START_COLL => g.dk_special_n_start_coll(gobj),
        SPECIAL_N_LOOP_COLL => g.dk_special_n_loop_coll(gobj),
        SPECIAL_N_CANCEL_COLL => g.dk_special_n_cancel_coll(gobj),
        SPECIAL_N_COLL => g.dk_special_n_punch_coll(gobj),
        SPECIAL_N_FULL_COLL => g.dk_special_n_full_coll(gobj),
        SPECIAL_AIR_N_START_COLL => g.dk_special_air_n_start_coll(gobj),
        SPECIAL_AIR_N_LOOP_COLL => g.dk_special_air_n_loop_coll(gobj),
        SPECIAL_AIR_N_CANCEL_COLL => g.dk_special_air_n_cancel_coll(gobj),
        SPECIAL_AIR_N_COLL => g.dk_special_air_n_punch_coll(gobj),
        SPECIAL_AIR_N_FULL_COLL => g.dk_special_air_n_full_coll(gobj),
        SPECIAL_N_DESTROY_EFFECTS => g.dk_special_n_destroy_effects(gobj),
        SPECIAL_N_DESTROY_EFFECTS_PLUS => g.dk_special_n_destroy_effects_plus(gobj),
        _ => return false,
    }
    true
}

impl Running<'_> {
    /// `ftDk_SpecialNStart_Coll`.
    #[doc(alias = "ftDk_SpecialNStart_Coll")]
    pub fn dk_special_n_start_coll(&mut self, gobj: GObjId) {
        self.dk_special_n_coll(gobj, ms::SPECIAL_AIR_N_START, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialNLoop_Coll`.
    #[doc(alias = "ftDk_SpecialNLoop_Coll")]
    pub fn dk_special_n_loop_coll(&mut self, gobj: GObjId) {
        self.dk_special_n_coll(gobj, ms::SPECIAL_AIR_N_LOOP, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialNCancel_Coll`.
    #[doc(alias = "ftDk_SpecialNCancel_Coll")]
    pub fn dk_special_n_cancel_coll(&mut self, gobj: GObjId) {
        self.dk_special_n_coll(gobj, ms::SPECIAL_AIR_N_CANCEL, false, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialN_Coll`.
    #[doc(alias = "ftDk_SpecialN_Coll")]
    pub fn dk_special_n_punch_coll(&mut self, gobj: GObjId) {
        self.dk_special_n_coll(gobj, ms::SPECIAL_AIR_N, true, COLL_FLAGS);
    }

    /// `ftDk_SpecialNFull_Coll`.
    #[doc(alias = "ftDk_SpecialNFull_Coll")]
    pub fn dk_special_n_full_coll(&mut self, gobj: GObjId) {
        self.dk_special_n_coll(gobj, ms::SPECIAL_AIR_N_FULL, true, COLL_FLAGS);
    }

    /// `ftDk_SpecialAirNStart_Coll`.
    #[doc(alias = "ftDk_SpecialAirNStart_Coll")]
    pub fn dk_special_air_n_start_coll(&mut self, gobj: GObjId) {
        self.dk_special_air_n_coll(gobj, ms::SPECIAL_N_START, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirNLoop_Coll`.
    #[doc(alias = "ftDk_SpecialAirNLoop_Coll")]
    pub fn dk_special_air_n_loop_coll(&mut self, gobj: GObjId) {
        self.dk_special_air_n_coll(gobj, ms::SPECIAL_N_LOOP, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirNCancel_Coll`.
    #[doc(alias = "ftDk_SpecialAirNCancel_Coll")]
    pub fn dk_special_air_n_cancel_coll(&mut self, gobj: GObjId) {
        self.dk_special_air_n_coll(gobj, ms::SPECIAL_N_CANCEL, COLL_CANCEL_FLAGS);
    }

    /// `ftDk_SpecialAirN_Coll`.
    #[doc(alias = "ftDk_SpecialAirN_Coll")]
    pub fn dk_special_air_n_punch_coll(&mut self, gobj: GObjId) {
        self.dk_special_air_n_coll(gobj, ms::SPECIAL_N, COLL_FLAGS);
    }

    /// `ftDk_SpecialAirNFull_Coll`.
    #[doc(alias = "ftDk_SpecialAirNFull_Coll")]
    pub fn dk_special_air_n_full_coll(&mut self, gobj: GObjId) {
        self.dk_special_air_n_coll(gobj, ms::SPECIAL_N_FULL, COLL_FLAGS);
    }

    /// The punch's effects gone (`ftDk_SpecialN_DestroyAllEffects`, its damage hook).
    #[doc(alias = "ftDk_SpecialN_DestroyAllEffects")]
    pub fn dk_special_n_destroy_effects(&mut self, gobj: GObjId) {
        ef::destroy_all(self, gobj);
    }

    /// The stored swings lost and the effects gone (`ftDk_SpecialN_DestroyAllEffectsPlus`).
    #[doc(alias = "ftDk_SpecialN_DestroyAllEffectsPlus")]
    pub fn dk_special_n_destroy_effects_plus(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_u_i32(U_SWINGS, 0);
        ef::destroy_all(self, gobj);
    }
}
