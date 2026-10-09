//! Din's Fire on Zelda's side (`ftzeldaspecials.c`, and its hooks in `ftzeldaspeciallw.c` and
//! `ftzelda.c`): the cast, which makes the fire when the script says (`cmd_vars[0]`), the
//! hold that steers it while B stays down, and the end; the fire is the fighter's `u.zd.x222C`
//! while it's hers, with the damage and death hooks that let it go.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::zeldadinfire;
use super::super::super::state::Running;
use super::super::motion::flags::*;
use super::super::vars::ZELDA_DIN_FIRE;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `ftCommon_GroundAirColl_MF`, and `ftZd_MF_SpecialSStart_Coll` (with `Ft_MF_KeepGfx`).
const COLL_FLAGS: u32 = SKIP_MAT_ANIM
    | SKIP_COL_ANIM
    | UPDATE_CMD
    | SKIP_ITEM_VIS
    | UNK_19
    | SKIP_MODEL_PART_VIS
    | SKIP_MODEL_FLAGS
    | UNK_27;
const START_COLL_FLAGS: u32 = COLL_FLAGS | KEEP_GFX;

/// The parts the fire starts at and the cast's effect is on: Zelda's code indexes `fp->parts`
/// directly.
const PART_FIRE: i32 = 89;
const PART_GFX: i32 = 76;
const GFX_CAST: i32 = 1275;

/// `HSD_PAD_B`.
const PAD_B: u32 = 1 << 9;

