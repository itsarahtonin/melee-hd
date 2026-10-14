//! The Spin Attack (`ftlinkspecialhi.c`): on the ground it spins in place and may slide off
//! a ledge into the air version, which rises once and ends in a helpless fall; and the
//! helpers his boomerang reads.

use gekko_fp as fp;

use super::super::super::ef::{self, Arg};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::GA_GROUND;
use super::super::parts::TRANS_N;
use super::{KIND_LINK, addr, attr, ms, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FtPart_L2ndNa` and `FtPart_L3rdNa`, taken as raw parts (not through the bone table), and
/// `FtPart_LThumbNb`.
const PART_L2ND_NA: i32 = 24;
const PART_L3RD_NA: i32 = 26;
const PART_L_THUMB_NB: i32 = 31;
/// The spin's trail on the ground and in the air.
const GFX_SPIN: i32 = 1211;
const GFX_SPIN_AIR: i32 = 1212;
/// `doColl`'s motion flags: the ground spin carried into the air version as it is.
const COLL_MF: u32 = 0x2 | 0x8 | 0x80 | 0x1000 | 0x4000 | 0x4_0000 | 0x8_0000 | 0x40_0000;
const COLL_MF_HI: u32 = 0x400_0000 | 0x800_0000;

impl Running<'_> {
    /// The spin's trail once (`onAccessory4`, the accessory hook its entries set): from his
    /// TransN to his left hand's second (Link) or third (Young Link) joint, paused in hitlag.
    #[doc(alias = "onAccessory4@800EBA4C")]
    pub fn link_special_hi_trail(&mut self, gobj: GObjId) {
        let from = self.part_joint(gobj, TRANS_N);
        let fp = self.game.fighter(gobj);
        let to_part = if fp.kind == KIND_LINK {
            PART_L2ND_NA
        } else {
            PART_L3RD_NA
        };
        let to = self.bone_joint(gobj, to_part);
        if !self.game.fighter(gobj).x2219_b0 {
            let id = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
                GFX_SPIN
            } else {
                GFX_SPIN_AIR
            };
            ef::spawn(self, id, Some(gobj), &[Arg::JObj(from), Arg::JObj(to)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftLk_SpecialHi_Enter`.
    #[doc(alias = "ftLk_SpecialHi_Enter")]
    pub fn link_special_hi_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_ON_ACCESSORY4));
    }

    /// `ftLk_SpecialAirHi_Enter`: the horizontal speed scaled, a rise, and no jumps left.
    #[doc(alias = "ftLk_SpecialAirHi_Enter")]
    pub fn link_special_air_hi_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.change_motion_state(gobj, ms::SPECIAL_AIR_HI, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let x34 = self.link_attr_f32(f, attr::X34);
        let x40 = self.link_attr_f32(f, attr::X40);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(reg(fp.self_vel.x), reg(x34)));
        fp.self_vel.y = x40;
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_HI_ON_ACCESSORY4));
    }

    /// `ftLk_SpecialHi_Anim`: back to waiting at the end.
    #[doc(alias = "ftLk_SpecialHi_Anim")]
    pub fn link_special_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftLk_SpecialAirHi_Anim`: the helpless fall at the end.
    #[doc(alias = "ftLk_SpecialAirHi_Anim")]
    pub fn link_special_air_hi_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            let drift = self.link_attr_f32(f, attr::SPECIAL_AIR_HI_DRIFT_STICK_MUL);
            let lag = self.link_attr_f32(f, attr::X30);
            self.enter_fall_special(gobj, 1, 1, 0, drift, lag, 0.0, true);
        }
    }

    /// `ftLk_SpecialAirHi_Phys`: lighter gravity and his own drift.
    #[doc(alias = "ftLk_SpecialAirHi_Phys")]
    pub fn link_special_air_hi_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let grav = self.link_attr_f32(f, attr::SPECIAL_HI_GRAV_MUL);
        let stick = self.link_attr_f32(f, attr::SPECIAL_AIR_HI_DRIFT_STICK_MUL);
        let max = self.link_attr_f32(f, attr::SPECIAL_AIR_HI_DRIFT_MAX_MUL);
        let fp = &mut self.game.fighters[f.0];
        let ca = &fp.co_attrs;
        let (gravity, terminal) = (ca.gravity, ca.terminal_velocity);
        let (stick_mul, drift_max) = (ca.air_drift_stick_mul, ca.air_drift_max);
        Self::fall(fp, fp::fmuls(reg(gravity), reg(grav)), reg(terminal));
        Self::drift_simple(
            fp,
            0.0,
            fp::fmuls(reg(stick_mul), reg(stick)),
            fp::fmuls(reg(drift_max), reg(max)),
        );
    }

    /// `ftLk_SpecialHi_Phys`: the air's when off the ground, else friction.
    #[doc(alias = "ftLk_SpecialHi_Phys")]
    pub fn link_special_hi_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air != GA_GROUND {
            self.link_special_air_hi_phys(gobj);
        } else {
            self.ground_friction(gobj);
        }
    }

    /// `ftLk_SpecialHi_Coll`: off a ledge, the air version at the same frame (`doColl`).
    #[doc(alias = "ftLk_SpecialHi_Coll")]
    #[doc(alias = "doColl@800EBE64")]
    pub fn link_special_hi_coll(&mut self, gobj: GObjId) {
        if self.ground_coll_off_ledges(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne_jumped(f);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_HI,
            COLL_MF | COLL_MF_HI,
            frame,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftLk_SpecialAirHi_Coll`: landing helpless (its lag read through the decomp's cast of
    /// the attributes as a fighter, at `facing_dir1`'s offset: the attributes' `x30`), or a
    /// ledge.
    #[doc(alias = "ftLk_SpecialAirHi_Coll")]
    pub fn link_special_air_hi_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.air_coll_ground_and_ledge(gobj, 0) {
            let lag = self.link_attr_f32(f, attr::X30);
            self.enter_landing_fall_special(gobj, 0, lag);
        } else {
            self.catch_ledge_check(gobj);
        }
    }

    /// `ftLk_SpecialHi_ProcessPartLThumbNb`: his left thumb's bone.
    #[doc(alias = "ftLk_SpecialHi_ProcessPartLThumbNb")]
    pub fn link_left_thumb_bone(&self, gobj: GObjId) -> i32 {
        let fp = self.game.fighter(gobj);
        self.bone_index(fp, PART_L_THUMB_NB)
    }

    /// `ftLk_SpecialHi_GetPosWithAdjustedY`: his position raised by the attributes' offset.
    #[doc(alias = "ftLk_SpecialHi_GetPosWithAdjustedY")]
    pub fn link_pos_raised(&self, gobj: GObjId) -> Vec3 {
        let f = self.game.fighter_id(gobj);
        let mut pos = self.game.fighters[f.0].cur_pos;
        let off = self.link_attr_f32(f, attr::SPECIAL_HI_POS_Y_OFFSET);
        pos.y = mem(fp::fadds(reg(pos.y), reg(off)));
        pos
    }

    /// `ftLk_SpecialHi_GetFv4`.
    #[doc(alias = "ftLk_SpecialHi_GetFv4")]
    pub fn link_u_x4(&self, gobj: GObjId) -> i32 {
        self.game.fighter(gobj).u_i32(u::X4)
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_HI_ON_ACCESSORY4 => g.link_special_hi_trail(gobj),
        addr::SPECIAL_HI_ENTER => g.link_special_hi_enter(gobj),
        addr::SPECIAL_AIR_HI_ENTER => g.link_special_air_hi_enter(gobj),
        addr::SPECIAL_HI_ANIM => g.link_special_hi_anim(gobj),
        addr::SPECIAL_AIR_HI_ANIM => g.link_special_air_hi_anim(gobj),
        addr::SPECIAL_HI_IASA | addr::SPECIAL_AIR_HI_IASA => {}
        addr::SPECIAL_HI_PHYS => g.link_special_hi_phys(gobj),
        addr::SPECIAL_AIR_HI_PHYS => g.link_special_air_hi_phys(gobj),
        addr::SPECIAL_HI_COLL => g.link_special_hi_coll(gobj),
        addr::SPECIAL_AIR_HI_COLL => g.link_special_air_hi_coll(gobj),
        _ => return false,
    }
    true
}
