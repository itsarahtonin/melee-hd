//! Nayru's Love (`ftzeldaspecialn.c`): a spin whose script turns the reflector on
//! (`cmd_vars[0]` 1, then 2) and off, with an effect at TransN.

use gekko_fp as fp;

use super::super::super::ef::{Arg, spawn};
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::guard::ReflectDesc;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;

/// `FtPart_TransN`: Zelda's code indexes `fp->parts` directly.
const PART_TRANS_N: i32 = 1;

/// `ftZd_MF_SpecialN_Coll`, as Sheik's specials' (`0xC4C508E`).
const COLL_FLAGS: u32 = 0x0C4C_508E;

/// The spin's effect on the ground and in the air.
const GFX_GROUND: i32 = 1268;
const GFX_AIR: i32 = 1269;

impl Running<'_> {
    /// The spin's effect, once (`x2219_b0`), then the hitlag hooks and no more accessory.
    fn zelda_special_n_gfx(&mut self, gobj: GObjId, gfx: i32) {
        if !self.game.fighter(gobj).x2219_b0 {
            let j = self.bone_joint(gobj, PART_TRANS_N);
            spawn(self, gfx, Some(gobj), &[Arg::JObj(j)]);
            self.game.fighter_mut(gobj).x2219_b0 = true;
        }
        self.set_effect_hitlag_cbs(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = None;
    }

    /// `ftZd_SpecialN_8013A830` (an accessory).
    // Ports `ftZd_SpecialN_8013A830`; aliased once a check reaches it.
    pub fn zelda_special_n_gfx_ground(&mut self, gobj: GObjId) {
        self.zelda_special_n_gfx(gobj, GFX_GROUND);
    }

    /// `ftZd_SpecialN_8013A8AC` (an accessory).
    // Ports `ftZd_SpecialN_8013A8AC`; aliased once a check reaches it.
    pub fn zelda_special_n_gfx_air(&mut self, gobj: GObjId) {
        self.zelda_special_n_gfx(gobj, GFX_AIR);
    }

    /// The state's start (`startActionHelper`): the script's flag clear, the hover's frames
    /// (`x4`).
    fn zelda_special_n_start(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frames = self.zelda_attr_i32(f, attr::X4);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[0] = 0;
        fp.set_mv_i32(mv::N_X0, frames);
    }

    /// `ftZd_SpecialN_Enter`.
    // Ports `ftZd_SpecialN_Enter`; aliased once a check reaches it.
    pub fn zelda_special_n_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::SPECIAL_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_n_start(gobj);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::N_8013A830));
    }

    /// `ftZd_SpecialAirN_Enter`: the fall stopped, the drift divided down (`x8`).
    // Ports `ftZd_SpecialAirN_Enter`; aliased once a check reaches it.
    pub fn zelda_special_air_n_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let x8 = reg(self.zelda_attr_f32(f, attr::X8));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.y = 0.0;
        fp.self_vel.x = mem(fp::fdivs(reg(fp.self_vel.x), x8));
        self.change_motion_state(gobj, ms::SPECIAL_AIR_N, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(gobj);
        self.zelda_special_n_start(gobj);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::N_8013A8AC));
    }

    /// The reflector, `ftColl_CreateReflectHit` with the attributes' `ReflectDesc` (`x84`),
    /// reflecting into `ftZd_SpecialN_8013ADB0` (nothing).
    fn zelda_special_n_reflect(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let a = |o: usize| self.zelda_attr_f32(f, o);
        let d = ReflectDesc {
            bone: self.zelda_attr_i32(f, 0x84),
            max_damage: self.zelda_attr_i32(f, 0x88),
            offset: crate::vec::Vec3::new(a(0x8C), a(0x90), a(0x94)),
            size: a(0x98),
            damage_mul: a(0x9C),
            speed_mul: a(0xA0),
            behavior: (self.zelda_attr_i32(f, 0xA4) as u32 >> 24) as u8,
        };
        self.create_reflect_hit(gobj, &d, CodeRef(addr::N_8013ADB0));
    }

    /// The reflector on when the script says (`cmd_vars[0]` 1 becomes 2), off while it's 0.
    fn zelda_special_n_script(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).cmd_vars[0] == 1 {
            self.game.fighter_mut(gobj).cmd_vars[0] = 2;
            self.zelda_special_n_reflect(gobj);
        }
        if self.game.fighter(gobj).cmd_vars[0] == 0 {
            self.game.fighter_mut(gobj).reflecting = false;
        }
    }

    /// `ftZd_SpecialN_Anim`: standing once it ends.
    // Ports `ftZd_SpecialN_Anim`; aliased once a check reaches it.
    pub fn zelda_special_n_anim(&mut self, gobj: GObjId) {
        self.zelda_special_n_script(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_wait(gobj);
        }
    }

    /// `ftZd_SpecialAirN_Anim`: falling once it ends.
    // Ports `ftZd_SpecialAirN_Anim`; aliased once a check reaches it.
    pub fn zelda_special_air_n_anim(&mut self, gobj: GObjId) {
        self.zelda_special_n_script(gobj);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// The states' empty IASA callbacks, and the reflection's (`ftZd_SpecialN_8013ADB0`).
    // Ports `ftZd_SpecialN_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialAirN_IASA`; aliased once a check reaches it.
    // Ports `ftZd_SpecialN_8013ADB0`; aliased once a check reaches it.
    pub fn zelda_special_n_nothing(&mut self, _gobj: GObjId) {}

    /// `ftZd_SpecialN_Phys`: friction, the reflector following.
    // Ports `ftZd_SpecialN_Phys`; aliased once a check reaches it.
    pub fn zelda_special_n_phys(&mut self, gobj: GObjId) {
        self.ground_friction(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftZd_SpecialAirN_Phys`: the hover's frames count down (`specialn.x0`), then the fall
    /// (`xC`, terminal velocity); the drift slowing, the reflector following.
    // Ports `ftZd_SpecialAirN_Phys`; aliased once a check reaches it.
    pub fn zelda_special_air_n_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let n = self.game.fighters[f.0].mv_i32(mv::N_X0);
        if n != 0 {
            self.game.fighters[f.0].set_mv_i32(mv::N_X0, n.wrapping_sub(1));
        } else {
            let g = reg(self.zelda_attr_f32(f, attr::XC));
            let t = reg(self.game.fighters[f.0].co_attrs.terminal_velocity);
            Self::fall(&mut self.game.fighters[f.0], g, t);
        }
        self.self_accel_deaccel_quick_air(gobj);
        self.reflect_follow(gobj);
    }

    /// `ftZd_SpecialN_Coll`: off the ground, the aerial spin.
    // Ports `ftZd_SpecialN_Coll`; aliased once a check reaches it.
    pub fn zelda_special_n_coll(&mut self, gobj: GObjId) {
        if !self.ground_coll_off_ledges(gobj) {
            self.zelda_special_n_to_air(gobj);
        }
    }

    /// `ftZd_SpecialAirN_Coll`: landing, the grounded spin.
    // Ports `ftZd_SpecialAirN_Coll`; aliased once a check reaches it.
    pub fn zelda_special_air_n_coll(&mut self, gobj: GObjId) {
        if self.air_coll_land(gobj) {
            self.zelda_special_n_to_ground(gobj);
        }
    }

    /// `ftZd_SpecialN_8013AC88`: the reflector made anew if on.
    // Ports `ftZd_SpecialN_8013AC88`; aliased once a check reaches it.
    pub fn zelda_special_n_to_air(&mut self, gobj: GObjId) {
        self.fox_ground_to_air(gobj, ms::SPECIAL_AIR_N, COLL_FLAGS);
        if self.game.fighter(gobj).cmd_vars[0] == 2 {
            self.zelda_special_n_reflect(gobj);
        }
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::N_8013A8AC));
    }

    /// `ftZd_SpecialN_8013AD1C`.
    // Ports `ftZd_SpecialN_8013AD1C`; aliased once a check reaches it.
    pub fn zelda_special_n_to_ground(&mut self, gobj: GObjId) {
        self.fox_air_to_ground(gobj, ms::SPECIAL_N, COLL_FLAGS);
        if self.game.fighter(gobj).cmd_vars[0] == 2 {
            self.zelda_special_n_reflect(gobj);
        }
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::N_8013A830));
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::N_8013A830 => g.zelda_special_n_gfx_ground(gobj),
        addr::N_8013A8AC => g.zelda_special_n_gfx_air(gobj),
        addr::SPECIAL_N_ENTER => g.zelda_special_n_enter(gobj),
        addr::SPECIAL_AIR_N_ENTER => g.zelda_special_air_n_enter(gobj),
        0x8013_AA38 => g.zelda_special_n_anim(gobj),
        0x8013_AACC => g.zelda_special_air_n_anim(gobj),
        0x8013_AB60 | 0x8013_AB64 | addr::N_8013ADB0 => g.zelda_special_n_nothing(gobj),
        0x8013_AB68 => g.zelda_special_n_phys(gobj),
        0x8013_AB9C => g.zelda_special_air_n_phys(gobj),
        0x8013_AC10 => g.zelda_special_n_coll(gobj),
        0x8013_AC4C => g.zelda_special_air_n_coll(gobj),
        0x8013_AC88 => g.zelda_special_n_to_air(gobj),
        0x8013_AD1C => g.zelda_special_n_to_ground(gobj),
        _ => return false,
    }
    true
}
