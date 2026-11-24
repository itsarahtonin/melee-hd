//! The transformation between Zelda and Sheik: Zelda's down special (`ftzeldaspeciallw.c`)
//! and Sheik's (`ftseakspeciallw.c`), each her own copy of the same code. The first half's
//! slowdown and wind, the swap (`ftCommon_8007EFC8`, `ft::transform`), and the second half,
//! which the other's down special ends in. A [`Transform`] says whose, Zelda's [`ZELDA`] here
//! and Sheik's (`ft::seak::special_lw::SHEIK`), and each function here carries both originals'
//! names.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::{GA_GROUND, addr, ms};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// What tells one side of the transformation from the other.
pub struct Transform {
    /// The first of the move's four states: the first half and the second on the ground,
    /// then both in the air ([`SECOND`], [`AIR`]).
    pub(crate) states: i32,
    /// Where the move's attributes start in the character's: the speeds' divisors (x and y),
    /// the aerial halves' gravity and top speed, and the second half's first frame. Zelda's
    /// x70, Sheik's x60.
    pub(crate) attrs: usize,
    /// The part the first half's effect follows (Zelda's 104, Sheik's `FtPart_R2ndNb`).
    pub(crate) start_part: i32,
    /// The accessories: the first half's effect, the second half's, and the swap.
    pub(crate) start_cb: u32,
    pub(crate) end_cb: u32,
    pub(crate) swap_cb: u32,
    /// The other's second half, the callback the swap hands the new fighter.
    pub(crate) into: u32,
}

/// Zelda's own.
const ZELDA: Transform = Transform {
    states: ms::SPECIAL_LW,
    attrs: 0x70,
    start_part: 104,
    start_cb: addr::SPECIAL_LW_START_GFX,
    end_cb: addr::SPECIAL_LW_END_GFX,
    swap_cb: addr::SPECIAL_LW_SWAP,
    into: super::super::seak::addr::SPECIAL_LW_FINISH,
};

/// The four states from [`Transform::states`].
pub(crate) const SECOND: i32 = 1;
pub(crate) const AIR: i32 = 2;

/// The attributes from [`Transform::attrs`].
const DIV_X: usize = 0x0;
const DIV_Y: usize = 0x4;
const GRAVITY: usize = 0x8;
const TERMINAL_VEL: usize = 0xC;
const SECOND_FRAME: usize = 0x10;

/// `ftZd_MF_SpecialLw_Coll`, `ftSk_MF_SpecialLw_Coll` (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;

/// `FtPart_TopN`, `FtPart_HipN`: the code indexes `fp->parts` directly.
const PART_TOP_N: i32 = 0;
const PART_HIP_N: i32 = 4;

/// The first half's effect, the second half's (both characters').
const GFX_START: i32 = 1276;
const GFX_END: i32 = 1277;

