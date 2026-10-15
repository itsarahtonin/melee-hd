//! Buried by a hit (`ftCo_Bury.c`): a ground-element hit on a grounded fighter (Donkey Kong's
//! Headbutt) sinks it into the floor, where it waits, mashing out, until it jumps free. The
//! stage devices that bury (`ftCo_800C08A0`, Jungle Japes' and the like) aren't ported.

// The original's comparisons are kept as written: a NaN fails them.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use gekko_fp as fp;

use super::super::super::code::{Val, unported_call};
use super::super::super::state::{Obj, Running};
use super::super::super::structs::{Fighter, Vec2, ftCollisionBox};
use super::super::parts::HIP_N;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::hsd::jobj;
use crate::lb::joint::world_position;
use crate::vec::Vec3;

/// `ftCommon_MotionVars_bury`: the frames sinking (`x0`), the collision box (`coll_box`), the
/// box's sink per frame (`x1C`), the floor (`x20`) and the effect's place (`translate`).
const BURY_FRAMES: usize = 0x0;
const BURY_BOX: usize = 0x4;
const BURY_SINK: usize = 0x1C;
const BURY_FLOOR: usize = 0x20;
const BURY_TRANSLATE: usize = 0x24;
/// `ftCommon_MotionVars_buryjump::x0`: the jump's frames.
const BURY_JUMP_FRAMES: usize = 0x0;

/// The bury effect.
const EFFECT: i32 = 0x447;

fn mv_box(fp: &Fighter, off: usize) -> ftCollisionBox {
    let f = |i: usize| fp.mv_f32(off + 4 * i);
    ftCollisionBox {
        top: f(0),
        bottom: f(1),
        left: Vec2 { x: f(2), y: f(3) },
        right: Vec2 { x: f(4), y: f(5) },
    }
}

fn set_mv_box(fp: &mut Fighter, off: usize, b: &ftCollisionBox) {
    for (i, x) in [b.top, b.bottom, b.left.x, b.left.y, b.right.x, b.right.y]
        .into_iter()
        .enumerate()
    {
        fp.mv[off + 4 * i..off + 4 * i + 4].copy_from_slice(&x.to_bits().to_be_bytes());
    }
}

fn mv_vec(fp: &Fighter, off: usize) -> Vec3 {
    Vec3::new(fp.mv_f32(off), fp.mv_f32(off + 4), fp.mv_f32(off + 8))
}

fn set_mv_vec(fp: &mut Fighter, off: usize, v: Vec3) {
    for (i, x) in [v.x, v.y, v.z].into_iter().enumerate() {
        fp.mv[off + 4 * i..off + 4 * i + 4].copy_from_slice(&x.to_bits().to_be_bytes());
    }
}

