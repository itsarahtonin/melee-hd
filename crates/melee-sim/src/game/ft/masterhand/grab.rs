//! The hands' grab (`ftmasterhandbackdisappear.c`'s reach and grab, `ftmasterhandwait11.c`,
//! `ftmasterhandsqueezing.c`, `ftmasterhandsqueeze.c`, `ftmasterhandthrow.c`,
//! `ftmasterhandslam.c`; Crazy Hand's `ftcrazyhandfingerbeam.c`, `ftcrazyhandfingergun2.c`,
//! `ftcrazyhandbackairplane1.c` to `3.c`) and the grabbed fighter's states
//! (`ftmasterhandtagcancel.c`'s capture, `ftmasterhandcapture*.c`,
//! `ftmasterhandthrownmasterhand.c`, `ftcrazyhandcapture*.c`, `ftcrazyhandthrowncrazyhand.c`):
//! he reaches toward the nearest opponent, closes his hand, squeezes, then throws one of two
//! ways; a victim mashing free makes him let go. Crazy Hand's runs in states Master Hand's
//! decomp names for other moves (`Grab`).

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::wait::set_mv_vec;
use super::{HAND, Hand, addr, mv, u};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::vec::Vec3;

/// `ftCo_800DB464`, `ftCo_800DE508`: a held and a thrown fighter's accessories.
const HELD_FOLLOW: u32 = 0x800D_B464;
const THROWN_FOLLOW: u32 = 0x800D_E508;

/// A hand's grab: its states, attributes and callbacks, and its victim's states.
pub struct Grab {
    /// The reach (his wait's frame kept), the grab, the hand closing on a catch and on
    /// nothing, the way back to his place after closing on nothing, letting go, the squeeze,
    /// and the two throws (the first turns the victim around).
    pub reach: i32,
    pub grab: i32,
    pub close_caught: i32,
    pub close_empty: i32,
    pub miss: i32,
    pub let_go: i32,
    pub squeeze: i32,
    pub throws: [i32; 2],
    /// The attributes: the reach's frames (its rate in the next word), the point off the
    /// nearest opponent the reach chases, the squeeze's point, and the frames before he lets
    /// go of a fighter mashing free.
    pub reach_frames: usize,
    pub reach_chase: usize,
    pub squeeze_at: usize,
    pub let_go_frames: usize,
    /// His grab box's size (`ftCommon_8007E2D0`), and its callbacks: his, once he catches
    /// someone, and the victim's.
    pub box_size: u16,
    pub caught: u32,
    pub victim: u32,
    /// The victim's states: caught, squeezed, waiting to be let go, thrown.
    pub capture: i32,
    pub capture_damage: i32,
    pub capture_wait: i32,
    pub thrown: i32,
}

