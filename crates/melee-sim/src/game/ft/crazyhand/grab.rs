//! Crazy Hand's grab (the hands' shared one, `ft::masterhand::grab`, in his states
//! `FingerBeamLoop` to `BackAirplane3`), his vanishing attacks (`ftcrazyhandbackcrush.c`,
//! `ftcrazyhandbackdisappear.c`, `ftcrazyhandwait11.c`) and the stomp (`ftcrazyhandgrab.c`,
//! `ftcrazyhandcancel.c`, `ftcrazyhandsqueezing0.c`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::masterhand::wait::set_mv_vec;
use super::{HAND, addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

impl Running<'_> {
    /// The wind-up of his vanishing attacks, `ftCh_Init_801597F0`: `cb` once it ends.
    #[doc(alias = "ftCh_Init_801597F0")]
    pub fn ch_back_crush_enter(&mut self, gobj: GObjId, cb: u32) {
        self.hand_enter(gobj, ms::BACK_CRUSH);
        self.game.fighter_mut(gobj).set_mv_i32(mv::X4, cb as i32);
    }

    /// `ftCh_BackCrush_Anim`: the callback the wind-up was given once it ends.
    #[doc(alias = "ftCh_BackCrush_Anim")]
    pub fn ch_back_crush_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let cb = self.game.fighter(gobj).mv_i32(mv::X4) as u32;
            super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
        }
    }

    /// Back over the nearest opponent at the height `y` (an attribute's), still.
    fn ch_appear_over_opponent(&mut self, gobj: GObjId, y: f32) {
        let pos = self.boss_nearest_pos(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// The dive, `fn_80159908`: over the nearest opponent at `xEC`'s height, following for
    /// `xF0` frames.
    #[doc(alias = "fn_80159908")]
    pub fn ch_dive_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, ms::BACK_DISAPPEAR);
        let n = super::HAND.i32(self, f, attr::XF0);
        self.game.fighters[f.0].set_mv_f32(mv::X0, fp::frsp(f64::from(n)));
        let y = super::HAND.f32(self, f, attr::XEC);
        self.ch_appear_over_opponent(gobj, y);
    }

    /// `ftCh_BackDisappear_Anim` and `ftCh_Wait1_1_Anim`'s end: still across, back to his
    /// place.
    #[doc(alias = "ftCh_BackDisappear_Anim")]
    pub fn ch_dive_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.hand_return(gobj, &super::HAND);
        }
    }

    /// The count in `mv.x0` (a float) down a frame: while it lasts, following the nearest
    /// opponent at most `xDC` more a frame (`ftBossLib_8015BF74`), else still across.
    fn ch_follow_while(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        if reg(fp.mv_f32(mv::X0)) > 0.0 {
            let max = super::HAND.f32(self, f, attr::XDC);
            self.boss_follow_x_add(gobj, max);
        } else {
            fp.self_vel.x = 0.0;
        }
    }

    /// `ftCh_BackDisappear_Phys`: following while its count lasts.
    #[doc(alias = "ftCh_BackDisappear_Phys")]
    pub fn ch_dive_phys(&mut self, gobj: GObjId) {
        self.ch_follow_while(gobj);
    }

    /// The shrinking stomp, `fn_80159AA4`: over the nearest opponent at `xF4`'s height,
    /// following for `xF8` frames, shrunk to `xFC` and growing back over `x100` frames.
    #[doc(alias = "fn_80159AA4")]
    pub fn ch_shrink_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let j = self.root_jobj(gobj);
        self.hand_enter(gobj, ms::WAIT1_1);
        let y = fp::frsp(f64::from(super::HAND.i32(self, f, attr::XF4)));
        self.ch_appear_over_opponent(gobj, mem(y));
        let frames = super::HAND.i32(self, f, attr::XF8);
        let n = super::HAND.i32(self, f, attr::X100);
        let small = reg(super::HAND.f32(self, f, attr::XFC));
        let scl = self.game.objs.jobjs[j].scale;
        let steps = fp::frsp(f64::from(n));
        let step = |v: f32| mem(fp::fdivs(fp::fsubs(reg(v), small), steps));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(frames)));
        fp.set_mv_i32(mv::SCALE_FRAMES, n);
        set_mv_vec(fp, mv::SCALE, scl);
        let st = Vec3::new(step(scl.x), step(scl.y), step(scl.z));
        set_mv_vec(fp, mv::SCALE_STEP, st);
        let s = mem(small);
        crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, j, Vec3::new(s, s, s));
        self.game.fighters[f.0].cmd_vars[0] = 1;
    }

    /// `ftCh_Wait1_1_Anim`: growing back a step a frame (his old scale once the steps run
    /// out); still across and back to his place once it ends.
    #[doc(alias = "ftCh_Wait1_1_Anim")]
    pub fn ch_shrink_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let j = self.root_jobj(gobj);
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(mv::SCALE_FRAMES).wrapping_sub(1);
            fp.set_mv_i32(mv::SCALE_FRAMES, n);
            let s = if n < 0 {
                fp.cmd_vars[0] = 0;
                super::super::masterhand::wait::mv_vec(fp, mv::SCALE)
            } else {
                let st = super::super::masterhand::wait::mv_vec(fp, mv::SCALE_STEP);
                let s = self.game.objs.jobjs[j].scale;
                let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
                Vec3::new(add(s.x, st.x), add(s.y, st.y), add(s.z, st.z))
            };
            crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, j, s);
        }
        self.ch_dive_anim(gobj);
    }

    /// `ftCh_Wait1_1_Phys`: following while its count lasts, over the stage.
    #[doc(alias = "ftCh_Wait1_1_Phys")]
    pub fn ch_shrink_phys(&mut self, gobj: GObjId) {
        self.ch_follow_while(gobj);
        self.boss_clamp_to_floor(gobj);
    }

    /// The stomp's rise, `ftCh_Init_80159F40`.
    #[doc(alias = "ftCh_Init_80159F40")]
    pub fn ch_stomp_rise_enter(&mut self, gobj: GObjId) {
        self.hand_enter(gobj, ms::GRAB);
    }

    /// `ftCh_Grab_Anim`: the stomp once the rise ends.
    #[doc(alias = "ftCh_Grab_Anim")]
    pub fn ch_stomp_rise_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_stomp_enter(gobj);
        }
    }

    /// The stomp, `ftCh_Init_8015A030`: over the nearest opponent at `xE0`'s height.
    #[doc(alias = "ftCh_Init_8015A030")]
    pub fn ch_stomp_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, ms::CANCEL);
        let y = super::HAND.f32(self, f, attr::XE0);
        self.ch_appear_over_opponent(gobj, y);
    }

    /// `ftCh_Cancel_Anim`: still across, the stomp's end once it ends.
    #[doc(alias = "ftCh_Cancel_Anim")]
    pub fn ch_stomp_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.ch_stomp_end_enter(gobj);
        }
    }

    /// `ftCh_Cancel_Phys`: following the nearest opponent at most `xDC` more a frame.
    #[doc(alias = "ftCh_Cancel_Phys")]
    pub fn ch_stomp_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let max = super::HAND.f32(self, f, attr::XDC);
        self.boss_follow_x_add(gobj, max);
    }

    /// The stomp's end, `ftCh_Init_8015A184`: at `xE4`, `xE8`, still.
    #[doc(alias = "ftCh_Init_8015A184")]
    pub fn ch_stomp_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, ms::SQUEEZING0);
        let (x, y) = (
            super::HAND.f32(self, f, attr::XE4),
            super::HAND.f32(self, f, attr::XE8),
        );
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        INIT_80158B3C => g.hand_reach_enter(gobj, &HAND),
        FINGER_BEAM_LOOP_ANIM => g.hand_reach_anim(gobj, &HAND),
        FINGER_BEAM_LOOP_PHYS => g.hand_reach_phys(gobj, &HAND),
        INIT_80158DFC => g.hand_grab_enter(gobj, &HAND),
        FINGER_BEAM_END_ANIM => g.hand_grab_anim(gobj, &HAND),
        FN_80159288 => g.hand_caught(gobj, &HAND),
        GRAB_UNK1_8015ABD0 => g.hand_close_caught_enter(gobj, &HAND),
        GRAB_UNK1_8015AC50 => g.hand_close_empty_enter(gobj, &HAND),
        FINGER_GUN2_ANIM => g.hand_close_anim(gobj, &HAND),
        INIT_80158F34 => g.hand_miss_enter(gobj, &HAND),
        INIT_80159098 => g.hand_let_go_enter(gobj, &HAND),
        FINGER_GUN1_ANIM => g.hand_let_go_anim(gobj, &HAND),
        INIT_801592D4 => g.hand_squeeze_enter(gobj, &HAND),
        BACK_AIRPLANE1_ANIM => g.hand_squeeze_anim(gobj, &HAND),
        INIT_801594D4 => g.hand_throw_enter(gobj, HAND.grab.throws[0]),
        INIT_80159670 => g.hand_throw_enter(gobj, HAND.grab.throws[1]),
        BACK_AIRPLANE2_ANIM | BACK_AIRPLANE3_ANIM => g.hand_throw_anim(gobj, &HAND),
        BACK_CRUSH_ANIM => g.ch_back_crush_anim(gobj),
        FN_80159908 => g.ch_dive_enter(gobj),
        BACK_DISAPPEAR_ANIM => g.ch_dive_anim(gobj),
        BACK_DISAPPEAR_PHYS => g.ch_dive_phys(gobj),
        FN_80159AA4 => g.ch_shrink_enter(gobj),
        WAIT1_1_ANIM => g.ch_shrink_anim(gobj),
        WAIT1_1_PHYS => g.ch_shrink_phys(gobj),
        INIT_80159F40 => g.ch_stomp_rise_enter(gobj),
        GRAB_ANIM => g.ch_stomp_rise_anim(gobj),
        INIT_8015A030 => g.ch_stomp_enter(gobj),
        CANCEL_ANIM => g.ch_stomp_anim(gobj),
        CANCEL_PHYS => g.ch_stomp_phys(gobj),
        INIT_8015A184 => g.ch_stomp_end_enter(gobj),
        GRAB_UNK1_8015B670 => g.hand_capture_damage_enter(gobj, &HAND),
        CAPTURE_DAMAGE_CRAZY_HAND_ANIM => g.hand_capture_damage_anim(gobj, &HAND),
        GRAB_UNK1_8015B778 => g.hand_capture_wait_enter(gobj, &HAND),
        GRAB_UNK1_8015B800 => g.hand_capture_cut(gobj),
        GRAB_UNK1_8015B850 => g.hand_thrown_enter(gobj, &HAND),
        _ => return false,
    }
    true
}
