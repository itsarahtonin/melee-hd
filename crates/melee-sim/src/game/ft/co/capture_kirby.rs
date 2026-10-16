//! The states of a fighter Kirby inhales (`ftCo_CaptureKirby.c`, `ftCo_CaptureWaitKirby.c`,
//! `ftCo_ThrownKirby.c`): pulled toward his mouth and shrinking, held inside him (hidden,
//! intangible, mashing out, and moving him about while he waits or falls), spat out or popped
//! out as a star that flies, slows and bounces off surfaces, and the tumble out of the star
//! growing back to its size.

use gekko_fp as fp;

use super::super::super::handles::CodeRef;
use super::super::super::state::Running;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::game::structs::{Vec2, ftCollisionBox};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::vec::Vec3;

/// `Ft_Kind_Kirby`, `Ft_Kind_Nana`, `Ft_Kind_Popo`, `Ft_Kind_Boy`, `Ft_Kind_Sandbag`.
const KIND_KIRBY: i32 = 4;
const KIND_POPO: i32 = 10;
const KIND_NANA: i32 = 11;
const KIND_BOY: i32 = 29;
const KIND_SANDBAG: i32 = 32;

/// `FtPart_YRotN`.
const PART_Y_ROT_N: i32 = 3;

/// `ftCo_800DB464`, a held fighter following its holder.
const HELD_FOLLOW: u32 = 0x800D_B464;

/// The states' callbacks by address.
pub mod addr {
    pub const DIST2: u32 = 0x800B_D19C;
    pub const CAPTURE: u32 = 0x800B_D1DC;
    pub const CAPTURE_ANIM: u32 = 0x800B_D38C;
    pub const CAPTURE_IASA: u32 = 0x800B_D390;
    pub const CAPTURE_PHYS: u32 = 0x800B_D394;
    pub const CAPTURE_COLL: u32 = 0x800B_D398;
    pub const PULLED: u32 = 0x800B_D39C;
    pub const CAPTURE_WAIT: u32 = 0x800B_D620;
    pub const CAPTURE_WAIT_ANIM: u32 = 0x800B_D6E8;
    pub const MOVE_KIRBY: u32 = 0x800B_D6EC;
    pub const CAPTURE_WAIT_IASA: u32 = 0x800B_D7BC;
    pub const CAPTURE_WAIT_PHYS: u32 = 0x800B_D9D8;
    pub const CAPTURE_WAIT_COLL: u32 = 0x800B_D9DC;
    pub const STAR_COLL_NORMAL: u32 = 0x800B_DA74;
    pub const THROWN_KIRBY_STAR: u32 = 0x800B_DB58;
    pub const THROWN_KIRBY_STAR_ANIM: u32 = 0x800B_DDC4;
    pub const THROWN_KIRBY_STAR_IASA: u32 = 0x800B_DDC8;
    pub const THROWN_KIRBY_STAR_PHYS: u32 = 0x800B_DDCC;
    pub const THROWN_KIRBY_STAR_COLL: u32 = 0x800B_DF10;
    pub const THROWN_COPY_STAR: u32 = 0x800B_E000;
    pub const THROWN_COPY_STAR_ANIM: u32 = 0x800B_E258;
    pub const THROWN_COPY_STAR_IASA: u32 = 0x800B_E25C;
    pub const THROWN_COPY_STAR_PHYS: u32 = 0x800B_E260;
    pub const THROWN_COPY_STAR_COLL: u32 = 0x800B_E3A4;
    pub const THROWN_KIRBY: u32 = 0x800B_E494;
    pub const THROWN_KIRBY_ANIM: u32 = 0x800B_E618;
    pub const THROWN_KIRBY_IASA: u32 = 0x800B_E61C;
    pub const THROWN_KIRBY_PHYS: u32 = 0x800B_E620;
    pub const THROWN_KIRBY_COLL: u32 = 0x800B_E6A8;
    pub const THROWN_KIRBY_GROW: u32 = 0x800B_E6AC;
    pub const REFRESH_SCALE: u32 = 0x800B_E7C0;
}

/// `ftCommon_MotionVars_capturekirby`: the offset from Kirby's mouth (`pos_offset`), the range
/// it shrinks within and its shrink (`x8`), the pull's speeds (`x10`), the last mash (`x18`)
/// and its own scale (`scale`).
mod cap {
    pub const OFFSET: usize = 0x0;
    pub const RANGE: usize = 0x8;
    pub const SHRINK: usize = 0xC;
    pub const SPEED_X: usize = 0x10;
    pub const SPEED_Y: usize = 0x14;
    pub const MASH: usize = 0x18;
    pub const SCALE: usize = 0x2C;
}