impl Running<'_> {
    /// `ftMh_BackDisappear_Anim`: the callback the disappearance was given once it ends.
    #[doc(alias = "ftMh_BackDisappear_Anim")]
    pub fn mh_back_disappear_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            let cb = self.game.fighter(gobj).mv_i32(mv::X4) as u32;
            super::super::super::code::run_fighter_cb(self, gobj, CodeRef(cb));
        }
    }

    /// The reach, `ftMh_MS_372_801542E0` and `ftCh_Init_80158B3C`: his wait's frame kept at
    /// his reach's rate, for its frames (`mv.x8`).
    #[doc(alias = "ftMh_MS_372_801542E0")]
    #[doc(alias = "ftCh_Init_80158B3C")]
    pub fn hand_reach_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let frame = self.game.fighters[f.0].cur_anim_frame;
        self.change_motion_state(gobj, h.grab.reach, 0, frame, 1.0, 0.0, None);
        let (n, rate) = (
            h.f32(self, f, h.grab.reach_frames),
            h.f32(self, f, h.grab.reach_frames + 4),
        );
        self.set_anim_rate(gobj, rate);
        self.game.fighters[f.0].set_mv_i32(mv::X8, fp::fctiwz(reg(n)));
    }

    /// `ftMh_Wait1_1_Anim` and `ftCh_FingerBeamLoop_Anim`: the grab once the count runs out;
    /// the reach's animation again.
    #[doc(alias = "ftMh_Wait1_1_Anim")]
    #[doc(alias = "ftCh_FingerBeamLoop_Anim")]
    pub fn hand_reach_anim(&mut self, gobj: GObjId, h: &Hand) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp.mv_i32(mv::X8).wrapping_sub(1);
        fp.set_mv_i32(mv::X8, n);
        if n == 0 {
            self.set_anim_rate(gobj, 1.0);
            self.hand_grab_enter(gobj, h);
        }
        if !self.anim_frames_remaining(gobj) {
            self.hand_enter(gobj, h.grab.reach);
        }
    }

    /// Chasing the point `off` of his attributes off the nearest opponent at his glide's rate
    /// of the distance a frame (all of it within its near distance).
    // NaN takes the original's branch (`len < near` fails).
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub(crate) fn hand_chase(&mut self, gobj: GObjId, h: &Hand, off: usize) {
        let f = self.game.fighter_id(gobj);
        let mut pos = self.boss_nearest_pos(gobj);
        pos.x = mem(fp::fadds(reg(pos.x), reg(h.f32(self, f, off))));
        pos.y = mem(fp::fadds(reg(pos.y), reg(h.f32(self, f, off + 4))));
        pos.z = 0.0;
        let (near, rate) = (h.f32(self, f, h.glide_near), h.f32(self, f, h.glide_rate));
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

    /// `ftMh_Wait1_1_Phys` and `ftCh_FingerBeamLoop_Phys`: the air motion, chasing the reach's
    /// point off the nearest opponent.
    #[doc(alias = "ftMh_Wait1_1_Phys")]
    #[doc(alias = "ftCh_FingerBeamLoop_Phys")]
    pub fn hand_reach_phys(&mut self, gobj: GObjId, h: &Hand) {
        self.air_anim_motion(gobj);
        self.hand_chase(gobj, h, h.grab.reach_chase);
    }

    /// The grab, `ftMh_MS_373_801545A0` and `ftCh_Init_80158DFC`: his grab box's callbacks
    /// (`ftCommon_8007E2D0`), holding no one.
    #[doc(alias = "ftMh_MS_373_801545A0")]
    #[doc(alias = "ftCh_Init_80158DFC")]
    pub fn hand_grab_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.grab.grab);
        let fp = self.game.fighter_mut(gobj);
        fp.x221E_b6 = true;
        fp.x1A68 = h.grab.box_size;
        fp.grab_cb = Some(CodeRef(h.grab.caught));
        fp.grabbed_cb = Some(CodeRef(h.grab.victim));
        fp.x2194 = None;
        fp.set_mv_i32(mv::HOLDING, 0);
    }

    /// `ftMh_Grab_Anim` and `ftCh_FingerBeamEnd_Anim`: still, then closing his hand on nothing
    /// once it ends.
    #[doc(alias = "ftMh_Grab_Anim")]
    #[doc(alias = "ftCh_FingerBeamEnd_Anim")]
    pub fn hand_grab_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            self.game.fighter_mut(gobj).self_vel = Vec3::new(0.0, 0.0, 0.0);
            self.hand_close_empty_enter(gobj, h);
        }
    }

    /// His place as the glide's point.
    fn hand_target_home(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let p = h.point(self, f, h.home);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The squeeze's point as the glide's.
    fn hand_target_squeeze(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let p = h.point(self, f, h.grab.squeeze_at);
        set_mv_vec(&mut self.game.fighters[f.0], mv::TARGET, p);
    }

    /// The miss, `ftMh_MS_374_801546D8` and `ftCh_Init_80158F34`: back toward his place.
    #[doc(alias = "ftMh_MS_374_801546D8")]
    #[doc(alias = "ftCh_Init_80158F34")]
    pub fn hand_miss_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.grab.miss);
        self.hand_target_home(gobj, h);
    }

    /// Letting go, `ftMh_MS_381_8015483C` and `ftCh_Init_80159098`: the victim freed after his
    /// let-go frames (`mv.x24`), still, back toward his place.
    #[doc(alias = "ftMh_MS_381_8015483C")]
    #[doc(alias = "ftCh_Init_80159098")]
    pub fn hand_let_go_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.hand_enter(gobj, h.grab.let_go);
        let t = h.f32(self, f, h.grab.let_go_frames);
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(mv::X24, reg(t));
        fp.cmd_vars[0] = 1;
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        self.hand_target_home(gobj, h);
    }

    /// `ftMh_Cancel_Anim` and `ftCh_FingerGun1_Anim`: the victim cut loose once the count runs
    /// out; back to his place once it ends.
    #[doc(alias = "ftMh_Cancel_Anim")]
    #[doc(alias = "ftCh_FingerGun1_Anim")]
    pub fn hand_let_go_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        let fp = &mut self.game.fighters[f.0];
        let t = fp::fsubs(reg(fp.mv_f32(mv::X24)), 1.0);
        fp.set_mv_f32(mv::X24, t);
        if reg(fp.mv_f32(mv::X24)) <= 0.0 && fp.cmd_vars[0] != 0 {
            let victim = fp.victim_gobj.expect("a hand lets go of no one");
            self.hand_capture_cut(victim);
            let fp = &mut self.game.fighters[f.0];
            fp.set_mv_i32(mv::HOLDING, 0);
            fp.cmd_vars[0] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            self.hand_return(gobj, h);
        }
    }

    /// A fighter caught, `ftMh_MS_375_80154A2C` and `fn_80159288` (the grab's callback):
    /// still, holding it, the grab box off, closing his hand.
    #[doc(alias = "ftMh_MS_375_80154A2C")]
    #[doc(alias = "fn_80159288")]
    pub fn hand_caught(&mut self, gobj: GObjId, h: &Hand) {
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = Vec3::new(0.0, 0.0, 0.0);
        fp.set_mv_i32(mv::HOLDING, 1);
        fp.x221E_b6 = false;
        self.hand_close_caught_enter(gobj, h);
    }

    /// The hand closing on a catch, `ftMh_MS_375_80154C78` and `ftCh_GrabUnk1_8015ABD0`:
    /// toward the squeeze's point.
    #[doc(alias = "ftMh_MS_375_80154C78")]
    #[doc(alias = "ftCh_GrabUnk1_8015ABD0")]
    pub fn hand_close_caught_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.grab.close_caught);
        self.hand_target_squeeze(gobj, h);
    }

    /// The hand closing on nothing yet, `ftMh_MS_378_80154CF8` and `ftCh_GrabUnk1_8015AC50`:
    /// toward his place.
    #[doc(alias = "ftMh_MS_378_80154CF8")]
    #[doc(alias = "ftCh_GrabUnk1_8015AC50")]
    pub fn hand_close_empty_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.hand_enter(gobj, h.grab.close_empty);
        self.hand_target_home(gobj, h);
    }

    /// `ftMh_Squeezing_Anim` and `ftCh_FingerGun2_Anim` (the closing hand's): the squeeze if
    /// he holds someone, else the miss.
    #[doc(alias = "ftMh_Squeezing_Anim")]
    #[doc(alias = "ftCh_FingerGun2_Anim")]
    pub fn hand_close_anim(&mut self, gobj: GObjId, h: &Hand) {
        if !self.anim_frames_remaining(gobj) {
            if self.game.fighter(gobj).mv_i32(mv::HOLDING) == 1 {
                self.hand_squeeze_enter(gobj, h);
            } else {
                self.hand_miss_enter(gobj, h);
            }
        }
    }

    /// The squeeze, `ftMh_MS_378_80154A78` and `ftCh_Init_801592D4`: the victim's damage state,
    /// the throw's angle (511, `ftCommon_8007E2F4`), still, toward the squeeze's point.
    #[doc(alias = "ftMh_MS_378_80154A78")]
    #[doc(alias = "ftCh_Init_801592D4")]
    #[doc(alias = "ftCommon_8007E2F4")]
    pub fn hand_squeeze_enter(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        self.game.fighters[f.0].cmd_vars[1] = 0;
        self.hand_enter(gobj, h.grab.squeeze);
        let fp = &mut self.game.fighters[f.0];
        fp.x2222_b2 = true;
        fp.x1A6A = 511;
        self.stop_dead(gobj);
        let victim = self.game.fighters[f.0]
            .victim_gobj
            .expect("a hand squeezes no one");
        self.hand_capture_damage_enter(victim, h);
        self.hand_target_squeeze(gobj, h);
    }

    /// `ftMh_Squeeze_Anim` and `ftCh_BackAirplane1_Anim`: a cry on the script's flag; then
    /// the throw he chose.
    #[doc(alias = "ftMh_Squeeze_Anim")]
    #[doc(alias = "ftCh_BackAirplane1_Anim")]
    pub fn hand_squeeze_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[1] != 0 {
            self.boss_cry(gobj);
            self.game.fighters[f.0].cmd_vars[1] = 0;
        }
        if !self.anim_frames_remaining(gobj) {
            let [first, second] = h.grab.throws;
            if self.game.fighters[f.0].u_i32(u::NEXT_ATTACK) == first {
                self.hand_throw_enter(gobj, first);
            } else {
                self.hand_throw_enter(gobj, second);
            }
        }
    }

    /// A throw, `ftMh_MS_376_80154E78` (the throw), `ftMh_MS_379_80155014` (the slam),
    /// `ftCh_Init_801594D4` and `ftCh_Init_80159670`.
    #[doc(alias = "ftMh_MS_376_80154E78")]
    #[doc(alias = "ftMh_MS_379_80155014")]
    #[doc(alias = "ftCh_Init_801594D4")]
    #[doc(alias = "ftCh_Init_80159670")]
    pub fn hand_throw_enter(&mut self, gobj: GObjId, msid: i32) {
        self.hand_enter(gobj, msid);
        self.game.fighter_mut(gobj).cmd_vars[0] = 0;
    }

    /// `ftMh_Throw_Anim`, `ftMh_Slam_Anim`, `ftCh_BackAirplane2_Anim` and
    /// `ftCh_BackAirplane3_Anim`: on the script's flag the victim is thrown (`ftCo_800DE2A8`,
    /// `ftCo_800DE7C0`; the first throw turns it around); back to his place once it ends.
    #[doc(alias = "ftMh_Throw_Anim")]
    #[doc(alias = "ftMh_Slam_Anim")]
    #[doc(alias = "ftCh_BackAirplane2_Anim")]
    #[doc(alias = "ftCh_BackAirplane3_Anim")]
    pub fn hand_throw_anim(&mut self, gobj: GObjId, h: &Hand) {
        let f = self.game.fighter_id(gobj);
        if self.game.fighters[f.0].cmd_vars[0] != 0 {
            self.game.fighters[f.0].cmd_vars[0] = 0;
            let turn = self.game.fighters[f.0].motion_id == h.grab.throws[0];
            let victim = self.game.fighters[f.0].victim_gobj;
            self.hand_thrown_enter(victim.expect("a hand throws no one"), h);
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
            self.hand_return(gobj, h);
        }
    }

    /// Caught by a hand, `ftMh_MS_388_80155A58` and `fn_8015B548` (the grab's victim callback):
    /// it lets go of what it held, faces him, its mash timer from his own damage and rank
    /// (`ftCo_800DA824` of the hand), held at his hand, invisible, airborne.
    #[doc(alias = "ftMh_MS_388_80155A58")]
    #[doc(alias = "fn_8015B548")]
    pub fn hand_capture_enter(&mut self, gobj: GObjId, hand: GObjId, h: &Hand) {
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
        self.change_motion_state(gobj, h.grab.capture, 0, 0.0, 1.0, 0.0, None);
        let fp = &mut self.game.fighters[f.0];
        fp.invisible = true;
        fp.x2220_b3 = true;
        fp.x1A6A = 511;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// Squeezed, `ftMh_CaptureMasterHand_80155B80` and `ftCh_GrabUnk1_8015B670`.
    #[doc(alias = "ftMh_CaptureMasterHand_80155B80")]
    #[doc(alias = "ftCh_GrabUnk1_8015B670")]
    pub fn hand_capture_damage_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.change_motion_state(gobj, h.grab.capture_damage, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x2220_b3 = true;
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        fp.x1A6A = 511;
        self.anim_update(gobj);
    }

    /// `ftMh_CaptureDamageMasterHand_Anim` and `ftCo_CaptureDamageCrazyHand_Anim`: mashing
    /// (`ftCommonData` x3A8); free once the timer runs out, and he lets go.
    #[doc(alias = "ftMh_CaptureDamageMasterHand_Anim")]
    #[doc(alias = "ftCo_CaptureDamageCrazyHand_Anim")]
    pub fn hand_capture_damage_anim(&mut self, gobj: GObjId, h: &Hand) {
        let amount = self.common().f32(0x3A8);
        self.grab_mash(gobj, amount);
        let fp = self.game.fighter(gobj);
        if fp.grab_timer <= 0.0 {
            let hand = fp.victim_gobj.expect("a held fighter without its holder");
            self.hand_capture_wait_enter(gobj, h);
            self.hand_let_go_enter(hand, h);
        }
    }

    /// Held, waiting to be let go, `ftMh_CaptureDamageMasterHand_80155C94` and
    /// `ftCh_GrabUnk1_8015B778`.
    #[doc(alias = "ftMh_CaptureDamageMasterHand_80155C94")]
    #[doc(alias = "ftCh_GrabUnk1_8015B778")]
    pub fn hand_capture_wait_enter(&mut self, gobj: GObjId, h: &Hand) {
        self.change_motion_state(gobj, h.grab.capture_wait, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = true;
        fp.x1A6A = 511;
        fp.x2220_b3 = true;
        self.anim_update(gobj);
    }

    /// Let go, `ftMh_CaptureWaitMasterHand_80155D1C` and `ftCh_GrabUnk1_8015B800`: visible,
    /// turned around, cut loose (`ftCo_CaptureCut_Enter`).
    #[doc(alias = "ftMh_CaptureWaitMasterHand_80155D1C")]
    #[doc(alias = "ftCh_GrabUnk1_8015B800")]
    pub fn hand_capture_cut(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.facing_dir = mem(fp::fneg(reg(fp.facing_dir)));
        self.enter_capture_cut(gobj);
    }

    /// Thrown, `ftMh_CaptureWaitMasterHand_80155D6C` and `ftCh_GrabUnk1_8015B850`: facing as
    /// its holder does, visible, following the throw (`ftCo_800DE508`). Crazy Hand's state is
    /// always `ftCo_MS_ThrownCrazyHand`, whatever the caller passes (both pass it).
    #[doc(alias = "ftMh_CaptureWaitMasterHand_80155D6C")]
    #[doc(alias = "ftCh_GrabUnk1_8015B850")]
    pub fn hand_thrown_enter(&mut self, gobj: GObjId, h: &Hand) {
        let hand = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("a thrown fighter without its thrower");
        let hf = self.game.fighter(hand).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = hf;
        fp.set_mv_i32(mv::X0, 0);
        self.change_motion_state(gobj, h.grab.thrown, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.invisible = false;
        fp.accessory1_cb = Some(CodeRef(THROWN_FOLLOW));
        fp.x1A6A = 511;
        self.anim_update(gobj);
    }
}

#[melee_callbacks::callbacks(Fighter)]
pub(super) fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::BACK_DISAPPEAR_ANIM => g.mh_back_disappear_anim(gobj),
        addr::BACK_DISAPPEAR_IASA => g.hand_human_nop(gobj),
        addr::MS_372_801542E0 => g.hand_reach_enter(gobj, &HAND),
        addr::WAIT1_1_ANIM => g.hand_reach_anim(gobj, &HAND),
        addr::WAIT1_1_PHYS => g.hand_reach_phys(gobj, &HAND),
        addr::MS_373_801545A0 => g.hand_grab_enter(gobj, &HAND),
        addr::GRAB_ANIM => g.hand_grab_anim(gobj, &HAND),
        addr::GRAB_IASA => g.hand_human_nop(gobj),
        addr::MS_374_801546D8 => g.hand_miss_enter(gobj, &HAND),
        addr::FAIL_PHYS | addr::CANCEL_PHYS | addr::SQUEEZING_PHYS | addr::SQUEEZE_PHYS => {
            g.hand_glide_motion(gobj, &HAND)
        }
        addr::MS_381_8015483C => g.hand_let_go_enter(gobj, &HAND),
        addr::CANCEL_ANIM => g.hand_let_go_anim(gobj, &HAND),
        addr::CANCEL_COLL | addr::SQUEEZE_COLL => g.hand_stop_at_target(gobj),
        addr::MS_375_80154A2C => g.hand_caught(gobj, &HAND),
        addr::MS_375_80154C78 => g.hand_close_caught_enter(gobj, &HAND),
        addr::MS_378_80154CF8 => g.hand_close_empty_enter(gobj, &HAND),
        addr::SQUEEZING_ANIM => g.hand_close_anim(gobj, &HAND),
        addr::MS_378_80154A78 => g.hand_squeeze_enter(gobj, &HAND),
        addr::SQUEEZE_ANIM => g.hand_squeeze_anim(gobj, &HAND),
        addr::MS_376_80154E78 => g.hand_throw_enter(gobj, HAND.grab.throws[0]),
        addr::MS_379_80155014 => g.hand_throw_enter(gobj, HAND.grab.throws[1]),
        addr::THROW_ANIM | addr::SLAM_ANIM => g.hand_throw_anim(gobj, &HAND),
        addr::CAPTURE_MASTER_HAND_80155B80 => g.hand_capture_damage_enter(gobj, &HAND),
        addr::CAPTURE_DAMAGE_MASTER_HAND_ANIM => g.hand_capture_damage_anim(gobj, &HAND),
        addr::CAPTURE_DAMAGE_MASTER_HAND_80155C94 => g.hand_capture_wait_enter(gobj, &HAND),
        addr::CAPTURE_WAIT_MASTER_HAND_80155D1C => g.hand_capture_cut(gobj),
        _ => return false,
    }
    true
}
