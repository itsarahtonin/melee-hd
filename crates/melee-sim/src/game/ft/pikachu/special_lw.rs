//! Pikachu's Thunder (`ftpikachuspeciallw.c`): a cloud above him drops a bolt; if the bolt
//! reaches him he crackles with it (the second loop), else the move ends when the bolt does.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::camera::request_quake;
use super::super::super::ef::{self, Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::pikachuthunder;
use super::super::super::state::Running;
use super::{addr, attr, ms, mv, part};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

/// The flags each phase keeps going between the ground and the air.
const START_FLAGS: u32 = 0x0C4C_5086;
const LOOP0_FLAGS: u32 = 0x0C4C_588E;
const LOOP1_FLAGS: u32 = 0x0C4C_508E;
const END_FLAGS: u32 = 0x0C4C_5086;

/// `mv` `x4`: waiting for the bolt, and hit by it.
const WAITING: i32 = 1;
const HIT: i32 = 3;

impl Running<'_> {
    /// `ftPk_SpecialLw_CheckProperty`: whether the fighter is in a state that ignores the
    /// bolt (the 4 bits at `x2071` among 1, 2 and 4 to 13).
    #[doc(alias = "ftPk_SpecialLw_CheckProperty")]
    pub fn pk_special_lw_check_property(&self, gobj: GObjId) -> bool {
        matches!(self.game.fighter(gobj).x2070[1] >> 4, 1 | 2 | 4..=13)
    }

    /// `ftPk_SpecialLw_80127608`: the bolt reached the ground: a small quake and a rumble.
    #[doc(alias = "ftPk_SpecialLw_80127608")]
    pub fn pk_special_lw_bolt_landed(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let pos = self.game.fighters[f.0].cur_pos;
        request_quake(self, QUAKE_SMALL, Some(pos));
        self.start_rumble(f, 11, 0);
    }

    /// `ftPk_SpecialLw_SetState_Unk0`: the bolt is gone without reaching him.
    #[doc(alias = "ftPk_SpecialLw_SetState_Unk0")]
    pub fn pk_special_lw_set_hit(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(mv::LW_STATE, HIT);
    }

    /// `ftPk_SpecialLw_SetState_Unk1` (the loop's `take_dmg_cb`): hit, he stops waiting.
    #[doc(alias = "ftPk_SpecialLw_SetState_Unk1")]
    pub fn pk_special_lw_clear_hit(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).set_mv_i32(mv::LW_STATE, 0);
    }

    /// `ftPk_SpecialLw_8012765C`: whether the bolt's tip reached him: within `xC4` sideways,
    /// and below `xC8` from `xBC` above him; then the bolt hits.
    #[doc(alias = "ftPk_SpecialLw_8012765C")]
    pub fn pk_special_lw_bolt_reached(&mut self, gobj: GObjId) -> bool {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let thunder = fp.mv.gobj_at(mv::LW_THUNDER);
        if fp.mv_i32(mv::LW_STATE) == 0 {
            return false;
        }
        let Some(thunder) = thunder else {
            return false;
        };
        let tip = pikachuthunder::tip(self, thunder);
        let reach_x = reg(self.pk_attr_f32(f, attr::XC4));
        let above = reg(self.pk_attr_f32(f, attr::XBC));
        let reach_y = reg(self.pk_attr_f32(f, attr::XC8));
        let cur = self.game.fighters[f.0].cur_pos;
        let dx = fp::fsubs(reg(cur.x), reg(tip.x));
        if dx.abs() < reach_x.abs() {
            let dy = fp::fsubs(fp::fadds(reg(cur.y), above.abs()), reg(tip.y)).abs();
            if dy < reach_y && !pikachuthunder::has_hit(self, thunder) {
                pikachuthunder::hit(self, thunder);
                return true;
            }
        }
        false
    }

    /// `ftPk_SpecialLw_SpawnEffect` (the accessory callback): when the script raises the
    /// throw flag and no bolt is out, the cloud's effect and the bolt, `xD0` above him.
    #[doc(alias = "ftPk_SpecialLw_SpawnEffect")]
    pub fn pk_special_lw_spawn_effect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        if !fp.throw_flag(0) {
            return;
        }
        fp.set_throw_flag(0, false);
        if fp.mv.gobj_at(mv::LW_THUNDER).is_some() {
            return;
        }
        let mut pos = fp.cur_pos;
        pos.y = mem(fp::fadds(reg(pos.y), reg(self.pk_attr_f32(f, attr::XD0))));
        let mut ef_pos = pos;
        ef_pos.y = mem(fp::fadds(
            reg(ef_pos.y),
            reg(self.pk_attr_f32(f, attr::XCC)),
        ));
        spawn(self, 0x4C3, Some(gobj), &[Arg::Vec(ef_pos)]);
        let vel = Vec3::new(0.0, self.pk_attr_f32(f, attr::XC0), 0.0);
        let count = self.pk_attr_i32(f, attr::XD4);
        let delay = self.pk_attr_i32(f, attr::XD8);
        let kind = self.pk_attr_i32(f, attr::XDC);
        let thunder = pikachuthunder::spawn(self, gobj, pos, vel, count, delay, kind);
        self.game.fighters[f.0]
            .mv
            .set_gobj_at(mv::LW_THUNDER, thunder);
    }

    fn pk_special_lw_start(&mut self, gobj: GObjId, msid: i32) {
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[0] = 0;
        fp.x2210 = [0; 4];
        fp.set_mv_i32(mv::LW_STATE, WAITING);
        fp.set_mv_i32(mv::LW_THUNDER, 0);
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// `ftPk_SpecialLw_Enter`.
    #[doc(alias = "ftPk_SpecialLw_Enter")]
    pub fn pk_special_lw_enter(&mut self, gobj: GObjId) {
        self.pk_special_lw_start(gobj, ms::SPECIAL_LW_START);
    }

    /// `ftPk_SpecialAirLw_Enter`.
    #[doc(alias = "ftPk_SpecialAirLw_Enter")]
    pub fn pk_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.pk_special_lw_start(gobj, ms::SPECIAL_AIR_LW_START);
    }

    /// The loop's hooks: hit, it stops waiting; the cloud's callback.
    fn pk_special_lw_loop_cbs(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_cb = Some(CodeRef(addr::SPECIAL_LW_CLEAR_HIT));
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_LW_SPAWN_EFFECT));
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk00`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk00")]
    pub fn pk_special_lw_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_START, START_FLAGS);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk01`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk01")]
    pub fn pk_special_lw_start_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_START, START_FLAGS);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk02`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk02")]
    pub fn pk_special_lw_loop0_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_LOOP0, LOOP0_FLAGS);
        self.pk_special_lw_loop_cbs(gobj);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk03`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk03")]
    pub fn pk_special_lw_loop0_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_LOOP0, LOOP0_FLAGS);
        self.pk_special_lw_loop_cbs(gobj);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk04`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk04")]
    pub fn pk_special_lw_loop1_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_LOOP1, LOOP1_FLAGS);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk05`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk05")]
    pub fn pk_special_lw_loop1_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_LOOP1, LOOP1_FLAGS);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk06`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk06")]
    pub fn pk_special_lw_end_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW_END, END_FLAGS);
    }

    /// `ftPk_SpecialLw_ChangeMotion_Unk07`.
    #[doc(alias = "ftPk_SpecialLw_ChangeMotion_Unk07")]
    pub fn pk_special_lw_end_ground_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW_END, END_FLAGS);
        let f = self.game.fighter_id(gobj);
        self.clamp_air_drift(f);
    }

    /// The wind-up's end: the first loop, with its hooks.
    fn pk_special_lw_start_tick(&mut self, gobj: GObjId, msid: i32) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.change_motion_state(gobj, msid, 0x800, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).x2210 = [0; 4];
        self.pk_special_lw_loop_cbs(gobj);
    }

    /// `ftPk_SpecialLwStart_Anim`.
    #[doc(alias = "ftPk_SpecialLwStart_Anim")]
    pub fn pk_special_lw_start_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_start_tick(gobj, ms::SPECIAL_LW_LOOP0);
    }

    /// `ftPk_SpecialAirLwStart_Anim`.
    #[doc(alias = "ftPk_SpecialAirLwStart_Anim")]
    pub fn pk_special_air_lw_start_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_start_tick(gobj, ms::SPECIAL_AIR_LW_LOOP0);
    }

    /// The first loop's frame: the bolt gone or the script's end, the end; the bolt reaching
    /// him, the second loop (`jump` the speed up it gives in the air) and its sparks.
    fn pk_special_lw_loop0_tick(&mut self, gobj: GObjId, end: i32, hit: i32, air: bool) {
        let fp = self.game.fighter(gobj);
        if fp.mv_i32(mv::LW_STATE) == HIT || fp.cmd_vars[0] != 0 {
            self.game.fighter_mut(gobj).take_dmg_cb = None;
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
            return;
        }
        if self.pk_special_lw_bolt_reached(gobj) {
            self.change_motion_state(gobj, hit, 0, 0.0, 1.0, 0.0, None);
            let f = self.game.fighter_id(gobj);
            let jump = self.pk_attr_f32(f, attr::XB4);
            let fp = &mut self.game.fighters[f.0];
            fp.cmd_vars[0] = 0;
            fp.take_dmg_cb = None;
            if air {
                fp.self_vel.y = jump;
            }
            let top = self.bone_joint(gobj, part::TOP_N);
            self.fighter_effect(gobj, ef::SpawnKind::Attach, 0x4C0, top, &[]);
        }
    }

    /// `ftPk_SpecialLwLoop0_Anim`.
    #[doc(alias = "ftPk_SpecialLwLoop0_Anim")]
    pub fn pk_special_lw_loop0_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_loop0_tick(gobj, ms::SPECIAL_LW_END, ms::SPECIAL_LW_LOOP1, false);
    }

    /// `ftPk_SpecialAirLwLoop0_Anim`.
    #[doc(alias = "ftPk_SpecialAirLwLoop0_Anim")]
    pub fn pk_special_air_lw_loop0_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_loop0_tick(gobj, ms::SPECIAL_AIR_LW_END, ms::SPECIAL_AIR_LW_LOOP1, true);
    }

    /// The second loop's frame: the end when the script says.
    fn pk_special_lw_loop1_tick(&mut self, gobj: GObjId, end: i32) {
        if self.game.fighter(gobj).cmd_vars[0] != 0 {
            self.game.fighter_mut(gobj).take_dmg_cb = None;
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
        }
    }

    /// `ftPk_SpecialLwLoop1_Anim`.
    #[doc(alias = "ftPk_SpecialLwLoop1_Anim")]
    pub fn pk_special_lw_loop1_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_loop1_tick(gobj, ms::SPECIAL_LW_END);
    }

    /// `ftPk_SpecialAirLwLoop1_Anim`.
    #[doc(alias = "ftPk_SpecialAirLwLoop1_Anim")]
    pub fn pk_special_air_lw_loop1_anim(&mut self, gobj: GObjId) {
        self.pk_special_lw_loop1_tick(gobj, ms::SPECIAL_AIR_LW_END);
    }

    /// `ftPk_SpecialLwEnd_Anim`.
    #[doc(alias = "ftPk_SpecialLwEnd_Anim")]
    pub fn pk_special_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftPk_SpecialAirLwEnd_Anim`.
    #[doc(alias = "ftPk_SpecialAirLwEnd_Anim")]
    pub fn pk_special_air_lw_end_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftPk_SpecialLwStart_Coll`: off a ledge, the aerial state from the same frame.
    #[doc(alias = "ftPk_SpecialLwStart_Coll")]
    pub fn pk_special_lw_start_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.pk_special_lw_start_ground_to_air(gobj));
    }

    /// `ftPk_SpecialAirLwStart_Coll`: landing, the grounded state from the same frame.
    #[doc(alias = "ftPk_SpecialAirLwStart_Coll")]
    pub fn pk_special_air_lw_start_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.pk_special_lw_start_air_to_ground(gobj));
    }

    /// `ftPk_SpecialLwLoop0_Coll`: off a ledge, the aerial state from the same frame.
    #[doc(alias = "ftPk_SpecialLwLoop0_Coll")]
    pub fn pk_special_lw_loop0_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.pk_special_lw_loop0_ground_to_air(gobj));
    }

    /// `ftPk_SpecialAirLwLoop0_Coll`: landing, the grounded state from the same frame.
    #[doc(alias = "ftPk_SpecialAirLwLoop0_Coll")]
    pub fn pk_special_air_lw_loop0_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.pk_special_lw_loop0_air_to_ground(gobj));
    }

    /// `ftPk_SpecialLwLoop1_Coll`: off a ledge, the aerial state from the same frame.
    #[doc(alias = "ftPk_SpecialLwLoop1_Coll")]
    pub fn pk_special_lw_loop1_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.pk_special_lw_loop1_ground_to_air(gobj));
    }

    /// `ftPk_SpecialAirLwLoop1_Coll`: landing, the grounded state from the same frame.
    #[doc(alias = "ftPk_SpecialAirLwLoop1_Coll")]
    pub fn pk_special_air_lw_loop1_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.pk_special_lw_loop1_air_to_ground(gobj));
    }

    /// `ftPk_SpecialLwEnd_Coll`: off a ledge, the aerial state from the same frame.
    #[doc(alias = "ftPk_SpecialLwEnd_Coll")]
    pub fn pk_special_lw_end_coll(&mut self, gobj: GObjId) {
        self.ground_coll_fall_off_then(gobj, |g, gobj| g.pk_special_lw_end_ground_to_air(gobj));
    }

    /// `ftPk_SpecialAirLwEnd_Coll`: landing, the grounded state from the same frame.
    #[doc(alias = "ftPk_SpecialAirLwEnd_Coll")]
    pub fn pk_special_air_lw_end_coll(&mut self, gobj: GObjId) {
        self.air_coll_then(gobj, |g, gobj| g.pk_special_lw_end_air_to_ground(gobj));
    }

    /// `ftPk_SpecialAirLwLoop1_Phys`: falling at `xB8`, with quick air friction.
    #[doc(alias = "ftPk_SpecialAirLwLoop1_Phys")]
    pub fn pk_special_air_lw_loop1_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.pk_attr_f32(f, attr::XB8));
        let fp = &mut self.game.fighters[f.0];
        let t = reg(fp.co_attrs.terminal_velocity);
        Self::fall(fp, g, t);
        self.self_accel_deaccel_quick_air(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_LW_SET_HIT => g.pk_special_lw_set_hit(gobj),
        addr::SPECIAL_LW_BOLT_LANDED => g.pk_special_lw_bolt_landed(gobj),
        addr::SPECIAL_LW_CLEAR_HIT => g.pk_special_lw_clear_hit(gobj),
        addr::SPECIAL_LW_SPAWN_EFFECT => g.pk_special_lw_spawn_effect(gobj),
        addr::SPECIAL_LW_ENTER => g.pk_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.pk_special_air_lw_enter(gobj),
        addr::SPECIAL_LW_START_AIR_TO_GROUND => g.pk_special_lw_start_air_to_ground(gobj),
        addr::SPECIAL_LW_START_GROUND_TO_AIR => g.pk_special_lw_start_ground_to_air(gobj),
        addr::SPECIAL_LW_LOOP0_AIR_TO_GROUND => g.pk_special_lw_loop0_air_to_ground(gobj),
        addr::SPECIAL_LW_LOOP0_GROUND_TO_AIR => g.pk_special_lw_loop0_ground_to_air(gobj),
        addr::SPECIAL_LW_LOOP1_AIR_TO_GROUND => g.pk_special_lw_loop1_air_to_ground(gobj),
        addr::SPECIAL_LW_LOOP1_GROUND_TO_AIR => g.pk_special_lw_loop1_ground_to_air(gobj),
        addr::SPECIAL_LW_END_AIR_TO_GROUND => g.pk_special_lw_end_air_to_ground(gobj),
        addr::SPECIAL_LW_END_GROUND_TO_AIR => g.pk_special_lw_end_ground_to_air(gobj),
        addr::SPECIAL_LW_START_ANIM => g.pk_special_lw_start_anim(gobj),
        addr::SPECIAL_AIR_LW_START_ANIM => g.pk_special_air_lw_start_anim(gobj),
        addr::SPECIAL_LW_LOOP0_ANIM => g.pk_special_lw_loop0_anim(gobj),
        addr::SPECIAL_AIR_LW_LOOP0_ANIM => g.pk_special_air_lw_loop0_anim(gobj),
        addr::SPECIAL_LW_LOOP1_ANIM => g.pk_special_lw_loop1_anim(gobj),
        addr::SPECIAL_AIR_LW_LOOP1_ANIM => g.pk_special_air_lw_loop1_anim(gobj),
        addr::SPECIAL_LW_END_ANIM => g.pk_special_lw_end_anim(gobj),
        addr::SPECIAL_AIR_LW_END_ANIM => g.pk_special_air_lw_end_anim(gobj),
        addr::SPECIAL_LW_START_PHYS
        | addr::SPECIAL_LW_LOOP0_PHYS
        | addr::SPECIAL_LW_LOOP1_PHYS
        | addr::SPECIAL_LW_END_PHYS => g.pk_ground_friction(gobj),
        addr::SPECIAL_AIR_LW_START_PHYS
        | addr::SPECIAL_AIR_LW_LOOP0_PHYS
        | addr::SPECIAL_AIR_LW_END_PHYS => g.pk_air_physics_no_drift(gobj),
        addr::SPECIAL_AIR_LW_LOOP1_PHYS => g.pk_special_air_lw_loop1_phys(gobj),
        addr::SPECIAL_LW_START_COLL => g.pk_special_lw_start_coll(gobj),
        addr::SPECIAL_AIR_LW_START_COLL => g.pk_special_air_lw_start_coll(gobj),
        addr::SPECIAL_LW_LOOP0_COLL => g.pk_special_lw_loop0_coll(gobj),
        addr::SPECIAL_AIR_LW_LOOP0_COLL => g.pk_special_air_lw_loop0_coll(gobj),
        addr::SPECIAL_LW_LOOP1_COLL => g.pk_special_lw_loop1_coll(gobj),
        addr::SPECIAL_AIR_LW_LOOP1_COLL => g.pk_special_air_lw_loop1_coll(gobj),
        addr::SPECIAL_LW_END_COLL => g.pk_special_lw_end_coll(gobj),
        addr::SPECIAL_AIR_LW_END_COLL => g.pk_special_air_lw_end_coll(gobj),
        _ => return false,
    }
    true
}