impl Running<'_> {
    /// Buried, `ftCo_800C0D0C`: the hit's hooks, the held item and victim let go, hitstun's
    /// setup, landed, the state, still, its mash timer by its damage, handicap and rank
    /// (`ftCommonData` x5F4 to x60C), the box sinking from the ECB's to the hip over x5F4
    /// frames on the floor it stands on, and the bury effect at its feet.
    #[doc(alias = "ftCo_800C0D0C")]
    #[doc(alias = "ftCo_ReleaseItemAndVictim")]
    pub fn enter_bury(&mut self, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        // `ftCo_ReleaseItemAndVictim`.
        self.damage_hooks(gobj);
        self.drop_heavy_item(gobj);
        self.release_victim(gobj);
        self.enter_damage(gobj, -1, 0.0);
        self.land(gobj);
        self.change_motion_state(gobj, ms::BURY, 0x1080, 0.0, 1.0, 0.0, None);
        self.stop_dead(gobj);
        let slot = i32::from(self.game.fighters[f.0].player_idx);
        let c = self.common();
        let rank = self.game.player_rank(slot).wrapping_add(1);
        let by_rank = fp::fmuls(
            reg(c.f32(0x604)),
            fp::fsubs(reg(c.f32(0x608)), fp::frsp(f64::from(rank))),
        );
        let handicap = self.game.player_handicap(slot);
        let by_handicap = fp::fsubs(reg(c.f32(0x600)), fp::frsp(f64::from(handicap)));
        let base = fp::fadds(
            fp::fmadds(reg(c.f32(0x5FC)), by_handicap, reg(c.f32(0x5F8))),
            by_rank,
        );
        let frames = c.i32(0x5F4);
        let fp = &mut self.game.fighters[f.0];
        let timer = fp::fmadds(reg(fp.dmg.x1830_percent), reg(c.f32(0x60C)), base);
        // `ftCommon_InitGrab(fp, 0, timer)`.
        fp.grab_timer = mem(timer);
        fp.x1A51 = 0;
        fp.x1A50 = 0;
        fp.x2224_b6 = false;
        fp.x1A6A = 0x1FF;
        fp.x221D_b5 = true;
        fp.x2220_b3 = true;
        fp.x2224_b4 = true;
        fp.set_mv_i32(BURY_FRAMES, frames);
        let floor = fp.coll_data.floor.index;
        fp.set_mv_i32(BURY_FLOOR, floor);
        // `ft_80084CB0`: the ECB's box now.
        let e = &fp.coll_data.ecb;
        let b = ftCollisionBox {
            top: e.top.y,
            bottom: e.bottom.y,
            right: Vec2 {
                x: e.right.x,
                y: e.right.y,
            },
            left: Vec2 {
                x: e.left.x,
                y: e.left.y,
            },
        };
        set_mv_box(fp, BURY_BOX, &b);
        let root = self.bone_joint(gobj, 0);
        let top = world_position(&mut self.game.objs, self.data, root, None);
        let hip = self.part_joint(gobj, HIP_N);
        let hip = world_position(&mut self.game.objs, self.data, hip, None);
        let fp = &mut self.game.fighters[f.0];
        let sink = fp::fdivs(
            fp::fsubs(reg(hip.y), reg(top.y)),
            fp::frsp(f64::from(frames)),
        );
        fp.set_mv_f32(BURY_SINK, sink);
        let pos = fp.cur_pos;
        set_mv_vec(fp, BURY_TRANSLATE, pos);
        let scale = fp.x34_scale.y;
        super::super::super::ef::spawn(
            self,
            EFFECT,
            Some(gobj),
            &[
                super::super::super::ef::Arg::Vec(pos),
                super::super::super::ef::Arg::Float(scale),
            ],
        );
        self.game.fighters[f.0].x2219_b0 = true;
    }

    /// The mash out of the floor (`ftCo_Bury_Anim`, `ftCo_BuryWait_Anim`): the timer runs
    /// down by `ftCommonData` x610 and mashing by x614; at its end, the jump free.
    fn bury_mash(&mut self, gobj: GObjId) {
        let c = self.common();
        let (dec, mash) = (reg(c.f32(0x610)), c.f32(0x614));
        let fp = self.game.fighter_mut(gobj);
        fp.grab_timer = mem(fp::fsubs(reg(fp.grab_timer), dec));
        let _ = self.grab_mash(gobj, mash);
        if self.game.fighter(gobj).grab_timer <= 0.0 {
            self.enter_bury_jump(gobj);
        }
    }

    /// `ftCo_Bury_Anim`: the mash, and the wait once the sinking's frames run out.
    #[doc(alias = "ftCo_Bury_Anim")]
    pub fn co_bury_anim(&mut self, gobj: GObjId) {
        self.bury_mash(gobj);
        let fp = self.game.fighter_mut(gobj);
        let left = fp.mv_i32(BURY_FRAMES).wrapping_sub(1);
        fp.set_mv_i32(BURY_FRAMES, left);
        if left == 0 {
            self.enter_bury_wait(gobj);
        }
    }

    /// The buried states' empty callbacks.
    #[doc(alias = "ftCo_Bury_IASA")]
    #[doc(alias = "ftCo_BuryWait_IASA")]
    #[doc(alias = "ftCo_BuryWait_Phys")]
    pub fn co_bury_nothing(&mut self, _gobj: GObjId) {}

    /// The bury effect follows the floor (`ftCo_800C0FCC`, its update's callback): tilted with
    /// the floor's normal and carried with its motion, while the floor is live.
    #[doc(alias = "ftCo_800C0FCC")]
    pub fn bury_effect_follow(&mut self, effect: GObjId, gobj: GObjId) {
        let f = self.game.fighter_id(gobj);
        let floor = self.game.fighters[f.0].mv_i32(BURY_FLOOR);
        let g = self.game.geometry.as_ref().expect("no collision geometry");
        if !g.line_live(floor) {
            return;
        }
        let j = match self.game.world.gobjs[effect.0].obj {
            Some(Obj::JObj(j)) => j,
            other => panic!("the bury effect's GObj holds {other:?}, not a joint"),
        };
        let normal = g.line_normal(floor);
        let fp = &self.game.fighters[f.0];
        let speed = g.line_speed(fp.coll_data.floor.index, mv_vec(fp, BURY_TRANSLATE));
        let z = crate::math::atan2(mem(fp::fneg(reg(normal.x))), normal.y);
        jobj::set_rotation_z(&mut self.game.objs.jobjs, j, z);
        if let Some(offset) = speed {
            let fp = &mut self.game.fighters[f.0];
            let t = mv_vec(fp, BURY_TRANSLATE).sdk_add(offset);
            set_mv_vec(fp, BURY_TRANSLATE, t);
            jobj::set_translate(&mut self.game.objs.jobjs, j, t);
        }
    }

    /// `ftCo_Bury_Phys`: the box's bottom sinks by `x1C` a frame, up to 1 below its sides.
    #[doc(alias = "ftCo_Bury_Phys")]
    pub fn co_bury_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let mut b = mv_box(fp, BURY_BOX);
        let low = if b.right.y < b.left.y {
            reg(b.right.y)
        } else {
            reg(b.left.y)
        };
        b.bottom = mem(fp::fadds(reg(b.bottom), reg(fp.mv_f32(BURY_SINK))));
        if fp::fadds(1.0, reg(b.bottom)) > low {
            b.bottom = mem(fp::fsubs(low, 1.0));
        }
        set_mv_box(fp, BURY_BOX, &b);
    }

    /// `ftCo_Bury_Coll` (`ftCo_BuryWait_Coll`): off its floor, on another, or on a floor the
    /// stage makes a hazard of, the jump free.
    #[doc(alias = "ftCo_Bury_Coll")]
    #[doc(alias = "ftCo_BuryWait_Coll")]
    pub fn co_bury_coll(&mut self, gobj: GObjId) {
        let b = mv_box(self.game.fighter(gobj), BURY_BOX);
        let floor = self.game.fighter(gobj).mv_i32(BURY_FLOOR);
        if !self.ground_coll_box(gobj, &b)
            || floor != self.game.fighter(gobj).coll_data.floor.index
            || self.line_hazard(floor)
        {
            self.enter_bury_jump(gobj);
        }
    }

    /// The buried wait, `ftCo_800C124C`.
    #[doc(alias = "ftCo_800C124C")]
    pub fn enter_bury_wait(&mut self, gobj: GObjId) {
        self.change_motion_state(gobj, ms::BURY_WAIT, 0x1082, 0.0, 1.0, 0.0, None);
        self.stop_dead(gobj);
        let fp = self.game.fighter_mut(gobj);
        fp.x1A6A = 0x1FF;
        fp.x221D_b5 = true;
        fp.x2220_b3 = true;
        fp.x2224_b4 = true;
    }

    /// `ftCo_BuryWait_Anim`: the mash.
    #[doc(alias = "ftCo_BuryWait_Anim")]
    pub fn co_bury_wait_anim(&mut self, gobj: GObjId) {
        self.bury_mash(gobj);
    }

    /// The jump free, `ftCo_800C13BC` (with the hammer, its own, not ported): airborne,
    /// straight up at `ftCommonData` x618, briefly intangible (x620).
    #[doc(alias = "ftCo_800C13BC")]
    pub fn enter_bury_jump(&mut self, gobj: GObjId) {
        if self.holding_hammer(gobj) {
            unported_call("ftCo_800C5A98 (a hammer jump out of the floor)");
        }
        let f = self.game.fighter_id(gobj);
        self.go_airborne(f);
        let c = self.common();
        let (up, frames) = (c.f32(0x618), c.i32(0x620));
        let fp = &mut self.game.fighters[f.0];
        fp.self_vel.x = 0.0;
        fp.self_vel.y = up;
        fp.set_mv_f32(BURY_JUMP_FRAMES, 0.0);
        self.change_motion_state(gobj, ms::BURY_JUMP, 0, 0.0, 1.0, 0.0, None);
        self.game.fighters[f.0].x1A6A = 0x1FF;
        self.respawn_invincibility(gobj, frames);
    }

    /// `ftCo_BuryJump_Anim`: counting the jump's frames; a fall at its end.
    #[doc(alias = "ftCo_BuryJump_Anim")]
    pub fn co_bury_jump_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let n = fp::fadds(reg(fp.mv_f32(BURY_JUMP_FRAMES)), 1.0);
        fp.set_mv_f32(BURY_JUMP_FRAMES, n);
        if !self.anim_frames_remaining(gobj) {
            self.enter_fall(gobj);
        }
    }

    /// `ftCo_BuryJump_IASA`: after `ftCommonData` x61C frames, the aerial actions.
    #[doc(alias = "ftCo_BuryJump_IASA")]
    pub fn co_bury_jump_iasa(&mut self, gobj: GObjId) {
        let after = self.common().f32(0x61C);
        if self.game.fighter(gobj).mv_f32(BURY_JUMP_FRAMES) >= after {
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

    /// `ftCo_BuryJump_Phys`: gravity and drift.
    #[doc(alias = "ftCo_BuryJump_Phys")]
    pub fn co_bury_jump_phys(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        let (g, t) = (reg(fp.co_attrs.gravity), reg(fp.co_attrs.terminal_velocity));
        Self::fall(fp, g, t);
        self.air_drift(gobj);
    }

    /// `ftCo_BuryJump_Coll`: rising, landing as a jump does; falling, as a hit airborne does.
    #[doc(alias = "ftCo_BuryJump_Coll")]
    pub fn co_bury_jump_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).self_vel.y >= 0.0 {
            self.air_coll_unlocked_land(gobj);
        } else {
            self.co_air_catch_hit_coll(gobj);
        }
    }
}

