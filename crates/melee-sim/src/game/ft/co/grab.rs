//! Holding a grab and being held (`ftCo_CatchPull.c`, `ftCo_CatchWait.c`,
//! `ftCo_CatchAttack.c`, `ftCo_CatchCut.c`, `ftCo_CapturePulled.c`, `ftCo_CaptureWait.c`,
//! `ftCo_CaptureCut.c`, `ftCo_CaptureJump.c`): the grabber pulls the victim in, holds and
//! pummels it, or lets go; the victim follows the grabber's hold point, mashes free, and
//! breaks out on the ground or jumps away. Link's and Samus's tethers, Yoshi's egg and
//! Kirby's swallow stop at their seams, as do the throws.

use gekko_fp as fp;

use super::super::super::code::{run_fighter_cb, unported_call};
use super::super::super::ef;
use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::super::common::{GA_AIR, GA_GROUND};
use super::super::motion::flags::UPDATE_CMD;
use super::checks::{KIND_CLINK, KIND_LINK, KIND_SAMUS, KIND_YOSHI};
use super::ids::ms;
use super::{PAD_A, PAD_XY};
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj::{self, JObjId};
use crate::lb::joint::world_position;

/// `ftCommon_MotionVars_capturedamage`'s `x18`: the grabber's hold point, which the victim
/// follows.
const HOLD_JOINT: usize = 0x18;
/// `ftCommon_MotionVars_capturewait`: frames held, the mash animation's timer, whether the
/// victim mashed this frame, and a jump buffered for the escape.
const WAIT_FRAMES: usize = 0x0;
const WAIT_MASH_TIMER: usize = 0x4;
const WAIT_MASHED: usize = 0x8;
const WAIT_JUMP: usize = 0xC;
/// `ftCommon_MotionVars_buryjump`'s `x0`: the escape jump's frames.
const JUMP_FRAMES: usize = 0x0;

/// `FtPart_XRotN`.
const X_ROT_N: i32 = 2;

/// The callbacks the grab's states store.
const PULL_FOLLOW: u32 = 0x800D_A190;
const WAIT_FOLLOW: u32 = 0x800D_A4A0;
const WAIT_ON_DAMAGE: u32 = 0x800D_A490;
const ATTACK_FOLLOW: u32 = 0x800D_A678;
const ATTACK_ON_DAMAGE: u32 = 0x800D_A668;

