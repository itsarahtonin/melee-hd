//! The punch (`ftmasterhandrockcrush.c`: the fist raised, held over the nearest opponent,
//! then down), the flat slap (`ftmasterhandpapercrush.c`) and the poke
//! (`ftmasterhandpoke.c`: pokes at the nearest opponent a random number of times).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::{addr, attr, ms, mv};
use crate::fpu::reg;
use crate::hsd::gobj::GObjId;

/// The poke's sound.
const SFX_POKE: i32 = 320007;

impl Running<'_> {
    /// `ftMh_RockCrushUp_Anim`: the fist held for `xA0` frames once it's up.
    #[doc(alias = "ftMh_RockCrushUp_Anim")]
    pub fn mh_rock_up_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.mh_enter(gobj, ms::ROCK_CRUSH_WAIT);
            let n = self.mh_da_i32(f, attr::XA0);
            self.game.fighters[f.0].set_mv_i32(mv::X8, n);
        }
    }

    /// `ftMh_RockCrushWait_Anim`: down once the count runs out; the hold's animation again.
    #[doc(alias = "ftMh_RockCrushWait_Anim")]
    pub fn mh_rock_wait_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.mh_rock_down_enter(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.mh_enter(gobj, ms::ROCK_CRUSH_WAIT);
        }
    }

    /// Following the nearest opponent at most `max_off` a frame, over the stage: the hold's
    /// physics (`ftMh_RockCrushWait_Phys`, `xA4`).
    #[doc(alias = "ftMh_RockCrushWait_Phys")]
    pub fn mh_rock_wait_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let max = self.mh_da_f32(f, attr::XA4);
        self.boss_follow_x(gobj, max);
        self.boss_clamp_to_floor(gobj);
    }

    /// The fist down, stopped, `ftMh_MS_356_801525E0`.
    #[doc(alias = "ftMh_MS_356_801525E0")]
    pub fn mh_rock_down_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.x = 0.0;
        self.mh_enter(gobj, ms::ROCK_CRUSH_DOWN);
    }

    /// The flat slap, `ftMh_MS_357_801526D8`: its frame count from 0 (`mv.x0`).
    #[doc(alias = "ftMh_MS_357_801526D8")]
    pub fn mh_paper_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::PAPER_CRUSH);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 0.0);
    }

    /// `ftMh_PaperCrush_Phys`: following the nearest opponent (at most `xB8` a frame)
    /// between frames `xB0` and `xB4`, over the stage.
    #[doc(alias = "ftMh_PaperCrush_Phys")]
    pub fn mh_paper_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let (lo, hi, max) = (
            self.mh_da_i32(f, attr::XB0),
            self.mh_da_i32(f, attr::XB4),
            self.mh_da_f32(f, attr::XB8),
        );
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fadds(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(lo)) && n < fp::frsp(f64::from(hi)) {
            self.boss_follow_x(gobj, max);
        } else {
            self.game.fighters[f.0].self_vel.x = 0.0;
        }
        self.boss_clamp_to_floor(gobj);
    }

    /// The poke, `ftMh_MS_358_80152880`: `x94` and up to `x90` pokes (`mv.x0`), not chasing
    /// yet (`mv.x4`).
    #[doc(alias = "ftMh_MS_358_80152880")]
    pub fn mh_poke_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::POKE1);
        let (hi, lo) = (self.mh_da_i32(f, attr::X90), self.mh_da_i32(f, attr::X94));
        let r = self.game.objs.rng.below(hi.wrapping_sub(lo));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(lo.wrapping_add(r))));
        fp.set_mv_f32(mv::X4, 0.0);
    }

    /// `ftMh_Poke1_Anim`: once a poke ends he chases; past the last, the last poke.
    #[doc(alias = "ftMh_Poke1_Anim")]
    pub fn mh_poke1_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X4, 1.0);
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        if reg(fp.mv_f32(mv::X0)) < 0.0 {
            fp.set_mv_f32(mv::X4, 0.0);
            fp.self_vel.x = 0.0;
            fp.self_vel.y = 0.0;
            self.mh_enter(gobj, ms::POKE2);
            self.fighter_play_sfx(f, SFX_POKE, 127, 64);
        }
    }

    /// `ftMh_Poke2_Anim`: back to his place once the last poke ends.
    #[doc(alias = "ftMh_Poke2_Anim")]
    pub fn mh_poke2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_return(gobj);
        }
    }

    /// `ftMh_Poke1_Phys`: chasing a point `x98`, `x9C` off the nearest opponent, at `x28`
    /// of the distance a frame (all of it within `x2C`).
    #[doc(alias = "ftMh_Poke1_Phys")]
    pub fn mh_poke1_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        if reg(self.game.fighters[f.0].mv_f32(mv::X4)) == 0.0 {
            return;
        }
        self.mh_chase(gobj, attr::X98, attr::X9C);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_354_80152370 => g.mh_enter(gobj, ms::ROCK_CRUSH_UP),
        addr::ROCK_CRUSH_UP_ANIM => g.mh_rock_up_anim(gobj),
        addr::MS_355_8015247C => g.mh_enter(gobj, ms::ROCK_CRUSH_WAIT),
        addr::ROCK_CRUSH_WAIT_ANIM => g.mh_rock_wait_anim(gobj),
        addr::ROCK_CRUSH_WAIT_PHYS => g.mh_rock_wait_phys(gobj),
        addr::MS_356_801525E0 => g.mh_rock_down_enter(gobj),
        addr::MS_357_801526D8 => g.mh_paper_enter(gobj),
        addr::PAPER_CRUSH_PHYS => g.mh_paper_phys(gobj),
        addr::MS_358_80152880 => g.mh_poke_enter(gobj),
        addr::POKE1_ANIM => g.mh_poke1_anim(gobj),
        addr::POKE2_ANIM => g.mh_poke2_anim(gobj),
        addr::POKE1_IASA => g.mh_human_nop(gobj),
        addr::POKE1_PHYS => g.mh_poke1_phys(gobj),
        _ => return false,
    }
    true
}