impl Running<'_> {
    /// An effect on a part's joint, once (`x2219_b0`), then the hitlag hooks and no more
    /// accessory.
    fn transform_part_gfx(&mut self, gobj: GObjId, gfx: i32, part: i32) {
        if !self.game.fighter(gobj).effect_once {
            let j = self.bone_joint(gobj, part);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).effect_once = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    fn transform_attr_f32(&self, gobj: GObjId, t: &Transform, off: usize) -> f32 {
        self.char_attr_f32(self.game.fighter_id(gobj), t.attrs + off)
    }

    /// The first half's effect (an accessory): Zelda's `ftZd_SpecialLw_8013ADB4`, Sheik's
    /// `fn_80114034`.
    #[doc(alias = "ftZd_SpecialLw_8013ADB4")]
    #[doc(alias = "fn_80114034")]
    pub fn transform_start_gfx(&mut self, gobj: GObjId, t: &Transform) {
        self.transform_part_gfx(gobj, GFX_START, t.start_part);
    }

    /// The second half's effect (an accessory): Zelda's `ftZd_SpecialLw_8013AE30`, Sheik's
    /// `fn_801140B0`.
    #[doc(alias = "ftZd_SpecialLw_8013AE30")]
    #[doc(alias = "fn_801140B0")]
    pub fn transform_end_gfx(&mut self, gobj: GObjId) {
        self.transform_part_gfx(gobj, GFX_END, PART_HIP_N);
    }

    /// The swap (an accessory), `ftCommon_8007EFC8` with the other's second half: Zelda's
    /// `ftZd_SpecialLw_8013AEAC`, Sheik's `fn_8011412C`.
    #[doc(alias = "ftZd_SpecialLw_8013AEAC")]
    #[doc(alias = "fn_8011412C")]
    pub fn transform_swap(&mut self, gobj: GObjId, t: &Transform) {
        self.game.fighter_mut(gobj).accessory4_cb = None;
        self.transform(gobj, CodeRef(t.into));
    }

    /// The first half: the speeds divided down (`DIV_X`, `DIV_Y`), and a wind at the top.
    fn transform_enter_as(&mut self, gobj: GObjId, t: &Transform, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        let dx = reg(self.transform_attr_f32(gobj, t, DIV_X));
        let dy = reg(self.transform_attr_f32(gobj, t, DIV_Y));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), dx));
        fp.self_vel.y = mem(fp::fdivs(reg(fp.self_vel.y), dy));
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), dx));
        let j = self.bone_joint(gobj, PART_TOP_N);
        let pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let third = mem(fp::frsp(fp::fdiv(std::f64::consts::PI, 3.0)));
        crate::lb::wind::blast(self.game.gr.winds_mut(), pos, 0x78, 0.4, 0.003, third);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// The first half on the ground: `ftZd_SpecialLw_Enter`, `ftSk_SpecialLw_Enter`.
    #[doc(alias = "ftZd_SpecialLw_Enter")]
    #[doc(alias = "ftSk_SpecialLw_Enter")]
    pub fn transform_enter(&mut self, gobj: GObjId, t: &Transform) {
        self.transform_enter_as(gobj, t, t.states);
    }

    /// The first half in the air: `ftZd_SpecialAirLw_Enter`, `ftSk_SpecialAirLw_Enter`.
    #[doc(alias = "ftZd_SpecialAirLw_Enter")]
    #[doc(alias = "ftSk_SpecialAirLw_Enter")]
    pub fn transform_air_enter(&mut self, gobj: GObjId, t: &Transform) {
        self.transform_enter_as(gobj, t, t.states + AIR);
    }

    /// The first half's end, the swap next: `ftZd_SpecialLw_Anim`, `ftZd_SpecialAirLw_Anim`,
    /// `ftSk_SpecialLw_Anim`, `ftSk_SpecialAirLw_Anim`.
    #[doc(alias = "ftZd_SpecialLw_Anim")]
    #[doc(alias = "ftZd_SpecialAirLw_Anim")]
    #[doc(alias = "ftSk_SpecialLw_Anim")]
    #[doc(alias = "ftSk_SpecialAirLw_Anim")]
    pub fn transform_anim(&mut self, gobj: GObjId, t: &Transform) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.swap_cb));
        }
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftZd_SpecialLw_IASA")]
    #[doc(alias = "ftZd_SpecialAirLw_IASA")]
    #[doc(alias = "ftZd_SpecialLw2_IASA")]
    #[doc(alias = "ftZd_SpecialAirLw2_IASA")]
    #[doc(alias = "ftSk_SpecialLw_IASA")]
    #[doc(alias = "ftSk_SpecialAirLw_IASA")]
    #[doc(alias = "ftSk_SpecialLw2_IASA")]
    #[doc(alias = "ftSk_SpecialAirLw2_IASA")]
    pub fn transform_no_iasa(&mut self, _gobj: GObjId) {}

    /// The aerial halves' fall (`GRAVITY`, `TERMINAL_VEL`), slowing.
    #[doc(alias = "ftZd_SpecialAirLw_Phys")]
    #[doc(alias = "ftZd_SpecialAirLw2_Phys")]
    #[doc(alias = "ftSk_SpecialAirLw_Phys")]
    #[doc(alias = "ftSk_SpecialAirLw2_Phys")]
    pub fn transform_air_phys(&mut self, gobj: GObjId, t: &Transform) {
        let f = self.game.fighter_id(gobj);
        let g = reg(self.transform_attr_f32(gobj, t, GRAVITY));
        let v = reg(self.transform_attr_f32(gobj, t, TERMINAL_VEL));
        Self::fall(&mut self.game.fighters[f.0], g, v);
        Self::self_accel_deaccel_air(&mut self.game.fighters[f.0]);
    }

    /// Off the ground, the aerial first half: `ftZd_SpecialLw_Coll`, `ftSk_SpecialLw_Coll`.
    #[doc(alias = "ftZd_SpecialLw_Coll")]
    #[doc(alias = "ftSk_SpecialLw_Coll")]
    pub fn transform_coll(&mut self, gobj: GObjId, t: &Transform) {
        if !self.ground_coll_off_ledges(gobj) {
            self.transform_to_air(gobj, t);
        }
    }

    /// Landing, the grounded first half: `ftZd_SpecialAirLw_Coll`, `ftSk_SpecialAirLw_Coll`.
    #[doc(alias = "ftZd_SpecialAirLw_Coll")]
    #[doc(alias = "ftSk_SpecialAirLw_Coll")]
    pub fn transform_air_coll(&mut self, gobj: GObjId, t: &Transform) {
        if self.air_coll_land(gobj) {
            self.transform_to_ground(gobj, t);
        }
    }

    /// The first half off the ground: `ftZd_SpecialLw_8013B1CC`, `ftSk_SpecialLw_8011444C`.
    #[doc(alias = "ftZd_SpecialLw_8013B1CC")]
    #[doc(alias = "ftSk_SpecialLw_8011444C")]
    pub fn transform_to_air(&mut self, gobj: GObjId, t: &Transform) {
        self.fox_ground_to_air(gobj, t.states + AIR, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// The first half onto the ground: `ftZd_SpecialLw_8013B238`, `ftSk_SpecialLw_801144B8`.
    #[doc(alias = "ftZd_SpecialLw_8013B238")]
    #[doc(alias = "ftSk_SpecialLw_801144B8")]
    pub fn transform_to_ground(&mut self, gobj: GObjId, t: &Transform) {
        self.fox_air_to_ground(gobj, t.states, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.start_cb));
    }

    /// Standing once the second half ends: `ftZd_SpecialLw2_Anim`, `ftSk_SpecialLw2_Anim`.
    #[doc(alias = "ftZd_SpecialLw2_Anim")]
    #[doc(alias = "ftSk_SpecialLw2_Anim")]
    pub fn transform_second_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// Falling once it ends: `ftZd_SpecialAirLw2_Anim`, `ftSk_SpecialAirLw2_Anim`.
    #[doc(alias = "ftZd_SpecialAirLw2_Anim")]
    #[doc(alias = "ftSk_SpecialAirLw2_Anim")]
    pub fn transform_air_second_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftZd_SpecialLw2_Coll`, `ftSk_SpecialLw2_Coll`.
    #[doc(alias = "ftZd_SpecialLw2_Coll")]
    #[doc(alias = "ftSk_SpecialLw2_Coll")]
    pub fn transform_second_coll(&mut self, gobj: GObjId, t: &Transform) {
        if !self.ground_coll_off_ledges(gobj) {
            self.transform_second_to_air(gobj, t);
        }
    }

    /// `ftZd_SpecialAirLw2_Coll`, `ftSk_SpecialAirLw2_Coll`.
    #[doc(alias = "ftZd_SpecialAirLw2_Coll")]
    #[doc(alias = "ftSk_SpecialAirLw2_Coll")]
    pub fn transform_air_second_coll(&mut self, gobj: GObjId, t: &Transform) {
        if self.air_coll_land(gobj) {
            self.transform_second_to_ground(gobj, t);
        }
    }

    /// The second half off the ground: `ftZd_SpecialLw_8013B400`, `ftSk_SpecialLw_80114680`.
    #[doc(alias = "ftZd_SpecialLw_8013B400")]
    #[doc(alias = "ftSk_SpecialLw_80114680")]
    pub fn transform_second_to_air(&mut self, gobj: GObjId, t: &Transform) {
        self.fox_ground_to_air(gobj, t.states + AIR + SECOND, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.end_cb));
    }

    /// The second half onto the ground: `ftZd_SpecialLw_8013B46C`, `ftSk_SpecialLw_801146EC`.
    #[doc(alias = "ftZd_SpecialLw_8013B46C")]
    #[doc(alias = "ftSk_SpecialLw_801146EC")]
    pub fn transform_second_to_ground(&mut self, gobj: GObjId, t: &Transform) {
        self.fox_air_to_ground(gobj, t.states + SECOND, COLL_FLAGS);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.end_cb));
    }

    /// The second half, after the swap, from frame `SECOND_FRAME`: Zelda's
    /// `ftZd_SpecialLw_8013B4D8`, Sheik's `ftSk_SpecialLw_80114758`.
    #[doc(alias = "ftZd_SpecialLw_8013B4D8")]
    #[doc(alias = "ftSk_SpecialLw_80114758")]
    pub fn transform_finish(&mut self, gobj: GObjId, t: &Transform) {
        let msid = if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            t.states + SECOND
        } else {
            t.states + AIR + SECOND
        };
        let frame = self.transform_attr_f32(gobj, t, SECOND_FRAME);
        self.change_motion_state(gobj, msid, 0, frame, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(t.end_cb));
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    let t = &ZELDA;
    match a {
        addr::SPECIAL_LW_START_GFX => g.transform_start_gfx(gobj, t),
        addr::SPECIAL_LW_END_GFX => g.transform_end_gfx(gobj),
        addr::SPECIAL_LW_SWAP => g.transform_swap(gobj, t),
        addr::SPECIAL_LW_ENTER => g.transform_enter(gobj, t),
        addr::SPECIAL_AIR_LW_ENTER => g.transform_air_enter(gobj, t),
        0x8013_B068 | 0x8013_B0A8 => g.transform_anim(gobj, t),
        0x8013_B0E8 | 0x8013_B0EC | 0x8013_B31C | 0x8013_B320 => g.transform_no_iasa(gobj),
        0x8013_B0F0 | 0x8013_B324 => g.seak_ground_phys(gobj),
        0x8013_B110 | 0x8013_B344 => g.transform_air_phys(gobj, t),
        0x8013_B154 => g.transform_coll(gobj, t),
        0x8013_B190 => g.transform_air_coll(gobj, t),
        0x8013_B1CC => g.transform_to_air(gobj, t),
        0x8013_B238 => g.transform_to_ground(gobj, t),
        0x8013_B2A4 => g.transform_second_anim(gobj),
        0x8013_B2E0 => g.transform_air_second_anim(gobj),
        0x8013_B388 => g.transform_second_coll(gobj, t),
        0x8013_B3C4 => g.transform_air_second_coll(gobj, t),
        0x8013_B400 => g.transform_second_to_air(gobj, t),
        0x8013_B46C => g.transform_second_to_ground(gobj, t),
        addr::SPECIAL_LW_FINISH => g.transform_finish(gobj, t),
        _ => return false,
    }
    true
}
