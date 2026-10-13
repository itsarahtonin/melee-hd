//! Bounces off walls and ceilings (`ftCo_FlyReflect.c`): a fighter tumbling fast enough into a
//! wall or the ceiling it hugs bounces off it (`FlyReflectWall`, `FlyReflectCeil`), its
//! knockback mirrored and slowed, once per surface until it touches another.

use gekko_fp as fp;

use super::super::super::camera;
use super::super::super::ef;
use super::super::super::state::Running;
use super::down::DOWN_BOUND_SFX;
use super::ids::ms;
use crate::fpu::{mem, reg};
use crate::hsd::gobj::GObjId;
use crate::math::{self, sqrt_reg};
use crate::mp::coll::{CEILING_HUG, LEFT_WALL_HUG, RIGHT_WALL_HUG};
use crate::vec::Vec3;

/// `ftCommon_MotionVars_damage`'s `x18` (frames before the bounce's techs and another bounce)
/// and `x19` (the surface last bounced off, `ftCo_Surface`).
const DAMAGE_X18: usize = 0x18;
const DAMAGE_X19: usize = 0x19;

/// `ftCo_Surface_LeftWall`, `ftCo_Surface_RightWall`, `ftCo_Surface_Ceiling`.
const SURFACE_LEFT_WALL: u8 = 1;
const SURFACE_RIGHT_WALL: u8 = 2;
const SURFACE_CEILING: u8 = 3;

/// `QuakeKind_Small`.
const QUAKE_SMALL: i32 = 2;

