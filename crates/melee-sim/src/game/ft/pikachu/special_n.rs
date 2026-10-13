//! Pikachu's Thunder Jolt (`ftpikachuspecialn.c`): a jolt thrown ahead when the script
//! raises its flag, which runs along the ground; Pichu's, with his own sound.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::it::kinds::pikachutjoltground;
use super::super::super::state::Running;
use super::{KIND_PICHU, KIND_PIKACHU, addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The flags the throw keeps going between the ground and the air.
const COLL_FLAGS: u32 = 0x0C4C_5082;

impl Running<'_> {
    fn pk_special_n_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[3] = 0;
        fp.cmd_vars[2] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[0] = 0;
        self.anim_update(gobj);
    }

    /// `ftPk_SpecialN_Enter`.
    // Ports `ftPk_SpecialN_Enter`; aliased once a check reaches it.
    pub fn pk_special_n_enter(&mut self, gobj: GObjId) {
        self.pk_special_n_start(gobj, ms::SPECIAL_N);
    }

    /// `ftPk_SpecialAirN_Enter`.
    #[doc(alias = "ftPk_SpecialAirN_Enter")]
    pub fn pk_special_air_n_enter(&mut self, gobj: GObjId) {
        self.pk_special_n_start(gobj, ms::SPECIAL_AIR_N);
    }

    /// The throw's frame: when the script raises its flag the first time, the jolt from
    /// `offset` ahead (scaled with the fighter), and the character's sound. Both the ground
    /// and the air throw spawn `specialn_itkind`.
    fn pk_special_n_throw(&mut self, gobj: GObjId, offset_x: usize, offset_y: usize) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if fp.cmd_vars[0] != 1 {
            return;
        }
        fp.cmd_vars[0] = 0;
        if fp.cmd_vars[1] != 0 {
            return;
        }
        fp.cmd_vars[1] = 1;
        let ox = reg(self.pk_attr_f32(f, offset_x));
        let oy = reg(self.pk_attr_f32(f, offset_y));
        let kind = self.pk_attr_i32(f, attr::SPECIALN_ITKIND);
        let fp = &self.game.fighters[f.0];
        let scale = reg(fp.x34_scale.y);
        let pos = Vec3::new(
            mem(fp::fmadds(
                scale,
                fp::fmuls(ox, reg(fp.facing_dir)),
                reg(fp.cur_pos.x),
            )),
            mem(fp::fmadds(oy, scale, reg(fp.cur_pos.y))),
            0.0,
        );
        let facing = fp.facing_dir;
        pikachutjoltground::spawn(self, gobj, pos, facing, kind);
        match self.game.fighters[f.0].kind {
            KIND_PIKACHU => self.fighter_play_sfx(f, 0x3A9CC, 127, 64),
            KIND_PICHU => self.fighter_play_sfx(f, 0x382B3, 127, 64),
            _ => {}
        }
    }

    /// `ftPk_SpecialN_Anim`: the throw, and waiting when it ends.
    #[doc(alias = "ftPk_SpecialN_Anim")]
    pub fn pk_special_n_anim(&mut self, gobj: GObjId) {
        self.pk_special_n_throw(
            gobj,
            attr::SPECIALN_SPAWN_OFFSET_X,
            attr::SPECIALN_SPAWN_OFFSET_Y,
        );
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPk_SpecialAirN_Anim`: the throw, and when it ends a fall, helpless when
    /// `specialairn_landing_lag` isn't zero.
    #[doc(alias = "ftPk_SpecialAirN_Anim")]
    pub fn pk_special_air_n_anim(&mut self, gobj: GObjId) {
        self.pk_special_n_throw(
            gobj,
            attr::SPECIALAIRN_SPAWN_OFFSET_X,
            attr::SPECIALAIRN_SPAWN_OFFSET_Y,
        );
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.pk_attr_f32(f, attr::SPECIALAIRN_LANDING_LAG);
            if reg(lag) == 0.0 {
                self.enter_fall(gobj);
            } else {
                self.fall_special(gobj, 1.0, lag);
            }
        }
    }

    /// `ftPk_SpecialN_Coll`: off a ledge, the aerial throw from the same frame.
    #[doc(alias = "ftPk_SpecialN_Coll")]
    pub fn pk_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_FLAGS);
        }
    }

    /// `ftPk_SpecialAirN_Coll`: landing, the grounded throw from the same frame, without
    /// falling speed.
    #[doc(alias = "ftPk_SpecialAirN_Coll")]
    pub fn pk_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.land(gobj);
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.y = 0.0;
            let frame = fp.cur_anim_frame;
            self.change_motion_state(gobj, ms::SPECIAL_N, COLL_FLAGS, frame, 1.0, 0.0, None);
        }
    }

    /// The Skull Bash's spark on the hip, once per state, its effects paused in hitlag.
    fn pk_special_n_spark(&mut self, gobj: GObjId, id: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let hip = self.part_joint(gobj, part::HIP_N);
            spawn(self, id, Some(gobj), &[Arg::JObj(hip)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftPk_SpecialN_SpawnEffect0`: the Skull Bash's charge spark (effect 0x4BE).
    // Ports `ftPk_SpecialN_SpawnEffect0`; aliased once a check reaches it.
    pub fn pk_special_n_spawn_effect0(&mut self, gobj: GObjId) {
        self.pk_special_n_spark(gobj, 0x4BE);
    }

    /// `ftPk_SpecialN_SpawnEffect1`: the Skull Bash's launch spark (effect 0x4BF).
    #[doc(alias = "ftPk_SpecialN_SpawnEffect1")]
    pub fn pk_special_n_spawn_effect1(&mut self, gobj: GObjId) {
        self.pk_special_n_spark(gobj, 0x4BF);
    }

    /// `ftPk_SpecialN_80124DC8` (the Skull Bash's `x21EC`): charged from the start by a smash
    /// input (the stick's tilt timer below `x1C`).
    #[doc(alias = "ftPk_SpecialN_80124DC8")]
    pub fn pk_special_n_set_charge(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let window = reg(self.pk_attr_f32(f, attr::X1C));
        let charge = reg(self.pk_attr_f32(f, attr::X20));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        let timer = fp::frsp(f64::from(i32::from(fp.active_sticky.lstick.x)));
        if timer < window {
            fp.set_mv_i32(mv::S_CHARGE, fp::fctiwz(charge));
            fp.x2070[2] |= 0x08;
        } else {
            fp.set_mv_i32(mv::S_CHARGE, 0);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_N_ENTER => g.pk_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.pk_special_air_n_enter(gobj),
        addr::SPECIAL_N_ANIM => g.pk_special_n_anim(gobj),
        addr::SPECIAL_AIR_N_ANIM => g.pk_special_air_n_anim(gobj),
        addr::SPECIAL_N_IASA | addr::SPECIAL_AIR_N_IASA => {}
        addr::SPECIAL_N_PHYS => g.ground_friction(gobj),
        addr::SPECIAL_AIR_N_PHYS => g.air_physics(gobj),
        addr::SPECIAL_N_COLL => g.pk_special_n_coll(gobj),
        addr::SPECIAL_AIR_N_COLL => g.pk_special_air_n_coll(gobj),
        addr::SPECIAL_N_SPAWN_EFFECT0 => g.pk_special_n_spawn_effect0(gobj),
        addr::SPECIAL_N_SPAWN_EFFECT1 => g.pk_special_n_spawn_effect1(gobj),
        addr::SPECIAL_N_SET_CHARGE => g.pk_special_n_set_charge(gobj),
        _ => return false,
    }
    true
}
