//! The attacks from the background (`ftmasterhandbackairplane1.c` to `3.c`: the jet over the
//! stage and back; `ftmasterhandbackcrush0.c`, `1.c`: the ram and the crush, which come out of
//! the background shrunk and grow).

use gekko_fp as fp;

use super::super::super::state::Running;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// `ftMh_BackAirplane1_Anim`: the jet's pass once the takeoff ends.
    #[doc(alias = "ftMh_BackAirplane1_Anim")]
    pub fn mh_airplane1_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.mh_airplane2_enter(gobj);
        }
    }

    /// The jet's pass, `doAnim`: over the nearest opponent's x at `x5C`, still.
    #[doc(alias = "doAnim")]
    pub fn mh_airplane2_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::BACK_AIRPLANE2);
        let pos = self.boss_nearest_pos(gobj);
        let y = self.mh_da_f32(f, attr::X5C);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// `ftMh_BackAirplane2_Anim`: the return once the pass ends.
    #[doc(alias = "ftMh_BackAirplane2_Anim")]
    pub fn mh_airplane2_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.mh_airplane3_enter(gobj);
        }
    }

    /// Toward the nearest opponent's x, at most `x58` more a frame: `ftMh_BackAirplane2_Phys`.
    #[doc(alias = "ftMh_BackAirplane2_Phys")]
    pub fn mh_airplane2_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let max = self.mh_da_f32(f, attr::X58);
        self.boss_follow_x_add(gobj, max);
    }

    /// The jet's return, `ftMh_MS_368_80153A64`: from `x60`, `x64`, still.
    #[doc(alias = "ftMh_MS_368_80153A64")]
    pub fn mh_airplane3_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::BACK_AIRPLANE3);
        let (x, y) = (self.mh_da_f32(f, attr::X60), self.mh_da_f32(f, attr::X64));
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// The ram, `ftMh_MS_369_80153B90`: `x6C` frames (`mv.x0`) from the nearest opponent's x
    /// at `x68`.
    #[doc(alias = "ftMh_MS_369_80153B90")]
    pub fn mh_back_punch_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::BACK_PUNCH);
        let n = self.mh_da_i32(f, attr::X6C);
        self.game.fighters[f.0].set_mv_f32(mv::X0, fp::frsp(f64::from(n)));
        let pos = self.boss_nearest_pos(gobj);
        let y = self.mh_da_f32(f, attr::X68);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// Back to his place once the ram or the crush ends, stopped: `ftMh_BackPunch_Anim`.
    #[doc(alias = "ftMh_BackPunch_Anim")]
    pub fn mh_back_punch_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.mh_return(gobj);
        }
    }

    /// The count down; while it lasts toward the nearest opponent's x (`x58`), then stopped.
    fn mh_back_follow(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        if reg(fp.mv_f32(mv::X0)) > 0.0 {
            let max = self.mh_da_f32(f, attr::X58);
            self.boss_follow_x_add(gobj, max);
        } else {
            self.game.fighters[f.0].self_vel.x = 0.0;
        }
    }

    /// `ftMh_BackPunch_Phys`.
    #[doc(alias = "ftMh_BackPunch_Phys")]
    pub fn mh_back_punch_phys(&mut self, gobj: GObjId) {
        self.mh_back_follow(gobj);
    }

    /// The crush, `ftMh_MS_370_80153D2C`: over the nearest opponent at `x70`, `x74` frames
    /// (`mv.x0`), his model shrunk to `x78` and growing back over `x7C` frames (`mv.x70`, the
    /// step `mv.x58`, the full scale `mv.x64`).
    #[doc(alias = "ftMh_MS_370_80153D2C")]
    pub fn mh_back_crush_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        self.mh_enter(gobj, ms::BACK_CRUSH);
        let pos = self.boss_nearest_pos(gobj);
        let (y, frames, small, grow) = (
            self.mh_da_i32(f, attr::X70),
            self.mh_da_i32(f, attr::X74),
            self.mh_da_f32(f, attr::X78),
            self.mh_da_i32(f, attr::X7C),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = mem(fp::frsp(f64::from(y)));
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(frames)));
        fp.set_mv_i32(mv::X70, grow);
        let scl = self.game.objs.jobjs[root].scale;
        let fp = &mut self.game.fighters[f.0];
        super::wait::set_mv_vec(fp, mv::X64, scl);
        let g = fp::frsp(f64::from(grow));
        let step = |c: f32| fp::fdivs(reg(mem(fp::fsubs(reg(c), reg(small)))), g);
        fp.set_mv_f32(mv::X58, step(scl.x));
        fp.set_mv_f32(mv::X58 + 4, step(scl.y));
        fp.set_mv_f32(mv::X58 + 8, step(scl.z));
        crate::hsd::jobj::set_scale(
            &mut self.game.objs.jobjs,
            root,
            Vec3::new(small, small, small),
        );
        self.game.fighters[f.0].cmd_vars[0] = 1;
    }

    /// `ftMh_BackCrush_Anim`: growing a step a frame while the script's flag is up, the full
    /// scale at the end; back to his place once it ends.
    #[doc(alias = "ftMh_BackCrush_Anim")]
    pub fn mh_back_crush_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let root = self.root_jobj(gobj);
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(mv::X70).wrapping_sub(1);
            fp.set_mv_i32(mv::X70, n);
            let scl = if n < 0 {
                fp.cmd_vars[0] = 0;
                super::wait::mv_vec(fp, mv::X64)
            } else {
                let s = self.game.objs.jobjs[root].scale;
                let d = super::wait::mv_vec(&self.game.fighters[f.0], mv::X58);
                let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
                Vec3::new(add(s.x, d.x), add(s.y, d.y), add(s.z, d.z))
            };
            crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, root, scl);
        }
        if !self.anim_frames_remaining(gobj) {
            self.game.fighters[f.0].self_vel.x = 0.0;
            self.mh_return(gobj);
        }
    }

    /// `ftMh_BackCrush_Phys`: the ram's, kept over the stage.
    #[doc(alias = "ftMh_BackCrush_Phys")]
    pub fn mh_back_crush_phys(&mut self, gobj: GObjId) {
        self.mh_back_follow(gobj);
        self.boss_clamp_to_floor(gobj);
    }

    /// Into the background, then `cb` once the animation ends, `ftMh_MS_371_801541C8`.
    #[doc(alias = "ftMh_MS_371_801541C8")]
    pub fn mh_back_disappear_enter(&mut self, gobj: GObjId, cb: u32) {
        self.mh_enter(gobj, ms::BACK_DISAPPEAR);
        self.game.fighter_mut(gobj).set_mv_i32(mv::X4, cb as i32);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::MS_366_80153820 => g.mh_enter(gobj, ms::BACK_AIRPLANE1),
        addr::BACK_AIRPLANE1_ANIM => g.mh_airplane1_anim(gobj),
        addr::DO_ANIM => g.mh_airplane2_enter(gobj),
        addr::BACK_AIRPLANE2_ANIM => g.mh_airplane2_anim(gobj),
        addr::BACK_AIRPLANE2_PHYS => g.mh_airplane2_phys(gobj),
        addr::MS_368_80153A64 => g.mh_airplane3_enter(gobj),
        addr::MS_369_80153B90 => g.mh_back_punch_enter(gobj),
        addr::BACK_PUNCH_ANIM => g.mh_back_punch_anim(gobj),
        addr::BACK_PUNCH_PHYS => g.mh_back_punch_phys(gobj),
        addr::MS_370_80153D2C => g.mh_back_crush_enter(gobj),
        addr::BACK_CRUSH_ANIM => g.mh_back_crush_anim(gobj),
        addr::BACK_CRUSH_PHYS => g.mh_back_crush_phys(gobj),
        _ => return false,
    }
    true
}