impl Running<'_> {
    /// A bounce off the wall the fighter hugs, `ftCo_800C15F4`: knockback into it past
    /// `ftCommonData` x1B0, unless it last bounced off that wall.
    #[doc(alias = "ftCo_800C15F4")]
    pub fn fly_reflect_wall_check(&mut self, gobj: GObjId) -> bool {
        let threshold = reg(self.common().f32(0x1B0));
        let fp = self.game.fighter(gobj);
        let c = &fp.coll_data;
        let kb_x = reg(fp.x8c_kb_vel.x);
        let last = fp.mv[DAMAGE_X19];
        if kb_x < fp::fneg(threshold)
            && c.env_flags & RIGHT_WALL_HUG != 0
            && last != SURFACE_LEFT_WALL
        {
            let v = Vec3::new(c.ecb.left.x, c.ecb.left.y, 0.0);
            let normal = c.right_facing_wall.normal;
            self.kirby_bump_effect(gobj, v);
            self.fly_reflect(gobj, ms::FLY_REFLECT_WALL, normal, v);
            self.game.fighter_mut(gobj).mv[DAMAGE_X19] = SURFACE_LEFT_WALL;
            return true;
        }
        let fp = self.game.fighter(gobj);
        let c = &fp.coll_data;
        if kb_x > threshold && c.env_flags & LEFT_WALL_HUG != 0 && last != SURFACE_RIGHT_WALL {
            let v = Vec3::new(c.ecb.right.x, c.ecb.right.y, 0.0);
            let normal = c.left_facing_wall.normal;
            self.kirby_bump_effect(gobj, v);
            self.fly_reflect(gobj, ms::FLY_REFLECT_WALL, normal, v);
            self.game.fighter_mut(gobj).mv[DAMAGE_X19] = SURFACE_RIGHT_WALL;
            return true;
        }
        false
    }

    /// A bounce off the ceiling the fighter hugs, `ftCo_800C1718`: rising knockback past
    /// `ftCommonData` x1B0, unless it last bounced off the ceiling.
    #[doc(alias = "ftCo_800C1718")]
    pub fn fly_reflect_ceil_check(&mut self, gobj: GObjId) -> bool {
        let threshold = reg(self.common().f32(0x1B0));
        let fp = self.game.fighter(gobj);
        let c = &fp.coll_data;
        if reg(fp.x8c_kb_vel.y) > threshold
            && c.env_flags & CEILING_HUG != 0
            && fp.mv[DAMAGE_X19] != SURFACE_CEILING
        {
            let v = Vec3::new(0.0, c.ecb.top.y, 0.0);
            let normal = c.ceiling.normal;
            self.kirby_bump_effect(gobj, v);
            self.fly_reflect(gobj, ms::FLY_REFLECT_CEIL, normal, v);
            self.game.fighter_mut(gobj).mv[DAMAGE_X19] = SURFACE_CEILING;
            return true;
        }
        false
    }

    /// A bounce off a wall, else the ceiling, `ftCo_800C17CC`.
    #[doc(alias = "ftCo_800C17CC")]
    pub fn fly_reflect_check(&mut self, gobj: GObjId) -> bool {
        self.fly_reflect_wall_check(gobj) || self.fly_reflect_ceil_check(gobj)
    }

    /// Bouncing off a surface, `ftCo_800C18A8`: an effect and a small quake where the ECB
    /// meets it, the knockback (with the fighter's own speed) mirrored across the surface's
    /// normal and slowed by `ftCommonData` x1BC, facing along it, the state, the fighter set
    /// against the surface by its TransN, a short wait before techs, a rumble, intangibility
    /// and the impact's sound.
    #[doc(alias = "ftCo_800C18A8")]
    pub fn fly_reflect(&mut self, gobj: GObjId, msid: i32, normal: Vec3, offset: Vec3) {
        let f = self.game.fighter_id(gobj);
        let fp = &self.game.fighters[f.0];
        let add = |a: f32, b: f32| mem(fp::fadds(reg(a), reg(b)));
        let pos = Vec3::new(
            add(fp.cur_pos.x, offset.x),
            add(fp.cur_pos.y, offset.y),
            add(fp.cur_pos.z, offset.z),
        );
        let param = math::atan2(mem(fp::fneg(reg(normal.x))), normal.y);
        self.fighter_effect(
            gobj,
            ef::SpawnKind::PosOffsetParam,
            0x406,
            None,
            &[ef::Arg::Vec(pos), ef::Arg::Float(param)],
        );
        camera::request_quake(self, QUAKE_SMALL, Some(pos));
        let slow = reg(self.common().f32(0x1BC));
        let fp = &mut self.game.fighters[f.0];
        let mut v = fp.self_vel.add_xy(fp.x8c_kb_vel);
        v.mirror_xy(normal);
        v.x = mem(fp::fmuls(reg(v.x), slow));
        v.y = mem(fp::fmuls(reg(v.y), slow));
        fp.x8c_kb_vel = v;
        fp.self_vel.z = 0.0;
        fp.self_vel.y = 0.0;
        fp.self_vel.x = 0.0;
        fp.facing_dir = if reg(fp.x8c_kb_vel.x) < 0.0 {
            -1.0
        } else {
            1.0
        };
        self.change_motion_state(
            gobj,
            msid,
            0x40 | 0x8000 | 0x1_0000 | 0x1000_0000,
            0.0,
            1.0,
            0.0,
            None,
        );
        let fp = &mut self.game.fighters[f.0];
        if msid == ms::FLY_REFLECT_WALL {
            let x = fp::fadds(reg(fp.cur_pos.x), reg(offset.x));
            fp.cur_pos.x = mem(fp::fnmsubs(
                reg(fp.x68C_transNPos.z),
                fp::fneg(reg(fp.facing_dir)),
                x,
            ));
            self.air_coll_ledges_snap_low(gobj);
        } else {
            let y = fp::fadds(reg(fp.cur_pos.y), reg(offset.y));
            fp.cur_pos.y = mem(fp::fadds(reg(fp.x68C_transNPos.y), y));
            self.air_coll_ledges_snap_unlocked(gobj);
        }
        let wait = fp::fctiwz(reg(self.common().f32(0x1C0))) as u8;
        self.game.fighters[f.0].mv[DAMAGE_X18] = wait;
        self.start_rumble(f, 7, 0);
        let frames = self.common().i32(0x1B8);
        self.set_intangible(gobj, frames);
        let fp = &self.game.fighters[f.0];
        let vx = fp::fadds(reg(fp.self_vel.x), reg(fp.x8c_kb_vel.x));
        let vy = fp::fadds(reg(fp.self_vel.y), reg(fp.x8c_kb_vel.y));
        let mag = sqrt_reg(fp::fmadds(vx, vx, fp::fmuls(vy, vy)));
        let dist = mem(fp::fmuls(mag, reg(fp.co_attrs.weight)));
        self.impact_sfx(gobj, &DOWN_BOUND_SFX, dist);
    }

    /// `ftCo_FlyReflect_Anim`: the wait before techs runs down, then as the tumble.
    #[doc(alias = "ftCo_FlyReflect_Anim")]
    pub fn co_fly_reflect_anim(&mut self, gobj: GObjId) {
        let fp = self.game.fighter_mut(gobj);
        if fp.mv[DAMAGE_X18] != 0 {
            fp.mv[DAMAGE_X18] -= 1;
        }
        self.co_damage_fly_anim(gobj);
    }

    /// `ftCo_FlyReflect_IASA`: as the tumble.
    #[doc(alias = "ftCo_FlyReflect_IASA")]
    pub fn co_fly_reflect_iasa(&mut self, gobj: GObjId) {
        self.co_damage_fly_iasa(gobj);
    }

    /// `ftCo_FlyReflect_Phys`: gravity or a fast fall, and air drift (`ft_80084DB0`).
    #[doc(alias = "ftCo_FlyReflect_Phys")]
    pub fn co_fly_reflect_phys(&mut self, gobj: GObjId) {
        self.air_physics(gobj);
    }

    /// `ftCo_FlyReflect_Coll`: landing into a tech or a knockdown; off a wall, a ceiling tech
    /// or bounce at once, then once the wait is over a wall tech or another bounce; off the
    /// ceiling, a wall tech or a wall bounce.
    #[doc(alias = "ftCo_FlyReflect_Coll")]
    pub fn co_fly_reflect_coll(&mut self, gobj: GObjId) {
        if self.game.fighter(gobj).motion_id == ms::FLY_REFLECT_WALL {
            if self.air_coll_ledges_snap_low(gobj) {
                self.tumble_land(gobj);
                return;
            }
            if self.passive_ceil_check(gobj) || self.fly_reflect_ceil_check(gobj) {
                return;
            }
            if self.game.fighter(gobj).mv[DAMAGE_X18] == 0 {
                let _ = self.passive_wall_check(gobj)
                    || self.fly_reflect_wall_check(gobj)
                    || self.fly_reflect_ceil_check(gobj);
            }
        } else {
            if self.air_coll_ledges_snap_unlocked(gobj) {
                self.tumble_land(gobj);
                return;
            }
            let _ = self.passive_wall_check(gobj) || self.fly_reflect_wall_check(gobj);
        }
    }
}
