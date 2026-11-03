//! Crazy Hand's attacks that hold no one: the rise and the slide from the stage's left end
//! (`ftcrazyhandentry.c`'s `fn_80156F6C`, `ftcrazyhanddamage0.c`), the sweep and the slap
//! (`ftcrazyhandsweep.c`, `ftcrazyhandsweepwait.c`), the walk (`ftcrazyhandwalk.c`), the drill
//! (`ftcrazyhanddrill.c`), the rising crush (`ftcrazyhandrockcrush0.c`, `1.c`), the poke with
//! its lasers (`ftcrazyhandpoke.c`) and the bombs (`ftcrazyhandfingerbeam0.c`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::it::kinds::{crazyhandbomb, masterhandlaser};
use super::super::super::state::Running;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `FtPart_RLegJA`, `BustN`, `LHandN` and `L3rdNa`: the fingers his lasers and bombs come
/// from.
const FINGERS: [i32; 4] = [11, 16, 21, 26];

/// The slide's and the crush's thud (`ft_PlaySFX`).
const SFX_THUD: i32 = 0x4E207;
/// A bomb's drop.
const SFX_BOMB: i32 = 0x4E203;
/// The poke's hums (`lbAudioAx_800237A8`).
const SFX_POKE: [i32; 3] = [320004, 320005, 320006];

