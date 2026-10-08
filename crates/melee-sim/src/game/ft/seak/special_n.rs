//! The needles (`ftseakspecialn.c`): drawing them, charging up to six while B is held, the
//! charge cancelled with a shield button, and throwing them one at a time when B is let go.
//! The needles' items are `it::kinds::seakneedleheld` and `seakneedlethrown`.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{seakneedleheld, seakneedlethrown};
use super::super::super::state::Running;
use super::{GA_GROUND, IT_NEEDLE_HELD, IT_NEEDLE_THROW, PAD_B, PAD_LR, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What the neutral special keeps between the ground and the air (`mf` in the original).
const COLL_FLAGS: u32 =
    1 << 7 | 1 << 12 | 1 << 14 | 1 << 18 | 1 << 19 | 1 << 22 | 1 << 26 | 1 << 27;

/// How high above Sheik's middle a needle leaves, by a random step (`needleYPosScale`).
const NEEDLE_Y: [f32; 9] = [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];

/// The part the held needles are attached to.
const NEEDLE_PART: i32 = 23;

/// The charge's sound and the throw's.
const SFX_CHARGE: i32 = 270134;
const SFX_THROW: i32 = 270140;

/// The throw's effect.
const GFX_THROW: i32 = 1283;

impl Running<'_> {
    /// The held needles' own question of their owner, `ftSk_SpecialS_80111F70`: whether Sheik
    /// has let go of them (holds no needle item).
    #[doc(alias = "ftSk_SpecialS_80111F70")]
    pub fn seak_lets_go(&self, gobj: Option<GObjId>) -> bool {
        match gobj {
            Some(g) => self.seak_needles(g).is_none(),
            None => true,
        }
    }

    /// How many needles Sheik holds, for her needle item, `ftSk_SpecialS_80111FA0`.
    #[doc(alias = "ftSk_SpecialS_80111FA0")]
    pub fn seak_held_count(&self, gobj: Option<GObjId>) -> i32 {
        match gobj {
            Some(g) => self.seak_needle_count(g),
            None => 0,
        }
    }

    /// `ftSk_SpecialN_80111FBC`: when hit with needles in hand, each one drops loose; the
    /// count goes to zero either way.
    #[doc(alias = "ftSk_SpecialN_80111FBC")]
    pub fn seak_drop_needles(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.seak_needles(gobj).is_some() {
            self.set_seak_needles(gobj, None);
            while self.seak_needle_count(gobj) != 0 {
                let fp = &self.game.fighters[f.0];
                let mut pos = fp.cur_pos;
                let y = if fp.ground_or_air == GA_GROUND {
                    self.seak_attr_f32(f, attr::X4)
                } else {
                    self.seak_attr_f32(f, attr::XC)
                };
                let fp = &self.game.fighters[f.0];
                pos.y = mem(fp::fmadds(reg(fp.x34_scale.y), reg(y), reg(pos.y)));
                pos.z = 0.0;
                let facing = fp.facing_dir;
                let item = seakneedlethrown::spawn(self, gobj, pos, IT_NEEDLE_THROW, facing);
                if item.is_some() {
                    seakneedlethrown::launch(self, item, Some(gobj), 1);
                }
                let n = self.seak_needle_count(gobj).wrapping_sub(1);
                self.set_seak_needle_count(gobj, n);
            }
            let fp = &self.game.fighters[f.0];
            let cb = Some(CodeRef(addr::DAMAGE_CB));
            if fp.death2_cb == cb || fp.take_dmg_cb == cb {
                self.seak_clear_damage_cb(gobj);
            }
        }
        self.set_seak_needle_count(gobj, 0);
    }

    /// `ftSk_SpecialN_801120D4`: no damage hook.
    #[doc(alias = "ftSk_SpecialN_801120D4")]
    pub fn seak_clear_damage_cb(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = None;
        fp.death2_cb = None;
    }

    /// `Fighter_SetDamageCallback(gobj, ftSk_Init_80110198)`.
    fn seak_set_damage_cb(&mut self, gobj: GObjId) {
        self.set_damage_cb(gobj, Some(addr::DAMAGE_CB));
    }

    /// `doEnter`: the draw, at least one needle held.
    fn seak_special_n_enter_as(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.set_throw_flag(0, false);
        fp.cmd_vars = [0; 4];
        fp.set_mv_i32(mv::N_X0, 0);
        if fp.u_i32(super::u::NEEDLE_COUNT) == 0 {
            fp.set_u_i32(super::u::NEEDLE_COUNT, 1);
        }
        fp.set_mv_i32(mv::N_X4, 0);
        fp.set_mv_i32(mv::N_X8, 0);
        self.seak_set_damage_cb(gobj);
        self.anim_update(gobj);
    }

    /// The grounded neutral special, `ftSk_SpecialN_Enter`.
    #[doc(alias = "ftSk_SpecialN_Enter")]
    pub fn seak_special_n_enter(&mut self, gobj: GObjId) {
        self.seak_special_n_enter_as(gobj, ms::SPECIAL_N_START);
    }

    /// The aerial neutral special, `ftSk_SpecialAirN_Enter`.
    #[doc(alias = "ftSk_SpecialAirN_Enter")]
    pub fn seak_special_air_n_enter(&mut self, gobj: GObjId) {
        self.seak_special_n_enter_as(gobj, ms::SPECIAL_AIR_N_START);
    }

    /// The draws' end, `ftSk_SpecialNStart_Anim` and `ftSk_SpecialAirNStart_Anim`: the
    /// needles in hand, into the charge.
    fn seak_special_n_start_anim(&mut self, gobj: GObjId, loop_msid: i32) {
        if !self.anim_frames_remaining(gobj) {
            let fp = self.game.fighter(gobj);
            let (pos, facing) = (fp.cur_pos, fp.facing_dir);
            let n = seakneedleheld::spawn(self, gobj, pos, NEEDLE_PART, IT_NEEDLE_HELD, facing);
            self.set_seak_needles(gobj, n);
            self.change_motion_state(gobj, loop_msid, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj);
        }
    }

    #[doc(alias = "ftSk_SpecialNStart_Anim")]
    pub fn seak_special_n_start_anim_cb(&mut self, gobj: GObjId) {
        self.seak_special_n_start_anim(gobj, ms::SPECIAL_N_LOOP);
    }

    #[doc(alias = "ftSk_SpecialAirNStart_Anim")]
    pub fn seak_special_air_n_start_anim(&mut self, gobj: GObjId) {
        self.seak_special_n_start_anim(gobj, ms::SPECIAL_AIR_N_LOOP);
    }

    /// The charge, `ftSk_SpecialNLoop_Anim` and `ftSk_SpecialAirNLoop_Anim`: a needle more
    /// each time the animation loops, up to six, which flashes.
    #[doc(alias = "ftSk_SpecialNLoop_Anim")]
    #[doc(alias = "ftSk_SpecialAirNLoop_Anim")]
    pub fn seak_special_n_loop_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X8) == 0 {
            self.fighter_play_sfx(f, SFX_CHARGE, 127, 64);
        }
        let fp = &mut self.game.fighters[f.0];
        let n = fp.mv_i32(mv::N_X8).wrapping_add(1);
        fp.set_mv_i32(mv::N_X8, n);
        if reg(fp.cur_anim_frame) == fp::frsp(0.0) {
            let c = fp.u_i32(super::u::NEEDLE_COUNT).wrapping_add(1);
            fp.set_u_i32(super::u::NEEDLE_COUNT, c);
            fp.set_mv_i32(mv::N_X8, 0);
            if c > 6 {
                fp.set_u_i32(super::u::NEEDLE_COUNT, 6);
                fp.set_mv_i32(mv::N_X8, 100);
                self.start_color_anim(gobj, 86, 0);
            }
        }
    }

    /// The grounded cancel, `ftSk_SpecialNCancel_Anim`: the needles put away.
    #[doc(alias = "ftSk_SpecialNCancel_Anim")]
    pub fn seak_special_n_cancel_anim(&mut self, gobj: GObjId) {
        self.set_seak_needles(gobj, None);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// Falling once an aerial needle state ends: helplessly if the attributes give it landing
    /// lag (`x10`).
    fn seak_special_n_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.seak_attr_f32(f, attr::X10);
        if reg(lag) == 0.0 {
            self.enter_fall(gobj);
        } else {
            self.fall_special(gobj, 1.0, lag);
        }
    }

    /// The throws' frames that let a needle go (every third from the third), the needles out
    /// of hand.
    fn seak_special_n_end_tick(&mut self, gobj: GObjId) {
        if matches!(
            self.game.fighter(gobj).mv_i32(mv::N_X0),
            2 | 5 | 8 | 11 | 14 | 17
        ) {
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 1);
            self.set_seak_needles(gobj, None);
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::N_X0).wrapping_add(1);
        fp.set_mv_i32(mv::N_X0, n);
    }

    /// The grounded throw, `ftSk_SpecialNEnd_Anim`.
    #[doc(alias = "ftSk_SpecialNEnd_Anim")]
    pub fn seak_special_n_end_anim(&mut self, gobj: GObjId) {
        self.seak_special_n_end_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The aerial cancel, `ftSk_SpecialAirNCancel_Anim`.
    #[doc(alias = "ftSk_SpecialAirNCancel_Anim")]
    pub fn seak_special_air_n_cancel_anim(&mut self, gobj: GObjId) {
        self.set_seak_needles(gobj, None);
        if !self.anim_frames_remaining(gobj) {
            self.seak_special_n_fall(gobj);
        }
    }

    /// The aerial throw, `ftSk_SpecialAirNEnd_Anim`.
    #[doc(alias = "ftSk_SpecialAirNEnd_Anim")]
    pub fn seak_special_air_n_end_anim(&mut self, gobj: GObjId) {
        self.seak_special_n_end_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.seak_special_n_fall(gobj);
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftSk_SpecialNStart_IASA")]
    #[doc(alias = "ftSk_SpecialNCancel_IASA")]
    #[doc(alias = "ftSk_SpecialNEnd_IASA")]
    #[doc(alias = "ftSk_SpecialAirNStart_IASA")]
    #[doc(alias = "ftSk_SpecialAirNCancel_IASA")]
    #[doc(alias = "ftSk_SpecialAirNEnd_IASA")]
    pub fn seak_no_iasa(&mut self, _gobj: GObjId) {}

    /// `doIasa`: letting go of B throws (the throws' accessory fires each needle); a shield
    /// button cancels the charge.
    fn seak_special_n_loop_iasa(&mut self, gobj: GObjId, end_msid: i32, cancel_msid: i32) {
        let fp = self.game.fighter(gobj);
        if fp.input.held_buttons[0] & PAD_B == 0 {
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X0, 0);
            self.change_motion_state(gobj, end_msid, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj);
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SHOOT_NEEDLES));
        } else if fp.input.pressed_buttons & PAD_LR != 0 {
            self.change_motion_state(gobj, cancel_msid, 0, 0.0, 1.0, 0.0, None);
            self.seak_set_damage_cb(gobj);
        }
    }

    #[doc(alias = "ftSk_SpecialNLoop_IASA")]
    pub fn seak_special_n_loop_iasa_cb(&mut self, gobj: GObjId) {
        self.seak_special_n_loop_iasa(gobj, ms::SPECIAL_N_END, ms::SPECIAL_N_CANCEL);
    }

    #[doc(alias = "ftSk_SpecialAirNLoop_IASA")]
    pub fn seak_special_air_n_loop_iasa(&mut self, gobj: GObjId) {
        self.seak_special_n_loop_iasa(gobj, ms::SPECIAL_AIR_N_END, ms::SPECIAL_AIR_N_CANCEL);
    }

    /// Grounded friction, `ft_80084F3C`.
    #[doc(alias = "ftSk_SpecialNStart_Phys")]
    #[doc(alias = "ftSk_SpecialNLoop_Phys")]
    #[doc(alias = "ftSk_SpecialNCancel_Phys")]
    #[doc(alias = "ftSk_SpecialNEnd_Phys")]
    #[doc(alias = "ftSk_SpecialS_Phys")]
    #[doc(alias = "ftSk_SpecialSEnd_Phys")]
    #[doc(alias = "ftSk_SpecialHiStart_0_Phys")]
    #[doc(alias = "ftSk_SpecialHi_Phys")]
    #[doc(alias = "ftSk_SpecialLw_Phys")]
    #[doc(alias = "ftSk_SpecialLw2_Phys")]
    pub fn seak_ground_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
    }

    /// Aerial physics without drift, `ft_80084EEC`.
    #[doc(alias = "ftSk_SpecialAirNStart_Phys")]
    #[doc(alias = "ftSk_SpecialAirNLoop_Phys")]
    #[doc(alias = "ftSk_SpecialAirNCancel_Phys")]
    #[doc(alias = "ftSk_SpecialAirNEnd_Phys")]
    #[doc(alias = "ftSk_SpecialAirS_Phys")]
    #[doc(alias = "ftSk_SpecialAirSEnd_Phys")]
    pub fn seak_air_phys(&mut self, gobj: GObjId) {
        self.air_physics_no_drift(gobj);
    }

    /// Off the ground in the draw or the charge, `ftSk_SpecialNStart_Coll` and
    /// `ftSk_SpecialNLoop_Coll`: the aerial state.
    fn seak_special_n_coll(&mut self, gobj: GObjId, air_msid: i32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air_msid, COLL_FLAGS);
            self.seak_set_damage_cb(gobj);
        }
    }

    #[doc(alias = "ftSk_SpecialNStart_Coll")]
    pub fn seak_special_n_start_coll(&mut self, gobj: GObjId) {
        self.seak_special_n_coll(gobj, ms::SPECIAL_AIR_N_START);
    }

    #[doc(alias = "ftSk_SpecialNLoop_Coll")]
    pub fn seak_special_n_loop_coll(&mut self, gobj: GObjId) {
        self.seak_special_n_coll(gobj, ms::SPECIAL_AIR_N_LOOP);
    }

    /// Off the ground in the cancel, `ftSk_SpecialNCancel_Coll`: a fall.
    #[doc(alias = "ftSk_SpecialNCancel_Coll")]
    pub fn seak_special_n_cancel_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.set_damage_cb(gobj, None);
            self.seak_special_n_fall(gobj);
        }
    }

    /// Off the ground in the throw, `ftSk_SpecialNEnd_Coll`: the needles gone, a fall.
    #[doc(alias = "ftSk_SpecialNEnd_Coll")]
    pub fn seak_special_n_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.set_damage_cb(gobj, None);
            self.set_seak_needle_count(gobj, 0);
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 0);
            self.seak_special_n_fall(gobj);
        }
    }

    /// Landing in the draw or the charge, `ftSk_SpecialAirNStart_Coll` and
    /// `ftSk_SpecialAirNLoop_Coll` (`doColl`): the grounded state.
    fn seak_special_air_n_coll(&mut self, gobj: GObjId, msid: i32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, msid, COLL_FLAGS);
            self.seak_set_damage_cb(gobj);
        }
    }

    #[doc(alias = "ftSk_SpecialAirNStart_Coll")]
    pub fn seak_special_air_n_start_coll(&mut self, gobj: GObjId) {
        self.seak_special_air_n_coll(gobj, ms::SPECIAL_N_START);
    }

    #[doc(alias = "ftSk_SpecialAirNLoop_Coll")]
    pub fn seak_special_air_n_loop_coll(&mut self, gobj: GObjId) {
        self.seak_special_air_n_coll(gobj, ms::SPECIAL_N_LOOP);
    }

    /// Landing in the cancel, `ftSk_SpecialAirNCancel_Coll`.
    #[doc(alias = "ftSk_SpecialAirNCancel_Coll")]
    pub fn seak_special_air_n_cancel_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.enter_landing_basic(gobj);
            self.set_damage_cb(gobj, None);
        }
    }

    /// Landing in the throw, `ftSk_SpecialAirNEnd_Coll`: the needles gone.
    #[doc(alias = "ftSk_SpecialAirNEnd_Coll")]
    pub fn seak_special_air_n_end_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.set_damage_cb(gobj, None);
            self.set_seak_needle_count(gobj, 0);
            self.game.fighter_mut(gobj).set_mv_i32(mv::N_X4, 0);
            self.enter_landing_basic(gobj);
        }
    }

    /// Throws a needle when the throw lets one go (`mv x4`), `shootNeedles` (the throws'
    /// accessory): from in front of Sheik, at a random height.
    #[doc(alias = "shootNeedles")]
    pub fn seak_shoot_needles(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].mv_i32(mv::N_X4) == 0 {
            return;
        }
        self.game.fighters[f.0].set_mv_i32(mv::N_X4, 0);
        if self.seak_needle_count(gobj) <= 0 {
            return;
        }
        let grounded = self.game.fighters[f.0].ground_or_air == GA_GROUND;
        let (ax, ay) = if grounded {
            (attr::X0, attr::X4)
        } else {
            (attr::X8, attr::XC)
        };
        let ax = reg(self.seak_attr_f32(f, ax));
        let ay = reg(self.seak_attr_f32(f, ay));
        let fp = &self.game.fighters[f.0];
        let mut pos = fp.cur_pos;
        let scale = reg(fp.x34_scale.y);
        let x = fp::fmuls(ax, reg(fp.facing_dir));
        pos.x = mem(fp::fmadds(scale, x, reg(pos.x)));
        let step = reg(NEEDLE_Y[self.game.objs.rng.below(9) as usize]);
        let y = if grounded {
            fp::fadds(ay, step)
        } else {
            fp::fmadds(2.0, step, ay)
        };
        pos.y = mem(fp::fmadds(scale, y, reg(pos.y)));
        pos.z = 0.0;
        let facing = self.game.fighters[f.0].facing_dir;
        let item = seakneedlethrown::spawn(self, gobj, pos, IT_NEEDLE_THROW, facing);
        if item.is_some() {
            seakneedlethrown::launch(self, item, Some(gobj), 0);
        }
        let n = self.seak_needle_count(gobj).wrapping_sub(1);
        self.set_seak_needle_count(gobj, n);
        spawn(self, GFX_THROW, Some(gobj), &[Arg::Vec(pos)]);
        self.fighter_play_sfx(f, SFX_THROW, 127, 64);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::N_80111FBC => g.seak_drop_needles(gobj),
        addr::N_801120D4 => g.seak_clear_damage_cb(gobj),
        addr::SPECIAL_N_ENTER => g.seak_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.seak_special_air_n_enter(gobj),
        0x8011_2248 => g.seak_special_n_start_anim_cb(gobj),
        0x8011_22D8 | 0x8011_24E0 => g.seak_special_n_loop_anim(gobj),
        0x8011_2384 => g.seak_special_n_cancel_anim(gobj),
        0x8011_23CC => g.seak_special_n_end_anim(gobj),
        0x8011_2450 => g.seak_special_air_n_start_anim(gobj),
        0x8011_258C => g.seak_special_air_n_cancel_anim(gobj),
        0x8011_260C => g.seak_special_air_n_end_anim(gobj),
        0x8011_26C8 | 0x8011_2798 | 0x8011_279C | 0x8011_27A0 | 0x8011_2870 | 0x8011_2874 => {
            g.seak_no_iasa(gobj)
        }
        0x8011_26CC => g.seak_special_n_loop_iasa_cb(gobj),
        0x8011_27A4 => g.seak_special_air_n_loop_iasa(gobj),
        0x8011_2878 | 0x8011_2898 | 0x8011_28B8 | 0x8011_28D8 => g.seak_ground_phys(gobj),
        0x8011_28F8 | 0x8011_2918 | 0x8011_2938 | 0x8011_2958 => g.seak_air_phys(gobj),
        0x8011_2978 => g.seak_special_n_start_coll(gobj),
        0x8011_29F8 => g.seak_special_n_loop_coll(gobj),
        0x8011_2A78 => g.seak_special_n_cancel_coll(gobj),
        0x8011_2B00 => g.seak_special_n_end_coll(gobj),
        0x8011_2B98 => g.seak_special_air_n_start_coll(gobj),
        0x8011_2C18 => g.seak_special_air_n_loop_coll(gobj),
        0x8011_2C98 => g.seak_special_air_n_cancel_coll(gobj),
        0x8011_2CE4 => g.seak_special_air_n_end_coll(gobj),
        addr::SHOOT_NEEDLES => g.seak_shoot_needles(gobj),
        _ => return false,
    }
    true
}
