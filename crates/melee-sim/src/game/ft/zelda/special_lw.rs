//! The transformation into Sheik (`ftzeldaspeciallw.c`): the first half's slowdown and wind,
//! the swap (`ftCommon_8007EFC8`), and the second half, which Sheik's own down special also
//! ends in; and Din's Fire's hooks on the character (`ftZd_SpecialLw_8013B540` on).

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{GA_GROUND, addr, attr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftZd_MF_SpecialLw_Coll`, as Sheik's (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;

/// `FtPart_TopN`, `FtPart_HipN`, and the part the first half's effect follows: Zelda's code
/// indexes `fp->parts` directly.
const PART_TOP_N: i32 = 0;
const PART_HIP_N: i32 = 4;
const PART_START_GFX: i32 = 104;

/// The first half's effect, the second half's.
const GFX_START: i32 = 1276;
const GFX_END: i32 = 1277;

impl Running<'_> {
    /// An effect on a part's joint, once (`x2219_b0`), then the hitlag hooks and no more
    /// accessory.
    fn zelda_part_gfx(&mut self, gobj: GObjId, gfx: i32, part: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, part);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// The first half's effect, `ftZd_SpecialLw_8013ADB4` (an accessory).
    // Ports `ftZd_SpecialLw_8013ADB4`; aliased once a check reaches it.
    pub fn zelda_special_lw_start_gfx(&mut self, gobj: GObjId) {
        self.zelda_part_gfx(gobj, GFX_START, PART_START_GFX);
    }

    /// The second half's effect, `ftZd_SpecialLw_8013AE30` (an accessory).
    // Ports `ftZd_SpecialLw_8013AE30`; aliased once a check reaches it.
    pub fn zelda_special_lw_end_gfx(&mut self, gobj: GObjId) {
        self.zelda_part_gfx(gobj, GFX_END, PART_HIP_N);
    }

    /// The swap, `ftZd_SpecialLw_8013AEAC` (an accessory): `ftCommon_8007EFC8` with Sheik's
    /// second half.
    // Ports `ftZd_SpecialLw_8013AEAC`; aliased once a check reaches it.
    pub fn zelda_special_lw_swap(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).accessory4_cb = None;
        self.transform(gobj, CodeRef(super::super::seak::addr::LW_80114758));
    }

    /// The first half, `ftZd_SpecialLw_Enter` and `ftZd_SpecialAirLw_Enter`: the speeds
    /// divided down (`x70`, `x74`), and a wind at the top.
    fn zelda_special_lw_enter_as(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        let x70 = reg(self.zelda_attr_f32(f, attr::X70));
        let x74 = reg(self.zelda_attr_f32(f, attr::X74));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x70));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), x74));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), x70));
        let j = self.bone_joint(gobj, PART_TOP_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let third = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 3.0)));
        crate::lb::wind::blast(&mut self.game.gr.winds, pos, 0x78, 0.4, 0.003, third);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::LW_8013ADB4));
    }

    // Ports `ftZd_SpecialLw_Enter`; aliased once a check reaches it.
    pub fn zelda_special_lw_enter(&mut self, gobj: GObjId) {
        self.zelda_special_lw_enter_as(gobj, ms::SPECIAL_LW);
    }

    // Ports `ftZd_SpecialAirLw_Enter`; aliased once a check reaches it.
    pub fn zelda_special_air_lw_enter(&mut self, gobj: GObjId) {
        self.zelda_special_lw_enter_as(gobj, ms::SPECIAL_AIR_LW);
    }

    /// The first half's end, `ftZd_SpecialLw_Anim` and `ftZd_SpecialAirLw_Anim`: the swap
    /// next.
    // Ports `ftZd_SpecialLw_Anim`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirLw_Anim`; aliased once a check reaches it.
    pub fn zelda_special_lw_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_8013AEAC));
        }
    }

    /// The states' empty IASA callbacks.
    // Ports `ftZd_SpecialLw_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirLw_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialLw2_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirLw2_IASA`; aliased once a check reaches it.
    pub fn zelda_no_iasa(&mut self, _gobj: GObjId) {}

    /// The aerial halves' fall (`x78`, `x7C`), slowing.
    // Ports `ftZd_SpecialAirLw_Phys`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirLw2_Phys`; aliased once a check reaches it.
    pub fn zelda_special_air_lw_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.zelda_attr_f32(f, attr::X78));
        let t = reg(self.zelda_attr_f32(f, attr::X7C));
        Self::fall(&mut self.game.fighters[f.0], g, t);
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// `ftZd_SpecialLw_Coll`: off the ground, the aerial first half.
    // Ports `ftZd_SpecialLw_Coll`; aliased once a check reaches it.
    pub fn zelda_special_lw_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.zelda_special_lw_to_air(gobj);
        }
    }

    /// `ftZd_SpecialAirLw_Coll`: landing, the grounded first half.
    // Ports `ftZd_SpecialAirLw_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_lw_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.zelda_special_lw_to_ground(gobj);
        }
    }

    /// `ftZd_SpecialLw_8013B1CC`.
    // Ports `ftZd_SpecialLw_8013B1CC`; aliased once a check reaches it.
    pub fn zelda_special_lw_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_8013ADB4));
    }

    /// `ftZd_SpecialLw_8013B238`.
    // Ports `ftZd_SpecialLw_8013B238`; aliased once a check reaches it.
    pub fn zelda_special_lw_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_8013ADB4));
    }

    /// `ftZd_SpecialLw2_Anim`: standing once the second half ends.
    // Ports `ftZd_SpecialLw2_Anim`; aliased once a check reaches it.
    pub fn zelda_special_lw2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftZd_SpecialAirLw2_Anim`: falling once it ends.
    // Ports `ftZd_SpecialAirLw2_Anim`; aliased once a check reaches it.
    pub fn zelda_special_air_lw2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftZd_SpecialLw2_Coll`.
    // Ports `ftZd_SpecialLw2_Coll`; aliased once a check reaches it.
    pub fn zelda_special_lw2_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.zelda_special_lw2_to_air(gobj);
        }
    }

    /// `ftZd_SpecialAirLw2_Coll`.
    // Ports `ftZd_SpecialAirLw2_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_lw2_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.zelda_special_lw2_to_ground(gobj);
        }
    }

    /// `ftZd_SpecialLw_8013B400`.
    // Ports `ftZd_SpecialLw_8013B400`; aliased once a check reaches it.
    pub fn zelda_special_lw2_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_LW2, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_8013AE30));
    }

    /// `ftZd_SpecialLw_8013B46C`.
    // Ports `ftZd_SpecialLw_8013B46C`; aliased once a check reaches it.
    pub fn zelda_special_lw2_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_LW2, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::LW_8013AE30));
    }

    /// The second half, after the swap, `ftZd_SpecialLw_8013B4D8`: from frame `x80`.
    #[doc(alias = "ftZd_SpecialLw_8013B4D8")]
    pub fn zelda_special_lw_finish(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let msid = if self.game.fighters[f.0].ground_or_air == GA_GROUND {
            ms::SPECIAL_LW2
        } else {
            ms::SPECIAL_AIR_LW2
        };
        let frame = self.zelda_attr_f32(f, attr::X80);
        self.change_motion_state(gobj, msid, 0, frame, 1.0, 0.0, None);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::LW_8013AE30));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::LW_8013ADB4 => g.zelda_special_lw_start_gfx(gobj),
        addr::LW_8013AE30 => g.zelda_special_lw_end_gfx(gobj),
        addr::LW_8013AEAC => g.zelda_special_lw_swap(gobj),
        addr::SPECIAL_LW_ENTER => g.zelda_special_lw_enter(gobj),
        addr::SPECIAL_AIR_LW_ENTER => g.zelda_special_air_lw_enter(gobj),
        0x8013_B068 | 0x8013_B0A8 => g.zelda_special_lw_anim(gobj),
        0x8013_B0E8 | 0x8013_B0EC | 0x8013_B31C | 0x8013_B320 => g.zelda_no_iasa(gobj),
        0x8013_B0F0 | 0x8013_B324 => g.seak_ground_phys(gobj),
        0x8013_B110 | 0x8013_B344 => g.zelda_special_air_lw_phys(gobj),
        0x8013_B154 => g.zelda_special_lw_coll(gobj),
        0x8013_B190 => g.zelda_special_air_lw_coll(gobj),
        0x8013_B1CC => g.zelda_special_lw_to_air(gobj),
        0x8013_B238 => g.zelda_special_lw_to_ground(gobj),
        0x8013_B2A4 => g.zelda_special_lw2_anim(gobj),
        0x8013_B2E0 => g.zelda_special_air_lw2_anim(gobj),
        0x8013_B388 => g.zelda_special_lw2_coll(gobj),
        0x8013_B3C4 => g.zelda_special_air_lw2_coll(gobj),
        0x8013_B400 => g.zelda_special_lw2_to_air(gobj),
        0x8013_B46C => g.zelda_special_lw2_to_ground(gobj),
        addr::LW_8013B4D8 => g.zelda_special_lw_finish(gobj),
        _ => return false,
    }
    true
}