/// `ftCommon_MotionVars_thrownkirby`: Kirby (`thrower_gobj`), the star's slowing (`x4`), the
/// wall it last bounced off (`x8`), the tumble's shrink (`xC`) and time (`x10`), the last mash
/// (`x14`), whether it's a swallowed fighter's star (`x18_b0`) and whether it was a Kirby with
/// a copy (`x18_b1`), its scale and the star's collision box.
mod thr {
    pub const THROWER: usize = 0x0;
    pub const DECEL: usize = 0x4;
    pub const WALL: usize = 0x8;
    pub const SHRINK: usize = 0xC;
    pub const TIME: usize = 0x10;
    pub const MASH: usize = 0x14;
    pub const FLAGS: usize = 0x18;
    pub const SCALE: usize = 0x1C;
    pub const BOX: usize = 0x28;
}

fn mv_vec(fp: &crate::game::structs::Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

fn set_mv_vec(fp: &mut crate::game::structs::Fighter, off: usize, v: Vec3) {
    fp.set_mv_f32(off, reg(v.x));
    fp.set_mv_f32(off + 4, reg(v.y));
    fp.set_mv_f32(off + 8, reg(v.z));
}

impl Running<'_> {
    /// The square of a fighter's distance to `pos`, `ftCo_800BD19C`.
    #[doc(alias = "ftCo_800BD19C")]
    pub fn capture_kirby_dist2(&self, gobj: GObjId, pos: Vec3) -> f64 {
        let p = self.game.fighter(gobj).cur_pos;
        let d = |a: f32, b: f32| fp::fsubs(reg(a), reg(b));
        let (dx, dy, dz) = (d(p.x, pos.x), d(p.y, pos.y), d(p.z, pos.z));
        fp::fadds(fp::fmuls(dz, dz), fp::fadds(fp::fmuls(dx, dx), fp::fmuls(dy, dy)))
    }

    /// `ftCommon_InitGrab(fp, 0, timer)`.
    fn init_grab(&mut self, gobj: GObjId, timer: f32) {
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = timer;
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
    }

    /// Caught by Kirby's inhale, `ftCo_800BD1DC` (the catch's `grabbed_cb`): it lets go of
    /// what it held, faces him, airborne, the pull's offset, range, shrink and speeds from
    /// him, its scale kept, his hold's time, still, pulled each frame (`ftCo_800BD39C`).
    #[doc(alias = "ftCo_800BD1DC")]
    pub fn enter_capture_kirby(&mut self, gobj: GObjId, kirby: GObjId) {
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        let facing = self.game.fighter(kirby).facing_dir;
        let fp = self.game.fighter_mut(gobj);
        fp.x1A5C = Some(kirby);
        fp.victim_gobj = Some(kirby);
        fp.x221B.x221B_b5 = false;
        fp.x221B.x221B_b7 = false;
        fp.facing_dir = mem(fp::fneg(reg(facing)));
        self.change_motion_state(gobj, ms::CAPTURE_KIRBY, 0, 0.0, 1.0, 0.0, None);
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        // `inlineA0`.
        let range = self.kb_pull_range(kirby);
        let shrink = self.kb_pull_shrink(kirby);
        let mouth = self.kb_mouth(kirby);
        let (outer, inner) = self.kb_pull_speeds(kirby);
        let root = self.root_jobj(gobj);
        let scale = self.game.objs.jobjs[root].scale;
        let fp = &mut self.game.fighters[f.0];
        fp.set_mv_f32(cap::RANGE, reg(range));
        fp.set_mv_f32(cap::SHRINK, reg(shrink));
        fp.set_mv_f32(cap::OFFSET, fp::fsubs(reg(fp.cur_pos.x), reg(mouth.x)));
        fp.set_mv_f32(cap::OFFSET + 4, fp::fsubs(reg(fp.cur_pos.y), reg(mouth.y)));
        fp.set_mv_f32(cap::SPEED_X, reg(outer));
        fp.set_mv_f32(cap::SPEED_Y, reg(inner));
        set_mv_vec(fp, cap::SCALE, scale);
        fp.x2226_b2 = true;
        let time = self.kb_hold_time(kirby);
        self.init_grab(gobj, time);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.start_rumble(f, 1, 0);
        self.stop_dead(gobj);
        self.game.fighters[f.0].accessory1_cb = Some(CodeRef(addr::PULLED));
    }

    /// The capture's empty callbacks.
    #[doc(alias = "ftCo_CaptureKirby_Anim")]
    #[doc(alias = "ftCo_CaptureKirby_IASA")]
    #[doc(alias = "ftCo_CaptureKirby_Phys")]
    #[doc(alias = "ftCo_CaptureKirby_Coll")]
    #[doc(alias = "ftCo_CaptureWaitKirby_Anim")]
    #[doc(alias = "ftCo_CaptureWaitKirby_Phys")]
    #[doc(alias = "ftCo_CaptureWaitKirby_Coll")]
    #[doc(alias = "ftCo_ThrownKirbyStar_Anim")]
    #[doc(alias = "ftCo_ThrownKirbyStar_IASA")]
    #[doc(alias = "ftCo_ThrownCopyStar_Anim")]
    #[doc(alias = "ftCo_ThrownCopyStar_IASA")]
    #[doc(alias = "ftCo_ThrownKirby_Anim")]
    #[doc(alias = "ftCo_ThrownKirby_IASA")]
    #[doc(alias = "ftCo_ThrownKirby_Coll")]
    pub fn capture_kirby_nothing(&mut self, _gobj: GObjId) {}

    /// The pull toward his mouth (`ftCo_800BD39C`, accessory 1): shrinking within the range
    /// (`inlineB0`), the offset closing by at most the pull's speeds (`inlineB1`), its place
    /// his mouth's plus what's left.
    #[doc(alias = "ftCo_800BD39C")]
    pub fn capture_kirby_pulled(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let mut pos = Vec3::new(fp.mv_f32(cap::OFFSET), fp.mv_f32(cap::OFFSET + 4), 0.0);
        // `inlineB0`.
        let dist = reg(pos.length());
        let range = reg(fp.mv_f32(cap::RANGE));
        if dist < range {
            let shrink = reg(fp.mv_f32(cap::SHRINK));
            let t = fp::fmuls(fp::fdivs(dist, range), shrink);
            let s = fp::fadds(fp::fsubs(1.0, shrink), t);
            let own = mv_vec(fp, cap::SCALE);
            let scale = Vec3::new(
                mem(fp::fmuls(s, reg(own.x))),
                mem(fp::fmuls(s, reg(own.y))),
                mem(fp::fmuls(s, reg(own.z))),
            );
            let root = self.root_jobj(gobj);
            jobj::set_scale(&mut self.game.objs.jobjs, root, scale);
        }
        // `inlineB1`.
        let fp = &self.game.fighters[f.0];
        let sign = |v: f64| fp::frsp(if v < fp::frsp(0.0) { -1.0 } else { 1.0 });
        let abs = |v: f64| if v < fp::frsp(0.0) { fp::fneg(v) } else { v };
        let (sx, sy) = (reg(fp.mv_f32(cap::SPEED_X)), reg(fp.mv_f32(cap::SPEED_Y)));
        if abs(reg(pos.x)) > sx {
            pos.x = mem(fp::fmuls(sx, sign(reg(pos.x))));
        }
        if abs(reg(pos.y)) > sy {
            pos.y = mem(fp::fmuls(sy, sign(reg(pos.y))));
        }
        let fp = &mut self.game.fighters[f.0];
        let ox = fp::fsubs(reg(fp.mv_f32(cap::OFFSET)), reg(pos.x));
        let oy = fp::fsubs(reg(fp.mv_f32(cap::OFFSET + 4)), reg(pos.y));
        fp.set_mv_f32(cap::OFFSET, ox);
        fp.set_mv_f32(cap::OFFSET + 4, oy);
        let kirby = fp.victim_gobj.expect("pulled by no one");
        let mouth = self.kb_mouth(kirby);
        let fp = &mut self.game.fighters[f.0];
        fp.cur_pos.x = mem(fp::fadds(reg(mouth.x), reg(fp.mv_f32(cap::OFFSET))));
        fp.cur_pos.y = mem(fp::fadds(reg(mouth.y), reg(fp.mv_f32(cap::OFFSET + 4))));
        fp.cur_pos.z = mouth.z;
    }

    /// Inside Kirby, `ftCo_800BD620`: fixed to him, hidden and intangible, airborne and still.
    #[doc(alias = "ftCo_800BD620")]
    pub fn enter_capture_wait_kirby(&mut self, gobj: GObjId) {
        let kirby = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("inhaled by no one");
        self.game.fighter_mut(gobj).x2226_b2 = false;
        self.attach_to_thrower(kirby, gobj);
        self.change_motion_state(gobj, ms::CAPTURE_WAIT_KIRBY, 0x90, 0.0, 1.0, 0.0, None);
        self.update_model_scale(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.accessory1_cb = Some(CodeRef(HELD_FOLLOW));
        fp.invisible = true;
        self.set_hit_status(gobj, 2);
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.anim_update(gobj);
        self.stop_dead(gobj);
    }

    /// Its stick moving Kirby up or down, `ftCo_800BD6EC`: a tap up hops him up; a tap down
    /// on a platform drops him through it.
    #[doc(alias = "ftCo_800BD6EC")]
    pub fn capture_kirby_move_kirby(&mut self, gobj: GObjId) {
        let c = self.common();
        let (tap, window) = (reg(c.f32(0x70)), c.i32(0x74));
        let (down, down_window) = (reg(c.f32(0x464)), reg(c.f32(0x468)));
        let fp = self.game.fighter(gobj);
        let kirby = fp.victim_gobj.expect("inhaled by no one");
        let y = reg(fp.input.lstick[0].y);
        let timer = i32::from(fp.active_timer.lstick.y);
        if y >= tap && timer < window {
            self.game.fighter_mut(gobj).active_timer.lstick.y = 254;
            self.kb_victim_hop(kirby, 1);
            return;
        }
        if y <= down && fp::frsp(f64::from(timer)) < down_window && self.kb_on_platform(kirby) {
            self.game.fighter_mut(gobj).active_timer.lstick.y = 254;
            self.kb_victim_hop(kirby, -1);
            self.kb_skip_floor(kirby);
        }
    }

    /// `ftCo_CaptureWaitKirby_IASA`: while Kirby waits or falls, its stick moves him; its time
    /// runs down by his divisor and by mashing, and runs out into breaking free, its scale
    /// back, Kirby letting go.
    #[doc(alias = "ftCo_CaptureWaitKirby_IASA")]
    pub fn co_capture_wait_kirby_iasa(&mut self, gobj: GObjId) {
        let kirby = self
            .game
            .fighter(gobj)
            .victim_gobj
            .expect("inhaled by no one");
        if self.kb_victim_can_move(kirby) {
            self.capture_kirby_move_kirby(gobj);
            // `inlineB0`.
            let c = self.common();
            let (threshold, window, extra) = (reg(c.f32(0x3C)), c.i32(0x40), reg(c.f32(0x44)));
            let fp = self.game.fighter(gobj);
            let mut x = reg(fp.input.lstick[0].x);
            if x < fp::frsp(0.0) {
                x = fp::fneg(x);
            }
            let timer = fp::frsp(f64::from(i32::from(fp.active_timer.lstick.x)));
            if x >= threshold && timer < fp::fadds(fp::frsp(f64::from(window)), extra) {
                let fp = self.game.fighter_mut(gobj);
                fp.active_timer.lstick.x = 254;
                let v = if fp.input.lstick[0].x < 0.0 { -1.0 } else { 1.0 };
                self.kb_victim_push(kirby, v);
            }
        }
        let div = reg(self.kb_hold_divisor(kirby));
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), div));
        let res = self.kb_hold_resistance(kirby);
        let mashed = self.grab_mash(gobj, res);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(cap::MASH, i32::from(mashed));
        if reg(self.game.fighter(gobj).grab_timer) <= fp::frsp(0.0) {
            let facing = self.kb_victim_facing(kirby);
            let fp = self.game.fighter_mut(gobj);
            fp.facing_dir = facing;
            let scale = mv_vec(fp, cap::SCALE);
            let root = self.root_jobj(gobj);
            jobj::set_scale(&mut self.game.objs.jobjs, root, scale);
            self.enter_capture_cut(gobj);
            self.enter_catch_cut(kirby, false);
        }
    }

    /// The kind whose power Kirby swallows, `ftCo_800BD9E0`: a Kirby's copy (marking it lost,
    /// `ftCo_800BDA50`), Kirby's own copy for the kinds without one, and the Ice Climbers as
    /// Popo.
    // Ports `ftCo_800BDA50` (a swallowed Kirby's copy); aliased once a check reaches it.
    #[doc(alias = "ftCo_800BD9E0")]
    pub fn copy_kind_of(&mut self, gobj: GObjId, victim: GObjId) -> i32 {
        let vf = self.game.fighter(victim);
        let kind = vf.kind;
        if kind == KIND_KIRBY {
            let hat = vf.u_i32(0xC);
            if hat != KIND_KIRBY {
                self.game.fighter_mut(victim).mv[thr::FLAGS] |= 0x40;
            }
            return hat;
        }
        if (kind.wrapping_sub(KIND_BOY) as u32) <= 1 || kind == KIND_SANDBAG {
            return self.game.fighter(gobj).u_i32(0xC);
        }
        if kind == KIND_NANA {
            return KIND_POPO;
        }
        kind
    }

    /// The normal of the surface the star meets, `ftCo_800BDA74`: the floor its box lands on,
    /// else a wall or the ceiling it touches, else none.
    #[doc(alias = "ftCo_800BDA74")]
    pub fn thrown_kirby_star_normal(&mut self, gobj: GObjId) -> Vec3 {
        let fp = self.game.fighter(gobj);
        let b = star_box(fp);
        let landed = self.air_coll_box(gobj, &b);
        let c = &self.game.fighter(gobj).coll_data;
        if landed {
            c.floor.normal
        } else if c.env_flags & 0xFC0 != 0 {
            c.right_facing_wall.normal
        } else if c.env_flags & 0x3F != 0 {
            c.left_facing_wall.normal
        } else if c.env_flags & (0x2000 | 0x4000) != 0 {
            c.ceiling.normal
        } else {
            Vec3::new(0.0, 0.0, 0.0)
        }
    }

    /// Into a star (`inlineB2`): facing Kirby, its velocity and slowing from him (`vel`), its
    /// scale kept, hidden and intangible, its time (`time`), airborne, the star (`ftData`'s
    /// `x48_items[4]`, his `ftKb_SpecialN_800F5898`) at its height scaled on its YRotN.
    fn enter_kirby_star(
        &mut self,
        gobj: GObjId,
        kirby: GObjId,
        msid: i32,
        swallowed: bool,
    ) -> Vec3 {
        let facing = mem(fp::fneg(reg(self.game.fighter(kirby).facing_dir)));
        let fp = self.game.fighter_mut(gobj);
        fp.facing_dir = facing;
        fp.mv.set_gobj_at(thr::THROWER, Some(kirby));
        fp.set_mv_f32(thr::WALL, 0.0);
        let (vel, decel) = if swallowed {
            self.kb_swallow_vel(kirby, facing)
        } else {
            self.kb_spit_vel(kirby, facing)
        };
        let fp = self.game.fighter_mut(gobj);
        fp.self_vel = vel;
        fp.set_mv_f32(thr::DECEL, reg(decel));
        self.update_model_scale(gobj);
        let root = self.root_jobj(gobj);
        let scale = self.game.objs.jobjs[root].scale;
        set_mv_vec(self.game.fighter_mut(gobj), thr::SCALE, scale);
        self.change_motion_state(gobj, msid, 0x100, 0.0, 1.0, 0.0, Some(kirby));
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_2_cb = Some(CodeRef(addr::REFRESH_SCALE));
        fp.take_dmg_cb = Some(CodeRef(addr::REFRESH_SCALE));
        fp.x1A6A = 0x1FF;
        fp.invisible = true;
        self.set_hit_status(gobj, 2);
        self.game.fighter_mut(gobj).x209A = 1;
        let time = if swallowed {
            self.kb_swallow_star_time(kirby)
        } else {
            self.kb_spit_star_time(kirby)
        };
        self.init_grab(gobj, time);
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let fp = self.game.fighter_mut(gobj);
        fp.mv[thr::FLAGS] = (fp.mv[thr::FLAGS] & 0x3F) | (u8::from(swallowed) << 7);
        let star = self.kb_star_model(kirby);
        self.set_accessory(gobj, star);
        let s = self.game.fighter(gobj).co_attrs.xDC;
        let scale = Vec3::new(s, s, s);
        let acc = self.game.fighter(gobj).x20A0_accessory.expect("the star");
        jobj::set_scale(&mut self.game.objs.jobjs, acc, scale);
        let y = self.part_joint(gobj, PART_Y_ROT_N);
        crate::lb::anim::attach(&mut self.game.objs, Some(acc), y);
        scale
    }

    /// Spat out as a star, `ftCo_800BDB58`: its hitbox's damage Kirby's star's, its box from
    /// him at its size.
    #[doc(alias = "ftCo_800BDB58")]
    pub fn enter_thrown_kirby_star(&mut self, gobj: GObjId, kirby: GObjId) {
        let scale = self.enter_kirby_star(gobj, kirby, ms::THROWN_KIRBY_STAR, false);
        let dmg = crate::ppc::cvt_fp2unsigned(reg(self.game.fighter(gobj).co_attrs.kirby_b_star_damage));
        let f = self.game.fighter_id(gobj);
        self.hitbox_damage(f, 0, false, dmg);
        let b = self.kb_star_box(kirby, scale.x);
        set_star_box(self.game.fighter_mut(gobj), b);
    }

    /// Popped out of Kirby as a star after he swallows it, `ftCo_800BE000`: no hitboxes, its
    /// box from him at its size.
    #[doc(alias = "ftCo_800BE000")]
    pub fn enter_thrown_copy_star(&mut self, gobj: GObjId, kirby: GObjId) {
        let scale = self.enter_kirby_star(gobj, kirby, ms::THROWN_COPY_STAR, true);
        let f = self.game.fighter_id(gobj);
        self.clear_hitboxes(f);
        let b = self.kb_star_box(kirby, scale.x);
        set_star_box(self.game.fighter_mut(gobj), b);
    }

    /// The star's flight (`ftCo_ThrownKirbyStar_Phys`, `ftCo_ThrownCopyStar_Phys`): slowing
    /// by its slowing (stopping once slower), facing up or down its flight; its time running
    /// down by Kirby's divisor and by mashing, out of the star when it runs out.
    #[doc(alias = "ftCo_ThrownKirbyStar_Phys")]
    #[doc(alias = "ftCo_ThrownCopyStar_Phys")]
    pub fn co_thrown_kirby_star_phys(&mut self, gobj: GObjId) {
        let div = reg(self.kb_star_divisor());
        let fp = self.game.fighter_mut(gobj);
        let (vx, vy) = (reg(fp.self_vel.x), reg(fp.self_vel.y));
        let mag = crate::math::sqrt_reg(fp::fadds(fp::fmuls(vx, vx), fp::fmuls(vy, vy)));
        let decel = reg(fp.mv_f32(thr::DECEL));
        if mag > decel {
            fp.self_vel.x = mem(fp::fdivs(fp::fmuls(vx, fp::fsubs(mag, decel)), mag));
            fp.self_vel.y = mem(fp::fdivs(fp::fmuls(vy, fp::fsubs(mag, decel)), mag));
            fp.facing_dir = if reg(fp.self_vel.y) < fp::frsp(0.0) {
                -1.0
            } else {
                1.0
            };
        } else {
            fp.self_vel.x = 0.0;
        }
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), div));
        let amount = self.kb_star_decel();
        let mashed = self.grab_mash(gobj, amount);
        self.game
            .fighter_mut(gobj)
            .set_mv_i32(thr::MASH, i32::from(mashed));
        if reg(self.game.fighter(gobj).grab_timer) <= fp::frsp(0.0) {
            self.enter_thrown_kirby(gobj);
        }
    }

    /// The star bouncing off a surface (`ftCo_ThrownKirbyStar_Coll`, `ftCo_ThrownCopyStar_Coll`):
    /// its velocity mirrored in the surface's normal; met head on, out of the star, the wall's
    /// side kept.
    #[doc(alias = "ftCo_ThrownKirbyStar_Coll")]
    #[doc(alias = "ftCo_ThrownCopyStar_Coll")]
    pub fn co_thrown_kirby_star_coll(&mut self, gobj: GObjId) {
        let n = self.thrown_kirby_star_normal(gobj);
        if n.x != 0.0 || n.y != 0.0 || n.z != 0.0 {
            let fp = self.game.fighter_mut(gobj);
            let before = fp.self_vel;
            fp.self_vel.mirror_xy(n);
            let dot = fp::fmadds(
                reg(before.x),
                reg(fp.self_vel.x),
                fp::fmuls(reg(before.y), reg(fp.self_vel.y)),
            );
            if dot < fp::frsp(0.0) {
                self.enter_thrown_kirby(gobj);
                let side = if reg(n.x) < fp::frsp(0.0) { -1.0 } else { 1.0 };
                self.game.fighter_mut(gobj).set_mv_f32(thr::WALL, side);
            }
        }
    }

    /// Out of the star, `ftCo_800BE494`: tumbling out, tangible and seen; a spat fighter flies
    /// at Kirby's tumble's speed (away from the wall it met, else on its way), a Kirby who had
    /// a copy loses it; its growth's time and shrink, airborne, growing each frame.
    #[doc(alias = "ftCo_800BE494")]
    pub fn enter_thrown_kirby(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::THROWN_KIRBY, 0x40, 0.0, 1.0, 0.0, None);
        let fp = self.game.fighter_mut(gobj);
        fp.take_dmg_2_cb = Some(CodeRef(addr::REFRESH_SCALE));
        fp.take_dmg_cb = Some(CodeRef(addr::REFRESH_SCALE));
        fp.x1A6A = 0x1FF;
        fp.invisible = false;
        self.set_hit_status(gobj, 0);
        let fp = self.game.fighter(gobj);
        if fp.mv[thr::FLAGS] & 0x80 == 0 {
            let v = self.kb_thrown_vel();
            let fp = self.game.fighter_mut(gobj);
            fp.self_vel.y = v.y;
            let wall = reg(fp.mv_f32(thr::WALL));
            fp.self_vel.x = if wall != 0.0 {
                mem(fp::fmuls(wall, reg(v.x)))
            } else {
                let s = fp::frsp(if reg(fp.self_vel.x) < fp::frsp(0.0) {
                    -1.0
                } else {
                    1.0
                });
                mem(fp::fmuls(reg(v.x), s))
            };
        } else if fp.kind == KIND_KIRBY && fp.mv[thr::FLAGS] & 0x40 != 0 && fp.u[0x8] & 0x80 == 0 {
            let kind = fp.u_i32(0xC);
            self.kb_reset_copy_vars(gobj);
            self.kb_remove_hat(gobj, kind);
            self.game.fighter_mut(gobj).set_u_i32(0xC, KIND_KIRBY);
        }
        let spin = self.kb_thrown_spin();
        self.game
            .fighter_mut(gobj)
            .set_mv_f32(thr::TIME, reg(spin));
        self.init_grab(gobj, spin);
        let shrink = self.kb_thrown_shrink();
        self.game
            .fighter_mut(gobj)
            .set_mv_f32(thr::SHRINK, reg(shrink));
        self.anim_update(gobj);
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        self.game.fighters[f.0].accessory4_cb = Some(CodeRef(addr::THROWN_KIRBY_GROW));
    }

    /// The tumble out of the star (`ftCo_ThrownKirby_Phys`): a swallowed fighter's falls at
    /// once, a spat one when its time runs out, its scale back.
    #[doc(alias = "ftCo_ThrownKirby_Phys")]
    pub fn co_thrown_kirby_phys(&mut self, gobj: GObjId) {
        let div = reg(self.kb_star_divisor());
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), div));
        let swallowed = fp.mv[thr::FLAGS] & 0x80 != 0;
        if swallowed || reg(fp.grab_timer) <= fp::frsp(0.0) {
            self.update_model_scale(gobj);
            self.enter_fall(gobj);
        }
    }

    /// Growing back to its size as its time runs out, `ftCo_800BE6AC` (accessory 4).
    #[doc(alias = "ftCo_800BE6AC")]
    pub fn thrown_kirby_grow(&mut self, gobj: GObjId) {
        let fp = self.game.fighter(gobj);
        let shrink = reg(fp.mv_f32(thr::SHRINK));
        let time = reg(fp.mv_f32(thr::TIME));
        let t = fp::fmuls(fp::fdivs(fp::fsubs(time, reg(fp.grab_timer)), time), shrink);
        let s = fp::fadds(fp::fsubs(1.0, shrink), t);
        let own = mv_vec(fp, thr::SCALE);
        let scale = Vec3::new(
            mem(fp::fmuls(s, reg(own.x))),
            mem(fp::fmuls(s, reg(own.y))),
            mem(fp::fmuls(s, reg(own.z))),
        );
        let root = self.root_jobj(gobj);
        jobj::set_scale(&mut self.game.objs.jobjs, root, scale);
    }

    /// Its scale back when hit, `ftCo_800BE7C0`.
    #[doc(alias = "ftCo_800BE7C0")]
    pub fn thrown_kirby_refresh_scale(&mut self, gobj: GObjId) {
        self.update_model_scale(gobj);
    }

    /// The star's box from Kirby at the victim's size, `ftKb_SpecialN_800F5820`.
    #[doc(alias = "ftKb_SpecialN_800F5820")]
    pub fn kb_star_box(&self, kirby: GObjId, scale: f32) -> ftCollisionBox {
        use super::super::kirby::attr;
        let f = self.game.fighter_id(kirby);
        let s = reg(scale);
        let a = |off| mem(fp::fmuls(reg(self.kb_attr_f32(f, off)), s));
        ftCollisionBox {
            top: a(attr::N_BOX_TOP),
            bottom: a(attr::N_BOX_BOTTOM),
            left: Vec2 { x: a(attr::N_BOX_LEFT_X), y: a(attr::N_BOX_LEFT_Y) },
            right: Vec2 { x: a(attr::N_BOX_RIGHT_X), y: a(attr::N_BOX_RIGHT_Y) },
        }
    }

    /// The tumble's speed out of the star, Kirby's data's (`ftKb_SpecialN_800F5874`: his
    /// common data's `x9C` and `xA0`).
    #[doc(alias = "ftKb_SpecialN_800F5874")]
    pub fn kb_thrown_vel(&self) -> Vec2 {
        let d = self.game.ft.data.data_list[KIND_KIRBY as usize]
            .expect("Kirby's data without Kirby");
        let ext = self.data.ptr(d.at(4)).expect("ftData::ext_attr");
        Vec2 {
            x: self.data.f32(ext.at(0x9C)),
            y: self.data.f32(ext.at(0xA0)),
        }
    }

    /// The star's model, Kirby's `ftData::x48_items[4]` (`ftKb_SpecialN_800F5898`).
    #[doc(alias = "ftKb_SpecialN_800F5898")]
    pub fn kb_star_model(&self, kirby: GObjId) -> melee_data::archive::DataRef {
        let d = self.game.fighter(kirby).ft_data.expect("a fighter without data");
        let items = self.data.ptr(d.at(0x48)).expect("ftData::x48_items");
        self.data.ptr(items.at(16)).expect("Kirby's star")
    }
}

