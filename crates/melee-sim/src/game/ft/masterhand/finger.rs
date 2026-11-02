//! The finger beams (`ftmasterhandfingerbeam.c`: a laser from four fingers, with their hum)
//! and the finger gun (`ftmasterhandfingergun.c`, `ftmasterhandfingergun3.c`: chasing the
//! nearest opponent, then a round of bullets from two fingers, faster and more of them once
//! he's hurt past `xEC`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{masterhandbullet, masterhandlaser};
use super::super::super::state::Running;
use super::{addr, attr, ms, mv};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FtPart_LKneeJ`, `FtPart_RLegJA`, `FtPart_RKneeJ`, `FtPart_BustN`, `FtPart_LHandN` and
/// `FtPart_L3rdNa`: the fingers his beams and shots come from (his parts table is a hand's).
const PART_L_KNEE_J: i32 = 8;
const PART_R_LEG_JA: i32 = 11;
const PART_R_KNEE_J: i32 = 13;
const PART_BUST_N: i32 = 16;
const PART_L_HAND_N: i32 = 21;
const PART_L_3RD_NA: i32 = 26;

/// The beams' three hums.
const SFX_BEAM: [i32; 3] = [320004, 320005, 320006];

impl Running<'_> {
    /// The finger beams' start, `ftMh_MS_359_80152BCC`: no hum playing (`mv.x28` to `x30`).
    #[doc(alias = "ftMh_MS_359_80152BCC")]
    pub fn mh_beam_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::FINGER_BEAM_START);
        let fp = self.game.fighter_mut(gobj);
        for off in [mv::X28, mv::X2C, mv::X30] {
            fp.set_mv_i32(off, -1);
        }
    }

    /// `ftMh_FingerBeamStart_Anim`: the beams once it ends.
    #[doc(alias = "ftMh_FingerBeamStart_Anim")]
    pub fn mh_beam_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_beam_loop_enter(gobj);
        }
    }

    /// The beams, `ftMh_MS_361_80152CD8`: made by the accessory on its first frame.
    #[doc(alias = "ftMh_MS_361_80152CD8")]
    pub fn mh_beam_loop_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::FINGER_BEAM_LOOP);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(addr::MS_362_80152E28));
        fp.cmd_vars[0] = 1;
    }

    /// `ftMh_FingerBeamLoop_Anim`: the end once it ends, the lasers ended.
    #[doc(alias = "ftMh_FingerBeamLoop_Anim")]
    pub fn mh_beam_loop_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.mh_beam_end_enter(gobj);
        let f = self.game.fighter_id(gobj);
        for off in [mv::X34, mv::X38, mv::X3C, mv::X40] {
            let l = self.game.fighters[f.0].mv.gobj_at(off);
            masterhandlaser::end(self, l);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in [mv::X34, mv::X38, mv::X3C, mv::X40] {
            fp.mv.set_gobj_at(off, None);
        }
    }

    /// The lasers and their hums on the accessory's first frame, `ftMh_MS_362_80152E28`.
    #[doc(alias = "ftMh_MS_362_80152E28")]
    pub fn mh_beam_lasers(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            return;
        }
        for (off, part) in [
            (mv::X34, PART_R_LEG_JA),
            (mv::X38, PART_BUST_N),
            (mv::X3C, PART_L_HAND_N),
            (mv::X40, PART_L_3RD_NA),
        ] {
            let j = self.mh_part_joint(gobj, part);
            let v = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
            let facing = self.game.fighters[f.0].facing_dir;
            let l = masterhandlaser::spawn(self, gobj, v, v, part, masterhandlaser::KIND, facing);
            self.game.fighters[f.0].mv.set_gobj_at(off, l);
        }
        for (off, id) in [mv::X28, mv::X2C, mv::X30].into_iter().zip(SFX_BEAM) {
            let h = self.sfx_play(id, 127, 64);
            self.game.fighters[f.0].set_mv_i32(off, h);
        }
        self.game.fighters[f.0].cmd_vars[0] = 0;
    }

    /// The joint of his part `part` (`fp->parts[part]`, his table indexed by the part).
    fn mh_part_joint(&self, gobj: GObjId, part: i32) -> Option<crate::hsd::jobj::JObjId> {
        let parts = self
            .game
            .fighter(gobj)
            .parts
            .expect("a fighter without parts");
        self.game.fighter_parts[parts.0][part as usize].joint
    }

    /// The beams' end, `ftMh_MS_362_80152F80`: the hums stopped.
    #[doc(alias = "ftMh_MS_362_80152F80")]
    pub fn mh_beam_end_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::FINGER_BEAM_END);
        let f = self.game.fighter_id(gobj);
        for off in [mv::X28, mv::X2C, mv::X30] {
            let h = self.game.fighters[f.0].mv_i32(off);
            self.sfx_stop(h);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in [mv::X28, mv::X2C, mv::X30] {
            fp.set_mv_i32(off, -1);
        }
    }

    /// The finger gun, `ftMh_MS_363_801530A4`: `xE4` and up to `xE8` chases first
    /// (`mv.x50`).
    #[doc(alias = "ftMh_MS_363_801530A4")]
    pub fn mh_gun_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::FINGER_GUN1);
        let (hi, lo) = (self.mh_da_f32(f, attr::XE8), self.mh_da_f32(f, attr::XE4));
        let r = self
            .game
            .objs
            .rng
            .below(fp::fctiwz(fp::fsubs(reg(hi), reg(lo))));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X50, fp::fadds(reg(lo), fp::frsp(f64::from(r))));
        fp.cmd_vars[0] = 0;
        fp.cmd_vars[1] = 0;
        fp.cmd_vars[2] = 0;
    }

    /// `ftMh_FingerGun1_Anim`: chasing once each round of the animation ends; the shots once
    /// the count runs out (`mv.x54` rounds of them, `xF0` when he's past `xEC`).
    #[doc(alias = "ftMh_FingerGun1_Anim")]
    pub fn mh_gun1_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.cmd_vars[2] = 1;
        let n = crate::fpu::mem(fp::fsubs(reg(fp.mv_f32(mv::X50)), 1.0));
        fp.set_mv_f32(mv::X50, reg(n));
        if reg(n) < 0.0 {
            self.mh_gun2_enter(gobj);
            let rounds = if self.game.ft_percent(gobj) > self.mh_da_i32(f, attr::XEC) {
                self.mh_da_i32(f, attr::XF0)
            } else {
                1
            };
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::X54, rounds);
            fp.cmd_vars[2] = 0;
        }
    }

    /// `ftMh_FingerGun1_Phys`: the air motion, chasing `xDC`, `xE0` off the nearest opponent
    /// once the script says.
    #[doc(alias = "ftMh_FingerGun1_Phys")]
    pub fn mh_gun1_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        if self.game.fighter(gobj).cmd_vars[2] != 0 {
            self.mh_chase(gobj, attr::XDC, attr::XE0);
        }
    }

    /// The shots, `ftMh_MS_364_801533CC`: faster past `xEC` (`xF4`), still, bullets made by
    /// the accessory.
    #[doc(alias = "ftMh_MS_364_801533CC")]
    pub fn mh_gun2_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::FINGER_GUN2);
        if self.game.ft_percent(gobj) > self.mh_da_i32(f, attr::XEC) {
            let rate = self.mh_da_f32(f, attr::XF4);
            self.set_anim_rate(gobj, rate);
        }
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = 0.0;
        fp.self_vel.y = 0.0;
        fp.accessory4_cb = Some(CodeRef(addr::MS_365_801535B0));
    }

    /// `ftMh_FingerGun2_Anim`: another round, or the end once the rounds run out.
    #[doc(alias = "ftMh_FingerGun2_Anim")]
    pub fn mh_gun2_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X54).wrapping_sub(1);
        fp.set_mv_i32(mv::X54, n);
        if n == 0 {
            self.set_anim_rate(gobj, 1.0);
            self.mh_enter(gobj, ms::FINGER_GUN3);
        } else {
            self.mh_gun2_enter(gobj);
        }
    }

    /// A bullet from each finger the script flags, `ftMh_MS_365_801535B0`: the left
    /// (`xF8`, `xFC` off it) and the right (`x100`, `x104`).
    #[doc(alias = "ftMh_MS_365_801535B0")]
    pub fn mh_gun_shots(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let j = self.mh_part_joint(gobj, PART_L_KNEE_J);
            let (dx, dy) = (self.mh_da_f32(f, attr::XF8), self.mh_da_f32(f, attr::XFC));
            self.mh_gun_shot(gobj, j, dx, dy);
            self.game.fighters[f.0].cmd_vars[0] = 0;
        }
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            let j = self.mh_part_joint(gobj, PART_R_KNEE_J);
            let (dx, dy) = (self.mh_da_f32(f, attr::X100), self.mh_da_f32(f, attr::X104));
            self.mh_gun_shot(gobj, j, dx, dy);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
    }

    /// A bullet from joint `j`, `dx`, `dy` off it, at `xD4`, `xD8`; fast past `xEC`,
    /// `ftMh_MS_365_8015364C`.
    #[doc(alias = "ftMh_MS_365_8015364C")]
    pub fn mh_gun_shot(
        &mut self,
        gobj: GObjId,
        j: Option<crate::hsd::jobj::JObjId>,
        dx: f32,
        dy: f32,
    ) {
        let f = self.game.fighter_id(gobj);
        let at = crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None);
        let from = at;
        let pos = Vec3::new(
            crate::fpu::mem(fp::fadds(reg(at.x), reg(dx))),
            crate::fpu::mem(fp::fadds(reg(at.y), reg(dy))),
            at.z,
        );
        let fast = self.game.ft_percent(gobj) > self.mh_da_i32(f, attr::XEC);
        let facing = self.game.fighters[f.0].facing_dir;
        let (sx, sy) = (self.mh_da_f32(f, attr::XD4), self.mh_da_f32(f, attr::XD8));
        masterhandbullet::spawn(
            self,
            gobj,
            pos,
            from,
            masterhandbullet::KIND,
            fast,
            facing,
            sx,
            sy,
        );
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_359_80152BCC => g.mh_beam_enter(gobj),
        addr::FINGER_BEAM_START_ANIM => g.mh_beam_start_anim(gobj),
        addr::MS_361_80152CD8 => g.mh_beam_loop_enter(gobj),
        addr::FINGER_BEAM_LOOP_ANIM => g.mh_beam_loop_anim(gobj),
        addr::MS_362_80152E28 => g.mh_beam_lasers(gobj),
        addr::MS_362_80152F80 => g.mh_beam_end_enter(gobj),
        addr::MS_363_801530A4 => g.mh_gun_enter(gobj),
        addr::FINGER_GUN1_ANIM => g.mh_gun1_anim(gobj),
        addr::FINGER_GUN1_PHYS => g.mh_gun1_phys(gobj),
        addr::MS_364_801533CC => g.mh_gun2_enter(gobj),
        addr::FINGER_GUN2_ANIM => g.mh_gun2_anim(gobj),
        addr::MS_365_801535B0 => g.mh_gun_shots(gobj),
        addr::MS_365_80153730 => g.mh_enter(gobj, ms::FINGER_GUN3),
        _ => return false,
    }
    true
}
