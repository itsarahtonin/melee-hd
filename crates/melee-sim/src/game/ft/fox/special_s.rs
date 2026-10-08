//! Fox Illusion and Falco Phantasm (`ftfoxspecials.c`): the wind-up, the dash with its
//! afterimages, and the end, on the ground and in the air.

use gekko_fp as fp;

use super::super::super::code::Val;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::motion::flags::SKIP_RUMBLE;
use super::{GA_AIR, KIND_FOX, PAD_B, addr, attr, facing_dir_int, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftFx_MF_SpecialS_Coll` and `ftFx_MF_SpecialSDash_Coll`: the flags a state keeps going
/// between the ground and the air.
const COLL_FLAGS: u32 = 0x0C4C_5880;
const DASH_COLL_FLAGS: u32 = 0x0C4C_5884;

/// `It_Kind_Fox_Illusion`, `It_Kind_Falco_Phantasm`.
const IT_KIND_FOX_ILLUSION: i32 = 56;
const IT_KIND_FALCO_PHANTASM: i32 = 57;

/// `FtPart_TopN`.
const PART_TOP_N: usize = 0;

impl Running<'_> {
    /// The afterimage effect, once (`x2219_b0`), `ftFx_SpecialS_CreateGFX` (accessory 4).
    #[doc(alias = "ftFx_SpecialS_CreateGFX")]
    pub fn fox_special_s_create_gfx(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_TOP_N as i32);
            let facing = self.game.fighter(gobj).facing_dir;
            use super::super::super::ef::{Arg, spawn};
            spawn(self, 0x48D, Some(gobj), &[Arg::JObj(j), Arg::Float(facing)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// Whether the Illusion's ghost should go: Fox is out of the side special's states,
    /// `ftFx_SpecialS_CheckGhostRemove`.
    #[doc(alias = "ftFx_SpecialS_CheckGhostRemove")]
    pub fn fox_special_s_ghost_gone(&self, gobj: GObjId) -> bool {
        let m = self.game.fighter(gobj).motion_id;
        !(ms::SPECIAL_S_START..=ms::SPECIAL_AIR_S_END).contains(&m)
    }

    /// `ftFx_SpecialS_GetCmdVar2`.
    #[doc(alias = "ftFx_SpecialS_GetCmdVar2")]
    pub fn fox_special_s_cmd_var2(&self, gobj: GObjId) -> i32 {
        self.game.fighter(gobj).cmd_vars[2] as i32
    }

    /// An afterimage's position, `ftFx_SpecialS_CopyGhostPosIndexed`.
    #[doc(alias = "ftFx_SpecialS_CopyGhostPosIndexed")]
    pub fn fox_special_s_ghost_pos(&self, gobj: GObjId, i: i32) -> Vec3 {
        let fp = self.game.fighter(gobj);
        let at = mv::S_GHOST_POS + 0xC * i as usize;
        Vec3::new(fp.mv_f32(at), fp.mv_f32(at + 4), fp.mv_f32(at + 8))
    }

    /// An afterimage's turn, `ftFx_SpecialS_ReturnFloatVarIndexed`.
    #[doc(alias = "ftFx_SpecialS_ReturnFloatVarIndexed")]
    pub fn fox_special_s_blend_frame(&self, gobj: GObjId, i: i32) -> f32 {
        self.game
            .fighter(gobj)
            .mv_f32(mv::S_BLEND_FRAMES + 4 * i as usize)
    }

    /// `ftFox_SpecialS` start's shared setup: the script's ghost flag and gravity's delay
    /// cleared, no ghost.
    fn fox_special_s_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let delay = fp::fctiwz(reg(self.fox_attr_f32(f, attr::ILLUSION_GRAVITY_DELAY)));
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[2] = 0;
        fp.set_mv_i32(mv::S_GRAVITY_DELAY, delay);
        fp.mv.set_gobj_at(mv::S_GHOST_GOBJ, None);
    }

    /// The grounded wind-up, `ftFx_SpecialSStart_Enter`.
    #[doc(alias = "ftFx_SpecialSStart_Enter")]
    pub fn fox_special_s_start_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fox_special_s_start(gobj);
        let div = reg(self.fox_attr_f32(f, attr::ILLUSION_GROUND_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fdivs(reg(fp.gr_vel), div));
        self.change_motion_state(gobj, ms::SPECIAL_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
    }

    /// The aerial wind-up, `ftFx_SpecialAirSStart_Enter`.
    #[doc(alias = "ftFx_SpecialAirSStart_Enter")]
    pub fn fox_special_air_s_start_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fox_special_s_start(gobj);
        let div = reg(self.fox_attr_f32(f, attr::ILLUSION_GROUND_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), div));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S_START, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.x1968_jumpsUsed = fp.co_attrs.max_jumps as u8;
    }

    /// `ftFx_SpecialSStart_Anim`: the dash when the wind-up ends.
    #[doc(alias = "ftFx_SpecialSStart_Anim")]
    pub fn fox_special_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fox_special_s_enter(gobj);
        }
    }

    /// `ftFx_SpecialAirSStart_Anim`.
    #[doc(alias = "ftFx_SpecialAirSStart_Anim")]
    pub fn fox_special_air_s_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fox_special_air_s_enter(gobj);
        }
    }

    /// `ftFx_SpecialSStart_Phys`: gravity's delay counts down, grounded friction.
    #[doc(alias = "ftFx_SpecialSStart_Phys")]
    pub fn fox_special_s_start_phys(&mut self, gobj: GObjId) {
        self.fox_count_down(gobj, mv::S_GRAVITY_DELAY);
        self.ground_friction(gobj);
    }

    /// Counts a state's delay down to 0; whether it was already 0.
    pub(crate) fn fox_count_down(&mut self, gobj: GObjId, off: usize) -> bool {
        let fp = self.game.fighter_mut(gobj);
        let d = fp.mv_i32(off);
        if d != 0 {
            fp.set_mv_i32(off, d.wrapping_sub(1));
            false
        } else {
            true
        }
    }

    /// A fall at `gravity` once the state's delay runs out, then air friction `friction`.
    fn fox_delayed_fall(&mut self, gobj: GObjId, off: usize, gravity: usize, friction: usize) {
        let f = self.game.fighter_id(gobj);
        if self.fox_count_down(gobj, off) {
            let g = reg(self.fox_attr_f32(f, gravity));
            let fp = &mut self.game.fighters[f.0];
            let t = reg(fp.co_attrs.terminal_velocity);
            Self::fall(fp, g, t);
        }
        let fr = reg(self.fox_attr_f32(f, friction));
        Self::self_accel_deaccel(&mut self.game.fighters[f.0], fr);
    }

    /// `ftFx_SpecialAirSStart_Phys`.
    #[doc(alias = "ftFx_SpecialAirSStart_Phys")]
    pub fn fox_special_air_s_start_phys(&mut self, gobj: GObjId) {
        self.fox_delayed_fall(
            gobj,
            mv::S_GRAVITY_DELAY,
            attr::ILLUSION_UNK2,
            attr::ILLUSION_UNK1,
        );
    }

    /// `ftFx_SpecialSStart_Coll`: off the ground, the aerial wind-up.
    #[doc(alias = "ftFx_SpecialSStart_Coll")]
    pub fn fox_special_s_start_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_special_s_start_ground_to_air(gobj);
        }
    }

    /// A landing or a ledge catch for the aerial states (`ft_CheckGroundAndLedge` toward the
    /// facing, then `ftCliffCommon_80081298`); whether it landed.
    fn fox_air_coll(&mut self, gobj: GObjId) -> bool {
        let dir = facing_dir_int(self.game.fighter(gobj).facing_dir);
        if self.check_ground_and_ledge(gobj, dir) {
            return true;
        }
        self.catch_ledge_check(gobj);
        false
    }

    /// `ftFx_SpecialAirSStart_Coll`.
    #[doc(alias = "ftFx_SpecialAirSStart_Coll")]
    pub fn fox_special_air_s_start_coll(&mut self, gobj: GObjId) {
        if self.fox_air_coll(gobj) {
            self.fox_special_air_s_start_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialSStart_GroundToAir`.
    #[doc(alias = "ftFx_SpecialSStart_GroundToAir")]
    pub fn fox_special_s_start_ground_to_air(&mut self, gobj: GObjId) {
        self.go_airborne_jumped(self.game.fighter_id(gobj));
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_S_START,
            COLL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
    }

    /// `ftFx_SpecialAirSStart_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirSStart_AirToGround")]
    pub fn fox_special_air_s_start_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S_START, COLL_FLAGS);
    }

    /// `ftFox_SpecialS_CreateGhostItem`: the Illusion's ghost when the script asks.
    fn fox_special_s_ghost(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[2] != 1 {
            return;
        }
        self.game.fighters[f.0].cmd_vars[2] = 0;
        let fp = &self.game.fighters[f.0];
        let kind = if fp.kind == KIND_FOX {
            IT_KIND_FOX_ILLUSION
        } else {
            IT_KIND_FALCO_PHANTASM
        };
        let (pos, facing) = (fp.cur_pos, fp.facing_dir);
        let ghost =
            super::super::super::it::kinds::foxillusion::spawn(self, gobj, pos, kind, facing);
        if let Some(g) = ghost {
            let fp = &mut self.game.fighters[f.0];
            fp.mv.set_gobj_at(mv::S_GHOST_GOBJ, Some(g));
            fp.x2222_b2 = true;
        }
    }

    /// The grounded dash, `ftFx_SpecialS_Anim`: the end when it's over, and the ghost.
    #[doc(alias = "ftFx_SpecialS_Anim")]
    pub fn fox_special_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fox_special_s_end_enter(gobj);
        }
        self.fox_special_s_ghost(gobj);
    }

    /// The aerial dash, `ftFx_SpecialAirS_Anim`.
    #[doc(alias = "ftFx_SpecialAirS_Anim")]
    pub fn fox_special_air_s_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fox_special_air_s_end_enter(gobj);
        }
        self.fox_special_s_ghost(gobj);
    }

    /// B cuts the dash short, `ftFx_SpecialS_IASA` and `ftFx_SpecialAirS_IASA`.
    #[doc(alias = "ftFx_SpecialS_IASA")]
    #[doc(alias = "ftFx_SpecialAirS_IASA")]
    pub fn fox_special_s_iasa(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        if fp.input.pressed_buttons & PAD_B != 0 {
            if fp.ground_or_air == GA_AIR {
                self.fox_special_air_s_end_enter(gobj);
            } else {
                self.fox_special_s_end_enter(gobj);
            }
        }
    }

    /// `ftFox_SpecialS_SetPhys`: the afterimages' trail moves on.
    fn fox_special_s_trail(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        for i in (1..4).rev() {
            let from = mv::S_GHOST_POS + 0xC * (i - 1);
            let to = mv::S_GHOST_POS + 0xC * i;
            let v: [u8; 0xC] = fp.mv[from..from + 0xC].try_into().unwrap();
            fp.mv[to..to + 0xC].copy_from_slice(&v);
        }
        let pos = fp.cur_pos;
        set_mv_vec(fp, mv::S_GHOST_POS, pos);
        for i in (1..4).rev() {
            let v = fp.mv_f32(mv::S_BLEND_FRAMES + 4 * (i - 1));
            fp.set_mv_f32(mv::S_BLEND_FRAMES + 4 * i, reg(v));
        }
        let rot = self.game.part_rot_x(f, 0);
        self.game.fighters[f.0].set_mv_f32(mv::S_BLEND_FRAMES, reg(rot));
    }

    /// `ftFx_SpecialS_Phys`: the animation's motion along the ground, the trail.
    #[doc(alias = "ftFx_SpecialS_Phys")]
    pub fn fox_special_s_phys(&mut self, gobj: GObjId) {
        self.ground_anim_speed_facing(gobj);
        self.fox_special_s_trail(gobj);
    }

    /// `ftFx_SpecialAirS_Phys`: the animation's motion in the air, the trail.
    #[doc(alias = "ftFx_SpecialAirS_Phys")]
    pub fn fox_special_air_s_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.fox_special_s_trail(gobj);
    }

    /// `ftFx_SpecialS_Coll`.
    #[doc(alias = "ftFx_SpecialS_Coll")]
    pub fn fox_special_s_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.fox_special_s_ground_to_air(gobj);
        }
    }

    /// `ftFx_SpecialAirS_Coll`.
    #[doc(alias = "ftFx_SpecialAirS_Coll")]
    pub fn fox_special_air_s_coll(&mut self, gobj: GObjId) {
        if self.fox_air_coll(gobj) {
            self.fox_special_air_s_air_to_ground(gobj);
        }
    }

    /// `ftFx_SpecialS_GroundToAir`.
    #[doc(alias = "ftFx_SpecialS_GroundToAir")]
    pub fn fox_special_s_ground_to_air(&mut self, gobj: GObjId) {
        self.go_airborne_jumped(self.game.fighter_id(gobj));
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_S,
            DASH_COLL_FLAGS,
            frame,
            1.0,
            0.0,
            None,
        );
        self.game.fighter_mut(gobj).cmd_vars[2] = 0;
    }

    /// `ftFx_SpecialAirS_AirToGround`.
    #[doc(alias = "ftFx_SpecialAirS_AirToGround")]
    pub fn fox_special_air_s_air_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_S, DASH_COLL_FLAGS);
        self.game.fighter_mut(gobj).cmd_vars[2] = 0;
    }

    /// `ftFox_SpecialS_SetVars`: the trail starts where Fox is; the afterimage effect comes.
    fn fox_special_s_dash_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let pos = fp.cur_pos;
        for i in (0..4).rev() {
            set_mv_vec(fp, mv::S_GHOST_POS + 0xC * i, pos);
        }
        let rot = reg(self.game.part_rot_x(f, 0));
        let fp = &mut self.game.fighters[f.0];
        for i in (0..4).rev() {
            fp.set_mv_f32(mv::S_BLEND_FRAMES + 4 * i, rot);
        }
        fp.accessory4_cb = Some(CodeRef(addr::SPECIAL_S_CREATE_GFX));
    }

    /// The grounded dash, `ftFx_SpecialS_Enter`.
    #[doc(alias = "ftFx_SpecialS_Enter")]
    pub fn fox_special_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_S, SKIP_RUMBLE, 0.0, 1.0, 0.0, None);
        self.fox_special_s_dash_vars(gobj);
    }

    /// The aerial dash, `ftFx_SpecialAirS_Enter`.
    #[doc(alias = "ftFx_SpecialAirS_Enter")]
    pub fn fox_special_air_s_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_AIR_S, SKIP_RUMBLE, 0.0, 1.0, 0.0, None);
        self.fox_special_s_dash_vars(gobj);
    }

    /// `ftFx_SpecialSEnd_Anim`: waiting when it ends.
    #[doc(alias = "ftFx_SpecialSEnd_Anim")]
    #[doc(alias = "ftFx_SpecialHiLanding_Anim")]
    pub fn fox_wait_when_done_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftFx_SpecialAirSEnd_Anim`: falling helplessly when it ends.
    #[doc(alias = "ftFx_SpecialAirSEnd_Anim")]
    pub fn fox_special_air_s_end_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let mobility = self.fox_attr_f32(f, attr::ILLUSION_FREEFALL_MOBILITY);
        let lag = self.fox_attr_f32(f, attr::ILLUSION_LANDING_LAG);
        if !self.anim_frames_remaining(gobj) {
            self.fall_special(gobj, mobility, lag);
        }
    }

    /// `ftFx_SpecialSEnd_Phys`.
    #[doc(alias = "ftFx_SpecialSEnd_Phys")]
    pub fn fox_special_s_end_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.fox_count_down(gobj, mv::S_GRAVITY_DELAY);
        let fr = reg(self.fox_attr_f32(f, attr::ILLUSION_GROUND_FRICTION));
        Self::ground_accel_deaccel(&mut self.game.fighters[f.0], fr);
        self.self_movement_from_ground(gobj);
        self.fox_special_s_trail(gobj);
    }

    /// `ftFx_SpecialAirSEnd_Phys`.
    #[doc(alias = "ftFx_SpecialAirSEnd_Phys")]
    pub fn fox_special_air_s_end_phys(&mut self, gobj: GObjId) {
        self.fox_delayed_fall(
            gobj,
            mv::S_GRAVITY_DELAY,
            attr::ILLUSION_TERMINAL_VELOCITY,
            attr::ILLUSION_AIR_MUL_X,
        );
        self.fox_special_s_trail(gobj);
    }

    /// `ftFx_SpecialSEnd_Coll`: falling off the edge.
    #[doc(alias = "ftFx_SpecialSEnd_Coll")]
    pub fn fox_special_s_end_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_slide(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftFx_SpecialAirSEnd_Coll`: landing with the special landing lag.
    #[doc(alias = "ftFx_SpecialAirSEnd_Coll")]
    pub fn fox_special_air_s_end_coll(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let lag = self.fox_attr_f32(f, attr::ILLUSION_LANDING_LAG);
        if self.fox_air_coll(gobj) {
            self.enter_landing_fall_special(gobj, 0, lag);
        }
    }

    /// `ftFox_SpecialSEnd_SetVars`.
    fn fox_special_s_end_vars(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let delay = fp::fctiwz(reg(self.fox_attr_f32(f, attr::ILLUSION_FALL_ACCEL)));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_i32(mv::S_GRAVITY_DELAY, delay);
        fp.x2222_b2 = true;
    }

    /// The grounded end, `ftFx_SpecialSEnd_Enter`.
    #[doc(alias = "ftFx_SpecialSEnd_Enter")]
    pub fn fox_special_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let v = reg(self.fox_attr_f32(f, attr::ILLUSION_GROUND_END_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.gr_vel = mem(fp::fmuls(v, reg(fp.facing_dir)));
        self.change_motion_state(gobj, ms::SPECIAL_S_END, SKIP_RUMBLE, 0.0, 1.0, 0.0, None);
        self.fox_special_s_end_vars(gobj);
    }

    /// The aerial end, `ftFx_SpecialAirSEnd_Enter`.
    #[doc(alias = "ftFx_SpecialAirSEnd_Enter")]
    pub fn fox_special_air_s_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let v = reg(self.fox_attr_f32(f, attr::ILLUSION_AIR_END_VEL_X));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fmuls(v, reg(fp.facing_dir)));
        fp.self_vel.y = 0.0;
        self.change_motion_state(
            gobj,
            ms::SPECIAL_AIR_S_END,
            SKIP_RUMBLE,
            0.0,
            1.0,
            0.0,
            None,
        );
        self.fox_special_s_end_vars(gobj);
    }
}