impl Running<'_> {
    /// The rise at the stage's left end, `fn_80156F6C`: at `x20`'s height.
    #[doc(alias = "fn_80156F6C")]
    pub fn ch_rise_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::ENTRY);
        let y = self.ch_da_f32(f, attr::X20);
        self.game.fighters[f.0].cur_pos.y = y;
    }

    /// The slide's wind-up at the stage's left end, `fn_80157080`.
    #[doc(alias = "fn_80157080")]
    pub fn ch_slide_windup_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::DAMAGE);
    }

    /// `ftCh_Damage_Anim`: the slide once the wind-up ends.
    #[doc(alias = "ftCh_Damage_Anim")]
    pub fn ch_slide_windup_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slide_enter(gobj);
        }
    }

    /// A stretch of the slide, `ftCh_Init_80157170`: gliding `x28` to his left at `x24`'s
    /// height, the thud.
    #[doc(alias = "ftCh_Init_80157170")]
    pub fn ch_slide_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let (dx, y) = (self.ch_da_f32(f, attr::X28), self.ch_da_f32(f, attr::X24));
        let fp = &mut self.game.fighters[f.0];
        let x = mem(fp::fsubs(reg(fp.cur_pos.x), reg(dx)));
        fp.set_mv_f32(mv::TARGET, reg(x));
        fp.set_mv_f32(mv::TARGET + 4, reg(y));
        fp.set_mv_f32(mv::TARGET + 8, 0.0);
        self.ch_enter(gobj, ms::DAMAGE2);
        self.fighter_play_sfx(f, SFX_THUD, 127, 64);
    }

    /// `ftCh_Damage2_Anim`: another stretch, or its end past the stage's right end.
    #[doc(alias = "ftCh_Damage2_Anim")]
    pub fn ch_slide_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        let right = self.floor_right0();
        if self.game.fighter(gobj).cur_pos.x > right.x {
            self.ch_slide_end_enter(gobj);
        } else {
            self.ch_slide_enter(gobj);
        }
    }

    /// The slide's end, `ftCh_Init_8015737C`.
    #[doc(alias = "ftCh_Init_8015737C")]
    pub fn ch_slide_end_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::WAIT_SWEEP);
    }

    /// The sweep, `fn_8015746C`.
    #[doc(alias = "fn_8015746C")]
    pub fn ch_sweep_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::SWEEP_LOOP);
    }

    /// The slap's wind-up, `fn_8015755C`.
    #[doc(alias = "fn_8015755C")]
    pub fn ch_slap_windup_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::SWEEP_WAIT);
    }

    /// `ftCh_SweepWait_Anim`: the slap once the wind-up ends.
    #[doc(alias = "ftCh_SweepWait_Anim")]
    pub fn ch_slap_windup_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slap_enter(gobj);
        }
    }

    /// The slap across the stage, `ftCh_Init_8015764C`: its end watched by the accessory.
    #[doc(alias = "ftCh_Init_8015764C")]
    pub fn ch_slap_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::SLAP);
        self.game.fighter_mut(gobj).accessory4_cb = Some(CodeRef(addr::FN_801577B4));
    }

    /// `ftCh_Slap_Anim`: the slap's animation again when it ends.
    #[doc(alias = "ftCh_Slap_Anim")]
    pub fn ch_slap_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_slap_enter(gobj);
        }
    }

    /// `ftCh_Slap_Phys`: the air motion and `x40` a frame.
    #[doc(alias = "ftCh_Slap_Phys")]
    pub fn ch_slap_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let dx = self.ch_da_f32(f, attr::X40);
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = mem(fp::fadds(reg(fp.self_vel.x), reg(dx)));
    }

    /// The slap's end past `x3C`, `fn_801577B4` (its accessory): still, its end.
    #[doc(alias = "fn_801577B4")]
    pub fn ch_slap_watch(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let end = self.ch_da_f32(f, attr::X3C);
        if self.game.fighters[f.0].cur_pos.x > end {
            self.game.fighters[f.0].self_vel.x = 0.0;
            self.ch_slap_end_enter(gobj);
        }
    }

    /// The slap's end, `ftCh_Init_801577F8`.
    #[doc(alias = "ftCh_Init_801577F8")]
    pub fn ch_slap_end_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::WALK2);
    }

    /// The walk's start, `fn_801578E8`.
    #[doc(alias = "fn_801578E8")]
    pub fn ch_walk_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::WALK_LOOP);
    }

    /// `ftCh_WalkLoop_Anim`: the walk for `x64` frames once the start ends.
    #[doc(alias = "ftCh_WalkLoop_Anim")]
    pub fn ch_walk_start_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let f = self.game.fighter_id(gobj);
            self.ch_walk_wait_enter(gobj);
            let n = self.ch_da_i32(f, attr::X64);
            self.game.fighters[f.0].set_mv_i32(mv::X8, n);
        }
    }

    /// The walk, `ftCh_Init_801579F4`.
    #[doc(alias = "ftCh_Init_801579F4")]
    pub fn ch_walk_wait_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::WALK_WAIT);
    }

    /// `ftCh_WalkWait_Anim`: the walk's end once its count runs out; its animation again.
    #[doc(alias = "ftCh_WalkWait_Anim")]
    pub fn ch_walk_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.ch_walk_end_enter(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.ch_enter(gobj, ms::WALK_WAIT);
        }
    }

    /// `ftCh_WalkWait_Phys`: the air motion, following the nearest opponent at most `x68` a
    /// frame, over the stage.
    #[doc(alias = "ftCh_WalkWait_Phys")]
    pub fn ch_walk_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let max = self.ch_da_f32(f, attr::X68);
        self.boss_follow_x(gobj, max);
        self.boss_clamp_to_floor(gobj);
    }

    /// The walk's end, `ftCh_Init_80157B58`: still.
    #[doc(alias = "ftCh_Init_80157B58")]
    pub fn ch_walk_end_enter(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).self_vel.x = 0.0;
        self.ch_enter(gobj, ms::WALK_SHOOT);
    }

    /// The drill, `fn_80157C50`: its frame count from 0.
    #[doc(alias = "fn_80157C50")]
    pub fn ch_drill_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::DRILL);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 0.0);
    }

    /// `ftCh_Drill_Phys`: the air motion; between frames `x74` and `x78` following the
    /// nearest opponent at most `x7C` a frame, else still; over the stage.
    #[doc(alias = "ftCh_Drill_Phys")]
    pub fn ch_drill_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let (lo, hi) = (self.ch_da_i32(f, attr::X74), self.ch_da_i32(f, attr::X78));
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fadds(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(lo)) && n < fp::frsp(f64::from(hi)) {
            let max = self.ch_da_f32(f, attr::X7C);
            self.boss_follow_x(gobj, max);
        } else {
            fp.self_vel.x = 0.0;
        }
        self.boss_clamp_to_floor(gobj);
    }

    /// The rising crush, `ftCh_Init_80157DF8`: `x58` and up to `x54` more rises at random,
    /// not following yet.
    #[doc(alias = "ftCh_Init_80157DF8")]
    pub fn ch_rock_up_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::ROCK_CRUSH_UP);
        let (hi, lo) = (self.ch_da_i32(f, attr::X54), self.ch_da_i32(f, attr::X58));
        let n = lo.wrapping_add(self.game.objs.rng.below(hi.wrapping_sub(lo)));
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X0, fp::frsp(f64::from(n)));
        fp.set_mv_f32(mv::X4, 0.0);
    }

    /// `ftCh_RockCrushUp_Anim`: once the rise ends, following; once the rises run out, still,
    /// the crush and its thud.
    #[doc(alias = "ftCh_RockCrushUp_Anim")]
    pub fn ch_rock_up_anim(&mut self, gobj: GObjId) {
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
            self.ch_enter(gobj, ms::ROCK_CRUSH_WAIT);
            self.fighter_play_sfx(f, SFX_THUD, 127, 64);
        }
    }

    /// `ftCh_RockCrushUp_Phys` (the rise's and the crush's): the air motion; while following,
    /// chasing `x5C`, `x60` off the nearest opponent.
    #[doc(alias = "ftCh_RockCrushUp_Phys")]
    pub fn ch_rock_up_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        if reg(self.game.fighter(gobj).mv_f32(mv::X4)) != 0.0 {
            self.ch_chase(gobj, attr::X5C, attr::X60);
        }
    }

    /// Chasing a point `off` off the nearest opponent at `x10` of the distance a frame (all
    /// of it within `x14`).
    // NaN takes the original's branch (`len < x14` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub(crate) fn ch_chase(&mut self, gobj: GObjId, off_x: usize, off_y: usize) {
        let f = self.game.fighter_id(gobj);
        let mut pos = self.boss_nearest_pos(gobj);
        pos.x = mem(fp::fadds(reg(pos.x), reg(self.ch_da_f32(f, off_x))));
        pos.y = mem(fp::fadds(reg(pos.y), reg(self.ch_da_f32(f, off_y))));
        pos.z = 0.0;
        let (near, rate) = (self.ch_da_f32(f, attr::X14), self.ch_da_f32(f, attr::X10));
        let fp = &mut self.game.fighters[f.0];
        let mut vel = pos - fp.cur_pos;
        let (x, y, z) = (reg(vel.x), reg(vel.y), reg(vel.z));
        let len = crate::math::sqrt_reg(fp::fadds(
            fp::fmuls(z, z),
            fp::fadds(fp::fmuls(x, x), fp::fmuls(y, y)),
        ));
        if !(len < reg(near)) {
            vel.normalize();
            let speed = fp::fmuls(len, reg(rate));
            vel.x = mem(fp::fmuls(reg(vel.x), speed));
            vel.y = mem(fp::fmuls(reg(vel.y), speed));
            vel.z = mem(fp::fmuls(reg(vel.z), speed));
        }
        fp.self_vel.x = vel.x;
        fp.self_vel.y = vel.y;
    }

    /// The falling crush, `fn_80158144`: its frame count from 107.
    #[doc(alias = "fn_80158144")]
    pub fn ch_rock_down_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::ROCK_CRUSH_DOWN);
        self.game.fighter_mut(gobj).set_mv_f32(mv::X0, 107.0);
    }

    /// `ftCh_RockCrushDown_Phys`: the air motion; at most `x48` frames before its end
    /// following the nearest opponent at most `x44` a frame, else still; over the stage.
    #[doc(alias = "ftCh_RockCrushDown_Phys")]
    pub fn ch_rock_down_phys(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.air_anim_motion(gobj);
        let late = self.ch_da_i32(f, attr::X48);
        let fp = &mut self.game.fighters[f.0];
        let n = fp::fsubs(reg(fp.mv_f32(mv::X0)), 1.0);
        fp.set_mv_f32(mv::X0, n);
        let n = reg(fp.mv_f32(mv::X0));
        if n > fp::frsp(f64::from(late)) || n < 0.0 {
            fp.self_vel.x = 0.0;
        } else {
            let max = self.ch_da_f32(f, attr::X44);
            self.boss_follow_x(gobj, max);
        }
        self.boss_clamp_to_floor(gobj);
    }

    /// The poke's wind-up, `fn_801582D8`: no hums yet.
    #[doc(alias = "fn_801582D8")]
    pub fn ch_poke_windup_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::PAPER_CRUSH);
        let fp = self.game.fighter_mut(gobj);
        for off in mv::HUMS {
            fp.set_mv_i32(off, -1);
        }
    }

    /// `ftCh_PaperCrush_Anim`: the poke once the wind-up ends.
    #[doc(alias = "ftCh_PaperCrush_Anim")]
    pub fn ch_poke_windup_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.ch_poke_enter(gobj);
        }
    }

    /// The poke, `ftCh_Init_801583E4`: its lasers made by the accessory on its first frame.
    #[doc(alias = "ftCh_Init_801583E4")]
    pub fn ch_poke_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::POKE1);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory4_cb = Some(CodeRef(addr::FN_80158534));
        fp.cmd_vars[0] = 1;
    }

    /// `ftCh_Poke1_Anim`: the poke's end once it ends, its lasers ended.
    #[doc(alias = "ftCh_Poke1_Anim")]
    pub fn ch_poke_anim(&mut self, gobj: GObjId) {
        if self.anim_frames_remaining(gobj) {
            return;
        }
        self.ch_poke_end_enter(gobj);
        let f = self.game.fighter_id(gobj);
        for off in mv::LASERS {
            let l = self.game.fighters[f.0].mv.gobj_at(off);
            masterhandlaser::end(self, l);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in mv::LASERS {
            fp.mv.set_gobj_at(off, None);
        }
    }

    /// The poke's lasers (Master Hand's kind) from his four fingers and their hums on the
    /// accessory's first frame, `fn_80158534`.
    #[doc(alias = "fn_80158534")]
    pub fn ch_poke_lasers(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] == 0 {
            return;
        }
        // Master Hand's kind: without a Master Hand in the match nothing registered its
        // article, and the item's creation reads it through null (docs/melee-bugs.md).
        let kind = masterhandlaser::KIND;
        if super::super::super::it::item::article_of(self.game, self.data, kind).is_none() {
            panic!(
                "Melee crashes: Crazy Hand's poke makes Master Hand's laser (item kind {kind}), whose article no Master Hand registered, and Item_80267AA8 reads it at address 0x10"
            );
        }
        for (off, part) in mv::LASERS.into_iter().zip(FINGERS) {
            let v = self.ch_part_pos(gobj, part);
            let facing = self.game.fighters[f.0].facing_dir;
            let l = masterhandlaser::spawn(self, gobj, v, v, part, kind, facing);
            self.game.fighters[f.0].mv.set_gobj_at(off, l);
        }
        for (off, id) in mv::HUMS.into_iter().zip(SFX_POKE) {
            let h = self.sfx_play(id, 127, 64);
            self.game.fighters[f.0].set_mv_i32(off, h);
        }
        self.game.fighters[f.0].cmd_vars[0] = 0;
    }

    /// The poke's end, `ftCh_Init_8015868C`: the hums stopped.
    #[doc(alias = "ftCh_Init_8015868C")]
    pub fn ch_poke_end_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::POKE2);
        self.ch_stop_hums(gobj);
    }

    /// The poke's hums stopped and forgotten.
    pub(crate) fn ch_stop_hums(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        for off in mv::HUMS {
            let h = self.game.fighters[f.0].mv_i32(off);
            self.sfx_stop(h);
        }
        let fp = &mut self.game.fighters[f.0];
        for off in mv::HUMS {
            fp.set_mv_i32(off, -1);
        }
    }

    /// The bombs' throw, `fn_801587B0`: the bombs dropped by the accessory on the script's
    /// four flags. The original doesn't run the new animation's first frame here.
    #[doc(alias = "fn_801587B0")]
    pub fn ch_bombs_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::FINGER_BEAM_START, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.cmd_vars[..4].fill(0);
        fp.accessory4_cb = Some(CodeRef(addr::FN_801588B8));
    }

    /// A bomb from each finger whose flag the script set, `fn_801588B8` (the bombs' throw's
    /// accessory): from the finger toward the attributes' offset (`xA0` to `xBC`), with its
    /// sound.
    #[doc(alias = "fn_801588B8")]
    pub fn ch_bombs(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let offsets = [
            (attr::XA0, attr::XA4),
            (attr::XA8, attr::XAC),
            (attr::XB0, attr::XB4),
            (attr::XB8, attr::XBC),
        ];
        for (i, (part, (ox, oy))) in FINGERS.into_iter().zip(offsets).enumerate() {
            if self.game.fighters[f.0].cmd_vars[i] == 0 {
                continue;
            }
            let from = self.ch_part_pos(gobj, part);
            let mut to = from;
            to.x = mem(fp::fadds(reg(to.x), reg(self.ch_da_f32(f, ox))));
            to.y = mem(fp::fadds(reg(to.y), reg(self.ch_da_f32(f, oy))));
            let facing = self.game.fighters[f.0].facing_dir;
            crazyhandbomb::spawn(self, gobj, to, from, crazyhandbomb::KIND, facing);
            self.game.fighters[f.0].cmd_vars[i] = 0;
            self.fighter_play_sfx(f, SFX_BOMB, 0x7F, 0x40);
        }
    }

    /// Where his part `part`'s joint is (`lb_8000B1CC` of `fp->parts[part]`).
    pub(crate) fn ch_part_pos(&mut self, gobj: GObjId, part: i32) -> Vec3 {
        let parts = self
            .game
            .fighter(gobj)
            .parts
            .expect("a fighter without parts");
        let j = self.game.fighter_parts[parts.0][part as usize].joint;
        crate::lb::joint::world_position(&mut self.game.objs, self.data, j, None)
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        FN_80156F6C => g.ch_rise_enter(gobj),
        FN_80157080 => g.ch_slide_windup_enter(gobj),
        DAMAGE_ANIM => g.ch_slide_windup_anim(gobj),
        INIT_80157170 => g.ch_slide_enter(gobj),
        DAMAGE2_ANIM => g.ch_slide_anim(gobj),
        DAMAGE2_PHYS => g.ch_update_motion(gobj),
        INIT_8015737C => g.ch_slide_end_enter(gobj),
        FN_8015746C => g.ch_sweep_enter(gobj),
        FN_8015755C => g.ch_slap_windup_enter(gobj),
        SWEEP_WAIT_ANIM => g.ch_slap_windup_anim(gobj),
        INIT_8015764C => g.ch_slap_enter(gobj),
        SLAP_ANIM => g.ch_slap_anim(gobj),
        SLAP_PHYS => g.ch_slap_phys(gobj),
        FN_801577B4 => g.ch_slap_watch(gobj),
        INIT_801577F8 => g.ch_slap_end_enter(gobj),
        FN_801578E8 => g.ch_walk_enter(gobj),
        WALK_LOOP_ANIM => g.ch_walk_start_anim(gobj),
        INIT_801579F4 => g.ch_walk_wait_enter(gobj),
        WALK_WAIT_ANIM => g.ch_walk_anim(gobj),
        WALK_WAIT_PHYS => g.ch_walk_phys(gobj),
        INIT_80157B58 => g.ch_walk_end_enter(gobj),
        FN_80157C50 => g.ch_drill_enter(gobj),
        DRILL_PHYS => g.ch_drill_phys(gobj),
        INIT_80157DF8 => g.ch_rock_up_enter(gobj),
        ROCK_CRUSH_UP_ANIM => g.ch_rock_up_anim(gobj),
        ROCK_CRUSH_UP_PHYS => g.ch_rock_up_phys(gobj),
        FN_80158144 => g.ch_rock_down_enter(gobj),
        ROCK_CRUSH_DOWN_PHYS => g.ch_rock_down_phys(gobj),
        FN_801582D8 => g.ch_poke_windup_enter(gobj),
        PAPER_CRUSH_ANIM => g.ch_poke_windup_anim(gobj),
        INIT_801583E4 => g.ch_poke_enter(gobj),
        POKE1_ANIM => g.ch_poke_anim(gobj),
        FN_80158534 => g.ch_poke_lasers(gobj),
        INIT_8015868C => g.ch_poke_end_enter(gobj),
        FN_801587B0 => g.ch_bombs_enter(gobj),
        FN_801588B8 => g.ch_bombs(gobj),
        _ => return false,
    }
    true
}
