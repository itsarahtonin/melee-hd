//! Raptor Boost and Gerudo Dragon (`ftcaptainspecials.c`): the wind-up, whose grab box
//! detecting a fighter or an item starts the lunge, and the lunge, on the ground and in the
//! air.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::ef::{Arg, eflib, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it;
use super::super::super::state::Running;
use super::{addr, attr, ms, mv, part, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// The flags the lunge keeps from the wind-up.
const DETECT_FLAGS: u32 = 0x0C4C_5082;

/// `HSD_GOBJ_CLASS_FIGHTER` and `HSD_GOBJ_CLASS_ITEM`.
const CLASS_FIGHTER: u16 = 4;
const CLASS_ITEM: u16 = 6;

/// The item kinds the wind-up detects: the common items before `It_Kind_BombHei`, the
/// enemies (`It_Kind_Kuriboh` up to `It_Kind_Octarock_Stone`, `It_Kind_Old_Kuri` up to
/// `It_Kind_Arwing_Laser`) and `It_PKind_Random`.
const IT_KIND_BOMB_HEI: i32 = 6;

/// `Collide_LeftWallMask` and `Collide_RightWallMask` in `CollData::env_flags`.
const LEFT_WALL: i32 = 0x3F;
const RIGHT_WALL: i32 = 0xFC0;

impl Running<'_> {
    /// The side special's effects end and their flags clear, `ftCa_SpecialS_RemoveGFX`.
    #[doc(alias = "ftCa_SpecialS_RemoveGFX")]
    pub fn ca_special_s_remove_gfx(&mut self, gobj: GObjId) {
        eflib::destroy_all(self, gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.set_u_i32(u::DURING_SPECIALS, 0);
        fp.set_u_i32(u::DURING_SPECIALS_START, 0);
    }

    /// The side special's damage and death hooks (`setCallbacks`): its effects go.
    fn ca_special_s_set_cbs(&mut self, gobj: GObjId) {
        self.set_damage_cb(gobj, Some(addr::REMOVE_GFX_CB));
    }

    /// The wind-up's start after its state (`ftCa_SpecialS_Enter`, `setupAirStart`): the
    /// head's effect, the detection hook, no speed.
    fn ca_special_s_start(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.ca_special_s_set_cbs(gobj);
        self.anim_update(gobj);
        if let Some(id) = self.ca_effect_id(gobj, 0x491, 0x50D) {
            let bone = if id == 0x491 {
                part::HEAD_N
            } else {
                part::L_2ND_NB
            };
            let j = self.bone_joint(gobj, bone);
            spawn(self, id, Some(gobj), &[Arg::JObj(j)]);
            self.game
                .fighter_mut(gobj)
                .set_u_i32(u::DURING_SPECIALS_START, 1);
        }
        self.game.fighter_mut(gobj).set_u_i32(u::DURING_SPECIALS, 0);
        self.set_effect_hitlag_cbs(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.hurtbox_detect_cb = Some(CodeRef(addr::SPECIAL_S_ON_DETECT));
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
    }

    /// `ftCa_SpecialS_Enter`: the script's variables cleared, landed, the wind-up standing.
    #[doc(alias = "ftCa_SpecialS_Enter")]
    pub fn ca_special_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.land(gobj);
        self.ca_special_s_start(gobj, ms::SPECIAL_S_START);
        self.game.fighter_mut(gobj).gr_vel = 0.0;
    }

    /// `ftCa_SpecialAirS_Enter`: the aerial wind-up, its fall still, the jumps used.
    #[doc(alias = "ftCa_SpecialAirS_Enter")]
    pub fn ca_special_air_s_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.ca_special_s_start(gobj, ms::SPECIAL_AIR_S_START);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].set_mv_f32(mv::S_GRAV, 0.0);
        self.go_airborne_jumped(f);
    }

    /// The wind-up's box detected something (`ftCa_SpecialS_OnDetect`, the
    /// `hurtbox_detect_cb`): once the script allows (`cmd_vars[0]`), a fighter, or an item the
    /// move can grab, starts the lunge.
    #[doc(alias = "ftCa_SpecialS_OnDetect")]
    pub fn ca_special_s_on_detect(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 0 {
            return;
        }
        let detected = fp.unk_gobj.expect("a detection without what it detected");
        let lunge = match self.game.world.gobjs[detected.0].classifier {
            CLASS_FIGHTER => true,
            CLASS_ITEM => {
                let k = it::item(self.game, detected).kind;
                k < IT_KIND_BOMB_HEI
                    || (it::kinds::KURIBOH..it::kinds::OCTAROCK_STONE).contains(&k)
                    || (it::kinds::OLD_KURI..it::kinds::ARWING_LASER).contains(&k)
                    || k == it::kinds::PKIND_RANDOM
            }
            _ => false,
        };
        if !lunge {
            return;
        }
        match self.game.fighter(gobj).motion_id {
            ms::SPECIAL_S_START => self.ca_special_s_detect_ground(gobj),
            ms::SPECIAL_AIR_S_START => self.ca_special_s_detect_air(gobj),
            _ => {}
        }
    }

    /// The grounded lunge (`onDetectGround`): landed, its speed scaled by
    /// `specials_gr_vel_x`.
    fn ca_special_s_detect_ground(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mul = reg(self.ca_attr_f32(f, attr::SPECIALS_GR_VEL_X));
        self.land(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_S, DETECT_FLAGS, 0.0, 1.0, 0.0, None);
        self.ca_special_s_set_cbs(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.gr_vel = mem(fp::fmuls(reg(fp.gr_vel), mul));
    }

    /// The aerial lunge (`onDetectAir`).
    fn ca_special_s_detect_air(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, DETECT_FLAGS, 0.0, 1.0, 0.0, None);
        self.ca_special_s_set_cbs(gobj);
        self.game.fighter_mut(gobj).self_vel.z = 0.0;
    }

    /// `ftCa_SpecialSStart_Anim`: waiting when the wind-up misses.
    #[doc(alias = "ftCa_SpecialSStart_Anim")]
    pub fn ca_special_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The lunge's trail, once (`during_specials`), facing the way the fighter does.
    fn ca_special_s_trail(&mut self, gobj: GObjId, ca: i32, gn: i32) {
        if self.game.fighter(gobj).u_i32(u::DURING_SPECIALS) != 0 {
            return;
        }
        if let Some(id) = self.ca_effect_id(gobj, ca, gn) {
            let j = self.bone_joint(gobj, part::TRANS_N);
            let facing = self.game.fighter(gobj).facing_dir;
            spawn(self, id, Some(gobj), &[Arg::JObj(j), Arg::Float(facing)]);
            self.game.fighter_mut(gobj).set_u_i32(u::DURING_SPECIALS, 1);
        }
        self.set_effect_hitlag_cbs(gobj);
    }

    /// `ftCa_SpecialS_Anim`: the trail, then waiting when the lunge ends.
    #[doc(alias = "ftCa_SpecialS_Anim")]
    pub fn ca_special_s_anim(&mut self, gobj: GObjId) {
        self.ca_special_s_trail(gobj, 0x492, 0x50E);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// The aerial move's end: airborne, a fall, or a helpless one with `lag` frames of
    /// landing if it has any.
    fn ca_special_s_fall(&mut self, gobj: GObjId, lag: usize, clamp: bool) {
        let f = self.game.fighter_id(gobj);
        let lag = self.ca_attr_f32(f, lag);
        self.go_airborne_jumped(f);
        if lag == 0.0 {
            self.enter_fall(gobj);
            return;
        }
        if clamp {
            self.clamp_air_drift(f);
        }
        self.enter_fall_special(gobj, 1, 1, 0, 1.0, lag, 0.0, true);
    }

    /// `ftCa_SpecialAirSStart_Anim`: a miss's fall when the wind-up ends.
    #[doc(alias = "ftCa_SpecialAirSStart_Anim")]
    pub fn ca_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ca_special_s_fall(gobj, attr::SPECIALS_MISS_LANDING_LAG, false);
        }
    }

    /// `ftCa_SpecialAirS_Anim`: the trail, then a hit's fall when the lunge ends.
    #[doc(alias = "ftCa_SpecialAirS_Anim")]
    pub fn ca_special_air_s_anim(&mut self, gobj: GObjId) {
        self.ca_special_s_trail(gobj, 0x493, 0x50F);
        if !self.anim_frames_remaining(gobj) {
            self.ca_special_s_fall(gobj, attr::SPECIALS_HIT_LANDING_LAG, false);
        }
    }

    /// `ftCa_SpecialSStart_IASA`, `ftCa_SpecialS_IASA`, `ftCa_SpecialAirSStart_IASA` and
    /// `ftCa_SpecialAirS_IASA`: nothing.
    #[doc(alias = "ftCa_SpecialSStart_IASA")]
    #[doc(alias = "ftCa_SpecialS_IASA")]
    #[doc(alias = "ftCa_SpecialAirSStart_IASA")]
    #[doc(alias = "ftCa_SpecialAirS_IASA")]
    pub fn ca_special_s_iasa(&mut self, _gobj: GObjId) {}

    /// `ftCa_SpecialSStart_Phys` and `ftCa_SpecialS_Phys`: friction or the animation's motion.
    #[doc(alias = "ftCa_SpecialSStart_Phys")]
    #[doc(alias = "ftCa_SpecialS_Phys")]
    pub fn ca_special_s_phys(&mut self, gobj: GObjId) {
        self.ground_friction_or_anim(gobj);
    }

    /// The aerial move's fall (`mv` `grav`), by `specials_grav` down to
    /// `specials_terminal_vel`.
    fn ca_special_s_air_fall(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.ca_attr_f32(f, attr::SPECIALS_GRAV));
        let t = reg(self.ca_attr_f32(f, attr::SPECIALS_TERMINAL_VEL));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::S_GRAV, fp::fsubs(reg(fp.mv_f32(mv::S_GRAV)), g));
        if reg(fp.mv_f32(mv::S_GRAV)) < fp::fneg(t) {
            fp.set_mv_f32(mv::S_GRAV, fp::fneg(t));
        }
        fp.self_vel.y = fp.mv_f32(mv::S_GRAV);
    }

    /// `ftCa_SpecialAirSStart_Phys`: the animation's motion, and once the script says
    /// (`cmd_vars[1]`) the fall.
    #[doc(alias = "ftCa_SpecialAirSStart_Phys")]
    pub fn ca_special_air_s_start_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        if self.game.fighter(gobj).cmd_vars[1] == 1 {
            self.ca_special_s_air_fall(gobj);
        }
    }

    /// `ftCa_SpecialAirS_Phys`: the animation's motion and the fall.
    #[doc(alias = "ftCa_SpecialAirS_Phys")]
    pub fn ca_special_air_s_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.ca_special_s_air_fall(gobj);
    }

    /// `ftCa_SpecialSStart_Coll`: until the script says (`cmd_vars[2]`), ground collision that
    /// falls; then off the ground, a miss's fall without its effects; and once the move can
    /// detect (`cmd_vars[0]`), a wall ahead ends it.
    #[doc(alias = "ftCa_SpecialSStart_Coll")]
    pub fn ca_special_s_start_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[2] == 0 {
            self.ground_coll(gobj);
            return;
        }
        if !self.ground_coll_off_ledges(gobj) {
            eflib::destroy_all(self, gobj);
            self.ca_special_s_fall(gobj, attr::SPECIALS_MISS_LANDING_LAG, true);
            return;
        }
        let fp = self.game.fighter(gobj);
        if fp.cmd_vars[0] == 1 {
            let facing = reg(fp.facing_dir);
            let env = fp.coll_data.env_flags;
            if (facing == 1.0 && env & LEFT_WALL != 0) || (facing == -1.0 && env & RIGHT_WALL != 0)
            {
                eflib::destroy_all(self, gobj);
                self.enter_wait(gobj);
            }
        }
    }

    /// `ftCa_SpecialS_Coll`: off the ground, a hit's fall without its effects.
    #[doc(alias = "ftCa_SpecialS_Coll")]
    pub fn ca_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            eflib::destroy_all(self, gobj);
            self.ca_special_s_fall(gobj, attr::SPECIALS_HIT_LANDING_LAG, true);
        }
    }

    /// `ftCa_SpecialAirSStart_Coll`: landing helpless with a miss's lag.
    #[doc(alias = "ftCa_SpecialAirSStart_Coll")]
    pub fn ca_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.ca_attr_f32(f, attr::SPECIALS_MISS_LANDING_LAG);
            eflib::destroy_all(self, gobj);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// `ftCa_SpecialAirS_Coll`: landing helpless with a hit's lag, its speed kept.
    #[doc(alias = "ftCa_SpecialAirS_Coll")]
    pub fn ca_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            let f = self.game.fighter_id(gobj);
            let lag = self.ca_attr_f32(f, attr::SPECIALS_HIT_LANDING_LAG);
            let fp = &mut self.game.fighters[f.0];
            fp.gr_vel = fp.self_vel.x;
            eflib::destroy_all(self, gobj);
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_REMOVE_GFX => g.ca_special_s_remove_gfx(gobj),
        addr::SPECIAL_S_ENTER => g.ca_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.ca_special_air_s_enter(gobj),
        addr::SPECIAL_S_ON_DETECT => g.ca_special_s_on_detect(gobj),
        addr::SPECIAL_S_START_ANIM => g.ca_special_s_start_anim(gobj),
        addr::SPECIAL_S_ANIM => g.ca_special_s_anim(gobj),
        addr::SPECIAL_AIR_S_START_ANIM => g.ca_special_air_s_start_anim(gobj),
        addr::SPECIAL_AIR_S_ANIM => g.ca_special_air_s_anim(gobj),
        addr::SPECIAL_S_START_IASA
        | addr::SPECIAL_S_IASA
        | addr::SPECIAL_AIR_S_START_IASA
        | addr::SPECIAL_AIR_S_IASA => g.ca_special_s_iasa(gobj),
        addr::SPECIAL_S_START_PHYS | addr::SPECIAL_S_PHYS => g.ca_special_s_phys(gobj),
        addr::SPECIAL_AIR_S_START_PHYS => g.ca_special_air_s_start_phys(gobj),
        addr::SPECIAL_AIR_S_PHYS => g.ca_special_air_s_phys(gobj),
        addr::SPECIAL_S_START_COLL => g.ca_special_s_start_coll(gobj),
        addr::SPECIAL_S_COLL => g.ca_special_s_coll(gobj),
        addr::SPECIAL_AIR_S_START_COLL => g.ca_special_air_s_start_coll(gobj),
        addr::SPECIAL_AIR_S_COLL => g.ca_special_air_s_coll(gobj),
        _ => return false,
    }
    true
}