/// Stores a vector into the state's variables.
fn set_mv_vec(fp: &mut super::super::super::structs::Fighter, at: usize, v: Vec3) {
    fp.set_mv_f32(at, reg(v.x));
    fp.set_mv_f32(at + 4, reg(v.y));
    fp.set_mv_f32(at + 8, reg(v.z));
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::SPECIAL_S_CREATE_GFX => g.fox_special_s_create_gfx(gobj),
        addr::SPECIAL_S_START_ENTER => g.fox_special_s_start_enter(gobj),
        addr::SPECIAL_AIR_S_START_ENTER => g.fox_special_air_s_start_enter(gobj),
        0x800E_A004 => g.fox_special_s_start_anim(gobj),
        0x800E_A040 => g.fox_special_air_s_start_anim(gobj),
        0x800E_A07C | 0x800E_A080 | 0x800E_A9A4 | 0x800E_A9A8 => g.fox_no_iasa(gobj),
        0x800E_A084 => g.fox_special_s_start_phys(gobj),
        0x800E_A0BC => g.fox_special_air_s_start_phys(gobj),
        0x800E_A128 => g.fox_special_s_start_coll(gobj),
        0x800E_A164 => g.fox_special_air_s_start_coll(gobj),
        0x800E_A1D4 => g.fox_special_s_start_ground_to_air(gobj),
        0x800E_A234 => g.fox_special_air_s_start_air_to_ground(gobj),
        0x800E_A294 => g.fox_special_s_anim(gobj),
        0x800E_A344 => g.fox_special_air_s_anim(gobj),
        0x800E_A3F4 | 0x800E_A438 => g.fox_special_s_iasa(gobj),
        0x800E_A47C => g.fox_special_s_phys(gobj),
        0x800E_A534 => g.fox_special_air_s_phys(gobj),
        0x800E_A5EC => g.fox_special_s_coll(gobj),
        0x800E_A628 => g.fox_special_air_s_coll(gobj),
        0x800E_A698 => g.fox_special_s_ground_to_air(gobj),
        0x800E_A700 => g.fox_special_air_s_air_to_ground(gobj),
        0x800E_A768 => g.fox_special_s_enter(gobj),
        0x800E_A838 => g.fox_special_air_s_enter(gobj),
        0x800E_A908 => g.fox_wait_when_done_anim(gobj),
        0x800E_A944 => g.fox_special_air_s_end_anim(gobj),
        0x800E_A9AC => g.fox_special_s_end_phys(gobj),
        0x800E_AA8C => g.fox_special_air_s_end_phys(gobj),
        0x800E_AB90 => g.fox_special_s_end_coll(gobj),
        0x800E_ABCC => g.fox_special_air_s_end_coll(gobj),
        0x800E_AC50 => g.fox_special_s_end_enter(gobj),
        0x800E_ACD8 => g.fox_special_air_s_end_enter(gobj),
        _ => return false,
    }
    true
}

pub(super) fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    let gobj = |i: usize| args[i].gobj().expect("a null fighter");
    Some(match f {
        addr::SPECIAL_S_CHECK_GHOST_REMOVE => {
            Val::Int(i32::from(r.fox_special_s_ghost_gone(gobj(0))))
        }
        addr::SPECIAL_S_GET_CMD_VAR2 => Val::Int(r.fox_special_s_cmd_var2(gobj(0))),
        addr::SPECIAL_S_COPY_GHOST_POS_INDEXED => {
            Val::Vec(r.fox_special_s_ghost_pos(gobj(0), args[1].int()))
        }
        addr::SPECIAL_S_RETURN_FLOAT_VAR_INDEXED => {
            Val::Float(reg(r.fox_special_s_blend_frame(gobj(0), args[1].int())))
        }
        _ => return None,
    })
}
