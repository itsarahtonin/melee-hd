//! Master Hand's grab (`ftmasterhandbackdisappear.c`'s reach and grab, `ftmasterhandwait11.c`,
//! `ftmasterhandsqueezing.c`, `ftmasterhandsqueeze.c`, `ftmasterhandthrow.c`,
//! `ftmasterhandslam.c`) and the grabbed fighter's states (`ftmasterhandtagcancel.c`'s
//! capture, `ftmasterhandcapture*.c`, `ftmasterhandthrownmasterhand.c`): he reaches toward
//! the nearest opponent, closes his hand, squeezes, then throws or slams; a victim mashing
//! free makes him let go.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::ids::ms as co_ms;
use super::wait::set_mv_vec;
use super::{addr, attr, ms, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCo_800DB464`, `ftCo_800DE508`: a held and a thrown fighter's accessories.
const HELD_FOLLOW: u32 = 0x800D_B464;
const THROWN_FOLLOW: u32 = 0x800D_E508;

impl Running<'_> {
    /// `ftMh_BackDisappear_Anim`: the callback the disappearance was given once it ends.
    #[doc(alias = "ftMh_BackDisappear_Anim")]
    pub fn mh_back_disappear_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let cb = self.game.fighter(gobj).mv_i32(mv::X4) as u32;
            super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
        }
    }

    /// The reach, `ftMh_MS_372_801542E0`: his wait's frame kept at `x110.y`'s rate, for
    /// `x110.x` frames (`mv.x8`).
    #[doc(alias = "ftMh_MS_372_801542E0")]
    pub fn mh_reach_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::WAIT1_1, 0, frame, 1.0, 0.0, None);
        let (n, rate) = (
            self.mh_da_f32(f, attr::X110),
            self.mh_da_f32(f, attr::X110 + 4),
        );
        self.set_anim_rate(gobj, rate);
        self.game.fighters[f.0].set_mv_i32(mv::X8, fp::fctiwz(reg(n)));
    }

    /// `ftMh_Wait1_1_Anim`: the grab once the count runs out; the reach's animation again.
    #[doc(alias = "ftMh_Wait1_1_Anim")]
    pub fn mh_reach_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.set_anim_rate(gobj, 1.0);
            self.mh_grab_enter(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.mh_enter(gobj, ms::WAIT1_1);
        }
    }

    /// Chasing a point `off` off the nearest opponent at `x28` of the distance a frame (all
    /// of it within `x2C`).
    // NaN takes the original's branch (`len < x2C` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub(crate) fn mh_chase(&mut self, gobj: GObjId, off_x: usize, off_y: usize) {
        let f = self.game.fighter_id(gobj);
        let mut pos = self.boss_nearest_pos(gobj);
        pos.x = mem(fp::fadds(reg(pos.x), reg(self.mh_da_f32(f, off_x))));
        pos.y = mem(fp::fadds(reg(pos.y), reg(self.mh_da_f32(f, off_y))));
        pos.z = 0.0;
        let (near, rate) = (self.mh_da_f32(f, attr::X2C), self.mh_da_f32(f, attr::X28));
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

    /// `ftMh_Wait1_1_Phys`: the air motion, chasing `x108`, `x10C` off the nearest opponent.
    #[doc(alias = "ftMh_Wait1_1_Phys")]
    pub fn mh_reach_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.mh_chase(gobj, attr::X108, attr::X10C);
    }

    /// The grab, `ftMh_MS_373_801545A0`: his grab box's callbacks (`ftCommon_8007E2D0`:
    /// 128, caught `ftMh_MS_375_80154A2C`, the victim `ftMh_MS_388_80155A58`), holding no one.
    #[doc(alias = "ftMh_MS_373_801545A0")]
    pub fn mh_grab_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::GRAB);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 128;
        fp.grab_cb = Some(CodeRef(addr::MS_375_80154A2C));
        fp.grabbed_cb = Some(CodeRef(addr::MS_388_80155A58));
        fp.x2194 = None;
        fp.set_mv_i32(mv::HOLDING, 0);
    }

    /// `ftMh_Grab_Anim`: still, then closing his hand once it ends.
    #[doc(alias = "ftMh_Grab_Anim")]
    pub fn mh_grab_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel = Vec3::new(0.0, 0.0, 0.0);
            self.mh_squeezing1_enter(gobj);
        }
    }

    /// His place as the glide's point (`x30`).
    fn mh_target_home(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let p = self.mh_da_vec2(f, attr::X30);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The miss, `ftMh_MS_374_801546D8`: back toward his place.
    #[doc(alias = "ftMh_MS_374_801546D8")]
    pub fn mh_fail_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::FAIL);
        self.mh_target_home(gobj);
    }

    /// The glide toward `mv.xC` with the air motion (`ftMh_UpdateBossMotion`): the miss's,
    /// the let-go's, the closing hand's and the squeeze's physics.
    #[doc(alias = "ftMh_Fail_Phys")]
    // Ports `ftMh_Cancel_Phys`; aliased once a check reaches it.
    #[doc(alias = "ftMh_Squeezing_Phys")]
    #[doc(alias = "ftMh_Squeeze_Phys")]
    pub fn mh_glide_motion(&mut self, gobj: GObjId) {
        self.mh_update_motion(gobj);
    }

    /// Letting go, `ftMh_MS_381_8015483C`: the victim freed after `x120` frames (`mv.x24`),
    /// still, back toward his place.
    // Ports `ftMh_MS_381_8015483C`; aliased once a check reaches it.
    pub fn mh_cancel_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::CANCEL);
        let t = self.mh_da_f32(f, attr::X120);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X24, reg(t));
        fp.cmd_vars[0] = 1;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        self.mh_target_home(gobj);
    }

    /// `ftMh_Cancel_Anim`: the victim cut loose once the count runs out; back to his place
    /// once it ends.
    // Ports `ftMh_Cancel_Anim`; aliased once a check reaches it.
    pub fn mh_cancel_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let t = fp::fsubs(reg(fp.mv_f32(mv::X24)), 1.0);
        fp.set_mv_f32(mv::X24, t);
        if reg(fp.mv_f32(mv::X24)) <= 0.0 && fp.cmd_vars[0] != 0 {
            let victim = fp.victim_gobj.expect("Master Hand lets go of no one");
            self.mh_capture_cut(victim);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::HOLDING, 0);
            fp.cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            self.mh_return(gobj);
        }
    }

    /// Stopped where the glide ends: `ftMh_Cancel_Coll` and `ftMh_Squeeze_Coll`.
    // Ports `ftMh_Cancel_Coll`; aliased once a check reaches it.
    #[doc(alias = "ftMh_Squeeze_Coll")]
    pub fn mh_stop_at_target(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(mv::DISTANCE)) == 0.0 {
            fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        }
    }

    /// A fighter caught, `ftMh_MS_375_80154A2C` (the grab's callback): still, holding it, the
    /// grab box off, closing his hand.
    #[doc(alias = "ftMh_MS_375_80154A2C")]
    pub fn mh_caught(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.set_mv_i32(mv::HOLDING, 1);
        fp.x221E_b6 = false;
        self.mh_squeezing0_enter(gobj);
    }

    /// The hand closing on a catch, `ftMh_MS_375_80154C78`: toward `x118`.
    #[doc(alias = "ftMh_MS_375_80154C78")]
    pub fn mh_squeezing0_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.mh_enter(gobj, ms::SQUEEZING0);
        let p = self.mh_da_vec2(f, attr::X118);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The hand closing on nothing yet, `ftMh_MS_378_80154CF8`: toward his place.
    #[doc(alias = "ftMh_MS_378_80154CF8")]
    pub fn mh_squeezing1_enter(&mut self, gobj: GObjId) {
        self.mh_enter(gobj, ms::SQUEEZING1);
        self.mh_target_home(gobj);
    }

    /// `ftMh_Squeezing_Anim`: the squeeze if he holds someone, else the miss.
    #[doc(alias = "ftMh_Squeezing_Anim")]
    pub fn mh_squeezing_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::HOLDING) == 1 {
                self.mh_squeeze_enter(gobj);
            } else {
                self.mh_fail_enter(gobj);
            }
        }
    }

    /// The squeeze, `ftMh_MS_378_80154A78`: the victim's damage state, the throw's angle
    /// (511, `ftCommon_8007E2F4`), still, toward `x118`.
    #[doc(alias = "ftMh_MS_378_80154A78")]
    pub fn mh_squeeze_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[1] = 0;
        self.mh_enter(gobj, ms::SQUEEZE);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b2 = true;
        fp.x1A6A = 511;
        self.stop_dead(gobj);
        let victim = self.game.fighters[f.0]
            .victim_gobj
            .expect("Master Hand squeezes no one");
        self.mh_capture_damage_enter(victim);
        let p = self.mh_da_vec2(f, attr::X118);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// `ftMh_Squeeze_Anim`: a cry on the script's flag; then the throw or the slam, as he
    /// chose.
    #[doc(alias = "ftMh_Squeeze_Anim")]
    pub fn mh_squeeze_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighters[f.0].u_i32(u::NEXT_ATTACK) == ms::THROW {
                self.mh_throw_enter(gobj, ms::THROW);
            } else {
                self.mh_throw_enter(gobj, ms::SLAM);
            }
        }
    }

    /// The throw (`ftMh_MS_376_80154E78`) or the slam (`ftMh_MS_379_80155014`).
    #[doc(alias = "ftMh_MS_376_80154E78")]
    #[doc(alias = "ftMh_MS_379_80155014")]
    pub fn mh_throw_enter(&mut self, gobj: GObjId, msid: i32) {
        self.mh_enter(gobj, msid);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
    }

    /// `ftMh_Throw_Anim` and `ftMh_Slam_Anim`: on the script's flag the victim is thrown
    /// (`ftCo_800DE2A8`, `ftCo_800DE7C0`; the throw turns it around); back to his place once
    /// it ends.
    #[doc(alias = "ftMh_Throw_Anim")]
    #[doc(alias = "ftMh_Slam_Anim")]
    pub fn mh_throw_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.game.fighters[f.0].cmd_vars[0] = 0;
            let throw = self.game.fighters[f.0].motion_id == ms::THROW;
            let victim = self.game.fighters[f.0].victim_gobj;
            self.mh_thrown_enter(victim.expect("Master Hand throws no one"));
            if let Some(v) = self.game.fighters[f.0].victim_gobj {
                self.game.fighters[f.0].x1A6A = 0;
                self.throw_hit(gobj, v, true);
                if throw {
                    let vf = self.game.fighter_mut(v);
                    vf.dmg.facing_dir_1 = mem(fp::fmuls(reg(vf.dmg.facing_dir_1), -1.0));
                }
                self.thrown_launch(v, None, false);
            }
            self.game.fighters[f.0].set_mv_i32(mv::HOLDING, 0);
        }
        if !self.anim_frames_remaining(gobj) {
            self.mh_return(gobj);
        }
    }

    /// Caught by Master Hand, `ftMh_MS_388_80155A58` (the grab's victim callback): it lets go
    /// of what it held, faces him, its mash timer from his own damage and rank
    /// (`ftCo_800DA824` of the hand), held at his hand, invisible, airborne.
    #[doc(alias = "ftMh_MS_388_80155A58")]
    pub fn mh_capture_enter(&mut self, gobj: GObjId, hand: GObjId) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let hf = self.game.fighter(hand).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(hand);
        fp.victim_gobj = Some(hand);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = hf;
        let timer = mem(self.grab_escape_time(hand));
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = timer;
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        self.attach_to_thrower(hand, gobj);
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        self.go_airborne(f);
        self.change_motion_state(gobj, co_ms::CAPTURE_MASTER_HAND, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.x2220_b3 = true;
        fp.x1A6A = 511;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// Squeezed, `ftMh_CaptureMasterHand_80155B80`.
    #[doc(alias = "ftMh_CaptureMasterHand_80155B80")]
    pub fn mh_capture_damage_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            co_ms::CAPTURE_DAMAGE_MASTER_HAND,
            0,
            0.0,
            1.0,
            0.0,
            None,
        );
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x2220_b3 = true;
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
    }

    /// `ftMh_CaptureDamageMasterHand_Anim`: mashing (`ftCommonData` x3A8); free once the
    /// timer runs out, and he lets go.
    #[doc(alias = "ftMh_CaptureDamageMasterHand_Anim")]
    pub fn mh_capture_damage_anim(&mut self, gobj: GObjId) {
        let amount = self.common().f32(0x3A8);
        self.grab_mash(gobj, amount);
        let fp = self.game.fighter(gobj);
        if fp.grab_timer <= 0.0 {
            let hand = fp.victim_gobj.expect("a held fighter without its holder");
            self.mh_capture_wait_enter(gobj);
            self.mh_cancel_enter(hand);
        }
    }

    /// Held, waiting to be let go, `ftMh_CaptureDamageMasterHand_80155C94`.
    // Ports `ftMh_CaptureDamageMasterHand_80155C94`; aliased once a check reaches it.
    pub fn mh_capture_wait_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            co_ms::CAPTURE_WAIT_MASTER_HAND,
            0,
            0.0,
            1.0,
            0.0,
            None,
        );
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
    }

    /// Let go, `ftMh_CaptureWaitMasterHand_80155D1C`: visible, turned around, cut loose
    /// (`ftCo_CaptureCut_Enter`).
    // Ports `ftMh_CaptureWaitMasterHand_80155D1C`; aliased once a check reaches it.
    pub fn mh_capture_cut(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        self.enter_capture_cut(gobj);
    }

    /// Thrown, `ftMh_CaptureWaitMasterHand_80155D6C`: facing as its holder does, visible,
    /// following the throw (`ftCo_800DE508`).
    #[doc(alias = "ftMh_CaptureWaitMasterHand_80155D6C")]
    pub fn mh_thrown_enter(&mut self, gobj: GObjId) {
        let hand = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("a thrown fighter without its thrower");
        let hf = self.game.fighter(hand).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = hf;
        fp.set_mv_i32(mv::X0, 0);
        self.change_motion_state(gobj, co_ms::THROWN_MASTER_HAND, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 511;
        self.anim_update(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::BACK_DISAPPEAR_ANIM => g.mh_back_disappear_anim(gobj),
        addr::BACK_DISAPPEAR_IASA => g.mh_human_nop(gobj),
        addr::MS_372_801542E0 => g.mh_reach_enter(gobj),
        addr::WAIT1_1_ANIM => g.mh_reach_anim(gobj),
        addr::WAIT1_1_PHYS => g.mh_reach_phys(gobj),
        addr::MS_373_801545A0 => g.mh_grab_enter(gobj),
        addr::GRAB_ANIM => g.mh_grab_anim(gobj),
        addr::GRAB_IASA => g.mh_human_nop(gobj),
        addr::MS_374_801546D8 => g.mh_fail_enter(gobj),
        addr::FAIL_PHYS | addr::CANCEL_PHYS | addr::SQUEEZING_PHYS | addr::SQUEEZE_PHYS => {
            g.mh_glide_motion(gobj)
        }
        addr::MS_381_8015483C => g.mh_cancel_enter(gobj),
        addr::CANCEL_ANIM => g.mh_cancel_anim(gobj),
        addr::CANCEL_COLL | addr::SQUEEZE_COLL => g.mh_stop_at_target(gobj),
        addr::MS_375_80154A2C => g.mh_caught(gobj),
        addr::MS_375_80154C78 => g.mh_squeezing0_enter(gobj),
        addr::MS_378_80154CF8 => g.mh_squeezing1_enter(gobj),
        addr::SQUEEZING_ANIM => g.mh_squeezing_anim(gobj),
        addr::MS_378_80154A78 => g.mh_squeeze_enter(gobj),
        addr::SQUEEZE_ANIM => g.mh_squeeze_anim(gobj),
        addr::MS_376_80154E78 => g.mh_throw_enter(gobj, ms::THROW),
        addr::MS_379_80155014 => g.mh_throw_enter(gobj, ms::SLAM),
        addr::THROW_ANIM | addr::SLAM_ANIM => g.mh_throw_anim(gobj),
        addr::CAPTURE_MASTER_HAND_80155B80 => g.mh_capture_damage_enter(gobj),
        addr::CAPTURE_DAMAGE_MASTER_HAND_ANIM => g.mh_capture_damage_anim(gobj),
        addr::CAPTURE_DAMAGE_MASTER_HAND_80155C94 => g.mh_capture_wait_enter(gobj),
        addr::CAPTURE_WAIT_MASTER_HAND_80155D1C => g.mh_capture_cut(gobj),
        _ => return false,
    }
    true
}
