//! The hands' attacks from the background and from above: the vanishing wind-up that runs a
//! callback once it ends (Master Hand's `ftmasterhandbackdisappear.c`, Crazy Hand's
//! `ftcrazyhandbackcrush.c`), the pass over the nearest opponent and back (Master Hand's jet,
//! `ftmasterhandbackairplane1.c` to `3.c`; Crazy Hand's stomp, `ftcrazyhandgrab.c`,
//! `ftcrazyhandcancel.c`, `ftcrazyhandsqueezing0.c`), the ram over the nearest opponent for a
//! count (Master Hand's `ftmasterhandbackcrush0.c`, Crazy Hand's dive,
//! `ftcrazyhandbackdisappear.c`) and the shrunk one that grows back (Master Hand's crush,
//! `ftmasterhandbackcrush1.c`; Crazy Hand's shrinking stomp, `ftcrazyhandwait11.c`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::wait::{mv_vec, set_mv_vec};
use super::{HAND, Hand, addr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// The pass: its states (over the nearest opponent, back; the rise before it is entered by
/// its address), the height over him, the most it follows a frame, and the point it comes
/// back from.
pub struct Pass {
    pub states: [i32; 2],
    pub over: usize,
    pub follow: usize,
    pub back: usize,
}

/// The ram and the shrunk ram: their states, the height over the nearest opponent (a float
/// for the ram, a word for the shrunk one), the frames they follow (a word) and the most a
/// frame; the shrunk one's scale (a float) and the frames it grows back over (a word); and
/// where in `mv` the growth keeps its step, his full scale (`+0xC`) and its count (`+0x18`).
pub struct Ram {
    pub states: [i32; 2],
    pub over: [usize; 2],
    pub frames: [usize; 2],
    pub follow: usize,
    pub small: usize,
    pub grow: usize,
    pub growth: usize,
}

impl Running<'_> {
    /// Into the background, then `cb` once the animation ends, `ftMh_MS_371_801541C8` and
    /// `ftCh_Init_801597F0`.
    #[doc(alias = "ftMh_MS_371_801541C8")]
    #[doc(alias = "ftCh_Init_801597F0")]
    pub fn hand_vanish_enter(&mut self, gobj: GObjId, h: &Hand, cb: u32) {
        self.hand_enter(gobj, h.vanish);
        self.game.fighter_mut(gobj).set_mv_i32(mv::X4, cb as i32);
    }

    /// `ftMh_BackDisappear_Anim` and `ftCh_BackCrush_Anim`: the callback the wind-up was given
    /// once it ends.
    #[doc(alias = "ftMh_BackDisappear_Anim")]
    #[doc(alias = "ftCh_BackCrush_Anim")]
    pub fn hand_vanish_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let cb = self.game.fighter(gobj).mv_i32(mv::X4) as u32;
            super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
        }
    }

    /// `ftMh_BackAirplane1_Anim` and `ftCh_Grab_Anim`: the pass once the rise ends.
    #[doc(alias = "ftMh_BackAirplane1_Anim")]
    #[doc(alias = "ftCh_Grab_Anim")]
    pub fn hand_pass_rise_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.hand_pass_enter(gobj, h);
        }
    }

    /// Back over the nearest opponent at the height `y`, still.
    fn hand_over_opponent(&mut self, gobj: GObjId, y: f32) {
        let pos = self.boss_nearest_pos(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.cur_pos.x = pos.x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// The pass, `doAnim` (Master Hand's jet) and `ftCh_Init_8015A030` (Crazy Hand's stomp):
    /// over the nearest opponent at the pass's height, still.
    #[doc(alias = "doAnim")]
    #[doc(alias = "ftCh_Init_8015A030")]
    pub fn hand_pass_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.pass.states[0]);
        let y = h.f32(self, f, h.pass.over);
        self.hand_over_opponent(gobj, y);
    }

    /// `ftMh_BackAirplane2_Anim` and `ftCh_Cancel_Anim`: still across, the way back once the
    /// pass ends.
    #[doc(alias = "ftMh_BackAirplane2_Anim")]
    #[doc(alias = "ftCh_Cancel_Anim")]
    pub fn hand_pass_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.hand_pass_back_enter(gobj, h);
        }
    }

    /// `ftMh_BackAirplane2_Phys` and `ftCh_Cancel_Phys`: following the nearest opponent at
    /// most the pass's most more a frame.
    #[doc(alias = "ftMh_BackAirplane2_Phys")]
    #[doc(alias = "ftCh_Cancel_Phys")]
    pub fn hand_pass_phys(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let max = h.f32(self, f, h.pass.follow);
        self.boss_follow_x_add(gobj, max);
    }

    /// The way back, `ftMh_MS_368_80153A64` and `ftCh_Init_8015A184`: from the pass's point,
    /// still.
    #[doc(alias = "ftMh_MS_368_80153A64")]
    #[doc(alias = "ftCh_Init_8015A184")]
    pub fn hand_pass_back_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.pass.states[1]);
        let p = h.point(self, f, h.pass.back);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = p.x;
        fp.cur_pos.y = p.y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// The ram, `ftMh_MS_369_80153B90` and `fn_80159908` (Crazy Hand's dive): over the nearest
    /// opponent at its height, following for its frames (`mv.x0`).
    #[doc(alias = "ftMh_MS_369_80153B90")]
    #[doc(alias = "fn_80159908")]
    pub fn hand_ram_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.ram.states[0]);
        let n = h.i32(self, f, h.ram.frames[0]);
        self.game.fighters[f.0].set_mv_f32(mv::X0, fp::frsp(f64::from(n)));
        let y = h.f32(self, f, h.ram.over[0]);
        self.hand_over_opponent(gobj, y);
    }

    /// Back to his place once the ram or the shrunk one ends, still across:
    /// `ftMh_BackPunch_Anim` and `ftCh_BackDisappear_Anim`.
    #[doc(alias = "ftMh_BackPunch_Anim")]
    #[doc(alias = "ftCh_BackDisappear_Anim")]
    pub fn hand_ram_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.hand_return(gobj, h);
        }
    }

    /// The count in `mv.x0` (a float) down a frame: while it lasts, following the nearest
    /// opponent at most the ram's most more a frame (`ftBossLib_8015BF74`), else still across.
    fn hand_ram_follow(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        if reg(fp.mv_f32(mv::X0)) > 0.0 {
            let max = h.f32(self, f, h.ram.follow);
            self.boss_follow_x_add(gobj, max);
        } else {
            self.game.fighters[f.0].self_vel.x = 0.0;
        }
    }

    /// `ftMh_BackPunch_Phys` and `ftCh_BackDisappear_Phys`: following while its count lasts.
    #[doc(alias = "ftMh_BackPunch_Phys")]
    #[doc(alias = "ftCh_BackDisappear_Phys")]
    pub fn hand_ram_phys(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_ram_follow(gobj, h);
    }

    /// The shrunk ram, `ftMh_MS_370_80153D2C` (Master Hand's crush) and `fn_80159AA4` (Crazy
    /// Hand's shrinking stomp): over the nearest opponent at its height, following for its
    /// frames (`mv.x0`), his model shrunk and growing back a step a frame over the growth's
    /// frames.
    #[doc(alias = "ftMh_MS_370_80153D2C")]
    #[doc(alias = "fn_80159AA4")]
    pub fn hand_shrink_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let root = self.root_jobj(gobj);
        self.hand_enter(gobj, h.ram.states[1]);
        let y = fp::frsp(f64::from(h.i32(self, f, h.ram.over[1])));
        self.hand_over_opponent(gobj, mem(y));
        let (frames, small, grow) = (
            h.i32(self, f, h.ram.frames[1]),
            h.f32(self, f, h.ram.small),
            h.i32(self, f, h.ram.grow),
        );
        let g = h.ram.growth;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(frames)));
        fp.set_mv_i32(g + 0x18, grow);
        let scl = self.game.objs.jobjs[root].scale;
        let fp = &mut self.game.fighters[f.0];
        set_mv_vec(fp, g + 0xC, scl);
        let steps = fp::frsp(f64::from(grow));
        let step = |c: f32| mem(fp::fdivs(fp::fsubs(reg(c), reg(small)), steps));
        set_mv_vec(fp, g, Vec3::new(step(scl.x), step(scl.y), step(scl.z)));
        crate::hsd::jobj::set_scale(
            &mut self.game.objs.jobjs,
            root,
            Vec3::new(small, small, small),
        );
        self.game.fighters[f.0].cmd_vars[0] = 1;
    }

    /// `ftMh_BackCrush_Anim` and `ftCh_Wait1_1_Anim`: growing a step a frame while the
    /// script's flag is up (his full scale once the steps run out); still across and back to
    /// his place once it ends.
    #[doc(alias = "ftMh_BackCrush_Anim")]
    #[doc(alias = "ftCh_Wait1_1_Anim")]
    pub fn hand_shrink_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            let root = self.root_jobj(gobj);
            let g = h.ram.growth;
            let fp = &mut self.game.fighters[f.0];
            let n = fp.mv_i32(g + 0x18).wrapping_sub(1);
            fp.set_mv_i32(g + 0x18, n);
            let scl = if n < 0 {
                fp.cmd_vars[0] = 0;
                mv_vec(fp, g + 0xC)
            } else {
                let d = mv_vec(fp, g);
                let s = self.game.objs.jobjs[root].scale;
                let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
                Vec3::new(add(s.x, d.x), add(s.y, d.y), add(s.z, d.z))
            };
            crate::hsd::jobj::set_scale(&mut self.game.objs.jobjs, root, scl);
        }
        self.hand_ram_anim(gobj, h);
    }

    /// `ftMh_BackCrush_Phys` and `ftCh_Wait1_1_Phys`: the ram's, kept over the stage.
    #[doc(alias = "ftMh_BackCrush_Phys")]
    #[doc(alias = "ftCh_Wait1_1_Phys")]
    pub fn hand_shrink_phys(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_ram_follow(gobj, h);
        self.boss_clamp_to_floor(gobj);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::BACK_AIRPLANE1_ENTER => g.hand_enter(gobj, ms::BACK_AIRPLANE1),
        addr::BACK_AIRPLANE1_ANIM => g.hand_pass_rise_anim(gobj, &HAND),
        addr::DO_ANIM => g.hand_pass_enter(gobj, &HAND),
        addr::BACK_AIRPLANE2_ANIM => g.hand_pass_anim(gobj, &HAND),
        addr::BACK_AIRPLANE2_PHYS => g.hand_pass_phys(gobj, &HAND),
        addr::PASS_BACK_ENTER => g.hand_pass_back_enter(gobj, &HAND),
        addr::RAM_ENTER => g.hand_ram_enter(gobj, &HAND),
        addr::BACK_PUNCH_ANIM => g.hand_ram_anim(gobj, &HAND),
        addr::BACK_PUNCH_PHYS => g.hand_ram_phys(gobj, &HAND),
        addr::SHRINK_ENTER => g.hand_shrink_enter(gobj, &HAND),
        addr::BACK_CRUSH_ANIM => g.hand_shrink_anim(gobj, &HAND),
        addr::BACK_CRUSH_PHYS => g.hand_shrink_phys(gobj, &HAND),
        addr::BACK_DISAPPEAR_ANIM => g.hand_vanish_anim(gobj),
        _ => return false,
    }
    true
}
