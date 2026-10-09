//! The transformation into Zelda (`ftseakspeciallw.c`): the first half's slowdown and wind,
//! the swap itself (`ftCommon_8007EFC8`, which the transformation's part of the port owns), and
//! the second half, which Zelda's own down special also ends in.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{GA_GROUND, addr, attr, ext, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftSk_MF_SpecialLw_Coll` (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;

/// `FtPart_TopN`, `FtPart_HipN`, `FtPart_R2ndNb`: Sheik's code indexes `fp->parts` by the
/// part directly.
const PART_TOP_N: i32 = 0;
const PART_HIP_N: i32 = 4;
const PART_R2ND_NB: i32 = 43;

/// The first half's effect, the second half's.
const GFX_START: i32 = 0x4FC;
const GFX_END: i32 = 0x4FD;

impl Running<'_> {
    /// An effect on a part's joint, once (`x2219_b0`), then the hitlag hooks and no more
    /// accessory.
    fn seak_part_gfx(&mut self, gobj: GObjId, gfx: i32, part: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, part);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The first half's effect, `fn_80114034` (an accessory).
    #[doc(alias = "fn_80114034")]
    pub fn seak_special_lw_start_gfx(&mut self, gobj: GObjId) {
        self.seak_part_gfx(gobj, GFX_START, PART_R2ND_NB);
    }

    /// The second half's effect, `fn_801140B0` (an accessory).
    #[doc(alias = "fn_801140B0")]
    pub fn seak_special_lw_end_gfx(&mut self, gobj: GObjId) {
        self.seak_part_gfx(gobj, GFX_END, PART_HIP_N);
    }

    /// The swap, `fn_8011412C` (an accessory): `ftCommon_8007EFC8` with Zelda's callback.
    #[doc(alias = "fn_8011412C")]
    pub fn seak_special_lw_swap(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).accessory4_cb = None;
        self.seak_ext(
            ext::FTCOMMON_8007EFC8,
            &[
                Val::GObj(Some(gobj)),
                Val::Int(ext::FTZD_SPECIAL_LW_8013B4D8 as i32),
            ],
        );
    }

    /// The first half, `ftSk_SpecialLw_Enter` and `ftSk_SpecialAirLw_Enter`: the speeds
    /// divided down (`x60`, `x64`), and a wind at the top.
    fn seak_special_lw_enter_as(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        let x60 = reg(self.seak_attr_f32(f, attr::X60));
        let x64 = reg(self.seak_attr_f32(f, attr::X64));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x60));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), x64));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), x60));
        let j = self.bone_joint(gobj, PART_TOP_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let third = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 3.0)));
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, 0x78, 0.4, 0.003, third);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::LW_FN_80114034));
    }

    #[doc(alias = "ftSk_SpecialLw_Enter")]
    pub fn seak_special_lw_enter(&mut self, gobj: GObjId) {
        self.seak_special_lw_enter_as(gobj, ms::SPECIAL_LW);
    }

    #[doc(alias = "ftSk_SpecialAirLw_Enter")]
    pub fn seak_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.seak_special_lw_enter_as(gobj, ms::SPECIAL_AIR_LW);
    }

    /// The first half's end, `ftSk_SpecialLw_Anim` and `ftSk_SpecialAirLw_Anim`: the swap
    /// next.
    #[doc(alias = "ftSk_SpecialLw_Anim")]
    #[doc(alias = "ftSk_SpecialAirLw_Anim")]
    pub fn seak_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_FN_8011412C));
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftSk_SpecialLw_IASA")]
    #[doc(alias = "ftSk_SpecialAirLw_IASA")]
    #[doc(alias = "ftSk_SpecialLw2_IASA")]
    #[doc(alias = "ftSk_SpecialAirLw2_IASA")]
    pub fn seak_special_lw_no_iasa(&mut self, _gobj: GObjId) {}

    /// The aerial halves' fall (`x68`, `x6C`), slowing.
    #[doc(alias = "ftSk_SpecialAirLw_Phys")]
    #[doc(alias = "ftSk_SpecialAirLw2_Phys")]
    pub fn seak_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.seak_attr_f32(f, attr::X68));
        let t = reg(self.seak_attr_f32(f, attr::X6C));
        Self::fall(&mut self.game.fighters[f.0], g, t);
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftSk_SpecialLw_Coll`: off the ground, the aerial first half.
    #[doc(alias = "ftSk_SpecialLw_Coll")]
    pub fn seak_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.seak_special_lw_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirLw_Coll`: landing, the grounded first half.
    #[doc(alias = "ftSk_SpecialAirLw_Coll")]
    pub fn seak_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.seak_special_lw_to_ground(gobj);
        }
    }

    /// `ftSk_SpecialLw_8011444C`.
    #[doc(alias = "ftSk_SpecialLw_8011444C")]
    pub fn seak_special_lw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_FN_80114034));
    }

    /// `ftSk_SpecialLw_801144B8`.
    #[doc(alias = "ftSk_SpecialLw_801144B8")]
    pub fn seak_special_lw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_FN_80114034));
    }

    /// `ftSk_SpecialLw2_Anim`: standing once the second half ends.
    #[doc(alias = "ftSk_SpecialLw2_Anim")]
    pub fn seak_special_lw2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftSk_SpecialAirLw2_Anim`: falling once it ends.
    #[doc(alias = "ftSk_SpecialAirLw2_Anim")]
    pub fn seak_special_air_lw2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftSk_SpecialLw2_Coll`.
    #[doc(alias = "ftSk_SpecialLw2_Coll")]
    pub fn seak_special_lw2_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.seak_special_lw2_to_air(gobj);
        }
    }

    /// `ftSk_SpecialAirLw2_Coll`.
    #[doc(alias = "ftSk_SpecialAirLw2_Coll")]
    pub fn seak_special_air_lw2_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.seak_special_lw2_to_ground(gobj);
        }
    }

    /// `ftSk_SpecialLw_80114680`.
    #[doc(alias = "ftSk_SpecialLw_80114680")]
    pub fn seak_special_lw2_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW2, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_FN_801140B0));
    }

    /// `ftSk_SpecialLw_801146EC`.
    #[doc(alias = "ftSk_SpecialLw_801146EC")]
    pub fn seak_special_lw2_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW2, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_FN_801140B0));
    }

    /// The second half, after the swap, `ftSk_SpecialLw_80114758`: from frame `x70`.
    #[doc(alias = "ftSk_SpecialLw_80114758")]
    pub fn seak_special_lw_finish(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let msid = if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            ms::SPECIAL_LW2
        } else {
            ms::SPECIAL_AIR_LW2
        };
        let frame = self.seak_attr_f32(f, attr::X70);
        self.change_motion_state(gobj, msid, 0, frame, 1.0, 0.0, None);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::LW_FN_801140B0));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_FN_80114034 => g.seak_special_lw_start_gfx(gobj),
        addr::LW_FN_801140B0 => g.seak_special_lw_end_gfx(gobj),
        addr::LW_FN_8011412C => g.seak_special_lw_swap(gobj),
        addr::SPECIAL_LW_ENTER => g.seak_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.seak_special_air_lw_enter(gobj),
        0x8011_42E8 | 0x8011_4328 => g.seak_special_lw_anim(gobj),
        0x8011_4368 | 0x8011_436C | 0x8011_459C | 0x8011_45A0 => g.seak_special_lw_no_iasa(gobj),
        0x8011_4370 | 0x8011_45A4 => g.seak_ground_phys(gobj),
        0x8011_4390 | 0x8011_45C4 => g.seak_special_air_lw_phys(gobj),
        0x8011_43D4 => g.seak_special_lw_coll(gobj),
        0x8011_4410 => g.seak_special_air_lw_coll(gobj),
        0x8011_444C => g.seak_special_lw_to_air(gobj),
        0x8011_44B8 => g.seak_special_lw_to_ground(gobj),
        0x8011_4524 => g.seak_special_lw2_anim(gobj),
        0x8011_4560 => g.seak_special_air_lw2_anim(gobj),
        0x8011_4608 => g.seak_special_lw2_coll(gobj),
        0x8011_4644 => g.seak_special_air_lw2_coll(gobj),
        0x8011_4680 => g.seak_special_lw2_to_air(gobj),
        0x8011_46EC => g.seak_special_lw2_to_ground(gobj),
        addr::LW_80114758 => g.seak_special_lw_finish(gobj),
        _ => return false,
    }
    true
}