impl Running<'_> {
    /// Zelda's fire (`u.zd.x222C`).
    pub fn zelda_din_fire(&self, gobj: GObjId) -> Option<GObjId> {
        self.game.fighter(gobj).u.gobj_at(ZELDA_DIN_FIRE)
    }

    fn set_zelda_din_fire(&mut self, gobj: GObjId, g: Option<GObjId>) {
        self.game.fighter_mut(gobj).u.set_gobj_at(ZELDA_DIN_FIRE, g);
    }

    /// Whether Zelda still holds the fire, `ftZd_SpecialLw_8013B540`: in the hold's states,
    /// with a fire out.
    #[doc(alias = "ftZd_SpecialLw_8013B540")]
    pub fn zelda_din_fire_held(&self, gobj: GObjId) -> bool {
        let m = self.game.fighter(gobj).motion_id;
        (m == ms::SPECIAL_S_LOOP || m == ms::SPECIAL_AIR_S_LOOP)
            && self.zelda_din_fire(gobj).is_some()
    }

    /// Whether Zelda let the fire go this frame, `ftZd_SpecialLw_8013B574`: in the end's states,
    /// the script's release (`cmd_vars[1]`), taken.
    #[doc(alias = "ftZd_SpecialLw_8013B574")]
    pub fn zelda_din_fire_released(&mut self, gobj: GObjId) -> bool {
        if self.zelda_din_fire(gobj).is_none() {
            return false;
        }
        let fp = self.game.fighter_mut(gobj);
        if (fp.motion_id == ms::SPECIAL_AIR_S_END || fp.motion_id == ms::SPECIAL_S_END)
            && fp.cmd_vars[1] == 1
        {
            fp.cmd_vars[1] = 0;
            return true;
        }
        false
    }

    /// Forgets the fire, `ftZd_SpecialLw_8013B5C4`: and the hooks that would let it go.
    #[doc(alias = "ftZd_SpecialLw_8013B5C4")]
    pub fn zelda_forget_din_fire(&mut self, gobj: GObjId) {
        if self.zelda_din_fire(gobj).is_some() {
            self.set_zelda_din_fire(gobj, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Lets the fire go, `ftZd_SpecialLw_8013B5EC` (`it_802C3D44`), and forgets it.
    #[doc(alias = "ftZd_SpecialLw_8013B5EC")]
    pub fn zelda_let_go_din_fire(&mut self, gobj: GObjId) {
        if let Some(fire) = self.zelda_din_fire(gobj) {
            zeldadinfire::let_go(self, Some(fire));
            self.set_zelda_din_fire(gobj, None);
        }
        let fp = self.game.fighter_mut(gobj);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// Zelda's damage and death hook while the fire is out, `ftZd_Init_801393AC`: the fire let
    /// go.
    #[doc(alias = "ftZd_Init_801393AC")]
    pub fn zelda_damage_cb(&mut self, gobj: GObjId) {
        self.zelda_let_go_din_fire(gobj);
    }

    /// The cast's start: the cmd vars clear, the hold's timers (`x10`, `x14`, `x18`, `x1C`),
    /// no fire yet.
    fn zelda_special_s_reset(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let [a, b, c, d] = [0x10, 0x14, 0x18, 0x1C].map(|o| self.zelda_attr_i32(f, o));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::S_X0, a);
        fp.set_mv_i32(mv::S_X4, b);
        fp.set_mv_i32(mv::S_X8, c);
        fp.u.set_gobj_at(ZELDA_DIN_FIRE, None);
        fp.set_mv_i32(mv::S_XC, d);
        fp.death2_cb = None;
        fp.take_dmg_cb = None;
    }

    /// `ftZd_SpecialS_Enter`.
    #[doc(alias = "ftZd_SpecialS_Enter")]
    pub fn zelda_special_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).cmd_vars = [0; 4];
        self.zelda_special_s_reset(gobj);
        self.anim_update(gobj);
    }

    /// `ftZd_SpecialAirS_Enter`: the fall stopped.
    #[doc(alias = "ftZd_SpecialAirS_Enter")]
    pub fn zelda_special_air_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars = [0; 4];
        fp.self_vel.y = 0.0;
        self.zelda_special_s_reset(gobj);
        self.anim_update(gobj);
    }

    /// The fire made when the script says (`cmd_vars[0]`) and none is out, at the hand plus
    /// `x20` ahead and `x24` up, with the hooks that let it go and the cast's effect.
    fn zelda_special_s_cast(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] != 1 || self.zelda_din_fire(gobj).is_some() {
            return;
        }
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[0] = 0;
        let j = self.bone_joint(gobj, PART_FIRE);
        let mut pos = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        pos.z = 0.0;
        let x20 = self.zelda_attr_f32(f, 0x20);
        let facing = self.game.fighters[f.0].facing_dir;
        pos.x = mem(fp::fmadds(reg(x20), reg(facing), reg(pos.x)));
        pos.y = mem(fp::fadds(reg(pos.y), reg(self.zelda_attr_f32(f, 0x24))));
        let fire = zeldadinfire::spawn(self, gobj, pos, facing);
        self.set_zelda_din_fire(gobj, fire);
        if fire.is_some() {
            let fp = &mut self.game.fighters[f.0];
            fp.death2_cb = Some(CodeRef(addr::INIT_801393AC));
            fp.take_dmg_cb = Some(CodeRef(addr::INIT_801393AC));
        }
        let j = self.bone_joint(gobj, PART_GFX);
        spawn(self, GFX_CAST, Some(gobj), &[Arg::JObj(j)]);
    }

    /// The cast, `ftZd_SpecialSStart_Anim` and `ftZd_SpecialAirSStart_Anim`: the hold once it
    /// ends, the jumps used up.
    fn zelda_special_s_start_anim(&mut self, gobj: GObjId, next: i32) {
        self.zelda_special_s_cast(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.change_motion_state(gobj, next, 0, 0.0, 1.0, 0.0, None);
            let fp = self.game.fighter_mut(gobj);
            fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
        }
    }

    #[doc(alias = "ftZd_SpecialSStart_Anim")]
    pub fn zelda_special_s_start_anim_ground(&mut self, gobj: GObjId) {
        self.zelda_special_s_start_anim(gobj, ms::SPECIAL_S_LOOP);
    }

    #[doc(alias = "ftZd_SpecialAirSStart_Anim")]
    pub fn zelda_special_air_s_start_anim(&mut self, gobj: GObjId) {
        self.zelda_special_s_start_anim(gobj, ms::SPECIAL_AIR_S_LOOP);
    }

    /// The hold, `ftZd_SpecialSLoop_Anim` and `ftZd_SpecialAirSLoop_Anim`: the timers count
    /// down; with no fire, the end once both ran out; a fire no longer hers forgotten.
    fn zelda_special_s_loop_anim(&mut self, gobj: GObjId, end: i32) {
        self.zelda_special_s_cast(gobj);
        let fp = self.game.fighter_mut(gobj);
        for o in [mv::S_X0, mv::S_X4] {
            let v = fp.mv_i32(o);
            if v != 0 {
                fp.set_mv_i32(o, v.wrapping_sub(1));
            }
        }
        match self.zelda_din_fire(gobj) {
            None => {
                let fp = self.game.fighter(gobj);
                if fp.mv_i32(mv::S_X0) <= 0 && fp.mv_i32(mv::S_X4) <= 0 {
                    self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
                }
            }
            Some(fire) => {
                if zeldadinfire::owner(self, fire) != Some(gobj) {
                    self.set_zelda_din_fire(gobj, None);
                }
            }
        }
    }

    #[doc(alias = "ftZd_SpecialSLoop_Anim")]
    pub fn zelda_special_s_loop_anim_ground(&mut self, gobj: GObjId) {
        self.zelda_special_s_loop_anim(gobj, ms::SPECIAL_S_END);
    }

    #[doc(alias = "ftZd_SpecialAirSLoop_Anim")]
    pub fn zelda_special_air_s_loop_anim(&mut self, gobj: GObjId) {
        self.zelda_special_s_loop_anim(gobj, ms::SPECIAL_AIR_S_END);
    }

    /// `ftZd_SpecialSEnd_Anim`: the arm's turn undone; standing once it ends, the cast's state
    /// reset.
    #[doc(alias = "ftZd_SpecialSEnd_Anim")]
    pub fn zelda_special_s_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if !self.anim_frames_remaining(gobj) {
            self.zelda_special_s_reset(gobj);
            self.enter_wait(gobj);
        }
    }

    /// `ftZd_SpecialAirSEnd_Anim`: as on the ground, then falling (helplessly when `x34`
    /// says, with that landing lag).
    #[doc(alias = "ftZd_SpecialAirSEnd_Anim")]
    pub fn zelda_special_air_s_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.part_set_rot_x(f, 0, 0.0);
        if !self.anim_frames_remaining(gobj) {
            self.zelda_special_s_reset(gobj);
            let x34 = self.zelda_attr_f32(f, attr::X34);
            if x34 == 0.0 {
                self.enter_fall(gobj);
            } else {
                self.fall_special(gobj, 1.0, x34);
            }
        }
    }

    /// The hold's input, `ftZd_SpecialSLoop_IASA` and `ftZd_SpecialAirSLoop_IASA`: once its
    /// least frames (`specials.xC`) pass, letting go of B ends it.
    fn zelda_special_s_loop_iasa(&mut self, gobj: GObjId, end: i32) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::S_XC).wrapping_sub(1);
        fp.set_mv_i32(mv::S_XC, n);
        let done = if n <= 0 {
            fp.set_mv_i32(mv::S_XC, 0);
            true
        } else {
            false
        };
        if done && fp.input.held_buttons[0] & PAD_B == 0 {
            self.change_motion_state(gobj, end, 0, 0.0, 1.0, 0.0, None);
        }
    }

    #[doc(alias = "ftZd_SpecialSLoop_IASA")]
    pub fn zelda_special_s_loop_iasa_ground(&mut self, gobj: GObjId) {
        self.zelda_special_s_loop_iasa(gobj, ms::SPECIAL_S_END);
    }

    #[doc(alias = "ftZd_SpecialAirSLoop_IASA")]
    pub fn zelda_special_air_s_loop_iasa(&mut self, gobj: GObjId) {
        self.zelda_special_s_loop_iasa(gobj, ms::SPECIAL_AIR_S_END);
    }

    /// The states' empty IASA callbacks.
    #[doc(alias = "ftZd_SpecialSStart_IASA")]
    #[doc(alias = "ftZd_SpecialSEnd_IASA")]
    #[doc(alias = "ftZd_SpecialAirSStart_IASA")]
    #[doc(alias = "ftZd_SpecialAirSEnd_IASA")]
    pub fn zelda_special_s_no_iasa(&mut self, _gobj: GObjId) {}

    /// `ftZd_SpecialSStart_Phys`: the hover's count (`specials.x8`) down, friction.
    #[doc(alias = "ftZd_SpecialSStart_Phys")]
    pub fn zelda_special_s_start_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let v = fp.mv_i32(mv::S_X8);
        if v != 0 {
            fp.set_mv_i32(mv::S_X8, v.wrapping_sub(1));
        }
        self.ground_friction(gobj);
    }

    /// The aerial states' physics, `ftZd_SpecialAirSStart_Phys`, `_Loop_Phys`, `_End_Phys`:
    /// hovering while `specials.x8` counts down, then falling (`x2C`); air friction.
    #[doc(alias = "ftZd_SpecialAirSStart_Phys")]
    #[doc(alias = "ftZd_SpecialAirSLoop_Phys")]
    #[doc(alias = "ftZd_SpecialAirSEnd_Phys")]
    pub fn zelda_special_air_s_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let v = self.game.fighters[f.0].mv_i32(mv::S_X8);
        if v != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::S_X8, v - 1);
        } else {
            let g = reg(self.zelda_attr_f32(f, attr::X2C));
            let t = reg(self.game.fighters[f.0].co_attrs.terminal_velocity);
            Self::fall(&mut self.game.fighters[f.0], g, t);
        }
        let fp = &mut self.game.fighters[f.0];
        let friction = reg(fp.co_attrs.aerial_friction);
        Self::self_accel_deaccel(fp, friction);
    }

    /// The grounded states' collision: off the ground, the aerial one.
    fn zelda_special_s_coll(&mut self, gobj: GObjId, air: i32, flags: u32) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_ground_to_air(gobj, air, flags);
        }
    }

    /// The aerial states' collision: landing, the grounded one.
    fn zelda_special_air_s_coll(&mut self, gobj: GObjId, ground: i32, flags: u32) {
        if self.air_coll_land(gobj) {
            self.fox_air_to_ground(gobj, ground, flags);
        }
    }

    #[doc(alias = "ftZd_SpecialSStart_Coll")]
    pub fn zelda_special_s_start_coll(&mut self, gobj: GObjId) {
        self.zelda_special_s_coll(gobj, ms::SPECIAL_AIR_S_START, START_COLL_FLAGS);
    }

    #[doc(alias = "ftZd_SpecialSLoop_Coll")]
    pub fn zelda_special_s_loop_coll(&mut self, gobj: GObjId) {
        self.zelda_special_s_coll(gobj, ms::SPECIAL_AIR_S_LOOP, COLL_FLAGS);
    }

    #[doc(alias = "ftZd_SpecialSEnd_Coll")]
    pub fn zelda_special_s_end_coll(&mut self, gobj: GObjId) {
        self.zelda_special_s_coll(gobj, ms::SPECIAL_AIR_S_END, COLL_FLAGS);
    }

    #[doc(alias = "ftZd_SpecialAirSStart_Coll")]
    pub fn zelda_special_air_s_start_coll(&mut self, gobj: GObjId) {
        self.zelda_special_air_s_coll(gobj, ms::SPECIAL_S_START, START_COLL_FLAGS);
    }

    #[doc(alias = "ftZd_SpecialAirSLoop_Coll")]
    pub fn zelda_special_air_s_loop_coll(&mut self, gobj: GObjId) {
        self.zelda_special_air_s_coll(gobj, ms::SPECIAL_S_LOOP, COLL_FLAGS);
    }

    #[doc(alias = "ftZd_SpecialAirSEnd_Coll")]
    pub fn zelda_special_air_s_end_coll(&mut self, gobj: GObjId) {
        self.zelda_special_air_s_coll(gobj, ms::SPECIAL_S_END, COLL_FLAGS);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_ENTER => g.zelda_special_s_enter(gobj),
        addr::SPECIAL_AIR_S_ENTER => g.zelda_special_air_s_enter(gobj),
        addr::INIT_801393AC => g.zelda_damage_cb(gobj),
        0x8013_B5C4 => g.zelda_forget_din_fire(gobj),
        0x8013_B5EC => g.zelda_let_go_din_fire(gobj),
        0x8013_B780 => g.zelda_special_s_start_anim_ground(gobj),
        0x8013_B89C => g.zelda_special_s_loop_anim_ground(gobj),
        0x8013_BA04 => g.zelda_special_s_end_anim(gobj),
        0x8013_BA8C => g.zelda_special_air_s_start_anim(gobj),
        0x8013_BBA8 => g.zelda_special_air_s_loop_anim(gobj),
        0x8013_BD10 => g.zelda_special_air_s_end_anim(gobj),
        0x8013_BDD0 | 0x8013_BE50 | 0x8013_BE54 | 0x8013_BED4 => g.zelda_special_s_no_iasa(gobj),
        0x8013_BDD4 => g.zelda_special_s_loop_iasa_ground(gobj),
        0x8013_BE58 => g.zelda_special_air_s_loop_iasa(gobj),
        0x8013_BED8 => g.zelda_special_s_start_phys(gobj),
        0x8013_BF10 | 0x8013_BF30 => g.seak_ground_phys(gobj),
        0x8013_BF50 | 0x8013_BFB0 | 0x8013_C010 => g.zelda_special_air_s_phys(gobj),
        0x8013_C070 => g.zelda_special_s_start_coll(gobj),
        0x8013_C0DC => g.zelda_special_s_loop_coll(gobj),
        0x8013_C148 => g.zelda_special_s_end_coll(gobj),
        0x8013_C1B4 => g.zelda_special_air_s_start_coll(gobj),
        0x8013_C220 => g.zelda_special_air_s_loop_coll(gobj),
        0x8013_C28C => g.zelda_special_air_s_end_coll(gobj),
        _ => return false,
    }
    true
}