impl Running<'_> {
    /// The joint the grab holds the victim at: the character's hold bone (`ftData::x8`'s
    /// `x11`).
    fn hold_bone_joint(&self, gobj: GObjId) -> Option<JObjId> {
        let fp = self.game.fighter(gobj);
        let d = fp.ft_data.expect("a fighter without data");
        let x8 = self.data.ptr(d.at(8)).expect("ftData::x8");
        let i = self.data.u8(x8.at(0x11));
        self.bone_joint(gobj, i32::from(i))
    }

    /// A grab catches, `fn_800D9CE8`: still, into the pull (the dash grab's for a dash grab)
    /// from the grab's frame, holding at the hold bone.
    #[doc(alias = "fn_800D9CE8")]
    pub fn catch_landed(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.gr_vel = 0.0;
        let frame = fp.cur_anim_frame;
        let next = if fp.motion_id == ms::CATCH {
            if fp.kind == KIND_YOSHI {
                unported_call("fn_800D9CE8 (Yoshi's tongue)");
            }
            ms::CATCH_PULL
        } else {
            ms::CATCH_DASH_PULL
        };
        let kind = self.game.fighter(gobj).kind;
        if kind == KIND_LINK || kind == KIND_CLINK || kind == KIND_SAMUS {
            unported_call("fn_800D9CE8 (a tether's grab)");
        }
        let j = self.hold_bone_joint(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.mv.set_jobj_at(HOLD_JOINT, j);
        fp.x2210 = [0; 4];
        self.change_motion_state(gobj, next, UPDATE_CMD, frame, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(PULL_FOLLOW));
        fp.x221B.x221B_b7 = false;
    }

    /// `ftCo_CatchPull_Anim`: holding once the pull's frames run out or the script marks it.
    #[doc(alias = "ftCo_CatchPull_Anim")]
    pub fn co_catch_pull_anim(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        if kind == KIND_LINK || kind == KIND_CLINK || kind == KIND_SAMUS {
            unported_call("ftCo_CatchPull_Anim (a tether's pull)");
        }
        let mut go = !self.anim_frames_remaining(gobj);
        if !go {
            let fp = self.game.fighter_mut(gobj);
            if fp.throw_flag(3) {
                fp.set_throw_flag(3, false);
                go = true;
            }
        }
        if go {
            self.enter_catch_wait(gobj);
        }
    }

    /// The grab's states' friction (`ftCommonData` x64 of the ground's): `ftCo_CatchPull_Phys`,
    /// `ftCo_CatchAttack_Phys` (and `ftCo_CatchWait_Phys`'s).
    #[doc(alias = "ftCo_CatchPull_Phys")]
    // Ports `ftCo_CatchAttack_Phys`; aliased once a check reaches it.
    pub fn co_catch_hold_phys(&mut self, gobj: GObjId) {
        let mul = reg(self.common().f32(0x64));
        let fp = self.game.fighter_mut(gobj);
        let friction = fp::fmuls(mul, reg(fp.co_attrs.ground_friction));
        Self::ground_accel_deaccel(fp, friction);
        self.self_movement_from_ground(gobj);
    }

    /// The grab's states' collision: off the ground, both let go and fall (`fn_800DA004`,
    /// `fn_800DA440`, `fn_800DA618`). `ftCo_CatchPull_Coll`, `ftCo_CatchWait_Coll`,
    /// `ftCo_CatchAttack_Coll`.
    #[doc(alias = "ftCo_CatchPull_Coll")]
    #[doc(alias = "ftCo_CatchWait_Coll")]
    // Ports `ftCo_CatchAttack_Coll`; aliased once a check reaches it.
    pub fn co_catch_hold_coll(&mut self, gobj: GObjId) {
        self.ground_coll_then(gobj, |g, gobj| g.catch_fall_apart(gobj));
    }

    /// Off the ground while holding, `fn_800DA004` (`fn_800DA440`, `fn_800DA618`): both let
    /// go and fall.
    // Ports `fn_800DA004`; aliased once a check reaches it.
    // Ports `fn_800DA440`; aliased once a check reaches it.
    // Ports `fn_800DA618`; aliased once a check reaches it.
    fn catch_fall_apart(&mut self, gobj: GObjId) {
        let victim = self.game.fighter(gobj).victim_gobj.expect("holding no one");
        self.grab_release(gobj, victim);
        self.enter_fall(gobj);
        self.enter_fall(victim);
    }

    /// The hold keeps the victim in reach, `fn_800DA054` (`fn_800DA190` but for the tethers,
    /// `fn_800DA4A0`, `fn_800DA678`): a victim too far ahead or above slips free; one behind
    /// draws the grabber after it at up to its walking speed.
    #[doc(alias = "fn_800DA054")]
    #[doc(alias = "fn_800DA190")]
    #[doc(alias = "fn_800DA4A0")]
    // Ports `fn_800DA678`; aliased once a check reaches it.
    pub fn catch_follow(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        if self.game.fighter(gobj).accessory1_cb == Some(CodeRef(PULL_FOLLOW))
            && matches!(kind, KIND_LINK | KIND_SAMUS | KIND_CLINK)
        {
            return;
        }
        let victim = self.game.fighter(gobj).victim_gobj.expect("holding no one");
        if self.game.fighter(victim).x2226_b2 {
            return;
        }
        let hold = self.game.fighter(gobj).mv.jobj_at(HOLD_JOINT);
        let me = world_position(&mut self.game.objs, self.data, hold, None);
        let vj = self.part_joint(victim, X_ROT_N);
        let them = world_position(&mut self.game.objs, self.data, vj, None);
        let fp = self.game.fighter(gobj);
        let facing = reg(fp.facing_dir);
        let dx = fp::fsubs(reg(them.x), reg(me.x));
        let mut dy = fp::fadds(fp::fsubs(reg(them.y), reg(me.y)), reg(fp.x2170));
        let c = self.common();
        let (ahead, above) = (reg(c.f32(0x34C)), reg(c.f32(0x350)));
        if fp::fmuls(dx, facing) > ahead {
            self.enter_catch_cut(gobj, true);
            return;
        }
        if dy < 0.0 {
            dy = fp::fneg(dy);
        }
        if dy > above {
            self.enter_catch_cut(gobj, true);
        } else if fp::fmuls(dx, facing) < 0.0 {
            let fp = self.game.fighter_mut(gobj);
            let mut v = if dx < 0.0 { fp::fneg(dx) } else { dx };
            let max = reg(fp.co_attrs.walk_max_vel);
            if v > max {
                v = max;
            }
            fp.gr_vel = mem(if dx > 0.0 { v } else { fp::fneg(v) });
        }
    }

    /// Holding, `fn_800DA1D8`: still, the hold's hooks, its hold point's effect and rumble,
    /// and the victim held (`fn_800DB6C8`).
    #[doc(alias = "fn_800DA1D8")]
    pub fn enter_catch_wait(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).gr_vel = 0.0;
        self.change_motion_state(gobj, ms::CATCH_WAIT, 0, 0.0, 1.0, 0.0, None);
        let j = self.hold_bone_joint(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(WAIT_FOLLOW));
        fp.take_dmg_cb = Some(CodeRef(WAIT_ON_DAMAGE));
        fp.x221B.x221B_b7 = false;
        fp.mv.set_jobj_at(HOLD_JOINT, j);
        self.fighter_effect(gobj, ef::SpawnKind::Pos, 0x41D, j, &[]);
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 3, 0);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        let victim = self.game.fighter(gobj).victim_gobj.expect("holding no one");
        self.enter_capture_wait(victim);
    }

    /// Back to holding after a pummel, `fn_800DA2B0`.
    // Ports `fn_800DA2B0`; aliased once a check reaches it.
    fn reenter_catch_wait(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::CATCH_WAIT, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(WAIT_FOLLOW));
        fp.take_dmg_cb = Some(CodeRef(WAIT_ON_DAMAGE));
        fp.x1A6A = 0x1FF;
    }

    /// `ftCo_CatchWait_IASA`: a pummel with A (`fn_800DA4C0`), else a throw.
    #[doc(alias = "ftCo_CatchWait_IASA")]
    #[doc(alias = "fn_800DA4C0")]
    pub fn co_catch_wait_iasa(&mut self, gobj: GObjId) {
        let kind = self.game.fighter(gobj).kind;
        let acted = if self.game.fighter(gobj).input.pressed_buttons & PAD_A != 0 {
            self.enter_catch_attack(gobj);
            true
        } else {
            self.throw_check(gobj)
        };
        if acted && matches!(kind, KIND_LINK | KIND_CLINK | KIND_SAMUS) {
            unported_call("it_802A7AAC, it_802BAC3C (a tether reeled in)");
        }
    }

    /// The hold's damage hooks clear themselves, `fn_800DA490` (`fn_800DA668`).
    // Ports `fn_800DA490`; aliased once a check reaches it.
    // Ports `fn_800DA668`; aliased once a check reaches it.
    pub fn catch_on_damage(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).take_dmg_cb = None;
    }

    /// A pummel, `fn_800DA4FC`.
    // Ports `fn_800DA4FC`; aliased once a check reaches it.
    fn enter_catch_attack(&mut self, gobj: GObjId) {
        self.game.fighter_mut(gobj).gr_vel = 0.0;
        self.change_motion_state(gobj, ms::CATCH_ATTACK, 0, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(ATTACK_FOLLOW));
        fp.take_dmg_cb = Some(CodeRef(ATTACK_ON_DAMAGE));
        fp.x1A6A = 0x1FF;
    }

    /// `ftCo_CatchAttack_Anim`: holding again once the pummel's frames run out.
    // Ports `ftCo_CatchAttack_Anim`; aliased once a check reaches it.
    pub fn co_catch_attack_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.reenter_catch_wait(gobj);
        }
    }

    /// A throw by the stick or the C-stick, `ftCo_800DD1E4` (`ftCo_800DF7F4`,
    /// `ftCo_800DF844`, `ftCo_800DF878`): forward or back, up, or down.
    #[doc(alias = "ftCo_800DD1E4")]
    #[doc(alias = "ftCo_800DF7F4")]
    #[doc(alias = "ftCo_800DF844")]
    #[doc(alias = "ftCo_800DF878")]
    pub fn throw_check(&mut self, gobj: GObjId) -> bool {
        let c = self.common();
        let (side, up, down) = (reg(c.f32(0x98)), reg(c.f32(0xAC)), reg(c.f32(0xB0)));
        let fp = self.game.fighter(gobj);
        let (l0, l1) = (&fp.input.lstick[0], &fp.input.lstick[1]);
        let (c0, c1) = (&fp.input.cstick[0], &fp.input.cstick[1]);
        let facing = reg(fp.facing_dir);
        let flick = |a0: f32, a1: f32| {
            (reg(a1) < side && reg(a0) >= side)
                || (reg(a1) > fp::fneg(side) && reg(a0) <= fp::fneg(side))
        };
        let mut msid = fp.motion_id;
        if flick(l0.x, l1.x) {
            msid = if fp::fmuls(reg(l0.x), facing) > 0.0 {
                ms::THROW_F
            } else {
                ms::THROW_B
            };
        } else if flick(c0.x, c1.x) {
            msid = if fp::fmuls(reg(c0.x), facing) > 0.0 {
                ms::THROW_F
            } else {
                ms::THROW_B
            };
        } else if (reg(l1.y) < up && reg(l0.y) >= up) || (reg(c1.y) < up && reg(c0.y) >= up) {
            msid = ms::THROW_HI;
        } else if (reg(l1.y) > down && reg(l0.y) <= down)
            || (reg(c1.y) <= down && reg(c0.y) <= down)
        {
            msid = ms::THROW_LW;
        }
        if msid != fp.motion_id {
            self.enter_throw(gobj, msid);
            return true;
        }
        false
    }

    /// Letting go, `ftCo_800DA698`: pushed back (in the air, up too), and with `cut` the
    /// victim breaks free.
    // Ports `ftCo_800DA698`; aliased once a check reaches it.
    pub fn enter_catch_cut(&mut self, gobj: GObjId, cut: bool) {
        let c = self.common();
        let (gr, air_x, air_y) = (c.f32(0x370), c.f32(0x374), c.f32(0x378));
        let fp = self.game.fighter_mut(gobj);
        let back = fp::fneg(reg(fp.facing_dir));
        if fp.ground_or_air == GA_AIR {
            fp.self_vel.x = mem(fp::fmuls(back, reg(air_x)));
            fp.self_vel.y = air_y;
        } else {
            fp.gr_vel = mem(fp::fmuls(back, reg(gr)));
        }
        self.change_motion_state(gobj, ms::CATCH_CUT, 0, 0.0, 1.0, 0.0, None);
        if cut {
            let victim = self
                .game
                .fighter(gobj)
                .victim_gobj
                .expect("letting go of no one");
            self.enter_capture_cut(victim);
        }
    }

    /// `ftCo_CatchCut_Anim` (`ftCo_CaptureCut_Anim`): falling or waiting once its frames run
    /// out.
    // Ports `ftCo_CatchCut_Anim`; aliased once a check reaches it.
    // Ports `ftCo_CaptureCut_Anim`; aliased once a check reaches it.
    pub fn co_catch_cut_anim(&mut self, gobj: GObjId) {
        if !self.anim_frames_remaining(gobj) {
            self.fall_or_wait(gobj);
        }
    }

    /// `ftCo_CatchCut_Phys`: the hold's friction on the ground, else falling and drifting.
    // Ports `ftCo_CatchCut_Phys`; aliased once a check reaches it.
    pub fn co_catch_cut_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.co_catch_hold_phys(gobj);
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_CatchCut_Coll`: on the ground, standing or falling; in the air, landing.
    // Ports `ftCo_CatchCut_Coll`; aliased once a check reaches it.
    pub fn co_catch_cut_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll(gobj);
        } else {
            self.air_coll_ledges_then(gobj, |g, gobj| g.land_normally(gobj));
        }
    }

    /// The grab's end for both, `ftCo_800DC920`: each forgets the other (a frozen partner
    /// thaws, `Fighter_UnkSetFlag_8006CFBC`). A victim the grab made part of the grabber's
    /// model (Yoshi's egg, Kirby's swallow) isn't ported.
    // Ports `ftCo_800DC920`; aliased once a check reaches it.
    // Ports `Fighter_UnkSetFlag_8006CFBC`; aliased once a check reaches it.
    pub fn grab_release(&mut self, a: GObjId, b: GObjId) {
        for g in [a, b] {
            let fp = self.game.fighter_mut(g);
            if fp.x2219_b7 {
                fp.x221A_b1 = true;
            }
        }
        let (holder, held) = if self.game.fighter(a).x221B.x221B_b7 {
            (b, a)
        } else {
            (a, b)
        };
        let fp = self.game.fighter_mut(holder);
        fp.x1A5C = None;
        fp.victim_gobj = None;
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        if self.game.fighter(held).x2226_b2 {
            unported_call("ftCo_800DC920 (a victim inside the grabber's model)");
        }
        let fp = self.game.fighter_mut(held);
        fp.x1A5C = None;
        fp.victim_gobj = None;
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
    }

    /// The victim's offset from the grabber's hold point, `fn_800DAC78`.
    #[doc(alias = "fn_800DAC78")]
    fn hold_offset(&mut self, gobj: GObjId) -> (f64, f64, f64) {
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        let hold = self.game.fighter(grabber).mv.jobj_at(HOLD_JOINT);
        let to = world_position(&mut self.game.objs, self.data, hold, None);
        let j = self.part_joint(gobj, X_ROT_N);
        let from = world_position(&mut self.game.objs, self.data, j, None);
        (
            fp::fsubs(reg(to.x), reg(from.x)),
            fp::fsubs(reg(to.y), reg(from.y)),
            fp::fsubs(reg(to.z), reg(from.z)),
        )
    }

    /// The victim to the hold point, `fn_800DAA40`: on the ground, the grabber's height
    /// difference noted (`x2170`); in the air, moved there.
    #[doc(alias = "fn_800DAA40")]
    fn follow_hold(&mut self, gobj: GObjId, grabber: GObjId) {
        let (dx, dy, dz) = self.hold_offset(gobj);
        let fp = self.game.fighter(gobj);
        if fp.ground_or_air == GA_GROUND {
            let y = reg(fp.cur_pos.y);
            let g = self.game.fighter_mut(grabber);
            g.x2170 = mem(fp::fsubs(fp::fadds(dy, y), reg(g.cur_pos.y)));
        } else {
            self.game.fighter_mut(grabber).x2170 = 0.0;
            let fp = self.game.fighter_mut(gobj);
            fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), dx));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), dy));
            fp.cur_pos.z = mem(fp::fadds(reg(fp.cur_pos.z), dz));
        }
    }

    /// The victim moved to the hold point, `fn_800DAD18`: whether it was pulled up off the
    /// ground (`ftCommonData` x3C4 times its size).
    #[doc(alias = "fn_800DAD18")]
    // Ports `ftCo_CapturePulledHi_Phys`; aliased once a check reaches it.
    // Ports `ftCo_CaptureWaitHi_Phys`; aliased once a check reaches it.
    pub fn move_to_hold(&mut self, gobj: GObjId) -> bool {
        let (dx, dy, dz) = self.hold_offset(gobj);
        let lift = reg(self.common().f32(0x3C4));
        let fp = self.game.fighter_mut(gobj);
        let up = dy > fp::fmuls(lift, reg(fp.x34_scale.y));
        fp.cur_pos.x = mem(fp::fadds(reg(fp.cur_pos.x), dx));
        fp.cur_pos.y = mem(fp::fadds(reg(fp.cur_pos.y), dy));
        fp.cur_pos.z = mem(fp::fadds(reg(fp.cur_pos.z), dz));
        up
    }

    /// Caught, `fn_800DAADC` (`fn_800DA8E4`): facing the grabber, its hooks run, its mash
    /// timer set by its damage, rank and handicap, pulled high in the air or low on the
    /// ground, moved to the hold point, and its collision run.
    #[doc(alias = "fn_800DAADC")]
    #[doc(alias = "fn_800DA8E4")]
    #[doc(alias = "ftCo_800DA824")]
    pub fn enter_capture_pulled(&mut self, victim: GObjId, grabber: GObjId) {
        let msid = if self.game.fighter(victim).ground_or_air == GA_GROUND {
            ms::CAPTURE_PULLED_LW
        } else {
            ms::CAPTURE_PULLED_HI
        };
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(victim);
        self.drop_heavy_item(victim);
        self.release_victim(victim);
        let gf = self.game.fighter(grabber).facing_dir;
        let fp = self.game.fighter_mut(victim);
        fp.x1A5C = Some(grabber);
        fp.victim_gobj = Some(grabber);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = mem(fp::fneg(reg(gf)));
        let slot = i32::from(fp.player_idx);
        let c = self.common();
        let rank = self.game.player_rank(slot).wrapping_add(1);
        let v = fp::fmuls(
            reg(c.f32(0x360)),
            fp::fsubs(reg(c.f32(0x364)), fp::frsp(f64::from(rank))),
        );
        let handicap = self.game.player_handicap(slot);
        let mut s3 = fp::fsubs(reg(c.f32(0x35C)), fp::frsp(f64::from(handicap)));
        s3 = fp::fmadds(reg(c.f32(0x358)), s3, reg(c.f32(0x354)));
        s3 = fp::fadds(s3, v);
        let fp = self.game.fighter_mut(victim);
        let timer = fp::fmadds(reg(fp.dmg.x1830_percent), reg(c.f32(0x368)), s3);
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        fp.set_mv_f32(WAIT_FRAMES, 0.0);
        fp.mv[0xC] = 0;
        fp.set_mv_f32(WAIT_MASH_TIMER, 0.0);
        fp.set_mv_i32(WAIT_MASHED, 0);
        self.change_motion_state(victim, msid, 0, 0.0, 1.0, 0.0, None);
        self.anim_update(victim);
        self.stop_dead(victim);
        self.game.fighter_mut(victim).x1A6A = 0x1FF;
        self.follow_hold(victim, grabber);
        let cb = self
            .game
            .fighter(victim)
            .coll_cb
            .expect("a state without collision");
        run_fighter_cb(self, victim, cb);
        let root = self.root_jobj(victim);
        let pos = self.game.fighter(victim).cur_pos;
        jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
    }

    /// A held heavy item falls, `ftCo_8009750C`: not ported.
    #[doc(alias = "ftCo_8009750C")]
    fn drop_heavy_item(&mut self, gobj: GObjId) {
        if let Some(item) = self.game.fighter(gobj).item_gobj
            && super::super::super::it::lib26b1::is_heavy(self.game, self.data, item)
        {
            unported_call("ftCo_8009750C (a heavy item dropped)");
        }
    }

    /// The fighter lets go of whomever it holds, `ftCo_800DD168`: not ported with someone held.
    #[doc(alias = "ftCo_800DD168")]
    fn release_victim(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).victim_gobj.is_some() {
            unported_call("ftCo_800DD168 (a grab let go of)");
        }
    }

    /// `ftCo_CapturePulledHi_Coll` (`ftCo_CaptureWaitHi_Coll`): touching down, the low pull
    /// (`fn_800DAEEC`) or hold (`fn_800DBBF8`).
    // Ports `ftCo_CapturePulledHi_Coll`; aliased once a check reaches it.
    pub fn co_capture_pulled_hi_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.air_coll_touch_then(gobj, |g, gobj| g.capture_land(gobj, ms::CAPTURE_PULLED_LW));
        }
    }

    /// `ftCo_CaptureWaitHi_Coll`.
    // Ports `ftCo_CaptureWaitHi_Coll`; aliased once a check reaches it.
    pub fn co_capture_wait_hi_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.air_coll_touch_then(gobj, |g, gobj| g.capture_land(gobj, ms::CAPTURE_WAIT_LW));
        }
    }

    /// A held victim touches down, `fn_800DAEEC` (`fn_800DBBF8`): landed into the low state
    /// from the same frame, and to the hold point.
    // Ports `fn_800DAEEC`; aliased once a check reaches it.
    // Ports `fn_800DAECC`; aliased once a check reaches it.
    // Ports `fn_800DBBF8`; aliased once a check reaches it.
    // Ports `fn_800DBAC4`; aliased once a check reaches it.
    fn capture_land(&mut self, gobj: GObjId, msid: i32) {
        self.land(gobj);
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, UPDATE_CMD, frame, 1.0, 0.0, None);
        if msid == ms::CAPTURE_WAIT_LW {
            self.capture_yoshi_check(gobj);
            self.game.fighter_mut(gobj).x1A6A = 0x1FF;
        }
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        self.follow_hold(gobj, grabber);
    }

    /// `ftCo_CapturePulledLw_Phys` (`ftCo_CaptureWaitLw_Phys`): to the hold point; pulled up
    /// off the ground, the high state (`fn_800DB230`, `fn_800DBED4`).
    #[doc(alias = "ftCo_CapturePulledLw_Phys")]
    pub fn co_capture_pulled_lw_phys(&mut self, gobj: GObjId) {
        if self.move_to_hold(gobj) {
            self.capture_lift(gobj, ms::CAPTURE_PULLED_HI);
        }
    }

    /// `ftCo_CaptureWaitLw_Phys`.
    #[doc(alias = "ftCo_CaptureWaitLw_Phys")]
    pub fn co_capture_wait_lw_phys(&mut self, gobj: GObjId) {
        if self.move_to_hold(gobj) {
            self.capture_lift(gobj, ms::CAPTURE_WAIT_HI);
        }
    }

    /// `ftCo_CapturePulledLw_Coll` (`ftCo_CaptureWaitLw_Coll`): off a ledge, the high state.
    #[doc(alias = "ftCo_CapturePulledLw_Coll")]
    pub fn co_capture_pulled_lw_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.ground_coll_fall_off_then(gobj, |g, gobj| {
                g.capture_lift(gobj, ms::CAPTURE_PULLED_HI)
            });
        }
    }

    /// `ftCo_CaptureWaitLw_Coll`.
    #[doc(alias = "ftCo_CaptureWaitLw_Coll")]
    pub fn co_capture_wait_lw_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.ground_coll_fall_off_then(gobj, |g, gobj| {
                g.capture_lift(gobj, ms::CAPTURE_WAIT_HI)
            });
        }
    }

    /// A held victim leaves the ground, `fn_800DB230` (`fn_800DBED4`): airborne with its
    /// ECB free, the high state from the same frame, to the hold point, its airborne
    /// collision run, and its model moved there.
    // Ports `fn_800DB230`; aliased once a check reaches it.
    // Ports `fn_800DBED4`; aliased once a check reaches it.
    fn capture_lift(&mut self, gobj: GObjId, msid: i32) {
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        super::super::fighter::unlock_ecb(self.game.fighter_mut(gobj));
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, UPDATE_CMD, frame, 1.0, 0.0, None);
        if msid == ms::CAPTURE_WAIT_HI {
            self.capture_wait_hooks(gobj);
        }
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        self.follow_hold(gobj, grabber);
        if !self.game.fighter(gobj).x2226_b2 {
            if msid == ms::CAPTURE_WAIT_HI {
                self.co_capture_wait_hi_coll(gobj);
            } else {
                self.co_capture_pulled_hi_coll(gobj);
            }
        }
        let root = self.root_jobj(gobj);
        let pos = self.game.fighter(gobj).cur_pos;
        jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
    }

    /// A held victim's hooks for its new state, `fn_800DB5D8`: in Yoshi's mouth it turns
    /// invisible (not ported); it can't be thrown out of hitstun for a while.
    #[doc(alias = "fn_800DB5D8")]
    fn capture_wait_hooks(&mut self, gobj: GObjId) {
        self.capture_yoshi_check(gobj);
        self.game.fighter_mut(gobj).x1A6A = 0x1FF;
    }

    /// Held still, high or low, `fn_800DB790` (`fn_800DBAE4`): the state, in Yoshi's mouth
    /// invisible (not ported), and with wobbling prevention its count of the holder's hits
    /// starting over (the Gecko codes at `0x800DB880` and `0x800DBBD4`).
    fn capture_wait_state(&mut self, gobj: GObjId, msid: i32) {
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.capture_yoshi_check(gobj);
        let wobbling = self.game.rules.prevent_wobbling;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A6A = 0x1FF;
        if wobbling {
            fp.mv[0x44] = 0;
            fp.mv[0x46..0x48].copy_from_slice(&[0xFF, 0xFF]);
        }
    }

    /// Hit while held, `ftCo_800DC284` (`ftCo_800DC3A4` low): the hold's flinch, its timer
    /// from 0.
    #[doc(alias = "ftCo_800DC284")]
    #[doc(alias = "ftCo_800DC3A4")]
    pub fn enter_capture_damage(&mut self, gobj: GObjId, high: bool) {
        let msid = if high {
            ms::CAPTURE_DAMAGE_HI
        } else {
            ms::CAPTURE_DAMAGE_LW
        };
        self.change_motion_state(gobj, msid, 0, 0.0, 1.0, 0.0, None);
        self.game.fighter_mut(gobj).set_mv_f32(WAIT_MASH_TIMER, 0.0);
        self.capture_wait_hooks(gobj);
    }

    /// The hold's frame for a victim, `fn_800DB8A4`: its frames, the hold's timer down, and
    /// its mashing.
    #[doc(alias = "fn_800DB8A4")]
    fn capture_tick(&mut self, gobj: GObjId) {
        let c = self.common();
        let (dec, mash) = (c.f32(0x3A4), c.f32(0x3A8));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(
            WAIT_FRAMES,
            fp::frsp(fp::fadd(reg(fp.mv_f32(WAIT_FRAMES)), 1.0)),
        );
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(dec)));
        let mashed = self.grab_mash(gobj, mash);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(WAIT_MASHED, i32::from(mashed));
    }

    /// `ftCo_CaptureDamageHi_Anim` (`ftCo_CaptureDamageLw_Anim`): the hold's frame; when
    /// the flinch ends, held still again.
    #[doc(alias = "ftCo_CaptureDamageHi_Anim")]
    #[doc(alias = "ftCo_CaptureDamageLw_Anim")]
    pub fn co_capture_damage_anim(&mut self, gobj: GObjId) {
        self.capture_tick(gobj);
        if !self.anim_frames_remaining(gobj) {
            let msid = if self.game.fighter(gobj).motion_id == ms::CAPTURE_DAMAGE_HI {
                ms::CAPTURE_WAIT_HI
            } else {
                ms::CAPTURE_WAIT_LW
            };
            self.capture_wait_state(gobj, msid);
        }
    }

    /// `ftCo_CaptureDamageHi_Phys`: to the hold point.
    #[doc(alias = "ftCo_CaptureDamageHi_Phys")]
    pub fn co_capture_damage_hi_phys(&mut self, gobj: GObjId) {
        self.move_to_hold(gobj);
    }

    /// `ftCo_CaptureDamageHi_Coll`: touching down, the low flinch from the same frame
    /// (`fn_800DC384`).
    #[doc(alias = "ftCo_CaptureDamageHi_Coll")]
    #[doc(alias = "fn_800DC384")]
    pub fn co_capture_damage_hi_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.air_coll_touch_then(gobj, |g, gobj| g.capture_damage_switch(gobj, false));
        }
    }

    /// The flinch switched high or low from the same frame, `fn_800DC404` (`fn_800DC624`
    /// high): the hooks, to the hold point; going high also runs the high collision and
    /// moves the model there.
    #[doc(alias = "fn_800DC404")]
    #[doc(alias = "fn_800DC624")]
    fn capture_damage_switch(&mut self, gobj: GObjId, high: bool) {
        let msid = if high {
            ms::CAPTURE_DAMAGE_HI
        } else {
            ms::CAPTURE_DAMAGE_LW
        };
        let frame = self.game.fighter(gobj).cur_anim_frame;
        self.change_motion_state(gobj, msid, UPDATE_CMD, frame, 1.0, 0.0, None);
        self.capture_wait_hooks(gobj);
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        self.follow_hold(gobj, grabber);
        if high {
            self.co_capture_damage_hi_coll(gobj);
            let root = self.root_jobj(gobj);
            let pos = self.game.fighter(gobj).cur_pos;
            jobj::set_translate(&mut self.game.objs.jobjs, root, pos);
        }
    }

    /// `ftCo_CaptureDamageLw_Phys`: to the hold point; pulled up off the ground, the high
    /// flinch.
    #[doc(alias = "ftCo_CaptureDamageLw_Phys")]
    pub fn co_capture_damage_lw_phys(&mut self, gobj: GObjId) {
        if self.move_to_hold(gobj) {
            self.capture_damage_switch(gobj, true);
        }
    }

    /// `ftCo_CaptureDamageLw_Coll`: off a ledge, the high flinch.
    #[doc(alias = "ftCo_CaptureDamageLw_Coll")]
    pub fn co_capture_damage_lw_coll(&mut self, gobj: GObjId) {
        if !self.game.fighter(gobj).x2226_b2 {
            self.ground_coll_fall_off_then(gobj, |g, gobj| g.capture_damage_switch(gobj, true));
        }
    }

    /// Held, `fn_800DB6C8`: high or low as it was pulled, a tether's end given up, and the
    /// rumble.
    #[doc(alias = "fn_800DB6C8")]
    pub fn enter_capture_wait(&mut self, gobj: GObjId) {
        let msid = if self.game.fighter(gobj).motion_id == ms::CAPTURE_PULLED_HI {
            ms::CAPTURE_WAIT_HI
        } else {
            ms::CAPTURE_WAIT_LW
        };
        self.capture_wait_state(gobj, msid);
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        match self.game.fighter(grabber).kind {
            KIND_YOSHI => unported_call("ftCo_800DB368 (held by Yoshi)"),
            KIND_LINK | KIND_CLINK | KIND_SAMUS => {
                unported_call("it_802A7840, it_802BAA94 (a tether's end)")
            }
            _ => {}
        }
        let f = self.game.fighter_id(gobj);
        self.start_rumble(f, 3, 0);
    }

    /// Held by Yoshi, the victim turns invisible with a hurtbox of its own (`fn_800DB790`,
    /// `fn_800DBAE4`): not ported.
    #[doc(alias = "fn_800DB790")]
    #[doc(alias = "fn_800DBAE4")]
    fn capture_yoshi_check(&mut self, gobj: GObjId) {
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        if self.game.fighter(grabber).kind == KIND_YOSHI {
            unported_call("fn_800DB790 (held in Yoshi's mouth)");
        }
    }

    /// `ftCo_CaptureWaitHi_Anim` (`ftCo_CaptureWaitLw_Anim`): the hold counts down, faster
    /// for mashing (`ftCommon_GrabMash`); at 0 the victim breaks free, jumping if it buffered
    /// one or holds up; mashing speeds the struggle's animation for a while.
    // Ports `ftCo_CaptureWaitHi_Anim`; aliased once a check reaches it.
    #[doc(alias = "ftCo_CaptureWaitLw_Anim")]
    pub fn co_capture_wait_anim(&mut self, gobj: GObjId) {
        let c = self.common();
        let (dec, mash, mash_frames, mash_rate) =
            (c.f32(0x3A4), c.f32(0x3A8), c.f32(0x3B0), c.f32(0x3B4));
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(
            WAIT_FRAMES,
            fp::frsp(fp::fadd(reg(fp.mv_f32(WAIT_FRAMES)), 1.0)),
        );
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(dec)));
        let mashed = self.grab_mash(gobj, mash);
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_i32(WAIT_MASHED, i32::from(mashed));
        if fp.grab_timer <= 0.0 {
            let grabber = fp.victim_gobj.expect("held by no one");
            self.enter_catch_cut(grabber, false);
            if self.game.fighter(gobj).mv[WAIT_JUMP] != 0 || self.capture_jump_held(gobj) {
                self.enter_capture_jump(gobj);
            } else {
                self.enter_capture_cut(gobj);
            }
            return;
        }
        let fp = self.game.fighter_mut(gobj);
        if fp.mv_f32(WAIT_MASH_TIMER) != 0.0 {
            fp.set_mv_f32(
                WAIT_MASH_TIMER,
                fp::fsubs(reg(fp.mv_f32(WAIT_MASH_TIMER)), 1.0),
            );
            if fp.mv_f32(WAIT_MASH_TIMER) <= 0.0 && fp.mv_i32(WAIT_MASHED) == 0 {
                self.set_anim_rate(gobj, 1.0);
                self.game.fighter_mut(gobj).set_mv_f32(WAIT_MASH_TIMER, 0.0);
            }
        }
        let fp = self.game.fighter(gobj);
        if fp.mv_f32(WAIT_MASH_TIMER) <= 0.0 && fp.mv_i32(WAIT_MASHED) != 0 {
            self.game
                .fighter_mut(gobj)
                .set_mv_f32(WAIT_MASH_TIMER, reg(mash_frames));
            self.set_anim_rate(gobj, mash_rate);
        }
    }

    /// Mashing out of a hold, `ftCommon_GrabMash`: a button press, or the stick pushed a new
    /// way, takes `amount` off the timer; the player's statistics count the frames and the
    /// mashes (`pl_800402D0`).
    #[doc(alias = "ftCommon_GrabMash")]
    #[doc(alias = "pl_800402D0")]
    pub fn grab_mash(&mut self, gobj: GObjId, amount: f32) -> bool {
        let push = reg(self.common().f32(0x308));
        let fp = self.game.fighter_mut(gobj);
        let mut mashed = false;
        let buttons = PAD_A | super::PAD_B | PAD_XY | super::PAD_LR;
        if fp.input.pressed_buttons & buttons != 0 {
            fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(amount)));
            mashed = true;
        }
        let (r5, r6) = (fp.x1A50, fp.x1A51);
        let (sx, sy) = (reg(fp.input.lstick[0].x), reg(fp.input.lstick[0].y));
        if sx < fp::fneg(push) {
            fp.x1A50 = -1;
        }
        if sx > push {
            fp.x1A50 = 1;
        }
        if sy < fp::fneg(push) {
            fp.x1A51 = -1;
        }
        if sy > push {
            fp.x1A51 = 1;
        }
        if r5 != fp.x1A50 || r6 != fp.x1A51 {
            fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), reg(amount)));
            mashed = true;
        }
        if mashed && fp.x2224_b6 {
            fp.x2224_b5 = true;
            fp.x1A52 = fp.x1A52.wrapping_add(1);
            if fp.x1A52 >= fp.x1A53 {
                fp.x1A52 = 0;
            }
        } else {
            fp.x2224_b5 = false;
        }
        if !fp.is_sub_fighter {
            let slot = i32::from(fp.player_idx);
            let t = self.game.player_stale_moves(slot);
            t.xCEC = t.xCEC.wrapping_add(1);
            if mashed {
                t.xCF0 = t.xCF0.wrapping_add(1);
            }
        }
        mashed
    }

    /// The victims' states' interrupt, `fn_800DC014` (`ftCo_CaptureWaitHi_IASA`,
    /// `ftCo_CaptureWaitLw_IASA`): a jump early in the hold is buffered for the escape.
    #[doc(alias = "fn_800DC014")]
    // Ports `ftCo_CaptureWaitHi_IASA`; aliased once a check reaches it.
    #[doc(alias = "ftCo_CaptureWaitLw_IASA")]
    pub fn co_capture_wait_iasa(&mut self, gobj: GObjId) {
        let window = reg(self.common().f32(0x3AC));
        let fp = self.game.fighter_mut(gobj);
        if reg(fp.mv_f32(WAIT_FRAMES)) < window && fp.input.pressed_buttons & PAD_XY != 0 {
            fp.mv[WAIT_JUMP] = 1;
        }
    }

    /// Whether the stick is up for a jump, `fn_800DC044`.
    // Ports `fn_800DC044`; aliased once a check reaches it.
    fn capture_jump_held(&self, gobj: GObjId) -> bool {
        let t = reg(self.common().f32(0x70));
        reg(self.game.fighter(gobj).input.lstick[0].y) >= t
    }

    /// Breaking free on the ground, `ftCo_CaptureCut_Enter`: the grab ends, pushed back.
    // Ports `ftCo_CaptureCut_Enter`; aliased once a check reaches it.
    pub fn enter_capture_cut(&mut self, gobj: GObjId) {
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        self.grab_release(grabber, gobj);
        let push = reg(self.common().f32(0x370));
        let fp = self.game.fighter_mut(gobj);
        let v = mem(fp::fmuls(fp::fneg(reg(fp.facing_dir)), push));
        if fp.ground_or_air == GA_GROUND {
            fp.gr_vel = v;
        } else {
            fp.self_vel.x = v;
        }
        self.change_motion_state(gobj, ms::CAPTURE_CUT, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_CaptureCut_Phys`: its friction (`ftCommonData` x36C of the ground's) on the
    /// ground, else falling and drifting.
    // Ports `ftCo_CaptureCut_Phys`; aliased once a check reaches it.
    pub fn co_capture_cut_phys(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            let mul = reg(self.common().f32(0x36C));
            let fp = self.game.fighter_mut(gobj);
            let friction = fp::fmuls(mul, reg(fp.co_attrs.ground_friction));
            Self::ground_accel_deaccel(fp, friction);
            self.self_movement_from_ground(gobj);
        } else {
            self.air_physics(gobj);
        }
    }

    /// `ftCo_CaptureCut_Coll`: on the ground, airborne off a ledge (`fn_800DC8D8`); in the
    /// air, landing (`fn_800DC8FC`).
    // Ports `ftCo_CaptureCut_Coll`; aliased once a check reaches it.
    // Ports `fn_800DC8D8`; aliased once a check reaches it.
    // Ports `fn_800DC8FC`; aliased once a check reaches it.
    pub fn co_capture_cut_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).ground_or_air == GA_GROUND {
            self.ground_coll_then(gobj, |g, gobj| {
                let f = g.game.fighter_id(gobj);
                g.go_airborne(f);
            });
        } else {
            unported_call("ft_80083910 from ftCo_CaptureCut_Coll (its registers)");
        }
    }

    /// Jumping free, `fn_800DC070`: airborne, pushed back and up, and the grab ends.
    // Ports `fn_800DC070`; aliased once a check reaches it.
    pub fn enter_capture_jump(&mut self, gobj: GObjId) {
        let grabber = self.game.fighter(gobj).victim_gobj.expect("held by no one");
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let c = self.common();
        let (vx, vy) = (c.f32(0x374), c.f32(0x378));
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel.x = mem(fp::fmuls(fp::fneg(reg(fp.facing_dir)), reg(vx)));
        fp.self_vel.y = vy;
        fp.set_mv_f32(JUMP_FRAMES, 0.0);
        self.grab_release(grabber, gobj);
        self.change_motion_state(gobj, ms::CAPTURE_JUMP, 0, 0.0, 1.0, 0.0, None);
    }

    /// `ftCo_CaptureJump_Anim`: its frames counted; falling once they run out.
    // Ports `ftCo_CaptureJump_Anim`; aliased once a check reaches it.
    pub fn co_capture_jump_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        fp.set_mv_f32(JUMP_FRAMES, fp::fadds(reg(fp.mv_f32(JUMP_FRAMES)), 1.0));
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_CaptureJump_IASA`: past `ftCommonData` x3B8 frames, the aerial actions.
    // Ports `ftCo_CaptureJump_IASA`; aliased once a check reaches it.
    pub fn co_capture_jump_iasa(&mut self, gobj: GObjId) {
        let after = reg(self.common().f32(0x3B8));
        if reg(self.game.fighter(gobj).mv_f32(JUMP_FRAMES)) >= after {
            let _ = self.special_air_check(gobj)
                || self.item_throw_air_check(gobj)
                || self.air_catch_item_check(gobj)
                || self.tether_check(gobj)
                || self.escape_air_check(gobj)
                || self.attack_air_check(gobj)
                || self.air_z_check(gobj)
                || self.jump_aerial_check(gobj);
        }
    }

    /// `ftCo_CaptureJump_Phys`: gravity and drift.
    // Ports `ftCo_CaptureJump_Phys`; aliased once a check reaches it.
    pub fn co_capture_jump_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
        self.air_drift(gobj);
    }

    /// The pull's victim keeps still where it is held (`ftCo_CapturePulledHi_Phys`): the
    /// hold point, as [`Self::move_to_hold`].
    // Ports `ftCo_CapturePulledHi_Anim`; aliased once a check reaches it.
    // Ports `ftCo_CapturePulledHi_IASA`; aliased once a check reaches it.
    #[doc(alias = "ftCo_CapturePulledLw_Anim")]
    #[doc(alias = "ftCo_CapturePulledLw_IASA")]
    #[doc(alias = "ftCo_CatchWait_Anim")]
    #[doc(alias = "ftCo_CatchPull_IASA")]
    // Ports `ftCo_CatchAttack_IASA`; aliased once a check reaches it.
    // Ports `ftCo_CatchCut_IASA`; aliased once a check reaches it.
    // Ports `ftCo_CaptureCut_IASA`; aliased once a check reaches it.
    pub fn co_grab_nothing(&mut self, _gobj: GObjId) {}

    /// `ftCo_CapturePulledHi_Phys` and `ftCo_CaptureWaitHi_Phys`: to the hold point.
    pub fn co_capture_hi_phys(&mut self, gobj: GObjId) {
        let _ = self.move_to_hold(gobj);
    }

    /// `ftCo_CaptureJump_Coll` (`ftCo_AirCatchHit_Coll`).
    // Ports `ftCo_CaptureJump_Coll`; aliased once a check reaches it.
    pub fn co_capture_jump_coll(&mut self, gobj: GObjId) {
        self.co_air_catch_hit_coll(gobj);
    }
}