/// The star's collision box in the thrown states' variables.
fn star_box(fp: &crate::game::structs::Fighter) -> ftCollisionBox {
    let f = |o| fp.mv_f32(thr::BOX + o);
    ftCollisionBox {
        top: f(0),
        bottom: f(4),
        left: Vec2 { x: f(8), y: f(0xC) },
        right: Vec2 { x: f(0x10), y: f(0x14) },
    }
}

fn set_star_box(fp: &mut crate::game::structs::Fighter, b: ftCollisionBox) {
    for (o, v) in [
        (0, b.top),
        (4, b.bottom),
        (8, b.left.x),
        (0xC, b.left.y),
        (0x10, b.right.x),
        (0x14, b.right.y),
    ] {
        fp.set_mv_f32(thr::BOX + o, reg(v));
    }
}

/// The victim's side of the inhale's catch, by `grabbed_cb`.
pub fn run_grabbed(g: &mut Running, victim: GObjId, kirby: GObjId, cb: u32) -> bool {
    match cb {
        addr::CAPTURE => g.enter_capture_kirby(victim, kirby),
        _ => return false,
    }
    true
}

/// Runs one of the victim's state callbacks, if `addr` is one.
pub fn run(g: &mut Running, gobj: GObjId, a: u32) -> bool {
    match a {
        addr::CAPTURE_ANIM
        | addr::CAPTURE_IASA
        | addr::CAPTURE_PHYS
        | addr::CAPTURE_COLL
        | addr::CAPTURE_WAIT_ANIM
        | addr::CAPTURE_WAIT_PHYS
        | addr::CAPTURE_WAIT_COLL
        | addr::THROWN_KIRBY_STAR_ANIM
        | addr::THROWN_KIRBY_STAR_IASA
        | addr::THROWN_COPY_STAR_ANIM
        | addr::THROWN_COPY_STAR_IASA
        | addr::THROWN_KIRBY_ANIM
        | addr::THROWN_KIRBY_IASA
        | addr::THROWN_KIRBY_COLL => g.capture_kirby_nothing(gobj),
        addr::PULLED => g.capture_kirby_pulled(gobj),
        addr::CAPTURE_WAIT => g.enter_capture_wait_kirby(gobj),
        addr::MOVE_KIRBY => g.capture_kirby_move_kirby(gobj),
        addr::CAPTURE_WAIT_IASA => g.co_capture_wait_kirby_iasa(gobj),
        addr::THROWN_KIRBY_STAR_PHYS | addr::THROWN_COPY_STAR_PHYS => {
            g.co_thrown_kirby_star_phys(gobj)
        }
        addr::THROWN_KIRBY_STAR_COLL | addr::THROWN_COPY_STAR_COLL => {
            g.co_thrown_kirby_star_coll(gobj)
        }
        addr::THROWN_KIRBY => g.enter_thrown_kirby(gobj),
        addr::THROWN_KIRBY_PHYS => g.co_thrown_kirby_phys(gobj),
        addr::THROWN_KIRBY_GROW => g.thrown_kirby_grow(gobj),
        addr::REFRESH_SCALE => g.thrown_kirby_refresh_scale(gobj),
        _ => return false,
    }
    true
}
