//! Crazy Hand's grab and the attacks that follow it (`ftcrazyhandfingerbeam.c`,
//! `ftcrazyhandfingergun2.c`, `ftcrazyhandbackairplane1.c` to `3.c`), his vanishing attacks
//! (`ftcrazyhandbackcrush.c`, `ftcrazyhandbackdisappear.c`, `ftcrazyhandwait11.c`), the
//! stomp (`ftcrazyhandgrab.c`, `ftcrazyhandcancel.c`, `ftcrazyhandsqueezing0.c`), and the
//! grabbed fighter's states (`ftcrazyhandcapture*.c`, `ftcrazyhandthrowncrazyhand.c`): he
//! follows the nearest opponent with his hand open, closes it, squeezes, then throws; a
//! victim mashing free makes him let go.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::co::ids::ms as co_ms;
use super::super::masterhand::u;
use super::super::masterhand::wait::set_mv_vec;
use super::{addr, attr, ms, mv};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCo_800DB464`, `ftCo_800DE508`: a held and a thrown fighter's accessories.
const HELD_FOLLOW: u32 = 0x800D_B464;
const THROWN_FOLLOW: u32 = 0x800D_E508;

impl Running<'_> {
    /// His place as the glide's point (`x18`, `x1C`).
    fn ch_target_home(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let p = self.ch_da_point(f, attr::X18, attr::X1C);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The reach, `ftCh_Init_80158B3C`: his wait's frame kept at `xCC`'s rate, for `xC8`
    /// frames (`mv.x8`).
    #[doc(alias = "ftCh_Init_80158B3C")]
    pub fn ch_reach_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, ms::FINGER_BEAM_LOOP, 0, frame, 1.0, 0.0, None);
        let rate = self.ch_da_f32(f, attr::XCC);
        self.set_anim_rate(gobj, rate);
        let n = self.ch_da_f32(f, attr::XC8);
        self.game.fighters[f.0].set_mv_i32(mv::X8, fp::fctiwz(reg(n)));
    }

    /// `ftCh_FingerBeamLoop_Anim`: the grab once the count runs out; the reach's animation
    /// again.
    #[doc(alias = "ftCh_FingerBeamLoop_Anim")]
    pub fn ch_reach_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.set_anim_rate(gobj, 1.0);
            self.ch_grab_enter(gobj);
        }
        if !self.anim_frames_remaining(gobj) {
            self.ch_enter(gobj, ms::FINGER_BEAM_LOOP);
        }
    }

    /// `ftCh_FingerBeamLoop_Phys`: the air motion, chasing `xC0`, `xC4` off the nearest
    /// opponent.
    #[doc(alias = "ftCh_FingerBeamLoop_Phys")]
    pub fn ch_reach_phys(&mut self, gobj: GObjId) {
        self.air_anim_motion(gobj);
        self.ch_chase(gobj, attr::XC0, attr::XC4);
    }

    /// The grab, `ftCh_Init_80158DFC`: his grab box's callbacks (`ftCommon_8007E2D0`: 0x100,
    /// caught `fn_80159288`, the victim `fn_8015B548`), holding no one.
    #[doc(alias = "ftCh_Init_80158DFC")]
    pub fn ch_grab_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::FINGER_BEAM_END);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = 0x100;
        fp.grab_cb = Some(CodeRef(addr::FN_80159288));
        fp.grabbed_cb = Some(CodeRef(addr::FN_8015B548));
        fp.x2194 = None;
        fp.set_mv_i32(mv::HOLDING, 0);
    }

    /// `ftCh_FingerBeamEnd_Anim`: a miss once it ends; still, back toward his place.
    #[doc(alias = "ftCh_FingerBeamEnd_Anim")]
    pub fn ch_grab_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel = Vec3::new(0.0, 0.0, 0.0);
            self.ch_miss_enter(gobj);
        }
    }

    /// A fighter caught, `fn_80159288` (the grab's callback): still, holding it, the grab box
    /// off, closing his hand.
    #[doc(alias = "fn_80159288")]
    pub fn ch_caught(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.set_mv_i32(mv::HOLDING, 1);
        fp.x221E_b6 = false;
        self.ch_close_enter(gobj);
    }

    /// The hand closing on a catch, `ftCh_GrabUnk1_8015ABD0`: toward `xD0`, `xD4`.
    #[doc(alias = "ftCh_GrabUnk1_8015ABD0")]
    pub fn ch_close_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::FINGER_GUN2);
        let p = self.ch_da_point(f, attr::XD0, attr::XD4);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The hand closing on nothing, `ftCh_GrabUnk1_8015AC50`: toward his place.
    #[doc(alias = "ftCh_GrabUnk1_8015AC50")]
    pub fn ch_miss_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::FINGER_GUN3);
        self.ch_target_home(gobj);
    }

    /// `ftCh_FingerGun2_Anim` (the closing hand's): the squeeze if he holds someone, else
    /// back to his place.
    #[doc(alias = "ftCh_FingerGun2_Anim")]
    pub fn ch_close_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::HOLDING) == 1 {
                self.ch_squeeze_enter(gobj);
            } else {
                self.ch_back_home_enter(gobj);
            }
        }
    }

    /// Back toward his place after a miss, `ftCh_Init_80158F34`.
    #[doc(alias = "ftCh_Init_80158F34")]
    pub fn ch_back_home_enter(&mut self, gobj: GObjId) {
        self.ch_enter(gobj, ms::BACK_PUNCH);
        self.ch_target_home(gobj);
    }

    /// Letting go, `ftCh_Init_80159098`: the victim freed after `xD8` frames (`mv.x24`),
    /// still, back toward his place.
    #[doc(alias = "ftCh_Init_80159098")]
    pub fn ch_let_go_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::FINGER_GUN1);
        let t = self.ch_da_f32(f, attr::XD8);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X24, reg(t));
        fp.cmd_vars[0] = 1;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        self.ch_target_home(gobj);
    }

    /// `ftCh_FingerGun1_Anim`: the victim cut loose once the count runs out; back to his place
    /// once it ends.
    #[doc(alias = "ftCh_FingerGun1_Anim")]
    pub fn ch_let_go_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let t = fp::fsubs(reg(fp.mv_f32(mv::X24)), 1.0);
        fp.set_mv_f32(mv::X24, t);
        if reg(fp.mv_f32(mv::X24)) <= 0.0 && fp.cmd_vars[0] != 0 {
            let victim = fp.victim_gobj.expect("Crazy Hand lets go of no one");
            self.ch_capture_cut(victim);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::HOLDING, 0);
            fp.cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            self.ch_return(gobj);
        }
    }

    /// The squeeze, `ftCh_Init_801592D4`: the victim's damage state, the throw's angle (511,
    /// `ftCommon_8007E2F4`), still, toward `xD0`, `xD4`.
    #[doc(alias = "ftCh_Init_801592D4")]
    pub fn ch_squeeze_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[1] = 0;
        self.ch_enter(gobj, ms::BACK_AIRPLANE1);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b2 = true;
        fp.x1A6A = 511;
        self.stop_dead(gobj);
        let victim = self.game.fighters[f.0]
            .victim_gobj
            .expect("Crazy Hand squeezes no one");
        self.ch_capture_damage_enter(victim);
        let p = self.ch_da_point(f, attr::XD0, attr::XD4);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// `ftCh_BackAirplane1_Anim`: a cry on the script's flag; then one of the two throws, as
    /// he chose.
    #[doc(alias = "ftCh_BackAirplane1_Anim")]
    pub fn ch_squeeze_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighters[f.0].u_i32(u::NEXT_ATTACK) == ms::BACK_AIRPLANE2 {
                self.ch_throw_enter(gobj, ms::BACK_AIRPLANE2);
            } else {
                self.ch_throw_enter(gobj, ms::BACK_AIRPLANE3);
            }
        }
    }

    /// The throws, `ftCh_Init_801594D4` and `ftCh_Init_80159670`.
    #[doc(alias = "ftCh_Init_801594D4")]
    #[doc(alias = "ftCh_Init_80159670")]
    pub fn ch_throw_enter(&mut self, gobj: GObjId, msid: i32) {
        self.ch_enter(gobj, msid);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
    }

    /// `ftCh_BackAirplane2_Anim` and `ftCh_BackAirplane3_Anim`: on the script's flag the
    /// victim is thrown (`ftCo_800DE2A8`, `ftCo_800DE7C0`; the first throw turns it around);
    /// back to his place once it ends.
    #[doc(alias = "ftCh_BackAirplane2_Anim")]
    #[doc(alias = "ftCh_BackAirplane3_Anim")]
    pub fn ch_throw_anim(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.game.fighters[f.0].cmd_vars[0] = 0;
            let turn = self.game.fighters[f.0].motion_id == ms::BACK_AIRPLANE2;
            let victim = self.game.fighters[f.0].victim_gobj;
            self.ch_thrown_enter(victim.expect("Crazy Hand throws no one"));
            if let Some(v) = self.game.fighters[f.0].victim_gobj {
                self.game.fighters[f.0].x1A6A = 0;
                self.throw_hit(gobj, v, true);
                if turn {
                    let vf = self.game.fighter_mut(v);
                    vf.dmg.facing_dir_1 = mem(fp::fmuls(reg(vf.dmg.facing_dir_1), -1.0));
                }
                self.thrown_launch(v, None, false);
            }
            self.game.fighters[f.0].set_mv_i32(mv::HOLDING, 0);
        }
        if !self.anim_frames_remaining(gobj) {
            self.ch_return(gobj);
        }
    }

    /// The wind-up of his vanishing attacks, `ftCh_Init_801597F0`: `cb` once it ends.
    #[doc(alias = "ftCh_Init_801597F0")]
    pub fn ch_back_crush_enter(&mut self, gobj: GObjId, cb: u32) {
        self.ch_enter(gobj, ms::BACK_CRUSH);
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
        self.ch_enter(gobj, ms::BACK_DISAPPEAR);
        let n = self.ch_da_i32(f, attr::XF0);
        self.game.fighters[f.0].set_mv_f32(mv::X0, fp::frsp(f64::from(n)));
        let y = self.ch_da_f32(f, attr::XEC);
        self.ch_appear_over_opponent(gobj, y);
    }

    /// `ftCh_BackDisappear_Anim` and `ftCh_Wait1_1_Anim`'s end: still across, back to his
    /// place.
    #[doc(alias = "ftCh_BackDisappear_Anim")]
    pub fn ch_dive_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel.x = 0.0;
            self.ch_return(gobj);
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
            let max = self.ch_da_f32(f, attr::XDC);
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
        self.ch_enter(gobj, ms::WAIT1_1);
        let y = fp::frsp(f64::from(self.ch_da_i32(f, attr::XF4)));
        self.ch_appear_over_opponent(gobj, mem(y));
        let frames = self.ch_da_i32(f, attr::XF8);
        let n = self.ch_da_i32(f, attr::X100);
        let small = reg(self.ch_da_f32(f, attr::XFC));
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
        self.ch_enter(gobj, ms::GRAB);
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
        self.ch_enter(gobj, ms::CANCEL);
        let y = self.ch_da_f32(f, attr::XE0);
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
        let max = self.ch_da_f32(f, attr::XDC);
        self.boss_follow_x_add(gobj, max);
    }

    /// The stomp's end, `ftCh_Init_8015A184`: at `xE4`, `xE8`, still.
    #[doc(alias = "ftCh_Init_8015A184")]
    pub fn ch_stomp_end_enter(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        self.ch_enter(gobj, ms::SQUEEZING0);
        let (x, y) = (self.ch_da_f32(f, attr::XE4), self.ch_da_f32(f, attr::XE8));
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = x;
        fp.cur_pos.y = y;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
    }

    /// Caught by Crazy Hand, `fn_8015B548` (the grab's victim callback): it lets go of what
    /// it held, faces him, its mash timer from his own damage and rank (`ftCo_800DA824` of
    /// the hand), held at his hand, invisible, airborne.
    #[doc(alias = "fn_8015B548")]
    pub fn ch_capture_enter(&mut self, gobj: GObjId, hand: GObjId) {
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
        self.change_motion_state(gobj, co_ms::CAPTURE_CRAZY_HAND, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// Squeezed, `ftCh_GrabUnk1_8015B670`.
    #[doc(alias = "ftCh_GrabUnk1_8015B670")]
    pub fn ch_capture_damage_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(
            gobj,
            co_ms::CAPTURE_DAMAGE_CRAZY_HAND,
            0,
            0.0,
            1.0,
            0.0,
            None,
        );
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
    }

    /// `ftCo_CaptureDamageCrazyHand_Anim`: mashing (`ftCommonData` x3A8); free once the
    /// timer runs out, and he lets go.
    #[doc(alias = "ftCo_CaptureDamageCrazyHand_Anim")]
    pub fn ch_capture_damage_anim(&mut self, gobj: GObjId) {
        let amount = self.common().f32(0x3A8);
        self.grab_mash(gobj, amount);
        let fp = self.game.fighter(gobj);
        if fp.grab_timer <= 0.0 {
            let hand = fp.victim_gobj.expect("a held fighter without its holder");
            self.ch_capture_wait_enter(gobj);
            self.ch_let_go_enter(hand);
        }
    }

    /// Held, waiting to be let go, `ftCh_GrabUnk1_8015B778`.
    #[doc(alias = "ftCh_GrabUnk1_8015B778")]
    pub fn ch_capture_wait_enter(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, co_ms::CAPTURE_WAIT_CRAZY_HAND, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
    }

    /// Let go, `ftCh_GrabUnk1_8015B800`: visible, turned around, cut loose
    /// (`ftCo_CaptureCut_Enter`).
    #[doc(alias = "ftCh_GrabUnk1_8015B800")]
    pub fn ch_capture_cut(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        self.enter_capture_cut(gobj);
    }

    /// Thrown, `ftCh_GrabUnk1_8015B850`: facing as its holder does, visible, following the
    /// throw (`ftCo_800DE508`). The state is always `ftCo_MS_ThrownCrazyHand`, whatever the
    /// caller passes (both pass it).
    #[doc(alias = "ftCh_GrabUnk1_8015B850")]
    pub fn ch_thrown_enter(&mut self, gobj: GObjId) {
        let hand = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("a thrown fighter without its thrower");
        let hf = self.game.fighter(hand).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = hf;
        fp.set_mv_i32(mv::X0, 0);
        self.change_motion_state(gobj, co_ms::THROWN_CRAZY_HAND, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 511;
        self.anim_update(gobj);
    }
}

pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    use addr::*;
    match a {
        INIT_80158B3C => g.ch_reach_enter(gobj),
        FINGER_BEAM_LOOP_ANIM => g.ch_reach_anim(gobj),
        FINGER_BEAM_LOOP_PHYS => g.ch_reach_phys(gobj),
        INIT_80158DFC => g.ch_grab_enter(gobj),
        FINGER_BEAM_END_ANIM => g.ch_grab_anim(gobj),
        FN_80159288 => g.ch_caught(gobj),
        GRAB_UNK1_8015ABD0 => g.ch_close_enter(gobj),
        GRAB_UNK1_8015AC50 => g.ch_miss_enter(gobj),
        FINGER_GUN2_ANIM => g.ch_close_anim(gobj),
        INIT_80158F34 => g.ch_back_home_enter(gobj),
        INIT_80159098 => g.ch_let_go_enter(gobj),
        FINGER_GUN1_ANIM => g.ch_let_go_anim(gobj),
        INIT_801592D4 => g.ch_squeeze_enter(gobj),
        BACK_AIRPLANE1_ANIM => g.ch_squeeze_anim(gobj),
        INIT_801594D4 => g.ch_throw_enter(gobj, ms::BACK_AIRPLANE2),
        INIT_80159670 => g.ch_throw_enter(gobj, ms::BACK_AIRPLANE3),
        BACK_AIRPLANE2_ANIM | BACK_AIRPLANE3_ANIM => g.ch_throw_anim(gobj),
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
        GRAB_UNK1_8015B670 => g.ch_capture_damage_enter(gobj),
        CAPTURE_DAMAGE_CRAZY_HAND_ANIM => g.ch_capture_damage_anim(gobj),
        GRAB_UNK1_8015B778 => g.ch_capture_wait_enter(gobj),
        GRAB_UNK1_8015B800 => g.ch_capture_cut(gobj),
        GRAB_UNK1_8015B850 => g.ch_thrown_enter(gobj),
        _ => return false,
    }
    true
}