/// Runs a buried state's callback, if `cb` is one.
pub fn run(g: &mut Running<'_>, gobj: GObjId, cb: u32) -> bool {
    match cb {
        0x800C_0F34 => g.co_bury_anim(gobj),
        0x800C_0FC8 | 0x800C_134C | 0x800C_1350 => g.co_bury_nothing(gobj),
        0x800C_1194 => g.co_bury_phys(gobj),
        0x800C_11E4 | 0x800C_1354 => g.co_bury_coll(gobj),
        0x800C_12D8 => g.co_bury_wait_anim(gobj),
        0x800C_1460 => g.co_bury_jump_anim(gobj),
        0x800C_14B0 => g.co_bury_jump_iasa(gobj),
        0x800C_1574 => g.co_bury_jump_phys(gobj),
        0x800C_15B4 => g.co_bury_jump_coll(gobj),
        _ => return false,
    }
    true
}

/// The buried states' functions other code calls by address: the bury effect's follow.
pub fn call(r: &mut Running, f: u32, args: &[Val]) -> Option<Val> {
    match f {
        0x800C_0FCC => {
            let effect = args[0].gobj().expect("a bury effect without a GObj");
            let gobj = args[1].gobj().expect("a bury effect without a fighter");
            r.bury_effect_follow(effect, gobj);
            Some(Val::None)
        }
        _ => None,
    }
}
